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
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 0,
            add_css_class: "background",

            // Header with filters
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 12,
                add_css_class: "toolbar",
                set_margin_all: 12,

                // Title and view controls
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 12,

                    gtk::Label {
                        set_text: "Library",
                        set_halign: gtk::Align::Start,
                        set_hexpand: true,
                        add_css_class: "title-2",
                    },

                    // View mode toggle
                    gtk::Box {
                        set_spacing: 0,
                        add_css_class: "linked",

                        gtk::ToggleButton {
                            set_icon_name: "view-grid-symbolic",
                            set_tooltip_text: Some("Grid View"),
                            #[watch]
                            set_active: model.view_mode == ViewMode::Grid,
                            connect_toggled[sender] => move |btn| {
                                if btn.is_active() {
                                    sender.input(LibraryPageInput::SetViewMode(ViewMode::Grid));
                                }
                            }
                        },

                        gtk::ToggleButton {
                            set_icon_name: "view-list-symbolic",
                            set_tooltip_text: Some("List View"),
                            #[watch]
                            set_active: model.view_mode == ViewMode::List,
                            connect_toggled[sender] => move |btn| {
                                if btn.is_active() {
                                    sender.input(LibraryPageInput::SetViewMode(ViewMode::List));
                                }
                            }
                        },
                    },
                },

                // Filter bar
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 12,

                    // Search entry matching GTK version
                    gtk::SearchEntry {
                        set_placeholder_text: Some("Search movies..."),
                        set_hexpand: true,
                        connect_search_changed[sender] => move |entry| {
                            sender.input(LibraryPageInput::SetFilter(entry.text().to_string()));
                        }
                    },

                    // Sort dropdown
                    gtk::DropDown {
                        set_model: Some(&gtk::StringList::new(&[
                            "Title",
                            "Year",
                            "Date Added",
                            "Rating",
                        ])),
                        #[watch]
                        set_selected: model.sort_by as u32,
                        connect_selected_notify[sender] => move |dropdown| {
                            let sort_by = match dropdown.selected() {
                                0 => SortBy::Title,
                                1 => SortBy::Year,
                                2 => SortBy::DateAdded,
                                3 => SortBy::Rating,
                                _ => SortBy::Title,
                            };
                            sender.input(LibraryPageInput::SetSortBy(sort_by));
                        }
                    },

                    // Refresh button
                    gtk::Button {
                        set_icon_name: "view-refresh-symbolic",
                        set_tooltip_text: Some("Refresh"),
                        add_css_class: "flat",
                        connect_clicked[sender] => move |_| {
                            sender.input(LibraryPageInput::Refresh);
                        }
                    },
                },
            },

            // Content area - matching GTK library page exactly
            gtk::ScrolledWindow {
                set_vexpand: true,
                set_hscrollbar_policy: gtk::PolicyType::Never,
                set_vscrollbar_policy: gtk::PolicyType::Automatic,

                #[local_ref]
                media_box -> gtk::FlowBox {
                    set_column_spacing: 16,  // Tighter spacing like GTK version
                    set_row_spacing: 20,      // Good vertical spacing
                    set_homogeneous: true,
                    set_min_children_per_line: 4,   // More items per row with smaller sizes
                    set_max_children_per_line: 12,  // Allow more on wide screens
                    set_selection_mode: gtk::SelectionMode::None,
                    set_margin_top: 16,
                    set_margin_bottom: 16,
                    set_margin_start: 16,
                    set_margin_end: 16,
                    set_valign: gtk::Align::Start,

                    // Scroll detection for infinite scrolling
                    add_controller = gtk::EventControllerScroll {
                        set_flags: gtk::EventControllerScrollFlags::VERTICAL,
                        connect_scroll[sender, has_more = model.has_more] => move |_, _, dy| {
                            // Load more when scrolling near bottom
                            if dy > 0.0 && has_more {
                                sender.input(LibraryPageInput::LoadMore);
                            }
                            gtk::glib::Propagation::Proceed
                        }
                    }
                },
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

            // Empty state
            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 12,
                set_valign: gtk::Align::Center,
                set_vexpand: true,
                set_margin_all: 48,
                #[watch]
                set_visible: !model.is_loading && model.media_factory.is_empty(),

                gtk::Image {
                    set_icon_name: Some("folder-videos-symbolic"),
                    set_pixel_size: 128,
                    add_css_class: "dim-label",
                },

                gtk::Label {
                    set_text: "No Media Found",
                    add_css_class: "title-2",
                },

                gtk::Label {
                    set_text: "This library is empty or still syncing",
                    add_css_class: "dim-label",
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
                let repo = MediaRepositoryImpl::new_without_events(db.clone());

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
