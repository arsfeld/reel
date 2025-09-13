use super::{Property, PropertySubscriber, ViewModel};
use crate::events::{DatabaseEvent, EventBus, EventPayload, EventType};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug, PartialEq)]
pub struct PageState {
    pub name: String,
    pub title: String,
    pub header_config: HeaderConfig,
    pub can_go_back: bool,
}

impl PageState {
    pub fn new(name: String, title: String) -> Self {
        Self {
            name,
            title: title.clone(),
            header_config: HeaderConfig::default_with_title(title),
            can_go_back: false,
        }
    }

    pub fn with_header_config(mut self, config: HeaderConfig) -> Self {
        self.header_config = config;
        self
    }

    pub fn with_can_go_back(mut self, can_go_back: bool) -> Self {
        self.can_go_back = can_go_back;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeaderConfig {
    pub title: String,
    pub show_back_button: bool,
    pub show_home_button: bool,
    pub custom_title_widget: Option<String>, // Identifier for custom widgets
    pub additional_actions: Vec<HeaderAction>,
}

impl HeaderConfig {
    pub fn default_with_title(title: String) -> Self {
        Self {
            title,
            show_back_button: false,
            show_home_button: false,
            custom_title_widget: None,
            additional_actions: Vec::new(),
        }
    }

    pub fn with_back_button(mut self, show: bool) -> Self {
        self.show_back_button = show;
        self
    }

    pub fn with_home_button(mut self, show: bool) -> Self {
        self.show_home_button = show;
        self
    }

    pub fn with_custom_title_widget(mut self, widget_id: String) -> Self {
        self.custom_title_widget = Some(widget_id);
        self
    }

    pub fn with_action(mut self, action: HeaderAction) -> Self {
        self.additional_actions.push(action);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeaderAction {
    pub id: String,
    pub icon: String,
    pub tooltip: String,
    pub enabled: bool,
}

impl HeaderAction {
    pub fn new(id: String, icon: String, tooltip: String) -> Self {
        Self {
            id,
            icon,
            tooltip,
            enabled: true,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

#[derive(Clone, Debug)]
pub enum NavigationRequest {
    NavigateToPage(PageState),
    GoBack,
    GoHome,
    UpdatePageTitle(String),
    UpdateHeaderConfig(HeaderConfig),
}

pub struct NavigationViewModel {
    // Core navigation state
    current_page: Property<Option<PageState>>,
    navigation_stack: Property<Vec<PageState>>,
    can_go_back: Property<bool>,
    can_go_forward: Property<bool>,

    // Header state
    page_title: Property<String>,
    header_config: Property<HeaderConfig>,

    // Services
    event_bus: RwLock<Option<Arc<EventBus>>>,
}

impl std::fmt::Debug for NavigationViewModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NavigationViewModel")
            .field("current_page", &"Property<Option<PageState>>")
            .field("navigation_stack", &"Property<Vec<PageState>>")
            .field("can_go_back", &"Property<bool>")
            .field("can_go_forward", &"Property<bool>")
            .field("page_title", &"Property<String>")
            .field("header_config", &"Property<HeaderConfig>")
            .field("event_bus", &"Option<Arc<EventBus>>")
            .finish()
    }
}

impl NavigationViewModel {
    pub fn new() -> Self {
        let default_config = HeaderConfig::default_with_title("Reel".to_string());

        Self {
            current_page: Property::new(None, "current_page"),
            navigation_stack: Property::new(Vec::new(), "navigation_stack"),
            can_go_back: Property::new(false, "can_go_back"),
            can_go_forward: Property::new(false, "can_go_forward"),
            page_title: Property::new("Reel".to_string(), "page_title"),
            header_config: Property::new(default_config, "header_config"),
            event_bus: RwLock::new(None),
        }
    }

    /// Get the current page property for subscription
    pub fn current_page(&self) -> &Property<Option<PageState>> {
        &self.current_page
    }

    /// Get the navigation stack property
    pub fn navigation_stack(&self) -> &Property<Vec<PageState>> {
        &self.navigation_stack
    }

    /// Get the can go back property
    pub fn can_go_back(&self) -> &Property<bool> {
        &self.can_go_back
    }

    /// Get the can go forward property
    pub fn can_go_forward(&self) -> &Property<bool> {
        &self.can_go_forward
    }

    /// Get the page title property
    pub fn page_title(&self) -> &Property<String> {
        &self.page_title
    }

    /// Get the header config property
    pub fn header_config(&self) -> &Property<HeaderConfig> {
        &self.header_config
    }

    /// Navigate to a specific page
    pub async fn navigate_to(&self, request: NavigationRequest) -> Result<(), String> {
        match request {
            NavigationRequest::NavigateToPage(mut page_state) => {
                self.emit_navigation_event(
                    EventType::NavigationRequested,
                    page_state.name.clone(),
                    Some(page_state.title.clone()),
                    None,
                    None,
                )
                .await;

                // Update navigation stack
                let mut stack = self.navigation_stack.get_sync();
                stack.push(page_state.clone());

                // Update can_go_back based on stack size
                let can_go_back = stack.len() > 1;
                page_state.can_go_back = can_go_back;

                // Update properties
                self.navigation_stack.set(stack).await;
                self.current_page.set(Some(page_state.clone())).await;
                self.can_go_back.set(can_go_back).await;
                self.page_title.set(page_state.title.clone()).await;
                self.header_config
                    .set(page_state.header_config.clone())
                    .await;

                self.emit_navigation_event(
                    EventType::NavigationCompleted,
                    page_state.name,
                    Some(page_state.title),
                    Some(can_go_back),
                    None,
                )
                .await;

                Ok(())
            }
            NavigationRequest::GoBack => {
                let mut stack = self.navigation_stack.get_sync();
                if stack.len() <= 1 {
                    return Err("Cannot go back: no previous page in history".to_string());
                }

                // Remove current page
                stack.pop();

                // Get previous page
                if let Some(previous_page) = stack.last() {
                    let previous_page = previous_page.clone();
                    let can_go_back = stack.len() > 1;

                    // Update properties
                    self.navigation_stack.set(stack).await;
                    self.current_page.set(Some(previous_page.clone())).await;
                    self.can_go_back.set(can_go_back).await;
                    self.page_title.set(previous_page.title.clone()).await;
                    self.header_config
                        .set(previous_page.header_config.clone())
                        .await;

                    self.emit_navigation_event(
                        EventType::NavigationCompleted,
                        previous_page.name,
                        Some(previous_page.title),
                        Some(can_go_back),
                        None,
                    )
                    .await;

                    Ok(())
                } else {
                    Err("Navigation stack is empty after pop".to_string())
                }
            }
            NavigationRequest::GoHome => {
                // Clear stack and navigate to home
                let home_page = PageState::new("home".to_string(), "Home".to_string());
                self.navigation_stack.set(vec![home_page.clone()]).await;
                self.current_page.set(Some(home_page.clone())).await;
                self.can_go_back.set(false).await;
                self.page_title.set(home_page.title.clone()).await;
                self.header_config
                    .set(home_page.header_config.clone())
                    .await;

                self.emit_navigation_event(
                    EventType::NavigationCompleted,
                    home_page.name,
                    Some(home_page.title),
                    Some(false),
                    None,
                )
                .await;

                Ok(())
            }
            NavigationRequest::UpdatePageTitle(title) => {
                self.page_title.set(title.clone()).await;

                // Update current page if it exists
                if let Some(mut current_page) = self.current_page.get_sync() {
                    current_page.title = title.clone();
                    current_page.header_config.title = title.clone();
                    self.current_page.set(Some(current_page)).await;
                    self.header_config
                        .set(self.header_config.get_sync().clone())
                        .await;
                }

                self.emit_navigation_event(
                    EventType::PageTitleChanged,
                    self.current_page
                        .get_sync()
                        .map(|p| p.name)
                        .unwrap_or_default(),
                    Some(title),
                    None,
                    None,
                )
                .await;

                Ok(())
            }
            NavigationRequest::UpdateHeaderConfig(config) => {
                self.header_config.set(config.clone()).await;

                // Update current page if it exists
                if let Some(mut current_page) = self.current_page.get_sync() {
                    current_page.header_config = config.clone();
                    self.current_page.set(Some(current_page)).await;
                }

                self.emit_navigation_event(
                    EventType::HeaderConfigChanged,
                    self.current_page
                        .get_sync()
                        .map(|p| p.name)
                        .unwrap_or_default(),
                    None,
                    None,
                    None,
                )
                .await;

                Ok(())
            }
        }
    }

    /// Get the current page name (synchronously)
    pub fn current_page_name(&self) -> Option<String> {
        self.current_page.get_sync().map(|page| page.name)
    }

    /// Check if navigation can go back
    pub fn can_navigate_back(&self) -> bool {
        self.can_go_back.get_sync()
    }

    /// Get current navigation history size
    pub fn history_size(&self) -> usize {
        self.navigation_stack.get_sync().len()
    }

    /// Clear navigation history
    pub async fn clear_history(&self) {
        self.navigation_stack.set(Vec::new()).await;
        self.current_page.set(None).await;
        self.can_go_back.set(false).await;
        self.can_go_forward.set(false).await;

        self.emit_navigation_event(
            EventType::NavigationHistoryChanged,
            "".to_string(),
            None,
            Some(false),
            None,
        )
        .await;
    }

    /// Helper to emit navigation events
    async fn emit_navigation_event(
        &self,
        event_type: EventType,
        page_name: String,
        page_title: Option<String>,
        can_go_back: Option<bool>,
        error: Option<String>,
    ) {
        if let Some(event_bus) = self.event_bus.read().await.as_ref() {
            let event = DatabaseEvent::new(
                event_type,
                EventPayload::Navigation {
                    page_name,
                    page_title,
                    can_go_back,
                    error,
                },
            );

            if let Err(e) = event_bus.publish(event).await {
                tracing::warn!("Failed to emit navigation event: {}", e);
            }
        }
    }

    /// Setup event handlers for sidebar navigation events
    async fn setup_sidebar_navigation_handlers(&self, event_bus: Arc<EventBus>) {
        // Subscribe to sidebar navigation events
        let mut receiver = event_bus.subscribe_to_types(vec![
            EventType::LibraryNavigationRequested,
            EventType::HomeNavigationRequested,
        ]);

        // Clone properties for event handler
        let current_page = self.current_page.clone();
        let navigation_stack = self.navigation_stack.clone();
        let can_go_back = self.can_go_back.clone();
        let page_title = self.page_title.clone();
        let header_config = self.header_config.clone();

        // Spawn event handler task
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                match event.event_type {
                    EventType::LibraryNavigationRequested => {
                        if let EventPayload::LibraryNavigation {
                            source_id,
                            library_id,
                            library_title,
                            library_type,
                        } = event.payload
                        {
                            tracing::info!(
                                "NavigationViewModel handling library navigation: {} ({})",
                                library_title,
                                library_type
                            );

                            // Create library page state
                            let page_name = format!("library:{}:{}", source_id, library_id);
                            let page_state = PageState::new(page_name, library_title.clone())
                                .with_can_go_back(true)
                                .with_header_config(
                                    HeaderConfig::default_with_title(library_title)
                                        .with_back_button(true),
                                );

                            // Update navigation state
                            let mut stack = navigation_stack.get_sync();
                            stack.push(page_state.clone());
                            navigation_stack.set(stack).await;
                            current_page.set(Some(page_state.clone())).await;
                            can_go_back.set(page_state.can_go_back).await;
                            page_title.set(page_state.title.clone()).await;
                            header_config.set(page_state.header_config.clone()).await;

                            tracing::info!("NavigationViewModel library navigation complete");
                        }
                    }
                    EventType::HomeNavigationRequested => {
                        if let EventPayload::HomeNavigation { source_id } = event.payload {
                            tracing::info!(
                                "NavigationViewModel handling home navigation: source_id={:?}",
                                source_id
                            );

                            // Create home page state
                            let page_state = PageState::new("home".to_string(), "Home".to_string());

                            // Clear stack and navigate to home
                            navigation_stack.set(vec![page_state.clone()]).await;
                            current_page.set(Some(page_state.clone())).await;
                            can_go_back.set(false).await;
                            page_title.set(page_state.title.clone()).await;
                            header_config.set(page_state.header_config.clone()).await;

                            tracing::info!("NavigationViewModel home navigation complete");
                        }
                    }
                    _ => {}
                }
            }
        });

        tracing::info!("NavigationViewModel sidebar navigation handlers setup complete");
    }
}

#[async_trait::async_trait]
impl ViewModel for NavigationViewModel {
    async fn initialize(&self, event_bus: Arc<EventBus>) {
        *self.event_bus.write().await = Some(event_bus.clone());
        tracing::info!("NavigationViewModel initialized with event bus");

        // Subscribe to sidebar navigation events
        self.setup_sidebar_navigation_handlers(event_bus.clone())
            .await;

        // Initialize with empty state
        self.clear_history().await;
    }

    fn subscribe_to_property(&self, property_name: &str) -> Option<PropertySubscriber> {
        match property_name {
            "current_page" => Some(self.current_page.subscribe()),
            "navigation_stack" => Some(self.navigation_stack.subscribe()),
            "can_go_back" => Some(self.can_go_back.subscribe()),
            "can_go_forward" => Some(self.can_go_forward.subscribe()),
            "page_title" => Some(self.page_title.subscribe()),
            "header_config" => Some(self.header_config.subscribe()),
            _ => None,
        }
    }

    async fn refresh(&self) {
        // NavigationViewModel doesn't need external data refresh
        // It's purely UI state management
        tracing::debug!("NavigationViewModel refresh requested (no-op)");
    }
}

impl Default for NavigationViewModel {
    fn default() -> Self {
        Self::new()
    }
}
