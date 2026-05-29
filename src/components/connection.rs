use std::path::PathBuf;

use adw;
use adw::prelude::*;
use relm4::prelude::*;
use tracing::info;

use crate::services::jellyfin::auth as jf_auth;
use crate::services::jellyfin::error::JellyfinError;
use crate::services::plex::auth;

/// Normalize a user-entered server URL: trim whitespace, prepend `https://` when
/// no scheme is present, and strip a single trailing slash.
pub fn normalize_server_url(input: &str) -> String {
    let trimmed = input.trim();
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    with_scheme.trim_end_matches('/').to_string()
}

/// True when the (normalized) URL uses a cleartext `http://` scheme. Used to
/// surface a warning that credentials and the token travel unencrypted.
pub fn is_insecure_url(url: &str) -> bool {
    normalize_server_url(url).starts_with("http://")
}

/// Classification of a connection failure for user-facing messaging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionErrorKind {
    /// Quick Connect is turned off on the server.
    QuickConnectDisabled,
    /// The server could not be reached (DNS, connect, timeout).
    Unreachable,
    /// Credentials were rejected (401).
    AuthFailed,
    /// Anything else.
    Other,
}

impl ConnectionErrorKind {
    /// Classify a `JellyfinError` into a `ConnectionErrorKind`.
    pub fn from_error(err: &JellyfinError) -> Self {
        match err {
            JellyfinError::Unauthorized => Self::AuthFailed,
            JellyfinError::Http(e) if e.is_connect() || e.is_timeout() => Self::Unreachable,
            _ => Self::Other,
        }
    }
}

/// Map a connection error classification to a user-facing message.
pub fn connection_error_message(kind: ConnectionErrorKind) -> &'static str {
    match kind {
        ConnectionErrorKind::QuickConnectDisabled => {
            "Quick Connect is disabled on this server. Use your username and password instead."
        }
        ConnectionErrorKind::Unreachable => {
            "Could not reach the server. Check the URL and your network."
        }
        ConnectionErrorKind::AuthFailed => "Incorrect username or password.",
        ConnectionErrorKind::Other => "Authentication failed. Please try again.",
    }
}

/// Derive a display name for a Jellyfin source from its server URL — the host,
/// or a generic fallback.
pub fn jellyfin_display_name(url: &str) -> String {
    let normalized = normalize_server_url(url);
    let without_scheme = normalized
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(&normalized);
    let host = without_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");
    if host.is_empty() {
        "Jellyfin Server".to_string()
    } else {
        host.to_string()
    }
}

#[allow(dead_code)]
pub struct ConnectionDialog {
    plex_client_id: String,
    jellyfin_device_id: String,
    auth_token: Option<String>,
    servers: Vec<auth::PlexResource>,
    /// Generation counter. Incremented on view changes and dialog close so a
    /// late Quick Connect poll completion can be ignored.
    epoch: u64,
    // Plex widgets
    status_label: gtk::Label,
    spinner: gtk::Spinner,
    sign_in_button: gtk::Button,
    server_dropdown: gtk::DropDown,
    server_box: gtk::Box,
    // View stack
    view_stack: gtk::Stack,
    // Jellyfin widgets
    jf_url_entry: gtk::Entry,
    jf_username_entry: gtk::Entry,
    jf_password_entry: gtk::PasswordEntry,
    jf_insecure_label: gtk::Label,
    jf_status_label: gtk::Label,
    jf_spinner: gtk::Spinner,
    jf_code_label: gtk::Label,
    jf_signin_button: gtk::Button,
    jf_quick_button: gtk::Button,
}

pub enum ConnectionDialogMsg {
    // Backend chooser
    ChoosePlex,
    ChooseJellyfin,
    BackToChooser,
    // Plex flow
    SignIn,
    AuthSucceeded(String),
    AuthFailed(String),
    ServersFound(Vec<auth::PlexResource>),
    ServerDiscoveryFailed(String),
    ConfirmServer,
    // Jellyfin flow
    JellyfinUrlChanged,
    JellyfinSignIn,
    JellyfinQuickConnect,
    Cancel,
}

impl std::fmt::Debug for ConnectionDialogMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChoosePlex => write!(f, "ChoosePlex"),
            Self::ChooseJellyfin => write!(f, "ChooseJellyfin"),
            Self::BackToChooser => write!(f, "BackToChooser"),
            Self::SignIn => write!(f, "SignIn"),
            Self::AuthSucceeded(_) => write!(f, "AuthSucceeded(..)"),
            Self::AuthFailed(e) => write!(f, "AuthFailed({e})"),
            Self::ServersFound(s) => write!(f, "ServersFound({} servers)", s.len()),
            Self::ServerDiscoveryFailed(e) => write!(f, "ServerDiscoveryFailed({e})"),
            Self::ConfirmServer => write!(f, "ConfirmServer"),
            Self::JellyfinUrlChanged => write!(f, "JellyfinUrlChanged"),
            Self::JellyfinSignIn => write!(f, "JellyfinSignIn"),
            Self::JellyfinQuickConnect => write!(f, "JellyfinQuickConnect"),
            Self::Cancel => write!(f, "Cancel"),
        }
    }
}

#[derive(Debug)]
pub enum ConnectionDialogOutput {
    ConnectionSaved {
        url: String,
        token: String,
        name: String,
        source_type: String,
        user_id: Option<String>,
        /// Whether the selected connection is remote/relay (R5/U2). Always
        /// `false` for non-Plex sources (the cap is Plex-specific).
        is_remote: bool,
    },
    Cancelled,
}

#[derive(Debug)]
pub enum ConnectionDialogCmd {
    TokenReceived(Result<String, String>),
    ServersReady(Result<(String, Vec<auth::PlexResource>), String>),
    /// Resolved the best connection for a server: (selected, token, server_name).
    /// `selected` is `None` when no connection was reachable. `SelectedConnection`
    /// carries the local/remote classification (U2).
    UriResolved(Option<auth::SelectedConnection>, String, String),
    /// Quick Connect code to display while polling. Carries its epoch.
    JellyfinCode {
        epoch: u64,
        code: String,
    },
    /// Jellyfin password/quick-connect auth result. Carries the epoch captured
    /// when the command started so a stale completion can be ignored.
    /// `Ok((url, token, user_id))`.
    JellyfinAuth {
        epoch: u64,
        result: Result<(String, String, String), ConnectionErrorKind>,
    },
}

#[relm4::component(pub)]
impl Component for ConnectionDialog {
    type Init = PathBuf; // data dir
    type Input = ConnectionDialogMsg;
    type Output = ConnectionDialogOutput;
    type CommandOutput = ConnectionDialogCmd;

    view! {
        #[root]
        adw::Window {
            set_title: Some("Add a Source"),
            set_default_width: 450,
            set_default_height: 360,
            set_modal: true,
            connect_close_request[sender] => move |_| {
                let _ = sender.input_sender().send(ConnectionDialogMsg::Cancel);
                gtk::glib::Propagation::Proceed
            },
        }
    }

    #[allow(clippy::too_many_lines)]
    fn init(
        data_dir: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = view_output!();

        let plex_client_id = auth::client_identifier(&data_dir);
        let jellyfin_device_id = jf_auth::device_id(&data_dir);

        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();

        let cancel_button = gtk::Button::builder().label("Cancel").build();
        let sender_cancel = sender.input_sender().clone();
        cancel_button.connect_clicked(move |_| {
            let _ = sender_cancel.send(ConnectionDialogMsg::Cancel);
        });
        header.pack_start(&cancel_button);
        toolbar.add_top_bar(&header);

        let view_stack = gtk::Stack::new();

        // --- View 1: backend chooser ---
        let chooser_box = build_chooser_view(&sender);
        view_stack.add_named(&chooser_box, Some("chooser"));

        // --- View 2: Plex sign-in (existing flow) ---
        let (plex_box, status_label, spinner, sign_in_button, server_dropdown, server_box) =
            build_plex_view(&sender);
        view_stack.add_named(&plex_box, Some("plex"));

        // --- View 3: Jellyfin sign-in ---
        let jf = build_jellyfin_view(&sender);
        view_stack.add_named(&jf.container, Some("jellyfin"));

        view_stack.set_visible_child_name("chooser");

        toolbar.set_content(Some(&view_stack));
        root.set_content(Some(&toolbar));

        let model = Self {
            plex_client_id,
            jellyfin_device_id,
            auth_token: None,
            servers: Vec::new(),
            epoch: 0,
            status_label,
            spinner,
            sign_in_button,
            server_dropdown,
            server_box,
            view_stack,
            jf_url_entry: jf.url_entry,
            jf_username_entry: jf.username_entry,
            jf_password_entry: jf.password_entry,
            jf_insecure_label: jf.insecure_label,
            jf_status_label: jf.status_label,
            jf_spinner: jf.spinner,
            jf_code_label: jf.code_label,
            jf_signin_button: jf.signin_button,
            jf_quick_button: jf.quick_button,
        };

        ComponentParts { model, widgets }
    }

    #[allow(clippy::too_many_lines)]
    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            ConnectionDialogMsg::ChoosePlex => {
                self.epoch += 1;
                self.view_stack.set_visible_child_name("plex");
            }
            ConnectionDialogMsg::ChooseJellyfin => {
                self.epoch += 1;
                self.view_stack.set_visible_child_name("jellyfin");
            }
            ConnectionDialogMsg::BackToChooser => {
                self.epoch += 1;
                self.reset_jellyfin_view();
                self.view_stack.set_visible_child_name("chooser");
            }

            ConnectionDialogMsg::SignIn => {
                self.sign_in_button.set_visible(false);
                self.spinner.set_visible(true);
                self.spinner.set_spinning(true);
                self.status_label.set_css_classes(&[]);
                self.status_label
                    .set_label("Opening your browser...\nSign in to your Plex account.");

                let client_id = self.plex_client_id.clone();
                sender.oneshot_command(async move {
                    let (pin_id, code, auth_url) = match auth::request_pin(&client_id).await {
                        Ok(v) => v,
                        Err(e) => {
                            return ConnectionDialogCmd::TokenReceived(Err(format!(
                                "Failed to start sign-in: {e}"
                            )));
                        }
                    };

                    auth::open_browser(&auth_url);

                    match auth::poll_for_token(&client_id, pin_id, &code).await {
                        Ok(token) => ConnectionDialogCmd::TokenReceived(Ok(token)),
                        Err(e) => ConnectionDialogCmd::TokenReceived(Err(e.to_string())),
                    }
                });
            }

            ConnectionDialogMsg::AuthSucceeded(token) => {
                info!("Plex authentication successful");
                self.auth_token = Some(token.clone());
                self.status_label
                    .set_label("Signed in! Finding your servers...");

                let client_id = self.plex_client_id.clone();
                sender.oneshot_command(async move {
                    match auth::discover_servers(&client_id, &token).await {
                        Ok(servers) => ConnectionDialogCmd::ServersReady(Ok((token, servers))),
                        Err(e) => ConnectionDialogCmd::ServersReady(Err(e.to_string())),
                    }
                });
            }

            ConnectionDialogMsg::AuthFailed(err) => {
                self.spinner.set_visible(false);
                self.spinner.set_spinning(false);
                self.sign_in_button.set_visible(true);
                self.sign_in_button.set_label("Try Again");
                self.status_label.set_label(&err);
                self.status_label.set_css_classes(&["error"]);
            }

            ConnectionDialogMsg::ServersFound(servers) => {
                self.spinner.set_visible(false);
                self.spinner.set_spinning(false);

                if servers.is_empty() {
                    self.status_label
                        .set_label("No Plex Media Servers found on your account.");
                    self.sign_in_button.set_visible(true);
                    self.sign_in_button.set_label("Try Again");
                    return;
                }

                self.servers = servers;

                if self.servers.len() == 1 {
                    // Auto-connect to the only server
                    sender.input(ConnectionDialogMsg::ConfirmServer);
                    return;
                }

                // Multiple servers — show picker
                self.status_label.set_visible(false);
                let string_list = gtk::StringList::new(&[] as &[&str]);
                for s in &self.servers {
                    string_list.append(&s.name);
                }
                self.server_dropdown.set_model(Some(&string_list));
                self.server_dropdown.set_selected(0);
                self.server_box.set_visible(true);
            }

            ConnectionDialogMsg::ServerDiscoveryFailed(err) => {
                self.spinner.set_visible(false);
                self.spinner.set_spinning(false);
                self.status_label
                    .set_label(&format!("Failed to find servers: {err}"));
                self.status_label.set_css_classes(&["error"]);
                self.sign_in_button.set_visible(true);
                self.sign_in_button.set_label("Try Again");
            }

            ConnectionDialogMsg::ConfirmServer => {
                let idx = if self.servers.len() == 1 {
                    0
                } else {
                    self.server_dropdown.selected() as usize
                };

                if let Some(server) = self.servers.get(idx)
                    && let Some(ref token) = self.auth_token
                {
                    self.status_label.set_label("Testing server connections...");
                    self.status_label.set_visible(true);
                    self.status_label.set_css_classes(&[]);
                    self.spinner.set_visible(true);
                    self.spinner.set_spinning(true);
                    self.server_box.set_visible(false);
                    self.sign_in_button.set_visible(false);

                    let server = server.clone();
                    let token = token.clone();
                    sender.oneshot_command(async move {
                        let selected = auth::best_server_uri(&server).await;
                        ConnectionDialogCmd::UriResolved(selected, token, server.name.clone())
                    });
                }
            }

            ConnectionDialogMsg::JellyfinUrlChanged => {
                let url = self.jf_url_entry.text().to_string();
                self.jf_insecure_label
                    .set_visible(!url.trim().is_empty() && is_insecure_url(&url));
            }

            ConnectionDialogMsg::JellyfinSignIn => {
                let url = normalize_server_url(&self.jf_url_entry.text());
                let username = self.jf_username_entry.text().to_string();
                let password = self.jf_password_entry.text().to_string();
                if url.is_empty() || username.is_empty() {
                    self.set_jf_error("Enter a server URL and username.");
                    return;
                }
                self.start_jellyfin_busy("Signing in...");
                let device_id = self.jellyfin_device_id.clone();
                let epoch = self.epoch;
                sender.oneshot_command(async move {
                    let result =
                        jf_auth::authenticate_by_name(&url, &device_id, &username, &password)
                            .await
                            .map(|(token, user_id)| (url.clone(), token, user_id))
                            .map_err(|e| ConnectionErrorKind::from_error(&e));
                    ConnectionDialogCmd::JellyfinAuth { epoch, result }
                });
            }

            ConnectionDialogMsg::JellyfinQuickConnect => {
                let url = normalize_server_url(&self.jf_url_entry.text());
                if url.is_empty() {
                    self.set_jf_error("Enter a server URL first.");
                    return;
                }
                self.start_jellyfin_busy("Checking Quick Connect...");
                let device_id = self.jellyfin_device_id.clone();
                let epoch = self.epoch;
                // `command` (not `oneshot_command`) gives us an output sender so
                // the async block can emit the code mid-flight without capturing
                // any (non-Send) GTK widget.
                sender.command(move |out, _shutdown| async move {
                    // Step 1: is Quick Connect enabled?
                    match jf_auth::quick_connect_enabled(&url, &device_id).await {
                        Ok(true) => {}
                        Ok(false) => {
                            let _ = out.send(ConnectionDialogCmd::JellyfinAuth {
                                epoch,
                                result: Err(ConnectionErrorKind::QuickConnectDisabled),
                            });
                            return;
                        }
                        Err(e) => {
                            let _ = out.send(ConnectionDialogCmd::JellyfinAuth {
                                epoch,
                                result: Err(ConnectionErrorKind::from_error(&e)),
                            });
                            return;
                        }
                    }

                    // Step 2: initiate and show the code.
                    let (code, secret) =
                        match jf_auth::quick_connect_initiate(&url, &device_id).await {
                            Ok(v) => v,
                            Err(e) => {
                                let _ = out.send(ConnectionDialogCmd::JellyfinAuth {
                                    epoch,
                                    result: Err(ConnectionErrorKind::from_error(&e)),
                                });
                                return;
                            }
                        };
                    let _ = out.send(ConnectionDialogCmd::JellyfinCode { epoch, code });

                    // Step 3: poll until approved.
                    if let Err(e) = jf_auth::quick_connect_poll(&url, &device_id, &secret).await {
                        let _ = out.send(ConnectionDialogCmd::JellyfinAuth {
                            epoch,
                            result: Err(ConnectionErrorKind::from_error(&e)),
                        });
                        return;
                    }

                    // Step 4: exchange for a token.
                    let result =
                        jf_auth::authenticate_with_quick_connect(&url, &device_id, &secret)
                            .await
                            .map(|(token, user_id)| (url.clone(), token, user_id))
                            .map_err(|e| ConnectionErrorKind::from_error(&e));
                    let _ = out.send(ConnectionDialogCmd::JellyfinAuth { epoch, result });
                });
            }

            ConnectionDialogMsg::Cancel => {
                // Invalidate any in-flight poll so a late approval cannot persist.
                self.epoch += 1;
                let _ = sender.output(ConnectionDialogOutput::Cancelled);
                root.close();
            }
        }
    }

    fn update_cmd(
        &mut self,
        cmd: Self::CommandOutput,
        sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        match cmd {
            ConnectionDialogCmd::TokenReceived(result) => match result {
                Ok(token) => sender.input(ConnectionDialogMsg::AuthSucceeded(token)),
                Err(e) => sender.input(ConnectionDialogMsg::AuthFailed(e)),
            },
            ConnectionDialogCmd::ServersReady(result) => match result {
                Ok((token, servers)) => {
                    self.auth_token = Some(token);
                    sender.input(ConnectionDialogMsg::ServersFound(servers));
                }
                Err(e) => sender.input(ConnectionDialogMsg::ServerDiscoveryFailed(e)),
            },
            ConnectionDialogCmd::UriResolved(selected, token, name) => {
                self.spinner.set_visible(false);
                self.spinner.set_spinning(false);

                if let Some(selected) = selected {
                    info!("Connecting to server: {} at {}", name, selected.uri);
                    let _ = sender.output(ConnectionDialogOutput::ConnectionSaved {
                        url: selected.uri,
                        token,
                        name,
                        source_type: "plex".to_string(),
                        user_id: None,
                        is_remote: selected.is_remote,
                    });
                    root.close();
                } else {
                    self.status_label
                        .set_label("Could not reach any server connections. Check your network.");
                    self.status_label.set_css_classes(&["error"]);
                    self.sign_in_button.set_visible(true);
                    self.sign_in_button.set_label("Try Again");
                }
            }
            ConnectionDialogCmd::JellyfinCode { epoch, code } => {
                if epoch != self.epoch {
                    return;
                }
                self.jf_status_label
                    .set_label("Enter this code in Jellyfin to approve sign-in:");
                self.jf_status_label.set_css_classes(&[]);
                self.jf_code_label.set_label(&code);
                self.jf_code_label.set_visible(true);
            }
            ConnectionDialogCmd::JellyfinAuth { epoch, result } => {
                // Epoch guard: ignore completions from a poll the user has since
                // abandoned (view switched, dialog closed, or restarted).
                if epoch != self.epoch {
                    return;
                }
                self.jf_spinner.set_visible(false);
                self.jf_spinner.set_spinning(false);
                self.jf_code_label.set_visible(false);
                self.set_jellyfin_inputs_sensitive(true);

                match result {
                    Ok((url, token, user_id)) => {
                        let name = jellyfin_display_name(&url);
                        info!("Jellyfin connection saved: {} ({})", name, url);
                        let _ = sender.output(ConnectionDialogOutput::ConnectionSaved {
                            url,
                            token,
                            name,
                            source_type: "jellyfin".to_string(),
                            user_id: Some(user_id),
                            // Jellyfin has no Plex-style connection cap.
                            is_remote: false,
                        });
                        root.close();
                    }
                    Err(kind) => {
                        self.set_jf_error(connection_error_message(kind));
                    }
                }
            }
        }
    }
}

impl ConnectionDialog {
    fn start_jellyfin_busy(&self, message: &str) {
        self.set_jellyfin_inputs_sensitive(false);
        self.jf_status_label.set_css_classes(&[]);
        self.jf_status_label.set_label(message);
        self.jf_code_label.set_visible(false);
        self.jf_spinner.set_visible(true);
        self.jf_spinner.set_spinning(true);
    }

    fn set_jf_error(&self, message: &str) {
        self.jf_spinner.set_visible(false);
        self.jf_spinner.set_spinning(false);
        self.set_jellyfin_inputs_sensitive(true);
        self.jf_status_label.set_label(message);
        self.jf_status_label.set_css_classes(&["error"]);
    }

    fn set_jellyfin_inputs_sensitive(&self, sensitive: bool) {
        self.jf_url_entry.set_sensitive(sensitive);
        self.jf_username_entry.set_sensitive(sensitive);
        self.jf_password_entry.set_sensitive(sensitive);
        self.jf_signin_button.set_sensitive(sensitive);
        self.jf_quick_button.set_sensitive(sensitive);
    }

    fn reset_jellyfin_view(&self) {
        self.jf_spinner.set_visible(false);
        self.jf_spinner.set_spinning(false);
        self.jf_code_label.set_visible(false);
        self.jf_status_label.set_label("");
        self.jf_status_label.set_css_classes(&[]);
        self.set_jellyfin_inputs_sensitive(true);
    }
}

/// Build the initial backend-chooser view: pick Plex or Jellyfin.
fn build_chooser_view(sender: &ComponentSender<ConnectionDialog>) -> gtk::Box {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(20)
        .margin_start(32)
        .margin_end(32)
        .margin_top(32)
        .margin_bottom(32)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();

    let title = gtk::Label::builder()
        .label("Choose a media server to connect.")
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["title-4"])
        .build();
    content.append(&title);

    let plex_button = gtk::Button::builder()
        .label("Plex")
        .css_classes(["suggested-action", "pill"])
        .build();
    let sender_plex = sender.input_sender().clone();
    plex_button.connect_clicked(move |_| {
        let _ = sender_plex.send(ConnectionDialogMsg::ChoosePlex);
    });

    let jellyfin_button = gtk::Button::builder()
        .label("Jellyfin")
        .css_classes(["pill"])
        .build();
    let sender_jf = sender.input_sender().clone();
    jellyfin_button.connect_clicked(move |_| {
        let _ = sender_jf.send(ConnectionDialogMsg::ChooseJellyfin);
    });

    content.append(&plex_button);
    content.append(&jellyfin_button);
    content
}

/// Build the Plex sign-in view (the unchanged PIN flow). Returns the box plus
/// the widgets the component must hold onto.
fn build_plex_view(
    sender: &ComponentSender<ConnectionDialog>,
) -> (
    gtk::Box,
    gtk::Label,
    gtk::Spinner,
    gtk::Button,
    gtk::DropDown,
    gtk::Box,
) {
    let content_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(20)
        .margin_start(32)
        .margin_end(32)
        .margin_top(32)
        .margin_bottom(32)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();

    let icon = gtk::Image::builder()
        .icon_name("network-server-symbolic")
        .pixel_size(64)
        .margin_bottom(4)
        .build();

    let status_label = gtk::Label::builder()
        .label("Sign in with your Plex account to access your media libraries.")
        .wrap(true)
        .justify(gtk::Justification::Center)
        .build();

    let spinner = gtk::Spinner::builder()
        .visible(false)
        .height_request(32)
        .build();

    let sign_in_button = gtk::Button::builder()
        .label("Sign in to Plex")
        .css_classes(["suggested-action", "pill"])
        .halign(gtk::Align::Center)
        .margin_top(4)
        .build();
    let sender_signin = sender.input_sender().clone();
    sign_in_button.connect_clicked(move |_| {
        let _ = sender_signin.send(ConnectionDialogMsg::SignIn);
    });

    let server_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .visible(false)
        .build();

    let server_label = gtk::Label::builder()
        .label("Choose a server:")
        .halign(gtk::Align::Start)
        .css_classes(["heading"])
        .build();

    let server_dropdown = gtk::DropDown::builder().build();

    let confirm_button = gtk::Button::builder()
        .label("Connect")
        .css_classes(["suggested-action", "pill"])
        .halign(gtk::Align::Center)
        .build();
    let sender_confirm = sender.input_sender().clone();
    confirm_button.connect_clicked(move |_| {
        let _ = sender_confirm.send(ConnectionDialogMsg::ConfirmServer);
    });

    server_box.append(&server_label);
    server_box.append(&server_dropdown);
    server_box.append(&confirm_button);

    content_box.append(&icon);
    content_box.append(&status_label);
    content_box.append(&spinner);
    content_box.append(&sign_in_button);
    content_box.append(&server_box);

    (
        content_box,
        status_label,
        spinner,
        sign_in_button,
        server_dropdown,
        server_box,
    )
}

/// Widgets returned from the Jellyfin view builder.
struct JellyfinWidgets {
    container: gtk::Box,
    url_entry: gtk::Entry,
    username_entry: gtk::Entry,
    password_entry: gtk::PasswordEntry,
    insecure_label: gtk::Label,
    status_label: gtk::Label,
    spinner: gtk::Spinner,
    code_label: gtk::Label,
    signin_button: gtk::Button,
    quick_button: gtk::Button,
}

/// Build the Jellyfin sign-in view: URL + credentials + Quick Connect.
fn build_jellyfin_view(sender: &ComponentSender<ConnectionDialog>) -> JellyfinWidgets {
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_start(32)
        .margin_end(32)
        .margin_top(24)
        .margin_bottom(24)
        .build();

    let back_button = gtk::Button::builder()
        .label("Back")
        .halign(gtk::Align::Start)
        .css_classes(["flat"])
        .build();
    let sender_back = sender.input_sender().clone();
    back_button.connect_clicked(move |_| {
        let _ = sender_back.send(ConnectionDialogMsg::BackToChooser);
    });
    content.append(&back_button);

    let url_entry = gtk::Entry::builder()
        .placeholder_text("Server URL (e.g. https://jelly.example)")
        .build();
    let sender_url = sender.input_sender().clone();
    url_entry.connect_changed(move |_| {
        let _ = sender_url.send(ConnectionDialogMsg::JellyfinUrlChanged);
    });
    content.append(&url_entry);

    let insecure_label = gtk::Label::builder()
        .label("Warning: this server uses http://. Your username, password, and token will travel in cleartext.")
        .wrap(true)
        .visible(false)
        .halign(gtk::Align::Start)
        .css_classes(["warning"])
        .build();
    content.append(&insecure_label);

    let username_entry = gtk::Entry::builder().placeholder_text("Username").build();
    content.append(&username_entry);

    let password_entry = gtk::PasswordEntry::builder().show_peek_icon(true).build();
    content.append(&password_entry);

    let signin_button = gtk::Button::builder()
        .label("Sign In")
        .css_classes(["suggested-action"])
        .build();
    let sender_signin = sender.input_sender().clone();
    signin_button.connect_clicked(move |_| {
        let _ = sender_signin.send(ConnectionDialogMsg::JellyfinSignIn);
    });
    content.append(&signin_button);

    let quick_button = gtk::Button::builder()
        .label("Use Quick Connect")
        .css_classes(["flat"])
        .build();
    let sender_quick = sender.input_sender().clone();
    quick_button.connect_clicked(move |_| {
        let _ = sender_quick.send(ConnectionDialogMsg::JellyfinQuickConnect);
    });
    content.append(&quick_button);

    let code_label = gtk::Label::builder()
        .visible(false)
        .halign(gtk::Align::Center)
        .css_classes(["title-1"])
        .build();
    content.append(&code_label);

    let spinner = gtk::Spinner::builder()
        .visible(false)
        .height_request(24)
        .build();
    content.append(&spinner);

    let status_label = gtk::Label::builder()
        .label("")
        .wrap(true)
        .justify(gtk::Justification::Center)
        .build();
    content.append(&status_label);

    JellyfinWidgets {
        container: content,
        url_entry,
        username_entry,
        password_entry,
        insecure_label,
        status_label,
        spinner,
        code_label,
        signin_button,
        quick_button,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_server_url_adds_scheme_and_strips_slash() {
        assert_eq!(
            normalize_server_url("jelly.example/"),
            "https://jelly.example"
        );
    }

    #[test]
    fn normalize_server_url_preserves_http_scheme() {
        assert_eq!(normalize_server_url("http://host:8096"), "http://host:8096");
    }

    #[test]
    fn normalize_server_url_trims_whitespace_and_slash() {
        assert_eq!(normalize_server_url("  https://x/  "), "https://x");
    }

    #[test]
    fn is_insecure_url_true_for_http() {
        assert!(is_insecure_url("http://host:8096"));
        // No scheme defaults to https → secure.
        assert!(!is_insecure_url("jelly.example"));
        assert!(!is_insecure_url("https://jelly.example"));
    }

    #[test]
    fn connection_error_message_for_quick_connect_disabled() {
        assert_eq!(
            connection_error_message(ConnectionErrorKind::QuickConnectDisabled),
            "Quick Connect is disabled on this server. Use your username and password instead."
        );
    }

    #[test]
    fn connection_error_message_for_unreachable() {
        assert_eq!(
            connection_error_message(ConnectionErrorKind::Unreachable),
            "Could not reach the server. Check the URL and your network."
        );
    }

    #[test]
    fn connection_error_message_for_auth_failed() {
        assert_eq!(
            connection_error_message(ConnectionErrorKind::AuthFailed),
            "Incorrect username or password."
        );
    }

    #[test]
    fn error_kind_classifies_unauthorized_as_auth_failed() {
        assert_eq!(
            ConnectionErrorKind::from_error(&JellyfinError::Unauthorized),
            ConnectionErrorKind::AuthFailed
        );
    }

    #[test]
    fn jellyfin_display_name_uses_host() {
        assert_eq!(
            jellyfin_display_name("https://jelly.example"),
            "jelly.example"
        );
        assert_eq!(jellyfin_display_name("http://host:8096"), "host");
    }
}
