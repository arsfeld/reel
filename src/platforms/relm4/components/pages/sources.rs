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
use crate::platforms::relm4::components::shared::broker::{BROKER, BrokerMessage, SourceMessage};
use crate::services::commands::{
    Command,
    auth_commands::{LoadSourcesCommand, RemoveSourceCommand},
    sync_commands::SyncSourceCommand,
};

#[derive(Debug)]
pub struct SourcesPage {
    db: DatabaseConnection,
    sources: Vec<Source>,
    sources_factory: FactoryVecDeque<SourceListItem>,
    is_loading: bool,
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
    /// Sync a source
    SyncSource(SourceId),
    /// Sync completed
    SyncCompleted(SourceId, Result<(), String>),
    /// Message from the broker
    BrokerMsg(BrokerMessage),
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
    sync_progress: Option<(usize, usize)>,
}

#[derive(Debug)]
pub enum SourceListItemInput {
    Sync,
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
                    set_pixel_size: 32,
                },

                // Source info
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_hexpand: true,
                    set_spacing: 2,

                    gtk::Label {
                        set_text: &self.source.name,
                        set_halign: gtk::Align::Start,
                        add_css_class: "heading",
                    },

                    gtk::Label {
                        set_text: &format!("{} • {}",
                            match self.source.source_type {
                                SourceType::PlexServer { .. } => "Plex",
                                SourceType::JellyfinServer => "Jellyfin",
                                _ => "Local",
                            },
                            self.source.connection_info.primary_url.as_ref()
                                .and_then(|url| url.split("://").nth(1))
                                .and_then(|url| url.split('/').next())
                                .unwrap_or("No URL")
                        ),
                        set_halign: gtk::Align::Start,
                        add_css_class: "dim-label",
                        add_css_class: "caption",
                    },
                },

                // Connection status indicator
                gtk::Image {
                    set_icon_name: Some(if self.source.connection_info.is_online {
                        "emblem-ok-symbolic"
                    } else {
                        "network-offline-symbolic"
                    }),
                    set_pixel_size: 16,
                    set_tooltip_text: Some(if self.source.connection_info.is_online {
                        "Connected"
                    } else {
                        "Offline"
                    }),
                    add_css_class: if self.source.connection_info.is_online {
                        "success"
                    } else {
                        "dim-label"
                    },
                },

                // Action buttons
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 6,

                    // Sync button
                    gtk::Button {
                        set_icon_name: "view-refresh-symbolic",
                        set_tooltip_text: Some("Sync Library"),
                        add_css_class: "flat",
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

                    // Remove button
                    gtk::Button {
                        set_icon_name: "user-trash-symbolic",
                        set_tooltip_text: Some("Remove"),
                        add_css_class: "flat",
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
            sync_progress: None,
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

            // Section header
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_margin_top: 12,
                set_margin_bottom: 12,
                set_margin_start: 24,
                set_margin_end: 24,

                gtk::Label {
                    set_text: "Servers & Accounts",
                    set_halign: gtk::Align::Start,
                    add_css_class: "title-2",
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
            },
        }
    }

    async fn init(
        db: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let sources_list = gtk::ListBox::new();
        sources_list.add_css_class("boxed-list");
        sources_list.set_selection_mode(gtk::SelectionMode::None);

        let mut sources_factory = FactoryVecDeque::<SourceListItem>::builder()
            .launch(sources_list.clone())
            .forward(sender.input_sender(), |output| match output {
                SourceItemAction::Sync(id) => SourcesPageInput::SyncSource(id),
                SourceItemAction::Remove(id) => SourcesPageInput::RemoveSource(id),
            });

        let model = Self {
            db: db.clone(),
            sources: Vec::new(),
            sources_factory,
            is_loading: true,
        };

        let widgets = view_output!();

        // Load sources on init
        sender.input(SourcesPageInput::LoadData);

        // Subscribe to MessageBroker
        let broker_sender = sender.input_sender().clone();
        relm4::spawn(async move {
            // Create a channel to forward broker messages
            let (tx, mut rx) = relm4::channel::<BrokerMessage>();

            // Subscribe to the broker with our channel
            BROKER.subscribe("SourcesPage".to_string(), tx).await;

            // Forward messages to the component
            while let Some(msg) = rx.recv().await {
                broker_sender
                    .send(SourcesPageInput::BrokerMsg(msg))
                    .unwrap();
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
                self.sources = sources.clone();
                self.is_loading = false;

                // Clear and repopulate the factory
                let mut factory_guard = self.sources_factory.guard();
                factory_guard.clear();
                for source in sources {
                    factory_guard.push_back(source);
                }
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

                // Remove from factory
                let mut factory_guard = self.sources_factory.guard();
                let index_to_remove = factory_guard
                    .iter()
                    .position(|s| s.source.id == source_id.to_string());
                if let Some(index) = index_to_remove {
                    factory_guard.remove(index);
                }
            }

            SourcesPageInput::BrokerMsg(msg) => {
                match msg {
                    BrokerMessage::Source(SourceMessage::SyncStarted {
                        source_id,
                        total_items,
                    }) => {
                        info!("Sync started for source: {}", source_id);
                        // Update UI to show sync in progress
                        let mut factory_guard = self.sources_factory.guard();
                        for item in factory_guard.iter_mut() {
                            if item.source.id == source_id {
                                item.is_syncing = true;
                                if let Some(total) = total_items {
                                    item.sync_progress = Some((0, total));
                                }
                            }
                        }
                    }
                    BrokerMessage::Source(SourceMessage::SyncProgress {
                        source_id,
                        current,
                        total,
                    }) => {
                        // Update sync progress
                        let mut factory_guard = self.sources_factory.guard();
                        for item in factory_guard.iter_mut() {
                            if item.source.id == source_id {
                                item.sync_progress = Some((current, total));
                            }
                        }
                    }
                    BrokerMessage::Source(SourceMessage::SyncCompleted { source_id, .. }) => {
                        info!("Sync completed for source: {}", source_id);
                        // Update UI to show sync completed
                        let mut factory_guard = self.sources_factory.guard();
                        for item in factory_guard.iter_mut() {
                            if item.source.id == source_id {
                                item.is_syncing = false;
                                item.sync_progress = None;
                            }
                        }
                        // Reload sources to get updated data
                        sender.input(SourcesPageInput::LoadData);
                    }
                    BrokerMessage::Source(SourceMessage::SyncError { source_id, error }) => {
                        error!("Sync error for source {}: {}", source_id, error);
                        // Update UI to show sync failed
                        let mut factory_guard = self.sources_factory.guard();
                        for item in factory_guard.iter_mut() {
                            if item.source.id == source_id {
                                item.is_syncing = false;
                            }
                        }
                        sender.input(SourcesPageInput::Error(format!("Sync failed: {}", error)));
                    }
                    _ => {}
                }
            }

            SourcesPageInput::SyncSource(source_id) => {
                info!("Starting sync for source: {}", source_id);
                // The broker messages will handle the UI state updates

                let db = self.db.clone();
                let source_id_clone = source_id.clone();

                sender.oneshot_command(async move {
                    use crate::services::core::backend::BackendService;

                    // Use BackendService to sync the source
                    match BackendService::sync_source(&db, &source_id_clone).await {
                        Ok(sync_result) => {
                            info!(
                                "Source sync completed successfully: {} items synced",
                                sync_result.items_synced
                            );
                            SourcesPageInput::SyncCompleted(source_id_clone, Ok(()))
                        }
                        Err(e) => {
                            error!("Source sync failed: {}", e);
                            SourcesPageInput::SyncCompleted(source_id_clone, Err(e.to_string()))
                        }
                    }
                });
            }

            SourcesPageInput::SyncCompleted(source_id, result) => {
                // Don't manually track syncing here - let the broker messages handle it
                match result {
                    Ok(_) => info!("Source sync command completed: {}", source_id),
                    Err(e) => error!("Source sync command failed {}: {}", source_id, e),
                }
            }

            SourcesPageInput::Error(msg) => {
                error!("Error: {}", msg);
                // For now, just log the error. Toast implementation would require
                // restructuring the view with an overlay wrapper
                tracing::error!("Source operation failed: {}", msg);
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
