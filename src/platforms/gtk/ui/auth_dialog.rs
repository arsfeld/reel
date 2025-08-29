use gtk4::{gio, glib, glib::clone, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::sync::Arc;
use tracing::{error, info};

use crate::backends::plex::{PlexAuth, PlexPin};
use crate::state::AppState;

// Re-export BackendType for external use
pub use imp::BackendType;

mod imp {
    use super::*;
    use gtk4::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/dev/arsfeld/Reel/auth_dialog.ui")]
    pub struct ReelAuthDialog {
        #[template_child]
        pub cancel_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub save_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub view_stack: TemplateChild<adw::ViewStack>,
        // Plex automatic auth elements
        #[template_child]
        pub pin_status_page: TemplateChild<gtk4::Box>,
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
        // Plex manual auth elements
        #[template_child]
        pub server_url_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub token_entry: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub manual_connect_button: TemplateChild<gtk4::Button>,
        // Jellyfin UI elements
        #[template_child]
        pub jellyfin_url_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub jellyfin_username_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub jellyfin_password_entry: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub jellyfin_progress: TemplateChild<gtk4::ProgressBar>,
        #[template_child]
        pub jellyfin_error: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub jellyfin_retry_button: TemplateChild<gtk4::Button>,

        pub state: RefCell<Option<Arc<AppState>>>,
        pub auth_handle: RefCell<Option<glib::JoinHandle<()>>>,
        pub current_pin: RefCell<Option<PlexPin>>,
        pub backend_type: RefCell<BackendType>,
    }

    #[derive(Debug, Clone, Copy, Default)]
    pub enum BackendType {
        #[default]
        Plex,
        Jellyfin,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ReelAuthDialog {
        const NAME: &'static str = "ReelAuthDialog";
        type Type = super::ReelAuthDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ReelAuthDialog {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            // Connect button signals manually instead of using template callbacks
            self.cancel_button.connect_clicked(clone!(
                #[weak]
                obj,
                move |_| {
                    info!("Cancel button clicked");
                    obj.close();
                }
            ));

            self.open_link_button.connect_clicked(|_| {
                info!("Opening plex.tv/link");
                if let Err(e) = gio::AppInfo::launch_default_for_uri(
                    "https://plex.tv/link",
                    None::<&gio::AppLaunchContext>,
                ) {
                    error!("Failed to open browser: {}", e);
                }
            });

            self.retry_button.connect_clicked(clone!(
                #[weak]
                obj,
                move |_| {
                    info!("Retrying authentication");
                    let imp = obj.imp();
                    imp.auth_error.set_visible(false);
                    imp.pin_status_page.set_visible(true);
                    obj.start_auth();
                }
            ));

            self.save_button.connect_clicked(clone!(
                #[weak]
                obj,
                move |_| {
                    let imp = obj.imp();
                    let backend_type = *imp.backend_type.borrow();
                    match backend_type {
                        BackendType::Plex => {
                            let url = imp.server_url_entry.text();
                            let token = imp.token_entry.text();

                            if !url.is_empty() && !token.is_empty() {
                                info!("Connecting to Plex server");
                                obj.connect_manual(url.to_string(), token.to_string());
                            }
                        }
                        BackendType::Jellyfin => {
                            let url = imp.jellyfin_url_entry.text();
                            let username = imp.jellyfin_username_entry.text();
                            let password = imp.jellyfin_password_entry.text();

                            if !url.is_empty() && !username.is_empty() && !password.is_empty() {
                                info!("Connecting to Jellyfin server");
                                obj.connect_jellyfin(
                                    url.to_string(),
                                    username.to_string(),
                                    password.to_string(),
                                );
                            }
                        }
                    }
                }
            ));

            self.manual_connect_button.connect_clicked(clone!(
                #[weak]
                obj,
                move |_| {
                    let imp = obj.imp();
                    let url = imp.server_url_entry.text();
                    let token = imp.token_entry.text();

                    if !url.is_empty() && !token.is_empty() {
                        info!("Manual connection to: {}", url);
                        obj.connect_manual(url.to_string(), token.to_string());
                    }
                }
            ));

            // jellyfin_connect_button is hidden - using Save/Connect button in header instead

            self.jellyfin_retry_button.connect_clicked(clone!(
                #[weak]
                obj,
                move |_| {
                    let imp = obj.imp();
                    imp.jellyfin_error.set_visible(false);
                    // Show the input fields again
                    imp.jellyfin_url_entry.set_visible(true);
                    imp.jellyfin_username_entry.set_visible(true);
                    imp.jellyfin_password_entry.set_visible(true);
                    // Re-enable them
                    imp.jellyfin_url_entry.set_sensitive(true);
                    imp.jellyfin_username_entry.set_sensitive(true);
                    imp.jellyfin_password_entry.set_sensitive(true);
                }
            ));

            // Update save button and connect buttons based on current backend
            let update_buttons = clone!(
                #[weak]
                obj,
                move || {
                    let imp = obj.imp();
                    let backend_type = *imp.backend_type.borrow();

                    let save_enabled = match backend_type {
                        BackendType::Plex => {
                            let has_url = !imp.server_url_entry.text().is_empty();
                            let has_token = !imp.token_entry.text().is_empty();
                            imp.manual_connect_button
                                .set_sensitive(has_url && has_token);
                            has_url && has_token
                        }
                        BackendType::Jellyfin => {
                            let has_url = !imp.jellyfin_url_entry.text().is_empty();
                            let has_username = !imp.jellyfin_username_entry.text().is_empty();
                            let has_password = !imp.jellyfin_password_entry.text().is_empty();
                            has_url && has_username && has_password
                        }
                    };

                    imp.save_button.set_sensitive(save_enabled);
                }
            );

            // Set placeholder text for server URL
            self.server_url_entry.set_text("http://192.168.1.100:32400");

            self.server_url_entry.connect_changed(clone!(
                #[strong]
                update_buttons,
                move |_| {
                    update_buttons();
                }
            ));

            self.token_entry.connect_changed(clone!(
                #[strong]
                update_buttons,
                move |_| {
                    update_buttons();
                }
            ));

            self.jellyfin_url_entry.connect_changed(clone!(
                #[strong]
                update_buttons,
                move |_| {
                    update_buttons();
                }
            ));

            self.jellyfin_username_entry.connect_changed(clone!(
                #[strong]
                update_buttons,
                move |_| {
                    update_buttons();
                }
            ));

            self.jellyfin_password_entry.connect_changed(move |_| {
                update_buttons();
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
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl ReelAuthDialog {
    pub fn new(state: Arc<AppState>) -> Self {
        let dialog: Self = glib::Object::builder().build();
        dialog.imp().state.replace(Some(state));
        dialog
    }

    pub fn set_backend_type(&self, backend_type: BackendType) {
        let imp = self.imp();
        imp.backend_type.replace(backend_type);

        // Update the title and visible stack page
        match backend_type {
            BackendType::Plex => {
                self.set_title("Connect to Plex");
                // Show Plex tabs, hide Jellyfin
                let stack = &*imp.view_stack;
                if let Some(child) = stack.child_by_name("automatic") {
                    let page = stack.page(&child);
                    page.set_visible(true);
                }
                if let Some(child) = stack.child_by_name("manual") {
                    let page = stack.page(&child);
                    page.set_visible(true);
                }
                if let Some(child) = stack.child_by_name("jellyfin") {
                    let page = stack.page(&child);
                    page.set_visible(false);
                }
                stack.set_visible_child_name("automatic");
            }
            BackendType::Jellyfin => {
                self.set_title("Connect to Jellyfin");
                // Hide Plex tabs, show Jellyfin
                let stack = &*imp.view_stack;
                if let Some(child) = stack.child_by_name("automatic") {
                    let page = stack.page(&child);
                    page.set_visible(false);
                }
                if let Some(child) = stack.child_by_name("manual") {
                    let page = stack.page(&child);
                    page.set_visible(false);
                }
                if let Some(child) = stack.child_by_name("jellyfin") {
                    let page = stack.page(&child);
                    page.set_visible(true);
                }
                stack.set_visible_child_name("jellyfin");
            }
        }
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
            if let Some(result) = rx.recv().await
                && let Some(dialog) = dialog_weak2.upgrade()
            {
                match result {
                    Ok(pin) => {
                        dialog.start_auth_with_pin(pin).await;
                    }
                    Err(e) => {
                        dialog.on_auth_error(e);
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
            if let Some(result) = rx.recv().await
                && let Some(dialog) = dialog_weak.upgrade()
            {
                match result {
                    Ok(token) => {
                        dialog.on_auth_success(token);
                    }
                    Err(e) => {
                        dialog.on_auth_error(e);
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
        imp.auth_status
            .set_description(Some("Connecting to your Plex server..."));

        // Store token and start server discovery
        if let Some(state) = imp.state.borrow().as_ref() {
            let state_clone = state.clone();
            let token_clone = token.clone();
            let dialog_weak = self.downgrade();
            let current_pin = imp.current_pin.borrow().clone();

            // Use Tokio for async operations
            let (tx, mut rx) = tokio::sync::mpsc::channel(1);
            let token_for_task = token_clone.clone();

            tokio::spawn(async move {
                // Discover servers and authenticate
                match PlexAuth::discover_servers(&token_for_task).await {
                    Ok(servers) => {
                        if !servers.is_empty() {
                            let server = servers.into_iter().next().unwrap();
                            let _ = tx.send(Some((token_for_task, server))).await;
                        } else {
                            error!("No Plex servers found");
                            let _ = tx.send(None).await;
                        }
                    }
                    Err(e) => {
                        error!("Failed to discover servers: {}", e);
                        let _ = tx.send(None).await;
                    }
                }
            });

            glib::spawn_future_local(async move {
                if let Some(Some((token, server))) = rx.recv().await {
                    // Backend IDs are managed by SourceCoordinator

                    // Use SourceCoordinator to add Plex account
                    let source_coordinator = state_clone.get_source_coordinator();
                    match source_coordinator.add_plex_account(&token).await {
                        Ok(sources) => {
                            info!(
                                "Successfully added Plex account with {} sources",
                                sources.len()
                            );

                            // No automatic backend switching - let the UI handle it

                            // Get user info
                            if let Ok(plex_user) = PlexAuth::get_user(&token).await {
                                let user = crate::models::User {
                                    id: plex_user.id.to_string(),
                                    username: plex_user.username,
                                    email: Some(plex_user.email),
                                    avatar_url: plex_user.thumb,
                                };
                                state_clone.set_user(user.clone()).await;
                            }
                        }
                        Err(e) => {
                            error!(
                                "Failed to add Plex account through SourceCoordinator: {}",
                                e
                            );
                        }
                    }
                } else {
                    // No token/server pair received
                    error!("Authentication failed: no token/server received");
                }

                // Close dialog
                if let Some(dialog) = dialog_weak.upgrade() {
                    dialog.close();
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
                // TODO: Manual server connection should be handled through SourceCoordinator
                // This code needs to be refactored to use proper authentication flow
                error!("Manual server connection not yet implemented with new architecture");

                // DEPRECATED CODE - this was removed due to architecture changes
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
                let result = state_clone.sync_all_backends().await;
                let _ = tx.send(result).await;
            });

            // Handle result in GTK context
            if let Some(result) = rx.recv().await {
                match result {
                    Ok(results) => {
                        info!("Sync completed for {} backends", results.len());
                        for result in results {
                            if result.success {
                                info!("Backend synced successfully: {} items", result.items_synced);
                            } else {
                                error!("Backend sync failed: {:?}", result.errors);
                            }
                        }
                    }
                    Err(e) => error!("Failed to sync: {}", e),
                }
            }
        }
    }

    async fn save_manual_config(&self, _url: String, _token: String) {
        // This method is deprecated - manual config should use SourceCoordinator
        error!("save_manual_config is deprecated - use SourceCoordinator instead");
        self.close();
    }

    async fn save_jellyfin_config(&self, _url: String, _username: String, _password: String) {
        // This method is deprecated - Jellyfin config should use SourceCoordinator
        error!("save_jellyfin_config is deprecated - use SourceCoordinator instead");
        self.close();
    }

    fn connect_jellyfin(&self, url: String, username: String, password: String) {
        info!("Connecting to Jellyfin server: {}", url);

        let imp = self.imp();

        // Show progress
        imp.jellyfin_progress.set_visible(true);
        imp.jellyfin_progress.pulse();

        // Start pulsing animation
        let progress = imp.jellyfin_progress.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            progress.pulse();
            glib::ControlFlow::Continue
        });

        let state = imp.state.borrow().as_ref().map(|s| s.clone());

        if let Some(state) = state {
            let dialog_weak = self.downgrade();

            glib::spawn_future_local(async move {
                // First create a temporary Jellyfin backend to authenticate and get credentials
                let temp_backend = crate::backends::jellyfin::JellyfinBackend::new();

                match temp_backend
                    .authenticate_with_credentials(&url, &username, &password)
                    .await
                {
                    Ok(()) => {
                        info!("Successfully authenticated with Jellyfin, getting credentials");

                        // Get the stored credentials from the backend
                        let (access_token, user_id) = temp_backend
                            .get_credentials()
                            .await
                            .unwrap_or_else(|| (String::new(), String::new()));

                        if !access_token.is_empty() && !user_id.is_empty() {
                            info!("Got credentials, adding through SourceCoordinator");

                            // Use SourceCoordinator to add the Jellyfin source
                            let source_coordinator = state.get_source_coordinator();
                            info!("Calling add_jellyfin_source...");
                            let result = source_coordinator
                                .add_jellyfin_source(
                                    &url,
                                    &username,
                                    &password,
                                    &access_token,
                                    &user_id,
                                )
                                .await;
                            info!("add_jellyfin_source returned: {:?}", result.is_ok());

                            match result {
                                Ok(source) => {
                                    info!("Successfully added Jellyfin source: {}", source.name);

                                    // Close dialog first to avoid blocking
                                    if let Some(dialog) = dialog_weak.upgrade() {
                                        info!("Closing auth dialog");
                                        dialog.close();
                                    }

                                    // No automatic backend switching or "active" backend setting
                                    info!("Jellyfin source added: {}", source.id);

                                    // Set the user
                                    info!("Setting user");
                                    let user = crate::models::User {
                                        id: user_id,
                                        username: username.clone(),
                                        email: None,
                                        avatar_url: None,
                                    };
                                    state.set_user(user).await;
                                    info!("Jellyfin setup complete");
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to add Jellyfin source through SourceCoordinator: {}",
                                        e
                                    );

                                    if let Some(dialog) = dialog_weak.upgrade() {
                                        let imp = dialog.imp();
                                        imp.jellyfin_progress.set_visible(false);
                                        imp.jellyfin_error.set_visible(true);
                                        imp.jellyfin_error.set_title("Connection Failed");
                                        imp.jellyfin_error.set_description(Some(&format!("{}", e)));
                                        imp.jellyfin_retry_button.set_visible(true);
                                        // Hide the input fields and connect button when showing error
                                        imp.jellyfin_url_entry.set_visible(false);
                                        imp.jellyfin_username_entry.set_visible(false);
                                        imp.jellyfin_password_entry.set_visible(false);
                                    }
                                }
                            }
                        } else {
                            error!("Failed to get credentials from Jellyfin backend");

                            if let Some(dialog) = dialog_weak.upgrade() {
                                let imp = dialog.imp();
                                imp.jellyfin_progress.set_visible(false);
                                imp.jellyfin_error.set_visible(true);
                                imp.jellyfin_error.set_title("Authentication Failed");
                                imp.jellyfin_error
                                    .set_description(Some("Could not retrieve access token"));
                                imp.jellyfin_retry_button.set_visible(true);
                                // Hide the input fields and connect button when showing error
                                imp.jellyfin_url_entry.set_visible(false);
                                imp.jellyfin_username_entry.set_visible(false);
                                imp.jellyfin_password_entry.set_visible(false);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to authenticate with Jellyfin: {}", e);

                        if let Some(dialog) = dialog_weak.upgrade() {
                            let imp = dialog.imp();
                            imp.jellyfin_progress.set_visible(false);
                            imp.jellyfin_error.set_visible(true);
                            imp.jellyfin_error.set_title("Authentication Failed");
                            imp.jellyfin_error.set_description(Some(&format!("{}", e)));
                            imp.jellyfin_retry_button.set_visible(true);
                            // Hide the input fields and connect button when showing error
                            imp.jellyfin_url_entry.set_visible(false);
                            imp.jellyfin_username_entry.set_visible(false);
                            imp.jellyfin_password_entry.set_visible(false);
                        }
                    }
                }
            });
        }
    }
}
