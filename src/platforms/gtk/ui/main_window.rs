use gtk4::{gio, glib, glib::clone, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::sync::Arc;
use tracing::{debug, error, info};

use super::filters::{SortOrder, WatchStatus};
use super::navigation::NavigationManager;
use super::widgets::sidebar::Sidebar;
use crate::config::Config;
use crate::models::MediaItem;
use crate::platforms::gtk::ui::page_factory::PageFactory;
use crate::state::AppState;
use tokio::sync::RwLock;

mod imp {
    use super::*;

    use gtk4::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/dev/arsfeld/Reel/window.ui")]
    pub struct ReelMainWindow {
        #[template_child]
        pub empty_state: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub content_page: TemplateChild<adw::NavigationPage>,
        #[template_child]
        pub content_toolbar: TemplateChild<adw::ToolbarView>,
        #[template_child]
        pub content_header: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub sidebar_placeholder: TemplateChild<gtk4::Box>,

        pub state: RefCell<Option<Arc<AppState>>>,
        pub config: RefCell<Option<Arc<RwLock<Config>>>>,
        pub content_stack: RefCell<Option<gtk4::Stack>>,
        pub home_page: RefCell<Option<crate::platforms::gtk::ui::pages::HomePage>>,
        pub sources_page: RefCell<Option<crate::platforms::gtk::ui::pages::SourcesPage>>,
        pub library_view: RefCell<Option<crate::platforms::gtk::ui::pages::LibraryView>>,
        pub player_page: RefCell<Option<crate::platforms::gtk::ui::pages::PlayerPage>>,
        pub show_details_page: RefCell<Option<crate::platforms::gtk::ui::pages::ShowDetailsPage>>,
        pub movie_details_page: RefCell<Option<crate::platforms::gtk::ui::pages::MovieDetailsPage>>,
        // Window state is now managed by NavigationManager
        pub filter_controls: RefCell<Option<gtk4::Box>>,
        pub edit_mode: RefCell<bool>,
        pub library_visibility: RefCell<std::collections::HashMap<String, bool>>,
        pub all_libraries: RefCell<Vec<(crate::models::Library, usize)>>,
        pub navigation_manager: RefCell<Option<Arc<NavigationManager>>>,
        pub header_add_button: RefCell<Option<gtk4::Button>>, // Track add button in header
        pub sidebar_viewmodel:
            RefCell<Option<Arc<crate::platforms::gtk::ui::viewmodels::SidebarViewModel>>>,
        pub sidebar_widget: RefCell<Option<Sidebar>>,
        pub page_factory: RefCell<Option<crate::platforms::gtk::ui::page_factory::PageFactory>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ReelMainWindow {
        const NAME: &'static str = "ReelMainWindow";
        type Type = super::ReelMainWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk4::template_callbacks]
    impl ReelMainWindow {
        #[template_callback]
        fn on_connect_clicked(&self, _button: &gtk4::Button) {
            info!("Connect button clicked");
            self.obj().show_auth_dialog();
        }

        #[template_callback]
        fn on_refresh_clicked(&self, _button: &gtk4::Button) {
            info!("Refresh button clicked");
            // We can't spawn async directly from template callback,
            // so we'll just log for now. The actual handler is in constructed()
        }
    }

    impl ObjectImpl for ReelMainWindow {
        fn constructed(&self) {
            self.parent_constructed();

            // Signal connections are now handled by the Sidebar widget

            // Library list row activation is now handled per-source group

            // Sources button is now always visible
        }
    }

    impl WidgetImpl for ReelMainWindow {}
    impl WindowImpl for ReelMainWindow {}
    impl ApplicationWindowImpl for ReelMainWindow {}
    impl adw::subclass::application_window::AdwApplicationWindowImpl for ReelMainWindow {}
}

glib::wrapper! {
    pub struct ReelMainWindow(ObjectSubclass<imp::ReelMainWindow>)
        @extends gtk4::Widget, gtk4::Window, gtk4::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk4::Accessible, gtk4::Buildable,
                    gtk4::ConstraintTarget, gtk4::Native, gtk4::Root, gtk4::ShortcutManager;
}

impl ReelMainWindow {
    pub fn new(app: &adw::Application, state: Arc<AppState>, config: Arc<RwLock<Config>>) -> Self {
        let window: Self = glib::Object::builder().property("application", app).build();

        // Store state and config
        window.imp().state.replace(Some(state.clone()));
        window.imp().config.replace(Some(config));

        // Initialize navigation manager
        let navigation_manager =
            Arc::new(NavigationManager::new(window.imp().content_header.clone()));
        // Setup back button callback now that NavigationManager is in Arc
        navigation_manager.setup_back_button_callback();
        window
            .imp()
            .navigation_manager
            .replace(Some(navigation_manager));

        // Initialize PageFactory
        let page_factory = PageFactory::new(state.clone());
        window.imp().page_factory.replace(Some(page_factory));

        // Initialize SidebarViewModel
        let sidebar_vm = Arc::new(
            crate::platforms::gtk::ui::viewmodels::SidebarViewModel::new(
                state.data_service.clone(),
            ),
        );
        window
            .imp()
            .sidebar_viewmodel
            .replace(Some(sidebar_vm.clone()));

        // Create and setup Sidebar widget
        let sidebar_widget = Sidebar::new();
        sidebar_widget.set_viewmodel(sidebar_vm.clone());
        sidebar_widget.set_event_bus(state.event_bus.clone());
        sidebar_widget.set_main_window(&window); // Still needed for fallback
        window
            .imp()
            .sidebar_widget
            .replace(Some(sidebar_widget.clone()));

        // Replace placeholder with actual sidebar widget
        if let Some(parent) = window.imp().sidebar_placeholder.parent() {
            if let Some(toolbar_view) = parent.downcast_ref::<adw::ToolbarView>() {
                toolbar_view.set_content(Some(&sidebar_widget));
            }
        }

        // Subscribe to navigation events from the EventBus
        let event_bus_clone = state.event_bus.clone();
        let window_weak_for_nav = window.downgrade();
        glib::spawn_future_local(async move {
            use crate::events::types::{EventPayload, EventType};

            let mut subscriber = event_bus_clone.subscribe();
            while let Ok(event) = subscriber.recv().await {
                if event.event_type == EventType::NavigationRequested {
                    if let EventPayload::NavigationRequest { request } = event.payload {
                        tracing::info!("MainWindow: Received navigation event");
                        if let Some(window) = window_weak_for_nav.upgrade() {
                            window.navigate_to(*request).await;
                        }
                    }
                }
            }
        });

        // Initialize the ViewModel with the event bus
        let event_bus = state.event_bus.clone();
        let sidebar_vm_clone = sidebar_vm.clone();
        let window_weak_for_vm = window.downgrade();
        glib::spawn_future_local(async move {
            use super::viewmodels::ViewModel;
            sidebar_vm_clone.initialize(event_bus).await;

            // Set up subscription to sources property
            if let Some(_window) = window_weak_for_vm.upgrade() {
                // Sidebar subscriptions are now handled by the Sidebar widget itself

                // Initial load should happen after ViewModel is initialized and subscriptions are set up
                tracing::info!("SidebarViewModel initialized, triggering initial data load");
                sidebar_vm_clone.refresh().await;
            }
        });

        // Setup actions
        window.setup_actions(app);

        // Apply theme
        let window_weak = window.downgrade();
        glib::spawn_future_local(async move {
            if let Some(window) = window_weak.upgrade() {
                window.apply_theme().await;
            }
        });

        // Check for existing backends and load them (SidebarViewModel will handle cached data display)
        window.check_and_load_backends(state);

        // Subscribe to user changes
        window.setup_state_subscriptions();

        window
    }

    fn setup_actions(&self, app: &adw::Application) {
        // Preferences action
        let preferences_action = gio::SimpleAction::new("preferences", None);
        preferences_action.connect_activate(clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                info!("Opening preferences");
                window.show_preferences();
            }
        ));
        app.add_action(&preferences_action);

        // About action
        let about_action = gio::SimpleAction::new("about", None);
        about_action.connect_activate(clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                window.show_about();
            }
        ));
        app.add_action(&about_action);

        // Keyboard shortcuts
        app.set_accels_for_action("app.preferences", &["<primary>comma"]);
        app.set_accels_for_action("window.close", &["<primary>w"]);
    }

    async fn apply_theme(&self) {
        if let Some(config_arc) = self.imp().config.borrow().as_ref() {
            let style_manager = adw::StyleManager::default();

            let theme = {
                let config = config_arc.read().await;
                config.general.theme.clone()
            };

            match theme.as_str() {
                "light" => style_manager.set_color_scheme(adw::ColorScheme::ForceLight),
                "dark" => style_manager.set_color_scheme(adw::ColorScheme::ForceDark),
                _ => style_manager.set_color_scheme(adw::ColorScheme::PreferDark),
            }
        }
    }

    fn check_and_load_backends(&self, state: Arc<AppState>) {
        let window_weak = self.downgrade();

        glib::spawn_future_local(async move {
            // Use SourceCoordinator for reactive initialization
            let source_coordinator = state.get_source_coordinator();

            // First, load saved providers from config
            if let Err(e) = source_coordinator.get_auth_manager().load_providers().await {
                error!("Failed to load providers: {}", e);
            }

            // Then, migrate any legacy backends
            if let Err(e) = source_coordinator.migrate_legacy_backends().await {
                error!("Failed to migrate legacy backends: {}", e);
            }

            // Start reactive initialization (returns immediately)
            let init_state = source_coordinator.initialize_sources_reactive();
            info!("Started reactive source initialization - UI ready immediately");

            // Pass initialization state to sidebar for progressive UI enhancement
            if let Some(window) = window_weak.upgrade() {
                if let Some(sidebar) = window.imp().sidebar_widget.borrow().as_ref() {
                    sidebar.set_initialization_state(init_state.clone());
                }
            }

            // Handle the results of backend initialization
            if let Some(window) = window_weak.upgrade() {
                // Update backend selector (now just hides it)
                window.update_backend_selector().await;

                // Check how many backends are connected
                let all_backends = state.source_coordinator.get_all_backends().await;
                let connected_count = all_backends.len();

                if connected_count > 0 {
                    info!("Successfully initialized {} backends", connected_count);
                    // Status is now managed by SidebarViewModel - no manual updates needed

                    // Refresh all libraries
                    window.refresh_all_libraries().await;

                    // Start background sync for all sources
                    let coordinator_clone = source_coordinator.clone();
                    let window_weak2 = window.downgrade();
                    glib::spawn_future_local(async move {
                        if let Some(_window) = window_weak2.upgrade() {
                            info!("Starting background sync for all sources...");
                            if let Err(e) = coordinator_clone.sync_all_visible_sources().await {
                                error!("Failed to sync sources: {}", e);
                            }
                        }
                    });
                } else {
                    info!("No backends were successfully initialized");

                    // Check if we have any providers that need authentication
                    let providers = source_coordinator
                        .get_auth_manager()
                        .get_all_providers()
                        .await;

                    if !providers.is_empty() {
                        info!(
                            "Found {} providers but no valid credentials",
                            providers.len()
                        );

                        // Status is now managed by SidebarViewModel - no manual updates needed
                    }
                }
            }
        });
    }

    fn setup_state_subscriptions(&self) {
        // Listen for state changes
        // For now, we'll handle updates manually when auth completes
    }

    pub fn show_auth_dialog(&self) {
        info!("Showing authentication dialog");

        // Get state from the window
        let state = self.imp().state.borrow().as_ref().unwrap().clone();

        // Create and show auth dialog
        let dialog = crate::platforms::gtk::ui::AuthDialog::new(state.clone());
        dialog.present(Some(self));

        // Start authentication automatically
        dialog.start_authentication();

        // Set up a callback for when the dialog closes
        let window_weak = self.downgrade();
        let state_clone = state.clone();
        dialog.connect_closed(move |_| {
            if let Some(window) = window_weak.upgrade() {
                // Check if we now have an authenticated backend
                let state_for_async = state_clone.clone();
                glib::spawn_future_local(async move {
                    let source_coordinator = state_for_async.get_source_coordinator();
                    let all_backends = source_coordinator.get_all_backends().await;
                    if let Some((backend_id, backend)) = all_backends.into_iter().next()
                        && backend.is_initialized().await
                    {
                        info!("Backend initialized after auth dialog closed");

                        // Backend status is now managed by SidebarViewModel
                        window.update_backend_selector().await;

                        // FIRST: Load cached data immediately via SidebarViewModel
                        info!(
                            "Refreshing SidebarViewModel for newly authenticated backend: {}",
                            backend_id
                        );
                        if let Some(sidebar_vm) = window.imp().sidebar_viewmodel.borrow().as_ref() {
                            use super::viewmodels::ViewModel;
                            sidebar_vm.refresh().await;
                        }

                        // THEN: Start background sync
                        let backend_clone = backend.clone();
                        let state_clone = state_for_async.clone();
                        let window_weak = window.downgrade();
                        glib::spawn_future_local(async move {
                            if let Some(window) = window_weak.upgrade() {
                                info!("Starting background sync after auth...");
                                // Sync progress is now managed by SidebarViewModel

                                // Start sync with backend ID
                                // Use the same backend_id from outer scope
                                if true {
                                    window
                                        .sync_and_update_libraries(
                                            &backend_id,
                                            backend_clone,
                                            state_clone,
                                        )
                                        .await;
                                }

                                // Sync progress is now managed by SidebarViewModel
                            }
                        });
                    }
                });
            }
        });
    }

    fn show_preferences(&self) {
        info!("Showing preferences");

        if let Some(config) = self.imp().config.borrow().as_ref()
            && let Some(state) = self.imp().state.borrow().as_ref()
        {
            let prefs_window = crate::platforms::gtk::ui::PreferencesWindow::new(
                self,
                config.clone(),
                state.event_bus.clone(),
            );
            prefs_window.present();
        }
    }

    fn show_about(&self) {
        let about = adw::AboutWindow::builder()
            .application_name("Reel")
            .application_icon("dev.arsfeld.Reel")
            .developer_name("Alexandre Rosenfeld")
            .version(env!("CARGO_PKG_VERSION"))
            .license_type(gtk4::License::Gpl30)
            .website("https://github.com/arsfeld/reel")
            .issue_url("https://github.com/arsfeld/reel/issues")
            .build();

        about.set_transient_for(Some(self));
        about.present();
    }

    // This method is now handled entirely by the Sidebar widget through reactive bindings

    fn show_sources_page(&self) {
        info!("show_sources_page called");
        let imp = self.imp();

        // Get or create content stack first to ensure it exists
        let content_stack = self.ensure_content_stack();

        // Create sources page if it doesn't exist
        if content_stack.child_by_name("sources").is_none() {
            if let Some(page_factory) = &*imp.page_factory.borrow() {
                // Create sources page with header setup callback
                let header_ref = imp.content_header.clone();
                let add_button_ref = imp.header_add_button.clone();
                let sources_page =
                    page_factory.get_or_create_sources_page(move |title_label, add_button| {
                        // Set the header title
                        header_ref.set_title_widget(Some(title_label));

                        // Add the button to header and store reference
                        header_ref.pack_end(add_button);
                        add_button_ref.replace(Some(add_button.clone()));
                    });

                content_stack.add_named(&sources_page, Some("sources"));
                imp.sources_page.replace(Some(sources_page));
            }
        }

        // Use NavigationManager for reactive navigation
        if let Some(nav_manager) = imp.navigation_manager.borrow().as_ref() {
            let nav_manager = Arc::clone(nav_manager);
            glib::spawn_future_local(async move {
                nav_manager
                    .navigate_to(super::navigation::NavigationPage::Sources)
                    .await;
            });
        }
    }

    pub fn show_home_page_for_source(&self, source_id: Option<String>) {
        let imp = self.imp();

        // Get content stack
        let content_stack = self.ensure_content_stack();

        // Use PageFactory to get or create home page
        if let Some(page_factory) = imp.page_factory.borrow().as_ref() {
            let window_weak = self.downgrade();
            let home_page =
                page_factory.get_or_create_home_page(source_id.clone(), move |nav_request| {
                    if let Some(window) = window_weak.upgrade() {
                        glib::spawn_future_local(async move {
                            window.navigate_to(nav_request).await;
                        });
                    }
                });

            // Ensure page is in the stack
            if content_stack.child_by_name("home").is_none() {
                content_stack.add_named(&home_page.clone().upcast::<gtk4::Widget>(), Some("home"));
            }

            // Store reference for compatibility
            imp.home_page.replace(Some(home_page.clone()));

            // Update the content page title
            imp.content_page.set_title("Home");

            // Set transition and show the home page
            content_stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);
            content_stack.set_transition_duration(300);
            content_stack.set_visible_child_name("home");

            // Show content pane on mobile
            if let Some(content) = self.content()
                && let Some(split_view) = content.downcast_ref::<adw::NavigationSplitView>()
            {
                split_view.set_show_content(true);
            }
        } else {
            tracing::error!("PageFactory not initialized");
        }
    }

    pub async fn trigger_sync(&self, state: Arc<AppState>) {
        info!("Manually triggering sync for all backends...");

        // Sync all backends
        self.sync_all_backends(state).await;
    }

    pub async fn sync_and_update_libraries(
        &self,
        backend_id: &str,
        backend: Arc<dyn crate::backends::MediaBackend>,
        state: Arc<AppState>,
    ) {
        info!("Starting library sync for backend: {}", backend_id);

        // Get sync manager from state
        let sync_manager = state.sync_manager.clone();

        // Perform sync
        match sync_manager.sync_backend(backend_id, backend).await {
            Ok(result) => {
                info!(
                    "Sync completed for {}: {} items synced",
                    backend_id, result.items_synced
                );
            }
            Err(e) => {
                error!("Sync failed for {}: {}", backend_id, e);
            }
        }

        // After syncing one backend, refresh all libraries display
        self.refresh_all_libraries().await;

        // Check if we should navigate to home page
        let imp = self.imp();
        let should_show_home = if let Some(stack) = imp.content_stack.borrow().as_ref() {
            // Check if we have any actual content pages
            let has_content = stack.child_by_name("library").is_some()
                || stack.child_by_name("movie_details").is_some()
                || stack.child_by_name("show_details").is_some()
                || stack.child_by_name("player").is_some();

            !has_content
        } else {
            true
        };

        if should_show_home {
            self.show_home_page_for_source(None);
        } else {
            // Just refresh the home page data if it exists
            if let Some(home_page) = &*imp.home_page.borrow() {
                home_page.refresh();
            }
        }
    }

    pub async fn sync_all_backends(&self, state: Arc<AppState>) {
        info!("Starting sync for all backends...");

        let all_backends = state.source_coordinator.get_all_backends().await;

        // Sync progress is now managed by SidebarViewModel

        // Sync each backend sequentially
        for (backend_id, backend) in all_backends {
            info!("Syncing backend: {}", backend_id);

            let sync_manager = state.sync_manager.clone();
            match sync_manager.sync_backend(&backend_id, backend).await {
                Ok(result) => {
                    info!(
                        "Backend {} synced: {} items",
                        backend_id, result.items_synced
                    );
                }
                Err(e) => {
                    error!("Failed to sync backend {}: {}", backend_id, e);
                }
            }
        }

        // Sync progress is now managed by SidebarViewModel

        // Refresh all libraries display
        self.refresh_all_libraries().await;

        // Navigate to home if appropriate
        let imp = self.imp();
        let should_show_home = if let Some(stack) = imp.content_stack.borrow().as_ref() {
            stack.child_by_name("library").is_none()
                && stack.child_by_name("movie_details").is_none()
                && stack.child_by_name("show_details").is_none()
                && stack.child_by_name("player").is_none()
        } else {
            true
        };

        if should_show_home {
            self.show_home_page_for_source(None);
        } else if let Some(home_page) = &*imp.home_page.borrow() {
            home_page.refresh();
        }
    }

    pub async fn sync_single_backend(&self, backend_id: &str) {
        info!("Starting sync for backend: {}", backend_id);

        let state = self.imp().state.borrow().as_ref().unwrap().clone();

        if let Some(backend) = state.source_coordinator.get_backend(backend_id).await {
            // Sync progress is now managed by SidebarViewModel

            let sync_manager = state.sync_manager.clone();
            match sync_manager.sync_backend(backend_id, backend).await {
                Ok(result) => {
                    info!(
                        "Backend {} synced: {} items",
                        backend_id, result.items_synced
                    );
                }
                Err(e) => {
                    error!("Failed to sync backend {}: {}", backend_id, e);
                }
            }

            // Sync progress is now managed by SidebarViewModel

            // Refresh all libraries display
            self.refresh_all_libraries().await;
        } else {
            error!("Backend {} not found", backend_id);
        }
    }

    pub async fn navigate_to_library(&self, library_id: &str) {
        info!("Navigating to library: {}", library_id);

        // Parse the library_id format "source_id:library_id"
        let parts: Vec<&str> = library_id.split(':').collect();
        if parts.len() != 2 {
            error!("Invalid library ID format: {}", library_id);
            return;
        }

        let source_id = parts[0];
        let lib_id = parts[1];

        // Get the library metadata from the state
        let state = self.imp().state.borrow().as_ref().unwrap().clone();

        // Find the library in the source coordinator
        if let Some(backend) = state.source_coordinator.get_backend(source_id).await {
            // Get libraries from this backend
            match backend.get_libraries().await {
                Ok(libraries) => {
                    // Find the specific library
                    if let Some(library) = libraries.iter().find(|l| l.id == lib_id) {
                        // Now show the library view
                        self.show_library_view(source_id.to_string(), library.clone())
                            .await;
                    } else {
                        error!("Library not found: {} in source {}", lib_id, source_id);
                    }
                }
                Err(e) => {
                    error!("Failed to get libraries from backend {}: {}", source_id, e);
                }
            }
        } else {
            error!("Backend not found: {}", source_id);
        }
    }

    async fn refresh_all_libraries(&self) {
        // Only refresh the SidebarViewModel - it will handle updating the UI via reactive properties
        if let Some(sidebar_vm) = self.imp().sidebar_viewmodel.borrow().as_ref() {
            use super::viewmodels::ViewModel;
            sidebar_vm.refresh().await;
            tracing::info!("Refreshed SidebarViewModel - UI will update via reactive properties");
        } else {
            tracing::warn!("SidebarViewModel not found during refresh_all_libraries");
        }
    }

    pub async fn show_movie_details(&self, movie: crate::models::Movie, _state: Arc<AppState>) {
        let imp = self.imp();
        let start_time = std::time::Instant::now();

        // Cleanup current page if needed
        self.cleanup_current_page().await;

        // Get content stack
        let content_stack = self.ensure_content_stack();

        // Set transition for details pages
        content_stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);
        content_stack.set_transition_duration(300);

        // Use PageFactory to get or create the movie details page
        let _movie_details_page = if let Some(page_factory) = imp.page_factory.borrow().as_ref() {
            let page = page_factory.get_or_create_movie_details_page();

            // Ensure page is in the stack
            if content_stack.child_by_name("movie_details").is_none() {
                content_stack.add_named(&page, Some("movie_details"));
            }

            // Set up the page with callbacks and data
            let window_weak = self.downgrade();
            page_factory.setup_movie_details_page(&page, &movie, move |movie| {
                if let Some(window) = window_weak.upgrade() {
                    let movie_item = crate::models::MediaItem::Movie(movie.clone());
                    glib::spawn_future_local(async move {
                        use super::navigation_request::NavigationRequest;
                        window
                            .navigate_to(NavigationRequest::ShowPlayer(movie_item))
                            .await;
                    });
                }
            });

            // Store reference for compatibility (will be removed later)
            imp.movie_details_page.replace(Some(page.clone()));
            page
        } else {
            tracing::error!("PageFactory not initialized");
            return;
        };

        // Update the content page title
        imp.content_page.set_title(&movie.title);

        // Defer the transition until data starts loading (immediate transition for perceived performance)
        // This gives the best of both worlds: immediate response + smooth transition
        glib::timeout_add_local_once(std::time::Duration::from_millis(10), {
            let content_stack = content_stack.clone();
            move || {
                content_stack.set_visible_child_name("movie_details");
            }
        });

        // Performance monitoring
        let elapsed = start_time.elapsed();
        if elapsed > std::time::Duration::from_millis(16) {
            tracing::warn!(
                "show_movie_details took {:?} (exceeds frame budget)",
                elapsed
            );
        }
    }

    pub async fn show_show_details(&self, show: crate::models::Show, _state: Arc<AppState>) {
        let imp = self.imp();
        let start_time = std::time::Instant::now();

        // Cleanup current page if needed
        self.cleanup_current_page().await;

        // Get content stack
        let content_stack = self.ensure_content_stack();

        // Set transition for details pages
        content_stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);
        content_stack.set_transition_duration(300);

        // Use PageFactory to get or create the show details page
        let _show_details_page = if let Some(page_factory) = imp.page_factory.borrow().as_ref() {
            let page = page_factory.get_or_create_show_details_page();

            // Ensure page is in the stack
            if content_stack.child_by_name("show_details").is_none() {
                content_stack.add_named(page.widget(), Some("show_details"));
            }

            // Set up the page with callbacks and data
            let window_weak = self.downgrade();
            page_factory.setup_show_details_page(&page, &show, move |episode| {
                if let Some(window) = window_weak.upgrade() {
                    let episode_item = crate::models::MediaItem::Episode(episode.clone());
                    glib::spawn_future_local(async move {
                        use super::navigation_request::NavigationRequest;
                        window
                            .navigate_to(NavigationRequest::ShowPlayer(episode_item))
                            .await;
                    });
                }
            });

            // Store reference for compatibility (will be removed later)
            imp.show_details_page.replace(Some(page.clone()));
            page
        } else {
            tracing::error!("PageFactory not initialized");
            return;
        };

        // Update the content page title
        imp.content_page.set_title(&show.title);

        // Defer the transition until data starts loading (immediate transition for perceived performance)
        // This gives the best of both worlds: immediate response + smooth transition
        glib::timeout_add_local_once(std::time::Duration::from_millis(10), {
            let content_stack = content_stack.clone();
            move || {
                content_stack.set_visible_child_name("show_details");
            }
        });

        // Performance monitoring
        let elapsed = start_time.elapsed();
        if elapsed > std::time::Duration::from_millis(16) {
            tracing::warn!(
                "show_show_details took {:?} (exceeds frame budget)",
                elapsed
            );
        }
    }

    /// Cleanup the current page if it needs cleanup
    async fn cleanup_current_page(&self) {
        let content_stack = self.ensure_content_stack();

        if let Some(current_name) = content_stack.visible_child_name() {
            let current_name_str = current_name.as_str();

            // Check if the current page needs cleanup
            if let Some(page_factory) = self.imp().page_factory.borrow().as_ref() {
                if page_factory.needs_cleanup(current_name_str) {
                    if let Some(widget) = content_stack.child_by_name(current_name_str) {
                        info!("Cleaning up page: {}", current_name_str);
                        page_factory
                            .cleanup_page_async(current_name_str, &widget)
                            .await;
                    }
                }
            }

            // Special case: player page cleanup
            if current_name_str == "player" {
                if let Some(old_page) = self.imp().player_page.borrow().as_ref() {
                    info!("Cleaning up existing PlayerPage");
                    old_page.cleanup().await;
                }
                // Clear the stored reference to allow proper Drop cleanup
                self.imp().player_page.replace(None);
            }
        }
    }

    pub async fn show_player(&self, media_item: &crate::models::MediaItem, state: Arc<AppState>) {
        let start_time = std::time::Instant::now();
        info!(
            "MainWindow::show_player() - Called for media: {}",
            media_item.title()
        );
        debug!(
            "MainWindow::show_player() - Media type: {:?}, ID: {}",
            std::mem::discriminant(media_item),
            media_item.id()
        );

        let imp = self.imp();

        // Get content stack
        let content_stack = self.ensure_content_stack();
        content_stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);
        content_stack.set_transition_duration(300);

        // Always destroy and recreate PlayerPage for each media item
        // This eliminates all MPV lifecycle issues and state pollution
        info!(
            "Destroying and recreating PlayerPage for media: {}",
            media_item.title()
        );

        // Cleanup any existing page that needs cleanup
        self.cleanup_current_page().await;

        // Remove old player page widget from stack if it exists
        if let Some(old_widget) = content_stack.child_by_name("player") {
            content_stack.remove(&old_widget);
        }

        // Use PageFactory to create a fresh PlayerPage instance
        let player_page = if let Some(page_factory) = imp.page_factory.borrow().as_ref() {
            let page = page_factory.create_player_page();
            // PlayerPage is always new, no setup needed from factory
            // Store reference for compatibility (will be removed later)
            self.imp().player_page.replace(Some(page.clone()));
            content_stack.add_named(page.widget(), Some("player"));
            page
        } else {
            tracing::error!("PageFactory not initialized");
            return;
        };

        // Update the content page title first
        self.imp().content_page.set_title(media_item.title());
        debug!("MainWindow::show_player() - Updated content page title");

        // Also clear any title widget that might have controls
        imp.content_header.set_title_widget(None::<&gtk4::Widget>);

        // Hide the main header bar completely to avoid duplicate back buttons
        imp.content_header.set_visible(false);
        imp.content_toolbar
            .set_top_bar_style(adw::ToolbarStyle::Flat); // Make toolbar flat/hidden

        // Configure back/close actions on the player page OSD buttons
        let window_weak_close = self.downgrade();
        player_page.set_on_close_clicked(move || {
            if let Some(window) = window_weak_close.upgrade() {
                window.close();
            }
        });

        let window_weak = self.downgrade();
        player_page.set_on_back_clicked(move || {
            if let Some(window) = window_weak.upgrade() {
                // Stop the player and wait for completion before navigation
                if let Some(player_page) = window.imp().player_page.borrow().as_ref() {
                    let player_page = player_page.clone();
                    let window_clone = window.clone();
                    glib::spawn_future_local(async move {
                        // FIRST: Stop player and wait for completion
                        player_page.stop().await;

                        // THEN: Execute navigation on main thread
                        glib::idle_add_local_once(move || {
                            // Show header bar again and restore toolbar style
                            window_clone.imp().content_header.set_visible(true);
                            window_clone
                                .imp()
                                .content_toolbar
                                .set_top_bar_style(adw::ToolbarStyle::Raised);

                            // Restore window size from NavigationManager
                            if let Some(nav_manager) =
                                window_clone.imp().navigation_manager.borrow().as_ref()
                            {
                                let state = nav_manager.get_saved_window_state();
                                if let Some((width, height)) = state.saved_size {
                                    window_clone.set_default_size(width, height);
                                }
                                if state.was_maximized {
                                    window_clone.maximize();
                                } else if state.was_fullscreen {
                                    window_clone.fullscreen();
                                }
                            }

                            // Restore sidebar
                            if let Some(content) = window_clone.content()
                                && let Some(split_view) =
                                    content.downcast_ref::<adw::NavigationSplitView>()
                            {
                                split_view.set_collapsed(false);
                                split_view.set_show_content(true);
                            }

                            // Use NavigationManager to go back to the previous page
                            if let Some(nav_manager) =
                                window_clone.imp().navigation_manager.borrow().as_ref()
                            {
                                let nav_manager = Arc::clone(nav_manager);
                                glib::spawn_future_local(async move {
                                    nav_manager.go_back().await;
                                });
                            } else {
                                // Fallback if NavigationManager is not available
                                if let Some(stack) =
                                    window_clone.imp().content_stack.borrow().as_ref()
                                {
                                    if stack.child_by_name("home").is_some() {
                                        stack.set_visible_child_name("home");
                                    } else if stack.child_by_name("library").is_some() {
                                        stack.set_visible_child_name("library");
                                    }
                                }
                            }
                        });
                    });
                }
            }
        });

        // Save current window size to NavigationManager before changing it
        let (current_width, current_height) = self.default_size();
        if let Some(nav_manager) = imp.navigation_manager.borrow().as_ref() {
            let nav_manager = Arc::clone(nav_manager);
            let window_weak = self.downgrade();
            glib::spawn_future_local(async move {
                if let Some(window) = window_weak.upgrade() {
                    let is_maximized = window.is_maximized();
                    let is_fullscreen = window.is_fullscreen();
                    nav_manager
                        .save_current_window_state(
                            Some((current_width, current_height)),
                            is_maximized,
                            is_fullscreen,
                        )
                        .await;
                }
            });
        }

        // Navigation stack management is now handled by NavigationManager
        // The NavigationManager will track this navigation automatically when we call navigate_to
        if let Some(current_page) = content_stack.visible_child_name() {
            info!(
                "MainWindow::show_player() - Current page '{}' will be tracked by NavigationManager",
                current_page
            );
        }

        // Defer the transition for smoother performance
        info!("MainWindow::show_player() - Switching stack to 'player' page");
        let media_item_clone = media_item.clone();
        let state_clone = state.clone();
        let player_page_clone = player_page.clone();
        let window_weak = self.downgrade();
        glib::timeout_add_local_once(std::time::Duration::from_millis(10), {
            let content_stack = content_stack.clone();
            let media_item = media_item_clone;
            let state = state_clone;
            let player_page = player_page_clone;
            move || {
                content_stack.set_visible_child_name("player");
                info!("MainWindow::show_player() - Navigation to player complete");

                // Load media after the page is visible so GLArea can be properly realized
                glib::spawn_future_local(async move {
                    debug!("Loading media after page transition");
                    if let Err(e) = player_page.load_media(&media_item, state).await {
                        error!("Failed to load media: {}", e);
                        // Error is already handled by the PlayerPage error state - no dialog needed
                    }
                });
            }
        });

        // Performance monitoring
        let elapsed = start_time.elapsed();
        if elapsed > std::time::Duration::from_millis(16) {
            tracing::warn!("show_player took {:?} (exceeds frame budget)", elapsed);
        }

        // Navigate to the content pane and collapse the sidebar for immersive playback
        if let Some(content) = self.content()
            && let Some(split_view) = content.downcast_ref::<adw::NavigationSplitView>()
        {
            // Ensure content is visible first
            split_view.set_show_content(true);
            // Then collapse the sidebar for immersive video playback
            split_view.set_collapsed(true);
        }

        // Try to resize window to match video aspect ratio after a short delay
        // (to give GStreamer time to negotiate the video format)
        let _window_weak = self.downgrade();
        let player_page_clone = player_page.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
            let window_weak = window_weak.clone();
            let player_page = player_page_clone.clone();
            glib::spawn_future_local(async move {
                if let Some(window) = window_weak.upgrade()
                    && let Some((width, height)) = player_page.get_video_dimensions().await
                {
                    // Calculate aspect ratio
                    let aspect_ratio = width as f64 / height as f64;

                    // Calculate new width based on aspect ratio
                    // Use a reasonable height (e.g., 720p)
                    let target_height = 720.min(height).max(480);
                    let target_width = (target_height as f64 * aspect_ratio) as i32;

                    // Set the new window size
                    window.set_default_size(target_width, target_height);

                    info!(
                        "Resized window to {}x{} (aspect ratio: {:.2})",
                        target_width, target_height, aspect_ratio
                    );
                }
            });
            glib::ControlFlow::Break
        });
    }

    async fn show_library_view(&self, backend_id: String, library: crate::models::Library) {
        let imp = self.imp();
        let start_time = std::time::Instant::now();

        // Cleanup current page if needed
        self.cleanup_current_page().await;

        // Get content stack
        let content_stack = self.ensure_content_stack();

        // Set transition for library pages
        content_stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);
        content_stack.set_transition_duration(300);

        // Use PageFactory to get or create library view
        let library_view = if let Some(page_factory) = imp.page_factory.borrow().as_ref() {
            let view = page_factory.get_or_create_library_page();

            // Ensure page is in the stack
            if content_stack.child_by_name("library").is_none() {
                content_stack.add_named(&view.clone().upcast::<gtk4::Widget>(), Some("library"));
            }

            // Set up the page with callbacks
            let window_weak = self.downgrade();
            page_factory.setup_library_page(&view, move |media_item| {
                info!("Library - Media selected: {}", media_item.title());
                if let Some(window) = window_weak.upgrade() {
                    let media_item = media_item.clone();
                    glib::spawn_future_local(async move {
                        use super::navigation_request::NavigationRequest;
                        use crate::models::MediaItem;
                        let nav_request = match &media_item {
                            MediaItem::Movie(movie) => {
                                NavigationRequest::ShowMovieDetails(movie.clone())
                            }
                            MediaItem::Show(show) => {
                                NavigationRequest::ShowShowDetails(show.clone())
                            }
                            MediaItem::Episode(_) => NavigationRequest::ShowPlayer(media_item),
                            _ => {
                                info!("Library - Unsupported media type");
                                return;
                            }
                        };
                        window.navigate_to(nav_request).await;
                    });
                }
            });

            // Store reference for compatibility (will be removed later)
            imp.library_view.replace(Some(view.clone()));
            view
        } else {
            tracing::error!("PageFactory not initialized");
            return;
        };

        // Update the content page title
        imp.content_page.set_title(&library.title);

        // Update header bar title
        imp.content_header.set_title_widget(Some(
            &gtk4::Label::builder()
                .label(&library.title)
                .single_line_mode(true)
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .build(),
        ));

        // Create filter controls for the header bar
        // Remove any existing filter controls to avoid duplicates when navigating
        if let Some(prev_controls) = imp.filter_controls.borrow().as_ref() {
            imp.content_header.remove(prev_controls);
        }
        let filter_controls = self.create_filter_controls(&library_view);
        imp.content_header.pack_end(&filter_controls);
        imp.filter_controls.replace(Some(filter_controls));

        // Start loading the library data
        library_view.load_library(backend_id, library).await;

        // Defer the transition for smoother performance
        // The await above ensures data starts loading before transition
        glib::timeout_add_local_once(std::time::Duration::from_millis(10), {
            let content_stack = content_stack.clone();
            move || {
                content_stack.set_visible_child_name("library");
            }
        });

        // Performance monitoring
        let elapsed = start_time.elapsed();
        if elapsed > std::time::Duration::from_millis(16) {
            tracing::warn!(
                "show_library_view took {:?} (exceeds frame budget)",
                elapsed
            );
        }

        // Get the split view from the window content and show content pane on mobile
        if let Some(content) = self.content()
            && let Some(split_view) = content.downcast_ref::<adw::NavigationSplitView>()
        {
            split_view.set_show_content(true);
        }
    }

    pub fn show_libraries_view(&self) {
        info!("Navigating back to libraries");

        let imp = self.imp();

        // Show empty state in content area
        if let Some(stack) = imp.content_stack.borrow().as_ref() {
            stack.set_visible_child_name("empty");
        }

        // Reset content page title
        imp.content_page.set_title("Content");

        // Reset header bar title
        imp.content_header.set_title_widget(gtk4::Widget::NONE);

        // Show sidebar in mobile view
        if let Some(content) = self.content()
            && let Some(split_view) = content.downcast_ref::<adw::NavigationSplitView>()
        {
            split_view.set_show_content(false);
        }
    }

    fn create_filter_controls(
        &self,
        library_view: &crate::platforms::gtk::ui::pages::LibraryView,
    ) -> gtk4::Box {
        let controls_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .build();

        // Create watch status dropdown
        let watch_status_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(4)
            .build();

        let watch_label = gtk4::Label::builder().label("Show:").build();
        watch_label.add_css_class("dim-label");

        let watch_model = gtk4::StringList::new(&["All", "Unwatched", "Watched", "In Progress"]);

        let watch_dropdown = gtk4::DropDown::builder()
            .model(&watch_model)
            .selected(0)
            .tooltip_text("Filter by watch status")
            .build();

        watch_status_box.append(&watch_label);
        watch_status_box.append(&watch_dropdown);

        // Create sort order dropdown
        let sort_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(4)
            .build();

        let sort_label = gtk4::Label::builder().label("Sort:").build();
        sort_label.add_css_class("dim-label");

        let sort_model = gtk4::StringList::new(&[
            "Title (A-Z)",
            "Title (Z-A)",
            "Year (Oldest)",
            "Year (Newest)",
            "Rating (Low-High)",
            "Rating (High-Low)",
            "Date Added (Oldest)",
            "Date Added (Newest)",
        ]);

        let sort_dropdown = gtk4::DropDown::builder()
            .model(&sort_model)
            .selected(0)
            .tooltip_text("Sort order")
            .build();

        sort_box.append(&sort_label);
        sort_box.append(&sort_dropdown);

        // Connect watch status filter handler
        let library_view_weak = library_view.downgrade();
        watch_dropdown.connect_selected_notify(move |dropdown| {
            if let Some(view) = library_view_weak.upgrade() {
                let status = match dropdown.selected() {
                    0 => WatchStatus::All,
                    1 => WatchStatus::Unwatched,
                    2 => WatchStatus::Watched,
                    3 => WatchStatus::InProgress,
                    _ => WatchStatus::All,
                };
                view.update_watch_status_filter(status);
            }
        });

        // Connect sort order handler
        let library_view_weak = library_view.downgrade();
        sort_dropdown.connect_selected_notify(move |dropdown| {
            if let Some(view) = library_view_weak.upgrade() {
                let order = match dropdown.selected() {
                    0 => SortOrder::TitleAsc,
                    1 => SortOrder::TitleDesc,
                    2 => SortOrder::YearAsc,
                    3 => SortOrder::YearDesc,
                    4 => SortOrder::RatingAsc,
                    5 => SortOrder::RatingDesc,
                    6 => SortOrder::DateAddedAsc,
                    7 => SortOrder::DateAddedDesc,
                    _ => SortOrder::TitleAsc,
                };
                view.update_sort_order(order);
            }
        });

        // Add controls to the box
        controls_box.append(&watch_status_box);
        controls_box.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
        controls_box.append(&sort_box);

        // Future: Add more filter buttons here (genre, year range, etc.)
        // Example:
        // let filter_button = gtk4::Button::builder()
        //     .icon_name("view-filter-symbolic")
        //     .tooltip_text("More filters")
        //     .build();
        // controls_box.append(&filter_button);

        controls_box
    }

    pub async fn update_backend_selector(&self) {
        // Sources button is always visible at the bottom - no selector needed
    }

    // Generic navigation handler - now uses NavigationManager as central coordinator
    pub async fn navigate_to(
        &self,
        request: crate::platforms::gtk::ui::navigation_request::NavigationRequest,
    ) {
        use super::navigation::NavigationPage;
        use super::navigation_request::NavigationRequest;

        tracing::info!("MainWindow: navigate_to called with request: {:?}", request);

        // Convert NavigationRequest to NavigationPage and use NavigationManager
        if let Some(nav_manager) = self.imp().navigation_manager.borrow().as_ref() {
            let nav_page = self.navigation_request_to_page(&request);
            tracing::info!("MainWindow: Converted to NavigationPage: {:?}", nav_page);

            let nav_manager = Arc::clone(nav_manager);

            // Update navigation state first
            match request {
                NavigationRequest::GoBack => {
                    nav_manager.go_back().await;
                    // Load the page we went back to
                    let current_page = nav_manager.current_page();
                    self.load_page_for_navigation_page(current_page).await;
                }
                NavigationRequest::RefreshCurrentPage => {
                    // Reload the current page without changing navigation state
                    let current_page = nav_manager.current_page();
                    self.load_page_for_navigation_page(current_page).await;
                }
                NavigationRequest::ClearHistory => {
                    nav_manager
                        .navigate_to_root(NavigationPage::Home { source_id: None })
                        .await;
                    self.show_home_page_for_source(None);
                }
                _ => {
                    // For all other requests, update navigation state then load the page
                    if let Some(page) = nav_page {
                        tracing::info!("MainWindow: Updating navigation state to {:?}", page);
                        nav_manager.navigate_to(page).await;
                    }

                    // Now load the actual page
                    self.load_page_for_request(request).await;
                }
            }
        } else {
            // NavigationManager should always be available
            tracing::error!("NavigationManager not initialized! This should never happen.");
        }
    }

    // Helper method to load a page based on NavigationRequest
    async fn load_page_for_request(
        &self,
        request: crate::platforms::gtk::ui::navigation_request::NavigationRequest,
    ) {
        use super::navigation_request::NavigationRequest;

        let state = self.imp().state.borrow().as_ref().unwrap().clone();

        match request {
            NavigationRequest::ShowHome(source_id) => {
                tracing::info!("Loading home page");
                self.show_home_page_for_source(source_id);
            }
            NavigationRequest::ShowSources => {
                tracing::info!("Loading sources page");
                self.show_sources_page();
            }
            NavigationRequest::ShowMovieDetails(movie) => {
                tracing::info!("Loading movie details");
                self.show_movie_details(movie, state).await;
            }
            NavigationRequest::ShowShowDetails(show) => {
                tracing::info!("Loading show details");
                self.show_show_details(show, state).await;
            }
            NavigationRequest::ShowPlayer(media_item) => {
                tracing::info!("Loading player");
                self.show_player(&media_item, state).await;
            }
            NavigationRequest::ShowLibrary(identifier, library) => {
                tracing::info!("Loading library view");
                self.show_library_view(identifier.source_id.clone(), library)
                    .await;
            }
            NavigationRequest::ShowLibraryByKey(library_key) => {
                tracing::info!("Loading library by key: {}", library_key);
                self.navigate_to_library(&library_key).await;
            }
            _ => {
                // GoBack, RefreshCurrentPage, ClearHistory are handled above
            }
        }
    }

    // Helper method to load a page based on NavigationPage
    async fn load_page_for_navigation_page(&self, page: super::navigation::NavigationPage) {
        use super::navigation::NavigationPage;

        let state = self.imp().state.borrow().as_ref().unwrap().clone();

        match page {
            NavigationPage::Home { source_id } => {
                self.show_home_page_for_source(source_id);
            }
            NavigationPage::Sources => {
                self.show_sources_page();
            }
            NavigationPage::Library {
                backend_id,
                library_id,
                title: _,
            } => {
                // Need to fetch the library object
                if let Some(backend) = state.source_coordinator.get_backend(&backend_id).await {
                    if let Ok(libraries) = backend.get_libraries().await {
                        if let Some(library) = libraries.iter().find(|l| l.id == library_id) {
                            self.show_library_view(backend_id, library.clone()).await;
                        }
                    }
                }
            }
            NavigationPage::MovieDetails { movie_id, title: _ } => {
                // Fetch movie from database and show details
                if let Some(media_item) = state
                    .data_service
                    .get_media_item(&movie_id)
                    .await
                    .ok()
                    .flatten()
                {
                    if let MediaItem::Movie(movie) = media_item {
                        self.show_movie_details(movie, state).await;
                    } else {
                        tracing::error!("Media item {} is not a movie", movie_id);
                    }
                } else {
                    tracing::error!("Failed to fetch movie with ID: {}", movie_id);
                }
            }
            NavigationPage::ShowDetails { show_id, title: _ } => {
                // Fetch show from database and show details
                if let Some(media_item) = state
                    .data_service
                    .get_media_item(&show_id)
                    .await
                    .ok()
                    .flatten()
                {
                    if let MediaItem::Show(show) = media_item {
                        self.show_show_details(show, state).await;
                    } else {
                        tracing::error!("Media item {} is not a show", show_id);
                    }
                } else {
                    tracing::error!("Failed to fetch show with ID: {}", show_id);
                }
            }
            NavigationPage::Player { media_id, title: _ } => {
                // Fetch media from database and show player
                if let Some(media_item) = state
                    .data_service
                    .get_media_item(&media_id)
                    .await
                    .ok()
                    .flatten()
                {
                    self.show_player(&media_item, state).await;
                } else {
                    tracing::error!("Failed to fetch media item with ID: {}", media_id);
                }
            }
            NavigationPage::Empty => {
                // No page to load for Empty
                tracing::debug!("Empty navigation page - nothing to load");
            }
        }
    }

    // Helper to convert NavigationRequest to NavigationPage
    fn navigation_request_to_page(
        &self,
        request: &crate::platforms::gtk::ui::navigation_request::NavigationRequest,
    ) -> Option<super::navigation::NavigationPage> {
        use super::navigation::NavigationPage;
        use super::navigation_request::{LibraryIdentifier, NavigationRequest};

        match request {
            NavigationRequest::ShowHome(source_id) => Some(NavigationPage::Home {
                source_id: source_id.clone(),
            }),
            NavigationRequest::ShowSources => Some(NavigationPage::Sources),
            NavigationRequest::ShowMovieDetails(movie) => Some(NavigationPage::MovieDetails {
                movie_id: movie.id.clone(),
                title: movie.title.clone(),
            }),
            NavigationRequest::ShowShowDetails(show) => Some(NavigationPage::ShowDetails {
                show_id: show.id.clone(),
                title: show.title.clone(),
            }),
            NavigationRequest::ShowPlayer(media_item) => Some(NavigationPage::Player {
                media_id: media_item.id().to_string(),
                title: media_item.title().to_string(),
            }),
            NavigationRequest::ShowLibrary(identifier, library) => Some(NavigationPage::Library {
                backend_id: identifier.source_id.clone(),
                library_id: identifier.library_id.clone(),
                title: library.title.clone(),
            }),
            NavigationRequest::ShowLibraryByKey(library_key) => {
                // Parse the old format and convert to new format
                if let Some(identifier) = LibraryIdentifier::from_string(library_key) {
                    // We need to fetch the library title, for now use a placeholder
                    Some(NavigationPage::Library {
                        backend_id: identifier.source_id,
                        library_id: identifier.library_id,
                        title: "Library".to_string(), // TODO: Fetch actual library title
                    })
                } else {
                    None
                }
            }
            NavigationRequest::GoBack => None, // Handle separately
            NavigationRequest::RefreshCurrentPage => None, // Handle separately
            NavigationRequest::ClearHistory => None, // Handle separately
        }
    }

    // Helper to ensure content stack exists
    fn ensure_content_stack(&self) -> gtk4::Stack {
        let imp = self.imp();

        if imp.content_stack.borrow().is_none() {
            let stack = gtk4::Stack::new();
            stack.add_named(&*imp.empty_state, Some("empty"));
            imp.content_toolbar.set_content(Some(&stack));
            imp.content_stack.replace(Some(stack.clone()));

            // Connect the stack to the NavigationManager
            if let Some(nav_manager) = imp.navigation_manager.borrow().as_ref() {
                nav_manager.set_content_stack(stack.clone());
            }

            stack
        } else {
            imp.content_stack.borrow().as_ref().unwrap().clone()
        }
    }

    // Progressive initialization is now handled by the Sidebar widget's reactive bindings

    // Backend switching removed - each view must track its own backend_id
    // The UI should be refactored to either:
    // 1. Show content from all backends simultaneously
    // 2. Have a backend selector that filters the view
    // 3. Pass backend_id through the navigation hierarchy
}
