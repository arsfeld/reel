use adw::prelude::*;
use libadwaita as adw;
use relm4::factory::{DynamicIndex, FactoryVecDeque};
use relm4::prelude::*;

use super::media_card::{MediaCard, MediaCardInit, MediaCardOutput};
use crate::db::entities::MediaItemModel;
use crate::models::MediaItemId;

#[derive(Debug)]
pub struct SectionRow {
    title: String,
    media_items: FactoryVecDeque<MediaCard>,
    #[allow(dead_code)]
    loading: bool,
}

#[derive(Debug, Clone)]
pub enum SectionRowInput {
    SetTitle(String),
    SetItems(Vec<MediaItemModel>),
    AppendItems(Vec<MediaItemModel>),
    SetLoading(bool),
    ClearItems,
}

#[derive(Debug, Clone)]
pub enum SectionRowOutput {
    MediaSelected(MediaItemId),
    MediaPlayRequested(MediaItemId),
    MarkWatched(MediaItemId),
    MarkUnwatched(MediaItemId),
    LoadMore,
}

#[relm4::factory(pub)]
impl FactoryComponent for SectionRow {
    type Init = String;
    type Input = SectionRowInput;
    type Output = SectionRowOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        root = gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 12,

            // Section header
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_margin_start: 12,
                set_margin_end: 12,

                gtk::Label {
                    set_label: &self.title,
                    add_css_class: "title-2",
                    set_halign: gtk::Align::Start,
                    set_hexpand: true,
                },

                gtk::Button {
                    set_label: "View All",
                    add_css_class: "flat",
                    set_visible: !self.media_items.is_empty(),
                },
            },

            // Scrollable carousel
            gtk::ScrolledWindow {
                set_hscrollbar_policy: gtk::PolicyType::Automatic,
                set_vscrollbar_policy: gtk::PolicyType::Never,
                set_height_request: 260,

                #[wrap(Some)]
                set_child = &gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_margin_start: 12,
                    set_margin_end: 12,

                    #[local_ref]
                    media_flowbox -> gtk::FlowBox {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_homogeneous: true,
                        set_column_spacing: 12,
                        set_min_children_per_line: 1,
                        set_max_children_per_line: 100,
                        set_selection_mode: gtk::SelectionMode::None,
                    },

                    // Loading indicator or empty state
                    gtk::Box {
                        set_visible: self.media_items.is_empty(),
                        set_hexpand: true,
                        set_valign: gtk::Align::Center,

                        gtk::Spinner {
                            set_visible: self.loading,
                            set_spinning: true,
                            set_size_request: (48, 48),
                        },

                        gtk::Label {
                            set_visible: !self.loading && self.media_items.is_empty(),
                            set_label: "No items to display",
                            add_css_class: "dim-label",
                        },
                    },
                },
            },
        }
    }

    fn init_model(title: Self::Init, _index: &DynamicIndex, sender: FactorySender<Self>) -> Self {
        let media_items = FactoryVecDeque::builder()
            .launch(gtk::FlowBox::default())
            .forward(sender.output_sender(), |output| match output {
                MediaCardOutput::Clicked(id) => SectionRowOutput::MediaSelected(id),
                MediaCardOutput::Play(id) => SectionRowOutput::MediaPlayRequested(id),
                MediaCardOutput::GoToShow(id) => SectionRowOutput::MediaSelected(id),
                MediaCardOutput::MarkWatched(id) => SectionRowOutput::MarkWatched(id),
                MediaCardOutput::MarkUnwatched(id) => SectionRowOutput::MarkUnwatched(id),
            });

        Self {
            title,
            media_items,
            loading: false,
        }
    }

    fn init_widgets(
        &mut self,
        _index: &DynamicIndex,
        root: Self::Root,
        _returned_widget: &gtk::Widget,
        sender: FactorySender<Self>,
    ) -> Self::Widgets {
        let media_flowbox = self.media_items.widget();

        // Check if we need to load more when scrolling reaches the end
        if let Some(scrolled_window) = root
            .first_child()
            .and_then(|w| w.next_sibling())
            .and_then(|w| w.downcast::<gtk::ScrolledWindow>().ok())
        {
            let sender = sender.output_sender().clone();
            scrolled_window
                .hadjustment()
                .connect_value_changed(move |adj| {
                    let value = adj.value();
                    let upper = adj.upper();
                    let page_size = adj.page_size();

                    // If we're within 100 pixels of the end, request more items
                    if upper - (value + page_size) < 100.0 && upper > 0.0 {
                        sender.emit(SectionRowOutput::LoadMore);
                    }
                });
        }

        let widgets = view_output!();
        widgets
    }

    fn update(&mut self, msg: Self::Input, _sender: FactorySender<Self>) {
        match msg {
            SectionRowInput::SetTitle(title) => {
                self.title = title;
            }
            SectionRowInput::SetItems(items) => {
                self.media_items.guard().clear();
                for item in items {
                    self.media_items.guard().push_back(MediaCardInit {
                        item,
                        show_progress: false, // Could be made conditional
                        watched: false,
                        progress_percent: 0.0,
                        show_media_type_icon: false,
                    });
                }
            }
            SectionRowInput::AppendItems(items) => {
                let mut guard = self.media_items.guard();
                for item in items {
                    guard.push_back(MediaCardInit {
                        item,
                        show_progress: false,
                        watched: false,
                        progress_percent: 0.0,
                        show_media_type_icon: false,
                    });
                }
            }
            SectionRowInput::SetLoading(loading) => {
                self.loading = loading;
            }
            SectionRowInput::ClearItems => {
                self.media_items.guard().clear();
            }
        }
    }
}
