mod media_card;

use std::collections::HashMap;
use std::sync::Arc;

use gtk4::prelude::*;
use libadwaita as adw;
use relm4::prelude::*;
use relm4::typed_view::grid::TypedGridView;
use tracing::info;

use crate::models::library::LibraryType;
use crate::models::media::MediaItem;
use crate::services::artwork::ArtworkCache;
use crate::services::library_filter::{
    self, FilterState, SortOrder,
};
use crate::services::media_source::MediaSource;

use media_card::MediaCardData;

pub struct LibraryView {
    grid: TypedGridView<MediaCardData, gtk4::SingleSelection>,
    library_type: LibraryType,
    source: Option<Arc<dyn MediaSource>>,
    artwork_cache: Option<Arc<ArtworkCache>>,
    // UI widgets
    stack: gtk4::Stack,
    loading_page: adw::StatusPage,
    empty_page: adw::StatusPage,
    error_page: adw::StatusPage,
    no_results_page: adw::StatusPage,
    grid_page: gtk4::ScrolledWindow,
    search_bar: gtk4::SearchBar,
    search_entry: gtk4::SearchEntry,
    // State retention for search/filter/sort
    all_items: Vec<MediaItem>,
    search_query: String,
    filter_state: FilterState,
    sort_order: SortOrder,
    /// Cached textures keyed by artwork URL to avoid re-fetching on grid rebuild.
    texture_cache: HashMap<String, gtk4::gdk::Texture>,
}

#[allow(dead_code)]
pub enum LibraryViewMsg {
    LoadLibrary(LibraryType),
    SetSource(Arc<dyn MediaSource>, Arc<ArtworkCache>),
    ItemActivated(u32),
    LibraryLoaded(Vec<MediaItem>),
    LoadError(String),
    // Search/filter/sort messages
    SearchChanged(String),
    GenreFilterChanged(Vec<String>),
    DecadeFilterChanged(Option<i32>),
    SortChanged(SortOrder),
    ClearFilters,
    FocusSearch,
}

impl std::fmt::Debug for LibraryViewMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadLibrary(lt) => write!(f, "LoadLibrary({lt:?})"),
            Self::SetSource(..) => write!(f, "SetSource(..)"),
            Self::ItemActivated(pos) => write!(f, "ItemActivated({pos})"),
            Self::LibraryLoaded(items) => write!(f, "LibraryLoaded({} items)", items.len()),
            Self::LoadError(msg) => write!(f, "LoadError({msg})"),
            Self::SearchChanged(q) => write!(f, "SearchChanged({q:?})"),
            Self::GenreFilterChanged(g) => write!(f, "GenreFilterChanged({g:?})"),
            Self::DecadeFilterChanged(d) => write!(f, "DecadeFilterChanged({d:?})"),
            Self::SortChanged(s) => write!(f, "SortChanged({s:?})"),
            Self::ClearFilters => write!(f, "ClearFilters"),
            Self::FocusSearch => write!(f, "FocusSearch"),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code, clippy::large_enum_variant)]
pub enum LibraryViewOutput {
    ShowDetail(MediaItem),
    Error(String),
}

pub enum LibraryViewCmd {
    Loaded(Vec<MediaItem>),
    Error(String),
    ArtworkReady(String, gtk4::gdk::Texture),
}

impl std::fmt::Debug for LibraryViewCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LibraryViewCmd")
    }
}

#[relm4::component(pub)]
impl Component for LibraryView {
    type Init = ();
    type Input = LibraryViewMsg;
    type Output = LibraryViewOutput;
    type CommandOutput = LibraryViewCmd;

    view! {
        #[root]
        gtk4::Box {
            set_orientation: gtk4::Orientation::Vertical,
            set_hexpand: true,
            set_vexpand: true,
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = view_output!();

        let stack = gtk4::Stack::new();
        stack.set_hexpand(true);
        stack.set_vexpand(true);

        let loading_page = adw::StatusPage::builder()
            .title("Loading...")
            .icon_name("content-loading-symbolic")
            .build();

        let empty_page = adw::StatusPage::builder()
            .title("No Media")
            .description("Connect a Plex server to browse your library")
            .icon_name("folder-videos-symbolic")
            .build();

        let error_page = adw::StatusPage::builder()
            .title("Error")
            .icon_name("dialog-error-symbolic")
            .build();

        let no_results_page = adw::StatusPage::builder()
            .title("No Results")
            .description("No items match your search and filters")
            .icon_name("edit-find-symbolic")
            .build();

        // Add "Clear Filters" button to no_results_page
        let clear_btn = gtk4::Button::builder()
            .label("Clear Filters")
            .css_classes(["pill", "suggested-action"])
            .halign(gtk4::Align::Center)
            .build();
        let sender_clear = sender.input_sender().clone();
        clear_btn.connect_clicked(move |_| {
            let _ = sender_clear.send(LibraryViewMsg::ClearFilters);
        });
        no_results_page.set_child(Some(&clear_btn));

        let grid: TypedGridView<MediaCardData, gtk4::SingleSelection> = TypedGridView::new();
        let grid_view = grid.view.clone();
        grid_view.set_min_columns(3);
        grid_view.set_max_columns(10);
        grid_view.set_margin_start(12);
        grid_view.set_margin_end(12);
        grid_view.set_margin_top(12);
        grid_view.set_margin_bottom(12);

        let sender_activate = sender.input_sender().clone();
        grid_view.connect_activate(move |_view, position| {
            let _ = sender_activate.send(LibraryViewMsg::ItemActivated(position));
        });

        let grid_page = gtk4::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .child(&grid_view)
            .build();

        // Search bar with entry
        let search_entry = gtk4::SearchEntry::builder()
            .placeholder_text("Search library...")
            .hexpand(true)
            .build();

        let sender_search = sender.input_sender().clone();
        search_entry.connect_search_changed(move |entry| {
            let _ = sender_search.send(LibraryViewMsg::SearchChanged(entry.text().to_string()));
        });

        let sender_stop = sender.input_sender().clone();
        search_entry.connect_stop_search(move |_| {
            let _ = sender_stop.send(LibraryViewMsg::SearchChanged(String::new()));
        });

        let search_bar = gtk4::SearchBar::builder()
            .search_mode_enabled(false)
            .show_close_button(true)
            .child(&search_entry)
            .build();
        search_bar.connect_entry(&search_entry);

        stack.add_child(&loading_page);
        stack.add_child(&empty_page);
        stack.add_child(&error_page);
        stack.add_child(&no_results_page);
        stack.add_child(&grid_page);
        stack.set_visible_child(&empty_page);

        root.append(&search_bar);
        root.append(&stack);

        let model = Self {
            grid,
            library_type: LibraryType::Movie,
            source: None,
            artwork_cache: None,
            stack,
            loading_page,
            empty_page,
            error_page,
            no_results_page,
            grid_page,
            search_bar,
            search_entry,
            all_items: Vec::new(),
            search_query: String::new(),
            filter_state: FilterState::default(),
            sort_order: SortOrder::default(),
            texture_cache: HashMap::new(),
        };

        ComponentParts { model, widgets }
    }

    #[allow(clippy::too_many_lines)]
    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            LibraryViewMsg::SetSource(source, artwork_cache) => {
                self.source = Some(source);
                self.artwork_cache = Some(artwork_cache);
            }
            LibraryViewMsg::LoadLibrary(library_type) => {
                // If same library type and we already have items, just show existing data.
                // This preserves filter/sort state when navigating back from detail pages.
                if library_type == self.library_type && !self.all_items.is_empty() {
                    self.stack.set_visible_child(&self.grid_page);
                    return;
                }

                // Switching library types: reset filters (genres differ) but keep sort
                if library_type != self.library_type {
                    self.filter_state.clear();
                    self.search_query.clear();
                    self.search_bar.set_search_mode(false);
                    self.search_entry.set_text("");
                }

                self.library_type = library_type;
                self.stack.set_visible_child(&self.loading_page);

                let Some(source) = self.source.clone() else {
                    self.stack.set_visible_child(&self.empty_page);
                    return;
                };

                let lt = library_type;
                sender.oneshot_command(async move {
                    match source.libraries().await {
                        Ok(libs) => {
                            let target_type = match lt {
                                LibraryType::Movie => "movie",
                                LibraryType::Show => "show",
                            };
                            let mut all_items = Vec::new();
                            for lib in libs
                                .iter()
                                .filter(|l| l.library_type.as_str() == target_type)
                            {
                                match source.library_items(&lib.key).await {
                                    Ok(items) => all_items.extend(items),
                                    Err(e) => return LibraryViewCmd::Error(e.to_string()),
                                }
                            }
                            LibraryViewCmd::Loaded(all_items)
                        }
                        Err(e) => LibraryViewCmd::Error(e.to_string()),
                    }
                });
            }
            LibraryViewMsg::ItemActivated(position) => {
                if let Some(item) = self.grid.get(position) {
                    let borrow = item.borrow();
                    if let Some(ref media_item) = borrow.media_item {
                        let _ = sender.output(LibraryViewOutput::ShowDetail(media_item.clone()));
                    }
                }
            }
            LibraryViewMsg::LibraryLoaded(items) => {
                if items.is_empty() {
                    self.all_items.clear();
                    self.grid.clear();
                    self.stack.set_visible_child(&self.empty_page);
                    return;
                }

                self.all_items = items;
                info!("Library loaded: {} items", self.all_items.len());
                self.rebuild_grid(&sender);
            }
            LibraryViewMsg::LoadError(msg) => {
                self.error_page.set_description(Some(&msg));
                self.stack.set_visible_child(&self.error_page);
            }
            LibraryViewMsg::SearchChanged(query) => {
                self.search_query = query;
                self.rebuild_grid(&sender);
            }
            LibraryViewMsg::GenreFilterChanged(genres) => {
                if genres.is_empty() {
                    self.filter_state.genres = None;
                } else {
                    self.filter_state.genres = Some(library_filter::GenreFilter {
                        selected_genres: genres,
                    });
                }
                self.rebuild_grid(&sender);
            }
            LibraryViewMsg::DecadeFilterChanged(decade) => {
                self.filter_state.decade =
                    decade.map(|d| library_filter::DecadeFilter { decade_start: d });
                self.rebuild_grid(&sender);
            }
            LibraryViewMsg::SortChanged(order) => {
                self.sort_order = order;
                self.rebuild_grid(&sender);
            }
            LibraryViewMsg::ClearFilters => {
                self.filter_state.clear();
                self.search_query.clear();
                self.search_bar.set_search_mode(false);
                self.search_entry.set_text("");
                self.rebuild_grid(&sender);
            }
            LibraryViewMsg::FocusSearch => {
                self.search_bar.set_search_mode(true);
                self.search_entry.grab_focus();
            }
        }
    }

    fn update_cmd(
        &mut self,
        cmd: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match cmd {
            LibraryViewCmd::Loaded(items) => {
                sender.input(LibraryViewMsg::LibraryLoaded(items));
            }
            LibraryViewCmd::Error(msg) => {
                if !msg.is_empty() {
                    sender.input(LibraryViewMsg::LoadError(msg));
                }
            }
            LibraryViewCmd::ArtworkReady(url, texture) => {
                // Store in cache for future rebuilds
                self.texture_cache.insert(url.clone(), texture.clone());

                // Find and update the matching grid item
                let len = self.grid.len();
                for i in 0..len {
                    if let Some(item) = self.grid.get(i) {
                        let mut borrow = item.borrow_mut();
                        if borrow.poster_url.as_deref() == Some(url.as_str()) && borrow.poster_texture.is_none() {
                            borrow.poster_texture = Some(texture);
                            break;
                        }
                    }
                }
            }
        }
    }
}

impl LibraryView {
    /// Rebuild the grid from all_items using current search/filter/sort state.
    fn rebuild_grid(&mut self, sender: &ComponentSender<Self>) {
        self.grid.clear();

        let filtered_indices = library_filter::apply_filters_and_sort(
            &self.all_items,
            &self.search_query,
            &self.filter_state,
            self.sort_order,
        );

        if filtered_indices.is_empty() {
            // Show no_results if we have items but filters exclude all,
            // or empty_page if we truly have no items.
            if self.all_items.is_empty() {
                self.stack.set_visible_child(&self.empty_page);
            } else {
                self.stack.set_visible_child(&self.no_results_page);
            }
            return;
        }

        let artwork_cache = self.artwork_cache.clone();
        let source = self.source.clone();

        for &item_idx in &filtered_indices {
            let item = &self.all_items[item_idx];
            let mut card = MediaCardData::from_media_item(item);

            // Build the artwork URL and check texture cache
            if let (Some(poster_path), Some(source)) = (&item.poster_path, &source) {
                let url = source.artwork_url(poster_path, 300, 450);
                card.poster_url = Some(url.clone());

                if let Some(texture) = self.texture_cache.get(&url) {
                    // Already cached — set immediately, no async fetch needed
                    card.poster_texture = Some(texture.clone());
                } else if let Some(cache) = &artwork_cache {
                    // Not cached — fetch asynchronously
                    let cache = Arc::clone(cache);
                    let fetch_url = url;
                    sender.oneshot_command(async move {
                        match cache.get_or_download(&fetch_url).await {
                            Ok(path) => match gtk4::gdk::Texture::from_filename(&path) {
                                Ok(texture) => LibraryViewCmd::ArtworkReady(fetch_url, texture),
                                Err(_) => LibraryViewCmd::Error(String::new()),
                            },
                            Err(_) => LibraryViewCmd::Error(String::new()),
                        }
                    });
                }
            }

            self.grid.append(card);
        }

        self.stack.set_visible_child(&self.grid_page);
    }
}
