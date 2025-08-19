use gtk4::{gio, glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::sync::Arc;
use tracing::{debug, info, error};

use crate::backends::plex::{PlexAuth, PlexPin, PlexServer, PlexConnection, PlexBackend};
use crate::state::AppState;
use crate::models::Credentials;

mod imp {
    use super::*;
    use gtk4::CompositeTemplate;
    
    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/arsfeld/Reel/auth_dialog.ui")]
    pub struct ReelAuthDialog {
        #[template_child]
        pub cancel_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub view_stack: TemplateChild<adw::ViewStack>,
        #[template_child]
        pub pin_status_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub pin_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub auth_progress: TemplateChild<gtk4::ProgressBar>,
        #[template_child]
        pub auth_status: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub auth_error: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub retry_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub open_link_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub server_url_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub token_entry: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub manual_connect_button: TemplateChild<gtk4::Button>,
        
        pub state: RefCell<Option<Arc<AppState>>>,
        pub auth_handle: RefCell<Option<glib::JoinHandle<()>>>,
        pub current_pin: RefCell<Option<PlexPin>>,
    }
    
    #[glib::object_subclass]
    impl ObjectSubclass for ReelAuthDialog {
        const NAME: &'static str = "ReelAuthDialog";
        type Type = super::ReelAuthDialog;
        type ParentType = adw::Dialog;
        
        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }
        
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }
    
    #[gtk4::template_callbacks]
    impl ReelAuthDialog {
        #[template_callback]
        fn on_cancel_clicked(&self) {
            info!("Cancel button clicked");
            self.obj().close();
        }
        
        #[template_callback]
        fn on_open_link_clicked(&self) {
            info!("Opening plex.tv/link");
            if let Err(e) = gio::AppInfo::launch_default_for_uri("https://plex.tv/link", None::<&gio::AppLaunchContext>) {
                error!("Failed to open browser: {}", e);
            }
        }
        
        #[template_callback]
        fn on_retry_clicked(&self) {
            info!("Retrying authentication");
            self.auth_error.set_visible(false);
            self.pin_status_page.set_visible(true);
            self.obj().start_auth();
        }
        
        #[template_callback]
        fn on_manual_connect_clicked(&self) {
            let url = self.server_url_entry.text();
            let token = self.token_entry.text();
            
            if !url.is_empty() && !token.is_empty() {
                info!("Manual connection to: {}", url);
                self.obj().connect_manual(url.to_string(), token.to_string());
            }
        }
    }
    
    impl ObjectImpl for ReelAuthDialog {
        fn constructed(&self) {
            self.parent_constructed();
            
            let obj = self.obj();
            
            // Set placeholder text for server URL
            self.server_url_entry.set_text("http://192.168.1.100:32400");
            
            // Enable manual connect when both fields have text
            let update_manual_button = glib::clone!(@weak obj => move || {
                let imp = obj.imp();
                let has_url = !imp.server_url_entry.text().is_empty();
                let has_token = !imp.token_entry.text().is_empty();
                imp.manual_connect_button.set_sensitive(has_url && has_token);
            });
            
            self.server_url_entry.connect_changed(
                glib::clone!(@strong update_manual_button => move |_| {
                    update_manual_button();
                })
            );
            
            self.token_entry.connect_changed(move |_| {
                update_manual_button();
            });
        }
        
        fn dispose(&self) {
            // Cancel any ongoing authentication
            if let Some(handle) = self.auth_handle.take() {
                handle.abort();
            }
        }
    }
    
    impl WidgetImpl for ReelAuthDialog {}
    impl adw::subclass::dialog::AdwDialogImpl for ReelAuthDialog {}
}

glib::wrapper! {
    pub struct ReelAuthDialog(ObjectSubclass<imp::ReelAuthDialog>)
        @extends gtk4::Widget, adw::Dialog,
        @implements gtk4::Accessible, gtk4::Buildable;
}

impl ReelAuthDialog {
    pub fn new(state: Arc<AppState>) -> Self {
        let dialog: Self = glib::Object::builder().build();
        dialog.imp().state.replace(Some(state));
        dialog
    }
    
    pub fn start_auth(&self) {
        info!("Starting Plex authentication");
        
        let imp = self.imp();
        
        // Cancel any existing auth
        if let Some(handle) = imp.auth_handle.take() {
            handle.abort();
        }
        
        // Show progress
        imp.auth_progress.set_visible(true);
        imp.auth_progress.pulse();
        
        // Start pulsing animation
        let progress = imp.auth_progress.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            progress.pulse();
            glib::ControlFlow::Continue
        });
        
        // Start authentication flow
        let dialog_weak = self.downgrade();
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        
        // Spawn Tokio task for async operations
        tokio::spawn(async move {
            // Get a PIN from Plex
            match PlexAuth::get_pin().await {
                Ok(pin) => {
                    let _ = tx.send(Ok(pin)).await;
                }
                Err(e) => {
                    let _ = tx.send(Err(e.to_string())).await;
                }
            }
        });
        
        let dialog_weak2 = self.downgrade();
        let handle = glib::spawn_future_local(async move {
            if let Some(result) = rx.recv().await {
                if let Some(dialog) = dialog_weak2.upgrade() {
                    match result {
                        Ok(pin) => {
                            dialog.start_auth_with_pin(pin).await;
                        }
                        Err(e) => {
                            dialog.on_auth_error(e);
                        }
                    }
                }
            }
        });
        
        imp.auth_handle.replace(Some(handle));
    }
    
    async fn start_auth_with_pin(&self, pin: PlexPin) {
        // Display the PIN to the user
        let imp = self.imp();
        imp.pin_label.set_text(&pin.code);
        imp.current_pin.replace(Some(pin.clone()));
        imp.pin_status_page.set_visible(true);
        imp.auth_status.set_visible(false);
        imp.auth_error.set_visible(false);
        
        // Poll for the auth token in Tokio runtime
        let pin_id = pin.id.clone();
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        
        tokio::spawn(async move {
            match PlexAuth::poll_for_token(&pin_id).await {
                Ok(token) => {
                    let _ = tx.send(Ok(token)).await;
                }
                Err(e) => {
                    let _ = tx.send(Err(e.to_string())).await;
                }
            }
        });
        
        let dialog_weak = self.downgrade();
        glib::spawn_future_local(async move {
            if let Some(result) = rx.recv().await {
                if let Some(dialog) = dialog_weak.upgrade() {
                    match result {
                        Ok(token) => {
                            dialog.on_auth_success(token);
                        }
                        Err(e) => {
                            dialog.on_auth_error(e);
                        }
                    }
                }
            }
        });
    }
    
    fn on_auth_success(&self, token: String) {
        info!("Authentication successful with token: {}...", &token[..8]);
        
        let imp = self.imp();
        imp.auth_progress.set_visible(false);
        imp.pin_status_page.set_visible(false);
        imp.auth_status.set_visible(true);
        imp.auth_status.set_icon_name(Some("emblem-ok-symbolic"));
        imp.auth_status.set_title("Authentication Successful!");
        imp.auth_status.set_description(Some("Connecting to your Plex server..."));
        
        // Store token and start server discovery
        if let Some(state) = imp.state.borrow().as_ref() {
            let state_clone = state.clone();
            let token_clone = token.clone();
            let dialog_weak = self.downgrade();
            
            // Use Tokio for async operations
            let (tx, mut rx) = tokio::sync::mpsc::channel(1);
            let token_for_task = token_clone.clone();
            
            tokio::spawn(async move {
                // Simply get user info and return token
                // Backend setup and server discovery will happen later
                
                // Get user info from Plex
                match PlexAuth::get_user(&token_for_task).await {
                    Ok(plex_user) => {
                        let user = crate::models::User {
                            id: plex_user.id.to_string(),
                            username: plex_user.username,
                            email: Some(plex_user.email),
                            avatar_url: plex_user.thumb,
                        };
                        
                        let _ = tx.send((user, token_for_task)).await;
                        info!("Successfully authenticated with Plex");
                    }
                    Err(e) => {
                        error!("Failed to get user info: {}", e);
                    }
                }
            });
            
            glib::spawn_future_local(async move {
                if let Some((user, token)) = rx.recv().await {
                    // Save the token to config file directly
                    let config_dir = dirs::config_dir().unwrap().join("reel");
                    std::fs::create_dir_all(&config_dir).ok();
                    let token_file = config_dir.join("plex_token");
                    if let Err(e) = std::fs::write(&token_file, &token) {
                        error!("Failed to save Plex token: {}", e);
                    } else {
                        info!("Plex token saved successfully");
                    }
                    
                    // Save user info
                    state_clone.set_user(user.clone()).await;
                    
                    // Update the main window UI
                    if let Some(dialog) = dialog_weak.upgrade() {
                        // Trigger sync after successful authentication
                        info!("Authentication successful, starting sync...");
                        
                        // Note: We'll trigger sync from the main window after dialog closes
                        // The main window will detect the new authentication and sync automatically
                        
                        dialog.close();
                    }
                }
            });
        }
    }
    
    fn on_auth_error(&self, error: String) {
        error!("Authentication failed: {}", error);
        
        let imp = self.imp();
        imp.auth_progress.set_visible(false);
        imp.pin_status_page.set_visible(false);
        imp.auth_error.set_visible(true);
        imp.auth_error.set_icon_name(Some("dialog-error-symbolic"));
        imp.auth_error.set_title("Authentication Failed");
        imp.auth_error.set_description(Some(&error));
        imp.retry_button.set_visible(true);
    }
    
    fn connect_manual(&self, url: String, _token: String) {
        info!("Connecting manually to {}", url);
        
        let state = self.imp().state.borrow().as_ref().map(|s| s.clone());
        
        if let Some(state) = state {
            let dialog_weak = self.downgrade();
            
            glib::spawn_future_local(async move {
                let mut backend_manager = state.backend_manager.write().await;
                
                // Create a new Plex backend
                let plex_backend = Arc::new(crate::backends::plex::PlexBackend::new());
                
                // Create a manual server entry
                let server = PlexServer {
                    name: "Manual Server".to_string(),
                    product: "Plex Media Server".to_string(),
                    product_version: String::new(),
                    platform: String::new(),
                    platform_version: String::new(),
                    device: String::new(),
                    client_identifier: String::new(),
                    created_at: String::new(),
                    last_seen_at: String::new(),
                    provides: "server".to_string(),
                    owned: true,
                    home: true,
                    connections: vec![crate::backends::plex::PlexConnection {
                        protocol: "https".to_string(),
                        address: url.clone(),
                        port: 32400,
                        uri: url,
                        local: false,
                        relay: false,
                    }],
                };
                
                // Connect with the manual server
                if let Err(e) = plex_backend.authenticate_with_pin(
                    &PlexPin { id: String::new(), code: String::new() }, 
                    &server
                ).await {
                    error!("Failed to connect to server: {}", e);
                } else {
                    // Get or reuse backend ID
                    let config = state.config.clone();
                    let last_active = config.get_last_active_backend();
                    
                    let backend_id = if let Some(ref last_id) = last_active {
                        if last_id.starts_with("plex") {
                            // Reuse the last active backend ID
                            info!("Reusing existing backend ID for manual connection: {}", last_id);
                            last_id.clone()
                        } else {
                            "plex".to_string()
                        }
                    } else {
                        "plex".to_string()
                    };
                    
                    // Register the backend
                    backend_manager.register_backend(backend_id.clone(), plex_backend.clone());
                    backend_manager.set_active(&backend_id).ok();
                    
                    // Save as last active backend
                    let mut config = state.config.as_ref().clone();
                    let _ = config.set_last_active_backend(&backend_id);
                    
                    // Start sync
                    if let Some(dialog) = dialog_weak.upgrade() {
                        dialog.start_sync().await;
                    }
                }
            });
        }
        
        self.close();
    }
    
    async fn start_sync(&self) {
        if let Some(state) = self.imp().state.borrow().as_ref() {
            info!("Starting library sync");
            let state_clone = state.clone();
            
            // Run sync in Tokio runtime
            let (tx, mut rx) = tokio::sync::mpsc::channel(1);
            tokio::spawn(async move {
                let result = state_clone.sync_active_backend().await;
                let _ = tx.send(result).await;
            });
            
            // Handle result in GTK context
            if let Some(result) = rx.recv().await {
                match result {
                    Ok(()) => info!("Sync completed successfully"),
                    Err(e) => error!("Failed to sync: {}", e),
                }
            }
        }
    }
}