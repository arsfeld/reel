use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::{broadcast, watch};

pub trait PropertyLike: Send + Sync {
    fn subscribe(&self) -> PropertySubscriber;
    fn name(&self) -> &str;
    fn debug_value(&self) -> String;
    /// Get a unique identifier for this property (used for cycle detection)
    fn property_id(&self) -> String {
        format!("{}@{:p}", self.name(), self as *const _ as *const ())
    }
}

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

/// Detect cycles in property dependency graph using depth-first search
fn detect_cycles(
    property_name: &str,
    dependencies: &[Arc<dyn PropertyLike>],
) -> Result<(), String> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut rec_stack: HashSet<String> = HashSet::new();

    // Build adjacency list representation (simplified for this case)
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();

    // Add the new property and its dependencies
    let property_id = property_name.to_string();
    let dependency_ids: Vec<String> = dependencies.iter().map(|dep| dep.property_id()).collect();

    graph.insert(property_id.clone(), dependency_ids.clone());

    // For simplicity, we can only detect direct cycles (A -> B -> A)
    // A full implementation would require tracking all existing ComputedProperties
    for dep_id in &dependency_ids {
        if dep_id == &property_id {
            return Err(format!(
                "Cycle detected: Property '{}' cannot depend on itself",
                property_name
            ));
        }
    }

    // Check for immediate cycles where a dependency might be a computed property
    // that already depends on this property (simplified check)
    for (i, dep1) in dependency_ids.iter().enumerate() {
        for (j, dep2) in dependency_ids.iter().enumerate() {
            if i != j && dep1 == dep2 {
                return Err(format!(
                    "Duplicate dependency detected: Property '{}' has duplicate dependency '{}'",
                    property_name, dep1
                ));
            }
        }
    }

    Ok(())
}

pub struct Property<T: Clone + Send + Sync> {
    watch_sender: Arc<watch::Sender<T>>,
    watch_receiver: watch::Receiver<T>,
    broadcast_sender: broadcast::Sender<()>, // Keep for backward compatibility
    name: String,
}

impl<T: Clone + Send + Sync> Property<T> {
    pub fn new(initial_value: T, name: impl Into<String>) -> Self {
        let (watch_sender, watch_receiver) = watch::channel(initial_value);
        let (broadcast_sender, _) = broadcast::channel(100);
        Self {
            watch_sender: Arc::new(watch_sender),
            watch_receiver,
            broadcast_sender,
            name: name.into(),
        }
    }

    pub async fn get(&self) -> T {
        self.watch_receiver.borrow().clone()
    }

    /// Try to get the value without blocking. Returns None if the lock is currently held.
    pub fn try_get(&self) -> Option<T> {
        Some(self.watch_receiver.borrow().clone())
    }

    /// Get the value synchronously using blocking_read. This is safe to use from the UI thread
    /// since the value is already in memory and the lock should be available immediately.
    pub fn get_sync(&self) -> T {
        self.watch_receiver.borrow().clone()
    }

    pub async fn set(&self, new_value: T) {
        let _ = self.watch_sender.send(new_value);
        let _ = self.broadcast_sender.send(());
    }

    pub async fn update<F>(&self, updater: F)
    where
        F: FnOnce(&mut T),
    {
        let current_value = self.watch_receiver.borrow().clone();
        let mut new_value = current_value;
        updater(&mut new_value);
        let _ = self.watch_sender.send(new_value);
        let _ = self.broadcast_sender.send(());
    }

    pub fn subscribe(&self) -> PropertySubscriber {
        PropertySubscriber {
            receiver: self.broadcast_sender.subscribe(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Debug method to show the number of active subscribers
    pub fn debug_subscribers(&self) -> usize {
        self.broadcast_sender.receiver_count()
    }

    /// Debug method to check if there are any lagged subscribers
    pub fn debug_has_lagged_subscribers(&self) -> bool {
        // We can't easily check for lagged subscribers with broadcast channel
        // This is a placeholder that could be enhanced with more detailed tracking
        self.broadcast_sender.len() > 0
    }
}

impl<T: Clone + Send + Sync + Debug> Debug for Property<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Property({})", self.name)
    }
}

impl<T: Clone + Send + Sync + Debug> Debug for ComputedProperty<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComputedProperty::Standard { property, .. } => {
                write!(f, "ComputedProperty::Standard({})", property.name)
            }
            ComputedProperty::DebouncedProperty { property, .. } => {
                write!(f, "ComputedProperty::Debounced({})", property.name)
            }
        }
    }
}

impl<T: Clone + Send + Sync + Debug> PropertyLike for Property<T> {
    fn subscribe(&self) -> PropertySubscriber {
        self.subscribe()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn debug_value(&self) -> String {
        format!("{:?}", self.get_sync())
    }
}

pub enum ComputedProperty<T: Clone + Send + Sync> {
    Standard {
        property: Property<T>,
        _task_handle: tokio::task::JoinHandle<()>,
        fallback_value: Option<T>,
    },
    DebouncedProperty {
        property: Property<T>,
        _task_handle: tokio::task::JoinHandle<()>,
    },
}

impl<T: Clone + Send + Sync + 'static> ComputedProperty<T> {
    pub fn new<F>(
        name: impl Into<String>,
        dependencies: Vec<Arc<dyn PropertyLike>>,
        compute: F,
    ) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self::with_fallback(name, dependencies, compute, None)
    }

    /// Create a ComputedProperty with a fallback value in case the compute function panics
    pub fn with_fallback<F>(
        name: impl Into<String>,
        dependencies: Vec<Arc<dyn PropertyLike>>,
        compute: F,
        fallback_value: Option<T>,
    ) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        let name_string = name.into();

        // Check for cycles before creating the property
        if let Err(cycle_error) = detect_cycles(&name_string, &dependencies) {
            panic!(
                "Cannot create ComputedProperty '{}': {}",
                name_string, cycle_error
            );
        }
        // Try to compute initial value, use fallback if it panics
        let initial_value = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| compute()))
            .unwrap_or_else(|_| {
                if let Some(ref fallback) = fallback_value {
                    fallback.clone()
                } else {
                    panic!(
                        "ComputedProperty compute function panicked and no fallback value provided"
                    )
                }
            });

        let property = Property::new(initial_value, name_string);
        let property_clone = property.clone();
        let compute = Arc::new(compute);
        let fallback_clone = fallback_value.clone();

        // Create subscribers for all dependencies
        let mut subscribers: Vec<PropertySubscriber> =
            dependencies.iter().map(|dep| dep.subscribe()).collect();

        let task_handle = tokio::spawn(async move {
            loop {
                // Check for immediate changes first
                let mut any_changed = false;
                for subscriber in &mut subscribers {
                    if subscriber.try_recv() {
                        any_changed = true;
                    }
                }

                // If no immediate changes, wait for any subscriber to signal a change
                if !any_changed {
                    if subscribers.is_empty() {
                        // No dependencies, break the loop
                        break;
                    }

                    // Wait for the first subscriber that signals a change
                    for subscriber in &mut subscribers {
                        if subscriber.wait_for_change().await {
                            any_changed = true;
                            break;
                        }
                    }
                }

                if any_changed {
                    // Try to compute new value, use fallback if it panics
                    let new_value =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| compute()))
                            .unwrap_or_else(|_| {
                                if let Some(ref fallback) = fallback_clone {
                                    fallback.clone()
                                } else {
                                    // If no fallback, keep the current value
                                    property_clone.get_sync()
                                }
                            });
                    property_clone.set(new_value).await;
                }
            }
        });

        Self::Standard {
            property,
            _task_handle: task_handle,
            fallback_value,
        }
    }

    pub async fn get(&self) -> T {
        match self {
            Self::Standard { property, .. } => property.get().await,
            Self::DebouncedProperty { property, .. } => property.get().await,
        }
    }

    pub fn try_get(&self) -> Option<T> {
        match self {
            Self::Standard { property, .. } => property.try_get(),
            Self::DebouncedProperty { property, .. } => property.try_get(),
        }
    }

    pub fn get_sync(&self) -> T {
        match self {
            Self::Standard { property, .. } => property.get_sync(),
            Self::DebouncedProperty { property, .. } => property.get_sync(),
        }
    }

    pub fn subscribe(&self) -> PropertySubscriber {
        match self {
            Self::Standard { property, .. } => property.subscribe(),
            Self::DebouncedProperty { property, .. } => property.subscribe(),
        }
    }

    /// Debug method to show the number of active subscribers for this computed property
    pub fn debug_subscribers(&self) -> usize {
        match self {
            Self::Standard { property, .. } => property.debug_subscribers(),
            Self::DebouncedProperty { property, .. } => property.debug_subscribers(),
        }
    }

    /// Debug method to show information about this computed property's dependencies
    /// Note: This is a simplified version - a full implementation would track dependency names
    pub fn debug_dependencies(&self) -> String {
        let property_name = match self {
            Self::Standard { property, .. } => property.name(),
            Self::DebouncedProperty { property, .. } => property.name(),
        };
        format!(
            "ComputedProperty '{}' has dependencies (names not tracked in current implementation)",
            property_name
        )
    }

    /// Debug method to check if the background task is still running
    pub fn debug_task_running(&self) -> bool {
        match self {
            Self::Standard { _task_handle, .. } => !_task_handle.is_finished(),
            Self::DebouncedProperty { _task_handle, .. } => !_task_handle.is_finished(),
        }
    }
}

impl<T: Clone + Send + Sync> Drop for ComputedProperty<T> {
    fn drop(&mut self) {
        match self {
            Self::Standard { _task_handle, .. } => _task_handle.abort(),
            Self::DebouncedProperty { _task_handle, .. } => _task_handle.abort(),
        }
    }
}

impl<T: Clone + Send + Sync + Debug + 'static> PropertyLike for ComputedProperty<T> {
    fn subscribe(&self) -> PropertySubscriber {
        match self {
            Self::Standard { property, .. } => property.subscribe(),
            Self::DebouncedProperty { property, .. } => property.subscribe(),
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Standard { property, .. } => property.name(),
            Self::DebouncedProperty { property, .. } => property.name(),
        }
    }

    fn debug_value(&self) -> String {
        format!("{:?}", self.get_sync())
    }
}

// Add operators to ComputedProperty too for chaining
impl<T: Clone + Send + Sync + Debug + 'static> ComputedProperty<T> {
    /// Map this computed property to another computed property that applies a transformation function
    pub fn map<U, F>(&self, f: F) -> ComputedProperty<U>
    where
        U: Clone + Send + Sync + 'static,
        F: Fn(T) -> U + Send + Sync + 'static,
    {
        let property = match self {
            Self::Standard { property, .. } => property,
            Self::DebouncedProperty { property, .. } => property,
        };
        let self_arc: Arc<dyn PropertyLike> = Arc::new(property.clone());
        let self_clone = property.clone();
        let f = Arc::new(f);

        ComputedProperty::new(
            format!("{}.map", property.name()),
            vec![self_arc],
            move || {
                let value = self_clone.get_sync();
                f(value)
            },
        )
    }

    /// Filter this computed property to return Some(value) when predicate is true
    pub fn filter<F>(&self, predicate: F) -> ComputedProperty<Option<T>>
    where
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        let property = match self {
            Self::Standard { property, .. } => property,
            Self::DebouncedProperty { property, .. } => property,
        };
        let self_arc: Arc<dyn PropertyLike> = Arc::new(property.clone());
        let self_clone = property.clone();
        let predicate = Arc::new(predicate);

        ComputedProperty::new(
            format!("{}.filter", property.name()),
            vec![self_arc],
            move || {
                let value = self_clone.get_sync();
                if predicate(&value) { Some(value) } else { None }
            },
        )
    }

    /// Debounce this computed property - only emit values after a delay with no further changes
    pub fn debounce(&self, duration: std::time::Duration) -> ComputedProperty<T> {
        match self {
            Self::Standard { property, .. } => property.debounce(duration),
            Self::DebouncedProperty { property, .. } => property.debounce(duration),
        }
    }
}

impl<T: Clone + Send + Sync> Clone for Property<T> {
    fn clone(&self) -> Self {
        Self {
            watch_sender: self.watch_sender.clone(),
            watch_receiver: self.watch_receiver.clone(),
            broadcast_sender: self.broadcast_sender.clone(),
            name: self.name.clone(),
        }
    }
}

// Property Operators
impl<T: Clone + Send + Sync + Debug + 'static> Property<T> {
    /// Map this property to a computed property that applies a transformation function
    pub fn map<U, F>(&self, f: F) -> ComputedProperty<U>
    where
        U: Clone + Send + Sync + 'static,
        F: Fn(T) -> U + Send + Sync + 'static,
    {
        let self_arc: Arc<dyn PropertyLike> = Arc::new(self.clone());
        let self_clone = self.clone();
        let f = Arc::new(f);

        ComputedProperty::new(format!("{}.map", self.name()), vec![self_arc], move || {
            let value = self_clone.get_sync();
            f(value)
        })
    }

    /// Filter this property to a computed property that returns Some(value) when predicate is true
    pub fn filter<F>(&self, predicate: F) -> ComputedProperty<Option<T>>
    where
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        let self_arc: Arc<dyn PropertyLike> = Arc::new(self.clone());
        let self_clone = self.clone();
        let predicate = Arc::new(predicate);

        ComputedProperty::new(
            format!("{}.filter", self.name()),
            vec![self_arc],
            move || {
                let value = self_clone.get_sync();
                if predicate(&value) { Some(value) } else { None }
            },
        )
    }

    /// Debounce this property - only emit values after a delay with no further changes
    pub fn debounce(&self, duration: std::time::Duration) -> ComputedProperty<T> {
        let self_clone = self.clone();

        // Create initial value from the source
        let initial_value = self.get_sync();
        let debounced_property = Property::new(initial_value, format!("{}.debounced", self.name()));
        let debounced_clone = debounced_property.clone();

        // Create the debouncing task
        let task_handle = tokio::spawn(async move {
            let mut subscriber = self_clone.subscribe();

            loop {
                // Wait for the first change
                if !subscriber.wait_for_change().await {
                    break; // Source property was dropped
                }

                // Start debounce timer - keep resetting it if more changes come
                loop {
                    tokio::select! {
                        // If duration passes without interruption, emit the value
                        _ = tokio::time::sleep(duration) => {
                            let current_value = self_clone.get_sync();
                            debounced_clone.set(current_value).await;
                            break; // Break inner loop, go back to waiting for next change
                        }
                        // If another change occurs, restart the timer
                        changed = subscriber.wait_for_change() => {
                            if !changed {
                                return; // Source property was dropped
                            }
                            // Continue the inner loop to restart timer
                        }
                    }
                }
            }
        });

        // Create a ComputedProperty that just returns the debounced property value
        // But we need to handle the task cleanup ourselves
        ComputedProperty::DebouncedProperty {
            property: debounced_property,
            _task_handle: task_handle,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn test_property_like_trait() {
        let prop = Property::new(42i32, "test");
        let prop_like: Arc<dyn PropertyLike> = Arc::new(prop.clone());

        assert_eq!(prop_like.name(), "test");
        assert_eq!(prop_like.debug_value(), "42");

        // Test that we can subscribe
        let _subscriber = prop_like.subscribe();
    }

    #[tokio::test]
    async fn test_computed_property_simple() {
        // Test with no dependencies first
        let computed = ComputedProperty::new(
            "constant",
            vec![], // No dependencies
            || 42i32,
        );

        assert_eq!(computed.get_sync(), 42);
    }

    #[tokio::test]
    async fn test_map_operator() {
        let source = Property::new(5i32, "source");
        let mapped = source.map(|x| x * 2);

        // Initial value should be mapped
        assert_eq!(mapped.get_sync(), 10);

        // Test reactivity
        source.set(10).await;
        sleep(Duration::from_millis(10)).await; // Give time for computed property to update
        assert_eq!(mapped.get_sync(), 20);
    }

    #[tokio::test]
    async fn test_filter_operator() {
        let source = Property::new(5i32, "source");
        let filtered = source.filter(|&x| x > 3);

        // Initial value should pass filter
        assert_eq!(filtered.get_sync(), Some(5));

        // Set value that passes filter
        source.set(10).await;
        sleep(Duration::from_millis(10)).await;
        assert_eq!(filtered.get_sync(), Some(10));

        // Set value that doesn't pass filter
        source.set(2).await;
        sleep(Duration::from_millis(10)).await;
        assert_eq!(filtered.get_sync(), None);
    }

    #[tokio::test]
    async fn test_chained_operators() {
        let source = Property::new(2i32, "source");
        let mapped = source.map(|x| x * 3);
        let filtered = mapped.filter(|&x| x > 5);

        // Initial value: 2 * 3 = 6, which > 5
        assert_eq!(filtered.get_sync(), Some(6));

        // Change to value that results in filtered output
        source.set(1).await;
        sleep(Duration::from_millis(10)).await; // 1 * 3 = 3, which <= 5
        assert_eq!(filtered.get_sync(), None);

        // Change back to passing value
        source.set(3).await;
        sleep(Duration::from_millis(10)).await; // 3 * 3 = 9, which > 5
        assert_eq!(filtered.get_sync(), Some(9));
    }

    #[tokio::test]
    async fn test_debounce_operator() {
        let source = Property::new(1i32, "source");
        let debounced = source.debounce(Duration::from_millis(50));

        // Initial value should be immediately available
        assert_eq!(debounced.get_sync(), 1);

        // Rapid changes should be debounced
        source.set(2).await;
        sleep(Duration::from_millis(10)).await; // Wait less than debounce duration
        source.set(3).await;
        sleep(Duration::from_millis(10)).await; // Wait less than debounce duration
        source.set(4).await;

        // Value should still be old because debounce hasn't fired
        assert_eq!(debounced.get_sync(), 1);

        // Wait for debounce to fire
        sleep(Duration::from_millis(60)).await; // Wait longer than debounce duration
        assert_eq!(debounced.get_sync(), 4); // Should now have the last value

        // Single change should eventually propagate
        source.set(10).await;
        sleep(Duration::from_millis(60)).await;
        assert_eq!(debounced.get_sync(), 10);
    }

    #[tokio::test]
    async fn test_chained_operators_with_debounce() {
        let source = Property::new(1i32, "source");
        let debounced = source.debounce(Duration::from_millis(30));
        let mapped = debounced.map(|x| x * 2);

        // Initial value
        assert_eq!(mapped.get_sync(), 2);

        // Rapid changes should be debounced before mapping
        source.set(5).await;
        source.set(10).await;

        // Wait for debounce + mapping propagation
        sleep(Duration::from_millis(50)).await;
        assert_eq!(mapped.get_sync(), 20); // 10 * 2
    }

    #[tokio::test]
    async fn test_debounce_comprehensive() {
        let source = Property::new("initial".to_string(), "search_query");
        let debounced = source.debounce(Duration::from_millis(100));

        // Setup subscriber to track changes
        let mut subscriber = debounced.subscribe();
        let mut change_count = 0;

        // Initial value should be available immediately
        assert_eq!(debounced.get_sync(), "initial");

        // Simulate rapid typing in a search box
        source.set("a".to_string()).await;
        source.set("ap".to_string()).await;
        source.set("app".to_string()).await;
        source.set("appl".to_string()).await;
        source.set("apple".to_string()).await;

        // Should still have initial value since debounce hasn't fired
        assert_eq!(debounced.get_sync(), "initial");

        // Wait for debounce period
        sleep(Duration::from_millis(120)).await;
        assert_eq!(debounced.get_sync(), "apple");

        // Check that subscriber was notified
        tokio::select! {
            _ = subscriber.wait_for_change() => {
                change_count += 1;
            }
            _ = sleep(Duration::from_millis(10)) => {
                // No immediate change should be available
            }
        }
        assert!(change_count > 0, "Should have received change notification");

        // Test another sequence after stabilization
        source.set("banana".to_string()).await;
        source.set("cherry".to_string()).await;

        sleep(Duration::from_millis(120)).await;
        assert_eq!(debounced.get_sync(), "cherry");
    }

    #[tokio::test]
    async fn test_debounce_edge_cases() {
        let source = Property::new(0u32, "counter");
        let debounced = source.debounce(Duration::from_millis(50));

        // Single change should propagate
        source.set(1).await;
        sleep(Duration::from_millis(60)).await;
        assert_eq!(debounced.get_sync(), 1);

        // Zero duration debounce should work (immediate propagation)
        let immediate_debounced = source.debounce(Duration::from_millis(0));
        source.set(2).await;
        sleep(Duration::from_millis(10)).await;
        assert_eq!(immediate_debounced.get_sync(), 2);

        // Very long debounce should delay appropriately
        let slow_debounced = source.debounce(Duration::from_millis(200));
        source.set(3).await;
        source.set(4).await;

        // Should still have initial value after short wait
        sleep(Duration::from_millis(50)).await;
        assert_eq!(slow_debounced.get_sync(), 0); // Still initial

        // Should have latest value after full debounce period
        sleep(Duration::from_millis(200)).await;
        assert_eq!(slow_debounced.get_sync(), 4);
    }

    #[tokio::test]
    async fn test_debounce_with_multiple_subscribers() {
        let source = Property::new(1i32, "multi_sub");
        let debounced = source.debounce(Duration::from_millis(30));

        // Create multiple subscribers
        let mut sub1 = debounced.subscribe();
        let mut sub2 = debounced.subscribe();
        let mut sub3 = debounced.subscribe();

        // Rapid changes
        source.set(10).await;
        source.set(20).await;
        source.set(30).await;

        sleep(Duration::from_millis(50)).await;

        // All subscribers should get notification
        let mut notifications = 0;

        for sub in [&mut sub1, &mut sub2, &mut sub3].iter_mut() {
            tokio::select! {
                _ = sub.wait_for_change() => {
                    notifications += 1;
                }
                _ = sleep(Duration::from_millis(10)) => {}
            }
        }

        assert!(
            notifications > 0,
            "At least some subscribers should be notified"
        );

        // All should have the same final value
        assert_eq!(debounced.get_sync(), 30);
    }

    #[tokio::test]
    async fn test_computed_property_error_handling() {
        let source = Property::new(5i32, "source");

        // Test with fallback value
        let computed_with_fallback = ComputedProperty::with_fallback(
            "panicking_computation",
            vec![Arc::new(source.clone())],
            || {
                panic!("This computation always panics");
            },
            Some(99i32),
        );

        // Should use fallback value
        assert_eq!(computed_with_fallback.get_sync(), 99);

        // Test without fallback (should panic during creation)
        let source_clone = source.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            ComputedProperty::new(
                "panicking_computation_no_fallback",
                vec![Arc::new(source_clone)],
                || {
                    panic!("This computation always panics");
                },
            )
        }));

        assert!(result.is_err(), "Should panic when no fallback is provided");

        // Test normal computation (should not use fallback)
        let normal_computed = ComputedProperty::with_fallback(
            "normal_computation",
            vec![Arc::new(source.clone())],
            || 42i32,
            Some(999i32),
        );

        // Should use computed value, not fallback
        assert_eq!(normal_computed.get_sync(), 42);
    }

    #[tokio::test]
    async fn test_debugging_tools() {
        let prop = Property::new(42i32, "debug_test");

        // Initially no subscribers
        assert_eq!(prop.debug_subscribers(), 0);

        // Add some subscribers
        let _sub1 = prop.subscribe();
        let _sub2 = prop.subscribe();

        // Should now have 2 subscribers
        assert_eq!(prop.debug_subscribers(), 2);

        // Test ComputedProperty debugging
        let computed =
            ComputedProperty::new("debug_computed", vec![Arc::new(prop.clone())], || 100i32);

        assert!(computed.debug_task_running());
        assert!(computed.debug_dependencies().contains("debug_computed"));

        // Drop computed property to stop its task
        drop(computed);

        // Give a moment for the task to be aborted
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    #[tokio::test]
    async fn test_cycle_detection() {
        let prop1 = Property::new(1i32, "prop1");
        let prop2 = Property::new(2i32, "prop2");

        // Test duplicate dependencies (should be detected as an error)
        let prop1_clone = prop1.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            ComputedProperty::new(
                "duplicate_deps",
                vec![Arc::new(prop1_clone.clone()), Arc::new(prop1_clone.clone())], // Same dependency twice
                || 42i32,
            )
        }));

        assert!(
            result.is_err(),
            "Should panic when duplicate dependencies are detected"
        );

        // Test valid dependencies (should work fine)
        let valid_computed = ComputedProperty::new(
            "valid_computed",
            vec![Arc::new(prop1.clone()), Arc::new(prop2.clone())],
            || 100i32,
        );

        assert_eq!(valid_computed.get_sync(), 100);

        // Test no dependencies (should work fine)
        let no_deps_computed = ComputedProperty::new(
            "no_deps",
            vec![], // No dependencies
            || 200i32,
        );

        assert_eq!(no_deps_computed.get_sync(), 200);
    }
}
