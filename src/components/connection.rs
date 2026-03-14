use adw::prelude::*;
use gtk4::prelude::*;
use libadwaita as adw;
use relm4::prelude::*;
use tracing::info;

use crate::services::plex::auth;

#[allow(dead_code)]
pub struct ConnectionDialog {
    client_id: String,
    auth_token: Option<String>,
    servers: Vec<auth::PlexResource>,
    // Widgets
    status_label: gtk4::Label,
    spinner: gtk4::Spinner,
    sign_in_button: gtk4::Button,
    server_dropdown: gtk4::DropDown,
    server_box: gtk4::Box,
}

pub enum ConnectionDialogMsg {
    SignIn,
    AuthSucceeded(String),
    AuthFailed(String),
    ServersFound(Vec<auth::PlexResource>),
    ServerDiscoveryFailed(String),
    ConfirmServer,
    Cancel,
}

impl std::fmt::Debug for ConnectionDialogMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SignIn => write!(f, "SignIn"),
            Self::AuthSucceeded(_) => write!(f, "AuthSucceeded(..)"),
            Self::AuthFailed(e) => write!(f, "AuthFailed({e})"),
            Self::ServersFound(s) => write!(f, "ServersFound({} servers)", s.len()),
            Self::ServerDiscoveryFailed(e) => write!(f, "ServerDiscoveryFailed({e})"),
            Self::ConfirmServer => write!(f, "ConfirmServer"),
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
    },
    Cancelled,
}

#[derive(Debug)]
pub enum ConnectionDialogCmd {
    TokenReceived(Result<String, String>),
    ServersReady(Result<(String, Vec<auth::PlexResource>), String>),
}

#[relm4::component(pub)]
impl Component for ConnectionDialog {
    type Init = String; // client_id
    type Input = ConnectionDialogMsg;
    type Output = ConnectionDialogOutput;
    type CommandOutput = ConnectionDialogCmd;

    view! {
        #[root]
        adw::Window {
            set_title: Some("Sign in to Plex"),
            set_default_width: 450,
            set_default_height: 300,
            set_modal: true,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn init(
        client_id: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = view_output!();

        let toolbar = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();

        let cancel_button = gtk4::Button::builder().label("Cancel").build();
        let sender_cancel = sender.input_sender().clone();
        cancel_button.connect_clicked(move |_| {
            let _ = sender_cancel.send(ConnectionDialogMsg::Cancel);
        });
        header.pack_start(&cancel_button);
        toolbar.add_top_bar(&header);

        let content_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(20)
            .margin_start(32)
            .margin_end(32)
            .margin_top(32)
            .margin_bottom(32)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .build();

        let icon = gtk4::Image::builder()
            .icon_name("network-server-symbolic")
            .pixel_size(64)
            .margin_bottom(4)
            .build();

        let status_label = gtk4::Label::builder()
            .label("Sign in with your Plex account to access your media libraries.")
            .wrap(true)
            .justify(gtk4::Justification::Center)
            .build();

        let spinner = gtk4::Spinner::builder()
            .visible(false)
            .height_request(32)
            .build();

        let sign_in_button = gtk4::Button::builder()
            .label("Sign in to Plex")
            .css_classes(["suggested-action", "pill"])
            .halign(gtk4::Align::Center)
            .margin_top(4)
            .build();
        let sender_signin = sender.input_sender().clone();
        sign_in_button.connect_clicked(move |_| {
            let _ = sender_signin.send(ConnectionDialogMsg::SignIn);
        });

        // Server picker (shown when multiple servers)
        let server_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(12)
            .visible(false)
            .build();

        let server_label = gtk4::Label::builder()
            .label("Choose a server:")
            .halign(gtk4::Align::Start)
            .css_classes(["heading"])
            .build();

        let server_dropdown = gtk4::DropDown::builder().build();

        let confirm_button = gtk4::Button::builder()
            .label("Connect")
            .css_classes(["suggested-action", "pill"])
            .halign(gtk4::Align::Center)
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

        toolbar.set_content(Some(&content_box));
        root.set_content(Some(&toolbar));

        let model = Self {
            client_id,
            auth_token: None,
            servers: Vec::new(),
            status_label,
            spinner,
            sign_in_button,
            server_dropdown,
            server_box,
        };

        ComponentParts { model, widgets }
    }

    #[allow(clippy::too_many_lines)]
    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            ConnectionDialogMsg::SignIn => {
                self.sign_in_button.set_visible(false);
                self.spinner.set_visible(true);
                self.spinner.set_spinning(true);
                self.status_label.set_css_classes(&[]);
                self.status_label
                    .set_label("Opening your browser...\nSign in to your Plex account.");

                let client_id = self.client_id.clone();
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

                let client_id = self.client_id.clone();
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
                let string_list = gtk4::StringList::new(&[] as &[&str]);
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
                    && let Some(uri) = auth::best_server_uri(server)
                    && let Some(ref token) = self.auth_token
                {
                    info!("Connecting to server: {} at {}", server.name, uri);
                    let _ = sender.output(ConnectionDialogOutput::ConnectionSaved {
                        url: uri,
                        token: token.clone(),
                        name: server.name.clone(),
                    });
                    root.close();
                }
            }

            ConnectionDialogMsg::Cancel => {
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
                    self.auth_token = Some(token.clone());

                    if servers.len() == 1 {
                        // Auto-connect
                        let server = &servers[0];
                        if let Some(uri) = auth::best_server_uri(server) {
                            let _ = sender.output(ConnectionDialogOutput::ConnectionSaved {
                                url: uri,
                                token,
                                name: server.name.clone(),
                            });
                            root.close();
                            return;
                        }
                    }

                    sender.input(ConnectionDialogMsg::ServersFound(servers));
                }
                Err(e) => sender.input(ConnectionDialogMsg::ServerDiscoveryFailed(e)),
            },
        }
    }
}
