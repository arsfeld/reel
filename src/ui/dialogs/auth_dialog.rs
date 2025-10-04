use gtk4::{glib, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use relm4::component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender};
use tracing::{error, info};

use crate::backends::MediaBackend;
use crate::backends::jellyfin::JellyfinBackend;
use crate::backends::plex::{PlexAuth, PlexHomeUser, PlexPin};
use crate::db::connection::DatabaseConnection;
use crate::models::{Credentials, SourceId};
use crate::services::commands::Command;
use crate::services::commands::auth_commands::CreateSourceCommand;

#[derive(Debug, Clone)]
pub enum BackendType {
    Plex,
}

#[derive(Debug, Clone)]
pub enum AuthDialogInput {
    Show,
    // Plex inputs
    StartPlexAuth,
    PlexPinReceived(PlexPin),
    PlexAuthError(String),
    PlexTokenReceived(String),
    PlexHomeUsersReceived(Vec<PlexHomeUser>),
    SelectPlexProfile(PlexHomeUser),
    PlexPinRequested(String),         // PIN from user input
    PlexProfileTokenReceived(String), // Token after switching profile
    SkipProfileSelection,             // For accounts without Home users
    SourceCreated(SourceId),
    CancelPlexAuth,
    RetryPlexAuth,
    OpenPlexLink,
    // Jellyfin inputs
    UpdateJellyfinUrl(String),
    UpdateJellyfinUsername(String),
    UpdateJellyfinPassword(String),
    ConnectJellyfin,
    JellyfinAuthError(String),
    RetryJellyfin,
    // Jellyfin Quick Connect inputs
    StartJellyfinQuickConnect,
    JellyfinQuickConnectInitiated { code: String, secret: String },
    JellyfinQuickConnectAuthenticated(String), // token
    JellyfinQuickConnectFailed(String),
    CancelJellyfinQuickConnect,
    CheckJellyfinQuickConnectStatus,
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
    parent_window: Option<gtk4::Window>,

    // UI state
    is_visible: bool,
    dialog_position: Option<(i32, i32)>,

    // Plex OAuth state
    plex_pin: Option<PlexPin>,
    plex_auth_in_progress: bool,
    plex_auth_success: bool,
    plex_auth_error: Option<String>,

    // Plex profile selection state
    plex_home_users: Vec<PlexHomeUser>,
    plex_selected_profile: Option<PlexHomeUser>,
    plex_profile_selection_active: bool,
    plex_primary_token: Option<String>, // Store primary token during profile selection
    plex_pin_input_active: bool,

    // Jellyfin state
    jellyfin_url: String,
    jellyfin_url_confirmed: bool,
    jellyfin_username: String,
    jellyfin_password: String,
    jellyfin_auth_in_progress: bool,
    jellyfin_auth_success: bool,
    jellyfin_auth_error: Option<String>,
    // Jellyfin Quick Connect state
    jellyfin_quick_connect_enabled: bool,
    jellyfin_quick_connect_in_progress: bool,
    jellyfin_quick_connect_code: Option<String>,
    jellyfin_quick_connect_secret: Option<String>,
    jellyfin_quick_connect_check_handle: Option<glib::SourceId>,

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
    jellyfin_quick_connect_code_label: gtk4::Label,
    jellyfin_quick_connect_progress: gtk4::ProgressBar,

    // Manual Plex widgets
    server_url_entry: adw::EntryRow,
    token_entry: adw::PasswordEntryRow,

    // Profile selection widgets
    profile_flowbox: gtk4::FlowBox,
}

impl AuthDialog {
    /// Helper method to proceed with Plex source creation after authentication
    fn proceed_with_plex_source_creation(
        &self,
        token: String,
        sender: AsyncComponentSender<AuthDialog>,
    ) {
        let db = self.db.clone();
        let server_url = if self.plex_server_url.is_empty() {
            None
        } else {
            Some(self.plex_server_url.clone())
        };

        let sender_clone = sender.clone();
        sender.oneshot_command(async move {
            use crate::backends::plex::PlexBackend;

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
                    "No Plex servers found. You can add a server manually or switch to a different profile.".to_string(),
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

            // For manual auth or when best_server is None, fetch machine_id
            let machine_id_to_use = if let Some(server) = best_server {
                Some(server.client_identifier.clone())
            } else if is_manual {
                // Fetch machine_id from the server for manual auth
                use crate::backends::plex::PlexApi;
                let api = PlexApi::with_backend_id(
                    selected_server_url.clone(),
                    token.clone(),
                    "temp".to_string(),
                );

                match api.get_machine_id().await {
                    Ok(machine_id) => {
                        info!("Fetched machine_id for manual auth: {}", machine_id);
                        Some(machine_id)
                    }
                    Err(e) => {
                        info!("Failed to fetch machine_id for manual auth: {}", e);
                        None
                    }
                }
            } else {
                None
            };

            // Create the source using CreateSourceCommand with machine_id
            let command = CreateSourceCommand {
                db: db.clone(),
                backend: &plex_backend as &dyn MediaBackend,
                source_type: "plex".to_string(),
                name: best_server
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| "Plex".to_string()),
                credentials,
                server_url: Some(selected_server_url),
                machine_id: machine_id_to_use,
                is_owned: best_server.map(|s| s.owned),
            };

            match command.execute().await {
                Ok(source) => {
                    info!(
                        "Successfully created Plex source: {} with machine_id",
                        source.id
                    );

                    // Update connections if we have them
                    if let Some(server) = best_server {
                        use crate::db::repository::source_repository::{
                            SourceRepository, SourceRepositoryImpl,
                        };
                        use crate::models::ServerConnections;
                        use crate::services::core::ConnectionService;

                        let repo = SourceRepositoryImpl::new(db.clone());

                        // Convert all discovered connections to our ServerConnection model
                        let connections =
                            ConnectionService::from_plex_connections(server.connections.clone());
                        let server_connections = ServerConnections::new(connections);

                        // Update just the connections
                        if let Err(e) = SourceRepository::update_connections(
                            &repo,
                            &source.id,
                            serde_json::to_value(&server_connections)
                                .unwrap_or(serde_json::Value::Null),
                        )
                        .await
                        {
                            info!("Warning: Failed to update source connections: {}", e);
                        }
                    }

                    sender_clone.input(AuthDialogInput::SourceCreated(SourceId::new(source.id)));
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
}

#[relm4::component(pub async)]
impl AsyncComponent for AuthDialog {
    type Input = AuthDialogInput;
    type Output = AuthDialogOutput;
    type Init = (DatabaseConnection, Option<gtk4::Window>);
    type CommandOutput = ();

    view! {
        #[root]
        #[name = "dialog"]
        adw::Dialog {
            set_title: "Add Media Source",
            set_content_width: 700,
            set_content_height: 650,
            set_follows_content_size: true,
            #[wrap(Some)]
            set_child = &adw::ToolbarView {
                // Top toolbar with proper draggable header
                add_top_bar = &adw::HeaderBar {
                    set_title_widget: Some(&gtk4::Label::new(Some("Add Media Source"))),
                    set_show_end_title_buttons: true,
                },

                // ViewSwitcherBar below the header for backend selection
                add_top_bar = &adw::ViewSwitcherBar {
                    set_stack: Some(&view_stack),
                    set_reveal: true,
                },

                #[wrap(Some)]
                set_content = &gtk4::Box {
                    set_orientation: gtk4::Orientation::Vertical,

                    // Content stack
                    #[name = "view_stack"]
                    adw::ViewStack {
                        set_vexpand: true,

                    // Plex page with OAuth and manual options
                    add_titled[Some("plex"), "Plex"] = &gtk4::Box {
                        set_orientation: gtk4::Orientation::Vertical,
                        set_spacing: 12,
                        set_margin_top: 12,
                        set_margin_bottom: 12,
                        set_margin_start: 12,
                        set_margin_end: 12,

                        // Initial state - clean and simple
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Vertical,
                            set_spacing: 24,
                            set_valign: gtk4::Align::Center,
                            set_vexpand: true,
                            #[watch]
                            set_visible: !model.plex_auth_in_progress && !model.plex_auth_success && !model.plex_profile_selection_active && !model.plex_pin_input_active && model.plex_auth_error.is_none(),

                            adw::StatusPage {
                                set_icon_name: Some("network-server-symbolic"),
                                set_title: "Connect Your Plex Account",
                                #[wrap(Some)]
                                set_child = &gtk4::Box {
                                    set_orientation: gtk4::Orientation::Vertical,
                                    set_spacing: 24,

                                    gtk4::Label {
                                        set_label: "Authenticate with Plex to access your media libraries",
                                        add_css_class: "dim-label",
                                        set_wrap: true,
                                        set_justify: gtk4::Justification::Center,
                                    },

                                    gtk4::Button {
                                        set_label: "Sign in with Plex",
                                        add_css_class: "suggested-action",
                                        add_css_class: "pill",
                                        set_halign: gtk4::Align::Center,
                                        connect_clicked => AuthDialogInput::StartPlexAuth,
                                    },
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

                        // Profile Selection state
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Vertical,
                            set_spacing: 24,
                            set_valign: gtk4::Align::Center,
                            set_vexpand: true,
                            #[watch]
                            set_visible: model.plex_profile_selection_active,

                            gtk4::Box {
                                set_orientation: gtk4::Orientation::Vertical,
                                set_spacing: 24,
                                set_margin_start: 48,
                                set_margin_end: 48,
                                set_halign: gtk4::Align::Center,

                                gtk4::Label {
                                    set_label: "Select Profile",
                                    add_css_class: "title-2",
                                    set_halign: gtk4::Align::Center,
                                },

                                gtk4::Label {
                                    set_label: "Choose which profile to use",
                                    add_css_class: "dim-label",
                                    set_halign: gtk4::Align::Center,
                                },

                                #[name = "profile_flowbox"]
                                gtk4::FlowBox {
                                    set_homogeneous: true,
                                    set_column_spacing: 16,
                                    set_row_spacing: 16,
                                    set_selection_mode: gtk4::SelectionMode::None,
                                    set_min_children_per_line: 2,
                                    set_max_children_per_line: 5,
                                    set_vexpand: false,
                                    set_hexpand: true,
                                    set_halign: gtk4::Align::Center,
                                    set_margin_top: 24,
                                    set_margin_bottom: 24,
                                },

                                gtk4::Button {
                                    set_label: "Use Primary Account",
                                    set_halign: gtk4::Align::Center,
                                    add_css_class: "pill",
                                    connect_clicked => AuthDialogInput::SkipProfileSelection,
                                },
                            },
                        },

                        // PIN Input Dialog for protected profiles
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Vertical,
                            set_spacing: 24,
                            set_valign: gtk4::Align::Center,
                            set_vexpand: true,
                            #[watch]
                            set_visible: model.plex_pin_input_active,

                            gtk4::Box {
                                set_orientation: gtk4::Orientation::Vertical,
                                set_spacing: 24,
                                set_margin_start: 48,
                                set_margin_end: 48,
                                set_halign: gtk4::Align::Center,
                                set_width_request: 400,

                                gtk4::Image {
                                    set_icon_name: Some("dialog-password-symbolic"),
                                    set_icon_size: gtk4::IconSize::Large,
                                    set_margin_bottom: 12,
                                    set_halign: gtk4::Align::Center,
                                },

                                gtk4::Label {
                                    set_label: "Enter Profile PIN",
                                    add_css_class: "title-2",
                                    set_halign: gtk4::Align::Center,
                                },

                                gtk4::Label {
                                    set_label: "Enter the PIN for this profile",
                                    add_css_class: "dim-label",
                                    set_halign: gtk4::Align::Center,
                                    set_wrap: true,
                                    set_justify: gtk4::Justification::Center,
                                },

                                adw::Clamp {
                                    set_maximum_size: 300,
                                    set_margin_top: 24,
                                    set_margin_bottom: 24,
                                    #[wrap(Some)]
                                    set_child = &adw::PasswordEntryRow {
                                        set_title: "PIN",
                                        set_show_apply_button: true,
                                        connect_apply[sender] => move |entry| {
                                            let pin = entry.text().to_string();
                                            sender.input(AuthDialogInput::PlexPinRequested(pin));
                                        },
                                    },
                                },

                                gtk4::Button {
                                    set_label: "Cancel",
                                    set_halign: gtk4::Align::Center,
                                    add_css_class: "pill",
                                    connect_clicked => AuthDialogInput::CancelPlexAuth,
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

                        // Manual setup expander
                        adw::PreferencesGroup {
                            set_title: "Manual Configuration",
                            set_description: Some("Connect using server URL and auth token"),
                            #[watch]
                            set_visible: !model.plex_auth_in_progress && !model.plex_auth_success && model.plex_auth_error.is_none(),
                            set_margin_top: 24,

                            add = &adw::ExpanderRow {
                                set_title: "Advanced Options",
                                set_expanded: false,
                                set_show_enable_switch: false,

                                #[name = "server_url_entry"]
                                add_row = &adw::EntryRow {
                                    set_title: "Server URL (optional)",
                                    set_text: &model.plex_server_url,
                                },

                                #[name = "token_entry"]
                                add_row = &adw::PasswordEntryRow {
                                    set_title: "Auth Token",
                                    set_text: &model.plex_token,
                                },

                                add_row = &adw::ActionRow {
                                    set_title: "Token Location",
                                    set_subtitle: "Find your auth token at plex.tv/api/v2/user",
                                },

                                add_row = &adw::ActionRow {
                                    #[wrap(Some)]
                                    set_child = &gtk4::Button {
                                        set_label: "Connect with Token",
                                        set_valign: gtk4::Align::Center,
                                        add_css_class: "suggested-action",
                                        connect_clicked => AuthDialogInput::ConnectManualPlex,
                                    },
                                },
                            },
                        },
                    },

                    // Jellyfin page - simplified single-step interface
                    add_titled[Some("jellyfin"), "Jellyfin"] = &gtk4::Box {
                        set_orientation: gtk4::Orientation::Vertical,
                        set_spacing: 24,
                        set_margin_top: 12,
                        set_margin_bottom: 12,
                        set_margin_start: 12,
                        set_margin_end: 12,

                        // Main form - always visible
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Vertical,
                            set_spacing: 24,
                            #[watch]
                            set_visible: !model.jellyfin_auth_in_progress && !model.jellyfin_auth_success && model.jellyfin_auth_error.is_none() && !model.jellyfin_quick_connect_in_progress,

                            // Server URL - always editable
                            adw::PreferencesGroup {
                                set_title: "Server Configuration",
                                set_description: Some("Enter your Jellyfin server address"),

                                #[name = "jellyfin_url_entry"]
                                add = &adw::EntryRow {
                                    set_title: "Server URL",
                                    set_text: &model.jellyfin_url,
                                    set_input_hints: gtk4::InputHints::NO_SPELLCHECK,
                                    connect_changed[sender] => move |entry| {
                                        sender.input(AuthDialogInput::UpdateJellyfinUrl(entry.text().to_string()));
                                    },
                                },

                                add = &adw::ActionRow {
                                    set_title: "Example",
                                    set_subtitle: "http://192.168.1.100:8096 or https://jellyfin.example.com",
                                    add_css_class: "property",
                                },
                            },

                            // Quick Connect option
                            adw::PreferencesGroup {
                                set_title: "Quick Connect",
                                set_description: Some("Sign in without entering username/password"),
                                #[watch]
                                set_sensitive: !model.jellyfin_url.is_empty(),

                                add = &adw::ActionRow {
                                    set_title: "Authorize with Quick Connect",
                                    set_subtitle: "Get a code to enter in your Jellyfin dashboard",

                                    add_suffix = &gtk4::Button {
                                        set_label: "Get Code",
                                        set_valign: gtk4::Align::Center,
                                        add_css_class: "suggested-action",
                                        #[watch]
                                        set_sensitive: !model.jellyfin_url.is_empty(),
                                        connect_clicked => AuthDialogInput::StartJellyfinQuickConnect,
                                    },
                                },
                            },

                            // Username/Password option
                            adw::PreferencesGroup {
                                set_title: "Username & Password",
                                set_description: Some("Traditional login with your Jellyfin credentials"),
                                #[watch]
                                set_sensitive: !model.jellyfin_url.is_empty(),

                                #[name = "jellyfin_username_entry"]
                                add = &adw::EntryRow {
                                    set_title: "Username",
                                    set_text: &model.jellyfin_username,
                                    connect_changed[sender] => move |entry| {
                                        sender.input(AuthDialogInput::UpdateJellyfinUsername(entry.text().to_string()));
                                    },
                                },

                                #[name = "jellyfin_password_entry"]
                                add = &adw::PasswordEntryRow {
                                    set_title: "Password",
                                    set_text: &model.jellyfin_password,
                                    connect_changed[sender] => move |entry| {
                                        sender.input(AuthDialogInput::UpdateJellyfinPassword(entry.text().to_string()));
                                    },
                                },

                                add = &adw::ActionRow {
                                    #[wrap(Some)]
                                    set_child = &gtk4::Button {
                                        set_label: "Sign In",
                                        set_valign: gtk4::Align::Center,
                                        add_css_class: "suggested-action",
                                        #[watch]
                                        set_sensitive: !model.jellyfin_url.is_empty() && !model.jellyfin_username.is_empty() && !model.jellyfin_password.is_empty() && !model.jellyfin_auth_in_progress,
                                        connect_clicked => AuthDialogInput::ConnectJellyfin,
                                    },
                                },
                            },
                        },

                        // Quick Connect Code Display - matching Plex PIN style
                        gtk4::Box {
                            set_orientation: gtk4::Orientation::Vertical,
                            set_spacing: 24,
                            set_valign: gtk4::Align::Center,
                            set_vexpand: true,
                            #[watch]
                            set_visible: model.jellyfin_quick_connect_in_progress,

                            adw::StatusPage {
                                set_icon_name: Some("dialog-password-symbolic"),
                                set_title: "Quick Connect Code",
                                #[wrap(Some)]
                                set_child = &gtk4::Box {
                                    set_orientation: gtk4::Orientation::Vertical,
                                    set_spacing: 20,

                                    gtk4::Label {
                                        set_label: "Enter this code in your Jellyfin dashboard:",
                                        add_css_class: "dim-label",
                                        set_wrap: true,
                                        set_justify: gtk4::Justification::Center,
                                    },

                                    #[name = "jellyfin_quick_connect_code_label"]
                                    gtk4::Label {
                                        add_css_class: "title-1",
                                        add_css_class: "accent",
                                        #[watch]
                                        set_label: model.jellyfin_quick_connect_code.as_ref().unwrap_or(&String::new()),
                                    },

                                    gtk4::Box {
                                        set_orientation: gtk4::Orientation::Vertical,
                                        set_spacing: 8,

                                        gtk4::Label {
                                            set_markup: "1. Go to your Jellyfin server's <b>Dashboard → Users</b>",
                                            add_css_class: "dim-label",
                                            add_css_class: "caption",
                                            set_halign: gtk4::Align::Start,
                                        },

                                        gtk4::Label {
                                            set_markup: "2. Click on <b>Quick Connect</b>",
                                            add_css_class: "dim-label",
                                            add_css_class: "caption",
                                            set_halign: gtk4::Align::Start,
                                        },

                                        gtk4::Label {
                                            set_markup: "3. Enter the code above and <b>Authorize</b>",
                                            add_css_class: "dim-label",
                                            add_css_class: "caption",
                                            set_halign: gtk4::Align::Start,
                                        },
                                    },

                                    #[name = "jellyfin_quick_connect_progress"]
                                    gtk4::ProgressBar {
                                        set_margin_top: 24,
                                        #[watch]
                                        set_pulse_step: if model.jellyfin_quick_connect_in_progress { 0.1 } else { 0.0 },
                                    },

                                    gtk4::Label {
                                        set_label: "Waiting for authorization...",
                                        add_css_class: "dim-label",
                                        add_css_class: "caption",
                                    },

                                    gtk4::Button {
                                        set_label: "Cancel",
                                        set_margin_top: 12,
                                        connect_clicked => AuthDialogInput::CancelJellyfinQuickConnect,
                                    },
                                },
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
                    },
                },
            },
        }
    }

    async fn init(
        (db, parent_window): Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let mut model = AuthDialog {
            db,
            backend_type: BackendType::Plex,
            parent_window,
            is_visible: false,
            dialog_position: None,

            // Plex OAuth state
            plex_pin: None,
            plex_auth_in_progress: false,
            plex_auth_success: false,
            plex_auth_error: None,

            // Plex profile selection state
            plex_home_users: Vec::new(),
            plex_selected_profile: None,
            plex_profile_selection_active: false,
            plex_primary_token: None,
            plex_pin_input_active: false,

            // Jellyfin state
            jellyfin_url: String::new(),
            jellyfin_url_confirmed: false,
            jellyfin_username: String::new(),
            jellyfin_password: String::new(),
            jellyfin_auth_in_progress: false,
            jellyfin_auth_success: false,
            jellyfin_auth_error: None,
            // Jellyfin Quick Connect state
            jellyfin_quick_connect_enabled: false,
            jellyfin_quick_connect_in_progress: false,
            jellyfin_quick_connect_code: None,
            jellyfin_quick_connect_secret: None,
            jellyfin_quick_connect_check_handle: None,

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
            jellyfin_quick_connect_code_label: gtk4::Label::new(None),
            jellyfin_quick_connect_progress: gtk4::ProgressBar::new(),
            server_url_entry: adw::EntryRow::new(),
            token_entry: adw::PasswordEntryRow::new(),
            profile_flowbox: gtk4::FlowBox::new(),
        };

        let widgets = view_output!();

        // Store reference to dialog for later use
        model.dialog = widgets.dialog.clone();

        // Store reference to profile_flowbox from the view
        model.profile_flowbox = widgets.profile_flowbox.clone();

        // Explicitly ensure no icons are displayed on auth tabs
        if let Some(plex_child) = widgets.view_stack.child_by_name("plex") {
            let plex_page = widgets.view_stack.page(&plex_child);
            plex_page.set_icon_name(None);
        }
        if let Some(jellyfin_child) = widgets.view_stack.child_by_name("jellyfin") {
            let jellyfin_page = widgets.view_stack.page(&jellyfin_child);
            jellyfin_page.set_icon_name(None);
        }

        // Start progress bar pulse animations
        glib::timeout_add_local(std::time::Duration::from_millis(100), {
            let auth_progress = model.auth_progress.clone();
            let jellyfin_progress = model.jellyfin_progress.clone();
            let jellyfin_quick_connect_progress = model.jellyfin_quick_connect_progress.clone();
            move || {
                auth_progress.pulse();
                jellyfin_progress.pulse();
                jellyfin_quick_connect_progress.pulse();
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
                info!("Showing auth dialog as modal");
                self.is_visible = true;
                // Present as modal dialog attached to the parent window
                if let Some(ref parent) = self.parent_window {
                    self.dialog.present(Some(parent));
                } else {
                    self.dialog.present(None::<&gtk4::Window>);
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
                info!(
                    "Token received (first 10 chars): {}...",
                    &token[..10.min(token.len())]
                );
                self.plex_auth_in_progress = false;

                // Store the primary token
                self.plex_primary_token = Some(token.clone());

                info!("Fetching Home users to check if profile selection is needed...");
                // Fetch Home users to see if we need profile selection
                let sender_clone = sender.clone();
                sender.oneshot_command(async move {
                    info!("Making API call to get_home_users...");
                    match PlexAuth::get_home_users(&token).await {
                        Ok(users) => {
                            info!("API call successful. Found {} Home users", users.len());
                            if !users.is_empty() {
                                info!("Home users found:");
                                for user in &users {
                                    info!(
                                        "  - {} (id: {}, admin: {}, protected: {})",
                                        user.name, user.id, user.is_admin, user.is_protected
                                    );
                                }
                            }

                            if users.is_empty() {
                                info!("No Home users found, proceeding with primary token");
                                // No Home users, proceed with primary token
                                sender_clone.input(AuthDialogInput::SkipProfileSelection);
                            } else {
                                info!(
                                    "Sending PlexHomeUsersReceived message with {} users",
                                    users.len()
                                );
                                // Show profile selection
                                sender_clone.input(AuthDialogInput::PlexHomeUsersReceived(users));
                            }
                        }
                        Err(e) => {
                            info!(
                                "Failed to get Home users (likely not a Home account): {}",
                                e
                            );
                            // Not a Home account or error, proceed with primary token
                            sender_clone.input(AuthDialogInput::SkipProfileSelection);
                        }
                    }
                });
            }

            AuthDialogInput::PlexHomeUsersReceived(users) => {
                info!(
                    "PlexHomeUsersReceived message received with {} users",
                    users.len()
                );
                info!(
                    "Current UI state - profile_selection_active: {}, auth_in_progress: {}, auth_success: {}",
                    self.plex_profile_selection_active,
                    self.plex_auth_in_progress,
                    self.plex_auth_success
                );

                self.plex_home_users = users.clone();

                // Clear previous profile cards
                while let Some(child) = self.profile_flowbox.first_child() {
                    self.profile_flowbox.remove(&child);
                }

                // Add profile cards for each user
                info!("Adding {} profile cards to flowbox", users.len());
                for user in users {
                    info!("Creating profile card for user: {}", user.name);
                    let button = gtk4::Button::new();
                    button.add_css_class("flat");
                    button.add_css_class("card");
                    button.set_tooltip_text(Some(&user.name));
                    button.set_visible(true);
                    button.set_can_focus(true);
                    button.set_width_request(160);
                    button.set_height_request(180);

                    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
                    vbox.set_margin_top(16);
                    vbox.set_margin_bottom(16);
                    vbox.set_margin_start(8);
                    vbox.set_margin_end(8);
                    vbox.set_visible(true);
                    vbox.set_valign(gtk4::Align::Center);
                    vbox.set_halign(gtk4::Align::Center);

                    let avatar_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
                    avatar_box.set_halign(gtk4::Align::Center);

                    let avatar = adw::Avatar::new(80, Some(&user.name), true);
                    // TODO: Set custom image from user.thumb if available
                    avatar_box.append(&avatar);

                    if user.is_protected {
                        let lock_icon = gtk4::Image::from_icon_name("dialog-password-symbolic");
                        lock_icon.add_css_class("warning");
                        lock_icon.set_icon_size(gtk4::IconSize::Normal);
                        lock_icon.set_margin_top(-20);
                        lock_icon.set_margin_start(60);
                        lock_icon.set_halign(gtk4::Align::End);
                        avatar_box.append(&lock_icon);
                    }

                    vbox.append(&avatar_box);

                    let name_label = gtk4::Label::new(Some(&user.name));
                    name_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                    name_label.set_max_width_chars(15);
                    vbox.append(&name_label);

                    if user.is_admin {
                        let admin_label = gtk4::Label::new(Some("Admin"));
                        admin_label.add_css_class("dim-label");
                        admin_label.add_css_class("caption");
                        vbox.append(&admin_label);
                    }

                    button.set_child(Some(&vbox));

                    let sender_clone = sender.clone();
                    let user_clone = user.clone();
                    button.connect_clicked(move |_| {
                        sender_clone.input(AuthDialogInput::SelectPlexProfile(user_clone.clone()));
                    });

                    self.profile_flowbox.append(&button);
                    info!("Added profile card for {} to flowbox", user.name);
                    info!(
                        "Button visible: {}, vbox visible: {}",
                        button.is_visible(),
                        vbox.is_visible()
                    );
                }

                info!(
                    "All profile cards added. Flowbox child count: {}",
                    self.profile_flowbox.observe_children().n_items()
                );

                // Force the flowbox to resize
                self.profile_flowbox.queue_resize();
                self.profile_flowbox.queue_draw();

                // Debug flowbox properties
                info!(
                    "Flowbox visible: {}, mapped: {}, realized: {}",
                    self.profile_flowbox.is_visible(),
                    self.profile_flowbox.is_mapped(),
                    self.profile_flowbox.is_realized()
                );

                // Check if any children are visible
                if let Some(first_child) = self.profile_flowbox.first_child() {
                    info!("First child visible: {}", first_child.is_visible());
                }

                info!("Setting profile_selection_active to true");
                self.plex_profile_selection_active = true;
                info!("Profile selection UI should now be visible");
            }

            AuthDialogInput::SelectPlexProfile(profile) => {
                info!("Selected profile: {}", profile.name);

                if profile.is_protected {
                    // Need PIN input
                    self.plex_selected_profile = Some(profile);
                    self.plex_profile_selection_active = false;
                    self.plex_pin_input_active = true;
                } else {
                    // Switch to profile without PIN
                    self.plex_selected_profile = Some(profile.clone());
                    self.plex_profile_selection_active = false;

                    let token = self.plex_primary_token.clone().unwrap();
                    let user_id = profile.id.clone();
                    let sender_clone = sender.clone();

                    sender.oneshot_command(async move {
                        match PlexAuth::switch_to_user(&token, &user_id, None).await {
                            Ok(profile_token) => {
                                info!("Successfully switched to profile");
                                sender_clone.input(AuthDialogInput::PlexProfileTokenReceived(
                                    profile_token,
                                ));
                            }
                            Err(e) => {
                                sender_clone.input(AuthDialogInput::PlexAuthError(format!(
                                    "Failed to switch to profile: {}",
                                    e
                                )));
                            }
                        }
                    });
                }
            }

            AuthDialogInput::PlexPinRequested(pin) => {
                info!("PIN entered for protected profile");
                self.plex_pin_input_active = false;

                let token = self.plex_primary_token.clone().unwrap();
                let profile = self.plex_selected_profile.clone().unwrap();
                let sender_clone = sender.clone();

                sender.oneshot_command(async move {
                    match PlexAuth::switch_to_user(&token, &profile.id, Some(&pin)).await {
                        Ok(profile_token) => {
                            info!("Successfully switched to protected profile");
                            sender_clone
                                .input(AuthDialogInput::PlexProfileTokenReceived(profile_token));
                        }
                        Err(e) => {
                            // Check if it's a PIN error specifically
                            if e.to_string().contains("Invalid PIN")
                                || e.to_string().contains("authentication failed")
                            {
                                // Show PIN input again with error message
                                sender_clone.input(AuthDialogInput::PlexAuthError(
                                    "Invalid PIN. Please try again.".to_string(),
                                ));
                                // TODO: Re-show PIN input with error
                            } else {
                                sender_clone.input(AuthDialogInput::PlexAuthError(format!(
                                    "Failed to switch to profile: {}",
                                    e
                                )));
                            }
                        }
                    }
                });
            }

            AuthDialogInput::PlexProfileTokenReceived(profile_token) => {
                info!("Profile token received, proceeding with source creation");
                self.plex_profile_selection_active = false; // Hide profile selection first
                self.plex_auth_success = true;
                self.proceed_with_plex_source_creation(profile_token, sender);
            }

            AuthDialogInput::SkipProfileSelection => {
                info!("SkipProfileSelection message received");
                info!(
                    "Current UI state - profile_selection_active: {}, auth_success: {}",
                    self.plex_profile_selection_active, self.plex_auth_success
                );
                info!("Using primary token to create source");

                self.plex_auth_success = true;
                let token = self
                    .plex_primary_token
                    .clone()
                    .unwrap_or_else(|| self.plex_token.clone());
                self.proceed_with_plex_source_creation(token, sender);
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
                self.plex_profile_selection_active = false;
                self.plex_pin_input_active = false;
                self.plex_primary_token = None;
                self.plex_selected_profile = None;
            }

            AuthDialogInput::RetryPlexAuth => {
                info!("Retrying Plex auth");
                self.plex_auth_error = None;
                self.plex_auth_success = false;
                self.plex_profile_selection_active = false;
                self.plex_pin_input_active = false;
                self.plex_primary_token = None;
                self.plex_selected_profile = None;
                self.plex_home_users.clear();
                // Don't automatically start auth, just reset to initial state
                // sender.input(AuthDialogInput::StartPlexAuth);
            }

            AuthDialogInput::OpenPlexLink => {
                info!("Opening Plex link");
                let _ = gtk4::gio::AppInfo::launch_default_for_uri(
                    "https://plex.tv/link",
                    None::<&gtk4::gio::AppLaunchContext>,
                );
            }

            AuthDialogInput::UpdateJellyfinUrl(url) => {
                self.jellyfin_url = url;
            }

            AuthDialogInput::UpdateJellyfinUsername(username) => {
                self.jellyfin_username = username;
            }

            AuthDialogInput::UpdateJellyfinPassword(password) => {
                self.jellyfin_password = password;
            }

            AuthDialogInput::ConnectJellyfin => {
                info!("Connecting to Jellyfin");
                // Model fields are already updated via UpdateJellyfin* messages

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
                                machine_id: None, // Not used for Jellyfin
                                is_owned: None,   // Not used for Jellyfin
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
                self.jellyfin_auth_in_progress = false;
                // Keep all fields so user can retry or modify them
            }

            AuthDialogInput::StartJellyfinQuickConnect => {
                info!("Starting Jellyfin Quick Connect - button clicked");
                info!("Current URL: '{}'", self.jellyfin_url);

                // URL is already in model from UpdateJellyfinUrl messages
                if self.jellyfin_url.is_empty() {
                    info!("URL is empty, showing error");
                    self.jellyfin_auth_error = Some("Please enter a server URL".to_string());
                    return;
                }

                info!("Setting quick connect in progress");
                self.jellyfin_quick_connect_in_progress = true;
                self.jellyfin_auth_error = None;

                let url = self.jellyfin_url.clone();
                let sender_clone = sender.clone();

                info!("Launching oneshot command to check Quick Connect");
                // First check if Quick Connect is enabled, then initiate
                sender.oneshot_command(async move {
                    use crate::backends::jellyfin::api::JellyfinApi;
                    use tracing::{info, error};

                    info!("Checking if Quick Connect is enabled at: {}", url);
                    // Check if Quick Connect is enabled
                    match JellyfinApi::check_quick_connect_enabled(&url).await {
                        Ok(enabled) => {
                            info!("Quick Connect enabled check result: {}", enabled);
                            if !enabled {
                                info!("Quick Connect is disabled on the server");
                                sender_clone.input(AuthDialogInput::JellyfinQuickConnectFailed(
                                    "Quick Connect is disabled on this server. Please enable it in the Jellyfin admin dashboard.".to_string(),
                                ));
                                return;
                            }

                            info!("Quick Connect is enabled, initiating...");
                            // Initiate Quick Connect
                            match JellyfinApi::initiate_quick_connect(&url).await {
                                Ok(state) => {
                                    info!("Quick Connect initiated successfully with code: {}", state.code);
                                    sender_clone.input(
                                        AuthDialogInput::JellyfinQuickConnectInitiated {
                                            code: state.code,
                                            secret: state.secret,
                                        },
                                    );
                                }
                                Err(e) => {
                                    error!("Failed to initiate Quick Connect: {}", e);
                                    sender_clone.input(
                                        AuthDialogInput::JellyfinQuickConnectFailed(format!(
                                            "Failed to initiate Quick Connect: {}\n\nMake sure your server URL is correct and the server is accessible.",
                                            e
                                        )),
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to check Quick Connect status: {}", e);
                            sender_clone.input(AuthDialogInput::JellyfinQuickConnectFailed(
                                format!("Failed to check Quick Connect status: {}\n\nPlease verify your server URL and network connection.", e),
                            ));
                        }
                    }
                });
            }

            AuthDialogInput::JellyfinQuickConnectInitiated { code, secret } => {
                info!("Quick Connect code received: {}", code);
                self.jellyfin_quick_connect_code = Some(code);
                self.jellyfin_quick_connect_secret = Some(secret);

                // Start polling for authentication status every 2 seconds
                let sender_clone = sender.clone();
                let handle =
                    glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
                        sender_clone.input(AuthDialogInput::CheckJellyfinQuickConnectStatus);
                        glib::ControlFlow::Continue
                    });
                self.jellyfin_quick_connect_check_handle = Some(handle);
            }

            AuthDialogInput::CheckJellyfinQuickConnectStatus => {
                if let Some(secret) = &self.jellyfin_quick_connect_secret {
                    let url = self.jellyfin_url.clone();
                    let secret = secret.clone();
                    let sender_clone = sender.clone();

                    sender.oneshot_command(async move {
                        use crate::backends::jellyfin::api::JellyfinApi;

                        match JellyfinApi::get_quick_connect_state(&url, &secret).await {
                            Ok(result) => {
                                if result.authenticated {
                                    info!("Quick Connect authenticated!");
                                    // Now authenticate with the secret
                                    match JellyfinApi::authenticate_with_quick_connect(&url, &secret).await {
                                        Ok(auth_response) => {
                                            // Pass both token and user_id as a combined string
                                            let combined = format!("{}|{}", auth_response.access_token, auth_response.user.id);
                                            sender_clone.input(AuthDialogInput::JellyfinQuickConnectAuthenticated(
                                                combined
                                            ));
                                        }
                                        Err(e) => {
                                            sender_clone.input(AuthDialogInput::JellyfinQuickConnectFailed(
                                                format!("Failed to authenticate with Quick Connect: {}", e)
                                            ));
                                        }
                                    }
                                }
                                // If not authenticated yet, polling continues
                            }
                            Err(e) => {
                                // Don't fail on polling errors, just log them
                                info!("Quick Connect polling error (will retry): {}", e);
                            }
                        }
                    });
                }
            }

            AuthDialogInput::JellyfinQuickConnectAuthenticated(combined_data) => {
                info!("Quick Connect authentication successful");

                // Stop polling
                if let Some(handle) = self.jellyfin_quick_connect_check_handle.take() {
                    handle.remove();
                }

                self.jellyfin_quick_connect_in_progress = false;
                self.jellyfin_auth_success = true;

                // Parse the combined token and user_id
                let parts: Vec<&str> = combined_data.split('|').collect();
                let (token, user_id) = if parts.len() == 2 {
                    (parts[0].to_string(), parts[1].to_string())
                } else {
                    // Fallback to old format if just token
                    (combined_data.clone(), String::new())
                };

                // Create the source with the token
                let url = self.jellyfin_url.clone();
                let db = self.db.clone();
                let sender_clone = sender.clone();

                sender.oneshot_command(async move {
                    // Create the backend with the server URL
                    let jellyfin_backend = JellyfinBackend::new();

                    // Set the base URL first
                    jellyfin_backend.set_base_url(url.clone()).await;

                    // Create credentials with Quick Connect token including user_id
                    // Format: token|user_id for Jellyfin to parse
                    let credentials = Credentials::Token {
                        token: format!("{}|{}", token, user_id),
                    };

                    // Authenticate with the token
                    match jellyfin_backend.authenticate(credentials.clone()).await {
                        Ok(user) => {
                            info!("Quick Connect user authenticated: {}", user.username);

                            // Create the source
                            let command = CreateSourceCommand {
                                db,
                                backend: &jellyfin_backend as &dyn MediaBackend,
                                source_type: "jellyfin".to_string(),
                                name: format!("Jellyfin - {}", user.username),
                                credentials,
                                server_url: Some(url),
                                machine_id: None,
                                is_owned: None,
                            };

                            match command.execute().await {
                                Ok(source) => {
                                    info!(
                                        "Created Jellyfin source via Quick Connect: {}",
                                        source.id
                                    );
                                    sender_clone.input(AuthDialogInput::SourceCreated(
                                        SourceId::new(source.id),
                                    ));
                                }
                                Err(e) => {
                                    sender_clone.input(
                                        AuthDialogInput::JellyfinQuickConnectFailed(format!(
                                            "Failed to create source: {}",
                                            e
                                        )),
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            sender_clone.input(AuthDialogInput::JellyfinQuickConnectFailed(
                                format!("Failed to authenticate with token: {}", e),
                            ));
                        }
                    }
                });
            }

            AuthDialogInput::JellyfinQuickConnectFailed(error) => {
                info!("Quick Connect failed: {}", error);

                // Stop polling if active
                if let Some(handle) = self.jellyfin_quick_connect_check_handle.take() {
                    handle.remove();
                }

                self.jellyfin_quick_connect_in_progress = false;
                self.jellyfin_quick_connect_code = None;
                self.jellyfin_quick_connect_secret = None;
                self.jellyfin_auth_error = Some(error);
            }

            AuthDialogInput::CancelJellyfinQuickConnect => {
                info!("Cancelling Jellyfin Quick Connect");

                // Stop polling
                if let Some(handle) = self.jellyfin_quick_connect_check_handle.take() {
                    handle.remove();
                }

                self.jellyfin_quick_connect_in_progress = false;
                self.jellyfin_quick_connect_code = None;
                self.jellyfin_quick_connect_secret = None;
            }

            AuthDialogInput::ConnectManualPlex => {
                info!("Connecting with manual Plex credentials");
                self.plex_server_url = self.server_url_entry.text().to_string();
                self.plex_token = self.token_entry.text().to_string();

                if self.plex_server_url.is_empty() || self.plex_token.is_empty() {
                    self.plex_auth_error = Some("Please fill in all fields".to_string());
                    return;
                }

                // Don't mark as successful yet - let PlexTokenReceived handle the flow
                self.plex_auth_in_progress = false;
                // self.plex_auth_success = true; // REMOVED - this was preventing profile selection!

                // Send the token to trigger the normal flow (including profile selection check)
                sender.input(AuthDialogInput::PlexTokenReceived(self.plex_token.clone()));
            }
        }
    }
}
