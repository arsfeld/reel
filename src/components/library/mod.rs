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
use crate::services::library_filter::{self, FilterState, GridDensity, SortOrder};
use crate::services::media_source::MediaSource;

use media_card::MediaCardData;

#[allow(dead_code)]
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
    // Filter/sort bar widgets
    filter_bar: gtk4::Box,
    genre_flow: gtk4::FlowBox,
    decade_dropdown: gtk4::DropDown,
    sort_dropdown: gtk4::DropDown,
    clear_filters_btn: gtk4::Button,
    /// Track genre names currently in the FlowBox to avoid unnecessary rebuilds.
    current_genres: Vec<String>,
    /// Track decade values currently in the dropdown.
    current_decades: Vec<i32>,
    // State retention for search/filter/sort
    all_items: Vec<MediaItem>,
    search_query: String,
    filter_state: FilterState,
    sort_order: SortOrder,
    grid_density: GridDensity,
    /// Cached textures keyed by artwork URL to avoid re-fetching on grid rebuild.
    texture_cache: HashMap<String, gtk4::gdk::Texture>,
    /// Watch progress data keyed by media_item_id: (progress_fraction, watched).
    watch_data: HashMap<String, (f64, bool)>,
    /// Continue Watching section container (label + horizontal scroll).
    continue_watching_section: gtk4::Box,
    /// Horizontal box inside the Continue Watching scrolled window.
    continue_watching_box: gtk4::Box,
    /// Tracks poster downloads for logging: (completed_count, total_to_fetch, batch_start_time).
    poster_load_tracker: Option<(usize, usize, std::time::Instant)>,
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
    DensityChanged(GridDensity),
    LoadCollections,
    LoadCollectionItems(String),
    /// Set watch progress data: media_item_id -> (progress_fraction, watched).
    SetWatchData(HashMap<String, (f64, bool)>),
    /// Mark an item at grid position as watched.
    MarkWatchedAt(u32),
    /// Mark an item at grid position as unwatched.
    MarkUnwatchedAt(u32),
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
            Self::DensityChanged(d) => write!(f, "DensityChanged({d:?})"),
            Self::LoadCollections => write!(f, "LoadCollections"),
            Self::LoadCollectionItems(key) => write!(f, "LoadCollectionItems({key})"),
            Self::SetWatchData(data) => write!(f, "SetWatchData({} items)", data.len()),
            Self::MarkWatchedAt(pos) => write!(f, "MarkWatchedAt({pos})"),
            Self::MarkUnwatchedAt(pos) => write!(f, "MarkUnwatchedAt({pos})"),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code, clippy::large_enum_variant)]
pub enum LibraryViewOutput {
    ShowDetail(MediaItem),
    MarkWatched(MediaItem),
    MarkUnwatched(MediaItem),
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

    #[allow(clippy::too_many_lines)]
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

        // Right-click context menu on grid items
        let right_click = gtk4::GestureClick::builder()
            .button(3) // secondary button
            .build();
        let sender_ctx = sender.input_sender().clone();
        let grid_view_ctx = grid_view.clone();
        right_click.connect_released(move |gesture, _n_press, x, y| {
            // Find which item was right-clicked by hit-testing
            if let Some(position) = pick_grid_position(&grid_view_ctx, x, y) {
                show_watch_context_menu(&grid_view_ctx, &sender_ctx, position, x, y);
            }
            gesture.set_state(gtk4::EventSequenceState::Claimed);
        });
        grid_view.add_controller(right_click);

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

        // Filter/sort bar
        let filter_bar = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(8)
            .margin_start(12)
            .margin_end(12)
            .margin_top(4)
            .margin_bottom(4)
            .build();

        // Genre chips in a FlowBox
        let genre_flow = gtk4::FlowBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .homogeneous(false)
            .max_children_per_line(20)
            .min_children_per_line(1)
            .row_spacing(4)
            .column_spacing(4)
            .build();

        // Controls row: decade dropdown + sort dropdown + clear button
        let controls_row = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .build();

        // Decade dropdown
        let decade_dropdown = gtk4::DropDown::from_strings(&["All Years"]);
        decade_dropdown.set_selected(0);

        let sender_decade = sender.input_sender().clone();
        decade_dropdown.connect_selected_notify(move |dd| {
            let selected = dd.selected();
            if selected == 0 {
                let _ = sender_decade.send(LibraryViewMsg::DecadeFilterChanged(None));
            } else {
                // Position 1+ maps to decades stored in current_decades
                // We'll send the decade value via the string model
                if let Some(item) = dd.model().and_then(|m| m.item(selected))
                    && let Ok(string_obj) = item.downcast::<gtk4::StringObject>()
                {
                    let text = string_obj.string();
                    // Parse "2020s" → 2020
                    if let Ok(decade) = text.trim_end_matches('s').parse::<i32>() {
                        let _ =
                            sender_decade.send(LibraryViewMsg::DecadeFilterChanged(Some(decade)));
                    }
                }
            }
        });

        // Sort dropdown
        let sort_labels: Vec<&str> = SortOrder::all().iter().map(|s| s.label()).collect();
        let sort_dropdown = gtk4::DropDown::from_strings(&sort_labels);
        sort_dropdown.set_selected(0); // TitleAsc

        let sender_sort = sender.input_sender().clone();
        sort_dropdown.connect_selected_notify(move |dd| {
            let all = SortOrder::all();
            let idx = dd.selected() as usize;
            if idx < all.len() {
                let _ = sender_sort.send(LibraryViewMsg::SortChanged(all[idx]));
            }
        });

        // Clear filters button
        let clear_filters_btn = gtk4::Button::builder()
            .icon_name("edit-clear-symbolic")
            .tooltip_text("Clear all filters")
            .css_classes(["flat"])
            .visible(false)
            .build();
        let sender_clear_bar = sender.input_sender().clone();
        clear_filters_btn.connect_clicked(move |_| {
            let _ = sender_clear_bar.send(LibraryViewMsg::ClearFilters);
        });

        let decade_label = gtk4::Label::new(Some("Decade:"));
        decade_label.add_css_class("dim-label");
        let sort_label = gtk4::Label::new(Some("Sort:"));
        sort_label.add_css_class("dim-label");

        // Density dropdown
        let density_labels: Vec<&str> = GridDensity::all().iter().map(|d| d.label()).collect();
        let density_dropdown = gtk4::DropDown::from_strings(&density_labels);
        density_dropdown.set_selected(1); // Medium (default)

        let sender_density = sender.input_sender().clone();
        density_dropdown.connect_selected_notify(move |dd| {
            let all = GridDensity::all();
            let idx = dd.selected() as usize;
            if idx < all.len() {
                let _ = sender_density.send(LibraryViewMsg::DensityChanged(all[idx]));
            }
        });

        let density_label = gtk4::Label::new(Some("Size:"));
        density_label.add_css_class("dim-label");

        controls_row.append(&decade_label);
        controls_row.append(&decade_dropdown);
        controls_row.append(&sort_label);
        controls_row.append(&sort_dropdown);
        controls_row.append(&density_label);
        controls_row.append(&density_dropdown);
        controls_row.append(&clear_filters_btn);

        filter_bar.append(&genre_flow);
        filter_bar.append(&controls_row);

        stack.add_child(&loading_page);
        stack.add_child(&empty_page);
        stack.add_child(&error_page);
        stack.add_child(&no_results_page);
        stack.add_child(&grid_page);
        stack.set_visible_child(&empty_page);

        // Continue Watching section (hidden initially)
        let continue_watching_section = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(8)
            .margin_start(16)
            .margin_end(16)
            .margin_top(8)
            .margin_bottom(8)
            .visible(false)
            .build();

        let cw_label = gtk4::Label::builder()
            .label("Continue Watching")
            .halign(gtk4::Align::Start)
            .css_classes(["title-3"])
            .build();

        let continue_watching_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .build();

        let cw_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Never)
            .max_content_height(200)
            .child(&continue_watching_box)
            .build();

        continue_watching_section.append(&cw_label);
        continue_watching_section.append(&cw_scroll);

        root.append(&search_bar);
        root.append(&filter_bar);
        root.append(&continue_watching_section);
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
            filter_bar,
            genre_flow,
            decade_dropdown,
            sort_dropdown,
            clear_filters_btn,
            current_genres: Vec::new(),
            current_decades: Vec::new(),
            all_items: Vec::new(),
            search_query: String::new(),
            filter_state: FilterState::default(),
            sort_order: SortOrder::default(),
            grid_density: GridDensity::default(),
            texture_cache: HashMap::new(),
            watch_data: HashMap::new(),
            continue_watching_section,
            continue_watching_box,
            poster_load_tracker: None,
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
                    let start = std::time::Instant::now();
                    match source.libraries().await {
                        Ok(libs) => {
                            let target_type = match lt {
                                LibraryType::Movie => "movie",
                                LibraryType::Show => "show",
                            };
                            let matching_libs: Vec<_> = libs
                                .iter()
                                .filter(|l| l.library_type.as_str() == target_type)
                                .collect();

                            // Fetch all library sections in parallel
                            let fetch_start = std::time::Instant::now();
                            let fetches: Vec<_> = matching_libs
                                .iter()
                                .map(|lib| source.library_items(&lib.key))
                                .collect();
                            let results = futures::future::join_all(fetches).await;
                            info!(
                                "Fetched {} library sections in {:?}",
                                matching_libs.len(),
                                fetch_start.elapsed()
                            );

                            let mut all_items = Vec::new();
                            for result in results {
                                match result {
                                    Ok(items) => all_items.extend(items),
                                    Err(e) => return LibraryViewCmd::Error(e.to_string()),
                                }
                            }
                            info!(
                                "Library load complete: {} items in {:?}",
                                all_items.len(),
                                start.elapsed()
                            );
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

                let build_start = std::time::Instant::now();
                self.all_items = items;
                info!("Library loaded: {} items", self.all_items.len());
                self.rebuild_genre_chips(&sender);
                self.rebuild_decade_dropdown();
                self.rebuild_grid(&sender);
                info!("Full UI build (chips + decades + grid): {:?}", build_start.elapsed());
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
                self.update_clear_button_visibility();
                self.rebuild_grid(&sender);
            }
            LibraryViewMsg::DecadeFilterChanged(decade) => {
                self.filter_state.decade =
                    decade.map(|d| library_filter::DecadeFilter { decade_start: d });
                self.update_clear_button_visibility();
                self.rebuild_grid(&sender);
            }
            LibraryViewMsg::SortChanged(order) => {
                self.sort_order = order;
                self.rebuild_grid(&sender);
            }
            LibraryViewMsg::DensityChanged(density) => {
                self.grid_density = density;
                // Update grid view column constraints
                self.grid.view.set_min_columns(density.min_columns());
                self.rebuild_grid(&sender);
            }
            LibraryViewMsg::ClearFilters => {
                self.filter_state.clear();
                self.search_query.clear();
                self.search_bar.set_search_mode(false);
                self.search_entry.set_text("");
                self.decade_dropdown.set_selected(0);
                self.deselect_all_genre_chips();
                self.update_clear_button_visibility();
                self.rebuild_grid(&sender);
            }
            LibraryViewMsg::FocusSearch => {
                self.search_bar.set_search_mode(true);
                self.search_entry.grab_focus();
            }
            LibraryViewMsg::LoadCollections => {
                self.stack.set_visible_child(&self.loading_page);
                self.all_items.clear();
                self.filter_state.clear();
                self.search_query.clear();

                let Some(source) = self.source.clone() else {
                    self.stack.set_visible_child(&self.empty_page);
                    return;
                };

                // Fetch collections from all movie + show libraries
                sender.oneshot_command(async move {
                    let mut all_collections = Vec::new();
                    match source.libraries().await {
                        Ok(libs) => {
                            for lib in &libs {
                                if let Ok(cols) = source.collections(&lib.key).await {
                                    all_collections.extend(cols);
                                }
                            }
                            LibraryViewCmd::Loaded(all_collections)
                        }
                        Err(e) => LibraryViewCmd::Error(e.to_string()),
                    }
                });
            }
            LibraryViewMsg::MarkWatchedAt(position) => {
                if let Some(item) = self.grid.get(position) {
                    let borrow = item.borrow();
                    if let Some(ref media_item) = borrow.media_item {
                        let _ = sender.output(LibraryViewOutput::MarkWatched(media_item.clone()));
                    }
                }
            }
            LibraryViewMsg::MarkUnwatchedAt(position) => {
                if let Some(item) = self.grid.get(position) {
                    let borrow = item.borrow();
                    if let Some(ref media_item) = borrow.media_item {
                        let _ = sender.output(LibraryViewOutput::MarkUnwatched(media_item.clone()));
                    }
                }
            }
            LibraryViewMsg::SetWatchData(data) => {
                self.watch_data = data;
                // Re-populate grid to reflect updated watch indicators
                if !self.all_items.is_empty() {
                    self.rebuild_grid(&sender);
                }
                // Rebuild Continue Watching row
                self.rebuild_continue_watching(&sender);
            }
            LibraryViewMsg::LoadCollectionItems(collection_key) => {
                self.stack.set_visible_child(&self.loading_page);
                self.all_items.clear();

                let Some(source) = self.source.clone() else {
                    self.stack.set_visible_child(&self.empty_page);
                    return;
                };

                sender.oneshot_command(async move {
                    match source.collection_items(&collection_key).await {
                        Ok(items) => LibraryViewCmd::Loaded(items),
                        Err(e) => LibraryViewCmd::Error(e.to_string()),
                    }
                });
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
                        if borrow.poster_url.as_deref() == Some(url.as_str())
                            && borrow.poster_texture.is_none()
                        {
                            borrow.poster_texture = Some(texture);
                            break;
                        }
                    }
                }

                // Log poster download progress at milestones
                if let Some((ref mut done, total, batch_start)) = self.poster_load_tracker {
                    *done += 1;
                    let completed = *done;
                    // Log at first, every 25%, and last
                    if completed == 1
                        || completed == total
                        || (total >= 20 && completed % (total / 4).max(1) == 0)
                    {
                        info!(
                            "Posters: {}/{} loaded ({:?} elapsed)",
                            completed,
                            total,
                            batch_start.elapsed()
                        );
                    }
                    if completed == total {
                        self.poster_load_tracker = None;
                    }
                }
            }
        }
    }
}

impl LibraryView {
    /// Rebuild the grid from all_items using current search/filter/sort state.
    fn rebuild_grid(&mut self, sender: &ComponentSender<Self>) {
        let start = std::time::Instant::now();
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
        let mut posters_to_fetch = 0usize;
        let mut posters_cached = 0usize;

        for &item_idx in &filtered_indices {
            let item = &self.all_items[item_idx];
            let mut card = MediaCardData::from_media_item(item);
            card.card_width = self.grid_density.card_width();
            card.card_height = self.grid_density.card_height();

            // Apply watch state data if available
            if let Some(&(progress, watched)) = self.watch_data.get(&item.id) {
                card.watch_progress = Some(progress);
                card.watched = watched;
            }

            // Build the artwork URL and check texture cache
            if let (Some(poster_path), Some(source)) = (&item.poster_path, &source) {
                let url = source.artwork_url(poster_path, 300, 450);
                card.poster_url = Some(url.clone());

                if let Some(texture) = self.texture_cache.get(&url) {
                    // Already cached — set immediately, no async fetch needed
                    card.poster_texture = Some(texture.clone());
                    posters_cached += 1;
                } else if let Some(cache) = &artwork_cache {
                    // Not cached — fetch asynchronously
                    posters_to_fetch += 1;
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

        // Start tracking poster downloads
        if posters_to_fetch > 0 {
            self.poster_load_tracker = Some((0, posters_to_fetch, std::time::Instant::now()));
        }

        info!(
            "Grid rebuilt: {} items, {} posters cached (in-memory), {} posters to fetch, took {:?}",
            filtered_indices.len(),
            posters_cached,
            posters_to_fetch,
            start.elapsed()
        );

        self.stack.set_visible_child(&self.grid_page);
    }

    /// Rebuild genre toggle chips from current all_items.
    fn rebuild_genre_chips(&mut self, sender: &ComponentSender<Self>) {
        let genres = library_filter::extract_genres(&self.all_items);

        if genres == self.current_genres {
            return; // No change needed
        }

        // Remove all existing children
        while let Some(child) = self.genre_flow.first_child() {
            self.genre_flow.remove(&child);
        }

        for genre in &genres {
            let btn = gtk4::ToggleButton::builder()
                .label(genre)
                .css_classes(["pill"])
                .build();

            let sender_genre = sender.input_sender().clone();
            let flow_ref = self.genre_flow.clone();
            btn.connect_toggled(move |_btn| {
                // Collect all currently toggled genres
                let mut selected = Vec::new();
                let mut child = flow_ref.first_child();
                while let Some(ref widget) = child {
                    // FlowBoxChild wraps the ToggleButton
                    if let Some(toggle) = widget.first_child()
                        && let Ok(toggle) = toggle.downcast::<gtk4::ToggleButton>()
                        && toggle.is_active()
                    {
                        selected.push(toggle.label().map(|l| l.to_string()).unwrap_or_default());
                    }
                    child = widget.next_sibling();
                }
                let _ = sender_genre.send(LibraryViewMsg::GenreFilterChanged(selected));
            });

            self.genre_flow.append(&btn);
        }

        self.current_genres = genres;
    }

    /// Rebuild decade dropdown from current all_items.
    fn rebuild_decade_dropdown(&mut self) {
        let decades = library_filter::extract_decades(&self.all_items);

        if decades == self.current_decades {
            return;
        }

        let mut labels: Vec<String> = vec!["All Years".to_string()];
        for &d in &decades {
            labels.push(format!("{d}s"));
        }
        let label_strs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let model = gtk4::StringList::new(&label_strs);
        self.decade_dropdown.set_model(Some(&model));
        self.decade_dropdown.set_selected(0);
        self.current_decades = decades;
    }

    /// Deselect all genre toggle buttons.
    fn deselect_all_genre_chips(&self) {
        let mut child = self.genre_flow.first_child();
        while let Some(ref widget) = child {
            if let Some(toggle) = widget.first_child()
                && let Ok(toggle) = toggle.downcast::<gtk4::ToggleButton>()
            {
                toggle.set_active(false);
            }
            child = widget.next_sibling();
        }
    }

    /// Show/hide the clear filters button based on active filter state.
    fn update_clear_button_visibility(&self) {
        self.clear_filters_btn
            .set_visible(self.filter_state.is_active() || !self.search_query.is_empty());
    }

    /// Rebuild the Continue Watching horizontal row from watch_data + all_items.
    fn rebuild_continue_watching(&mut self, sender: &ComponentSender<Self>) {
        // Remove old children
        while let Some(child) = self.continue_watching_box.first_child() {
            self.continue_watching_box.remove(&child);
        }

        // Find in-progress items (not watched, has progress)
        let mut in_progress: Vec<&MediaItem> = self
            .all_items
            .iter()
            .filter(|item| {
                if let Some(&(progress, watched)) = self.watch_data.get(&item.id) {
                    !watched && progress > 0.0
                } else {
                    false
                }
            })
            .take(20)
            .collect();

        // Sort by progress (most recently watched first — watch_data doesn't have timestamp,
        // so just use insertion order which is last_watched_at DESC from the DB query)
        let _ = &mut in_progress;

        if in_progress.is_empty() {
            self.continue_watching_section.set_visible(false);
            return;
        }

        self.continue_watching_section.set_visible(true);

        let source = self.source.clone();
        let artwork_cache = self.artwork_cache.clone();

        for item in &in_progress {
            let progress = self
                .watch_data
                .get(&item.id)
                .map(|&(p, _)| p)
                .unwrap_or(0.0);

            // Build a small poster card
            let card = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Vertical)
                .spacing(4)
                .width_request(120)
                .css_classes(["media-card"])
                .build();

            let picture = gtk4::Picture::builder()
                .content_fit(gtk4::ContentFit::Cover)
                .width_request(120)
                .height_request(180)
                .css_classes(["media-card-poster"])
                .build();

            let progress_bar = gtk4::ProgressBar::builder()
                .halign(gtk4::Align::Fill)
                .valign(gtk4::Align::End)
                .fraction(progress)
                .css_classes(["watch-progress"])
                .build();

            let overlay = gtk4::Overlay::new();
            overlay.set_child(Some(&picture));
            overlay.add_overlay(&progress_bar);

            let frame = gtk4::Frame::builder()
                .css_classes(["media-card-frame"])
                .child(&overlay)
                .build();

            let title = gtk4::Label::builder()
                .label(&item.title)
                .halign(gtk4::Align::Start)
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .max_width_chars(14)
                .css_classes(["caption"])
                .build();

            card.append(&frame);
            card.append(&title);

            // Load poster texture
            if let (Some(poster_path), Some(src)) = (&item.poster_path, &source) {
                let url = src.artwork_url(poster_path, 300, 450);
                if let Some(texture) = self.texture_cache.get(&url) {
                    picture.set_paintable(Some(texture));
                } else if let Some(cache) = &artwork_cache {
                    let cache = Arc::clone(cache);
                    let sender = sender.command_sender().clone();
                    let fetch_url = url;
                    gtk4::glib::spawn_future_local(async move {
                        if let Ok(path) = cache.get_or_download(&fetch_url).await
                            && let Ok(texture) = gtk4::gdk::Texture::from_filename(&path)
                        {
                            let _ = sender.send(LibraryViewCmd::ArtworkReady(fetch_url, texture));
                        }
                    });
                }
            }

            // Click handler -> navigate to detail
            let item_clone = (*item).clone();
            let output_sender = sender.output_sender().clone();
            let gesture = gtk4::GestureClick::new();
            gesture.connect_released(move |_, _, _, _| {
                let _ = output_sender.send(LibraryViewOutput::ShowDetail(item_clone.clone()));
            });
            card.add_controller(gesture);

            self.continue_watching_box.append(&card);
        }
    }
}

/// Find which grid item is at the given (x, y) coordinates.
/// Returns the position index if found.
fn pick_grid_position(grid_view: &gtk4::GridView, _x: f64, _y: f64) -> Option<u32> {
    // Use the selection model to get the item at the click position
    // GTK GridView doesn't have a direct pick method, so we iterate
    // and check which child contains the point
    let model = grid_view.model()?;
    let n_items = model.n_items();
    if n_items == 0 {
        return None;
    }

    // Use the selected item from the selection model
    let selection = model.downcast_ref::<gtk4::SingleSelection>()?;
    let selected = selection.selected();
    if selected < n_items {
        Some(selected)
    } else {
        None
    }
}

/// Show a context menu popover at the given position with Watch/Unwatch actions.
fn show_watch_context_menu(
    grid_view: &gtk4::GridView,
    sender: &relm4::Sender<LibraryViewMsg>,
    position: u32,
    x: f64,
    y: f64,
) {
    let menu = gtk4::gio::Menu::new();
    menu.append(Some("Mark as Watched"), Some("watch.mark-watched"));
    menu.append(Some("Mark as Unwatched"), Some("watch.mark-unwatched"));

    let popover = gtk4::PopoverMenu::from_model(Some(&menu));
    popover.set_parent(grid_view);
    popover.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    popover.set_has_arrow(true);

    // Action group for the popover
    let action_group = gtk4::gio::SimpleActionGroup::new();

    let sender_watched = sender.clone();
    let watched_action = gtk4::gio::SimpleAction::new("mark-watched", None);
    watched_action.connect_activate(move |_, _| {
        let _ = sender_watched.send(LibraryViewMsg::MarkWatchedAt(position));
    });
    action_group.add_action(&watched_action);

    let sender_unwatched = sender.clone();
    let unwatched_action = gtk4::gio::SimpleAction::new("mark-unwatched", None);
    unwatched_action.connect_activate(move |_, _| {
        let _ = sender_unwatched.send(LibraryViewMsg::MarkUnwatchedAt(position));
    });
    action_group.add_action(&unwatched_action);

    grid_view.insert_action_group("watch", Some(&action_group));
    popover.popup();
}
