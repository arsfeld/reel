use gtk4::{gio, glib, glib::clone, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use tracing::{info, error, warn, debug};

use crate::config::Config;
use crate::state::AppState;
use crate::ui::filters::{WatchStatus, SortOrder};

mod imp {
    use super::*;
    use gtk4::subclass::prelude::*;
    use gtk4::CompositeTemplate;
    
    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/arsfeld/Reel/window.ui")]
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
        pub libraries_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub libraries_list: TemplateChild<gtk4::ListBox>,
        #[template_child]
        pub edit_libraries_button: TemplateChild<gtk4::Button>,
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
        pub home_page: RefCell<Option<crate::ui::pages::HomePage>>,
        pub library_view: RefCell<Option<crate::ui::pages::LibraryView>>,
        pub player_page: RefCell<Option<crate::ui::pages::PlayerPage>>,
        pub show_details_page: RefCell<Option<crate::ui::pages::ShowDetailsPage>>,
        pub back_button: RefCell<Option<gtk4::Button>>,
        pub saved_window_size: RefCell<(i32, i32)>,
        pub filter_controls: RefCell<Option<gtk4::Box>>,
        pub edit_mode: RefCell<bool>,
        pub library_visibility: RefCell<std::collections::HashMap<String, bool>>,
        pub all_libraries: RefCell<Vec<(crate::models::Library, usize)>>,
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
                clone!(#[weak] obj, move |_| {
                    obj.show_auth_dialog();
                })
            );
            
            self.refresh_button.connect_clicked(
                clone!(#[weak] obj, move |_| {
                    let state_clone = obj.imp().state.borrow().as_ref().map(|s| s.clone());
                    if let Some(state) = state_clone {
                        glib::spawn_future_local(async move {
                            obj.trigger_sync(state).await;
                        });
                    }
                })
            );
            
            self.edit_libraries_button.connect_clicked(
                clone!(#[weak] obj, move |button| {
                    obj.toggle_edit_mode(button);
                })
            );
            
            // Connect to home list row activation
            self.home_list.connect_row_activated(
                clone!(#[weak] obj, move |_, row| {
                    if let Some(action_row) = row.downcast_ref::<adw::ActionRow>() {
                        info!("Home selected");
                        obj.show_home_page();
                    }
                })
            );
            
            // Connect to library list row activation
            self.libraries_list.connect_row_activated(
                clone!(#[weak] obj, move |_, row| {
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
        @implements gio::ActionGroup, gio::ActionMap, gtk4::Accessible, gtk4::Buildable, 
                    gtk4::ConstraintTarget, gtk4::Native, gtk4::Root, gtk4::ShortcutManager;
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
        preferences_action.connect_activate(clone!(#[weak(rename_to = window)] self, move |_, _| {
            info!("Opening preferences");
            window.show_preferences();
        }));
        app.add_action(&preferences_action);
        
        // About action
        let about_action = gio::SimpleAction::new("about", None);
        about_action.connect_activate(clone!(#[weak(rename_to = window)] self, move |_, _| {
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
            imp.home_group.set_visible(true);
            imp.libraries_group.set_visible(true);
            
            // TODO: Load libraries from backend
        } else {
            imp.status_row.set_title("Not Connected");
            imp.status_row.set_subtitle("No server configured");
            imp.status_icon.set_icon_name(Some("network-offline-symbolic"));
            imp.welcome_page.set_visible(true);   // Show welcome message
            imp.home_group.set_visible(false);
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
        let imp = self.imp();
        
        // Store all libraries for edit mode
        imp.all_libraries.replace(libraries.clone());
        
        // Load visibility settings if not already loaded
        if imp.library_visibility.borrow().is_empty() && !libraries.is_empty() {
            self.load_library_visibility();
        }
        
        // Clear existing home rows
        while let Some(child) = imp.home_list.first_child() {
            imp.home_list.remove(&child);
        }
        
        // Add Home row to the home section
        let home_row = adw::ActionRow::builder()
            .title("Home")
            .subtitle("Recently added, continue watching, and more")
            .activatable(true)
            .build();
        
        let home_icon = gtk4::Image::from_icon_name("user-home-symbolic");
        home_row.add_prefix(&home_icon);
        
        let home_arrow = gtk4::Image::from_icon_name("go-next-symbolic");
        home_row.add_suffix(&home_arrow);
        
        // Use special ID for home
        home_row.set_widget_name("__home__");
        
        imp.home_list.append(&home_row);
        
        // Show home group
        imp.home_group.set_visible(true);
        
        // Update the library display
        self.update_libraries_display(libraries);
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
                
                // Show home page instead of empty state if we have libraries
                self.show_home_page();
            }
            Err(e) => {
                info!("No cached libraries available: {}", e);
            }
        }
    }
    
    fn show_home_page(&self) {
        let imp = self.imp();
        
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
                let home_page = crate::ui::pages::HomePage::new(state.clone());
                
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
                                MediaItem::Movie(_) => {
                                    info!("HomePage - Navigating to movie player");
                                    window.show_player(&media_item, state).await;
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
        imp.content_header.set_title_widget(Some(&gtk4::Label::new(Some("Home"))));
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
                        
                        // Show home page if we have libraries
                        if !libraries.is_empty() {
                            self.show_home_page();
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
    
    async fn show_show_details(&self, show: crate::models::Show, state: Arc<AppState>) {
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
            let existing_page = imp.show_details_page.borrow();
            existing_page.as_ref().cloned()
        }.unwrap_or_else(|| {
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
            
            imp.show_details_page.replace(Some(page.clone()));
            
            // Add to content stack
            content_stack.add_named(page.widget(), Some("show_details"));
            
            page
        });
        
        // Load the show
        show_details_page.load_show(show.clone()).await;
        
        // Update the content page title
        imp.content_page.set_title(&show.title);
        
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
    
    async fn show_player(&self, media_item: &crate::models::MediaItem, state: Arc<AppState>) {
        info!("MainWindow::show_player() - Called for media: {}", media_item.title());
        debug!("MainWindow::show_player() - Media type: {:?}, ID: {}", 
            std::mem::discriminant(media_item), media_item.id());
        
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
        
        // Create or get player page
        debug!("MainWindow::show_player() - Getting or creating player page");
        let player_page = {
            let existing_page = imp.player_page.borrow();
            if existing_page.is_some() {
                debug!("MainWindow::show_player() - Using existing player page");
            }
            existing_page.as_ref().cloned()
        }.unwrap_or_else(|| {
            info!("MainWindow::show_player() - Creating new player page");
            let page = crate::ui::pages::PlayerPage::new(state.clone());
            imp.player_page.replace(Some(page.clone()));
            
            // Add to content stack
            debug!("MainWindow::show_player() - Adding player page to content stack");
            content_stack.add_named(page.widget(), Some("player"));
            info!("MainWindow::show_player() - Player page added to stack with name 'player'");
            
            page
        });
        
        // Update the content page title first
        imp.content_page.set_title(media_item.title());
        debug!("MainWindow::show_player() - Updated content page title");
        
        // Load the media (but don't block navigation on failure)
        debug!("MainWindow::show_player() - Loading media into player");
        if let Err(e) = player_page.load_media(media_item, state).await {
            error!("MainWindow::show_player() - Failed to load media: {}", e);
            // Show error dialog but still navigate to player page
            let dialog = adw::AlertDialog::new(
                Some("Failed to Load Media"),
                Some(&format!("Error: {}", e))
            );
            dialog.add_response("ok", "OK");
            dialog.set_default_response(Some("ok"));
            dialog.present(Some(self));
        }
        
        // Clear any existing back buttons from the main header before hiding it
        if let Some(old_button) = imp.back_button.borrow().as_ref() {
            imp.content_header.remove(old_button);
        }
        imp.back_button.replace(None);
        
        // Hide the main header bar completely for player mode
        imp.content_header.set_visible(false);
        
        // Create a custom overlay header bar for the player
        let player_header = adw::HeaderBar::new();
        player_header.add_css_class("osd");
        player_header.add_css_class("overlay-header");
        player_header.set_show_title(false);
        player_header.set_show_end_title_buttons(true);  // Keep close button visible
        
        // Always add back button for player since we need to stop playback
        // The NavigationSplitView's sidebar toggle doesn't handle video cleanup
        {
            // Create a simple back button for the overlay header
            let back_button = gtk4::Button::builder()
                .icon_name("go-previous-symbolic")
                .tooltip_text("Back to Library")
                .build();
            back_button.add_css_class("osd");
            
            player_header.pack_start(&back_button);
            
            // Connect the back button click handler with all necessary cleanup
            let window_weak = self.downgrade();
            back_button.connect_clicked(move |_| {
                if let Some(window) = window_weak.upgrade() {
                    // Stop the player before going back
                    if let Some(player_page) = window.imp().player_page.borrow().as_ref() {
                        let player_page = player_page.clone();
                        glib::spawn_future_local(async move {
                            player_page.stop().await;
                        });
                    }
                    
                    // Show the main header bar again
                    window.imp().content_header.set_visible(true);
                    
                    // Restore saved window size first
                    let (width, height) = *window.imp().saved_window_size.borrow();
                    window.set_default_size(width, height);
                    
                    // First restore the sidebar state
                    if let Some(content) = window.content() {
                        if let Some(split_view) = content.downcast_ref::<adw::NavigationSplitView>() {
                            // Ensure both sidebar and content are visible
                            split_view.set_collapsed(false);
                            split_view.set_show_content(true);
                        }
                    }
                    
                    // Navigate back to library view
                    // First try to just show the existing library view
                    if let Some(stack) = window.imp().content_stack.borrow().as_ref() {
                        stack.set_visible_child_name("library");
                    }
                    
                    // If we need to properly restore the library view with correct state
                    if let Some(state) = window.imp().state.borrow().as_ref() {
                        let state = state.clone();
                        let window_weak = window.downgrade();
                        glib::spawn_future_local(async move {
                            if let Some(window) = window_weak.upgrade() {
                                // Get the current library from state if available
                                let backend_manager = state.backend_manager.read().await;
                                if let Some((backend_id, backend)) = backend_manager.get_active_backend() {
                                    // Get the libraries from the backend
                                    if let Ok(libraries) = backend.get_libraries().await {
                                        if let Some(library) = libraries.first() {
                                            window.show_library_view(backend_id.clone(), library.clone()).await;
                                        }
                                    }
                                }
                            }
                        });
                    }
                }
            });
        }
        
        // Add the overlay header to the player page's overlay
        // The player page widget is a Box, and its first child is the Overlay
        let player_widget = player_page.widget();
        if let Some(first_child) = player_widget.first_child() {
            if let Some(overlay) = first_child.downcast_ref::<gtk4::Overlay>() {
                // Position the header at the top
                let header_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
                header_box.set_valign(gtk4::Align::Start);
                header_box.append(&player_header);
                overlay.add_overlay(&header_box);
                
                // Initially hide the overlay header
                header_box.set_visible(false);
                
                // Set up hover detection for the overlay header
                let header_box_weak = header_box.downgrade();
                let hide_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
                let hover_controller = gtk4::EventControllerMotion::new();
                
                let hide_timer_clone = hide_timer.clone();
                hover_controller.connect_motion(move |_, _, _| {
                    if let Some(header) = header_box_weak.upgrade() {
                        header.set_visible(true);
                        
                        // Cancel previous timer if exists
                        if let Some(timer_id) = hide_timer_clone.borrow_mut().take() {
                            timer_id.remove();
                        }
                        
                        // Hide again after 3 seconds of no movement
                        let header_weak_inner = header_box_weak.clone();
                        let hide_timer_inner = hide_timer_clone.clone();
                        let timer_id = glib::timeout_add_local(std::time::Duration::from_secs(3), move || {
                            if let Some(header) = header_weak_inner.upgrade() {
                                header.set_visible(false);
                            }
                            hide_timer_inner.borrow_mut().take();
                            glib::ControlFlow::Break
                        });
                        hide_timer_clone.borrow_mut().replace(timer_id);
                    }
                });
                
                // Apply controller to the entire overlay
                overlay.add_controller(hover_controller);
            }
        }
        
        // Save current window size before changing it
        let (current_width, current_height) = self.default_size();
        imp.saved_window_size.replace((current_width, current_height));
        
        // Show the player page
        info!("MainWindow::show_player() - Switching stack to 'player' page");
        content_stack.set_visible_child_name("player");
        info!("MainWindow::show_player() - Navigation to player complete");
        
        // Hide the sidebar and header bar for immersive playback
        if let Some(content) = self.content() {
            if let Some(split_view) = content.downcast_ref::<adw::NavigationSplitView>() {
                split_view.set_collapsed(true);
            }
        }
        
        
        // Try to resize window to match video aspect ratio after a short delay
        // (to give GStreamer time to negotiate the video format)
        let window_weak = self.downgrade();
        let player_page_clone = player_page.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
            let window_weak = window_weak.clone();
            let player_page = player_page_clone.clone();
            glib::spawn_future_local(async move {
                if let Some(window) = window_weak.upgrade() {
                    if let Some((width, height)) = player_page.get_video_dimensions().await {
                        // Calculate aspect ratio
                        let aspect_ratio = width as f64 / height as f64;
                        
                        // Calculate new width based on aspect ratio
                        // Use a reasonable height (e.g., 720p)
                        let target_height = 720.min(height).max(480);
                        let target_width = (target_height as f64 * aspect_ratio) as i32;
                        
                        // Set the new window size
                        window.set_default_size(target_width, target_height);
                        
                        info!("Resized window to {}x{} (aspect ratio: {:.2})", 
                              target_width, target_height, aspect_ratio);
                    }
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
        }.unwrap_or_else(|| {
            let state = imp.state.borrow().as_ref().unwrap().clone();
            let view = crate::ui::pages::LibraryView::new(state.clone());
            
            // Set the media selected callback to handle different media types
            let window_weak = self.downgrade();
            let state_clone = state.clone();
            view.set_on_media_selected(move |media_item| {
                info!("MainWindow - Media selected callback triggered: {}", media_item.title());
                if let Some(window) = window_weak.upgrade() {
                    let media_item = media_item.clone();
                    let state = state_clone.clone();
                    debug!("MainWindow - Spawning navigation task for: {}", media_item.title());
                    glib::spawn_future_local(async move {
                        use crate::models::MediaItem;
                        info!("MainWindow - Processing media selection: {}", media_item.title());
                        match &media_item {
                            MediaItem::Movie(_) => {
                                // Movies go directly to player
                                info!("MainWindow - Movie selected, navigating to player");
                                window.show_player(&media_item, state).await;
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
        imp.content_header.set_title_widget(Some(&gtk4::Label::builder()
            .label(&library.title)
            .single_line_mode(true)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .build()));
        
        // Create filter controls for the header bar
        let filter_controls = self.create_filter_controls(&library_view);
        imp.content_header.pack_end(&filter_controls);
        imp.filter_controls.replace(Some(filter_controls));
        
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
        if let Some(content) = self.content() {
            if let Some(split_view) = content.downcast_ref::<adw::NavigationSplitView>() {
                split_view.set_show_content(false);
            }
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
        
        let watch_label = gtk4::Label::builder()
            .label("Show:")
            .build();
        watch_label.add_css_class("dim-label");
        
        let watch_model = gtk4::StringList::new(&[
            "All",
            "Unwatched",
            "Watched",
            "In Progress"
        ]);
        
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
        
        let sort_label = gtk4::Label::builder()
            .label("Sort:")
            .build();
        sort_label.add_css_class("dim-label");
        
        let sort_model = gtk4::StringList::new(&[
            "Title (A-Z)",
            "Title (Z-A)",
            "Year (Oldest)",
            "Year (Newest)",
            "Rating (Low-High)",
            "Rating (High-Low)",
            "Date Added (Oldest)",
            "Date Added (Newest)"
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
    
    fn toggle_edit_mode(&self, button: &gtk4::Button) {
        let imp = self.imp();
        let current_mode = *imp.edit_mode.borrow();
        let new_mode = !current_mode;
        
        imp.edit_mode.replace(new_mode);
        
        if new_mode {
            button.set_icon_name("object-select-symbolic");
            button.set_tooltip_text(Some("Done Editing"));
            
            // Show all libraries in edit mode
            let all_libraries = imp.all_libraries.borrow().clone();
            self.update_libraries_display(all_libraries);
        } else {
            button.set_icon_name("document-edit-symbolic");
            button.set_tooltip_text(Some("Edit Libraries"));
            
            // Save the visibility settings
            self.save_library_visibility();
            
            // Refresh display with visibility applied
            let all_libraries = imp.all_libraries.borrow().clone();
            self.update_libraries_display(all_libraries);
        }
    }
    
    fn update_libraries_display(&self, libraries: Vec<(crate::models::Library, usize)>) {
        let imp = self.imp();
        
        // Clear existing library rows
        while let Some(child) = imp.libraries_list.first_child() {
            imp.libraries_list.remove(&child);
        }
        
        let is_edit_mode = *imp.edit_mode.borrow();
        let visibility_map = imp.library_visibility.borrow();
        
        // Add a row for each library
        for (library, item_count) in libraries {
            // Check if library should be shown
            let is_visible = visibility_map.get(&library.id).copied().unwrap_or(true);
            
            if is_edit_mode || is_visible {
                let row = adw::ActionRow::builder()
                    .title(&library.title)
                    .subtitle(&format!("{} items", item_count))
                    .activatable(!is_edit_mode)  // Only activatable when not in edit mode
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
                
                // In edit mode, add checkbox; otherwise add navigation arrow
                if is_edit_mode {
                    let check_button = gtk4::CheckButton::builder()
                        .active(is_visible)
                        .build();
                    
                    let library_id = library.id.clone();
                    let window_weak = self.downgrade();
                    check_button.connect_toggled(move |button| {
                        if let Some(window) = window_weak.upgrade() {
                            window.imp().library_visibility.borrow_mut()
                                .insert(library_id.clone(), button.is_active());
                        }
                    });
                    
                    row.add_suffix(&check_button);
                } else {
                    let arrow = gtk4::Image::from_icon_name("go-next-symbolic");
                    row.add_suffix(&arrow);
                }
                
                // Store the library ID in the widget name
                row.set_widget_name(&library.id);
                
                imp.libraries_list.append(&row);
            }
        }
    }
    
    fn load_library_visibility(&self) {
        // Load from existing config
        if let Some(config) = self.imp().config.borrow().as_ref() {
            let visibility = config.get_all_library_visibility();
            *self.imp().library_visibility.borrow_mut() = visibility;
        }
    }
    
    fn save_library_visibility(&self) {
        // Save to config using proper methods
        let visibility = self.imp().library_visibility.borrow().clone();
        
        // Clone the config, update it, and save
        let config_clone = {
            let config_ref = self.imp().config.borrow();
            if let Some(config) = config_ref.as_ref() {
                Some(config.as_ref().clone())
            } else {
                None
            }
        }; // Drop the borrow here
        
        if let Some(mut config) = config_clone {
            if let Err(e) = config.set_all_library_visibility(visibility) {
                error!("Failed to save library visibility: {}", e);
            } else {
                // Update the stored config
                self.imp().config.replace(Some(Arc::new(config)));
            }
        }
    }
}