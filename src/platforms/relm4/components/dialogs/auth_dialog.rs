use gtk4::{glib, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use relm4::component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender};
use tracing::info;

use crate::backends::MediaBackend;
use crate::backends::jellyfin::JellyfinBackend;
use crate::backends::plex::{PlexAuth, PlexBackend, PlexPin};
use crate::db::connection::DatabaseConnection;
use crate::models::{Credentials, Source, SourceId};
use crate::services::commands::Command;
use crate::services::commands::auth_commands::CreateSourceCommand;

#[derive(Debug, Clone)]
pub enum BackendType {
    Plex,
    Jellyfin,
}

#[derive(Debug, Clone)]
pub enum AuthDialogInput {
    Show,
    Hide,
    SelectBackend(BackendType),
    // Plex inputs
    StartPlexAuth,
    PlexPinReceived(PlexPin),
    PlexAuthError(String),
    PlexTokenReceived(String),
    SourceCreated(SourceId),
    CancelPlexAuth,
    RetryPlexAuth,
    OpenPlexLink,
    // Jellyfin inputs
    ConnectJellyfin,
    JellyfinAuthError(String),
    RetryJellyfin,
    // Manual Plex inputs
    ConnectManualPlex,
}

#[derive(Debug, Clone)]
pub enum AuthDialogOutput {
    SourceAdded(SourceId),
    Cancelled,
}

#[derive(Debug)]
pub struct AuthDialog {
    db: DatabaseConnection,
    backend_type: BackendType,

    // UI state
    is_visible: bool,

    // Plex OAuth state
    plex_pin: Option<PlexPin>,
    plex_auth_in_progress: bool,
    plex_auth_success: bool,
    plex_auth_error: Option<String>,

    // Jellyfin state
    jellyfin_url: String,
    jellyfin_username: String,
    jellyfin_password: String,
    jellyfin_auth_in_progress: bool,
    jellyfin_auth_success: bool,
    jellyfin_auth_error: Option<String>,

    // Manual Plex state
    plex_server_url: String,
    plex_token: String,

    // Widgets
    dialog: adw::Dialog,
    view_stack: adw::ViewStack,

    // Plex widgets
    pin_label: gtk4::Label,
    auth_progress: gtk4::ProgressBar,
    auth_status: adw::StatusPage,
    auth_error: adw::StatusPage,

    // Jellyfin widgets
    jellyfin_url_entry: adw::EntryRow,
    jellyfin_username_entry: adw::EntryRow,
    jellyfin_password_entry: adw::PasswordEntryRow,
    jellyfin_progress: gtk4::ProgressBar,
    jellyfin_success: adw::StatusPage,
    jellyfin_error: adw::StatusPage,

    // Manual Plex widgets
    server_url_entry: adw::EntryRow,
    token_entry: adw::PasswordEntryRow,
}

#[relm4::component(pub async)]
impl AsyncComponent for AuthDialog {
    type Input = AuthDialogInput;
    type Output = AuthDialogOutput;
    type Init = DatabaseConnection;
    type CommandOutput = ();

    view! {
        #[root]
        #[name = "dialog"]
        adw::Dialog {
            set_title: "Add Media Source",
            set_content_width: 500,
            set_content_height: 600,
            #[wrap(Some)]
            set_child = &gtk4::Box {
                set_orientation: gtk4::Orientation::Vertical,

                // Header with backend selector
                adw::HeaderBar {
                    set_show_end_title_buttons: false,

                    pack_start = &gtk4::Button {
                        set_label: "Cancel",
                        connect_clicked => AuthDialogInput::Hide,
                    },

                    #[wrap(Some)]
                    set_title_widget = &adw::ViewSwitcher {
                        set_stack: Some(&view_stack),
                        set_policy: adw::ViewSwitcherPolicy::Wide,
                    },
                },

                // Content stack
                #[name = "view_stack"]
                adw::ViewStack {
                    set_vexpand: true,

                    // Plex OAuth page
                    add_titled[Some("plex"), "Plex"] = &gtk4::Box {
                        set_orientation: gtk4::Orientation::Vertical,
                        set_spacing: 12,
                        set_margin_top: 12,
                        set_margin_bottom: 12,
                        set_margin_start: 12,
                        set_margin_end: 12,

                        // Initial state - start button
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Vertical,
                            set_spacing: 24,
                            set_valign: gtk4::Align::Center,
                            set_vexpand: true,
                            #[watch]
                            set_visible: !model.plex_auth_in_progress && !model.plex_auth_success && model.plex_auth_error.is_none(),

                            adw::StatusPage {
                                set_icon_name: Some("network-server-symbolic"),
                                set_title: "Connect to Plex",
                                set_description: Some("Authenticate with your Plex account to access your media libraries"),
                            },

                            gtk4::Box {
                                set_orientation: gtk4::Orientation::Horizontal,
                                set_halign: gtk4::Align::Center,
                                set_spacing: 12,

                                gtk4::Button {
                                    set_label: "Sign in with Plex",
                                    add_css_class: "suggested-action",
                                    add_css_class: "pill",
                                    connect_clicked => AuthDialogInput::StartPlexAuth,
                                },

                                gtk4::Button {
                                    set_label: "Manual Setup",
                                    add_css_class: "pill",
                                    connect_clicked => AuthDialogInput::SelectBackend(BackendType::Plex),
                                },
                            },
                        },

                        // PIN display state
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Vertical,
                            set_spacing: 24,
                            set_valign: gtk4::Align::Center,
                            set_vexpand: true,
                            #[watch]
                            set_visible: model.plex_auth_in_progress,

                            adw::StatusPage {
                                set_icon_name: Some("dialog-password-symbolic"),
                                set_title: "Enter this PIN on Plex.tv",
                                #[wrap(Some)]
            set_child = &gtk4::Box {
                                    set_orientation: gtk4::Orientation::Vertical,
                                    set_spacing: 12,

                                    #[name = "pin_label"]
                                    gtk4::Label {
                                        add_css_class: "title-1",
                                        #[watch]
                                        set_label: &model.plex_pin.as_ref().map(|p| p.code.clone()).unwrap_or_default(),
                                    },

                                    gtk4::Button {
                                        set_label: "Open plex.tv/link",
                                        add_css_class: "suggested-action",
                                        add_css_class: "pill",
                                        connect_clicked => AuthDialogInput::OpenPlexLink,
                                    },

                                    #[name = "auth_progress"]
                                    gtk4::ProgressBar {
                                        set_margin_top: 24,
                                        #[watch]
                                        set_pulse_step: if model.plex_auth_in_progress { 0.1 } else { 0.0 },
                                    },

                                    gtk4::Button {
                                        set_label: "Cancel",
                                        set_margin_top: 12,
                                        connect_clicked => AuthDialogInput::CancelPlexAuth,
                                    },
                                },
                            },
                        },

                        // Success state
                        #[name = "auth_status"]
                        adw::StatusPage {
                            set_icon_name: Some("emblem-ok-symbolic"),
                            set_title: "Connected Successfully",
                            set_description: Some("Your Plex account has been connected"),
                            #[watch]
                            set_visible: model.plex_auth_success,
                        },

                        // Error state
                        #[name = "auth_error"]
                        adw::StatusPage {
                            set_icon_name: Some("dialog-error-symbolic"),
                            set_title: "Connection Failed",
                            #[watch]
                            set_description: model.plex_auth_error.as_deref(),
                            #[watch]
                            set_visible: model.plex_auth_error.is_some(),
                            #[wrap(Some)]
            set_child = &gtk4::Button {
                                set_label: "Try Again",
                                set_halign: gtk4::Align::Center,
                                add_css_class: "pill",
                                connect_clicked => AuthDialogInput::RetryPlexAuth,
                            },
                        },
                    },

                    // Jellyfin page
                    add_titled[Some("jellyfin"), "Jellyfin"] = &gtk4::Box {
                        set_orientation: gtk4::Orientation::Vertical,
                        set_spacing: 12,
                        set_margin_top: 12,
                        set_margin_bottom: 12,
                        set_margin_start: 12,
                        set_margin_end: 12,

                        // Login form
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Vertical,
                            set_spacing: 12,
                            #[watch]
                            set_visible: !model.jellyfin_auth_in_progress && !model.jellyfin_auth_success && model.jellyfin_auth_error.is_none(),

                            adw::PreferencesGroup {
                                set_title: "Jellyfin Server",

                                #[name = "jellyfin_url_entry"]
                                add = &adw::EntryRow {
                                    set_title: "Server URL",
                                    set_text: &model.jellyfin_url,
                                    connect_changed[sender] => move |entry| {
                                        let text = entry.text().to_string();
                                        sender.input(AuthDialogInput::ConnectJellyfin);
                                    },
                                },

                                #[name = "jellyfin_username_entry"]
                                add = &adw::EntryRow {
                                    set_title: "Username",
                                    set_text: &model.jellyfin_username,
                                },

                                #[name = "jellyfin_password_entry"]
                                add = &adw::PasswordEntryRow {
                                    set_title: "Password",
                                    set_text: &model.jellyfin_password,
                                },
                            },

                            gtk4::Button {
                                set_label: "Connect",
                                set_halign: gtk4::Align::Center,
                                add_css_class: "suggested-action",
                                add_css_class: "pill",
                                connect_clicked => AuthDialogInput::ConnectJellyfin,
                            },
                        },

                        // Progress state
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Vertical,
                            set_spacing: 24,
                            set_valign: gtk4::Align::Center,
                            set_vexpand: true,
                            #[watch]
                            set_visible: model.jellyfin_auth_in_progress,

                            adw::StatusPage {
                                set_icon_name: Some("network-transmit-receive-symbolic"),
                                set_title: "Connecting...",
                                set_description: Some("Authenticating with Jellyfin server"),
                            },

                            #[name = "jellyfin_progress"]
                            gtk4::ProgressBar {
                                #[watch]
                                set_pulse_step: if model.jellyfin_auth_in_progress { 0.1 } else { 0.0 },
                            },
                        },

                        // Success state
                        #[name = "jellyfin_success"]
                        adw::StatusPage {
                            set_icon_name: Some("emblem-ok-symbolic"),
                            set_title: "Connected Successfully",
                            set_description: Some("Your Jellyfin server has been connected"),
                            #[watch]
                            set_visible: model.jellyfin_auth_success,
                        },

                        // Error state
                        #[name = "jellyfin_error"]
                        adw::StatusPage {
                            set_icon_name: Some("dialog-error-symbolic"),
                            set_title: "Connection Failed",
                            #[watch]
                            set_description: model.jellyfin_auth_error.as_deref(),
                            #[watch]
                            set_visible: model.jellyfin_auth_error.is_some(),
                            #[wrap(Some)]
            set_child = &gtk4::Button {
                                set_label: "Try Again",
                                set_halign: gtk4::Align::Center,
                                add_css_class: "pill",
                                connect_clicked => AuthDialogInput::RetryJellyfin,
                            },
                        },
                    },

                    // Manual Plex page
                    add_titled[Some("plex-manual"), "Plex (Manual)"] = &gtk4::Box {
                        set_orientation: gtk4::Orientation::Vertical,
                        set_spacing: 12,
                        set_margin_top: 12,
                        set_margin_bottom: 12,
                        set_margin_start: 12,
                        set_margin_end: 12,

                        adw::PreferencesGroup {
                            set_title: "Manual Plex Setup",
                            set_description: Some("Connect using server URL and authentication token"),

                            #[name = "server_url_entry"]
                            add = &adw::EntryRow {
                                set_title: "Server URL",
                                set_text: &model.plex_server_url,
                            },

                            #[name = "token_entry"]
                            add = &adw::PasswordEntryRow {
                                set_title: "Auth Token",
                                set_text: &model.plex_token,
                            },
                        },

                        gtk4::Label {
                            set_text: "You can find your auth token at plex.tv/api/v2/user",
                            set_wrap: true,
                            add_css_class: "dim-label",
                        },

                        gtk4::Button {
                            set_label: "Connect",
                            set_halign: gtk4::Align::Center,
                            add_css_class: "suggested-action",
                            add_css_class: "pill",
                            connect_clicked => AuthDialogInput::ConnectManualPlex,
                        },
                    },
                },
            },
        }
    }

    async fn init(
        db: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let mut model = AuthDialog {
            db,
            backend_type: BackendType::Plex,
            is_visible: false,

            // Plex OAuth state
            plex_pin: None,
            plex_auth_in_progress: false,
            plex_auth_success: false,
            plex_auth_error: None,

            // Jellyfin state
            jellyfin_url: String::new(),
            jellyfin_username: String::new(),
            jellyfin_password: String::new(),
            jellyfin_auth_in_progress: false,
            jellyfin_auth_success: false,
            jellyfin_auth_error: None,

            // Manual Plex state
            plex_server_url: String::new(),
            plex_token: String::new(),

            // Initialize widgets (will be set by view! macro)
            dialog: adw::Dialog::new(),
            view_stack: adw::ViewStack::new(),
            pin_label: gtk4::Label::new(None),
            auth_progress: gtk4::ProgressBar::new(),
            auth_status: adw::StatusPage::new(),
            auth_error: adw::StatusPage::new(),
            jellyfin_url_entry: adw::EntryRow::new(),
            jellyfin_username_entry: adw::EntryRow::new(),
            jellyfin_password_entry: adw::PasswordEntryRow::new(),
            jellyfin_progress: gtk4::ProgressBar::new(),
            jellyfin_success: adw::StatusPage::new(),
            jellyfin_error: adw::StatusPage::new(),
            server_url_entry: adw::EntryRow::new(),
            token_entry: adw::PasswordEntryRow::new(),
        };

        let widgets = view_output!();

        // Store reference to dialog for later use
        model.dialog = widgets.dialog.clone();

        // Start progress bar pulse animations
        glib::timeout_add_local(std::time::Duration::from_millis(100), {
            let auth_progress = model.auth_progress.clone();
            let jellyfin_progress = model.jellyfin_progress.clone();
            move || {
                auth_progress.pulse();
                jellyfin_progress.pulse();
                glib::ControlFlow::Continue
            }
        });

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            AuthDialogInput::Show => {
                info!("Showing auth dialog");
                self.is_visible = true;

                // Try to get the application and window
                if let Some(app) = relm4::main_application().downcast_ref::<adw::Application>() {
                    info!("Got application");
                    if let Some(window) = app.active_window() {
                        info!("Got active window, presenting dialog");
                        self.dialog.present(Some(&window));
                    } else {
                        info!("No active window found, presenting without parent");
                        self.dialog.present(None::<&gtk4::Window>);
                    }
                } else {
                    info!("Could not get application, presenting without parent");
                    self.dialog.present(None::<&gtk4::Window>);
                }
            }

            AuthDialogInput::Hide => {
                info!("Hiding auth dialog");
                self.is_visible = false;
                self.dialog.close();
                sender.output(AuthDialogOutput::Cancelled).unwrap();
            }

            AuthDialogInput::SelectBackend(backend_type) => {
                info!("Selected backend: {:?}", backend_type);
                self.backend_type = backend_type;
                match self.backend_type {
                    BackendType::Plex => self.view_stack.set_visible_child_name("plex"),
                    BackendType::Jellyfin => self.view_stack.set_visible_child_name("jellyfin"),
                }
            }

            AuthDialogInput::StartPlexAuth => {
                info!("Starting Plex OAuth flow");
                self.plex_auth_in_progress = true;
                self.plex_auth_error = None;
                self.plex_auth_success = false;

                // Get PIN from Plex auth service
                let sender_clone = sender.clone();
                sender.oneshot_command(async move {
                    match PlexAuth::get_pin().await {
                        Ok(pin) => {
                            info!("Got Plex PIN: {}", pin.code);
                            sender_clone.input(AuthDialogInput::PlexPinReceived(pin));
                        }
                        Err(e) => {
                            let error_msg = format!("Failed to get Plex PIN: {}", e);
                            sender_clone.input(AuthDialogInput::PlexAuthError(error_msg));
                        }
                    }
                });
            }

            AuthDialogInput::PlexPinReceived(pin) => {
                info!("Received Plex PIN: {}", pin.code);
                self.plex_pin = Some(pin.clone());

                // Start polling for token
                let pin_id = pin.id.clone();
                let sender_clone = sender.clone();
                sender.oneshot_command(async move {
                    // PlexAuth::poll_for_token handles the polling internally
                    match PlexAuth::poll_for_token(&pin_id, None).await {
                        Ok(token) => {
                            info!("Got Plex auth token!");
                            sender_clone.input(AuthDialogInput::PlexTokenReceived(token));
                        }
                        Err(e) => {
                            let error_msg = format!("Failed to authenticate: {}", e);
                            sender_clone.input(AuthDialogInput::PlexAuthError(error_msg));
                        }
                    }
                });
            }

            AuthDialogInput::PlexAuthError(error) => {
                info!("Plex auth error: {}", error);
                self.plex_auth_in_progress = false;
                self.plex_auth_error = Some(error);
                self.plex_pin = None;
            }

            AuthDialogInput::PlexTokenReceived(token) => {
                info!("Successfully received Plex token");
                self.plex_auth_in_progress = false;
                self.plex_auth_success = true;

                // Create the source in the database
                let db = self.db.clone();
                let server_url = if self.plex_server_url.is_empty() {
                    None
                } else {
                    Some(self.plex_server_url.clone())
                };

                let sender_clone = sender.clone();
                sender.oneshot_command(async move {
                    use crate::backends::plex::{PlexBackend, PlexServer};
                    use crate::services::core::auth::AuthService;

                    // Discover available servers
                    let servers = match PlexAuth::discover_servers(&token).await {
                        Ok(servers) => servers,
                        Err(e) => {
                            sender_clone.input(AuthDialogInput::PlexAuthError(format!(
                                "Failed to discover servers: {}",
                                e
                            )));
                            return;
                        }
                    };

                    if servers.is_empty() {
                        sender_clone.input(AuthDialogInput::PlexAuthError(
                            "No Plex servers found".to_string(),
                        ));
                        return;
                    }

                    // Check if we have a manual URL
                    let is_manual = server_url.is_some();

                    // Select the best server URL
                    let selected_server_url = if let Some(manual_url) = server_url {
                        // User provided a manual URL
                        manual_url
                    } else {
                        // Find the best connection from discovered servers
                        // Priority: owned servers > home servers > shared servers
                        let best_server = servers
                            .iter()
                            .find(|s| s.owned)
                            .or_else(|| servers.iter().find(|s| s.home))
                            .unwrap_or(&servers[0]);

                        // Find the best connection for this server
                        // Priority: local connections > remote direct > relay
                        let best_connection = best_server
                            .connections
                            .iter()
                            .find(|c| c.local && !c.relay)
                            .or_else(|| best_server.connections.iter().find(|c| !c.relay))
                            .or_else(|| best_server.connections.first());

                        match best_connection {
                            Some(conn) => conn.uri.clone(),
                            None => {
                                sender_clone.input(AuthDialogInput::PlexAuthError(format!(
                                    "No connections available for server '{}'",
                                    best_server.name
                                )));
                                return;
                            }
                        }
                    };

                    // Get the best server info for creating the source
                    let best_server = if is_manual {
                        // For manual entry, we don't have server details
                        None
                    } else {
                        // Find the best server from discovered servers
                        servers
                            .iter()
                            .find(|s| s.owned)
                            .or_else(|| servers.iter().find(|s| s.home))
                            .or_else(|| servers.first())
                    };

                    // Create credentials
                    let credentials = Credentials::Token {
                        token: token.clone(),
                    };

                    // Create PlexBackend for authentication
                    let plex_backend =
                        PlexBackend::new_for_auth(selected_server_url.clone(), token.clone());

                    // Create the source using CreateSourceCommand (which handles auth_provider_id)
                    let command = CreateSourceCommand {
                        db: db.clone(),
                        backend: &plex_backend as &dyn MediaBackend,
                        source_type: "plex".to_string(),
                        name: best_server
                            .map(|s| s.name.clone())
                            .unwrap_or_else(|| "Plex".to_string()),
                        credentials,
                        server_url: Some(selected_server_url),
                    };

                    match command.execute().await {
                        Ok(source) => {
                            info!("Successfully created Plex source: {}", source.id);

                            // Now update the source with additional Plex-specific metadata
                            use crate::db::repository::Repository;
                            use crate::db::repository::source_repository::SourceRepositoryImpl;
                            use crate::models::ServerConnections;
                            use crate::models::SourceId;
                            use crate::services::core::ConnectionService;

                            let repo = SourceRepositoryImpl::new(db.clone());

                            // Get the created source
                            if let Ok(Some(mut source_model)) =
                                Repository::find_by_id(&repo, &source.id).await
                            {
                                // Update with Plex-specific fields
                                if let Some(server) = best_server {
                                    source_model.machine_id =
                                        Some(server.client_identifier.clone());
                                    source_model.is_owned = server.owned;

                                    // Convert all discovered connections to our ServerConnection model
                                    let connections = ConnectionService::from_plex_connections(
                                        server.connections.clone(),
                                    );
                                    let server_connections = ServerConnections::new(connections);
                                    source_model.connections = Some(
                                        serde_json::to_value(&server_connections)
                                            .unwrap_or(serde_json::Value::Null),
                                    );
                                }

                                // Update the source with additional metadata
                                if let Err(e) = Repository::update(&repo, source_model).await {
                                    // Log error but don't fail - the source was created successfully
                                    info!(
                                        "Warning: Failed to update source with Plex metadata: {}",
                                        e
                                    );
                                }
                            }

                            sender_clone
                                .input(AuthDialogInput::SourceCreated(SourceId::new(source.id)));
                        }
                        Err(e) => {
                            sender_clone.input(AuthDialogInput::PlexAuthError(format!(
                                "Failed to create source: {}",
                                e
                            )));
                        }
                    }
                });
            }

            AuthDialogInput::SourceCreated(source_id) => {
                info!("Source created successfully: {:?}", source_id);
                self.dialog.close();
                sender
                    .output(AuthDialogOutput::SourceAdded(source_id))
                    .unwrap();
            }

            AuthDialogInput::CancelPlexAuth => {
                info!("Cancelling Plex auth");
                self.plex_auth_in_progress = false;
                self.plex_pin = None;
            }

            AuthDialogInput::RetryPlexAuth => {
                info!("Retrying Plex auth");
                self.plex_auth_error = None;
                self.plex_auth_success = false;
                sender.input(AuthDialogInput::StartPlexAuth);
            }

            AuthDialogInput::OpenPlexLink => {
                info!("Opening Plex link");
                let _ = gtk4::gio::AppInfo::launch_default_for_uri(
                    "https://plex.tv/link",
                    None::<&gtk4::gio::AppLaunchContext>,
                );
            }

            AuthDialogInput::ConnectJellyfin => {
                info!("Connecting to Jellyfin");
                self.jellyfin_url = self.jellyfin_url_entry.text().to_string();
                self.jellyfin_username = self.jellyfin_username_entry.text().to_string();
                self.jellyfin_password = self.jellyfin_password_entry.text().to_string();

                if self.jellyfin_url.is_empty()
                    || self.jellyfin_username.is_empty()
                    || self.jellyfin_password.is_empty()
                {
                    self.jellyfin_auth_error = Some("Please fill in all fields".to_string());
                    return;
                }

                self.jellyfin_auth_in_progress = true;
                self.jellyfin_auth_error = None;

                // Authenticate with Jellyfin
                let url = self.jellyfin_url.clone();
                let username = self.jellyfin_username.clone();
                let password = self.jellyfin_password.clone();
                let db = self.db.clone();
                let sender_clone = sender.clone();

                sender.oneshot_command(async move {
                    // Create Jellyfin backend
                    let jellyfin_client = JellyfinBackend::new();

                    // Authenticate directly with credentials and URL
                    match jellyfin_client
                        .authenticate_with_credentials(&url, &username, &password)
                        .await
                    {
                        Ok(_) => {
                            info!("Jellyfin authentication successful for user: {}", username);

                            // Create credentials for storage
                            let credentials = Credentials::UsernamePassword {
                                username: username.clone(),
                                password: password.clone(),
                            };

                            // Create the source using CreateSourceCommand
                            let command = CreateSourceCommand {
                                db,
                                backend: &jellyfin_client as &dyn MediaBackend,
                                source_type: "jellyfin".to_string(),
                                name: format!("Jellyfin - {}", username),
                                credentials,
                                server_url: Some(url),
                            };

                            match command.execute().await {
                                Ok(source) => {
                                    info!("Created Jellyfin source: {}", source.id);
                                    let source_id = SourceId::new(source.id);
                                    sender_clone.input(AuthDialogInput::SourceCreated(source_id));
                                }
                                Err(e) => {
                                    let error_msg = format!("Failed to create source: {}", e);
                                    sender_clone
                                        .input(AuthDialogInput::JellyfinAuthError(error_msg));
                                }
                            }
                        }
                        Err(e) => {
                            let error_msg = format!("Authentication failed: {}", e);
                            sender_clone.input(AuthDialogInput::JellyfinAuthError(error_msg));
                        }
                    }
                });
            }

            AuthDialogInput::JellyfinAuthError(error) => {
                info!("Jellyfin auth error: {}", error);
                self.jellyfin_auth_error = Some(error);
                self.jellyfin_auth_in_progress = false;
                self.jellyfin_auth_success = false;
            }

            AuthDialogInput::RetryJellyfin => {
                info!("Retrying Jellyfin auth");
                self.jellyfin_auth_error = None;
                self.jellyfin_auth_success = false;
            }

            AuthDialogInput::ConnectManualPlex => {
                info!("Connecting with manual Plex credentials");
                self.plex_server_url = self.server_url_entry.text().to_string();
                self.plex_token = self.token_entry.text().to_string();

                if self.plex_server_url.is_empty() || self.plex_token.is_empty() {
                    self.plex_auth_error = Some("Please fill in all fields".to_string());
                    return;
                }

                // Mark as successful since user provided token directly
                self.plex_auth_in_progress = false;
                self.plex_auth_success = true;

                // Store the token to create source later
                sender.input(AuthDialogInput::PlexTokenReceived(self.plex_token.clone()));
            }
        }
    }
}
