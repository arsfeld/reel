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
        
        pub state: RefCell<Option<Arc<AppState>>>,
        pub config: RefCell<Option<Arc<Config>>>,
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
                        // TODO: Navigate to library view
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
        
        // Check for existing Plex token and load it
        window.check_and_load_plex_token(state);
        
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
    
    fn check_and_load_plex_token(&self, state: Arc<AppState>) {
        let window_weak = self.downgrade();
        
        glib::spawn_future_local(async move {
            // Initialize all configured backends
            let plex_backend = {
                let mut backend_manager = state.backend_manager.write().await;
                
                // Try to initialize Plex backend
                let plex_backend = Arc::new(crate::backends::plex::PlexBackend::new());
                
                // Import the trait to access the initialize method
                use crate::backends::MediaBackend;
                
                match plex_backend.initialize().await {
                    Ok(Some(user)) => {
                        info!("Successfully initialized Plex backend for user: {}", user.username);
                        
                        // Register the backend
                        backend_manager.register_backend("plex".to_string(), plex_backend.clone());
                        backend_manager.set_active("plex").ok();
                        
                        // Set the user
                        state.set_user(user).await;
                        
                        Some(plex_backend)
                    }
                    Ok(None) => {
                        info!("No Plex credentials found");
                        None
                    }
                    Err(e) => {
                        error!("Failed to initialize Plex backend: {}", e);
                        None
                    }
                }
            }; // Write lock is dropped here
            
            // If we have a backend, load cached data and update UI immediately
            if let Some(backend) = plex_backend {
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
                    
                    // FIRST: Load cached data immediately
                    info!("Loading cached libraries for instant display...");
                    window.load_cached_libraries().await;
                    
                    // THEN: Start background sync (without blocking)
                    let backend_clone = backend.clone();
                    let state_clone = state.clone();
                    let window_weak2 = window.downgrade();
                    glib::spawn_future_local(async move {
                        info!("Starting background sync...");
                        if let Some(window) = window_weak2.upgrade() {
                            // Show sync is starting
                            window.show_sync_progress(true);
                            
                            // Start sync
                            window.sync_and_update_libraries(backend_clone, state_clone).await;
                            
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
                    // Try to get server info from Plex backend
                    let any_backend = backend.as_any();
                    info!("Attempting to downcast to PlexBackend");
                    if let Some(plex_backend) = any_backend.downcast_ref::<crate::backends::plex::PlexBackend>() {
                        info!("Successfully downcast to PlexBackend, getting server info...");
                        if let Some(server_info) = plex_backend.get_server_info().await {
                            info!("Updating UI with server info: {:?}", server_info);
                            // Update title with server name
                            imp.status_row.set_title(&server_info.name);
                            
                            // Create detailed subtitle
                            let connection_type = if server_info.is_local {
                                "Local"
                            } else if server_info.is_relay {
                                "Relay"
                            } else {
                                "Remote"
                            };
                            imp.status_row.set_subtitle(&format!("{} - {} connection", user.username, connection_type));
                            
                            // Update icon based on connection type
                            let icon_name = if server_info.is_local {
                                "network-wired-symbolic"  // Local connection
                            } else if server_info.is_relay {
                                "network-cellular-symbolic"  // Relay connection
                            } else {
                                "network-wireless-symbolic"  // Remote direct connection
                            };
                            imp.status_icon.set_icon_name(Some(icon_name));
                        } else {
                            info!("No server info available from Plex backend");
                            imp.status_row.set_title("Plex Server");
                            imp.status_row.set_subtitle(&format!("{} - Connected", user.username));
                            imp.status_icon.set_icon_name(Some("network-transmit-receive-symbolic"));
                        }
                    } else {
                        imp.status_row.set_title("Media Server");
                        imp.status_row.set_subtitle(&format!("{} - Connected", user.username));
                        imp.status_icon.set_icon_name(Some("network-transmit-receive-symbolic"));
                    }
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
                    // Try to get server info from Plex backend
                    let any_backend = backend.as_any();
                    if let Some(plex_backend) = any_backend.downcast_ref::<crate::backends::plex::PlexBackend>() {
                        if let Some(server_info) = plex_backend.get_server_info().await {
                            // Update title with server name
                            imp.status_row.set_title(&server_info.name);
                            
                            // Create detailed subtitle
                            let connection_type = if server_info.is_local {
                                "Local"
                            } else if server_info.is_relay {
                                "Relay"
                            } else {
                                "Remote"
                            };
                            imp.status_row.set_subtitle(&format!("{} - {} connection", user.username, connection_type));
                            
                            // Update icon based on connection type
                            let icon_name = if server_info.is_local {
                                "network-wired-symbolic"  // Local connection
                            } else if server_info.is_relay {
                                "network-cellular-symbolic"  // Relay connection
                            } else {
                                "network-wireless-symbolic"  // Remote direct connection
                            };
                            imp.status_icon.set_icon_name(Some(icon_name));
                        } else {
                            imp.status_row.set_title("Plex Server");
                            imp.status_row.set_subtitle(&format!("{} - Connected", user.username));
                            imp.status_icon.set_icon_name(Some("network-transmit-receive-symbolic"));
                        }
                    } else {
                        imp.status_row.set_title("Media Server");
                        imp.status_row.set_subtitle(&format!("{} - Connected", user.username));
                        imp.status_icon.set_icon_name(Some("network-transmit-receive-symbolic"));
                    }
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
                            info!("Loading cached libraries for instant display...");
                            window.load_cached_libraries().await;
                            
                            // THEN: Start background sync
                            let backend_clone = backend.clone();
                            let state_clone = state_for_async.clone();
                            let window_weak = window.downgrade();
                            glib::spawn_future_local(async move {
                                if let Some(window) = window_weak.upgrade() {
                                    info!("Starting background sync after auth...");
                                    // Show sync progress
                                    window.show_sync_progress(true);
                                    
                                    // Start sync
                                    window.sync_and_update_libraries(backend_clone, state_clone).await;
                                    
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
        // TODO: Implement preferences dialog
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
    
    pub async fn load_cached_libraries(&self) {
        info!("Loading libraries from cache...");
        
        // Create a cache manager to read from cache
        let cache = match crate::services::cache::CacheManager::new() {
            Ok(cache) => Arc::new(cache),
            Err(e) => {
                error!("Failed to create cache manager: {}", e);
                return;
            }
        };
        
        // Create sync manager just for reading cache
        let sync_manager = crate::services::sync::SyncManager::new(cache.clone());
        
        // Get cached libraries and update UI
        match sync_manager.get_cached_libraries("plex").await {
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
                            match sync_manager.get_cached_movies("plex", &library.id).await {
                                Ok(movies) => movies.len(),
                                Err(_) => 0,
                            }
                        }
                        LibraryType::Shows => {
                            // Get show count for this library
                            match sync_manager.get_cached_shows("plex", &library.id).await {
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
                
                // Start sync
                self.sync_and_update_libraries(backend, state.clone()).await;
                
                // Hide sync progress
                self.show_sync_progress(false);
            } else {
                warn!("Backend not initialized, cannot sync");
            }
        } else {
            warn!("No active backend, cannot sync");
        }
    }
    
    pub async fn sync_and_update_libraries(&self, backend: Arc<dyn crate::backends::MediaBackend>, state: Arc<AppState>) {
        info!("Starting library sync...");
        
        // Create a cache manager
        let cache = match crate::services::cache::CacheManager::new() {
            Ok(cache) => Arc::new(cache),
            Err(e) => {
                error!("Failed to create cache manager: {}", e);
                return;
            }
        };
        
        // Create sync manager
        let sync_manager = crate::services::sync::SyncManager::new(cache.clone());
        
        // Perform sync
        match sync_manager.sync_backend("plex", backend).await {
            Ok(result) => {
                info!("Sync completed: {} items synced", result.items_synced);
                
                // Get cached libraries and update UI
                match sync_manager.get_cached_libraries("plex").await {
                    Ok(libraries) => {
                        info!("Found {} libraries in cache", libraries.len());
                        
                        // Build library list with counts
                        let mut library_info = Vec::new();
                        
                        for library in &libraries {
                            use crate::models::LibraryType;
                            let item_count = match library.library_type {
                                LibraryType::Movies => {
                                    // Get movie count for this library
                                    match sync_manager.get_cached_movies("plex", &library.id).await {
                                        Ok(movies) => movies.len(),
                                        Err(_) => 0,
                                    }
                                }
                                LibraryType::Shows => {
                                    // Get show count for this library
                                    match sync_manager.get_cached_shows("plex", &library.id).await {
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
}