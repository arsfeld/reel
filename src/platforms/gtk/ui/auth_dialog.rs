use gtk4::{glib, glib::clone, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::sync::Arc;
use tracing::{error, info};

use crate::core::viewmodels::authentication_view_model::{
    AuthenticationError, AuthenticationState, BackendAuthProgress, JellyfinAuthProgress,
    PlexAuthProgress,
};
use crate::core::viewmodels::{AuthenticationViewModel, ViewModel};
use crate::state::AppState;

pub use imp::BackendType;

mod imp {
    use super::*;
    use gtk4::CompositeTemplate;

    #[derive(Default, CompositeTemplate)]
    #[template(resource = "/dev/arsfeld/Reel/auth_dialog.ui")]
    pub struct ReelAuthDialog {
        #[template_child]
        pub cancel_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub save_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub view_stack: TemplateChild<adw::ViewStack>,

        // Plex elements
        #[template_child]
        pub pin_status_page: TemplateChild<gtk4::Box>,
        #[template_child]
        pub pin_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub auth_progress: TemplateChild<gtk4::ProgressBar>,
        #[template_child]
        pub auth_status: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub success_checkmark: TemplateChild<gtk4::Image>,
        #[template_child]
        pub auth_error: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub retry_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub open_link_button: TemplateChild<gtk4::Button>,

        // Plex manual elements
        #[template_child]
        pub server_url_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub token_entry: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub manual_connect_button: TemplateChild<gtk4::Button>,

        // Jellyfin elements
        #[template_child]
        pub jellyfin_url_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub jellyfin_username_entry: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub jellyfin_password_entry: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub jellyfin_progress: TemplateChild<gtk4::ProgressBar>,
        #[template_child]
        pub jellyfin_success: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub jellyfin_success_checkmark: TemplateChild<gtk4::Image>,
        #[template_child]
        pub jellyfin_error: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub jellyfin_retry_button: TemplateChild<gtk4::Button>,

        // Reactive state
        pub state: RefCell<Option<Arc<AppState>>>,
        pub auth_view_model: RefCell<Option<Arc<AuthenticationViewModel>>>,
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

            // Setup button connections
            self.setup_button_handlers(&obj);
            self.setup_entry_handlers(&obj);
        }

        fn dispose(&self) {
            // Cancel any ongoing authentication when dialog is disposed
            if let Some(vm) = self.auth_view_model.borrow().as_ref() {
                let vm_clone = vm.clone();
                glib::spawn_future_local(async move {
                    let _ = vm_clone.cancel_authentication().await;
                });
            }
        }
    }

    impl ReelAuthDialog {
        fn setup_button_handlers(&self, obj: &super::ReelAuthDialog) {
            // Cancel button
            self.cancel_button.connect_clicked(clone!(
                #[weak]
                obj,
                move |_| {
                    info!("Cancel button clicked");
                    obj.close();
                }
            ));

            // Open Plex link button
            self.open_link_button.connect_clicked(|_| {
                info!("Opening plex.tv/link");
                if let Err(e) = gtk4::gio::AppInfo::launch_default_for_uri(
                    "https://plex.tv/link",
                    None::<&gtk4::gio::AppLaunchContext>,
                ) {
                    error!("Failed to open browser: {}", e);
                }
            });

            // Retry buttons
            self.retry_button.connect_clicked(clone!(
                #[weak]
                obj,
                move |_| {
                    obj.retry_authentication();
                }
            ));

            self.jellyfin_retry_button.connect_clicked(clone!(
                #[weak]
                obj,
                move |_| {
                    obj.retry_authentication();
                }
            ));

            // Save/Connect button
            self.save_button.connect_clicked(clone!(
                #[weak]
                obj,
                move |_| {
                    obj.start_authentication();
                }
            ));

            // Manual connect button
            self.manual_connect_button.connect_clicked(clone!(
                #[weak]
                obj,
                move |_| {
                    obj.start_manual_connection();
                }
            ));
        }

        fn setup_entry_handlers(&self, obj: &super::ReelAuthDialog) {
            let update_buttons = clone!(
                #[weak]
                obj,
                move || {
                    obj.update_button_states();
                }
            );

            // Set placeholder text
            self.server_url_entry.set_text("http://192.168.1.100:32400");

            // Connect entry change handlers
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

        // Create AuthenticationViewModel
        let auth_manager = state.get_source_coordinator().get_auth_manager().clone();
        let auth_vm = Arc::new(AuthenticationViewModel::new(
            auth_manager,
            state.data_service.clone(),
        ));

        dialog.imp().state.replace(Some(state.clone()));
        dialog.imp().auth_view_model.replace(Some(auth_vm.clone()));

        // Initialize ViewModel with EventBus
        glib::spawn_future_local({
            let vm = auth_vm.clone();
            let event_bus = state.event_bus.clone();
            async move {
                vm.initialize(event_bus).await;
            }
        });

        // Setup reactive bindings
        dialog.setup_reactive_bindings(auth_vm);

        dialog
    }

    pub fn set_backend_type(&self, backend_type: BackendType) {
        let imp = self.imp();
        imp.backend_type.replace(backend_type);

        // Update the title and visible stack page
        match backend_type {
            BackendType::Plex => {
                self.set_title("Connect to Plex");
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

        self.update_button_states();
    }

    fn setup_reactive_bindings(&self, auth_vm: Arc<AuthenticationViewModel>) {
        let weak_self = self.downgrade();

        // Subscribe to authentication state changes
        {
            let weak_self = weak_self.clone();
            let mut state_subscriber = auth_vm.authentication_state().subscribe();

            glib::spawn_future_local(async move {
                while state_subscriber.wait_for_change().await {
                    if let Some(dialog) = weak_self.upgrade()
                        && let Some(vm) = &*dialog.imp().auth_view_model.borrow()
                    {
                        let state = vm.authentication_state().get().await;
                        dialog.handle_authentication_state_change(state).await;
                    }
                }
            });
        }

        // Subscribe to progress changes
        {
            let weak_self = weak_self.clone();
            let mut progress_subscriber = auth_vm.progress().subscribe();

            glib::spawn_future_local(async move {
                while progress_subscriber.wait_for_change().await {
                    if let Some(dialog) = weak_self.upgrade()
                        && let Some(vm) = &*dialog.imp().auth_view_model.borrow()
                    {
                        let progress = vm.progress().get().await;
                        dialog.handle_progress_change(progress).await;
                    }
                }
            });
        }

        // Subscribe to error changes
        {
            let weak_self = weak_self.clone();
            let mut error_subscriber = auth_vm.error().subscribe();

            glib::spawn_future_local(async move {
                while error_subscriber.wait_for_change().await {
                    if let Some(dialog) = weak_self.upgrade()
                        && let Some(vm) = &*dialog.imp().auth_view_model.borrow()
                    {
                        let error = vm.error().get().await;
                        dialog.handle_error_change(error).await;
                    }
                }
            });
        }

        // Subscribe to PIN changes
        {
            let weak_self = weak_self.clone();
            let mut pin_subscriber = auth_vm.current_pin().subscribe();

            glib::spawn_future_local(async move {
                while pin_subscriber.wait_for_change().await {
                    if let Some(dialog) = weak_self.upgrade()
                        && let Some(vm) = &*dialog.imp().auth_view_model.borrow()
                    {
                        let pin = vm.current_pin().get().await;
                        dialog.handle_pin_change(pin).await;
                    }
                }
            });
        }

        // Subscribe to cancellable state
        {
            let weak_self = weak_self.clone();
            let mut cancellable_subscriber = auth_vm.is_cancellable().subscribe();

            glib::spawn_future_local(async move {
                while cancellable_subscriber.wait_for_change().await {
                    if let Some(dialog) = weak_self.upgrade()
                        && let Some(vm) = &*dialog.imp().auth_view_model.borrow()
                    {
                        let is_cancellable = vm.is_cancellable().get().await;
                        dialog.handle_cancellable_change(is_cancellable).await;
                    }
                }
            });
        }
    }

    async fn handle_authentication_state_change(&self, state: AuthenticationState) {
        let imp = self.imp();
        info!("Authentication state changed to: {:?}", state);

        match state {
            AuthenticationState::Idle => {
                // Reset UI to initial state
                imp.auth_progress.set_visible(false);
                imp.pin_status_page.set_visible(false);
                imp.auth_status.set_visible(false);
                imp.auth_error.set_visible(false);
                imp.jellyfin_progress.set_visible(false);
                imp.jellyfin_success.set_visible(false);
                imp.jellyfin_error.set_visible(false);
            }

            AuthenticationState::RequestingPin => {
                imp.auth_progress.set_visible(true);
                imp.auth_progress.pulse();
                imp.auth_status.set_visible(false);
                imp.auth_error.set_visible(false);
            }

            AuthenticationState::WaitingForUser {
                pin,
                elapsed_seconds,
            } => {
                imp.pin_label.set_text(&pin);
                imp.pin_status_page.set_visible(true);
                imp.auth_progress.set_visible(true);

                if elapsed_seconds > 10 {
                    imp.auth_progress.set_text(Some(
                        "Waiting for authorization... Make sure to visit plex.tv/link",
                    ));
                }
            }

            AuthenticationState::ValidatingToken => {
                // Keep the PIN visible during validation, just show progress
                imp.auth_progress.set_visible(true);
                imp.auth_progress.pulse();
                imp.auth_progress.set_text(Some("Validating token..."));
                // Don't hide the pin_status_page - user should still see the PIN
            }

            AuthenticationState::DiscoveringServers { found } => {
                imp.auth_status.set_title("Discovering Servers");
                imp.auth_status
                    .set_description(Some(&format!("Found {} servers", found)));
            }

            AuthenticationState::TestingConnections { current, total } => {
                imp.auth_status.set_title("Testing Connections");
                imp.auth_status.set_description(Some(&format!(
                    "Testing connection {} of {}",
                    current + 1,
                    total
                )));
            }

            AuthenticationState::CreatingAccount => {
                imp.auth_status.set_title("Creating Account");
                imp.auth_status
                    .set_description(Some("Setting up your account and servers..."));
            }

            AuthenticationState::LoadingUserInfo => {
                imp.auth_status.set_title("Loading User Information");
                imp.auth_status
                    .set_description(Some("Getting your profile details..."));
            }

            AuthenticationState::StartingSyncForSources { sources } => {
                imp.auth_status.set_title("Starting Sync");
                imp.auth_status.set_description(Some(&format!(
                    "Starting background sync for {} source{}",
                    sources.len(),
                    if sources.len() != 1 { "s" } else { "" }
                )));
            }

            AuthenticationState::Complete { sources, .. } => {
                imp.auth_progress.set_visible(false);
                imp.pin_status_page.set_visible(false);
                imp.auth_status.set_visible(true);
                imp.auth_status.set_icon_name(Some("emblem-ok-symbolic"));
                imp.auth_status.set_title("Setup Complete!");
                imp.auth_status.set_description(Some(&format!(
                    "Connected to {} server{}. Syncing libraries in background...",
                    sources.len(),
                    if sources.len() > 1 { "s" } else { "" }
                )));
                imp.success_checkmark.set_visible(true);
                imp.success_checkmark.add_css_class("success");

                // Close dialog after brief delay
                glib::timeout_future(std::time::Duration::from_millis(1500)).await;
                self.close();
            }

            AuthenticationState::Error { error, can_retry } => {
                imp.auth_progress.set_visible(false);
                imp.pin_status_page.set_visible(false);
                imp.auth_status.set_visible(false);

                // Show appropriate error page
                let backend_type = *imp.backend_type.borrow();
                match backend_type {
                    BackendType::Plex => {
                        imp.auth_error.set_visible(true);
                        imp.auth_error.set_title("Authentication Failed");
                        imp.auth_error.set_description(Some(&error));
                        imp.retry_button.set_visible(can_retry);
                    }
                    BackendType::Jellyfin => {
                        imp.jellyfin_error.set_visible(true);
                        imp.jellyfin_error.set_title("Authentication Failed");
                        imp.jellyfin_error.set_description(Some(&error));
                        imp.jellyfin_retry_button.set_visible(can_retry);
                    }
                }
            }
        }
    }

    async fn handle_progress_change(&self, progress: Option<BackendAuthProgress>) {
        if let Some(progress) = progress {
            info!("Progress changed to: {:?}", progress);

            match progress {
                BackendAuthProgress::Plex(plex_progress) => {
                    self.handle_plex_progress(plex_progress).await;
                }
                BackendAuthProgress::Jellyfin(jellyfin_progress) => {
                    self.handle_jellyfin_progress(jellyfin_progress).await;
                }
            }
        }
    }

    async fn handle_plex_progress(&self, progress: PlexAuthProgress) {
        let imp = self.imp();

        match progress {
            PlexAuthProgress::RequestingPin => {
                imp.auth_progress.pulse();
            }
            PlexAuthProgress::WaitingForUser {
                elapsed_seconds, ..
            } => {
                if elapsed_seconds > 10 {
                    imp.auth_progress.set_text(Some(
                        "Waiting for authorization... Make sure to visit plex.tv/link",
                    ));
                }
            }
            PlexAuthProgress::DiscoveringServers { found } => {
                imp.auth_status
                    .set_description(Some(&format!("Found {} servers", found)));
            }
            PlexAuthProgress::TestingConnections { current, total } => {
                imp.auth_status.set_description(Some(&format!(
                    "Testing connection {} of {}",
                    current + 1,
                    total
                )));
            }
            _ => {}
        }
    }

    async fn handle_jellyfin_progress(&self, progress: JellyfinAuthProgress) {
        let imp = self.imp();

        match progress {
            JellyfinAuthProgress::ValidatingCredentials => {
                imp.jellyfin_progress.set_visible(true);
                imp.jellyfin_progress.pulse();
            }
            JellyfinAuthProgress::TestingConnection => {
                // Progress already visible from ValidatingCredentials
            }
            JellyfinAuthProgress::CreatingAccount => {
                // Progress continues
            }
            JellyfinAuthProgress::Complete { .. } => {
                imp.jellyfin_progress.set_visible(false);
                imp.jellyfin_success.set_visible(true);
                imp.jellyfin_success.set_title("Setup Complete!");
                imp.jellyfin_success_checkmark.set_visible(true);
                imp.jellyfin_success_checkmark.add_css_class("success");

                // Hide input fields
                imp.jellyfin_url_entry.set_visible(false);
                imp.jellyfin_username_entry.set_visible(false);
                imp.jellyfin_password_entry.set_visible(false);
            }
        }
    }

    async fn handle_error_change(&self, error: Option<AuthenticationError>) {
        if let Some(error) = error {
            info!("Authentication error: {:?}", error);
            // Error handling is done in handle_authentication_state_change
        }
    }

    async fn handle_pin_change(&self, pin: Option<String>) {
        if let Some(pin) = pin {
            self.imp().pin_label.set_text(&pin);
        }
    }

    async fn handle_cancellable_change(&self, is_cancellable: bool) {
        // Could enable/disable cancel button based on state
        info!("Cancellable state: {}", is_cancellable);
    }

    pub fn start_authentication(&self) {
        let backend_type = *self.imp().backend_type.borrow();

        if let Some(vm) = &*self.imp().auth_view_model.borrow() {
            let vm_clone = vm.clone();

            match backend_type {
                BackendType::Plex => {
                    glib::spawn_future_local(async move {
                        if let Err(e) = vm_clone.start_plex_authentication().await {
                            error!("Failed to start Plex authentication: {}", e);
                        }
                    });
                }
                BackendType::Jellyfin => {
                    // Get credentials from the form
                    let imp = self.imp();
                    let url = imp.jellyfin_url_entry.text().to_string();
                    let username = imp.jellyfin_username_entry.text().to_string();
                    let password = imp.jellyfin_password_entry.text().to_string();

                    glib::spawn_future_local(async move {
                        if !url.is_empty() && !username.is_empty() && !password.is_empty() {
                            if let Err(e) = vm_clone
                                .start_jellyfin_authentication(url, username, password)
                                .await
                            {
                                error!("Failed to start Jellyfin authentication: {}", e);
                            }
                        }
                    });
                }
            }
        }
    }

    pub fn start_jellyfin_authentication_with_credentials(
        &self,
        url: String,
        username: String,
        password: String,
    ) {
        if let Some(vm) = &*self.imp().auth_view_model.borrow() {
            let vm_clone = vm.clone();

            glib::spawn_future_local(async move {
                if let Err(e) = vm_clone
                    .start_jellyfin_authentication(url, username, password)
                    .await
                {
                    error!("Failed to start Jellyfin authentication: {}", e);
                }
            });
        }
    }

    pub fn start_manual_connection(&self) {
        let imp = self.imp();
        let url = imp.server_url_entry.text().to_string();
        let token = imp.token_entry.text().to_string();

        if !url.is_empty() && !token.is_empty() {
            info!("Manual connection not yet implemented for reactive dialog");
            // TODO: Implement manual connection through ViewModel
            self.close();
        }
    }

    pub fn retry_authentication(&self) {
        if let Some(vm) = &*self.imp().auth_view_model.borrow() {
            let vm_clone = vm.clone();

            glib::spawn_future_local(async move {
                if let Err(e) = vm_clone.retry_authentication().await {
                    error!("Failed to retry authentication: {}", e);
                }
            });
        }
    }

    fn update_button_states(&self) {
        let imp = self.imp();
        let backend_type = *imp.backend_type.borrow();

        let save_enabled = match backend_type {
            BackendType::Plex => true, // Always enabled for automatic auth
            BackendType::Jellyfin => {
                let has_url = !imp.jellyfin_url_entry.text().is_empty();
                let has_username = !imp.jellyfin_username_entry.text().is_empty();
                let has_password = !imp.jellyfin_password_entry.text().is_empty();
                has_url && has_username && has_password
            }
        };

        imp.save_button.set_sensitive(save_enabled);

        // Update manual connect button for Plex
        if matches!(backend_type, BackendType::Plex) {
            let has_url = !imp.server_url_entry.text().is_empty();
            let has_token = !imp.token_entry.text().is_empty();
            imp.manual_connect_button
                .set_sensitive(has_url && has_token);
        }
    }
}
