use gtk4::{gio, glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::sync::Arc;
use tracing::{info, error, warn};

use crate::config::Config;
use crate::state::AppState;

mod imp {
    use super::*;
    use gtk4::CompositeTemplate;
    
    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/arsfeld/Reel/window.ui")]
    pub struct ReelMainWindow {
        #[template_child]
        pub welcome_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub connect_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub libraries_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub libraries_list: TemplateChild<gtk4::ListBox>,
        #[template_child]
        pub refresh_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub status_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub status_icon: TemplateChild<gtk4::Image>,
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
        pub config: RefCell<Option<Arc<Config>>>,
        pub content_stack: RefCell<Option<gtk4::Stack>>,
        pub library_view: RefCell<Option<crate::ui::pages::LibraryView>>,
        pub back_button: RefCell<Option<gtk4::Button>>,
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
            self.connect_button.connect_clicked(
                glib::clone!(@weak obj => move |_| {
                    obj.show_auth_dialog();
                })
            );
            
            self.refresh_button.connect_clicked(
                glib::clone!(@weak obj => move |_| {
                    let state_clone = obj.imp().state.borrow().as_ref().map(|s| s.clone());
                    if let Some(state) = state_clone {
                        glib::spawn_future_local(async move {
                            obj.trigger_sync(state).await;
                        });
                    }
                })
            );
            
            // Connect to library list row activation
            self.libraries_list.connect_row_activated(
                glib::clone!(@weak obj => move |_, row| {
                    if let Some(action_row) = row.downcast_ref::<adw::ActionRow>() {
                        info!("Library selected: {}", action_row.title());
                        
                        // Get library ID from the row's widget name
                        let library_id = action_row.widget_name().to_string();
                        
                        // Navigate to library view
                        obj.navigate_to_library(&library_id);
                    }
                })
            );
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
        @implements gio::ActionGroup, gio::ActionMap;
}

impl ReelMainWindow {
    pub fn new(app: &adw::Application, state: Arc<AppState>, config: Arc<Config>) -> Self {
        let window: Self = glib::Object::builder()
            .property("application", app)
            .build();
        
        // Store state and config
        window.imp().state.replace(Some(state.clone()));
        window.imp().config.replace(Some(config));
        
        // Setup actions
        window.setup_actions(app);
        
        // Apply theme
        window.apply_theme();
        
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
        preferences_action.connect_activate(glib::clone!(@weak self as window => move |_, _| {
            info!("Opening preferences");
            window.show_preferences();
        }));
        app.add_action(&preferences_action);
        
        // About action
        let about_action = gio::SimpleAction::new("about", None);
        about_action.connect_activate(glib::clone!(@weak self as window => move |_, _| {
            window.show_about();
        }));
        app.add_action(&about_action);
        
        // Keyboard shortcuts
        app.set_accels_for_action("app.preferences", &["<primary>comma"]);
        app.set_accels_for_action("window.close", &["<primary>w"]);
    }
    
    fn apply_theme(&self) {
        if let Some(config) = self.imp().config.borrow().as_ref() {
            let style_manager = adw::StyleManager::default();
            
            match config.general.theme.as_str() {
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
                
                // Get the last active backend ID from config
                let config = state.config.clone();
                let backend_id = config.get_last_active_backend();
                
                // If no last active backend, skip cache loading
                if backend_id.is_none() {
                    info!("No last active backend found, skipping cache load");
                    return;
                }
                
                let backend_id = backend_id.unwrap();
                info!("Loading cached data for last active backend: {}", backend_id);
                
                // Use the sync manager from AppState instead of creating a new one
                let sync_manager = state.sync_manager.clone();
                let cache = state.cache_manager.clone();
                
                // Try to get cached libraries
                match sync_manager.get_cached_libraries(&backend_id).await {
                    Ok(libraries) if !libraries.is_empty() => {
                        info!("Found {} cached libraries on startup, showing immediately", libraries.len());
                        
                        // Hide welcome page and show libraries immediately
                        window.imp().welcome_page.set_visible(false);
                        window.imp().libraries_group.set_visible(true);
                        
                        // Load and display the cached libraries
                        window.load_cached_libraries_for_backend(&backend_id).await;
                        
                        // Update status to show we're using cached data
                        // Try to load cached server name if available
                        let cache_key = format!("{}:server_name", backend_id);
                        let title = if let Ok(Some(server_name)) = cache.get_media::<String>(&cache_key).await {
                            server_name
                        } else {
                            "Cached Libraries".to_string()
                        };
                        
                        window.imp().status_row.set_title(&title);
                        window.imp().status_row.set_subtitle("Loading from cache...");
                        window.imp().status_icon.set_icon_name(Some("view-refresh-symbolic"));
                    }
                    Ok(_) => {
                        info!("No cached libraries found on startup");
                    }
                    Err(e) => {
                        info!("Could not load cached libraries on startup: {}", e);
                    }
                }
            }
        });
    }
    
    fn check_and_load_backends(&self, state: Arc<AppState>) {
        let window_weak = self.downgrade();
        
        glib::spawn_future_local(async move {
            // Initialize all configured backends
            let plex_backend = {
                let mut backend_manager = state.backend_manager.write().await;
                
                // Import the trait to access the initialize method
                use crate::backends::MediaBackend;
                
                // First check if we have a last active backend that we should reuse
                let config = state.config.clone();
                let last_active = config.get_last_active_backend();
                
                // Check if the last active backend was a plex backend
                let backend_id = if let Some(ref last_id) = last_active {
                    if last_id.starts_with("plex") {
                        // Reuse the last active backend ID
                        info!("Reusing existing backend ID: {}", last_id);
                        last_id.clone()
                    } else {
                        // Last active was not plex, find an appropriate ID
                        let existing_backends = state.config.get_configured_backends();
                        let mut new_id = "plex".to_string();
                        let mut counter = 1;
                        
                        while existing_backends.contains(&new_id) {
                            new_id = format!("plex_{}", counter);
                            counter += 1;
                        }
                        new_id
                    }
                } else {
                    // No last active backend, use "plex" as the ID
                    "plex".to_string()
                };
                
                // Try to initialize Plex backend
                let plex_backend = Arc::new(crate::backends::plex::PlexBackend::new());
                
                match plex_backend.initialize().await {
                    Ok(Some(user)) => {
                        info!("Successfully initialized backend {} for user: {}", backend_id, user.username);
                        
                        // Register the backend with its ID
                        backend_manager.register_backend(backend_id.clone(), plex_backend.clone());
                        backend_manager.set_active(&backend_id).ok();
                        
                        // Set the user
                        state.set_user(user.clone()).await;
                        
                        // Save this backend as the last active (only if it changed)
                        if last_active.as_ref() != Some(&backend_id) {
                            let mut config = state.config.as_ref().clone();
                            let _ = config.set_last_active_backend(&backend_id);
                        }
                        
                        // Cache the server name for next startup
                        let cache = state.cache_manager.clone();
                        // Get server info and cache it
                        let backend_info = plex_backend.get_backend_info().await;
                        if let Some(server_name) = backend_info.server_name {
                            let cache_key = format!("{}:server_name", backend_id);
                            let _ = cache.set_media(&cache_key, "server_name", &server_name).await;
                        }
                        let user_cache_key = format!("{}:last_user", backend_id);
                        let _ = cache.set_media(&user_cache_key, "user", &user.username).await;
                        
                        Some((backend_id, plex_backend))
                    }
                    Ok(None) => {
                        info!("No credentials found for backend");
                        None
                    }
                    Err(e) => {
                        error!("Failed to initialize backend: {}", e);
                        None
                    }
                }
            }; // Write lock is dropped here
            
            // If we have a backend, load cached data and update UI immediately
            if let Some((backend_id, backend)) = plex_backend {
                // Now update UI with a read lock
                if let Some(window) = window_weak.upgrade() {
                    info!("Updating UI after successful Plex initialization");
                    window.update_connection_status(true).await;
                    
                    // Re-read backend manager to get the registered backend
                    let backend_manager = state.backend_manager.read().await;
                    info!("Backend manager has active backend: {}", backend_manager.get_active().is_some());
                    
                    if let Some(current_user) = state.get_user().await {
                        info!("Updating user display for: {}", current_user.username);
                        // Pass the backend manager to update_user_display_with_backend
                        window.update_user_display_with_backend(Some(current_user.clone()), &backend_manager).await;
                    } else {
                        info!("No current user found in state");
                    }
                    
                    // Refresh the cached data with the authenticated backend
                    // (this will update with any new data, but cache was already shown)
                    info!("Refreshing libraries with authenticated backend...");
                    window.load_cached_libraries_for_backend(&backend_id).await;
                    
                    // THEN: Start background sync (without blocking)
                    let backend_clone = backend.clone();
                    let state_clone = state.clone();
                    let window_weak2 = window.downgrade();
                    glib::spawn_future_local(async move {
                        info!("Starting background sync...");
                        if let Some(window) = window_weak2.upgrade() {
                            // Show sync is starting
                            window.show_sync_progress(true);
                            
                            // Start sync with backend ID
                            window.sync_and_update_libraries(&backend_id, backend_clone, state_clone).await;
                            
                            // Hide sync indicator
                            window.show_sync_progress(false);
                        }
                    });
                } else {
                    error!("Failed to upgrade window weak reference");
                }
            }
            
            // TODO: Initialize other backends (Jellyfin, Local)
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
            imp.welcome_page.set_visible(false);  // Hide welcome message
            imp.libraries_group.set_visible(true);
            
            // TODO: Load libraries from backend
        } else {
            imp.status_row.set_title("Not Connected");
            imp.status_row.set_subtitle("No server configured");
            imp.status_icon.set_icon_name(Some("network-offline-symbolic"));
            imp.welcome_page.set_visible(true);   // Show welcome message
            imp.libraries_group.set_visible(false);
        }
    }
    
    pub async fn update_user_display_with_backend(&self, user: Option<crate::models::User>, backend_manager: &crate::backends::BackendManager) {
        let imp = self.imp();
        info!("update_user_display_with_backend called with user: {:?}", user.as_ref().map(|u| &u.username));
        
        if let Some(user) = user {
            if let Some(backend) = backend_manager.get_active() {
                info!("Active backend found, is_initialized: {}", backend.is_initialized().await);
                if backend.is_initialized().await {
                    // Get backend info using the trait method
                    let backend_info = backend.get_backend_info().await;
                    info!("Got backend info: {:?}", backend_info);
                    
                    // Update title with server name or backend display name
                    let title = backend_info.server_name.unwrap_or(backend_info.display_name);
                    imp.status_row.set_title(&title);
                    
                    // Create detailed subtitle based on connection type
                    use crate::backends::traits::ConnectionType;
                    let connection_type_str = match backend_info.connection_type {
                        ConnectionType::Local => "Local",
                        ConnectionType::Remote => "Remote",
                        ConnectionType::Relay => "Relay",
                        ConnectionType::Unknown => "Connected",
                    };
                    imp.status_row.set_subtitle(&format!("{} - {} connection", user.username, connection_type_str));
                    
                    // Update icon based on connection type
                    let icon_name = match backend_info.connection_type {
                        ConnectionType::Local => "network-wired-symbolic",
                        ConnectionType::Relay => "network-cellular-symbolic",
                        ConnectionType::Remote => "network-wireless-symbolic",
                        ConnectionType::Unknown => "network-transmit-receive-symbolic",
                    };
                    imp.status_icon.set_icon_name(Some(icon_name));
                } else {
                    imp.status_row.set_title("Authenticated");
                    imp.status_row.set_subtitle(&format!("{} - Not connected to server", user.username));
                    imp.status_icon.set_icon_name(Some("network-idle-symbolic"));
                }
            } else {
                imp.status_row.set_title("Not Connected");
                imp.status_row.set_subtitle(&format!("Logged in as {}", user.username));
                imp.status_icon.set_icon_name(Some("network-offline-symbolic"));
            }
        } else {
            imp.status_row.set_title("Not Connected");
            imp.status_row.set_subtitle("No server configured");
            imp.status_icon.set_icon_name(Some("network-offline-symbolic"));
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
                    
                    // Update title with server name or backend display name
                    let title = backend_info.server_name.unwrap_or(backend_info.display_name);
                    imp.status_row.set_title(&title);
                    
                    // Create detailed subtitle based on connection type
                    use crate::backends::traits::ConnectionType;
                    let connection_type_str = match backend_info.connection_type {
                        ConnectionType::Local => "Local",
                        ConnectionType::Remote => "Remote",
                        ConnectionType::Relay => "Relay",
                        ConnectionType::Unknown => "Connected",
                    };
                    imp.status_row.set_subtitle(&format!("{} - {} connection", user.username, connection_type_str));
                    
                    // Update icon based on connection type
                    let icon_name = match backend_info.connection_type {
                        ConnectionType::Local => "network-wired-symbolic",
                        ConnectionType::Relay => "network-cellular-symbolic",
                        ConnectionType::Remote => "network-wireless-symbolic",
                        ConnectionType::Unknown => "network-transmit-receive-symbolic",
                    };
                    imp.status_icon.set_icon_name(Some(icon_name));
                } else {
                    imp.status_row.set_title("Authenticated");
                    imp.status_row.set_subtitle(&format!("{} - Not connected to server", user.username));
                    imp.status_icon.set_icon_name(Some("network-idle-symbolic"));
                }
            } else {
                imp.status_row.set_title("Not Connected");
                imp.status_row.set_subtitle(&format!("Logged in as {}", user.username));
                imp.status_icon.set_icon_name(Some("network-offline-symbolic"));
            }
        } else {
            imp.status_row.set_title("Not Connected");
            imp.status_row.set_subtitle("No server configured");
            imp.status_icon.set_icon_name(Some("network-offline-symbolic"));
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
                    if let Some(backend) = backend_manager.get_active() {
                        if backend.is_initialized().await {
                            info!("Backend initialized after auth dialog closed");
                            
                            // Update connection status
                            window.update_connection_status(true).await;
                            
                            // Update user display
                            if let Some(user) = state_for_async.get_user().await {
                                window.update_user_display_with_backend(Some(user), &backend_manager).await;
                            }
                            
                            // FIRST: Load cached data immediately
                            if let Some(backend_id) = state_for_async.get_active_backend_id().await {
                                info!("Loading cached libraries for instant display...");
                                window.load_cached_libraries_for_backend(&backend_id).await;
                                
                                // Save this backend as the last active
                                let mut config = state_for_async.config.as_ref().clone();
                                let _ = config.set_last_active_backend(&backend_id);
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
                                    if let Some(backend_id) = state_clone.get_active_backend_id().await {
                                        window.sync_and_update_libraries(&backend_id, backend_clone, state_clone).await;
                                    }
                                    
                                    // Hide sync progress
                                    window.show_sync_progress(false);
                                }
                            });
                        }
                    }
                });
            }
        });
    }
    
    fn show_preferences(&self) {
        info!("Showing preferences");
        
        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        let prefs_window = crate::ui::PreferencesWindow::new(self, state);
        prefs_window.present();
    }
    
    fn show_about(&self) {
        let about = adw::AboutWindow::builder()
            .application_name("Reel")
            .application_icon("com.github.arsfeld.Reel")
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
        let imp = self.imp();
        
        // Clear existing rows
        while let Some(child) = imp.libraries_list.first_child() {
            imp.libraries_list.remove(&child);
        }
        
        // Add a row for each library
        for (library, item_count) in libraries {
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
            
            // Add navigation arrow
            let suffix_icon = gtk4::Image::from_icon_name("go-next-symbolic");
            row.add_suffix(&suffix_icon);
            
            // Store library ID in the row's name for later retrieval
            row.set_widget_name(&library.id);
            
            imp.libraries_list.append(&row);
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
                            match sync_manager.get_cached_movies(backend_id, &library.id).await {
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
                
                // Hide empty state if we have libraries
                self.imp().empty_state.set_visible(false);
            }
            Err(e) => {
                info!("No cached libraries available: {}", e);
            }
        }
    }
    
    pub async fn trigger_sync(&self, state: Arc<AppState>) {
        info!("Manually triggering sync...");
        
        // Get the active backend
        let backend_manager = state.backend_manager.read().await;
        if let Some(backend) = backend_manager.get_active() {
            if backend.is_initialized().await {
                // Show sync progress
                self.show_sync_progress(true);
                
                // Start sync with backend ID
                if let Some(backend_id) = state.get_active_backend_id().await {
                    self.sync_and_update_libraries(&backend_id, backend, state.clone()).await;
                }
                
                // Hide sync progress
                self.show_sync_progress(false);
            } else {
                warn!("Backend not initialized, cannot sync");
            }
        } else {
            warn!("No active backend, cannot sync");
        }
    }
    
    pub async fn sync_and_update_libraries(&self, backend_id: &str, backend: Arc<dyn crate::backends::MediaBackend>, state: Arc<AppState>) {
        info!("Starting library sync...");
        
        // Get sync manager from state
        let sync_manager = state.sync_manager.clone();
        
        // Perform sync
        match sync_manager.sync_backend(backend_id, backend).await {
            Ok(result) => {
                info!("Sync completed: {} items synced", result.items_synced);
                
                // Get cached libraries and update UI
                match sync_manager.get_cached_libraries(backend_id).await {
                    Ok(libraries) => {
                        info!("Found {} libraries in cache", libraries.len());
                        
                        // Build library list with counts
                        let mut library_info = Vec::new();
                        
                        for library in &libraries {
                            use crate::models::LibraryType;
                            let item_count = match library.library_type {
                                LibraryType::Movies => {
                                    // Get movie count for this library
                                    match sync_manager.get_cached_movies(backend_id, &library.id).await {
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
                        
                        info!("Updating UI with {} libraries", library_info.len());
                        self.update_libraries(library_info);
                        
                        // Hide empty state if we have libraries
                        if !libraries.is_empty() {
                            self.imp().empty_state.set_visible(false);
                        }
                    }
                    Err(e) => {
                        error!("Failed to get cached libraries: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("Sync failed: {}", e);
            }
        }
    }
    
    pub fn navigate_to_library(&self, library_id: &str) {
        info!("Navigating to library: {}", library_id);
        
        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        let library_id = library_id.to_string();
        let window_weak = self.downgrade();
        
        glib::spawn_future_local(async move {
            if let Some(window) = window_weak.upgrade() {
                // Get backend ID and sync manager
                if let Some(backend_id) = state.get_active_backend_id().await {
                    let sync_manager = state.sync_manager.clone();
                    
                    // Get the library from cache
                    match sync_manager.get_cached_libraries(&backend_id).await {
                        Ok(libraries) => {
                            if let Some(library) = libraries.iter().find(|l| l.id == library_id) {
                                window.show_library_view(backend_id, library.clone()).await;
                            } else {
                                error!("Library not found: {}", library_id);
                            }
                        }
                        Err(e) => {
                            error!("Failed to get libraries: {}", e);
                        }
                    }
                } else {
                    error!("No active backend");
                }
            }
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
        }.unwrap_or_else(|| {
            let state = imp.state.borrow().as_ref().unwrap().clone();
            let view = crate::ui::pages::LibraryView::new(state);
            imp.library_view.replace(Some(view.clone()));
            
            // Add to content stack
            content_stack.add_named(&view, Some("library"));
            
            view
        });
        
        // Update the content page title
        imp.content_page.set_title(&library.title);
        
        // Remove any existing back button
        if let Some(old_button) = imp.back_button.borrow().as_ref() {
            imp.content_header.remove(old_button);
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
        imp.content_header.set_title_widget(Some(&gtk4::Label::builder()
            .label(&library.title)
            .single_line_mode(true)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .build()));
        
        // Load the library
        library_view.load_library(backend_id, library).await;
        
        // Switch to library view in the content area
        content_stack.set_visible_child_name("library");
        
        // Get the split view from the window content and show content pane on mobile
        if let Some(content) = self.content() {
            if let Some(split_view) = content.downcast_ref::<adw::NavigationSplitView>() {
                split_view.set_show_content(true);
            }
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
        
        // Remove back button if it exists
        if let Some(back_button) = imp.back_button.borrow().as_ref() {
            imp.content_header.remove(back_button);
        }
        imp.back_button.replace(None);
        
        // Reset header bar title
        imp.content_header.set_title_widget(gtk4::Widget::NONE);
        
        // Show sidebar in mobile view
        if let Some(content) = self.content() {
            if let Some(split_view) = content.downcast_ref::<adw::NavigationSplitView>() {
                split_view.set_show_content(false);
            }
        }
    }
}