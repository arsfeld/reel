use gtk::prelude::*;
use libadwaita as adw;
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender, FactoryVecDeque};
use relm4::prelude::*;
use relm4::{Component, ComponentParts, ComponentSender, gtk};
use tracing::{debug, error};

use crate::db::connection::DatabaseConnection;
use crate::models::auth_provider::Source;
use crate::models::{Library, LibraryId, LibraryType, SourceId};
use crate::services::commands::{Command, auth_commands::LoadSourcesCommand};
use crate::services::core::media::MediaService;

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
    db: DatabaseConnection,
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
            hbox.set_margin_top(8);
            hbox.set_margin_bottom(8);
            hbox.set_margin_start(8);
            hbox.set_margin_end(8);

            // Icon based on library type
            let icon_name = match library.library_type {
                LibraryType::Movies => "video-x-generic-symbolic",
                LibraryType::Shows => "video-display-symbolic",
                LibraryType::Music => "audio-x-generic-symbolic",
                LibraryType::Photos => "image-x-generic-symbolic",
                LibraryType::Mixed => "folder-symbolic",
            };
            let icon = gtk::Image::from_icon_name(icon_name);
            hbox.append(&icon);

            // Library info box
            let vbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
            vbox.set_hexpand(true);

            let name_label = gtk::Label::new(Some(&library.title));
            name_label.set_halign(gtk::Align::Start);
            name_label.add_css_class("heading");
            vbox.append(&name_label);

            // For now, we'll show a placeholder count
            // TODO: Get actual item count from database
            let count_label = gtk::Label::new(Some("Loading..."));
            count_label.set_halign(gtk::Align::Start);
            count_label.add_css_class("dim-label");
            count_label.add_css_class("caption");
            vbox.append(&count_label);

            hbox.append(&vbox);

            // Remove arrow icon for cleaner look

            row.set_child(Some(&hbox));
            library_list.append(&row);
        }

        // If no libraries, show a placeholder
        if self.libraries.is_empty() {
            let row = gtk::ListBoxRow::new();
            row.set_activatable(false);

            let label = gtk::Label::new(Some("No libraries found"));
            label.set_margin_top(12);
            label.set_margin_bottom(12);
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
            set_spacing: 12,

            gtk::Label {
                set_text: &self.source.name,
                set_halign: gtk::Align::Start,
                add_css_class: "dim-label",
                add_css_class: "caption",
            },

            #[local_ref]
            library_list -> gtk::ListBox {
                set_selection_mode: gtk::SelectionMode::None,
                add_css_class: "boxed-list",
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
            db,
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
        library_list.add_css_class("boxed-list");

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
                    set_spacing: 12,

                    // Welcome section - shown when no sources
                    #[name = "welcome_box"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 24,
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,
                        set_vexpand: true,
                        set_visible: !model.has_sources,

                        gtk::Image {
                            set_icon_name: Some("applications-multimedia-symbolic"),
                            set_pixel_size: 128,
                            add_css_class: "dim-label",
                        },

                        gtk::Label {
                            set_text: "Welcome to Reel",
                            add_css_class: "title-2",
                            set_halign: gtk::Align::Center,
                        },

                        gtk::Label {
                            set_text: "Connect to your media server to get started",
                            add_css_class: "body",
                            set_halign: gtk::Align::Center,
                        },

                        gtk::Button {
                            set_label: "Connect to Server",
                            set_halign: gtk::Align::Center,
                            add_css_class: "pill",
                            add_css_class: "suggested-action",
                            connect_clicked => SidebarInput::ManageSources,
                        }
                    },

                    // Home section - shown when sources exist
                    #[name = "home_section"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 12,
                        set_visible: model.has_sources,

                        gtk::ListBox {
                            set_selection_mode: gtk::SelectionMode::None,
                            add_css_class: "boxed-list",

                            gtk::ListBoxRow {
                                set_activatable: true,

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Horizontal,
                                    set_spacing: 12,
                                    set_margin_all: 12,

                                    gtk::Image {
                                        set_icon_name: Some("user-home-symbolic"),
                                    },

                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Vertical,
                                        set_spacing: 2,
                                        set_hexpand: true,

                                        gtk::Label {
                                            set_text: "Home",
                                            set_halign: gtk::Align::Start,
                                            add_css_class: "heading",
                                        },

                                        gtk::Label {
                                            set_text: "Recently added from all sources",
                                            set_halign: gtk::Align::Start,
                                            add_css_class: "dim-label",
                                            add_css_class: "caption",
                                        }
                                    },

                                },

                                connect_activate => SidebarInput::NavigateHome,
                            }
                        }
                    },

                    // Sources container
                    #[local_ref]
                    sources_container -> gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 18,
                        set_visible: model.has_sources,
                    },

                    // Status container
                    #[name = "status_container"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 6,
                        set_margin_top: 6,
                        set_visible: model.has_sources,

                        gtk::Image {
                            set_icon_name: Some("network-transmit-receive-symbolic"),
                            set_opacity: 0.5,
                        },

                        gtk::Label {
                            set_text: &model.connection_status,
                            add_css_class: "dim-label",
                            add_css_class: "caption",
                        },

                        gtk::Spinner {
                            set_spinning: model.is_syncing,
                            set_visible: model.is_syncing,
                            set_margin_start: 6,
                        }
                    }
                }
            },

            // Sticky Sources button at the bottom
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                gtk::Separator {
                    set_orientation: gtk::Orientation::Horizontal,
                },

                gtk::Button {
                    set_margin_top: 12,
                    set_margin_bottom: 12,
                    set_margin_start: 12,
                    set_margin_end: 12,
                    add_css_class: "pill",
                    connect_clicked => SidebarInput::ManageSources,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 6,
                        set_halign: gtk::Align::Center,

                        gtk::Image {
                            set_icon_name: Some("network-server-symbolic"),
                        },

                        gtk::Label {
                            set_text: "Servers & Accounts",
                        }
                    }
                }
            }
        }
    }

    fn init(
        db: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let source_groups = FactoryVecDeque::builder()
            .launch(gtk::Box::new(gtk::Orientation::Vertical, 18))
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
        };

        let sources_container = model.source_groups.widget();
        let widgets = view_output!();

        // Load initial sources
        sender.input(SidebarInput::RefreshSources);

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
                widgets.home_section.set_visible(self.has_sources);
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
                sender.output(SidebarOutput::NavigateToHome);
            }

            SidebarInput::NavigateToLibrary(library_id) => {
                debug!("Navigating to library: {}", library_id);
                sender.output(SidebarOutput::NavigateToLibrary(library_id));
            }

            SidebarInput::ManageSources => {
                debug!("Managing sources");
                sender.output(SidebarOutput::NavigateToSources);
            }

            SidebarInput::UpdateConnectionStatus(status) => {
                self.connection_status = status;
            }
        }
    }
}
