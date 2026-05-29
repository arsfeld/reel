mod active_filters;
mod filter_popover;
mod media_card;

use std::collections::HashMap;
use std::sync::Arc;

use adw;
use gtk::prelude::*;
use relm4::prelude::*;
use relm4::typed_view::grid::TypedGridView;
use tracing::info;

use crate::models::library::{LibrarySection, LibraryType};
use crate::models::media::MediaItem;
use crate::services::artwork::ArtworkCache;
use crate::services::library_filter::{
    self, FilterState, FilterTag, GridDensity, SortOrder, ViewMode,
};
use crate::services::media_source::MediaSource;
use crate::settings::{LibrarySettings, LibraryUiState};

use active_filters::ActiveFiltersBar;
use filter_popover::FilterPopover;
use media_card::MediaCardData;

#[allow(dead_code)]
pub struct LibraryView {
    grid: TypedGridView<MediaCardData, gtk::SingleSelection>,
    library_type: LibraryType,
    /// Plex section key of the currently-loaded library. Drives the cache
    /// short-circuit so switching between same-type libraries refetches.
    library_key: Option<String>,
    source: Option<Arc<dyn MediaSource>>,
    artwork_cache: Option<Arc<ArtworkCache>>,
    // UI widgets
    stack: gtk::Stack,
    loading_page: adw::StatusPage,
    empty_page: adw::StatusPage,
    error_page: adw::StatusPage,
    no_results_page: adw::StatusPage,
    grid_page: gtk::ScrolledWindow,
    /// Overlay wrapping grid_page + alphabet bar.
    grid_overlay: gtk::Overlay,
    search_bar: gtk::SearchBar,
    search_entry: gtk::SearchEntry,
    // Filter/sort bar widgets
    filter_bar: gtk::Box,
    filter_button: gtk::MenuButton,
    filter_dot: gtk::Image,
    library_title: gtk::Label,
    sort_dropdown: gtk::DropDown,
    /// Signal handler for the sort dropdown, blocked during programmatic
    /// restore so it doesn't re-emit SortChanged (and re-persist) on load.
    sort_handler: gtk::glib::SignalHandlerId,
    /// Adwaita filter popover (7 filter groups). Owns the popover the filter
    /// button displays.
    filter_popover: FilterPopover,
    /// Active-filter pill row shown above the grid.
    active_filters_bar: ActiveFiltersBar,
    // State retention for search/filter/sort
    all_items: Vec<MediaItem>,
    search_query: String,
    filter_state: FilterState,
    sort_order: SortOrder,
    grid_density: GridDensity,
    /// Persisted per-library filter/sort state, seeded from settings on
    /// startup. Read on library load to restore; never written here (saves go
    /// out as `SaveLibraryUiState` outputs).
    saved_ui: LibrarySettings,
    /// Composite key for the currently-loaded library, set once items load.
    library_id: Option<String>,
    /// Cached textures keyed by artwork URL to avoid re-fetching on grid rebuild.
    texture_cache: HashMap<String, gtk::gdk::Texture>,
    /// Effective watch data keyed by media_item_id: (progress_fraction,
    /// watched). Each source's own reported state (e.g. Plex view count/offset)
    /// takes precedence over `local_watch_data`; recomputed when either input
    /// changes. This is what the grid, filters, and Continue Watching read.
    watch_data: HashMap<String, (f64, bool)>,
    /// Locally tracked watch progress from the DB, used as a fallback for items
    /// whose source reports no watch state (non-Plex sources, offline play).
    local_watch_data: HashMap<String, (f64, bool)>,
    /// Media ids with a completed offline download, for the "downloaded" badge
    /// (R14). Pushed by the app via [`LibraryViewMsg::SetDownloadedIds`].
    downloaded_ids: std::collections::HashSet<String>,
    /// Continue Watching section container (label + horizontal scroll).
    continue_watching_section: gtk::Box,
    /// Horizontal box inside the Continue Watching scrolled window.
    continue_watching_box: gtk::Box,
    /// Tracks poster downloads for logging: (completed_count, total_to_fetch, batch_start_time).
    poster_load_tracker: Option<(usize, usize, std::time::Instant)>,
    /// True when an async library fetch is in flight; prevents redundant
    /// concurrent LoadLibrary calls from starting duplicate fetches.
    loading_in_progress: bool,
    /// Active filter count badge shown on the filter button.
    filter_count_badge: gtk::Label,
    /// Hero header container (icon + title + subtitle).
    library_hero: gtk::Box,
    /// Hero subtitle label (e.g., "1,247 movies · Browse your collection").
    library_hero_subtitle: gtk::Label,
    /// View mode: grid or list.
    view_mode: ViewMode,
    /// Skeleton grid container for loading state.
    skeleton_grid: gtk::Box,
    /// Scrolled window wrapping skeleton grid.
    skeleton_scroll: gtk::ScrolledWindow,
    /// Alphabet jump bar overlay.
    alphabet_bar: gtk::Box,
    /// List view widget (alternative to grid).
    list_box: gtk::ListBox,
    /// Scrolled window wrapping the list view.
    list_scroll: gtk::ScrolledWindow,
}

#[allow(dead_code)]
pub enum LibraryViewMsg {
    LoadLibrary(LibrarySection),
    SetSource(Arc<dyn MediaSource>, Arc<ArtworkCache>),
    /// Seed the persisted per-library filter/sort state (from app settings).
    SetSavedUiState(LibrarySettings),
    ItemActivated(u32),
    LibraryLoaded(Vec<MediaItem>),
    /// Render a library instantly from session-cached items (no skeleton, no
    /// fetch). Emitted by App on a content-cache hit.
    ShowCached(LibrarySection, Vec<MediaItem>),
    /// Apply a background-revalidation result to the displayed library in place
    /// (no skeleton, no loading page; current filter/sort/search preserved).
    /// App sends this only when the content actually changed. `cache_key` is the
    /// full composite ({source}:{section}) so a result for one server's section
    /// can't be applied to another server's same-numbered section.
    ApplyRevalidated {
        cache_key: String,
        items: Vec<MediaItem>,
    },
    LoadError(String),
    // Search/filter/sort messages
    SearchChanged(String),
    /// Full filter state changed (emitted by the filter popover).
    FilterStateChanged(FilterState),
    /// Remove a single active filter value (emitted by a pill's ✘).
    RemoveActiveFilter(FilterTag),
    SortChanged(SortOrder),
    ClearFilters,
    FocusSearch,
    DensityChanged(GridDensity),
    ViewModeChanged(ViewMode),
    LoadCollections,
    LoadCollectionItems(String),
    /// Set watch progress data: media_item_id -> (progress_fraction, watched).
    SetWatchData(HashMap<String, (f64, bool)>),
    /// Set the media ids with a completed offline download (downloaded badge).
    SetDownloadedIds(std::collections::HashSet<String>),
    /// Mark an item at grid position as watched.
    MarkWatchedAt(u32),
    /// Mark an item at grid position as unwatched.
    MarkUnwatchedAt(u32),
    /// Show watch-state context menu at grid coordinates.
    ContextMenuAt {
        x: f64,
        y: f64,
    },
}

impl std::fmt::Debug for LibraryViewMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadLibrary(section) => write!(f, "LoadLibrary({})", section.title),
            Self::SetSource(..) => write!(f, "SetSource(..)"),
            Self::SetSavedUiState(..) => write!(f, "SetSavedUiState(..)"),
            Self::ItemActivated(pos) => write!(f, "ItemActivated({pos})"),
            Self::LibraryLoaded(items) => write!(f, "LibraryLoaded({} items)", items.len()),
            Self::ShowCached(section, items) => {
                write!(f, "ShowCached({}, {} items)", section.title, items.len())
            }
            Self::ApplyRevalidated { cache_key, items } => {
                write!(f, "ApplyRevalidated({cache_key}, {} items)", items.len())
            }
            Self::LoadError(msg) => write!(f, "LoadError({msg})"),
            Self::SearchChanged(q) => write!(f, "SearchChanged({q:?})"),
            Self::FilterStateChanged(_) => write!(f, "FilterStateChanged(..)"),
            Self::RemoveActiveFilter(t) => write!(f, "RemoveActiveFilter({t:?})"),
            Self::SortChanged(s) => write!(f, "SortChanged({s:?})"),
            Self::ClearFilters => write!(f, "ClearFilters"),
            Self::FocusSearch => write!(f, "FocusSearch"),
            Self::DensityChanged(d) => write!(f, "DensityChanged({d:?})"),
            Self::ViewModeChanged(vm) => write!(f, "ViewModeChanged({vm:?})"),
            Self::LoadCollections => write!(f, "LoadCollections"),
            Self::LoadCollectionItems(key) => write!(f, "LoadCollectionItems({key})"),
            Self::SetWatchData(data) => write!(f, "SetWatchData({} items)", data.len()),
            Self::SetDownloadedIds(ids) => write!(f, "SetDownloadedIds({} ids)", ids.len()),
            Self::MarkWatchedAt(pos) => write!(f, "MarkWatchedAt({pos})"),
            Self::MarkUnwatchedAt(pos) => write!(f, "MarkUnwatchedAt({pos})"),
            Self::ContextMenuAt { x, y } => write!(f, "ContextMenuAt({x}, {y})"),
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
    /// Persist the filter/sort state for a library (App writes it to settings).
    SaveLibraryUiState {
        library_id: String,
        state: LibraryUiState,
    },
    /// The poster density changed; other grids (e.g. Downloads) follow it.
    DensityChanged(GridDensity),
    /// A network fetch for `section_key` completed; App stores the items in the
    /// session content cache. Carries the section key (not the composite) — App
    /// composes the full cache key from its browsed source.
    ItemsFetched {
        section_key: String,
        items: Vec<MediaItem>,
    },
}

pub enum LibraryViewCmd {
    Loaded(Vec<MediaItem>),
    Error(String),
    ArtworkReady(String, gtk::gdk::Texture),
}

impl std::fmt::Debug for LibraryViewCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LibraryViewCmd")
    }
}

/// What a `LoadLibrary` request should do given the currently-loaded library.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LoadDecision {
    /// The requested library is already loaded with items — just show it.
    ShowExisting,
    /// A fetch for the requested library is already in flight — ignore.
    SkipDuplicateFetch,
    /// Fetch the requested library.
    Fetch,
}

/// Decide how to handle a `LoadLibrary` request. Keyed on the library *key* (not
/// type) so switching between two same-type libraries (e.g. "Movies" and
/// "4K Movies") refetches rather than showing the first library's stale items.
pub(crate) fn load_decision(
    current_key: Option<&str>,
    requested_key: &str,
    has_items: bool,
    loading: bool,
) -> LoadDecision {
    let same = current_key == Some(requested_key);
    if same && has_items {
        LoadDecision::ShowExisting
    } else if same && loading {
        LoadDecision::SkipDuplicateFetch
    } else {
        LoadDecision::Fetch
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
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
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

        let stack = gtk::Stack::new();
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
        let clear_btn = gtk::Button::builder()
            .label("Clear Filters")
            .css_classes(["pill", "suggested-action"])
            .halign(gtk::Align::Center)
            .build();
        let sender_clear = sender.input_sender().clone();
        clear_btn.connect_clicked(move |_| {
            let _ = sender_clear.send(LibraryViewMsg::ClearFilters);
        });
        no_results_page.set_child(Some(&clear_btn));

        let grid: TypedGridView<MediaCardData, gtk::SingleSelection> = TypedGridView::new();
        let grid_view = grid.view.clone();
        grid_view.set_min_columns(3);
        grid_view.set_max_columns(10);
        grid_view.set_margin_start(20);
        grid_view.set_margin_end(20);
        grid_view.set_margin_top(16);
        grid_view.set_margin_bottom(16);
        grid_view.set_single_click_activate(true);

        let sender_activate = sender.input_sender().clone();
        grid_view.connect_activate(move |_view, position| {
            let _ = sender_activate.send(LibraryViewMsg::ItemActivated(position));
        });

        // Right-click context menu on grid items
        let right_click = gtk::GestureClick::builder()
            .button(3) // secondary button
            .build();
        let sender_ctx = sender.input_sender().clone();
        right_click.connect_released(move |gesture, _n_press, x, y| {
            let _ = sender_ctx.send(LibraryViewMsg::ContextMenuAt { x, y });
            gesture.set_state(gtk::EventSequenceState::Claimed);
        });
        grid_view.add_controller(right_click);

        let grid_page = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&grid_view)
            .build();

        // Search bar with entry
        let search_entry = gtk::SearchEntry::builder()
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

        let search_bar = gtk::SearchBar::builder()
            .search_mode_enabled(false)
            .show_close_button(true)
            .child(&search_entry)
            .build();
        search_bar.connect_entry(&search_entry);

        // Filter / sort / view bar (header row under the search bar).
        let filter_bar = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .margin_start(12)
            .margin_end(12)
            .margin_top(4)
            .margin_bottom(4)
            .css_classes(["library-filter-bar"])
            .build();

        // Sort dropdown (visible in the header).
        let sort_labels: Vec<&str> = SortOrder::all().iter().map(|s| s.label()).collect();
        let sort_dropdown = gtk::DropDown::from_strings(&sort_labels);
        sort_dropdown.set_selected(0);
        sort_dropdown.set_tooltip_text(Some("Sort order"));

        let sender_sort = sender.input_sender().clone();
        let sort_handler = sort_dropdown.connect_selected_notify(move |dd| {
            let all = SortOrder::all();
            let idx = dd.selected() as usize;
            if idx < all.len() {
                let _ = sender_sort.send(LibraryViewMsg::SortChanged(all[idx]));
            }
        });

        // Density dropdown (thumbnail size).
        let density_labels: Vec<&str> = GridDensity::all().iter().map(|d| d.label()).collect();
        let density_dropdown = gtk::DropDown::from_strings(&density_labels);
        density_dropdown.set_selected(1); // Medium (default)
        density_dropdown.set_tooltip_text(Some("Thumbnail size"));

        let sender_density = sender.input_sender().clone();
        density_dropdown.connect_selected_notify(move |dd| {
            let all = GridDensity::all();
            let idx = dd.selected() as usize;
            if idx < all.len() {
                let _ = sender_density.send(LibraryViewMsg::DensityChanged(all[idx]));
            }
        });

        // Adwaita filter popover with the seven filter groups.
        let filter_popover = FilterPopover::new(sender.input_sender().clone());

        let filter_button = gtk::MenuButton::builder()
            .icon_name("funnel-symbolic")
            .tooltip_text("Filter")
            .popover(&filter_popover.popover)
            .css_classes(["flat"])
            .build();

        // Active filter indicator dot.
        let filter_dot = gtk::Image::builder()
            .icon_name("media-record-symbolic")
            .css_classes(["accent"])
            .visible(false)
            .build();

        // Active filter count badge.
        let filter_count_badge = gtk::Label::builder()
            .css_classes(["filter-count-badge"])
            .visible(false)
            .build();

        let filter_btn_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(2)
            .build();
        filter_btn_box.append(&filter_button);
        filter_btn_box.append(&filter_count_badge);

        // Active-filter pill row (its own container, placed below the bar).
        let active_filters_bar = ActiveFiltersBar::new(sender.input_sender().clone());

        // --- View mode toggle (grid / list) ---
        let view_mode_toggle = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(0)
            .css_classes(["view-mode-toggle", "linked"])
            .build();

        let grid_btn = gtk::ToggleButton::builder()
            .icon_name("view-grid-symbolic")
            .tooltip_text("Grid view")
            .active(true)
            .build();

        let list_btn = gtk::ToggleButton::builder()
            .icon_name("view-list-symbolic")
            .tooltip_text("List view")
            .build();

        // Wire toggle behavior: exactly one active at a time
        let list_btn_weak = list_btn.clone();
        let sender_vm_g = sender.input_sender().clone();
        grid_btn.connect_toggled(move |btn| {
            if btn.is_active() {
                list_btn_weak.set_active(false);
                let _ = sender_vm_g.send(LibraryViewMsg::ViewModeChanged(ViewMode::Grid));
            }
        });

        let grid_btn_weak2 = grid_btn.clone();
        let sender_vm_l = sender.input_sender().clone();
        list_btn.connect_toggled(move |btn| {
            if btn.is_active() {
                grid_btn_weak2.set_active(false);
                let _ = sender_vm_l.send(LibraryViewMsg::ViewModeChanged(ViewMode::List));
            }
        });

        view_mode_toggle.append(&grid_btn);
        view_mode_toggle.append(&list_btn);

        let bar_spacer = gtk::Box::builder().hexpand(true).build();
        filter_bar.append(&sort_dropdown);
        filter_bar.append(&density_dropdown);
        filter_bar.append(&bar_spacer);
        filter_bar.append(&filter_dot);
        filter_bar.append(&filter_btn_box);
        filter_bar.append(&view_mode_toggle);

        stack.add_child(&loading_page);
        stack.add_child(&empty_page);
        stack.add_child(&error_page);
        stack.add_child(&no_results_page);
        stack.set_visible_child(&empty_page);

        // Continue Watching section (hidden initially)
        let continue_watching_section = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .margin_start(16)
            .margin_end(16)
            .margin_top(8)
            .margin_bottom(8)
            .css_classes(["library-section"])
            .visible(false)
            .build();

        let cw_label = gtk::Label::builder()
            .label("Continue Watching")
            .halign(gtk::Align::Start)
            .css_classes(["library-section-title"])
            .build();

        let continue_watching_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        let cw_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .max_content_height(240)
            .child(&continue_watching_box)
            .build();

        continue_watching_section.append(&cw_label);
        continue_watching_section.append(&cw_scroll);

        // --- Library hero header (icon + title + subtitle) ---
        let library_hero = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .css_classes(["library-hero"])
            .build();

        let hero_icon = gtk::Image::builder()
            .icon_name("folder-videos-symbolic")
            .pixel_size(36)
            .css_classes(["library-hero-icon"])
            .build();

        let hero_text = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .build();

        let library_title = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .css_classes(["library-hero-title"])
            .build();

        let library_hero_subtitle = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .css_classes(["library-hero-subtitle", "dim-label"])
            .build();

        hero_text.append(&library_title);
        hero_text.append(&library_hero_subtitle);
        library_hero.append(&hero_icon);
        library_hero.append(&hero_text);

        // --- Skeleton loading grid (shimmer placeholders) ---
        let skeleton_grid = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .visible(false)
            .build();

        let skeleton_scroll = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&skeleton_grid)
            .build();

        // --- Alphabet jump bar ---
        let alphabet_bar = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .valign(gtk::Align::Center)
            .halign(gtk::Align::End)
            .css_classes(["alphabet-bar"])
            .visible(false)
            .build();

        // Populate alphabet bar with A-Z letter buttons
        let sender_alpha = sender.input_sender().clone();
        for letter in 'A'..='Z' {
            let btn = gtk::Button::builder()
                .label(letter.to_string())
                .css_classes(["flat"])
                .build();
            let sender_l = sender_alpha.clone();
            btn.connect_clicked(move |_| {
                let _ = sender_l.send(LibraryViewMsg::SearchChanged(letter.to_string()));
            });
            alphabet_bar.append(&btn);
        }

        // Add "#" button for numeric/symbol titles
        let hash_btn = gtk::Button::builder()
            .label("#")
            .css_classes(["flat"])
            .build();
        let sender_hash = sender_alpha.clone();
        hash_btn.connect_clicked(move |_| {
            let _ = sender_hash.send(LibraryViewMsg::SearchChanged("#".to_string()));
        });
        alphabet_bar.append(&hash_btn);

        // Overlay alphabet bar on top of the scrolled grid page
        let grid_overlay = gtk::Overlay::new();
        grid_overlay.set_child(Some(&grid_page));
        grid_overlay.add_overlay(&alphabet_bar);

        // --- List view (compact alternative to grid) ---
        let list_box = gtk::ListBox::builder()
            .css_classes(["rich-list"])
            .selection_mode(gtk::SelectionMode::None)
            .build();

        let list_scroll = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&list_box)
            .build();

        stack.add_child(&grid_overlay);
        stack.add_child(&list_scroll);
        stack.add_child(&skeleton_scroll);

        root.append(&search_bar);
        root.append(&filter_bar);
        root.append(&active_filters_bar.container);
        root.append(&library_hero);
        root.append(&continue_watching_section);
        root.append(&stack);

        // Apply .rich-list for spacious grid style
        grid_view.add_css_class("rich-list");
        grid_view.add_css_class("library-grid");

        let model = Self {
            grid,
            library_type: LibraryType::Movie,
            library_key: None,
            source: None,
            artwork_cache: None,
            stack,
            loading_page,
            empty_page,
            error_page,
            no_results_page,
            grid_page,
            grid_overlay,
            search_bar,
            search_entry,
            filter_bar,
            filter_button,
            filter_dot,
            library_title,
            sort_dropdown,
            sort_handler,
            filter_popover,
            active_filters_bar,
            all_items: Vec::new(),
            search_query: String::new(),
            filter_state: FilterState::default(),
            sort_order: SortOrder::default(),
            grid_density: GridDensity::default(),
            saved_ui: LibrarySettings::default(),
            library_id: None,
            texture_cache: HashMap::new(),
            watch_data: HashMap::new(),
            local_watch_data: HashMap::new(),
            downloaded_ids: std::collections::HashSet::new(),
            continue_watching_section,
            continue_watching_box,
            poster_load_tracker: None,
            loading_in_progress: false,
            filter_count_badge,
            library_hero,
            library_hero_subtitle,
            view_mode: ViewMode::default(),
            skeleton_grid,
            skeleton_scroll,
            alphabet_bar,
            list_box,
            list_scroll,
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
            LibraryViewMsg::SetSavedUiState(saved) => {
                self.saved_ui = saved;
            }
            LibraryViewMsg::LoadLibrary(section) => {
                let current_key = self.library_key.as_deref();
                match load_decision(
                    current_key,
                    &section.key,
                    !self.all_items.is_empty(),
                    self.loading_in_progress,
                ) {
                    // Same library already loaded: show it, preserving filter/sort
                    // state when navigating back from a detail page.
                    LoadDecision::ShowExisting => {
                        self.stack.set_visible_child(&self.grid_overlay);
                        return;
                    }
                    // A fetch for this same library is already in flight.
                    LoadDecision::SkipDuplicateFetch => return,
                    LoadDecision::Fetch => {}
                }

                // Switching to a different library: reset filters (genres differ)
                // but keep sort.
                if current_key != Some(section.key.as_str()) {
                    self.filter_state.clear();
                    self.search_query.clear();
                    self.search_bar.set_search_mode(false);
                    self.search_entry.set_text("");
                }

                self.library_type = section.library_type;
                self.library_key = Some(section.key.clone());
                self.loading_in_progress = true;

                // Update hero header right away with the library's own name.
                self.library_title.set_label(&section.title);
                self.library_hero_subtitle.set_label("Loading...");

                // Build skeleton grid placeholders for a more polished loading state
                self.build_skeleton_grid();
                self.stack.set_visible_child(&self.skeleton_scroll);

                let Some(source) = self.source.clone() else {
                    self.loading_in_progress = false;
                    self.empty_page.set_title("No Media");
                    self.empty_page
                        .set_description(Some("Connect a Plex server to browse your library"));
                    self.stack.set_visible_child(&self.empty_page);
                    return;
                };

                let key = section.key.clone();
                sender.oneshot_command(async move {
                    let start = std::time::Instant::now();
                    match source.library_items(&key).await {
                        Ok(items) => {
                            info!(
                                "Library {key} load complete: {} items in {:?}",
                                items.len(),
                                start.elapsed()
                            );
                            LibraryViewCmd::Loaded(items)
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
                // Hand the freshly-fetched items to App for session caching, then
                // render them. The section key lets App compose the cache key.
                let section_key = self.library_key.clone().unwrap_or_default();
                let _ = sender.output(LibraryViewOutput::ItemsFetched {
                    section_key,
                    items: items.clone(),
                });
                self.apply_loaded_items(items, &sender);
            }
            LibraryViewMsg::ShowCached(section, items) => {
                // Instant render from the session content cache: no skeleton, no
                // fetch, no re-cache (App already holds these items).
                if self.library_key.as_deref() != Some(section.key.as_str()) {
                    self.filter_state.clear();
                    self.search_query.clear();
                    self.search_bar.set_search_mode(false);
                    self.search_entry.set_text("");
                }
                self.library_type = section.library_type;
                self.library_key = Some(section.key.clone());
                self.library_title.set_label(&section.title);
                self.apply_loaded_items(items, &sender);
            }
            LibraryViewMsg::ApplyRevalidated { cache_key, items } => {
                // Silent in-place update from background revalidation. Guard on the
                // full composite key (library_id) — not the bare section key — so a
                // result for another server's same-numbered section is rejected if
                // the user navigated there. Preserve current filter/sort/search;
                // only the items and grid refresh.
                if self.library_id.as_deref() != Some(cache_key.as_str()) {
                    return;
                }
                self.all_items = items;
                self.recompute_watch_data();
                self.filter_popover
                    .reset(&self.all_items, &self.filter_state);
                self.active_filters_bar.update(&self.filter_state);
                self.rebuild_grid(&sender);
            }
            LibraryViewMsg::LoadError(msg) => {
                self.loading_in_progress = false;
                self.error_page.set_description(Some(&msg));
                self.stack.set_visible_child(&self.error_page);
            }
            LibraryViewMsg::SearchChanged(query) => {
                self.search_query = query;
                self.rebuild_grid(&sender);
            }
            LibraryViewMsg::FilterStateChanged(state) => {
                // Emitted by the filter popover. The popover already reflects
                // this state, so don't echo it back — just refresh pills + grid.
                self.filter_state = state;
                self.active_filters_bar.update(&self.filter_state);
                self.update_clear_button_visibility();
                self.rebuild_grid(&sender);
                self.persist_ui_state(&sender);
            }
            LibraryViewMsg::RemoveActiveFilter(tag) => {
                // Emitted by a pill's ✘. Mutate state, then push it back into
                // the popover so its controls reflect the removal.
                self.filter_state.remove(&tag);
                self.filter_popover.set_state(&self.filter_state);
                self.active_filters_bar.update(&self.filter_state);
                self.update_clear_button_visibility();
                self.rebuild_grid(&sender);
                self.persist_ui_state(&sender);
            }
            LibraryViewMsg::SortChanged(order) => {
                self.sort_order = order;
                self.rebuild_grid(&sender);
                self.persist_ui_state(&sender);
            }
            LibraryViewMsg::DensityChanged(density) => {
                self.grid_density = density;
                // Update grid view column constraints
                self.grid.view.set_min_columns(density.min_columns());
                self.rebuild_grid(&sender);
                // Let other grids (Downloads) match the chosen density.
                let _ = sender.output(LibraryViewOutput::DensityChanged(density));
            }
            LibraryViewMsg::ViewModeChanged(view_mode) => {
                self.view_mode = view_mode;
                self.rebuild_grid(&sender);
            }
            LibraryViewMsg::ClearFilters => {
                self.filter_state.clear();
                self.search_query.clear();
                self.search_bar.set_search_mode(false);
                self.search_entry.set_text("");
                self.filter_popover.set_state(&self.filter_state);
                self.active_filters_bar.update(&self.filter_state);
                self.update_clear_button_visibility();
                self.rebuild_grid(&sender);
                self.persist_ui_state(&sender);
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
            LibraryViewMsg::ContextMenuAt { x, y } => {
                if let Some(position) = self.pick_grid_position_at(x, y) {
                    show_watch_context_menu(&self.grid.view, sender.input_sender(), position, x, y);
                }
            }
            LibraryViewMsg::SetWatchData(data) => {
                self.local_watch_data = data;
                self.recompute_watch_data();
                // Re-populate grid to reflect updated watch indicators
                if !self.all_items.is_empty() {
                    self.rebuild_grid(&sender);
                }
                // Rebuild the Continue Watching shelf (independent of filters).
                self.rebuild_continue_watching(&sender);
            }
            LibraryViewMsg::SetDownloadedIds(ids) => {
                // The app pushes this on every download state change, which during
                // a multi-episode download recurs many times a second. A full grid
                // rebuild per push (307 items, ~100-300ms each) jams the GTK loop,
                // so flip only the affected cards' badge in place instead.
                if ids != self.downloaded_ids {
                    self.downloaded_ids = ids;
                    let len = self.grid.len();
                    for i in 0..len {
                        if let Some(item) = self.grid.get(i) {
                            let mut card = item.borrow_mut();
                            let now = self.downloaded_ids.contains(&card.media_id);
                            if card.downloaded != now {
                                card.downloaded = now;
                                // Mutating the data model alone does not re-run
                                // bind(); update the badge widget directly.
                                if let Some(ref icon) = card.downloaded_widget {
                                    icon.set_visible(now);
                                }
                            }
                        }
                    }
                }
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
                            borrow.poster_texture = Some(texture.clone());

                            // Directly update the GTK Picture widget — mutating the
                            // data model alone does NOT trigger RelmGridItem::bind()
                            // to re-run, so we must set the paintable on the widget.
                            if let Some(ref picture) = borrow.picture_widget {
                                picture.set_paintable(Some(&texture));
                                picture.remove_css_class("loading");
                                picture.set_opacity(1.0);
                            }
                            if let Some(ref placeholder) = borrow.placeholder_widget {
                                placeholder.set_visible(false);
                            }

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
    /// Build a compact list view from the filtered indices.
    fn build_list_view(&mut self, filtered_indices: &[usize], sender: &ComponentSender<Self>) {
        // Clear existing rows
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        for &item_idx in filtered_indices {
            let item = &self.all_items[item_idx];
            let row = gtk::ListBoxRow::builder()
                .css_classes(["library-list-row"])
                .activatable(true)
                .build();

            let hbox = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(12)
                .margin_start(8)
                .margin_end(8)
                .margin_top(6)
                .margin_bottom(6)
                .build();

            // Small poster thumbnail
            let poster = gtk::Picture::builder()
                .content_fit(gtk::ContentFit::Cover)
                .width_request(48)
                .height_request(72)
                .css_classes(["library-list-poster"])
                .build();

            // Try to load poster from cache
            if let (Some(poster_path), Some(source)) = (&item.poster_path, &self.source) {
                let url = source.artwork_url(poster_path, 96, 144);
                if let Some(texture) = self.texture_cache.get(&url) {
                    poster.set_paintable(Some(texture));
                }
            }

            // Text column
            let text_col = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(2)
                .valign(gtk::Align::Center)
                .build();

            let title_text = match item.year {
                Some(year) => format!("{} ({})", item.title, year),
                None => item.title.clone(),
            };
            let title = gtk::Label::builder()
                .label(&title_text)
                .halign(gtk::Align::Start)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .css_classes(["library-list-title"])
                .build();

            let meta_parts: Vec<String> = [
                item.content_rating.clone(),
                item.video_resolution
                    .as_ref()
                    .map(|r| MediaItem::format_resolution(r).to_string()),
                item.format_runtime(),
            ]
            .into_iter()
            .flatten()
            .collect();

            let meta = gtk::Label::builder()
                .label(meta_parts.join(" · "))
                .halign(gtk::Align::Start)
                .css_classes(["library-list-meta", "dim-label"])
                .visible(!meta_parts.is_empty())
                .build();

            text_col.append(&title);
            text_col.append(&meta);

            hbox.append(&poster);
            hbox.append(&text_col);
            row.set_child(Some(&hbox));

            // Click handler → navigate to detail
            let item_clone = item.clone();
            let output_sender = sender.output_sender().clone();
            let gesture = gtk::GestureClick::new();
            gesture.connect_released(move |_, _, _, _| {
                let _ = output_sender.send(LibraryViewOutput::ShowDetail(item_clone.clone()));
            });
            row.add_controller(gesture);

            self.list_box.append(&row);
        }
    }

    /// Recompute the effective watch map, overlaying each source's own reported
    /// state (e.g. Plex view count/offset) on top of locally tracked progress.
    /// Call whenever `all_items` or `local_watch_data` changes.
    fn recompute_watch_data(&mut self) {
        self.watch_data =
            library_filter::build_effective_watch_map(&self.all_items, &self.local_watch_data);
    }

    /// Render a loaded set of items: store them, recompute watch state, restore
    /// persisted filter/sort, and rebuild the grid. Shared by a completed network
    /// fetch (`LibraryLoaded`) and an instant cache render (`ShowCached`).
    fn apply_loaded_items(&mut self, items: Vec<MediaItem>, sender: &ComponentSender<Self>) {
        self.loading_in_progress = false;
        if items.is_empty() {
            self.all_items.clear();
            self.grid.clear();
            // A connected, visible library that happens to have no items
            // — distinct from the "no source" empty state.
            self.empty_page.set_title("Empty Library");
            self.empty_page
                .set_description(Some("This library has no items yet."));
            self.stack.set_visible_child(&self.empty_page);
            return;
        }

        let build_start = std::time::Instant::now();
        self.all_items = items;
        self.recompute_watch_data();
        info!("Library loaded: {} items", self.all_items.len());

        // Compute the persistence key for this library and restore any saved
        // filter/sort state, reconciled against the loaded items so stale values
        // (e.g. a genre no longer present) are dropped. The key is section-based
        // so two same-type libraries don't share state; fall back to the legacy
        // library-type key so existing prefs seed the first load.
        let source_type = self.all_items[0].source_type.as_str();
        let source_id = self.all_items[0].source_id.clone();
        let section_key = self.library_key.clone().unwrap_or_default();
        let lib_id = format!("{source_type}:{source_id}:{section_key}");
        let saved = if self.saved_ui.per_library.contains_key(&lib_id) {
            self.saved_ui.get(&lib_id)
        } else {
            let legacy_id = format!("{source_type}:{source_id}:{}", self.library_type.as_str());
            self.saved_ui.get(&legacy_id)
        };
        self.library_id = Some(lib_id);
        self.filter_state = library_filter::reconcile(&saved.filters, &self.all_items);
        self.sort_order = saved.sort;
        if let Some(idx) = SortOrder::all().iter().position(|s| *s == self.sort_order) {
            // Block the change handler so restoring the selection doesn't
            // re-emit SortChanged and re-persist on load.
            self.sort_dropdown.block_signal(&self.sort_handler);
            self.sort_dropdown.set_selected(idx as u32);
            self.sort_dropdown.unblock_signal(&self.sort_handler);
        }

        self.filter_popover
            .reset(&self.all_items, &self.filter_state);
        self.active_filters_bar.update(&self.filter_state);
        self.update_clear_button_visibility();
        self.rebuild_grid(sender);
        info!("Full UI build: {:?}", build_start.elapsed());
    }

    /// Apply watch + downloaded indicators to a card from current state.
    fn apply_card_state(&self, card: &mut MediaCardData, id: &str) {
        if let Some(&(progress, watched)) = self.watch_data.get(id) {
            card.watch_progress = Some(progress);
            card.watched = watched;
        }
        card.downloaded = self.downloaded_ids.contains(id);
    }

    /// Rebuild the grid from all_items using current search/filter/sort state.
    fn rebuild_grid(&mut self, sender: &ComponentSender<Self>) {
        let start = std::time::Instant::now();
        self.grid.clear();

        let filtered_indices = library_filter::apply_filters_and_sort(
            &self.all_items,
            &self.search_query,
            &self.filter_state,
            self.sort_order,
            &self.watch_data,
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

            self.apply_card_state(&mut card, &item.id);

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
                            Ok(path) => match gtk::gdk::Texture::from_filename(&path) {
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

        // Show appropriate view based on mode
        match self.view_mode {
            ViewMode::Grid => {
                self.stack.set_visible_child(&self.grid_overlay);
            }
            ViewMode::List => {
                self.build_list_view(&filtered_indices, sender);
                self.stack.set_visible_child(&self.list_scroll);
            }
        }

        // Update library hero header
        let type_name = match self.library_type {
            LibraryType::Movie => "Movies",
            LibraryType::Show => "TV Shows",
        };
        let showing_count = filtered_indices.len();
        let total_count = self.all_items.len();
        self.library_title.set_label(type_name);
        if showing_count == total_count {
            self.library_hero_subtitle.set_label(&format!(
                "{total_count} {type_name_lower} · Browse your collection",
                type_name_lower = match self.library_type {
                    LibraryType::Movie => "movies",
                    LibraryType::Show => "shows",
                }
            ));
        } else {
            self.library_hero_subtitle.set_label(&format!(
                "{showing_count} of {total_count} {type_name_lower} · Filtered",
                type_name_lower = match self.library_type {
                    LibraryType::Movie => "movies",
                    LibraryType::Show => "shows",
                }
            ));
        }

        // Continue Watching shelf is independent of the active filters.
        self.rebuild_continue_watching(sender);

        // Show alphabet bar when sorting by title (most useful for letter jumping)
        let show_alpha = matches!(self.sort_order, SortOrder::TitleAsc | SortOrder::TitleDesc)
            && filtered_indices.len() > 20;
        self.alphabet_bar.set_visible(show_alpha);
    }

    /// Build a skeleton grid of shimmer placeholder cards for the loading state.
    fn build_skeleton_grid(&mut self) {
        // Clear existing placeholders
        while let Some(child) = self.skeleton_grid.first_child() {
            self.skeleton_grid.remove(&child);
        }

        let card_w = self.grid_density.card_width();
        let card_h = self.grid_density.card_height();

        // Build a grid-like layout of skeleton cards
        let flow = gtk::FlowBox::builder()
            .hexpand(true)
            .margin_start(20)
            .margin_end(20)
            .margin_top(16)
            .margin_bottom(16)
            .selection_mode(gtk::SelectionMode::None)
            .row_spacing(12)
            .column_spacing(0)
            .build();

        let placeholder_count = self.grid_density.min_columns() as i32 * 3;
        for i in 0..placeholder_count {
            let card = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(6)
                .width_request(card_w)
                .margin_start(6)
                .margin_end(10)
                .build();

            let poster = gtk::Box::builder()
                .width_request(card_w)
                .height_request(card_h)
                .css_classes(["skeleton-card"])
                .build();

            let title_line = gtk::Box::builder()
                .width_request((card_w as f64 * 0.85) as i32)
                .height_request(12)
                .css_classes(["skeleton-card"])
                .halign(gtk::Align::Start)
                .margin_start(3)
                .build();

            // Stagger animation delay
            let delay_ms = (i % 6) as u32 * 80;
            let poster_clone = poster.clone();
            let title_clone = title_line.clone();
            gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(delay_ms as u64),
                move || {
                    poster_clone.set_opacity(1.0);
                    title_clone.set_opacity(1.0);
                },
            );

            // Start invisible, fade in with stagger
            poster.set_opacity(0.0);
            title_line.set_opacity(0.0);

            card.append(&poster);
            card.append(&title_line);
            flow.append(&card);
        }

        self.skeleton_grid.append(&flow);
    }

    /// Emit a save request for the current library's filter/sort state. No-op
    /// until a library has loaded (and thus `library_id` is known).
    fn persist_ui_state(&self, sender: &ComponentSender<Self>) {
        if let Some(ref id) = self.library_id {
            let _ = sender.output(LibraryViewOutput::SaveLibraryUiState {
                library_id: id.clone(),
                state: LibraryUiState {
                    filters: self.filter_state.clone(),
                    sort: self.sort_order,
                },
            });
        }
    }

    /// Update the filter-dot indicator and the filter-count badge on the
    /// filter button. The count is the number of active filter values (one per
    /// pill), so it matches what the pill row shows.
    fn update_clear_button_visibility(&self) {
        let has_active = self.filter_state.is_active() || !self.search_query.is_empty();
        self.filter_dot.set_visible(has_active);

        let count = library_filter::pill_labels(&self.filter_state).len() as u32;
        if count > 0 {
            self.filter_count_badge.set_label(&count.to_string());
            self.filter_count_badge.set_visible(true);
        } else {
            self.filter_count_badge.set_visible(false);
        }
    }

    /// Resolve a grid item index from click coordinates via widget hit-testing.
    fn pick_grid_position_at(&self, x: f64, y: f64) -> Option<u32> {
        let grid_view = &self.grid.view;
        let mut widget = grid_view.pick(x, y, gtk::PickFlags::DEFAULT)?;
        loop {
            if widget.has_css_class("media-card") {
                let media_id = widget.widget_name();
                if media_id.is_empty() {
                    return None;
                }
                for i in 0..self.grid.len() {
                    if let Some(item) = self.grid.get(i)
                        && item.borrow().media_id == media_id
                    {
                        return Some(i);
                    }
                }
                return None;
            }
            widget = widget.parent()?;
        }
    }

    /// Build a shelf card widget (poster + optional progress bar + title).
    /// Used by Continue Watching and Recently Added shelves.
    fn build_shelf_card(
        item: &MediaItem,
        progress: Option<f64>,
        source: &Option<Arc<dyn MediaSource>>,
        artwork_cache: &Option<Arc<ArtworkCache>>,
        texture_cache: &HashMap<String, gtk::gdk::Texture>,
        cmd_sender: &relm4::Sender<LibraryViewCmd>,
        output_sender: &relm4::Sender<LibraryViewOutput>,
    ) -> gtk::Box {
        let card = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .width_request(140)
            .css_classes(["media-card"])
            .build();

        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .width_request(140)
            .height_request(210)
            .css_classes(["media-card-poster"])
            .build();

        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&picture));

        let scrim = gtk::Box::builder()
            .halign(gtk::Align::Fill)
            .valign(gtk::Align::End)
            .height_request(48)
            .css_classes(["media-card-scrim"])
            .build();
        overlay.add_overlay(&scrim);

        // Progress bar (only for continue watching)
        if let Some(pct) = progress
            && pct > 0.0
        {
            let progress_bar = gtk::ProgressBar::builder()
                .halign(gtk::Align::Fill)
                .valign(gtk::Align::End)
                .fraction(pct)
                .css_classes(["watch-progress"])
                .build();
            overlay.add_overlay(&progress_bar);
        }

        let frame = gtk::Frame::builder()
            .css_classes(["media-card-frame"])
            .child(&overlay)
            .build();

        let title_text = match item.year {
            Some(year) => format!("{} · {year}", item.title),
            None => item.title.clone(),
        };
        let title = gtk::Label::builder()
            .label(&title_text)
            .halign(gtk::Align::Start)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .max_width_chars(16)
            .css_classes(["media-card-title"])
            .build();

        card.append(&frame);
        card.append(&title);

        // Load poster texture
        if let (Some(poster_path), Some(src)) = (&item.poster_path, source) {
            let url = src.artwork_url(poster_path, 300, 450);
            if let Some(texture) = texture_cache.get(&url) {
                picture.set_paintable(Some(texture));
            } else if let Some(cache) = artwork_cache {
                let cache = Arc::clone(cache);
                let sender = cmd_sender.clone();
                let fetch_url = url;
                gtk::glib::spawn_future_local(async move {
                    if let Ok(path) = cache.get_or_download(&fetch_url).await
                        && let Ok(texture) = gtk::gdk::Texture::from_filename(&path)
                    {
                        let _ = sender.send(LibraryViewCmd::ArtworkReady(fetch_url, texture));
                    }
                });
            }
        }

        // Click handler → navigate to detail
        let item_clone = item.clone();
        let os = output_sender.clone();
        let gesture = gtk::GestureClick::new();
        gesture.connect_released(move |_, _, _, _| {
            let _ = os.send(LibraryViewOutput::ShowDetail(item_clone.clone()));
        });
        card.add_controller(gesture);

        card
    }

    /// Rebuild the Continue Watching horizontal row from watch_data + all_items.
    fn rebuild_continue_watching(&mut self, sender: &ComponentSender<Self>) {
        while let Some(child) = self.continue_watching_box.first_child() {
            self.continue_watching_box.remove(&child);
        }

        let in_progress: Vec<&MediaItem> = self
            .all_items
            .iter()
            .filter(|item| {
                self.watch_data
                    .get(&item.id)
                    .map(|&(_, watched)| !watched)
                    .unwrap_or(false)
            })
            .take(20)
            .collect();

        if in_progress.is_empty() {
            self.continue_watching_section.set_visible(false);
            return;
        }

        self.continue_watching_section.set_visible(true);

        let source = self.source.clone();
        let artwork_cache = self.artwork_cache.clone();
        let cmd_sender = sender.command_sender().clone();
        let output_sender = sender.output_sender().clone();

        for item in &in_progress {
            let progress = self
                .watch_data
                .get(&item.id)
                .and_then(|&(p, watched)| if !watched { Some(p) } else { None });

            let card = Self::build_shelf_card(
                item,
                progress,
                &source,
                &artwork_cache,
                &self.texture_cache,
                &cmd_sender,
                &output_sender,
            );
            self.continue_watching_box.append(&card);
        }
    }
}

/// Show a context menu popover at the given position with Watch/Unwatch actions.
fn show_watch_context_menu(
    grid_view: &gtk::GridView,
    sender: &relm4::Sender<LibraryViewMsg>,
    position: u32,
    x: f64,
    y: f64,
) {
    let menu = gtk::gio::Menu::new();
    menu.append(Some("Mark as Watched"), Some("watch.mark-watched"));
    menu.append(Some("Mark as Unwatched"), Some("watch.mark-unwatched"));

    let popover = gtk::PopoverMenu::from_model(Some(&menu));
    popover.set_parent(grid_view);
    popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    popover.set_has_arrow(true);

    // Action group for the popover
    let action_group = gtk::gio::SimpleActionGroup::new();

    let sender_watched = sender.clone();
    let watched_action = gtk::gio::SimpleAction::new("mark-watched", None);
    watched_action.connect_activate(move |_, _| {
        let _ = sender_watched.send(LibraryViewMsg::MarkWatchedAt(position));
    });
    action_group.add_action(&watched_action);

    let sender_unwatched = sender.clone();
    let unwatched_action = gtk::gio::SimpleAction::new("mark-unwatched", None);
    unwatched_action.connect_activate(move |_, _| {
        let _ = sender_unwatched.send(LibraryViewMsg::MarkUnwatchedAt(position));
    });
    action_group.add_action(&unwatched_action);

    grid_view.insert_action_group("watch", Some(&action_group));
    popover.popup();
}

#[cfg(test)]
mod tests {
    use super::{LoadDecision, load_decision};

    #[test]
    fn first_load_fetches() {
        assert_eq!(load_decision(None, "1", false, false), LoadDecision::Fetch);
    }

    #[test]
    fn same_library_with_items_shows_existing() {
        assert_eq!(
            load_decision(Some("1"), "1", true, false),
            LoadDecision::ShowExisting
        );
    }

    #[test]
    fn switching_to_same_type_other_library_refetches() {
        // "Movies" (key 1) loaded; navigating to "4K Movies" (key 2) must refetch
        // even though both are movie-type — the old type-keyed cache would not.
        assert_eq!(
            load_decision(Some("1"), "2", true, false),
            LoadDecision::Fetch
        );
    }

    #[test]
    fn duplicate_fetch_for_same_library_is_skipped() {
        assert_eq!(
            load_decision(Some("1"), "1", false, true),
            LoadDecision::SkipDuplicateFetch
        );
    }
}
