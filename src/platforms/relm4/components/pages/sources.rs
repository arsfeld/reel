use gtk::prelude::*;
use libadwaita as adw;
use relm4::factory::FactoryVecDeque;
use relm4::gtk;
use relm4::prelude::*;
use tracing::{debug, error, info};

use crate::db::connection::DatabaseConnection;
use crate::models::{
    SourceId,
    auth_provider::{ConnectionInfo, Source, SourceType},
};
use crate::services::commands::{
    Command,
    auth_commands::{LoadSourcesCommand, RemoveSourceCommand},
    sync_commands::SyncSourceCommand,
};

#[derive(Debug)]
pub struct SourcesPage {
    db: DatabaseConnection,
    sources: Vec<Source>,
    is_loading: bool,
    syncing_sources: std::collections::HashSet<SourceId>,
}

#[derive(Debug)]
pub enum SourcesPageInput {
    /// Load sources data
    LoadData,
    /// Sources loaded from database
    SourcesLoaded(Vec<Source>),
    /// Add a new source
    AddSource,
    /// Remove a source
    RemoveSource(SourceId),
    /// Source removed successfully
    SourceRemoved(SourceId),
    /// Test connection for a source
    TestConnection(SourceId),
    /// Sync a source
    SyncSource(SourceId),
    /// Sync completed
    SyncCompleted(SourceId, Result<(), String>),
    /// Error occurred
    Error(String),
}

#[derive(Debug)]
pub enum SourcesPageOutput {
    /// Open authentication dialog for adding a source
    OpenAuthDialog,
}

#[derive(Debug)]
pub struct SourceListItem {
    source: Source,
    is_syncing: bool,
}

#[derive(Debug)]
pub enum SourceListItemInput {
    Sync,
    TestConnection,
    Remove,
}

#[relm4::factory(pub)]
impl FactoryComponent for SourceListItem {
    type Init = Source;
    type Input = SourceListItemInput;
    type Output = SourceItemAction;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        gtk::ListBoxRow {
            set_activatable: false,
            set_selectable: false,

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_margin_all: 12,
                set_spacing: 12,

                // Icon
                gtk::Image {
                    set_icon_name: Some(match self.source.source_type {
                        SourceType::PlexServer { .. } => "tv-symbolic",
                        SourceType::JellyfinServer => "folder-videos-symbolic",
                        _ => "folder-symbolic",
                    }),
                    set_pixel_size: 40,
                    add_css_class: "dim-label",
                },

                // Source info
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_hexpand: true,
                    set_spacing: 4,

                    gtk::Label {
                        set_text: &self.source.name,
                        set_halign: gtk::Align::Start,
                        add_css_class: "heading",
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 8,

                        gtk::Label {
                            set_text: &format!("{} • {}",
                                format!("{:?}", self.source.source_type).to_uppercase(),
                                self.source.connection_info.primary_url.as_ref().unwrap_or(&"No URL".to_string())
                            ),
                            set_halign: gtk::Align::Start,
                            add_css_class: "dim-label",
                            add_css_class: "caption",
                        },

                        // Connection status
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 4,

                            gtk::Image {
                                set_icon_name: Some(if self.source.connection_info.is_online {
                                    "emblem-ok-symbolic"
                                } else {
                                    "network-offline-symbolic"
                                }),
                                set_pixel_size: 16,
                                add_css_class: if self.source.connection_info.is_online {
                                    "success"
                                } else {
                                    "error"
                                },
                            },

                            gtk::Label {
                                set_text: if self.source.connection_info.is_online { "Connected" } else { "Offline" },
                                add_css_class: "caption",
                                add_css_class: if self.source.connection_info.is_online {
                                    "success"
                                } else {
                                    "error"
                                },
                            },
                        },
                    },
                },

                // Action buttons
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 6,

                    // Sync button
                    gtk::Button {
                        set_icon_name: "view-refresh-symbolic",
                        set_tooltip_text: Some("Sync Source"),
                        add_css_class: "flat",
                        add_css_class: "circular",
                        #[watch]
                        set_sensitive: !self.is_syncing,
                        #[watch]
                        set_icon_name: if self.is_syncing {
                            "content-loading-symbolic"
                        } else {
                            "view-refresh-symbolic"
                        },
                        connect_clicked => SourceListItemInput::Sync,
                    },

                    // Test connection button
                    gtk::Button {
                        set_icon_name: "network-transmit-receive-symbolic",
                        set_tooltip_text: Some("Test Connection"),
                        add_css_class: "flat",
                        add_css_class: "circular",
                        connect_clicked => SourceListItemInput::TestConnection,
                    },

                    // Remove button
                    gtk::Button {
                        set_icon_name: "user-trash-symbolic",
                        set_tooltip_text: Some("Remove Source"),
                        add_css_class: "flat",
                        add_css_class: "circular",
                        add_css_class: "error",
                        connect_clicked => SourceListItemInput::Remove,
                    },
                },
            },
        }
    }

    fn init_model(source: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            source,
            is_syncing: false,
        }
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        match msg {
            SourceListItemInput::Sync => {
                sender
                    .output(SourceItemAction::Sync(SourceId::from(
                        self.source.id.clone(),
                    )))
                    .unwrap();
            }
            SourceListItemInput::TestConnection => {
                sender
                    .output(SourceItemAction::TestConnection(SourceId::from(
                        self.source.id.clone(),
                    )))
                    .unwrap();
            }
            SourceListItemInput::Remove => {
                sender
                    .output(SourceItemAction::Remove(SourceId::from(
                        self.source.id.clone(),
                    )))
                    .unwrap();
            }
        }
    }
}

#[derive(Debug)]
pub enum SourceItemAction {
    Sync(SourceId),
    TestConnection(SourceId),
    Remove(SourceId),
}

#[relm4::component(pub async)]
impl AsyncComponent for SourcesPage {
    type Init = DatabaseConnection;
    type Input = SourcesPageInput;
    type Output = SourcesPageOutput;
    type CommandOutput = SourcesPageInput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 0,
            add_css_class: "background",

            // Header
            adw::HeaderBar {
                set_title_widget: Some(&adw::WindowTitle::new("Sources", "Manage your media servers")),

                pack_end = &gtk::Button {
                    set_icon_name: "list-add-symbolic",
                    set_tooltip_text: Some("Add Source"),
                    add_css_class: "suggested-action",
                    connect_clicked => SourcesPageInput::AddSource,
                },
            },

            // Content
            gtk::ScrolledWindow {
                set_vexpand: true,
                set_hscrollbar_policy: gtk::PolicyType::Never,

                #[watch]
                set_visible: !model.is_loading && !model.sources.is_empty(),

                adw::Clamp {
                    set_maximum_size: 800,
                    set_margin_all: 24,
                    #[wrap(Some)]
                    set_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 24,

                        // Sources list
                        #[local_ref]
                        sources_list -> gtk::ListBox {
                            add_css_class: "boxed-list",
                            set_selection_mode: gtk::SelectionMode::None,
                        },

                        // Info box
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 12,
                            set_margin_all: 12,
                            add_css_class: "card",

                            gtk::Label {
                                set_text: "About Sources",
                                set_halign: gtk::Align::Start,
                                add_css_class: "heading",
                            },

                            gtk::Label {
                                set_text: "Sources are your media servers like Plex or Jellyfin. You can connect to multiple servers and sync their libraries for offline access.",
                                set_wrap: true,
                                set_halign: gtk::Align::Start,
                                add_css_class: "dim-label",
                            },
                        },
                    },
                },
            },

            // Loading state
            #[name(loading_spinner)]
            gtk::Spinner {
                #[watch]
                set_visible: model.is_loading,
                set_spinning: true,
                set_vexpand: true,
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
                set_size_request: (48, 48),
            },

            // Empty state
            adw::StatusPage {
                #[watch]
                set_visible: !model.is_loading && model.sources.is_empty(),
                set_icon_name: Some("network-server-symbolic"),
                set_title: "No Sources",
                set_description: Some("Add a media server to get started"),
                set_vexpand: true,

                #[wrap(Some)]
                set_child = &gtk::Button {
                    set_label: "Add Source",
                    set_halign: gtk::Align::Center,
                    add_css_class: "pill",
                    add_css_class: "suggested-action",
                    connect_clicked => SourcesPageInput::AddSource,
                },
            }
        }
    }

    async fn init(
        db: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let model = Self {
            db: db.clone(),
            sources: Vec::new(),
            is_loading: true,
            syncing_sources: std::collections::HashSet::new(),
        };

        let sources_list = gtk::ListBox::new();
        sources_list.add_css_class("boxed-list");
        sources_list.set_selection_mode(gtk::SelectionMode::None);

        let widgets = view_output!();

        // Load sources on init
        sender.input(SourcesPageInput::LoadData);

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            SourcesPageInput::LoadData => {
                info!("Loading sources");
                self.is_loading = true;

                let db = self.db.clone();
                sender.oneshot_command(async move {
                    let command = LoadSourcesCommand { db };
                    match command.execute().await {
                        Ok(sources) => SourcesPageInput::SourcesLoaded(sources),
                        Err(e) => {
                            error!("Failed to load sources: {}", e);
                            SourcesPageInput::Error(e.to_string())
                        }
                    }
                });
            }

            SourcesPageInput::SourcesLoaded(sources) => {
                info!("Loaded {} sources", sources.len());
                self.sources = sources;
                self.is_loading = false;
            }

            SourcesPageInput::AddSource => {
                info!("Opening auth dialog to add source");
                sender.output(SourcesPageOutput::OpenAuthDialog).unwrap();
            }

            SourcesPageInput::RemoveSource(source_id) => {
                info!("Removing source: {}", source_id);

                let db = self.db.clone();
                let source_id_clone = source_id.clone();
                sender.oneshot_command(async move {
                    let command = RemoveSourceCommand {
                        db,
                        source_id: source_id_clone.clone(),
                    };
                    match command.execute().await {
                        Ok(_) => SourcesPageInput::SourceRemoved(source_id_clone),
                        Err(e) => {
                            error!("Failed to remove source: {}", e);
                            SourcesPageInput::Error(e.to_string())
                        }
                    }
                });
            }

            SourcesPageInput::SourceRemoved(source_id) => {
                info!("Source removed: {}", source_id);
                self.sources.retain(|s| s.id != source_id.to_string());
            }

            SourcesPageInput::TestConnection(source_id) => {
                info!("Testing connection for source: {}", source_id);
                // TODO: Implement connection testing
                sender.input(SourcesPageInput::Error(
                    "Connection testing not yet implemented".to_string(),
                ));
            }

            SourcesPageInput::SyncSource(source_id) => {
                info!("Syncing source: {}", source_id);
                self.syncing_sources.insert(source_id.clone());

                // TODO: SyncSourceCommand requires a backend instance
                // For now, just report completion
                sender.input(SourcesPageInput::SyncCompleted(
                    source_id.clone(),
                    Err("Sync not yet implemented - requires backend instance".to_string()),
                ));
            }

            SourcesPageInput::SyncCompleted(source_id, result) => {
                self.syncing_sources.remove(&source_id);
                match result {
                    Ok(_) => info!("Source synced successfully: {}", source_id),
                    Err(e) => error!("Failed to sync source {}: {}", source_id, e),
                }
                // Reload sources to get updated status
                sender.input(SourcesPageInput::LoadData);
            }

            SourcesPageInput::Error(msg) => {
                error!("Error: {}", msg);
                // TODO: Show error in UI (toast notification)
            }
        }
    }

    async fn update_cmd(
        &mut self,
        msg: Self::CommandOutput,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        // Command outputs are SourcesPageInput messages, so forward to update
        self.update(msg, sender, _root).await;
    }
}
