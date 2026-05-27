use gtk4::prelude::*;
use relm4::prelude::*;

use crate::models::library::LibraryType;

pub struct Sidebar {
    selected: LibraryType,
    home_selected: bool,
    syncing: bool,
    listbox: gtk4::ListBox,
    movies_count: gtk4::Label,
    shows_count: gtk4::Label,
    collections_count: gtk4::Label,
}

#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
#[allow(dead_code)]
pub enum SidebarMsg {
    SelectHome,
    SelectMovies,
    SelectShows,
    SelectCollections,
    /// Select a row programmatically (from ViewSwitcher sync). Does NOT emit output.
    SelectExternal(LibraryType),
    /// Update the item counts shown next to each row label.
    SetCounts {
        movies: usize,
        shows: usize,
        collections: usize,
    },
}

#[derive(Debug)]
pub enum SidebarOutput {
    NavigateHome,
    Navigate(LibraryType),
    ShowCollections,
}

#[relm4::component(pub)]
impl SimpleComponent for Sidebar {
    type Init = ();
    type Input = SidebarMsg;
    type Output = SidebarOutput;

    view! {
        gtk4::Box {
            set_orientation: gtk4::Orientation::Vertical,
            set_width_request: 220,

            adw::HeaderBar {
                set_show_end_title_buttons: false,
                #[wrap(Some)]
                set_title_widget = &gtk4::Label {
                    set_label: "Reel",
                    add_css_class: "title",
                },
            },

            #[name = "listbox"]
            gtk4::ListBox {
                add_css_class: "navigation-sidebar",
                set_selection_mode: gtk4::SelectionMode::Single,

                // --- Home row ---
                gtk4::ListBoxRow {
                    set_selectable: true,
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_spacing: 12,
                        set_margin_all: 8,

                        gtk4::Image {
                            set_icon_name: Some("go-home-symbolic"),
                        },
                        gtk4::Label {
                            set_label: "Home",
                            set_hexpand: true,
                            set_halign: gtk4::Align::Start,
                        },
                    },
                },

                // --- Movies row ---
                gtk4::ListBoxRow {
                    set_selectable: true,
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_spacing: 12,
                        set_margin_all: 8,

                        gtk4::Image {
                            set_icon_name: Some("video-display-symbolic"),
                        },
                        #[name = "movies_label"]
                        gtk4::Label {
                            set_label: "Movies",
                            set_hexpand: true,
                            set_halign: gtk4::Align::Start,
                        },
                        #[name = "movies_count"]
                        gtk4::Label {
                            add_css_class: "dim-label",
                            add_css_class: "caption",
                            set_halign: gtk4::Align::End,
                            set_margin_end: 4,
                        },
                    },
                },

                // --- TV Shows row ---
                gtk4::ListBoxRow {
                    set_selectable: true,
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_spacing: 12,
                        set_margin_all: 8,

                        gtk4::Image {
                            set_icon_name: Some("view-list-symbolic"),
                        },
                        #[name = "shows_label"]
                        gtk4::Label {
                            set_label: "TV Shows",
                            set_hexpand: true,
                            set_halign: gtk4::Align::Start,
                        },
                        #[name = "shows_count"]
                        gtk4::Label {
                            add_css_class: "dim-label",
                            add_css_class: "caption",
                            set_halign: gtk4::Align::End,
                            set_margin_end: 4,
                        },
                    },
                },

                // --- Collections row ---
                gtk4::ListBoxRow {
                    set_selectable: true,
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_spacing: 12,
                        set_margin_all: 8,

                        gtk4::Image {
                            set_icon_name: Some("view-grid-symbolic"),
                        },
                        #[name = "collections_label"]
                        gtk4::Label {
                            set_label: "Collections",
                            set_hexpand: true,
                            set_halign: gtk4::Align::Start,
                        },
                        #[name = "collections_count"]
                        gtk4::Label {
                            add_css_class: "dim-label",
                            add_css_class: "caption",
                            set_halign: gtk4::Align::End,
                            set_margin_end: 4,
                        },
                    },
                },

                connect_row_selected[sender] => move |_, row| {
                    if let Some(row) = row {
                        match row.index() {
                            0 => sender.input(SidebarMsg::SelectHome),
                            1 => sender.input(SidebarMsg::SelectMovies),
                            2 => sender.input(SidebarMsg::SelectShows),
                            3 => sender.input(SidebarMsg::SelectCollections),
                            _ => {}
                        }
                    }
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = view_output!();

        // Select first row by default (Home)
        let first_row = widgets.listbox.row_at_index(0);
        widgets.listbox.select_row(first_row.as_ref());

        let model = Self {
            selected: LibraryType::Movie,
            home_selected: true,
            syncing: false,
            listbox: widgets.listbox.clone(),
            movies_count: widgets.movies_count.clone(),
            shows_count: widgets.shows_count.clone(),
            collections_count: widgets.collections_count.clone(),
        };

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            SidebarMsg::SelectHome => {
                if self.syncing {
                    return;
                }
                self.home_selected = true;
                let _ = sender.output(SidebarOutput::NavigateHome);
            }
            SidebarMsg::SelectMovies => {
                if self.syncing {
                    return;
                }
                self.home_selected = false;
                self.selected = LibraryType::Movie;
                let _ = sender.output(SidebarOutput::Navigate(LibraryType::Movie));
            }
            SidebarMsg::SelectShows => {
                if self.syncing {
                    return;
                }
                self.home_selected = false;
                self.selected = LibraryType::Show;
                let _ = sender.output(SidebarOutput::Navigate(LibraryType::Show));
            }
            SidebarMsg::SelectCollections => {
                if self.syncing {
                    return;
                }
                self.home_selected = false;
                let _ = sender.output(SidebarOutput::ShowCollections);
            }
            SidebarMsg::SelectExternal(library_type) => {
                self.syncing = true;
                let row_idx = match library_type {
                    LibraryType::Movie => 1,
                    LibraryType::Show => 2,
                };
                if let Some(row) = self.listbox.row_at_index(row_idx) {
                    self.listbox.select_row(Some(&row));
                }
                self.selected = library_type;
                self.home_selected = false;
                self.syncing = false;
            }
            SidebarMsg::SetCounts {
                movies,
                shows,
                collections,
            } => {
                self.movies_count
                    .set_label(&format!("({movies})"));
                self.shows_count
                    .set_label(&format!("({shows})"));
                self.collections_count
                    .set_label(&format!("({collections})"));
            }
        }
    }
}
