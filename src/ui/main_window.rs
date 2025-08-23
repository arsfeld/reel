use gtk4::{gio, glib, glib::clone, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::config::Config;
use crate::constants::PLAYER_CONTROLS_HIDE_DELAY_SECS;
use crate::state::AppState;
use crate::ui::filters::{SortOrder, WatchStatus};
use crate::ui::pages;
use tokio::sync::RwLock;

mod imp {
    use super::*;

    use gtk4::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/dev/arsfeld/Reel/window.ui")]
    pub struct ReelMainWindow {
        #[template_child]
        pub welcome_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub connect_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub home_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub home_list: TemplateChild<gtk4::ListBox>,
        #[template_child]
        pub sources_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub sources_container: TemplateChild<gtk4::Box>,
        #[template_child]
        pub status_container: TemplateChild<gtk4::Box>,
        #[template_child]
        pub status_icon: TemplateChild<gtk4::Image>,
        #[template_child]
        pub status_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub sync_spinner: TemplateChild<gtk4::Spinner>,
        #[template_child]
        pub empty_state: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub content_page: TemplateChild<adw::NavigationPage>,
        #[template_child]
        pub content_toolbar: TemplateChild<adw::ToolbarView>,
        #[template_child]
        pub content_header: TemplateChild<adw::HeaderBar>,

        pub state: RefCell<Option<Arc<AppState>>>,
        pub config: RefCell<Option<Arc<RwLock<Config>>>>,
        pub content_stack: RefCell<Option<gtk4::Stack>>,
        pub home_page: RefCell<Option<crate::ui::pages::HomePage>>,
        pub sources_page: RefCell<Option<crate::ui::pages::SourcesPage>>,
        pub library_view: RefCell<Option<crate::ui::pages::LibraryView>>,
        pub player_page: RefCell<Option<crate::ui::pages::PlayerPage>>,
        pub show_details_page: RefCell<Option<crate::ui::pages::ShowDetailsPage>>,
        pub movie_details_page: RefCell<Option<crate::ui::pages::MovieDetailsPage>>,
        pub back_button: RefCell<Option<gtk4::Button>>,
        pub saved_window_size: RefCell<(i32, i32)>,
        pub filter_controls: RefCell<Option<gtk4::Box>>,
        pub edit_mode: RefCell<bool>,
        pub library_visibility: RefCell<std::collections::HashMap<String, bool>>,
        pub all_libraries: RefCell<Vec<(crate::models::Library, usize)>>,
        pub navigation_stack: RefCell<Vec<String>>, // Track navigation history
        pub header_add_button: RefCell<Option<gtk4::Button>>, // Track add button in header
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

            let obj = self.obj();

            // Connect signals
            self.connect_button.connect_clicked(clone!(
                #[weak]
                obj,
                move |_| {
                    obj.show_auth_dialog();
                }
            ));

            // Status is now more subtle, no need for clickable row

            // No longer need refresh_button and edit_libraries_button connections here
            // They will be handled per-source group

            // Connect to home list row activation
            self.home_list.connect_row_activated(clone!(
                #[weak]
                obj,
                move |_, row| {
                    if let Some(action_row) = row.downcast_ref::<adw::ActionRow>() {
                        info!("Home selected");
                        obj.show_home_page();
                    }
                }
            ));

            // Connect to sources button click
            self.sources_button.connect_clicked(clone!(
                #[weak]
                obj,
                move |_| {
                    info!("Sources button clicked");
                    obj.show_sources_page();
                }
            ));

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

        // Setup actions
        window.setup_actions(app);

        // Apply theme
        let window_weak = window.downgrade();
        glib::spawn_future_local(async move {
            if let Some(window) = window_weak.upgrade() {
                window.apply_theme().await;
            }
        });

        // FIRST: Try to load any cached data immediately (before authentication)
        window.load_cached_data_on_startup(state.clone());

        // THEN: Check for existing backends and load them
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

    fn load_cached_data_on_startup(&self, state: Arc<AppState>) {
        let window_weak = self.downgrade();

        // Immediately try to load cached data without waiting for authentication
        glib::spawn_future_local(async move {
            if let Some(window) = window_weak.upgrade() {
                info!("Checking for cached data on startup...");

                // Get all providers from source coordinator
                if let Some(source_coordinator) = state.get_source_coordinator().await {
                    // Load saved providers first
                    if let Err(e) = source_coordinator.get_auth_manager().load_providers().await {
                        error!("Failed to load providers: {}", e);
                    }

                    let providers = source_coordinator
                        .get_auth_manager()
                        .get_all_providers()
                        .await;
                    if providers.is_empty() {
                        info!("No auth providers found, skipping cache load");
                        return;
                    }

                    // Use the sync manager from AppState
                    let sync_manager = state.sync_manager.clone();
                    let mut has_any_cached_data = false;
                    let mut backends_libraries = Vec::new();
                    let provider_count = providers.len();

                    // Try to load cached data from all providers
                    for provider in &providers {
                        let provider_id = provider.id();
                        info!("Checking cached data for provider: {}", provider_id);

                        match sync_manager.get_cached_libraries(provider_id).await {
                            Ok(libraries) if !libraries.is_empty() => {
                                info!(
                                    "Found {} cached libraries for provider {}",
                                    libraries.len(),
                                    provider_id
                                );

                                // Build library list with counts
                                let mut library_info = Vec::new();

                                for library in &libraries {
                                    use crate::models::LibraryType;
                                    let item_count = match library.library_type {
                                        LibraryType::Movies => {
                                            match sync_manager
                                                .get_cached_movies(provider_id, &library.id)
                                                .await
                                            {
                                                Ok(movies) => movies.len(),
                                                Err(_) => 0,
                                            }
                                        }
                                        LibraryType::Shows => {
                                            match sync_manager
                                                .get_cached_shows(provider_id, &library.id)
                                                .await
                                            {
                                                Ok(shows) => shows.len(),
                                                Err(_) => 0,
                                            }
                                        }
                                        _ => 0,
                                    };

                                    library_info.push((library.clone(), item_count));
                                }

                                if !library_info.is_empty() {
                                    backends_libraries
                                        .push((provider_id.to_string(), library_info));
                                    has_any_cached_data = true;
                                }
                            }
                            Ok(_) => {
                                info!("No cached libraries for provider {}", provider_id);
                            }
                            Err(e) => {
                                info!(
                                    "Could not load cached libraries for provider {}: {}",
                                    provider_id, e
                                );
                            }
                        }
                    }

                    if has_any_cached_data {
                        info!(
                            "Found cached data from {} providers, showing immediately",
                            backends_libraries.len()
                        );

                        // Hide welcome page and show libraries immediately
                        window.imp().welcome_page.set_visible(false);
                        window.imp().sources_container.set_visible(true);
                        window.imp().home_group.set_visible(true);

                        // Update UI with all backends' libraries
                        window.update_all_backends_libraries(backends_libraries);

                        // Update subtle status for cached data
                        window.imp().status_container.set_visible(true);
                        window.imp().status_label.set_text(&format!(
                            "{} source{} (cached)",
                            provider_count,
                            if provider_count == 1 { "" } else { "s" }
                        ));
                        window
                            .imp()
                            .status_icon
                            .set_icon_name(Some("folder-remote-symbolic"));
                    } else {
                        info!("No cached data found from any provider");
                    }
                } else {
                    error!("SourceCoordinator not initialized");
                }
            }
        });
    }

    fn check_and_load_backends(&self, state: Arc<AppState>) {
        let window_weak = self.downgrade();

        glib::spawn_future_local(async move {
            // Use SourceCoordinator to initialize all sources
            if let Some(source_coordinator) = state.get_source_coordinator().await {
                // First, load saved providers from config
                if let Err(e) = source_coordinator.get_auth_manager().load_providers().await {
                    error!("Failed to load providers: {}", e);
                }

                // Then, migrate any legacy backends
                if let Err(e) = source_coordinator.migrate_legacy_backends().await {
                    error!("Failed to migrate legacy backends: {}", e);
                }

                // Initialize all sources
                match source_coordinator.initialize_all_sources().await {
                    Ok(source_statuses) => {
                        let connected_count = source_statuses
                            .iter()
                            .filter(|s| matches!(s.connection_status, crate::services::source_coordinator::ConnectionStatus::Connected))
                            .count();

                        info!(
                            "Initialized {} sources, {} connected",
                            source_statuses.len(),
                            connected_count
                        );
                    }
                    Err(e) => {
                        error!("Failed to initialize sources: {}", e);
                    }
                }

                // Handle the results of backend initialization
                if let Some(window) = window_weak.upgrade() {
                    // Update backend selector (now just hides it)
                    window.update_backend_selector().await;

                    // Check how many backends are connected
                    let backend_manager = state.backend_manager.read().await;
                    let all_backends = backend_manager.get_all_backends();
                    let connected_count = all_backends.len();
                    drop(backend_manager);

                    if connected_count > 0 {
                        info!("Successfully initialized {} backends", connected_count);
                        window.update_connection_status(true).await;

                        // Update subtle status
                        window.imp().status_container.set_visible(true);
                        window.imp().status_label.set_text(&format!(
                            "{} source{} connected",
                            connected_count,
                            if connected_count == 1 { "" } else { "s" }
                        ));
                        window
                            .imp()
                            .status_icon
                            .set_icon_name(Some("network-transmit-receive-symbolic"));

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

                            // Show subtle authentication needed status
                            window.imp().status_container.set_visible(true);
                            window
                                .imp()
                                .status_label
                                .set_text("Authentication required");
                            window
                                .imp()
                                .status_icon
                                .set_icon_name(Some("dialog-password-symbolic"));
                        }
                    }
                }
            } else {
                error!("SourceCoordinator not initialized");
            }
        });
    }

    fn setup_state_subscriptions(&self) {
        // Listen for state changes
        // For now, we'll handle updates manually when auth completes
    }

    pub async fn update_connection_status(&self, connected: bool) {
        let imp = self.imp();

        if connected {
            // Don't set default values here - let update_user_display handle the details
            imp.welcome_page.set_visible(false); // Hide welcome message
            imp.home_group.set_visible(true);
            imp.sources_container.set_visible(true);

            // Show subtle status
            imp.status_container.set_visible(true);
            imp.status_label.set_text("Connected");
            imp.status_icon
                .set_icon_name(Some("network-transmit-receive-symbolic"));
        } else {
            imp.status_container.set_visible(false);
            imp.welcome_page.set_visible(true); // Show welcome message
            imp.home_group.set_visible(false);
            imp.sources_container.set_visible(false);
        }
    }

    pub async fn update_user_display_with_backend(
        &self,
        user: Option<crate::models::User>,
        backend_manager: &crate::backends::BackendManager,
    ) {
        let imp = self.imp();
        info!(
            "update_user_display_with_backend called with user: {:?}",
            user.as_ref().map(|u| &u.username)
        );

        if let Some(user) = user {
            if let Some(backend) = backend_manager.get_active() {
                info!(
                    "Active backend found, is_initialized: {}",
                    backend.is_initialized().await
                );
                if backend.is_initialized().await {
                    // Get backend info using the trait method
                    let backend_info = backend.get_backend_info().await;
                    info!("Got backend info: {:?}", backend_info);

                    // Check if we're in offline mode
                    if user.id == "offline"
                        && backend_info.connection_type
                            == crate::backends::traits::ConnectionType::Unknown
                    {
                        // We have cached credentials but can't connect
                        imp.status_container.set_visible(true);
                        imp.status_label.set_text("Offline (cached)");
                        imp.status_icon
                            .set_icon_name(Some("network-offline-symbolic"));
                    } else {
                        // Create detailed status based on connection type
                        use crate::backends::traits::ConnectionType;
                        let (status_text, icon_name) = match backend_info.connection_type {
                            ConnectionType::Local => {
                                ("Connected (local)", "network-wired-symbolic")
                            }
                            ConnectionType::Remote => {
                                ("Connected (remote)", "network-wireless-symbolic")
                            }
                            ConnectionType::Relay => {
                                ("Connected (relay)", "network-cellular-symbolic")
                            }
                            ConnectionType::Offline => {
                                ("Offline (cached)", "network-offline-symbolic")
                            }
                            ConnectionType::Unknown => {
                                ("Connected", "network-transmit-receive-symbolic")
                            }
                        };

                        imp.status_container.set_visible(true);
                        imp.status_label.set_text(status_text);
                        imp.status_icon.set_icon_name(Some(icon_name));
                    }
                } else {
                    imp.status_container.set_visible(true);
                    imp.status_label.set_text("Authenticated");
                    imp.status_icon.set_icon_name(Some("network-idle-symbolic"));
                }
            } else {
                imp.status_container.set_visible(true);
                imp.status_label.set_text("Not connected");
                imp.status_icon
                    .set_icon_name(Some("network-offline-symbolic"));
            }
        } else {
            imp.status_container.set_visible(false);
        }
    }

    pub async fn update_user_display(&self, user: Option<crate::models::User>) {
        let imp = self.imp();

        if let Some(user) = user {
            // Check if backend is initialized
            let state = self.imp().state.borrow().as_ref().unwrap().clone();
            let backend_manager = state.backend_manager.read().await;

            if let Some(backend) = backend_manager.get_active() {
                if backend.is_initialized().await {
                    // Get backend info using the trait method
                    let backend_info = backend.get_backend_info().await;

                    // Create detailed status based on connection type
                    use crate::backends::traits::ConnectionType;
                    let (status_text, icon_name) = match backend_info.connection_type {
                        ConnectionType::Local => ("Connected (local)", "network-wired-symbolic"),
                        ConnectionType::Remote => {
                            ("Connected (remote)", "network-wireless-symbolic")
                        }
                        ConnectionType::Relay => ("Connected (relay)", "network-cellular-symbolic"),
                        ConnectionType::Offline => ("Offline (cached)", "network-offline-symbolic"),
                        ConnectionType::Unknown => {
                            ("Connected", "network-transmit-receive-symbolic")
                        }
                    };

                    imp.status_container.set_visible(true);
                    imp.status_label.set_text(status_text);
                    imp.status_icon.set_icon_name(Some(icon_name));
                } else {
                    imp.status_container.set_visible(true);
                    imp.status_label.set_text("Authenticated");
                    imp.status_icon.set_icon_name(Some("network-idle-symbolic"));
                }
            } else {
                imp.status_container.set_visible(true);
                imp.status_label.set_text("Not connected");
                imp.status_icon
                    .set_icon_name(Some("network-offline-symbolic"));
            }
        } else {
            imp.status_container.set_visible(false);
        }
    }

    pub fn show_auth_dialog(&self) {
        info!("Showing authentication dialog");

        // Get state from the window
        let state = self.imp().state.borrow().as_ref().unwrap().clone();

        // Create and show auth dialog
        let dialog = crate::ui::AuthDialog::new(state.clone());
        dialog.present(Some(self));

        // Start authentication automatically
        dialog.start_auth();

        // Set up a callback for when the dialog closes
        let window_weak = self.downgrade();
        let state_clone = state.clone();
        dialog.connect_closed(move |_| {
            if let Some(window) = window_weak.upgrade() {
                // Check if we now have an authenticated backend
                let state_for_async = state_clone.clone();
                glib::spawn_future_local(async move {
                    let backend_manager = state_for_async.backend_manager.read().await;
                    if let Some(backend) = backend_manager.get_active()
                        && backend.is_initialized().await
                    {
                        info!("Backend initialized after auth dialog closed");

                        // Update backend selector with new backend
                        window.update_backend_selector().await;

                        // Update connection status
                        window.update_connection_status(true).await;

                        // Update user display
                        if let Some(user) = state_for_async.get_user().await {
                            window
                                .update_user_display_with_backend(Some(user), &backend_manager)
                                .await;
                        }

                        // FIRST: Load cached data immediately
                        if let Some(backend_id) = state_for_async.get_active_backend_id().await {
                            info!("Loading cached libraries for instant display...");
                            window.load_cached_libraries_for_backend(&backend_id).await;

                            // Save this backend as the last active
                            {
                                let mut config = state_for_async.config.write().await;
                                let _ = config.set_last_active_backend(&backend_id);
                            }
                        }

                        // THEN: Start background sync
                        let backend_clone = backend.clone();
                        let state_clone = state_for_async.clone();
                        let window_weak = window.downgrade();
                        glib::spawn_future_local(async move {
                            if let Some(window) = window_weak.upgrade() {
                                info!("Starting background sync after auth...");
                                // Show sync progress
                                window.show_sync_progress(true);

                                // Start sync with backend ID
                                if let Some(backend_id) = state_clone.get_active_backend_id().await
                                {
                                    window
                                        .sync_and_update_libraries(
                                            &backend_id,
                                            backend_clone,
                                            state_clone,
                                        )
                                        .await;
                                }

                                // Hide sync progress
                                window.show_sync_progress(false);
                            }
                        });
                    }
                });
            }
        });
    }

    fn show_preferences(&self) {
        info!("Showing preferences");

        if let Some(config) = self.imp().config.borrow().as_ref() {
            let prefs_window = crate::ui::PreferencesWindow::new(self, config.clone());
            prefs_window.present();
        }
    }

    fn show_about(&self) {
        let about = adw::AboutWindow::builder()
            .application_name("Reel")
            .application_icon("dev.arsfeld.Reel")
            .developer_name("Alexandre Rosenfeld")
            .version("0.1.0")
            .license_type(gtk4::License::Gpl30)
            .website("https://github.com/arsfeld/reel")
            .issue_url("https://github.com/arsfeld/reel/issues")
            .build();

        about.set_transient_for(Some(self));
        about.present();
    }

    pub fn update_libraries(&self, libraries: Vec<(crate::models::Library, usize)>) {
        // For single backend, use update_all_backends_libraries
        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        let libraries_clone = libraries.clone();
        let window_weak = self.downgrade();

        glib::spawn_future_local(async move {
            if let Some(window) = window_weak.upgrade() {
                // Get backend ID
                if let Some(backend_id) = state.get_active_backend_id().await {
                    window.update_all_backends_libraries(vec![(backend_id, libraries_clone)]);
                }
            }
        });
    }

    pub fn update_all_backends_libraries(
        &self,
        backends_libraries: Vec<(String, Vec<(crate::models::Library, usize)>)>,
    ) {
        let imp = self.imp();

        // Clear existing source containers
        while let Some(child) = imp.sources_container.first_child() {
            imp.sources_container.remove(&child);
        }

        // Clear existing home rows
        while let Some(child) = imp.home_list.first_child() {
            imp.home_list.remove(&child);
        }

        // Add unified Home row for all sources
        let home_row = adw::ActionRow::builder()
            .title("Home")
            .subtitle("Recently added from all sources")
            .activatable(true)
            .build();

        let home_icon = gtk4::Image::from_icon_name("user-home-symbolic");
        home_row.add_prefix(&home_icon);

        let home_arrow = gtk4::Image::from_icon_name("go-next-symbolic");
        home_row.add_suffix(&home_arrow);

        home_row.set_widget_name("__home__");
        imp.home_list.append(&home_row);
        imp.home_group.set_visible(true);

        // Collect all libraries for edit mode
        let mut all_libraries = Vec::new();

        // Create a separate PreferencesGroup for each backend
        for (backend_id, libraries) in backends_libraries.iter() {
            if libraries.is_empty() {
                continue;
            }

            // Create a preferences group for this source
            let source_group = adw::PreferencesGroup::builder()
                .title(&self.get_backend_display_name(backend_id))
                .build();

            // Add edit/refresh buttons in the header suffix
            let header_buttons = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .spacing(6)
                .build();

            let edit_button = gtk4::Button::builder()
                .icon_name("document-edit-symbolic")
                .valign(gtk4::Align::Center)
                .tooltip_text("Edit Libraries")
                .css_classes(vec!["flat"])
                .build();

            let refresh_button = gtk4::Button::builder()
                .icon_name("view-refresh-symbolic")
                .valign(gtk4::Align::Center)
                .tooltip_text("Refresh")
                .css_classes(vec!["flat"])
                .build();

            header_buttons.append(&edit_button);
            header_buttons.append(&refresh_button);
            source_group.set_header_suffix(Some(&header_buttons));

            // Connect refresh button for this specific backend
            let backend_id_clone = backend_id.clone();
            let window_weak = self.downgrade();
            refresh_button.connect_clicked(move |_| {
                if let Some(window) = window_weak.upgrade() {
                    let backend_id = backend_id_clone.clone();
                    glib::spawn_future_local(async move {
                        window.sync_single_backend(&backend_id).await;
                    });
                }
            });

            // Create list box for libraries
            let libraries_list = gtk4::ListBox::builder()
                .selection_mode(gtk4::SelectionMode::None)
                .css_classes(vec!["boxed-list"])
                .build();

            // Add libraries for this backend
            for (library, item_count) in libraries {
                all_libraries.push((library.clone(), *item_count));

                let visibility_map = imp.library_visibility.borrow();
                let is_visible = visibility_map.get(&library.id).copied().unwrap_or(true);

                if is_visible {
                    let row = adw::ActionRow::builder()
                        .title(&library.title)
                        .subtitle(&format!("{} items", item_count))
                        .activatable(true)
                        .build();

                    // Add icon based on library type
                    let icon_name = match library.library_type {
                        crate::models::LibraryType::Movies => "video-x-generic-symbolic",
                        crate::models::LibraryType::Shows => "video-display-symbolic",
                        crate::models::LibraryType::Music => "audio-x-generic-symbolic",
                        crate::models::LibraryType::Photos => "image-x-generic-symbolic",
                        _ => "folder-symbolic",
                    };

                    let prefix_icon = gtk4::Image::from_icon_name(icon_name);
                    row.add_prefix(&prefix_icon);

                    let arrow = gtk4::Image::from_icon_name("go-next-symbolic");
                    row.add_suffix(&arrow);

                    // Store backend_id:library_id in widget name for navigation
                    row.set_widget_name(&format!("{}:{}", backend_id, library.id));

                    libraries_list.append(&row);
                }
            }

            // Connect row activation for this list
            let window_weak = self.downgrade();
            libraries_list.connect_row_activated(move |_, row| {
                if let Some(action_row) = row.downcast_ref::<adw::ActionRow>() {
                    if let Some(window) = window_weak.upgrade() {
                        let library_id = action_row.widget_name().to_string();
                        window.navigate_to_library(&library_id);
                    }
                }
            });

            source_group.add(&libraries_list);
            imp.sources_container.append(&source_group);
        }

        // Store all libraries for edit mode
        imp.all_libraries.replace(all_libraries);

        // Show sources container if we have any backends
        imp.sources_container
            .set_visible(!backends_libraries.is_empty());
    }

    fn get_backend_display_name(&self, backend_id: &str) -> String {
        // Get display name from cache or backend info
        if let Some(state) = self.imp().state.borrow().as_ref() {
            let state = state.clone();
            let backend_id = backend_id.to_string();
            let cache_key = format!("{}:server_name", backend_id);

            // Try to get cached server name synchronously (this should be refactored to async)
            // For now, just use the backend ID
            if backend_id.starts_with("plex") {
                "Plex Server".to_string()
            } else if backend_id.starts_with("jellyfin") {
                "Jellyfin Server".to_string()
            } else if backend_id.starts_with("local") {
                "Local Files".to_string()
            } else {
                backend_id.to_string()
            }
        } else {
            backend_id.to_string()
        }
    }

    pub fn show_sync_progress(&self, syncing: bool) {
        self.imp().sync_spinner.set_visible(syncing);
        self.imp().sync_spinner.set_spinning(syncing);
    }

    pub async fn load_cached_libraries_for_backend(&self, backend_id: &str) {
        info!("Loading libraries from cache...");

        // Get sync manager from AppState
        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        let sync_manager = state.sync_manager.clone();

        // Get cached libraries and update UI
        match sync_manager.get_cached_libraries(backend_id).await {
            Ok(libraries) => {
                if libraries.is_empty() {
                    info!("No cached libraries found");
                    return;
                }

                info!("Found {} cached libraries", libraries.len());

                // Build library list with counts
                let mut library_info = Vec::new();

                for library in &libraries {
                    use crate::models::LibraryType;
                    let item_count = match library.library_type {
                        LibraryType::Movies => {
                            // Get movie count for this library
                            match sync_manager
                                .get_cached_movies(backend_id, &library.id)
                                .await
                            {
                                Ok(movies) => movies.len(),
                                Err(_) => 0,
                            }
                        }
                        LibraryType::Shows => {
                            // Get show count for this library
                            match sync_manager.get_cached_shows(backend_id, &library.id).await {
                                Ok(shows) => shows.len(),
                                Err(_) => 0,
                            }
                        }
                        _ => {
                            // For other library types, we don't have counts yet
                            0
                        }
                    };

                    library_info.push((library.clone(), item_count));
                }

                info!("Updating UI with {} cached libraries", library_info.len());
                self.update_libraries(library_info);

                // Show home page instead of empty state if we have libraries
                self.show_home_page();
            }
            Err(e) => {
                info!("No cached libraries available: {}", e);
            }
        }
    }

    fn show_sources_page(&self) {
        info!("show_sources_page called");
        let imp = self.imp();

        // Clear header end widgets first
        self.clear_header_end_widgets();

        // Remove any filter controls since sources page doesn't need them
        if let Some(filter_controls) = imp.filter_controls.borrow().as_ref() {
            imp.content_header.remove(filter_controls);
        }
        imp.filter_controls.replace(None);

        // Get or create content stack
        let content_stack = if imp.content_stack.borrow().is_none() {
            let stack = gtk4::Stack::new();
            stack.add_named(&*imp.empty_state, Some("empty"));
            imp.content_toolbar.set_content(Some(&stack));
            imp.content_stack.replace(Some(stack.clone()));
            stack
        } else {
            imp.content_stack.borrow().as_ref().unwrap().clone()
        };

        // Create sources page if it doesn't exist
        if content_stack.child_by_name("sources").is_none() {
            if let Some(state) = &*imp.state.borrow() {
                let sources_page = pages::SourcesPage::new(state.clone());
                content_stack.add_named(&sources_page, Some("sources"));
                imp.sources_page.replace(Some(sources_page));
            }
        }

        // Show the sources page
        content_stack.set_visible_child_name("sources");

        // Update header title
        imp.content_header
            .set_title_widget(Some(&gtk4::Label::new(Some("Sources & Accounts"))));

        // Add "Add Source" button to header
        let add_button = gtk4::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("Add Source")
            .css_classes(vec!["suggested-action"])
            .build();

        if let Some(sources_page) = imp.sources_page.borrow().as_ref() {
            let sources_page_weak = sources_page.downgrade();
            add_button.connect_clicked(move |_| {
                if let Some(sources_page) = sources_page_weak.upgrade() {
                    sources_page.show_add_source_dialog();
                }
            });
        }

        imp.content_header.pack_end(&add_button);
        imp.header_add_button.replace(Some(add_button));
    }

    fn clear_header_end_widgets(&self) {
        let imp = self.imp();

        // Remove the add button if it exists
        if let Some(button) = imp.header_add_button.borrow().as_ref() {
            imp.content_header.remove(button);
        }
        imp.header_add_button.replace(None);
    }

    fn show_home_page(&self) {
        let imp = self.imp();

        // Clear header end widgets
        self.clear_header_end_widgets();

        // Remove any filter controls since homepage doesn't need them
        if let Some(filter_controls) = imp.filter_controls.borrow().as_ref() {
            imp.content_header.remove(filter_controls);
        }
        imp.filter_controls.replace(None);

        // Get or create content stack
        let content_stack = if imp.content_stack.borrow().is_none() {
            let stack = gtk4::Stack::new();
            stack.add_named(&*imp.empty_state, Some("empty"));
            imp.content_toolbar.set_content(Some(&stack));
            imp.content_stack.replace(Some(stack.clone()));
            stack
        } else {
            imp.content_stack.borrow().as_ref().unwrap().clone()
        };

        // Create home page if it doesn't exist
        if content_stack.child_by_name("home").is_none() {
            if let Some(state) = &*imp.state.borrow() {
                let home_page = pages::HomePage::new(state.clone());

                // Set up media selected callback - same as library view
                let window_weak = self.downgrade();
                let state_clone = state.clone();
                home_page.set_on_media_selected(move |media_item| {
                    info!("HomePage - Media selected: {}", media_item.title());
                    if let Some(window) = window_weak.upgrade() {
                        let media_item = media_item.clone();
                        let state = state_clone.clone();
                        glib::spawn_future_local(async move {
                            use crate::models::MediaItem;
                            match &media_item {
                                MediaItem::Movie(movie) => {
                                    info!("HomePage - Navigating to movie details");
                                    window.show_movie_details(movie.clone(), state).await;
                                }
                                MediaItem::Episode(_) => {
                                    info!("HomePage - Navigating to episode player");
                                    window.show_player(&media_item, state).await;
                                }
                                MediaItem::Show(show) => {
                                    info!("HomePage - Navigating to show details");
                                    window.show_show_details(show.clone(), state).await;
                                }
                                _ => {
                                    info!("HomePage - Unsupported media type");
                                }
                            }
                        });
                    }
                });

                content_stack.add_named(&home_page, Some("home"));
                imp.home_page.replace(Some(home_page));
            }
        } else {
            // Refresh the home page if it already exists
            if let Some(home_page) = &*imp.home_page.borrow() {
                home_page.refresh();
            }
        }

        // Show the home page
        content_stack.set_visible_child_name("home");

        // Update header title
        imp.content_header
            .set_title_widget(Some(&gtk4::Label::new(Some("Home"))));
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
            self.show_home_page();
        } else {
            // Just refresh the home page data if it exists
            if let Some(home_page) = &*imp.home_page.borrow() {
                home_page.refresh();
            }
        }
    }

    pub async fn sync_all_backends(&self, state: Arc<AppState>) {
        info!("Starting sync for all backends...");

        let backend_manager = state.backend_manager.read().await;
        let all_backends = backend_manager.get_all_backends();
        drop(backend_manager);

        // Show sync progress
        self.show_sync_progress(true);

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

        // Hide sync progress
        self.show_sync_progress(false);

        // Refresh all libraries display
        self.refresh_all_libraries().await;

        // Navigate to home if appropriate
        let imp = self.imp();
        let should_show_home = if let Some(stack) = imp.content_stack.borrow().as_ref() {
            !stack.child_by_name("library").is_some()
                && !stack.child_by_name("movie_details").is_some()
                && !stack.child_by_name("show_details").is_some()
                && !stack.child_by_name("player").is_some()
        } else {
            true
        };

        if should_show_home {
            self.show_home_page();
        } else if let Some(home_page) = &*imp.home_page.borrow() {
            home_page.refresh();
        }
    }

    pub async fn sync_single_backend(&self, backend_id: &str) {
        info!("Starting sync for backend: {}", backend_id);

        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        let backend_manager = state.backend_manager.read().await;

        if let Some(backend) = backend_manager.get_backend(backend_id) {
            drop(backend_manager);

            // Show sync progress
            self.show_sync_progress(true);

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

            // Hide sync progress
            self.show_sync_progress(false);

            // Refresh all libraries display
            self.refresh_all_libraries().await;
        } else {
            error!("Backend {} not found", backend_id);
        }
    }

    pub fn navigate_to_library(&self, library_id: &str) {
        info!("Navigating to library: {}", library_id);

        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        let library_id = library_id.to_string();
        let window_weak = self.downgrade();

        glib::spawn_future_local(async move {
            if let Some(window) = window_weak.upgrade() {
                // Check if library_id contains backend_id
                let (backend_id, actual_library_id) = if library_id.contains(':') {
                    let parts: Vec<&str> = library_id.splitn(2, ':').collect();
                    (parts[0].to_string(), parts[1].to_string())
                } else {
                    // Fallback to active backend for backward compatibility
                    if let Some(active_id) = state.get_active_backend_id().await {
                        (active_id, library_id.clone())
                    } else {
                        error!("No active backend");
                        return;
                    }
                };

                let sync_manager = state.sync_manager.clone();

                // Get the library from cache
                match sync_manager.get_cached_libraries(&backend_id).await {
                    Ok(libraries) => {
                        if let Some(library) = libraries.iter().find(|l| l.id == actual_library_id)
                        {
                            window.show_library_view(backend_id, library.clone()).await;
                        } else {
                            error!("Library not found: {}", actual_library_id);
                        }
                    }
                    Err(e) => {
                        error!("Failed to get libraries: {}", e);
                    }
                }
            }
        });
    }

    async fn move_backend_up(&self, backend_id: &str) {
        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        let mut backend_manager = state.backend_manager.write().await;
        backend_manager.move_backend_up(backend_id);
        drop(backend_manager);

        // Refresh the display
        self.refresh_all_libraries().await;
    }

    async fn move_backend_down(&self, backend_id: &str) {
        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        let mut backend_manager = state.backend_manager.write().await;
        backend_manager.move_backend_down(backend_id);
        drop(backend_manager);

        // Refresh the display
        self.refresh_all_libraries().await;
    }

    async fn refresh_all_libraries(&self) {
        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        let backend_manager = state.backend_manager.read().await;
        let all_backends = backend_manager.get_all_backends();
        drop(backend_manager);

        let sync_manager = state.sync_manager.clone();
        let mut backends_libraries = Vec::new();

        for (backend_id, _backend) in all_backends {
            // Get cached libraries for this backend
            match sync_manager.get_cached_libraries(&backend_id).await {
                Ok(libraries) => {
                    // Build library list with counts
                    let mut library_info = Vec::new();

                    for library in &libraries {
                        use crate::models::LibraryType;
                        let item_count = match library.library_type {
                            LibraryType::Movies => {
                                match sync_manager
                                    .get_cached_movies(&backend_id, &library.id)
                                    .await
                                {
                                    Ok(movies) => movies.len(),
                                    Err(_) => 0,
                                }
                            }
                            LibraryType::Shows => {
                                match sync_manager
                                    .get_cached_shows(&backend_id, &library.id)
                                    .await
                                {
                                    Ok(shows) => shows.len(),
                                    Err(_) => 0,
                                }
                            }
                            _ => 0,
                        };

                        library_info.push((library.clone(), item_count));
                    }

                    if !library_info.is_empty() {
                        backends_libraries.push((backend_id.clone(), library_info));
                    }
                }
                Err(e) => {
                    info!("No cached libraries for backend {}: {}", backend_id, e);
                }
            }
        }

        // Update the UI with all backends' libraries
        self.update_all_backends_libraries(backends_libraries);
    }

    pub async fn show_movie_details(&self, movie: crate::models::Movie, state: Arc<AppState>) {
        let imp = self.imp();

        // Get or create content stack
        let content_stack = if imp.content_stack.borrow().is_none() {
            let stack = gtk4::Stack::builder()
                .transition_type(gtk4::StackTransitionType::SlideLeftRight)
                .transition_duration(300)
                .build();

            // Set the stack as content
            imp.content_toolbar.set_content(Some(&stack));
            imp.content_stack.replace(Some(stack.clone()));

            stack
        } else {
            imp.content_stack.borrow().as_ref().unwrap().clone()
        };

        // Create or get movie details page
        let movie_details_page = {
            // Check if page exists (drop borrow immediately)
            let page_exists = imp.movie_details_page.borrow().is_some();

            if page_exists {
                // Get the page with a fresh borrow
                let page = imp.movie_details_page.borrow().as_ref().unwrap().clone();

                // Page exists, make sure it's in the stack
                if content_stack.child_by_name("movie_details").is_none() {
                    content_stack.add_named(&page, Some("movie_details"));
                }
                page
            } else {
                // Create new page
                let page = crate::ui::pages::MovieDetailsPage::new(state.clone());

                // Set callback for when play is clicked
                let window_weak = self.downgrade();
                let state_clone = state.clone();
                page.set_on_play_clicked(move |movie| {
                    if let Some(window) = window_weak.upgrade() {
                        let movie_item = crate::models::MediaItem::Movie(movie.clone());
                        let state = state_clone.clone();
                        glib::spawn_future_local(async move {
                            window.show_player(&movie_item, state).await;
                        });
                    }
                });

                // Store the page
                imp.movie_details_page.replace(Some(page.clone()));

                // Add to content stack
                content_stack.add_named(&page, Some("movie_details"));

                page
            }
        };

        // Update the content page title immediately
        imp.content_page.set_title(&movie.title);

        // Load the movie (non-blocking)
        movie_details_page.load_movie(movie.clone());

        // Remove any existing back button
        if let Some(old_button) = imp.back_button.borrow().as_ref() {
            imp.content_header.remove(old_button);
        }

        // Create a new back button for the header bar
        let back_button = gtk4::Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text("Back to Library")
            .build();

        let window_weak = self.downgrade();
        back_button.connect_clicked(move |_| {
            if let Some(window) = window_weak.upgrade() {
                // Go back to library view or home
                if let Some(stack) = window.imp().content_stack.borrow().as_ref() {
                    if stack.child_by_name("library").is_some() {
                        stack.set_visible_child_name("library");
                    } else if stack.child_by_name("home").is_some() {
                        stack.set_visible_child_name("home");
                    }
                }
            }
        });

        imp.content_header.pack_start(&back_button);
        imp.back_button.replace(Some(back_button));

        // Show the movie details page
        content_stack.set_visible_child_name("movie_details");
    }

    pub async fn show_show_details(&self, show: crate::models::Show, state: Arc<AppState>) {
        let imp = self.imp();

        // Get or create content stack
        let content_stack = if imp.content_stack.borrow().is_none() {
            let stack = gtk4::Stack::builder()
                .transition_type(gtk4::StackTransitionType::SlideLeftRight)
                .transition_duration(300)
                .build();

            // Set the stack as content
            imp.content_toolbar.set_content(Some(&stack));
            imp.content_stack.replace(Some(stack.clone()));

            stack
        } else {
            imp.content_stack.borrow().as_ref().unwrap().clone()
        };

        // Create or get show details page
        let show_details_page = {
            // Check if page exists (drop borrow immediately)
            let page_exists = imp.show_details_page.borrow().is_some();

            if page_exists {
                // Get the page with a fresh borrow
                let page = imp.show_details_page.borrow().as_ref().unwrap().clone();

                // Page exists, make sure it's in the stack
                if content_stack.child_by_name("show_details").is_none() {
                    content_stack.add_named(page.widget(), Some("show_details"));
                }
                page
            } else {
                // Create new page
                let page = crate::ui::pages::ShowDetailsPage::new(state.clone());

                // Set callback for when episode is selected
                let window_weak = self.downgrade();
                let state_clone = state.clone();
                page.set_on_episode_selected(move |episode| {
                    if let Some(window) = window_weak.upgrade() {
                        let episode_item = crate::models::MediaItem::Episode(episode.clone());
                        let state = state_clone.clone();
                        glib::spawn_future_local(async move {
                            window.show_player(&episode_item, state).await;
                        });
                    }
                });

                // Store the page
                imp.show_details_page.replace(Some(page.clone()));

                // Add to content stack
                content_stack.add_named(page.widget(), Some("show_details"));

                page
            }
        };

        // Update the content page title immediately
        imp.content_page.set_title(&show.title);

        // Load the show (non-blocking)
        show_details_page.load_show(show.clone());

        // Remove any existing back button
        if let Some(old_button) = imp.back_button.borrow().as_ref() {
            imp.content_header.remove(old_button);
        }

        // Create a new back button for the header bar
        let back_button = gtk4::Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text("Back to Library")
            .build();

        let window_weak = self.downgrade();
        back_button.connect_clicked(move |_| {
            if let Some(window) = window_weak.upgrade() {
                // Go back to library view
                if let Some(stack) = window.imp().content_stack.borrow().as_ref() {
                    stack.set_visible_child_name("library");
                }
            }
        });

        // Add the back button to the header bar (pack_start)
        imp.content_header.pack_start(&back_button);
        imp.back_button.replace(Some(back_button));

        // Show the show details page
        content_stack.set_visible_child_name("show_details");
    }

    pub async fn show_player(&self, media_item: &crate::models::MediaItem, state: Arc<AppState>) {
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

        // Get or create content stack
        debug!("MainWindow::show_player() - Getting content stack");
        let content_stack = if imp.content_stack.borrow().is_none() {
            info!("MainWindow::show_player() - Creating new content stack");
            let stack = gtk4::Stack::builder()
                .transition_type(gtk4::StackTransitionType::SlideLeftRight)
                .transition_duration(300)
                .build();

            // Set the stack as content
            imp.content_toolbar.set_content(Some(&stack));
            imp.content_stack.replace(Some(stack.clone()));

            stack
        } else {
            debug!("MainWindow::show_player() - Using existing content stack");
            imp.content_stack.borrow().as_ref().unwrap().clone()
        };

        // Always create a new player page to ensure we use the latest backend setting
        debug!("MainWindow::show_player() - Creating fresh player page");

        // Clear any existing player page first
        if let Some(existing_page) = imp.player_page.borrow().as_ref() {
            debug!("MainWindow::show_player() - Removing existing player page from stack");
            content_stack.remove(existing_page.widget());
        }
        imp.player_page.replace(None);

        info!("MainWindow::show_player() - Creating new player page with current backend");
        let page = crate::ui::pages::PlayerPage::new(state.clone());
        imp.player_page.replace(Some(page.clone()));

        // Add to content stack
        debug!("MainWindow::show_player() - Adding player page to content stack");
        content_stack.add_named(page.widget(), Some("player"));
        info!("MainWindow::show_player() - Player page added to stack with name 'player'");

        let player_page = page;

        // Update the content page title first
        imp.content_page.set_title(media_item.title());
        debug!("MainWindow::show_player() - Updated content page title");

        // Load the media (but don't block navigation on failure)
        debug!("MainWindow::show_player() - Loading media into player");
        if let Err(e) = player_page.load_media(media_item, state).await {
            error!("MainWindow::show_player() - Failed to load media: {}", e);
            // Show error dialog but still navigate to player page
            let dialog =
                adw::AlertDialog::new(Some("Failed to Load Media"), Some(&format!("Error: {}", e)));
            dialog.add_response("ok", "OK");
            dialog.set_default_response(Some("ok"));
            dialog.present(Some(self));
        }

        // Clear any existing back buttons from the main header
        if let Some(old_button) = imp.back_button.borrow().as_ref() {
            imp.content_header.remove(old_button);
        }
        imp.back_button.replace(None);

        // Also clear any title widget that might have controls
        imp.content_header.set_title_widget(None::<&gtk4::Widget>);

        // Hide the main header bar completely to avoid duplicate back buttons
        imp.content_header.set_visible(false);
        imp.content_toolbar
            .set_top_bar_style(adw::ToolbarStyle::Flat); // Make toolbar flat/hidden

        // Create minimal OSD overlay buttons (back and close)
        // Add them directly to the player's overlay to avoid duplication
        let player_widget = player_page.widget();
        if let Some(first_child) = player_widget.first_child()
            && let Some(overlay) = first_child.downcast_ref::<gtk4::Overlay>()
        {
            // Create a minimal back button
            let back_button = gtk4::Button::builder()
                .icon_name("go-previous-symbolic")
                .tooltip_text("Back")
                .margin_top(12)
                .margin_start(12)
                .build();
            back_button.add_css_class("osd");
            back_button.add_css_class("circular");

            // Create a close button
            let close_button = gtk4::Button::builder()
                .icon_name("window-close-symbolic")
                .tooltip_text("Close")
                .margin_top(12)
                .margin_end(12)
                .build();
            close_button.add_css_class("osd");
            close_button.add_css_class("circular");

            // Connect button handlers BEFORE adding to containers
            // Connect close button handler
            let window_weak_close = self.downgrade();
            close_button.connect_clicked(move |_| {
                if let Some(window) = window_weak_close.upgrade() {
                    window.close();
                }
            });

            // Connect back button handler
            let window_weak = self.downgrade();
            back_button.connect_clicked(move |_| {
                if let Some(window) = window_weak.upgrade() {
                    // Stop the player
                    if let Some(player_page) = window.imp().player_page.borrow().as_ref() {
                        let player_page = player_page.clone();
                        glib::spawn_future_local(async move {
                            player_page.stop().await;
                        });
                    }

                    // Show header bar again and restore toolbar style
                    window.imp().content_header.set_visible(true);
                    window
                        .imp()
                        .content_toolbar
                        .set_top_bar_style(adw::ToolbarStyle::Raised);

                    // Restore window size
                    let (width, height) = *window.imp().saved_window_size.borrow();
                    window.set_default_size(width, height);

                    // Restore sidebar
                    if let Some(content) = window.content()
                        && let Some(split_view) = content.downcast_ref::<adw::NavigationSplitView>()
                    {
                        split_view.set_collapsed(false);
                        split_view.set_show_content(true);
                    }

                    // Navigate back to the previous page from navigation stack
                    let previous_page = window.imp().navigation_stack.borrow_mut().pop();
                    if let Some(page_name) = previous_page {
                        if let Some(stack) = window.imp().content_stack.borrow().as_ref() {
                            if stack.child_by_name(&page_name).is_some() {
                                stack.set_visible_child_name(&page_name);
                            }
                        }
                    } else {
                        // Fallback if no navigation history
                        if let Some(stack) = window.imp().content_stack.borrow().as_ref() {
                            if stack.child_by_name("home").is_some() {
                                stack.set_visible_child_name("home");
                            } else if stack.child_by_name("library").is_some() {
                                stack.set_visible_child_name("library");
                            }
                        }
                    }
                }
            });

            // Add back button as separate overlay (top-left)
            let back_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
            back_box.set_halign(gtk4::Align::Start);
            back_box.set_valign(gtk4::Align::Start);
            back_box.append(&back_button);
            overlay.add_overlay(&back_box);

            // Add close button as separate overlay (top-right)
            let close_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
            close_box.set_halign(gtk4::Align::End);
            close_box.set_valign(gtk4::Align::Start);
            close_box.append(&close_button);
            overlay.add_overlay(&close_box);

            // Initially hide buttons, show on hover like player controls
            back_box.set_visible(false);
            back_box.set_opacity(0.0);
            close_box.set_visible(false);
            close_box.set_opacity(0.0);

            // Set up hover detection for both buttons
            let back_box_weak = back_box.downgrade();
            let close_box_weak = close_box.downgrade();
            let hide_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
            let hover_controller = gtk4::EventControllerMotion::new();

            let hide_timer_clone = hide_timer.clone();
            hover_controller.connect_motion(move |_, _, _| {
                // Show both buttons on hover
                if let Some(back_box) = back_box_weak.upgrade() {
                    back_box.set_visible(true);
                    back_box.set_opacity(1.0);
                }
                if let Some(close_box) = close_box_weak.upgrade() {
                    close_box.set_visible(true);
                    close_box.set_opacity(1.0);
                }

                // Cancel previous timer
                if let Some(timer_id) = hide_timer_clone.borrow_mut().take() {
                    timer_id.remove();
                }

                // Hide again after same delay as player controls
                let back_box_inner = back_box_weak.clone();
                let close_box_inner = close_box_weak.clone();
                let hide_timer_inner = hide_timer_clone.clone();
                let timer_id = glib::timeout_add_local(
                    std::time::Duration::from_secs(PLAYER_CONTROLS_HIDE_DELAY_SECS),
                    move || {
                        if let Some(back_box) = back_box_inner.upgrade() {
                            back_box.set_opacity(0.0);
                            back_box.set_visible(false);
                        }
                        if let Some(close_box) = close_box_inner.upgrade() {
                            close_box.set_opacity(0.0);
                            close_box.set_visible(false);
                        }
                        hide_timer_inner.borrow_mut().take();
                        glib::ControlFlow::Break
                    },
                );
                hide_timer_clone.borrow_mut().replace(timer_id);
            });

            overlay.add_controller(hover_controller);

            // Button handlers already connected above
        }

        // Save current window size before changing it
        let (current_width, current_height) = self.default_size();
        imp.saved_window_size
            .replace((current_width, current_height));

        // Push current page to navigation stack before switching to player
        if let Some(current_page) = content_stack.visible_child_name() {
            imp.navigation_stack
                .borrow_mut()
                .push(current_page.to_string());
            info!(
                "MainWindow::show_player() - Pushed '{}' to navigation stack",
                current_page
            );
        }

        // Show the player page
        info!("MainWindow::show_player() - Switching stack to 'player' page");
        content_stack.set_visible_child_name("player");
        info!("MainWindow::show_player() - Navigation to player complete");

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
        let window_weak = self.downgrade();
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

        // Get or create content stack
        let content_stack = if imp.content_stack.borrow().is_none() {
            let stack = gtk4::Stack::builder()
                .transition_type(gtk4::StackTransitionType::SlideLeftRight)
                .transition_duration(300)
                .build();

            // Add the empty state as the default page
            stack.add_named(&*imp.empty_state, Some("empty"));

            // Set the stack as content of the toolbar view
            imp.content_toolbar.set_content(Some(&stack));

            imp.content_stack.replace(Some(stack.clone()));
            stack
        } else {
            imp.content_stack.borrow().as_ref().unwrap().clone()
        };

        // Create or get library view
        let library_view = {
            let existing_view = imp.library_view.borrow();
            existing_view.as_ref().cloned()
        }
        .unwrap_or_else(|| {
            let state = imp.state.borrow().as_ref().unwrap().clone();
            let view = crate::ui::pages::LibraryView::new(state.clone());

            // Set the media selected callback to handle different media types
            let window_weak = self.downgrade();
            let state_clone = state.clone();
            view.set_on_media_selected(move |media_item| {
                info!(
                    "MainWindow - Media selected callback triggered: {}",
                    media_item.title()
                );
                if let Some(window) = window_weak.upgrade() {
                    let media_item = media_item.clone();
                    let state = state_clone.clone();
                    debug!(
                        "MainWindow - Spawning navigation task for: {}",
                        media_item.title()
                    );
                    glib::spawn_future_local(async move {
                        use crate::models::MediaItem;
                        info!(
                            "MainWindow - Processing media selection: {}",
                            media_item.title()
                        );
                        match &media_item {
                            MediaItem::Movie(movie) => {
                                // Movies go to movie details page
                                info!("MainWindow - Movie selected, navigating to movie details");
                                window.show_movie_details(movie.clone(), state).await;
                            }
                            MediaItem::Show(show) => {
                                // Shows go to episode selection
                                info!("MainWindow - Show selected, navigating to show details");
                                window.show_show_details(show.clone(), state).await;
                            }
                            _ => {
                                // Other media types could be handled here
                                info!("MainWindow - Unsupported media type selected");
                            }
                        }
                    });
                } else {
                    error!("MainWindow - Failed to upgrade window weak reference!");
                }
            });

            imp.library_view.replace(Some(view.clone()));

            // Add to content stack
            content_stack.add_named(&view, Some("library"));

            view
        });

        // Update the content page title
        imp.content_page.set_title(&library.title);

        // Remove any existing back button and filter controls
        if let Some(old_button) = imp.back_button.borrow().as_ref() {
            imp.content_header.remove(old_button);
        }
        if let Some(old_controls) = imp.filter_controls.borrow().as_ref() {
            imp.content_header.remove(old_controls);
        }

        // Create a new back button for the header bar
        let back_button = gtk4::Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text("Back to Libraries")
            .build();

        let window_weak = self.downgrade();
        back_button.connect_clicked(move |_| {
            if let Some(window) = window_weak.upgrade() {
                window.show_libraries_view();
            }
        });

        // Add the back button to the header bar
        imp.content_header.pack_start(&back_button);
        imp.back_button.replace(Some(back_button));

        // Update header bar title
        imp.content_header.set_title_widget(Some(
            &gtk4::Label::builder()
                .label(&library.title)
                .single_line_mode(true)
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .build(),
        ));

        // Create filter controls for the header bar
        let filter_controls = self.create_filter_controls(&library_view);
        imp.content_header.pack_end(&filter_controls);
        imp.filter_controls.replace(Some(filter_controls));

        // Load the library
        library_view.load_library(backend_id, library).await;

        // Switch to library view in the content area
        content_stack.set_visible_child_name("library");

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

        // Remove back button and filter controls if they exist
        if let Some(back_button) = imp.back_button.borrow().as_ref() {
            imp.content_header.remove(back_button);
        }
        imp.back_button.replace(None);

        if let Some(filter_controls) = imp.filter_controls.borrow().as_ref() {
            imp.content_header.remove(filter_controls);
        }
        imp.filter_controls.replace(None);

        // Reset header bar title
        imp.content_header.set_title_widget(gtk4::Widget::NONE);

        // Show sidebar in mobile view
        if let Some(content) = self.content()
            && let Some(split_view) = content.downcast_ref::<adw::NavigationSplitView>()
        {
            split_view.set_show_content(false);
        }
    }

    fn create_filter_controls(&self, library_view: &crate::ui::pages::LibraryView) -> gtk4::Box {
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

    fn toggle_edit_mode(&self, _button: &gtk4::Button) {
        // Edit mode is now handled per-source group
        // This method might be removed or repurposed
    }

    async fn load_library_visibility(&self) {
        // Load from existing config
        if let Some(config_arc) = self.imp().config.borrow().as_ref() {
            let visibility = {
                let config = config_arc.read().await;
                config.get_all_library_visibility()
            };
            *self.imp().library_visibility.borrow_mut() = visibility;
        }
    }

    async fn save_library_visibility(&self) {
        // Save to config using proper methods
        let visibility = self.imp().library_visibility.borrow().clone();

        if let Some(config_arc) = self.imp().config.borrow().as_ref() {
            let mut config = config_arc.write().await;
            if let Err(e) = config.set_all_library_visibility(visibility) {
                error!("Failed to save library visibility: {}", e);
            }
        }
    }

    pub async fn update_backend_selector(&self) {
        // Backend selector is now deprecated - we show all backends at once
        // Sources button is always visible at the bottom
    }

    async fn switch_backend(&self, backend_id: &str, state: Arc<AppState>) {
        info!("Switching to backend: {}", backend_id);

        // Update the active backend in the backend manager
        {
            let mut backend_manager = state.backend_manager.write().await;
            if let Err(e) = backend_manager.set_active(backend_id) {
                error!("Failed to set active backend: {}", e);
                return;
            }
        }

        // Save the new active backend to config
        {
            let mut config = state.config.write().await;
            let _ = config.set_last_active_backend(backend_id);
        }

        // Clear current sources display
        while let Some(child) = self.imp().sources_container.first_child() {
            self.imp().sources_container.remove(&child);
        }

        // Show loading state
        self.show_sync_progress(true);

        // Load cached libraries for the new backend
        self.load_cached_libraries_for_backend(backend_id).await;

        // Get the backend and sync
        let backend_manager = state.backend_manager.read().await;
        if let Some(backend) = backend_manager.get_backend(backend_id) {
            if backend.is_initialized().await {
                // Update user display for the new backend
                if let Some(user) = state.get_user().await {
                    self.update_user_display_with_backend(Some(user), &backend_manager)
                        .await;
                }

                // Start background sync
                let backend_clone = backend.clone();
                let state_clone = state.clone();
                let window_weak = self.downgrade();
                let backend_id = backend_id.to_string();
                glib::spawn_future_local(async move {
                    if let Some(window) = window_weak.upgrade() {
                        window
                            .sync_and_update_libraries(&backend_id, backend_clone, state_clone)
                            .await;
                        window.show_sync_progress(false);
                    }
                });
            } else {
                // Backend not initialized, show auth needed
                self.imp().status_container.set_visible(true);
                self.imp().status_label.set_text("Authentication required");
                self.imp()
                    .status_icon
                    .set_icon_name(Some("dialog-password-symbolic"));
                self.show_sync_progress(false);
            }
        } else {
            self.show_sync_progress(false);
        }
    }
}
