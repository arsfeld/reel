use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

pub struct PropertySubscriber {
    receiver: broadcast::Receiver<()>,
}

// PropertySubscriber intentionally does not implement Clone.
// Each subscriber should be unique to avoid conflicts.
// To get multiple subscribers, call Property::subscribe() multiple times.

impl PropertySubscriber {
    pub async fn wait_for_change(&mut self) -> bool {
        loop {
            match self.receiver.recv().await {
                Ok(_) => return true,
                // If we lagged behind, skip to the latest and keep waiting
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                // Channel closed: no more updates
                Err(broadcast::error::RecvError::Closed) => return false,
            }
        }
    }

    pub fn try_recv(&mut self) -> bool {
        match self.receiver.try_recv() {
            Ok(_) => true,
            Err(broadcast::error::TryRecvError::Empty) => false,
            // Consider lag as a change signal; the next recv() will align
            Err(broadcast::error::TryRecvError::Lagged(_)) => true,
            Err(broadcast::error::TryRecvError::Closed) => false,
        }
    }
}

pub struct Property<T: Clone + Send + Sync> {
    value: Arc<RwLock<T>>,
    sender: broadcast::Sender<()>,
    name: String,
}

impl<T: Clone + Send + Sync> Property<T> {
    pub fn new(initial_value: T, name: impl Into<String>) -> Self {
        let (sender, _) = broadcast::channel(100);
        Self {
            value: Arc::new(RwLock::new(initial_value)),
            sender,
            name: name.into(),
        }
    }

    pub async fn get(&self) -> T {
        self.value.read().await.clone()
    }

    pub async fn set(&self, new_value: T) {
        {
            let mut value = self.value.write().await;
            *value = new_value;
        }
        let _ = self.sender.send(());
    }

    pub async fn update<F>(&self, updater: F)
    where
        F: FnOnce(&mut T),
    {
        {
            let mut value = self.value.write().await;
            updater(&mut *value);
        }
        let _ = self.sender.send(());
    }

    pub fn subscribe(&self) -> PropertySubscriber {
        PropertySubscriber {
            receiver: self.sender.subscribe(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl<T: Clone + Send + Sync + Debug> Debug for Property<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Property({})", self.name)
    }
}

pub struct ComputedProperty<T: Clone + Send + Sync> {
    property: Property<T>,
}

impl<T: Clone + Send + Sync> ComputedProperty<T> {
    pub fn new<F>(
        name: impl Into<String>,
        dependencies: Vec<&Property<impl Clone + Send + Sync>>,
        compute: F,
    ) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
        T: 'static,
    {
        let initial_value = compute();
        let property = Property::new(initial_value, name);

        let property_clone = property.clone();
        let compute = Arc::new(compute);

        // Create subscribers from dependencies
        let mut subscribers = Vec::new();
        for dep in dependencies {
            subscribers.push(dep.subscribe());
        }

        tokio::spawn(async move {
            let mut deps = subscribers;
            loop {
                // Wait for any dependency to change
                let mut changed = false;
                for dep in &mut deps {
                    if dep.try_recv() {
                        changed = true;
                    }
                }

                if !changed {
                    // If no immediate changes, wait for next change
                    for dep in &mut deps {
                        if dep.wait_for_change().await {
                            changed = true;
                            break;
                        }
                    }
                }

                if changed {
                    let new_value = compute();
                    property_clone.set(new_value).await;
                }
            }
        });

        Self { property }
    }

    pub async fn get(&self) -> T {
        self.property.get().await
    }

    pub fn subscribe(&self) -> PropertySubscriber {
        self.property.subscribe()
    }
}

impl<T: Clone + Send + Sync> Clone for Property<T> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            sender: self.sender.clone(),
            name: self.name.clone(),
        }
    }
}
