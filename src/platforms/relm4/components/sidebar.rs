use gtk::prelude::*;
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender, FactoryVecDeque};
use relm4::prelude::*;
use relm4::{Component, ComponentParts, ComponentSender, gtk};
use tracing::{debug, error};

use crate::db::connection::DatabaseConnection;
use crate::models::auth_provider::Source;
use crate::models::{LibraryId, SourceId};
use crate::services::commands::{Command, auth_commands::LoadSourcesCommand};

// Messages for the sidebar component
#[derive(Debug)]
pub enum SidebarInput {
    /// Refresh sources from database
    RefreshSources,
    /// Sources loaded from database
    SourcesLoaded(Vec<Source>),
    /// Source selected
    SourceSelected(SourceId),
    /// Library selected
    LibrarySelected(LibraryId),
    /// Toggle source expanded/collapsed
    ToggleSource(SourceId),
}

#[derive(Debug)]
pub enum SidebarOutput {
    /// Navigate to source
    NavigateToSource(SourceId),
    /// Navigate to library
    NavigateToLibrary(LibraryId),
}

// Source factory component for the sidebar
#[derive(Debug)]
pub struct SourceItem {
    source: Source,
    library_count: usize,
    is_expanded: bool,
    is_online: bool,
}

#[derive(Debug)]
pub enum SourceItemInput {
    /// Toggle expanded state
    Toggle,
    /// Update online status
    UpdateStatus(bool),
    /// Update library count
    UpdateLibraryCount(usize),
}

#[derive(Debug)]
pub enum SourceItemOutput {
    /// Source was selected
    SourceSelected(SourceId),
    /// Source expand state toggled
    ToggleExpanded(SourceId),
}

#[relm4::factory(pub)]
impl FactoryComponent for SourceItem {
    type Init = Source;
    type Input = SourceItemInput;
    type Output = SourceItemOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        root = gtk::ListBoxRow {
            set_activatable: true,
            set_selectable: true,
            add_css_class: "source-row",

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 12,
                set_margin_all: 8,

                // Online indicator
                gtk::Box {
                    set_width_request: 8,
                    set_height_request: 8,
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::Center,
                    add_css_class: if self.is_online { "online-indicator" } else { "offline-indicator" },
                },

                // Source icon (placeholder for now)
                gtk::Image {
                    set_icon_name: Some("folder-symbolic"),
                    set_pixel_size: 16,
                    add_css_class: "dim-label",
                },

                // Source info
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 2,
                    set_hexpand: true,

                    gtk::Label {
                        set_text: &self.source.name,
                        set_halign: gtk::Align::Start,
                        add_css_class: "heading",
                    },

                    gtk::Label {
                        set_text: &format!("{} libraries", self.library_count),
                        set_halign: gtk::Align::Start,
                        add_css_class: "dim-label",
                        add_css_class: "caption",
                    },
                },

                // Expand indicator
                gtk::Image {
                    set_icon_name: Some(if self.is_expanded { "go-down-symbolic" } else { "go-next-symbolic" }),
                    set_pixel_size: 12,
                    add_css_class: "dim-label",
                },
            },

            connect_activate[sender, source_id = self.source.id.clone()] => move |_| {
                sender.output(SourceItemOutput::SourceSelected(SourceId::new(source_id.clone())));
            },
        }
    }

    fn init_model(source: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            library_count: source.library_count,
            is_online: source.connection_info.is_online,
            is_expanded: false,
            source,
        }
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        match msg {
            SourceItemInput::Toggle => {
                self.is_expanded = !self.is_expanded;
                sender.output(SourceItemOutput::ToggleExpanded(SourceId::new(
                    self.source.id.clone(),
                )));
            }
            SourceItemInput::UpdateStatus(is_online) => {
                self.is_online = is_online;
            }
            SourceItemInput::UpdateLibraryCount(count) => {
                self.library_count = count;
            }
        }
    }
}

// Main sidebar component
#[derive(Debug)]
pub struct Sidebar {
    db: DatabaseConnection,
    sources: FactoryVecDeque<SourceItem>,
    selected_source: Option<SourceId>,
    selected_library: Option<LibraryId>,
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
            set_width_request: 280,
            add_css_class: "sidebar",

            // Header
            gtk::HeaderBar {
                set_show_title_buttons: false,
                add_css_class: "flat",

                #[wrap(Some)]
                set_title_widget = &gtk::Label {
                    set_text: "Sources",
                    add_css_class: "heading",
                },

                pack_end = &gtk::Button {
                    set_icon_name: "list-add-symbolic",
                    set_tooltip_text: Some("Add Source"),
                    add_css_class: "flat",
                },
            },

            // Sources list
            gtk::ScrolledWindow {
                set_vexpand: true,
                set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                #[local_ref]
                sources_list -> gtk::ListBox {
                    add_css_class: "navigation-sidebar",
                    set_selection_mode: gtk::SelectionMode::Single,
                },
            },
        }
    }

    fn init(
        db: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let sources = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |output| match output {
                SourceItemOutput::SourceSelected(id) => SidebarInput::SourceSelected(id),
                SourceItemOutput::ToggleExpanded(id) => SidebarInput::ToggleSource(id),
            });

        let model = Self {
            db,
            sources,
            selected_source: None,
            selected_library: None,
        };

        let sources_list = model.sources.widget();
        let widgets = view_output!();

        // Load initial sources
        sender.input(SidebarInput::RefreshSources);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            SidebarInput::RefreshSources => {
                debug!("Refreshing sources from database");

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
                self.update_sources_list(sources);
            }

            SidebarInput::SourceSelected(source_id) => {
                debug!("Source selected: {}", source_id);
                self.selected_source = Some(source_id.clone());
                sender.output(SidebarOutput::NavigateToSource(source_id));
            }

            SidebarInput::LibrarySelected(library_id) => {
                debug!("Library selected: {}", library_id);
                self.selected_library = Some(library_id.clone());
                sender.output(SidebarOutput::NavigateToLibrary(library_id));
            }

            SidebarInput::ToggleSource(source_id) => {
                debug!("Toggling source: {}", source_id);
                // TODO: Implement source toggle functionality
                // This would expand/collapse the source to show its libraries
            }
        }
    }
}

impl Sidebar {
    fn update_sources_list(&mut self, sources: Vec<Source>) {
        debug!("Updating sources list with {} sources", sources.len());

        // Clear existing sources
        self.sources.guard().clear();

        // Add new sources
        for source in sources {
            self.sources.guard().push_back(source);
        }
    }
}
