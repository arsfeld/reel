use std::collections::HashSet;
use std::sync::Arc;

use adw;
use gtk::prelude::*;
use relm4::prelude::*;

use crate::models::hub::MediaHub;
use crate::models::media::MediaItem;
use crate::models::watch::WatchProgress;
use crate::services::artwork::ArtworkCache;
use crate::services::media_source::MediaSource;

mod hero;
mod shelf;

use hero::{Hero, hero_candidates, next_index, prev_index};
use shelf::{Generation, HomeCard, Shelf, ShelfId, hub_duplicates_core, poster_result_is_current};

pub struct HomeView {
    source: Option<Arc<dyn MediaSource>>,
    artwork_cache: Option<Arc<ArtworkCache>>,
    /// The vertical container holding all shelf sections.
    shelves_box: gtk::Box,
    /// All shelves currently rendered, in display order.
    shelves: Vec<Shelf>,
    /// Monotonic id handed to the next shelf created.
    next_shelf_id: ShelfId,
    /// Incremented on every clear so stale in-flight poster loads are dropped.
    build_generation: Generation,
    /// The featured hero region (persists across shelf rebuilds).
    hero: Hero,
    /// Current hero rotation candidates and the index being shown.
    hero_items: Vec<MediaItem>,
    hero_index: usize,
    /// Guards async backdrop loads so rotation can't paint a stale backdrop.
    hero_token: u64,
    /// The scrolled shelves view (the populated home).
    scroll: gtk::ScrolledWindow,
    /// Empty state page (shown when no source / no content).
    empty_page: adw::StatusPage,
    /// Loading page (shown while a load is in flight with nothing to show yet).
    loading_page: adw::StatusPage,
    /// Error page (shown when a load fails and there is nothing else to show).
    error_page: adw::StatusPage,
    /// Connecting page (shown while validating a saved source on startup).
    connecting_page: adw::StatusPage,
    /// Stack switches between shelves and the status pages.
    stack: gtk::Stack,
    /// Last load error, used to decide between the empty and error pages.
    last_error: Option<String>,
    /// Prevent concurrent loads.
    loading: bool,
    /// Hidden-library visibility keys (`source_type:source_id:section_key`).
    /// Items belonging to a hidden library are dropped from Continue Watching
    /// and Recently Added when home data is built.
    hidden: HashSet<String>,
}

#[allow(clippy::large_enum_variant)]
pub enum HomeViewMsg {
    SetSource(Arc<dyn MediaSource>, Arc<ArtworkCache>),
    /// Show/hide the "Connecting to Plex…" loading page.
    SetConnecting(bool),
    /// Update the hidden-library set. Applied to subsequently loaded home data.
    SetVisibility(HashSet<String>),
    /// Load home page data: watch_progress items from local DB + recently_added from source.
    LoadHome {
        in_progress: Vec<(MediaItem, WatchProgress)>,
    },
    LoadError(String),
    /// A card was activated. `resume` is set for Continue Watching cards, which
    /// resume playback rather than opening a detail page.
    CardActivated {
        item: MediaItem,
        resume: bool,
    },
    /// Hero rotation (auto via timer, or manual via arrow keys).
    HeroAdvance,
    HeroBack,
    /// Hero Play / info act on the currently shown hero item.
    HeroPlay,
    HeroInfo,
}

impl std::fmt::Debug for HomeViewMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SetSource(..) => write!(f, "SetSource(..)"),
            Self::SetConnecting(v) => write!(f, "SetConnecting({v})"),
            Self::SetVisibility(h) => write!(f, "SetVisibility({} hidden)", h.len()),
            Self::LoadHome { in_progress } => {
                write!(f, "LoadHome({} in progress)", in_progress.len())
            }
            Self::LoadError(msg) => write!(f, "LoadError({msg})"),
            Self::CardActivated { item, resume } => {
                write!(f, "CardActivated({}, resume={resume})", item.title)
            }
            Self::HeroAdvance => write!(f, "HeroAdvance"),
            Self::HeroBack => write!(f, "HeroBack"),
            Self::HeroPlay => write!(f, "HeroPlay"),
            Self::HeroInfo => write!(f, "HeroInfo"),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum HomeViewOutput {
    ShowDetail(MediaItem),
    PlayMedia { url: String, media_item: MediaItem },
    ShowConnectionDialog,
    Error(String),
}

#[derive(Debug)]
pub enum HomeViewCmd {
    HomeData {
        continue_watching: Vec<MediaItem>,
        /// (library title, items) for each non-empty library, in library order.
        recently_added: Vec<(String, Vec<MediaItem>)>,
        collections: Vec<MediaItem>,
        /// Server-curated hubs (Recommended / Because-you-watched / genres),
        /// in server order. Empty for non-Plex sources.
        hubs: Vec<MediaHub>,
    },
    PosterLoaded {
        generation: Generation,
        shelf_id: ShelfId,
        index: usize,
        texture: gtk::gdk::Texture,
    },
    HeroBackdropLoaded {
        token: u64,
        texture: gtk::gdk::Texture,
    },
    Error(String),
}

#[relm4::component(pub)]
impl Component for HomeView {
    type Init = ();
    type Input = HomeViewMsg;
    type Output = HomeViewOutput;
    type CommandOutput = HomeViewCmd;

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

        // Empty state (no source configured / nothing to show)
        let empty_page = adw::StatusPage::builder()
            .title("Welcome to Reel")
            .description("Connect a Plex server to see your Continue Watching and Recently Added")
            .icon_name("folder-videos-symbolic")
            .build();

        let connect_btn = gtk::Button::builder()
            .label("Connect to Plex")
            .halign(gtk::Align::Center)
            .css_classes(["pill", "suggested-action"])
            .build();
        let sender_btn = sender.output_sender().clone();
        connect_btn.connect_clicked(move |_| {
            let _ = sender_btn.send(HomeViewOutput::ShowConnectionDialog);
        });
        empty_page.set_child(Some(&connect_btn));

        // Loading state (a load is in flight and nothing is rendered yet)
        let loading_spinner = gtk::Spinner::builder()
            .spinning(true)
            .halign(gtk::Align::Center)
            .width_request(32)
            .height_request(32)
            .build();
        let loading_page = adw::StatusPage::builder()
            .title("Loading your library…")
            .icon_name("folder-videos-symbolic")
            .child(&loading_spinner)
            .build();

        // Error state (load failed, nothing else to show)
        let error_page = adw::StatusPage::builder()
            .title("Couldn't load your home")
            .icon_name("dialog-error-symbolic")
            .build();

        // Connecting page (validating saved source on startup)
        let connecting_spinner = gtk::Spinner::builder()
            .spinning(true)
            .halign(gtk::Align::Center)
            .width_request(32)
            .height_request(32)
            .build();
        let connecting_page = adw::StatusPage::builder()
            .title("Connecting to Plex…")
            .description("Checking your saved server connection")
            .icon_name("network-server-symbolic")
            .child(&connecting_spinner)
            .build();

        // Shelves container, with the hero pinned above the shelves so it
        // scrolls together with the content.
        let scroll = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();

        let hero = Hero::new(sender.input_sender());

        let shelves_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .margin_start(24)
            .margin_end(24)
            .margin_top(16)
            .margin_bottom(16)
            .build();

        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .build();
        content_box.append(&hero.root);
        content_box.append(&shelves_box);
        scroll.set_child(Some(&content_box));

        // Auto-advance the hero rotation every 8s. The timer lives for the
        // component's lifetime; HeroAdvance is a no-op without candidates.
        let rotate_sender = sender.input_sender().clone();
        gtk::glib::timeout_add_local(std::time::Duration::from_secs(8), move || {
            let _ = rotate_sender.send(HomeViewMsg::HeroAdvance);
            gtk::glib::ControlFlow::Continue
        });

        stack.add_child(&empty_page);
        stack.add_child(&connecting_page);
        stack.add_child(&loading_page);
        stack.add_child(&error_page);
        stack.add_child(&scroll);
        stack.set_visible_child(&empty_page);

        root.append(&stack);

        let model = Self {
            source: None,
            artwork_cache: None,
            shelves_box,
            shelves: Vec::new(),
            next_shelf_id: 0,
            build_generation: 0,
            hero,
            hero_items: Vec::new(),
            hero_index: 0,
            hero_token: 0,
            scroll,
            empty_page,
            loading_page,
            error_page,
            connecting_page,
            stack,
            last_error: None,
            loading: false,
            hidden: HashSet::new(),
        };

        ComponentParts { model, widgets }
    }

    #[allow(clippy::too_many_lines)]
    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            HomeViewMsg::SetSource(source, artwork_cache) => {
                self.source = Some(source);
                self.artwork_cache = Some(artwork_cache);
            }
            HomeViewMsg::SetConnecting(connecting) => {
                if connecting {
                    self.stack.set_visible_child(&self.connecting_page);
                } else {
                    self.refresh_visible_page();
                }
            }
            HomeViewMsg::SetVisibility(hidden) => {
                self.hidden = hidden;
            }
            HomeViewMsg::LoadHome { in_progress } => {
                if self.loading {
                    return;
                }
                self.loading = true;
                self.last_error = None;
                self.clear_shelves();

                let is_plex = self
                    .source
                    .as_ref()
                    .map(|s| s.source_type() == crate::models::media::SourceType::Plex)
                    .unwrap_or(false);

                // Non-Plex sources have no On Deck; build Continue Watching from
                // the local DB progress the app pushes in.
                if !is_plex && !in_progress.is_empty() {
                    let cw_id = self.add_shelf("Continue Watching");
                    let cards: Vec<(MediaItem, Option<f64>)> = in_progress
                        .iter()
                        .map(|(item, progress)| (item.clone(), Some(progress.progress_fraction())))
                        .collect();
                    self.populate_shelf(cw_id, cards, true, &sender);
                }

                // Fetch On Deck (Plex) + Recently Added from the source (async).
                if let Some(ref source) = self.source {
                    self.refresh_visible_page();
                    let src = source.clone();
                    sender.oneshot_command(async move {
                        let continue_watching = if is_plex {
                            src.continue_watching().await.unwrap_or_default()
                        } else {
                            Vec::new()
                        };

                        let libs = match src.libraries().await {
                            Ok(l) => l,
                            Err(e) => return HomeViewCmd::Error(e.to_string()),
                        };

                        // Per-library Recently Added, fetched concurrently.
                        let ra_futures = libs.iter().map(|lib| {
                            let src = src.clone();
                            let key = lib.key.clone();
                            let title = lib.title.clone();
                            async move {
                                let mut items = src
                                    .recently_added_in_library(&key)
                                    .await
                                    .unwrap_or_default();
                                items.truncate(20);
                                (title, items)
                            }
                        });
                        let recently_added: Vec<(String, Vec<MediaItem>)> =
                            futures::future::join_all(ra_futures)
                                .await
                                .into_iter()
                                .filter(|(_, items)| !items.is_empty())
                                .collect();

                        // Collections across all libraries.
                        let col_futures = libs.iter().map(|lib| {
                            let src = src.clone();
                            let key = lib.key.clone();
                            async move { src.collections(&key).await.unwrap_or_default() }
                        });
                        let collections: Vec<MediaItem> = futures::future::join_all(col_futures)
                            .await
                            .into_iter()
                            .flatten()
                            .collect();

                        // Server-curated hubs. NotSupported (non-Plex) yields
                        // an empty list, so those sources simply show no hub
                        // rows.
                        let hubs = src.hubs().await.unwrap_or_default();

                        HomeViewCmd::HomeData {
                            continue_watching,
                            recently_added,
                            collections,
                            hubs,
                        }
                    });
                } else {
                    self.loading = false;
                    self.refresh_visible_page();
                }
            }
            HomeViewMsg::LoadError(msg) => {
                self.loading = false;
                tracing::warn!("HomeView load error: {msg}");
                self.error_page.set_description(Some(&msg));
                self.last_error = Some(msg);
                self.refresh_visible_page();
            }
            HomeViewMsg::CardActivated { item, resume } => {
                let should_play = item.file_path.is_some()
                    && (resume || item.media_type == crate::models::media::MediaType::Episode);
                if should_play
                    && let (Some(source), Some(part)) =
                        (self.source.as_ref(), item.file_path.as_ref())
                {
                    let url = source.playback_url(part);
                    let _ = sender.output(HomeViewOutput::PlayMedia {
                        url,
                        media_item: item,
                    });
                    return;
                }
                let _ = sender.output(HomeViewOutput::ShowDetail(item));
            }
            HomeViewMsg::HeroAdvance => {
                if !self.hero_items.is_empty() {
                    let idx = next_index(self.hero_index, self.hero_items.len());
                    self.show_hero_item(idx, &sender);
                }
            }
            HomeViewMsg::HeroBack => {
                if !self.hero_items.is_empty() {
                    let idx = prev_index(self.hero_index, self.hero_items.len());
                    self.show_hero_item(idx, &sender);
                }
            }
            HomeViewMsg::HeroPlay => {
                if let Some(item) = self.hero_items.get(self.hero_index).cloned() {
                    if let (Some(source), Some(part)) =
                        (self.source.as_ref(), item.file_path.as_ref())
                    {
                        let url = source.playback_url(part);
                        let _ = sender.output(HomeViewOutput::PlayMedia {
                            url,
                            media_item: item,
                        });
                    } else {
                        // Shows carry no directly-playable file; open detail.
                        let _ = sender.output(HomeViewOutput::ShowDetail(item));
                    }
                }
            }
            HomeViewMsg::HeroInfo => {
                if let Some(item) = self.hero_items.get(self.hero_index).cloned() {
                    let _ = sender.output(HomeViewOutput::ShowDetail(item));
                }
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
            HomeViewCmd::HomeData {
                continue_watching,
                recently_added,
                collections,
                hubs,
            } => {
                self.loading = false;

                // Drop content from hidden libraries before anything consumes
                // the lists. Filtering here (at receive time, against the
                // current hidden set) rather than at fetch time means a library
                // hidden while a fetch was in flight is still excluded.
                let continue_watching = crate::services::visibility::retain_visible_items(
                    continue_watching,
                    &self.hidden,
                );
                let recently_added: Vec<(String, Vec<MediaItem>)> = recently_added
                    .into_iter()
                    .filter_map(|(title, items)| {
                        let items =
                            crate::services::visibility::retain_visible_items(items, &self.hidden);
                        (!items.is_empty()).then_some((title, items))
                    })
                    .collect();

                // Featured hero: drawn from recently-added items that have a
                // backdrop. Computed before the shelves consume the lists.
                let hero_pool: Vec<MediaItem> = recently_added
                    .iter()
                    .flat_map(|(_, items)| items.iter().cloned())
                    .collect();
                self.set_hero(hero_candidates(&hero_pool, 6), &sender);

                if !continue_watching.is_empty() {
                    let cw_id = self.add_shelf("Continue Watching");
                    let cards: Vec<(MediaItem, Option<f64>)> = continue_watching
                        .into_iter()
                        .map(|item| {
                            let frac = item.resume_fraction();
                            (item, frac)
                        })
                        .collect();
                    self.populate_shelf(cw_id, cards, true, &sender);
                }

                // One Recently Added shelf per library (already filtered to
                // non-empty libraries, in library order).
                for (library_title, items) in recently_added {
                    let ra_id = self.add_shelf(&format!("Recently Added — {library_title}"));
                    let cards: Vec<(MediaItem, Option<f64>)> =
                        items.into_iter().map(|item| (item, None)).collect();
                    self.populate_shelf(ra_id, cards, false, &sender);
                }

                if !collections.is_empty() {
                    let col_id = self.add_shelf("Collections");
                    let cards: Vec<(MediaItem, Option<f64>)> =
                        collections.into_iter().map(|item| (item, None)).collect();
                    self.populate_shelf(col_id, cards, false, &sender);
                }

                // Append server-curated hubs below the Reel-owned core, dropping
                // any that duplicate Continue Watching / Recently Added.
                for hub in hubs {
                    if hub.items.is_empty() || hub_duplicates_core(hub.identifier.as_deref()) {
                        continue;
                    }
                    let hub_id = self.add_shelf(&hub.title);
                    let cards: Vec<(MediaItem, Option<f64>)> =
                        hub.items.into_iter().map(|item| (item, None)).collect();
                    self.populate_shelf(hub_id, cards, false, &sender);
                }

                self.refresh_visible_page();
            }
            HomeViewCmd::Error(msg) => {
                sender.input(HomeViewMsg::LoadError(msg));
            }
            HomeViewCmd::PosterLoaded {
                generation,
                shelf_id,
                index,
                texture,
            } => {
                if !poster_result_is_current(generation, self.build_generation) {
                    return;
                }
                if let Some(shelf) = self.shelves.iter().find(|s| s.id == shelf_id)
                    && let Some((card, _)) = shelf.cards.get(index)
                {
                    card.set_poster(&texture);
                }
            }
            HomeViewCmd::HeroBackdropLoaded { token, texture } => {
                if token == self.hero_token {
                    self.hero.set_backdrop(&texture);
                }
            }
        }
    }
}

impl HomeView {
    /// Drop all shelves and bump the build generation so any in-flight poster
    /// loads from the previous build are ignored when they arrive.
    fn clear_shelves(&mut self) {
        self.build_generation += 1;
        while let Some(child) = self.shelves_box.first_child() {
            self.shelves_box.remove(&child);
        }
        self.shelves.clear();
    }

    /// Create an empty titled shelf, append it to the shelves box, and return
    /// its id for population.
    fn add_shelf(&mut self, title: &str) -> ShelfId {
        let id = self.next_shelf_id;
        self.next_shelf_id += 1;
        let shelf = Shelf::new(id, title);
        self.shelves_box.append(&shelf.section);
        self.shelves.push(shelf);
        id
    }

    /// Fill a shelf with cards, wire activation, reveal it, and kick off poster
    /// downloads. Each card carries an optional progress fraction.
    fn populate_shelf(
        &mut self,
        shelf_id: ShelfId,
        cards: Vec<(MediaItem, Option<f64>)>,
        resume_on_click: bool,
        sender: &ComponentSender<Self>,
    ) {
        if cards.is_empty() {
            return;
        }
        if let Some(shelf) = self.shelves.iter_mut().find(|s| s.id == shelf_id) {
            for (item, progress) in cards {
                let card = HomeCard::new();
                card.set_media(&item, progress);

                let sender_card = sender.input_sender().clone();
                let item_click = item.clone();
                card.gesture.connect_released(move |_, _, _, _| {
                    let _ = sender_card.send(HomeViewMsg::CardActivated {
                        item: item_click.clone(),
                        resume: resume_on_click,
                    });
                });

                shelf.row.append(&card.container);
                shelf.cards.push((card, item));
            }
            shelf.section.set_visible(true);
        }

        self.fetch_posters(shelf_id, sender);
    }

    fn fetch_posters(&self, shelf_id: ShelfId, sender: &ComponentSender<Self>) {
        let Some(ref cache) = self.artwork_cache else {
            return;
        };
        let Some(ref source) = self.source else {
            return;
        };
        let Some(shelf) = self.shelves.iter().find(|s| s.id == shelf_id) else {
            return;
        };
        let generation = self.build_generation;

        for (idx, (_, item)) in shelf.cards.iter().enumerate() {
            // Episodes prefer their series poster so shelves show a portrait
            // show poster rather than the episode's landscape still.
            let Some(poster_path) = item.shelf_poster_path() else {
                continue;
            };
            let artwork_url = source.artwork_url(poster_path, 320, 480);
            let cache_clone = cache.clone();
            let sender_clone = sender.command_sender().clone();

            gtk::glib::spawn_future_local(async move {
                match cache_clone.get_or_download(&artwork_url).await {
                    Ok(path) => {
                        if let Ok(texture) = gtk::gdk::Texture::from_filename(&path) {
                            let _ = sender_clone.send(HomeViewCmd::PosterLoaded {
                                generation,
                                shelf_id,
                                index: idx,
                                texture,
                            });
                        }
                    }
                    Err(e) => {
                        tracing::debug!("Poster load failed for {}: {e}", artwork_url);
                    }
                }
            });
        }
    }

    /// Replace the hero rotation set and show the first item (or hide the hero
    /// when there are no backdrop-bearing candidates).
    fn set_hero(&mut self, items: Vec<MediaItem>, sender: &ComponentSender<Self>) {
        self.hero_items = items;
        self.hero_index = 0;
        if self.hero_items.is_empty() {
            self.hero.set_revealed(false);
        } else {
            self.hero.set_revealed(true);
            self.show_hero_item(0, sender);
        }
    }

    /// Show the hero item at `index`: update its labels/dots and kick off the
    /// backdrop download, guarded by a token so rotation can't be overtaken by
    /// a slow load of a previously-shown item.
    fn show_hero_item(&mut self, index: usize, sender: &ComponentSender<Self>) {
        let Some(item) = self.hero_items.get(index).cloned() else {
            return;
        };
        self.hero_index = index;
        let total = self.hero_items.len();
        self.hero.set_item(&item, index, total);

        self.hero_token += 1;
        let token = self.hero_token;
        let (Some(cache), Some(source), Some(backdrop_path)) = (
            self.artwork_cache.as_ref(),
            self.source.as_ref(),
            item.backdrop_path.as_ref(),
        ) else {
            return;
        };
        let url = source.artwork_url(backdrop_path, 1280, 420);
        let cache = cache.clone();
        let cmd_sender = sender.command_sender().clone();
        gtk::glib::spawn_future_local(async move {
            if let Ok(path) = cache.get_or_download(&url).await
                && let Ok(texture) = gtk::gdk::Texture::from_filename(&path)
            {
                let _ = cmd_sender.send(HomeViewCmd::HeroBackdropLoaded { token, texture });
            }
        });
    }

    fn has_any_cards(&self) -> bool {
        self.shelves.iter().any(|s| !s.cards.is_empty())
    }

    /// Pick the stack page that matches current state: shelves when there is
    /// content, otherwise the loading / error / empty page in that priority.
    fn refresh_visible_page(&self) {
        if self.has_any_cards() {
            self.stack.set_visible_child(&self.scroll);
        } else if self.loading {
            self.stack.set_visible_child(&self.loading_page);
        } else if self.last_error.is_some() {
            self.stack.set_visible_child(&self.error_page);
        } else {
            self.stack.set_visible_child(&self.empty_page);
        }
    }
}
