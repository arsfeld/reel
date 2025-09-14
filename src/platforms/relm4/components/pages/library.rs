use gtk::prelude::*;
use libadwaita as adw;
use relm4::factory::FactoryVecDeque;
use relm4::gtk;
use relm4::prelude::*;
use tracing::{debug, error};

use crate::db::connection::DatabaseConnection;
use crate::db::entities::MediaItemModel;
use crate::models::{LibraryId, MediaItemId};
use crate::platforms::relm4::components::factories::media_card::{
    MediaCard, MediaCardInit, MediaCardOutput,
};

#[derive(Debug)]
pub struct LibraryPage {
    db: DatabaseConnection,
    library_id: Option<LibraryId>,
    media_factory: FactoryVecDeque<MediaCard>,
    is_loading: bool,
    current_page: usize,
    items_per_page: usize,
    has_more: bool,
    view_mode: ViewMode,
    sort_by: SortBy,
    filter_text: String,
    search_visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    Grid,
    List,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortBy {
    Title,
    Year,
    DateAdded,
    Rating,
}

#[derive(Debug)]
pub enum LibraryPageInput {
    /// Set the library to display
    SetLibrary(LibraryId),
    /// Load the next page of items
    LoadMore,
    /// Media items loaded
    MediaItemsLoaded {
        items: Vec<MediaItemModel>,
        has_more: bool,
    },
    /// Media item selected
    MediaItemSelected(MediaItemId),
    /// Change view mode
    SetViewMode(ViewMode),
    /// Change sort order
    SetSortBy(SortBy),
    /// Filter by text
    SetFilter(String),
    /// Clear all items and reload
    Refresh,
    /// Show search bar
    ShowSearch,
    /// Hide search bar
    HideSearch,
}

#[derive(Debug)]
pub enum LibraryPageOutput {
    /// Navigate to media item
    NavigateToMediaItem(MediaItemId),
}

#[relm4::component(pub async)]
impl AsyncComponent for LibraryPage {
    type Init = DatabaseConnection;
    type Input = LibraryPageInput;
    type Output = LibraryPageOutput;
    type CommandOutput = ();

    view! {
        gtk::Overlay {
            set_can_focus: true,
            grab_focus: (),

            // Add keyboard event controller to capture typing
            add_controller = gtk::EventControllerKey {
                connect_key_pressed[sender] => move |_, key, _, _| {
                    // Show search on slash or Control+F
                    if key == gtk::gdk::Key::slash || key == gtk::gdk::Key::f {
                        sender.input(LibraryPageInput::ShowSearch);
                        gtk::glib::Propagation::Stop
                    }
                    // Hide search on Escape
                    else if key == gtk::gdk::Key::Escape {
                        sender.input(LibraryPageInput::HideSearch);
                        gtk::glib::Propagation::Stop
                    } else {
                        gtk::glib::Propagation::Proceed
                    }
                }
            },

            // Main content
            gtk::ScrolledWindow {
                set_vexpand: true,
                set_hscrollbar_policy: gtk::PolicyType::Never,
                set_vscrollbar_policy: gtk::PolicyType::Automatic,

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 0,

                    #[local_ref]
                    media_box -> gtk::FlowBox {
                        set_column_spacing: 12,  // Tighter grid spacing
                        set_row_spacing: 16,      // Reduced vertical spacing
                        set_homogeneous: true,
                        set_min_children_per_line: 4,   // More items per row with smaller sizes
                        set_max_children_per_line: 12,  // Allow more on wide screens
                        set_selection_mode: gtk::SelectionMode::None,
                        set_margin_top: 24,
                        set_margin_bottom: 16,
                        set_margin_start: 16,
                        set_margin_end: 16,
                        set_valign: gtk::Align::Start,
                    },

                    // Loading indicator at bottom
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_halign: gtk::Align::Center,
                        set_margin_all: 12,
                        #[watch]
                        set_visible: model.is_loading,

                        gtk::Spinner {
                            set_spinning: true,
                        },

                        gtk::Label {
                            set_text: "Loading more...",
                            set_margin_start: 12,
                            add_css_class: "dim-label",
                        },
                    },

                    // Empty state - modern design with large icon
                    adw::StatusPage {
                        #[watch]
                        set_visible: !model.is_loading && model.media_factory.is_empty(),
                        set_icon_name: Some("folder-videos-symbolic"),
                        set_title: "No Media Found",
                        set_description: Some("This library is empty or still syncing"),
                        add_css_class: "compact",
                    }
                },
            },

            // Floating search bar overlay
            add_overlay = &gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Start,
                set_margin_top: 12,
                #[watch]
                set_visible: model.search_visible,
                add_css_class: "osd",
                add_css_class: "toolbar",
                set_css_classes: &["osd", "toolbar", "floating-search"],

                #[name = "search_entry"]
                gtk::SearchEntry {
                    set_placeholder_text: Some("Type to search..."),
                    set_width_request: 350,
                    #[watch]
                    set_text: &model.filter_text,
                    connect_search_changed[sender] => move |entry| {
                        sender.input(LibraryPageInput::SetFilter(entry.text().to_string()));
                    },
                    connect_stop_search[sender] => move |_| {
                        sender.input(LibraryPageInput::HideSearch);
                    }
                },
            }
        }
    }

    async fn init(
        db: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let media_box = gtk::FlowBox::default();

        let media_factory = FactoryVecDeque::<MediaCard>::builder()
            .launch(media_box.clone())
            .forward(sender.input_sender(), |output| match output {
                MediaCardOutput::Clicked(id) => LibraryPageInput::MediaItemSelected(id),
                MediaCardOutput::Play(id) => LibraryPageInput::MediaItemSelected(id),
            });

        let model = Self {
            db,
            library_id: None,
            media_factory,
            is_loading: false,
            current_page: 0,
            items_per_page: 50,
            has_more: false,
            view_mode: ViewMode::Grid,
            sort_by: SortBy::Title,
            filter_text: String::new(),
            search_visible: false,
        };

        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            LibraryPageInput::SetLibrary(library_id) => {
                debug!("Setting library: {}", library_id);
                self.library_id = Some(library_id.clone());
                self.current_page = 0;
                self.media_factory.guard().clear();
                self.load_page(sender.clone());
            }

            LibraryPageInput::LoadMore => {
                if !self.is_loading && self.has_more {
                    debug!("Loading more items");
                    self.current_page += 1;
                    self.load_page(sender.clone());
                }
            }

            LibraryPageInput::MediaItemsLoaded { items, has_more } => {
                debug!("Loaded {} items", items.len());

                for item in items {
                    self.media_factory.guard().push_back(MediaCardInit {
                        item,
                        show_progress: false,
                    });
                }

                self.has_more = has_more;
                self.is_loading = false;
            }

            LibraryPageInput::MediaItemSelected(item_id) => {
                debug!("Media item selected: {}", item_id);
                sender
                    .output(LibraryPageOutput::NavigateToMediaItem(item_id))
                    .unwrap();
            }

            LibraryPageInput::SetViewMode(mode) => {
                debug!("Setting view mode: {:?}", mode);
                self.view_mode = mode;
                // TODO: Update FlowBox layout based on view mode
            }

            LibraryPageInput::SetSortBy(sort_by) => {
                debug!("Setting sort by: {:?}", sort_by);
                self.sort_by = sort_by;
                self.refresh(sender.clone());
            }

            LibraryPageInput::SetFilter(filter) => {
                debug!("Setting filter: {}", filter);
                self.filter_text = filter;
                self.refresh(sender.clone());
            }

            LibraryPageInput::Refresh => {
                debug!("Refreshing library");
                self.refresh(sender.clone());
            }

            LibraryPageInput::ShowSearch => {
                self.search_visible = true;
            }

            LibraryPageInput::HideSearch => {
                self.search_visible = false;
                if !self.filter_text.is_empty() {
                    self.filter_text.clear();
                    self.refresh(sender.clone());
                }
            }
        }
    }
}

impl LibraryPage {
    fn load_page(&mut self, sender: AsyncComponentSender<Self>) {
        if let Some(library_id) = &self.library_id {
            self.is_loading = true;

            let db = self.db.clone();
            let library_id = library_id.clone();
            let page = self.current_page;
            let items_per_page = self.items_per_page;
            let sort_by = self.sort_by;
            let filter = self.filter_text.clone();

            relm4::spawn(async move {
                use crate::db::repository::{MediaRepository, MediaRepositoryImpl};
                let repo = MediaRepositoryImpl::new(db.clone());

                // Calculate offset
                let offset = page * items_per_page;

                // Get items for this library with pagination
                match repo
                    .find_by_library_paginated(
                        &library_id.to_string(),
                        offset as u64,
                        items_per_page as u64,
                    )
                    .await
                {
                    Ok(items) => {
                        let has_more = items.len() == items_per_page;

                        // Apply client-side filtering if needed
                        let filtered_items = if filter.is_empty() {
                            items
                        } else {
                            items
                                .into_iter()
                                .filter(|item| {
                                    item.title.to_lowercase().contains(&filter.to_lowercase())
                                })
                                .collect()
                        };

                        sender.input(LibraryPageInput::MediaItemsLoaded {
                            items: filtered_items,
                            has_more,
                        });
                    }
                    Err(e) => {
                        error!("Failed to load library items: {}", e);
                        sender.input(LibraryPageInput::MediaItemsLoaded {
                            items: Vec::new(),
                            has_more: false,
                        });
                    }
                }
            });
        }
    }

    fn refresh(&mut self, sender: AsyncComponentSender<Self>) {
        self.current_page = 0;
        self.media_factory.guard().clear();
        self.load_page(sender);
    }
}
