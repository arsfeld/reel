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
    type Init = Source;
    type Input = SourceGroupInput;
    type Output = SourceGroupOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        root = gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 18,

            gtk::Label {
                set_text: &self.source.name,
                set_halign: gtk::Align::Start,
                add_css_class: "heading",
                set_margin_start: 12,
            },

            gtk::ListBox {
                set_selection_mode: gtk::SelectionMode::None,
                add_css_class: "boxed-list",
                set_margin_start: 12,
                set_margin_end: 12,

                // Movies library
                gtk::ListBoxRow {
                    set_activatable: true,
                    connect_activate => move |_| {
                        // Will be handled by row activation
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 12,
                        set_margin_all: 12,

                        gtk::Image {
                            set_icon_name: Some("video-x-generic-symbolic"),
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 2,
                            set_hexpand: true,

                            gtk::Label {
                                set_text: "Movies",
                                set_halign: gtk::Align::Start,
                                add_css_class: "heading",
                            },

                            gtk::Label {
                                set_text: "1,250 items",
                                set_halign: gtk::Align::Start,
                                add_css_class: "dim-label",
                                add_css_class: "caption",
                            }
                        },

                        gtk::Image {
                            set_icon_name: Some("go-next-symbolic"),
                            add_css_class: "dim-label",
                        }
                    }
                },

                // TV Shows library
                gtk::ListBoxRow {
                    set_activatable: true,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 12,
                        set_margin_all: 12,

                        gtk::Image {
                            set_icon_name: Some("video-display-symbolic"),
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 2,
                            set_hexpand: true,

                            gtk::Label {
                                set_text: "TV Shows",
                                set_halign: gtk::Align::Start,
                                add_css_class: "heading",
                            },

                            gtk::Label {
                                set_text: "450 items",
                                set_halign: gtk::Align::Start,
                                add_css_class: "dim-label",
                                add_css_class: "caption",
                            }
                        },

                        gtk::Image {
                            set_icon_name: Some("go-next-symbolic"),
                            add_css_class: "dim-label",
                        }
                    }
                },

                // Music library
                gtk::ListBoxRow {
                    set_activatable: true,

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 12,
                        set_margin_all: 12,

                        gtk::Image {
                            set_icon_name: Some("audio-x-generic-symbolic"),
                        },

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_spacing: 2,
                            set_hexpand: true,

                            gtk::Label {
                                set_text: "Music",
                                set_halign: gtk::Align::Start,
                                add_css_class: "heading",
                            },

                            gtk::Label {
                                set_text: "2,890 items",
                                set_halign: gtk::Align::Start,
                                add_css_class: "dim-label",
                                add_css_class: "caption",
                            }
                        },

                        gtk::Image {
                            set_icon_name: Some("go-next-symbolic"),
                            add_css_class: "dim-label",
                        }
                    }
                }
            }
        }
    }

    fn init_model(source: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            source,
            libraries: Vec::new(),
            is_loading: false,
        }
    }

    fn update(&mut self, msg: Self::Input, _sender: FactorySender<Self>) {
        match msg {
            SourceGroupInput::LoadLibraries => {
                self.is_loading = true;
            }
            SourceGroupInput::LibrariesLoaded(libraries) => {
                self.libraries = libraries;
                self.is_loading = false;
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

            // Scrollable content area
            gtk::ScrolledWindow {
                set_hscrollbar_policy: gtk::PolicyType::Never,
                set_vscrollbar_policy: gtk::PolicyType::Automatic,
                set_vexpand: true,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_top: 12,
                    set_margin_bottom: 12,
                    set_margin_start: 12,
                    set_margin_end: 12,
                    set_spacing: 12,

                    // Welcome section - shown when no sources
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 24,
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,
                        set_vexpand: true,
                        set_visible: !model.has_sources,

                        gtk::Image {
                            set_icon_name: Some("applications-multimedia-symbolic"),
                            set_pixel_size: 64,
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

                                    gtk::Image {
                                        set_icon_name: Some("go-next-symbolic"),
                                    }
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

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
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

                // Update source groups
                self.source_groups.guard().clear();
                for source in sources {
                    self.source_groups.guard().push_back(source);
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
