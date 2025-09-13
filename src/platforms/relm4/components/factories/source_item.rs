use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use relm4::factory::DynamicIndex;
use relm4::prelude::*;

use crate::models::{Library, LibraryId, Source, SourceId};

#[derive(Debug)]
pub struct SourceItem {
    source: Source,
    libraries: Vec<Library>,
    expanded: bool,
    selected_library: Option<LibraryId>,
    connection_status: ConnectionStatus,
    libraries_box: gtk::Box,
    header_row: gtk::ListBoxRow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Connecting,
    Disconnected,
    Error,
}

#[derive(Debug, Clone)]
pub enum SourceItemInput {
    SetSource(Source),
    SetLibraries(Vec<Library>),
    SetExpanded(bool),
    ToggleExpanded,
    SetSelectedLibrary(Option<LibraryId>),
    SetConnectionStatus(ConnectionStatus),
    LibraryClicked(LibraryId),
}

#[derive(Debug, Clone)]
pub enum SourceItemOutput {
    SourceSelected(SourceId),
    LibrarySelected(LibraryId),
    RefreshRequested(SourceId),
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
        root = gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 0,

            // Source header row
            #[name = "header_row"]
            gtk::ListBoxRow {
                add_css_class: "source-item",
                set_activatable: true,

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 12,
                    set_margin_all: 12,

                    // Expand/collapse arrow
                    gtk::Image {
                        set_icon_name: if self.expanded {
                            Some("go-down-symbolic")
                        } else {
                            Some("go-next-symbolic")
                        },
                        set_pixel_size: 16,
                        set_visible: !self.libraries.is_empty(),
                    },

                    // Source icon
                    gtk::Image {
                        set_icon_name: Some(match &self.source.source_type {
                            crate::models::SourceType::PlexServer { .. } => "network-server-symbolic",
                            crate::models::SourceType::JellyfinServer => "network-server-symbolic",
                            crate::models::SourceType::NetworkShare { .. } => "folder-remote-symbolic",
                            crate::models::SourceType::LocalFolder { .. } => "folder-symbolic",
                        }),
                        set_pixel_size: 24,
                    },

                    // Source name and library count
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_hexpand: true,
                        set_valign: gtk::Align::Center,

                        gtk::Label {
                            set_label: &self.source.name,
                            set_halign: gtk::Align::Start,
                            add_css_class: "body",
                        },

                        gtk::Label {
                            set_label: &format!("{} libraries", self.libraries.len()),
                            set_halign: gtk::Align::Start,
                            add_css_class: "dim-label",
                            add_css_class: "caption",
                            set_visible: !self.libraries.is_empty(),
                        },
                    },

                    // Connection status indicator
                    gtk::Box {
                        set_spacing: 6,

                        gtk::Spinner {
                            set_visible: matches!(self.connection_status, ConnectionStatus::Connecting),
                            set_spinning: true,
                            set_size_request: (16, 16),
                        },

                        gtk::Image {
                            set_visible: !matches!(self.connection_status, ConnectionStatus::Connecting),
                            set_icon_name: match self.connection_status {
                                ConnectionStatus::Connected => Some("emblem-ok-symbolic"),
                                ConnectionStatus::Disconnected => Some("network-offline-symbolic"),
                                ConnectionStatus::Error => Some("dialog-error-symbolic"),
                                _ => None,
                            },
                            set_pixel_size: 16,
                            add_css_class: match self.connection_status {
                                ConnectionStatus::Connected => "success",
                                ConnectionStatus::Error => "error",
                                _ => "dim-label",
                            },
                        },
                    },
                },
            },

            // Libraries container
            #[name = "libraries_container"]
            gtk::Box {
                set_visible: self.expanded && !self.libraries.is_empty(),
                set_orientation: gtk::Orientation::Vertical,
                set_margin_start: 36,
                append: &self.libraries_box,
            },
        }
    }

    fn init_model(source: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            source,
            libraries: Vec::new(),
            expanded: false,
            selected_library: None,
            connection_status: ConnectionStatus::Disconnected,
            libraries_box: gtk::Box::new(gtk::Orientation::Vertical, 0),
            header_row: gtk::ListBoxRow::new(),
        }
    }

    fn init_widgets(
        &mut self,
        _index: &DynamicIndex,
        _root: Self::Root,
        _returned_widget: &gtk::ListBoxRow,
        sender: FactorySender<Self>,
    ) -> Self::Widgets {
        let widgets = view_output!();

        // Connect the header row click handler
        self.header_row = widgets.header_row.clone();
        let sender_clone = sender.input_sender().clone();
        self.header_row.connect_activate(move |_| {
            sender_clone
                .send(SourceItemInput::ToggleExpanded)
                .unwrap_or_default();
        });

        widgets
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        match msg {
            SourceItemInput::SetSource(source) => {
                self.source = source;
            }
            SourceItemInput::SetLibraries(libraries) => {
                self.libraries = libraries;
                self.rebuild_libraries_list(sender.clone());
            }
            SourceItemInput::SetExpanded(expanded) => {
                self.expanded = expanded;
            }
            SourceItemInput::ToggleExpanded => {
                self.expanded = !self.expanded;
                sender
                    .output(SourceItemOutput::ToggleExpanded(SourceId::from(
                        self.source.id.clone(),
                    )))
                    .unwrap_or_default();
            }
            SourceItemInput::SetSelectedLibrary(library_id) => {
                self.selected_library = library_id;
                self.update_library_selection();
            }
            SourceItemInput::SetConnectionStatus(status) => {
                self.connection_status = status;
            }
            SourceItemInput::LibraryClicked(library_id) => {
                sender
                    .output(SourceItemOutput::LibrarySelected(library_id))
                    .unwrap_or_default();
            }
        }
    }
}

impl SourceItem {
    fn rebuild_libraries_list(&mut self, sender: FactorySender<Self>) {
        // Clear existing children
        while let Some(child) = self.libraries_box.first_child() {
            self.libraries_box.remove(&child);
        }

        // Add library items
        for library in &self.libraries {
            let library_row = gtk::ListBoxRow::new();
            library_row.add_css_class("library-item");
            library_row.set_activatable(true);

            let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            hbox.set_margin_top(8);
            hbox.set_margin_bottom(8);
            hbox.set_margin_start(8);
            hbox.set_margin_end(8);

            // Library icon
            let icon = gtk::Image::new();
            icon.set_icon_name(match library.library_type {
                crate::models::LibraryType::Movies => Some("video-x-generic-symbolic"),
                crate::models::LibraryType::Shows => Some("video-x-generic-symbolic"),
                crate::models::LibraryType::Music => Some("audio-x-generic-symbolic"),
                _ => Some("folder-symbolic"),
            });
            icon.set_pixel_size(16);
            hbox.append(&icon);

            // Library name
            let label = gtk::Label::new(Some(&library.title));
            label.set_halign(gtk::Align::Start);
            label.set_hexpand(true);
            hbox.append(&label);

            // No item count in Library model, could add if needed

            library_row.set_child(Some(&hbox));

            // Connect click handler
            let library_id = LibraryId::from(library.id.clone());
            let sender = sender.clone();
            library_row.connect_activate(move |_| {
                sender.input(SourceItemInput::LibraryClicked(library_id.clone()));
            });

            // Update selection state
            if Some(LibraryId::from(library.id.clone())) == self.selected_library {
                library_row.add_css_class("selected");
            }

            self.libraries_box.append(&library_row);
        }
    }

    fn update_library_selection(&self) {
        // Update selection state for all library rows
        let mut child = self.libraries_box.first_child();
        let mut index = 0;

        while let Some(row) = child {
            if let Some(list_row) = row.downcast_ref::<gtk::ListBoxRow>() {
                list_row.remove_css_class("selected");

                if index < self.libraries.len() {
                    if Some(LibraryId::from(self.libraries[index].id.clone()))
                        == self.selected_library
                    {
                        list_row.add_css_class("selected");
                    }
                }
            }

            child = row.next_sibling();
            index += 1;
        }
    }
}
