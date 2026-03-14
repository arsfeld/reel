use gtk4::prelude::*;
use relm4::prelude::*;

use crate::models::library::LibraryType;

pub struct Sidebar {
    selected: LibraryType,
}

#[derive(Debug)]
pub enum SidebarMsg {
    SelectMovies,
    SelectShows,
}

#[derive(Debug)]
pub enum SidebarOutput {
    Navigate(LibraryType),
}

#[relm4::component(pub)]
impl SimpleComponent for Sidebar {
    type Init = ();
    type Input = SidebarMsg;
    type Output = SidebarOutput;

    view! {
        gtk4::Box {
            set_orientation: gtk4::Orientation::Vertical,
            set_width_request: 200,

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

                gtk4::ListBoxRow {
                    set_selectable: true,
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_spacing: 12,
                        set_margin_all: 8,

                        gtk4::Image {
                            set_icon_name: Some("video-display-symbolic"),
                        },
                        gtk4::Label {
                            set_label: "Movies",
                            set_hexpand: true,
                            set_halign: gtk4::Align::Start,
                        },
                    },
                },

                gtk4::ListBoxRow {
                    set_selectable: true,
                    gtk4::Box {
                        set_orientation: gtk4::Orientation::Horizontal,
                        set_spacing: 12,
                        set_margin_all: 8,

                        gtk4::Image {
                            set_icon_name: Some("view-list-symbolic"),
                        },
                        gtk4::Label {
                            set_label: "TV Shows",
                            set_hexpand: true,
                            set_halign: gtk4::Align::Start,
                        },
                    },
                },

                connect_row_selected[sender] => move |_, row| {
                    if let Some(row) = row {
                        match row.index() {
                            0 => sender.input(SidebarMsg::SelectMovies),
                            1 => sender.input(SidebarMsg::SelectShows),
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
        let model = Self {
            selected: LibraryType::Movie,
        };
        let widgets = view_output!();

        // Select first row by default
        let first_row = widgets.listbox.row_at_index(0);
        widgets.listbox.select_row(first_row.as_ref());

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            SidebarMsg::SelectMovies => {
                self.selected = LibraryType::Movie;
                let _ = sender.output(SidebarOutput::Navigate(LibraryType::Movie));
            }
            SidebarMsg::SelectShows => {
                self.selected = LibraryType::Show;
                let _ = sender.output(SidebarOutput::Navigate(LibraryType::Show));
            }
        }
    }
}
