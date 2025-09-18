use gtk::prelude::*;
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender, FactoryVecDeque};
use relm4::prelude::*;
use relm4::{Component, ComponentParts, ComponentSender, gtk};
use std::collections::{HashMap, HashSet};
use tracing::{debug, error};

use crate::db::connection::DatabaseConnection;
use crate::models::auth_provider::{Source, SourceType};
use crate::models::{Library, LibraryId, LibraryType, SourceId};
use crate::services::commands::{Command, auth_commands::LoadSourcesCommand};
use crate::services::core::media::MediaService;
use crate::ui::shared::broker::{BROKER, BrokerMessage, SourceMessage};

/// Connection state for sources
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Source is connected and syncing works
    Connected,
    /// Source is reachable but sync failed
    SyncFailed,
    /// Source is not reachable
    Disconnected,
}

// Messages for the sidebar component
#[derive(Debug)]
pub enum SidebarInput {
    /// Refresh sources from database
    RefreshSources,
    /// Sources loaded from database
    SourcesLoaded(Vec<Source>),
    /// Libraries loaded for a source
    LibrariesLoaded(SourceId, Vec<Library>),
    /// Navigate to home
    NavigateHome,
    /// Navigate to library
    NavigateToLibrary(LibraryId),
    /// Navigate to source management
    ManageSources,
    /// Update connection status
    UpdateConnectionStatus(String),
    /// Update specific source connection status
    UpdateSourceConnectionStatus {
        source_id: SourceId,
        state: ConnectionState,
    },
    /// Broker message received
    BrokerMsg(BrokerMessage),
}

#[derive(Debug)]
pub enum SidebarOutput {
    /// Navigate to home
    NavigateToHome,
    /// Navigate to library
    NavigateToLibrary(LibraryId),
    /// Navigate to source management
    NavigateToSources,
}

// Source group factory component
#[derive(Debug)]
pub struct SourceGroup {
    source: Source,
    libraries: Vec<Library>,
    is_loading: bool,
    is_expanded: bool,
    db: DatabaseConnection,
    syncing_libraries: HashSet<String>,
    connection_state: ConnectionState,
}

impl SourceGroup {
    fn update_library_list(&self, library_list: &gtk::ListBox) {
        // Clear existing children
        while let Some(child) = library_list.first_child() {
            library_list.remove(&child);
        }

        // Add libraries from the actual data
        for library in &self.libraries {
            let row = gtk::ListBoxRow::new();
            row.set_activatable(true);

            // Store the library ID as data on the row
            unsafe {
                row.set_data("library_id", library.id.clone());
            }

            let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            hbox.add_css_class("library-item");

            // Icon based on library type
            let icon_name = match library.library_type {
                LibraryType::Movies => "video-x-generic-symbolic",
                LibraryType::Shows => "video-display-symbolic",
                LibraryType::Music => "audio-x-generic-symbolic",
                LibraryType::Photos => "image-x-generic-symbolic",
                LibraryType::Mixed => "folder-symbolic",
            };
            let icon = gtk::Image::from_icon_name(icon_name);
            icon.set_pixel_size(16);
            hbox.append(&icon);

            // Library info box
            let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
            vbox.set_hexpand(true);

            let name_label = gtk::Label::new(Some(&library.title));
            name_label.set_halign(gtk::Align::Start);
            vbox.append(&name_label);

            // Display the actual item count from the library model
            let count_text = format!("{} items", library.item_count);
            let count_label = gtk::Label::new(Some(&count_text));
            count_label.set_halign(gtk::Align::Start);
            count_label.add_css_class("dim-label");
            count_label.add_css_class("caption");
            vbox.append(&count_label);

            hbox.append(&vbox);

            // Add spinner if this library is syncing
            if self.syncing_libraries.contains(&library.id) {
                let spinner = gtk::Spinner::new();
                spinner.set_spinning(true);
                hbox.append(&spinner);
            }

            row.set_child(Some(&hbox));
            library_list.append(&row);
        }

        // If no libraries, show a placeholder
        if self.libraries.is_empty() {
            let row = gtk::ListBoxRow::new();
            row.set_activatable(false);

            let label = gtk::Label::new(Some("No libraries found"));
            label.set_margin_top(4);
            label.set_margin_bottom(4);
            label.add_css_class("dim-label");
            row.set_child(Some(&label));
            library_list.append(&row);
        }
    }
}

#[derive(Debug)]
pub enum SourceGroupInput {
    /// Load libraries for this source
    LoadLibraries,
    /// Libraries loaded
    LibrariesLoaded(Vec<Library>),
    /// Refresh this source
    Refresh,
    /// Toggle expanded state
    ToggleExpanded,
    /// Reload libraries from database (e.g., after sync)
    ReloadLibraries,
    /// Update connection status
    UpdateConnectionStatus(ConnectionState),
    /// Library sync started
    LibrarySyncStarted(String),
    /// Library sync completed
    LibrarySyncCompleted(String),
}

#[derive(Debug)]
pub enum SourceGroupOutput {
    /// Navigate to library
    NavigateToLibrary(LibraryId),
}

#[relm4::factory(pub)]
impl FactoryComponent for SourceGroup {
    type Init = (Source, DatabaseConnection);
    type Input = SourceGroupInput;
    type Output = SourceGroupOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        root = gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 0,
            add_css_class: "source-group",

            gtk::Button {
                add_css_class: "flat",
                add_css_class: "source-header",
                connect_clicked => SourceGroupInput::ToggleExpanded,

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 12,
                    set_margin_top: 8,
                    set_margin_bottom: 8,
                    set_margin_start: 8,
                    set_margin_end: 8,

                    gtk::Image {
                        #[watch]
                        set_icon_name: Some(match &self.source.source_type {
                            SourceType::PlexServer { .. } => "network-server-symbolic",
                            SourceType::JellyfinServer => "network-workgroup-symbolic",
                            SourceType::LocalFolder { .. } => "folder-symbolic",
                            SourceType::NetworkShare { .. } => "folder-remote-symbolic",
                        }),
                        set_pixel_size: 16,
                    },

                    gtk::Label {
                        set_text: &self.source.name,
                        set_halign: gtk::Align::Start,
                        set_hexpand: true,
                    },

                    // Connection status indicator
                    gtk::Image {
                        #[watch]
                        set_icon_name: Some(match self.connection_state {
                            ConnectionState::Connected => "emblem-ok-symbolic",
                            ConnectionState::SyncFailed => "dialog-warning-symbolic",
                            ConnectionState::Disconnected => "network-offline-symbolic",
                        }),
                        set_pixel_size: 16,
                        #[watch]
                        set_css_classes: &[match self.connection_state {
                            ConnectionState::Connected => "success",
                            ConnectionState::SyncFailed => "warning",
                            ConnectionState::Disconnected => "error",
                        }],
                        #[watch]
                        set_tooltip_text: Some(match self.connection_state {
                            ConnectionState::Connected => "Connected and synced",
                            ConnectionState::SyncFailed => "Connected but sync failed",
                            ConnectionState::Disconnected => "Disconnected",
                        }),
                    },

                    gtk::Image {
                        set_icon_name: Some("go-next-symbolic"),
                        set_pixel_size: 12,
                        #[watch]
                        add_css_class: if self.is_expanded { "source-expand-icon source-expanded" } else { "source-expand-icon" },
                    },
                }
            },

            #[local_ref]
            library_list -> gtk::ListBox {
                set_selection_mode: gtk::SelectionMode::None,
                add_css_class: "library-list",
                #[watch]
                set_visible: self.is_expanded,
            }
        }
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, sender: FactorySender<Self>) -> Self {
        let (source, db) = init;
        let source_clone = source.clone();
        let db_clone = db.clone();
        let sender_clone = sender.clone();

        // Load libraries for this source asynchronously
        relm4::spawn(async move {
            let source_id = SourceId::new(source_clone.id.clone());
            match MediaService::get_libraries_for_source(&db_clone, &source_id).await {
                Ok(libraries) => {
                    debug!(
                        "Loaded {} libraries for source {}",
                        libraries.len(),
                        source_clone.name
                    );
                    for lib in &libraries {
                        debug!("Library '{}': item_count = {}", lib.title, lib.item_count);
                    }
                    sender_clone.input(SourceGroupInput::LibrariesLoaded(libraries));
                }
                Err(e) => {
                    error!(
                        "Failed to load libraries for source {}: {}",
                        source_clone.name, e
                    );
                    sender_clone.input(SourceGroupInput::LibrariesLoaded(Vec::new()));
                }
            }
        });

        Self {
            source,
            libraries: Vec::new(),
            is_loading: true,
            is_expanded: true, // Start expanded by default
            db,
            syncing_libraries: HashSet::new(),
            connection_state: ConnectionState::Connected, // Assume connected initially
        }
    }

    fn init_widgets(
        &mut self,
        _index: &DynamicIndex,
        root: Self::Root,
        _returned_widget: &gtk::Widget,
        sender: FactorySender<Self>,
    ) -> Self::Widgets {
        let library_list = gtk::ListBox::new();
        library_list.set_selection_mode(gtk::SelectionMode::None);
        library_list.add_css_class("library-list");

        // Connect row activation to send proper library IDs
        let sender_clone = sender.clone();
        library_list.connect_row_activated(move |_, row| {
            // Get the library ID from the row data
            unsafe {
                if let Some(library_id) = row.data::<String>("library_id") {
                    let lib_id = LibraryId::new(library_id.as_ref().clone());
                    sender_clone
                        .output(SourceGroupOutput::NavigateToLibrary(lib_id))
                        .unwrap_or_else(|_| error!("Failed to send library navigation"));
                }
            }
        });

        let widgets = view_output!();

        // Initially populate with any libraries we already have
        self.update_library_list(&widgets.library_list);

        widgets
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        msg: Self::Input,
        sender: FactorySender<Self>,
    ) {
        match msg {
            SourceGroupInput::LoadLibraries => {
                self.is_loading = true;
            }
            SourceGroupInput::LibrariesLoaded(libraries) => {
                self.libraries = libraries;
                self.is_loading = false;
                // Update the library list widget
                self.update_library_list(&widgets.library_list);
            }
            SourceGroupInput::Refresh => {
                debug!("Refreshing source: {}", self.source.name);
                // Trigger library reload
                sender.input(SourceGroupInput::ReloadLibraries);
            }
            SourceGroupInput::ToggleExpanded => {
                self.is_expanded = !self.is_expanded;
            }
            SourceGroupInput::ReloadLibraries => {
                debug!("Reloading libraries for source: {}", self.source.name);
                self.is_loading = true;

                // Load libraries from database
                let source_clone = self.source.clone();
                let db_clone = self.db.clone();
                let sender_clone = sender.clone();

                relm4::spawn(async move {
                    let source_id = SourceId::new(source_clone.id.clone());
                    match MediaService::get_libraries_for_source(&db_clone, &source_id).await {
                        Ok(libraries) => {
                            debug!(
                                "Reloaded {} libraries for source {}",
                                libraries.len(),
                                source_clone.name
                            );
                            for lib in &libraries {
                                debug!(
                                    "Reloaded library '{}': item_count = {}",
                                    lib.title, lib.item_count
                                );
                            }
                            sender_clone.input(SourceGroupInput::LibrariesLoaded(libraries));
                        }
                        Err(e) => {
                            error!(
                                "Failed to reload libraries for source {}: {}",
                                source_clone.name, e
                            );
                            sender_clone.input(SourceGroupInput::LibrariesLoaded(Vec::new()));
                        }
                    }
                });
            }
            SourceGroupInput::LibrarySyncStarted(library_id) => {
                debug!(
                    "Library {} sync started for source {}",
                    library_id, self.source.name
                );
                self.syncing_libraries.insert(library_id);
                // Update the library list to show spinners
                self.update_library_list(&widgets.library_list);
            }
            SourceGroupInput::LibrarySyncCompleted(library_id) => {
                debug!(
                    "Library {} sync completed for source {}",
                    library_id, self.source.name
                );
                self.syncing_libraries.remove(&library_id);
                // Update the library list to hide spinners
                self.update_library_list(&widgets.library_list);
            }
            SourceGroupInput::UpdateConnectionStatus(state) => {
                self.connection_state = state;
                debug!(
                    "Source {} connection status updated: {:?}",
                    self.source.name, state
                );
                // Trigger a view refresh by updating the widgets
                // The #[watch] attributes will now pick up the state change
                widgets.root.queue_draw();
            }
        }
    }
}

// Main sidebar component
#[derive(Debug)]
pub struct Sidebar {
    db: DatabaseConnection,
    source_groups: FactoryVecDeque<SourceGroup>,
    has_sources: bool,
    connection_status: String,
    is_syncing: bool,
    selected_library_id: Option<LibraryId>,
    syncing_sources: HashMap<String, String>,
    syncing_libraries: HashMap<String, (String, String)>,
}

impl Sidebar {
    fn update_status_text(&mut self) {
        if !self.has_sources {
            self.connection_status = "No sources configured".to_string();
        } else if !self.syncing_sources.is_empty() || !self.syncing_libraries.is_empty() {
            // Build status message based on what's syncing
            let mut status_parts = Vec::new();

            // Add source sync status
            if !self.syncing_sources.is_empty() {
                let source_names: Vec<String> = self.syncing_sources.values().cloned().collect();
                status_parts.push(format!("Syncing {}", source_names.join(", ")));
            }

            // Add library sync status
            if !self.syncing_libraries.is_empty() {
                if self.syncing_libraries.len() == 1 {
                    let (_, library_name) = self.syncing_libraries.values().next().unwrap();
                    status_parts.push(format!("Library: {}", library_name));
                } else {
                    status_parts.push(format!("{} libraries", self.syncing_libraries.len()));
                }
            }

            self.connection_status = status_parts.join(" • ");
        } else {
            // No active syncs
            let source_count = self.source_groups.guard().len();
            if source_count > 0 {
                self.connection_status = format!("All {} sources connected", source_count);
            } else {
                self.connection_status = "Ready".to_string();
            }
        }
    }
}

#[relm4::component(pub)]
impl Component for Sidebar {
    type Init = DatabaseConnection;
    type Input = SidebarInput;
    type Output = SidebarOutput;
    type CommandOutput = ();

    view! {
        root = gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            add_css_class: "navigation-sidebar",

            // Scrollable content area
            gtk::ScrolledWindow {
                set_hscrollbar_policy: gtk::PolicyType::Never,
                set_vscrollbar_policy: gtk::PolicyType::Automatic,
                set_vexpand: true,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 0,

                    // Welcome section - shown when no sources
                    #[name = "welcome_box"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 0,
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,
                        set_vexpand: true,
                        set_visible: !model.has_sources,
                        add_css_class: "welcome-container",

                        gtk::Image {
                            set_icon_name: Some("applications-multimedia-symbolic"),
                            add_css_class: "welcome-icon",
                        },

                        gtk::Label {
                            set_text: "Welcome to Reel",
                            add_css_class: "welcome-title",
                            set_halign: gtk::Align::Center,
                        },

                        gtk::Label {
                            set_text: "Connect to your media server to get started",
                            add_css_class: "welcome-subtitle",
                            set_halign: gtk::Align::Center,
                        },

                        gtk::Button {
                            set_label: "Connect to Server",
                            set_halign: gtk::Align::Center,
                            add_css_class: "welcome-button",
                            connect_clicked => SidebarInput::ManageSources,
                        }
                    },

                    // Home button - shown only when sources exist
                    #[name = "home_button"]
                    gtk::Button {
                        set_visible: model.has_sources,
                        add_css_class: "flat",
                        add_css_class: "home-button",
                        connect_clicked => SidebarInput::NavigateHome,

                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 12,
                            set_margin_top: 8,
                            set_margin_bottom: 8,
                            set_margin_start: 8,
                            set_margin_end: 8,

                            gtk::Image {
                                set_icon_name: Some("user-home-symbolic"),
                                set_pixel_size: 16,
                            },

                            gtk::Label {
                                set_text: "Home",
                                set_halign: gtk::Align::Start,
                                set_hexpand: true,
                            },
                        },
                    },

                    // Sources container
                    #[local_ref]
                    sources_container -> gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 0,
                        set_visible: model.has_sources,
                    },

                    // Status section (minimal, GNOME-like)
                    #[name = "status_container"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 6,
                        set_margin_all: 8,
                        set_visible: model.has_sources,

                        gtk::Label {
                            set_text: &model.connection_status,
                            add_css_class: "dim-label",
                            add_css_class: "caption",
                        },

                        gtk::Spinner {
                            set_spinning: model.is_syncing,
                            set_visible: model.is_syncing,
                        }
                    }
                }
            },

            // Sources button at the bottom
            gtk::Button {
                set_label: "Media Sources",
                set_margin_all: 8,
                add_css_class: "pill",
                connect_clicked => SidebarInput::ManageSources,
            }
        }
    }

    fn init(
        db: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let source_groups = FactoryVecDeque::builder()
            .launch(gtk::Box::new(gtk::Orientation::Vertical, 8))
            .forward(sender.input_sender(), |output| match output {
                SourceGroupOutput::NavigateToLibrary(library_id) => {
                    SidebarInput::NavigateToLibrary(library_id)
                }
            });

        let model = Self {
            db,
            source_groups,
            has_sources: false,
            connection_status: "No sources configured".to_string(),
            is_syncing: false,
            selected_library_id: None,
            syncing_sources: HashMap::new(),
            syncing_libraries: HashMap::new(),
        };

        let sources_container = model.source_groups.widget();
        let widgets = view_output!();

        // Load initial sources
        sender.input(SidebarInput::RefreshSources);

        // Subscribe to broker messages for sync updates
        let broker_sender = sender.clone();
        relm4::spawn(async move {
            let (tx, rx) = relm4::channel::<BrokerMessage>();
            BROKER.subscribe("sidebar".to_string(), tx).await;

            while let Some(msg) = rx.recv().await {
                broker_sender.input(SidebarInput::BrokerMsg(msg));
            }
        });

        ComponentParts { model, widgets }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        msg: Self::Input,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            SidebarInput::RefreshSources => {
                debug!("Refreshing sources from database");
                self.is_syncing = true;

                // Clone the database connection for the async block
                let db = self.db.clone();
                let sender = sender.clone();

                // Load sources asynchronously
                relm4::spawn(async move {
                    let command = LoadSourcesCommand { db };
                    match command.execute().await {
                        Ok(sources) => {
                            debug!("Loaded {} sources from database", sources.len());
                            // Send the sources back to the component
                            sender.input(SidebarInput::SourcesLoaded(sources));
                        }
                        Err(e) => {
                            error!("Failed to load sources: {}", e);
                        }
                    }
                });
            }

            SidebarInput::SourcesLoaded(sources) => {
                debug!("Handling loaded sources: {} sources", sources.len());
                self.has_sources = !sources.is_empty();
                self.is_syncing = false;

                if sources.is_empty() {
                    self.connection_status = "No sources configured".to_string();
                } else {
                    self.connection_status = format!("All {} sources connected", sources.len());
                }

                // Update visibility based on has_sources
                widgets.welcome_box.set_visible(!self.has_sources);
                widgets.home_button.set_visible(self.has_sources);
                widgets.sources_container.set_visible(self.has_sources);
                widgets.status_container.set_visible(self.has_sources);

                // Update source groups
                self.source_groups.guard().clear();
                for source in sources {
                    self.source_groups
                        .guard()
                        .push_back((source, self.db.clone()));
                }
            }

            SidebarInput::LibrariesLoaded(source_id, libraries) => {
                debug!(
                    "Loaded {} libraries for source {}",
                    libraries.len(),
                    source_id
                );
                // Libraries are handled by individual SourceGroup components
            }

            SidebarInput::NavigateHome => {
                debug!("Navigating to home");
                let _ = sender.output(SidebarOutput::NavigateToHome);
            }

            SidebarInput::NavigateToLibrary(library_id) => {
                debug!("Navigating to library: {}", library_id);
                self.selected_library_id = Some(library_id.clone());
                let _ = sender.output(SidebarOutput::NavigateToLibrary(library_id));
            }

            SidebarInput::ManageSources => {
                debug!("Managing sources");
                let _ = sender.output(SidebarOutput::NavigateToSources);
            }

            SidebarInput::UpdateConnectionStatus(status) => {
                self.connection_status = status;
            }

            SidebarInput::UpdateSourceConnectionStatus { source_id, state } => {
                // Find the source group and update its connection status
                let idx_to_update = {
                    let guard = self.source_groups.guard();
                    guard.iter().enumerate().find_map(|(idx, sg)| {
                        if sg.source.id == source_id.to_string() {
                            Some(idx)
                        } else {
                            None
                        }
                    })
                };

                if let Some(idx) = idx_to_update {
                    self.source_groups
                        .send(idx, SourceGroupInput::UpdateConnectionStatus(state));
                }
            }

            SidebarInput::BrokerMsg(msg) => {
                match msg {
                    BrokerMessage::Source(SourceMessage::SyncStarted { source_id, .. }) => {
                        self.is_syncing = true;

                        // Find source name
                        let source_name = {
                            let guard = self.source_groups.guard();
                            guard.iter().find_map(|sg| {
                                if sg.source.id == source_id {
                                    Some(sg.source.name.clone())
                                } else {
                                    None
                                }
                            })
                        }
                        .unwrap_or_else(|| source_id.clone());

                        self.syncing_sources.insert(source_id, source_name.clone());
                        self.update_status_text();
                    }
                    BrokerMessage::Source(SourceMessage::SyncCompleted {
                        source_id,
                        items_synced,
                    }) => {
                        self.syncing_sources.remove(&source_id);

                        // Check if any sync is still running
                        self.is_syncing =
                            !self.syncing_sources.is_empty() || !self.syncing_libraries.is_empty();
                        self.update_status_text();

                        debug!(
                            "Sync completed for source: {}, {} items synced, refreshing libraries",
                            source_id, items_synced
                        );

                        // Find the index of the source group to update
                        let idx_to_update = {
                            let guard = self.source_groups.guard();
                            guard.iter().enumerate().find_map(|(idx, sg)| {
                                if sg.source.id == source_id {
                                    Some(idx)
                                } else {
                                    None
                                }
                            })
                        };

                        // Send reload message and update connection status if we found the source
                        if let Some(idx) = idx_to_update {
                            self.source_groups
                                .send(idx, SourceGroupInput::ReloadLibraries);
                            // Set connection state to Connected since sync completed successfully
                            self.source_groups.send(
                                idx,
                                SourceGroupInput::UpdateConnectionStatus(
                                    ConnectionState::Connected,
                                ),
                            );
                        }
                    }
                    BrokerMessage::Source(SourceMessage::SyncError { source_id, error }) => {
                        self.syncing_sources.remove(&source_id);
                        self.is_syncing =
                            !self.syncing_sources.is_empty() || !self.syncing_libraries.is_empty();

                        // Update status to show error briefly
                        self.connection_status = format!("Sync failed: {}", error);

                        // Update the source's connection state to show sync failed
                        let idx_to_update = {
                            let guard = self.source_groups.guard();
                            guard.iter().enumerate().find_map(|(idx, sg)| {
                                if sg.source.id == source_id {
                                    Some(idx)
                                } else {
                                    None
                                }
                            })
                        };

                        if let Some(idx) = idx_to_update {
                            self.source_groups.send(
                                idx,
                                SourceGroupInput::UpdateConnectionStatus(
                                    ConnectionState::SyncFailed,
                                ),
                            );
                        }

                        // Reset status after 3 seconds
                        let sender_clone = sender.clone();
                        relm4::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                            sender_clone
                                .input(SidebarInput::UpdateConnectionStatus("Ready".to_string()));
                        });
                    }
                    BrokerMessage::Source(SourceMessage::LibrarySyncStarted {
                        source_id,
                        library_id,
                        library_name,
                    }) => {
                        self.syncing_libraries.insert(
                            library_id.clone(),
                            (source_id.clone(), library_name.clone()),
                        );
                        self.is_syncing = true;

                        // Find the source group index and send library sync started message
                        let idx = {
                            let guard = self.source_groups.guard();
                            guard.iter().enumerate().find_map(|(idx, sg)| {
                                if sg.source.id == source_id {
                                    Some(idx)
                                } else {
                                    None
                                }
                            })
                        };

                        if let Some(idx) = idx {
                            self.source_groups
                                .send(idx, SourceGroupInput::LibrarySyncStarted(library_id));
                        }

                        self.update_status_text();
                    }
                    BrokerMessage::Source(SourceMessage::LibrarySyncCompleted {
                        source_id,
                        library_id,
                        library_name,
                        items_synced,
                    }) => {
                        self.syncing_libraries.remove(&library_id);
                        self.is_syncing =
                            !self.syncing_sources.is_empty() || !self.syncing_libraries.is_empty();

                        // Find the source group index and send library sync completed message
                        let idx = {
                            let guard = self.source_groups.guard();
                            guard.iter().enumerate().find_map(|(idx, sg)| {
                                if sg.source.id == source_id {
                                    Some(idx)
                                } else {
                                    None
                                }
                            })
                        };

                        if let Some(idx) = idx {
                            self.source_groups.send(
                                idx,
                                SourceGroupInput::LibrarySyncCompleted(library_id.clone()),
                            );
                        }

                        // Show completion message briefly
                        self.connection_status =
                            format!("Synced '{}' ({} items)", library_name, items_synced);

                        // Update status after a short delay
                        let sender_clone = sender.clone();
                        relm4::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                            sender_clone
                                .input(SidebarInput::UpdateConnectionStatus("Ready".to_string()));
                        });
                    }
                    _ => {}
                }
            }
        }
    }
}
