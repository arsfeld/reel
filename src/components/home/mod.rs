use std::collections::HashSet;
use std::sync::Arc;

use adw;
use gtk::prelude::*;
use relm4::prelude::*;

use crate::models::hub::MediaHub;
use crate::models::media::{MediaItem, SourceType};
use crate::models::watch::WatchProgress;
use crate::services::artwork::ArtworkCache;
use crate::services::media_source::{MediaSource, SourceError};
use crate::services::session_cache::CachedHome;

mod hero;
mod shelf;

use hero::{Hero, hero_candidates, next_index, prev_index};
use shelf::{Generation, HomeCard, Shelf, ShelfId, hub_duplicates_core, poster_result_is_current};

pub struct HomeView {
    /// The browsed source: drives Latest / Recently Added / Collections / hubs.
    source: Option<Arc<dyn MediaSource>>,
    /// All connected sources (label + source), used only to build the single
    /// merged Continue Watching row that spans every server.
    /// All connected sources as `(source_type, source_id, source)`, so the
    /// merged Continue Watching row can resolve each item's OWNING source for
    /// playback/artwork — never the browsed one (a CW card may come from a
    /// different server than the one being browsed).
    sources: Vec<(SourceType, String, Arc<dyn MediaSource>)>,
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
    /// True while a saved source is being validated on startup. Keeps the
    /// connecting page pinned so a premature `LoadHome` (fired before the source
    /// is ready) can't fall through to the "Connect to Plex" empty page.
    connecting: bool,
    /// Hidden-library visibility keys (`source_type:source_id:section_key`).
    /// Items belonging to a hidden library are dropped from Continue Watching
    /// and Recently Added when home data is built.
    hidden: HashSet<String>,
}

#[allow(clippy::large_enum_variant)]
pub enum HomeViewMsg {
    SetSource(Arc<dyn MediaSource>, Arc<ArtworkCache>),
    /// Update the full set of connected sources (label + source). Drives the
    /// merged, cross-source Continue Watching row.
    SetSources(Vec<(SourceType, String, Arc<dyn MediaSource>)>),
    /// Show/hide the "Connecting to Plex…" loading page.
    SetConnecting(bool),
    /// Update the hidden-library set. Applied to subsequently loaded home data.
    SetVisibility(HashSet<String>),
    /// Load home page data: watch_progress items from local DB + recently_added from source.
    LoadHome {
        in_progress: Vec<(MediaItem, WatchProgress)>,
    },
    /// Render Home instantly from a session-cached payload (no fetch, no loading
    /// page). Emitted by App on a Home content-cache hit.
    ShowCached(Box<CachedHome>),
    /// Background-revalidate Home: refetch while cached shelves stay visible (no
    /// clear, no loading page). The result reports to App, which re-renders only
    /// if the content changed. Server sources only (Local is deferred).
    Revalidate,
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
            Self::SetSources(s) => write!(f, "SetSources({} sources)", s.len()),
            Self::SetConnecting(v) => write!(f, "SetConnecting({v})"),
            Self::SetVisibility(h) => write!(f, "SetVisibility({} hidden)", h.len()),
            Self::LoadHome { in_progress } => {
                write!(f, "LoadHome({} in progress)", in_progress.len())
            }
            Self::ShowCached(_) => write!(f, "ShowCached(..)"),
            Self::Revalidate => write!(f, "Revalidate"),
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

/// Whether a fetch result is a real failure (vs a genuine empty). A
/// `NotSupported` source/endpoint legitimately offers no row and counts as
/// empty-success, so it must not mark a Home load incomplete. Any other error is
/// a real failure that should skip caching the partial Home.
pub(crate) fn is_real_source_error<T>(r: &Result<T, SourceError>) -> bool {
    matches!(r, Err(e) if !matches!(e, SourceError::NotSupported(_)))
}

/// Merge per-source Continue Watching lists into one badged list. Dedupe by the
/// composite MediaItem id (first occurrence wins — the same title held on two
/// servers has DIFFERENT external ids, so it correctly appears once per server,
/// each badged; cross-source title de-dup is explicitly out of scope). Order is
/// best-effort: per-source order preserved, sources concatenated in input order
/// (MediaItem carries no reliable cross-source last-played timestamp).
pub fn merge_continue_watching(
    per_source: Vec<(String, Vec<MediaItem>)>,
) -> Vec<(MediaItem, String)> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut merged: Vec<(MediaItem, String)> = Vec::new();
    for (label, items) in per_source {
        for item in items {
            if seen.insert(item.id.clone()) {
                merged.push((item, label.clone()));
            }
        }
    }
    merged
}

/// Drop merged Continue Watching entries belonging to hidden libraries while
/// preserving each surviving item's source label. Mirrors
/// `retain_visible_items` but operates on `(item, label)` pairs.
pub fn retain_visible_merged(
    items: Vec<(MediaItem, String)>,
    hidden: &HashSet<String>,
) -> Vec<(MediaItem, String)> {
    items
        .into_iter()
        .filter(|(item, _)| crate::services::visibility::is_item_visible(item, hidden))
        .collect()
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum HomeViewOutput {
    ShowDetail(MediaItem),
    PlayMedia {
        url: String,
        media_item: MediaItem,
    },
    ShowConnectionDialog,
    Error(String),
    /// A complete Home load finished assembling; App stores it in the session
    /// content cache keyed by the current source set. Only emitted when every
    /// source contributed without error, so a partial Home is never cached.
    HomeAssembled(Box<CachedHome>),
    /// A background Home revalidation finished (success, incomplete, or error).
    /// App clears its in-flight flag so the next revisit can revalidate again.
    RevalidationDone,
}

#[derive(Debug)]
pub enum HomeViewCmd {
    HomeData {
        /// Merged, cross-source Continue Watching: each item paired with its
        /// source's display label for badging. Empty when no source advertises
        /// server-side Continue Watching (the local-DB fallback is used instead).
        continue_watching: Vec<(MediaItem, String)>,
        /// (library title, items) for each non-empty library, in library order.
        recently_added: Vec<(String, Vec<MediaItem>)>,
        collections: Vec<MediaItem>,
        /// Server-curated hubs (Recommended / Because-you-watched / genres),
        /// in server order. Empty for non-Plex sources.
        hubs: Vec<MediaHub>,
        /// True when every per-source fetch in this load succeeded (no errors,
        /// only genuine empties). A partial load is rendered but not cached, so a
        /// transient source failure never freezes an incomplete Home on revisit.
        complete: bool,
        /// True when this result came from a background revalidation (cached
        /// shelves are still on screen). The handler then reports to App without
        /// rendering, and App re-renders only if the content changed. False for a
        /// normal first load, which renders immediately.
        revalidation: bool,
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
            .description(
                "Connect a Plex or Jellyfin server to see your Continue Watching and Recently Added",
            )
            .icon_name("folder-videos-symbolic")
            .build();

        let connect_btn = gtk::Button::builder()
            .label("Connect a Server")
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
            sources: Vec::new(),
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
            connecting: false,
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
            HomeViewMsg::SetSources(sources) => {
                self.sources = sources;
            }
            HomeViewMsg::SetConnecting(connecting) => {
                self.connecting = connecting;
                self.refresh_visible_page();
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

                // With no server-CW source at all (e.g. only Local), build the
                // single Continue Watching row from the local DB progress the app
                // pushes in. Server sources get CW from the async fetch instead.
                let server_continue_watching = self
                    .sources
                    .iter()
                    .any(|(source_type, _, _)| source_type.provides_server_hubs());
                if !server_continue_watching && !in_progress.is_empty() {
                    let cw_id = self.add_shelf("Continue Watching");
                    let cards: Vec<(MediaItem, Option<f64>)> = in_progress
                        .iter()
                        .map(|(item, progress)| (item.clone(), Some(progress.progress_fraction())))
                        .collect();
                    self.populate_shelf(cw_id, cards, true, &sender);
                }

                if self.source.is_some() {
                    self.refresh_visible_page();
                    self.spawn_home_fetch(false, &sender);
                } else {
                    self.loading = false;
                    self.refresh_visible_page();
                }
            }
            HomeViewMsg::Revalidate => {
                // Background refresh while cached shelves stay visible. Server
                // sources only — Local revalidation is deferred (its content
                // changes via filesystem events, out of scope). No clear, no
                // loading page; the result reports to App which re-renders on
                // change.
                let has_server = self
                    .sources
                    .iter()
                    .any(|(source_type, _, _)| source_type.provides_server_hubs());
                if has_server && self.source.is_some() {
                    self.spawn_home_fetch(true, &sender);
                } else {
                    // Can't dispatch (no server source, or browsed source not set
                    // yet on a startup race). Clear App's in-flight flag so a later
                    // revisit can revalidate again — otherwise it sticks forever.
                    let _ = sender.output(HomeViewOutput::RevalidationDone);
                }
            }
            HomeViewMsg::ShowCached(home) => {
                // Instant render from the session cache: no fetch, no loading page.
                self.loading = false;
                self.last_error = None;
                self.clear_shelves();
                let CachedHome {
                    continue_watching,
                    recently_added,
                    collections,
                    hubs,
                    ..
                } = *home;
                self.render_home(
                    continue_watching,
                    recently_added,
                    collections,
                    hubs,
                    &sender,
                );
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
                        (self.source_for_item(&item), item.file_path.as_ref())
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
                        (self.source_for_item(&item), item.file_path.as_ref())
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
                complete,
                revalidation,
            } => {
                self.loading = false;
                // Hand a complete load to App for session caching (App fills the
                // source-set key). A partial load renders but is not cached, so a
                // transient source failure never freezes an incomplete Home.
                if complete {
                    let _ = sender.output(HomeViewOutput::HomeAssembled(Box::new(CachedHome {
                        source_set_key: String::new(),
                        continue_watching: continue_watching.clone(),
                        recently_added: recently_added.clone(),
                        collections: collections.clone(),
                        hubs: hubs.clone(),
                        epoch: 0,
                    })));
                }
                // First load renders immediately (shelves were cleared, a loading
                // page is showing). A revalidation keeps the cached shelves up —
                // App diffs the reported payload and re-renders via ShowCached only
                // if the content changed, so a revisit doesn't churn the view. The
                // RevalidationDone signal always fires so App clears its in-flight
                // flag even when the result was incomplete (and thus not cached).
                if revalidation {
                    let _ = sender.output(HomeViewOutput::RevalidationDone);
                } else {
                    self.render_home(
                        continue_watching,
                        recently_added,
                        collections,
                        hubs,
                        &sender,
                    );
                }
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
    /// Fan out over every server source to assemble Home data (Continue
    /// Watching, Recently Added, Collections, hubs), then deliver it as a
    /// `HomeData` command. Shared by the first load (`revalidation = false`,
    /// shelves already cleared and a loading page showing) and background
    /// revalidation (`revalidation = true`, cached shelves still on screen).
    fn spawn_home_fetch(&self, revalidation: bool, sender: &ComponentSender<Self>) {
        let cw_sources: Vec<(String, Arc<dyn MediaSource>)> = self
            .sources
            .iter()
            .filter(|(source_type, _, _)| source_type.provides_server_hubs())
            .map(|(_, _, s)| (s.name().to_string(), s.clone()))
            .collect();
        let Some(src) = self.source.clone() else {
            return;
        };
        sender.oneshot_command(async move {
            // `complete` tracks whether every fetch returned without a real error.
            // NotSupported (an endpoint that simply doesn't offer a row) counts as
            // a genuine empty, not a failure, so it does not block caching.
            let mut complete = true;

            let cw_futures = cw_sources
                .into_iter()
                .map(|(label, s)| async move { (label, s.continue_watching().await) });
            let cw_results: Vec<(String, Result<Vec<MediaItem>, SourceError>)> =
                futures::future::join_all(cw_futures).await;
            let per_source: Vec<(String, Vec<MediaItem>)> = cw_results
                .into_iter()
                .map(|(label, r)| {
                    complete &= !is_real_source_error(&r);
                    (label, r.unwrap_or_default())
                })
                .collect();
            let continue_watching = merge_continue_watching(per_source);

            let libs = match src.libraries().await {
                Ok(l) => l,
                Err(e) => {
                    // A background revalidation failure must not replace the cached
                    // shelves with an error page — report an incomplete result so
                    // App leaves the cache untouched and clears the in-flight flag.
                    if revalidation {
                        return HomeViewCmd::HomeData {
                            continue_watching,
                            recently_added: Vec::new(),
                            collections: Vec::new(),
                            hubs: Vec::new(),
                            complete: false,
                            revalidation: true,
                        };
                    }
                    return HomeViewCmd::Error(e.to_string());
                }
            };

            // Per-library Recently Added, fetched concurrently.
            let ra_futures = libs.iter().map(|lib| {
                let src = src.clone();
                let key = lib.key.clone();
                let title = lib.title.clone();
                async move { (title, src.recently_added_in_library(&key).await) }
            });
            let mut recently_added: Vec<(String, Vec<MediaItem>)> = Vec::new();
            for (title, r) in futures::future::join_all(ra_futures).await {
                complete &= !is_real_source_error(&r);
                let mut items = r.unwrap_or_default();
                items.truncate(20);
                if !items.is_empty() {
                    recently_added.push((title, items));
                }
            }

            // Collections across all libraries.
            let col_futures = libs.iter().map(|lib| {
                let src = src.clone();
                let key = lib.key.clone();
                async move { src.collections(&key).await }
            });
            let mut collections: Vec<MediaItem> = Vec::new();
            for r in futures::future::join_all(col_futures).await {
                complete &= !is_real_source_error(&r);
                collections.extend(r.unwrap_or_default());
            }

            // Server-curated hubs. NotSupported (non-Plex) yields an empty list.
            let hubs = match src.hubs().await {
                Ok(h) => h,
                Err(e) => {
                    complete &= matches!(e, SourceError::NotSupported(_));
                    Vec::new()
                }
            };

            HomeViewCmd::HomeData {
                continue_watching,
                recently_added,
                collections,
                hubs,
                complete,
                revalidation,
            }
        });
    }

    /// Build the Home shelves from assembled data. Shared by a completed network
    /// load (`HomeData`) and an instant cache render (`ShowCached`). Applies the
    /// current hidden-library filter at render time, so a visibility change is
    /// reflected even when rendering from cache. Assumes shelves were already
    /// cleared by the caller.
    fn render_home(
        &mut self,
        continue_watching: Vec<(MediaItem, String)>,
        recently_added: Vec<(String, Vec<MediaItem>)>,
        collections: Vec<MediaItem>,
        hubs: Vec<MediaHub>,
        sender: &ComponentSender<Self>,
    ) {
        // Drop content from hidden libraries before anything consumes the lists.
        // Filtering here (at render time, against the current hidden set) means a
        // library hidden while a fetch was in flight — or since the cache was
        // populated — is still excluded.
        let continue_watching = retain_visible_merged(continue_watching, &self.hidden);
        let recently_added: Vec<(String, Vec<MediaItem>)> = recently_added
            .into_iter()
            .filter_map(|(title, items)| {
                let items = crate::services::visibility::retain_visible_items(items, &self.hidden);
                (!items.is_empty()).then_some((title, items))
            })
            .collect();

        // Featured hero: drawn from recently-added items that have a backdrop.
        // Computed before the shelves consume the lists.
        let hero_pool: Vec<MediaItem> = recently_added
            .iter()
            .flat_map(|(_, items)| items.iter().cloned())
            .collect();
        self.set_hero(hero_candidates(&hero_pool, 6), sender);

        if !continue_watching.is_empty() {
            let cw_id = self.add_shelf("Continue Watching");
            let cards: Vec<(MediaItem, Option<f64>, String)> = continue_watching
                .into_iter()
                .map(|(item, label)| {
                    let frac = item.resume_fraction();
                    (item, frac, label)
                })
                .collect();
            self.populate_continue_watching(cw_id, cards, sender);
        }

        // One Recently Added shelf per library (already filtered to non-empty
        // libraries, in library order).
        for (library_title, items) in recently_added {
            let ra_id = self.add_shelf(&format!("Recently Added — {library_title}"));
            let cards: Vec<(MediaItem, Option<f64>)> =
                items.into_iter().map(|item| (item, None)).collect();
            self.populate_shelf(ra_id, cards, false, sender);
        }

        if !collections.is_empty() {
            let col_id = self.add_shelf("Collections");
            let cards: Vec<(MediaItem, Option<f64>)> =
                collections.into_iter().map(|item| (item, None)).collect();
            self.populate_shelf(col_id, cards, false, sender);
        }

        // Append server-curated hubs below the Reel-owned core, dropping any that
        // duplicate Continue Watching / Recently Added.
        for hub in hubs {
            if hub.items.is_empty() || hub_duplicates_core(hub.identifier.as_deref()) {
                continue;
            }
            let hub_id = self.add_shelf(&hub.title);
            let cards: Vec<(MediaItem, Option<f64>)> =
                hub.items.into_iter().map(|item| (item, None)).collect();
            self.populate_shelf(hub_id, cards, false, sender);
        }

        self.refresh_visible_page();
    }

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

    /// Fill the cross-source Continue Watching shelf. Like `populate_shelf` with
    /// `resume_on_click = true`, but each card is badged with its source label
    /// so the merged row makes clear which server an item came from.
    fn populate_continue_watching(
        &mut self,
        shelf_id: ShelfId,
        cards: Vec<(MediaItem, Option<f64>, String)>,
        sender: &ComponentSender<Self>,
    ) {
        if cards.is_empty() {
            return;
        }
        if let Some(shelf) = self.shelves.iter_mut().find(|s| s.id == shelf_id) {
            for (item, progress, label) in cards {
                let card = HomeCard::new();
                card.set_media_with_source(&item, progress, &label);

                let sender_card = sender.input_sender().clone();
                let item_click = item.clone();
                card.gesture.connect_released(move |_, _, _, _| {
                    let _ = sender_card.send(HomeViewMsg::CardActivated {
                        item: item_click.clone(),
                        resume: true,
                    });
                });

                shelf.row.append(&card.container);
                shelf.cards.push((card, item));
            }
            shelf.section.set_visible(true);
        }

        self.fetch_posters(shelf_id, sender);
    }

    /// Resolve the source that owns an item — matched by `source_type` +
    /// `source_id` against the connected-source list — so a merged Continue
    /// Watching card from another server uses ITS server's playback/artwork URL.
    /// Falls back to the browsed source for items not in the list.
    fn source_for_item(&self, item: &MediaItem) -> Option<&Arc<dyn MediaSource>> {
        self.sources
            .iter()
            .find(|(source_type, source_id, _)| {
                *source_type == item.source_type && *source_id == item.source_id
            })
            .map(|(_, _, source)| source)
            .or(self.source.as_ref())
    }

    fn fetch_posters(&self, shelf_id: ShelfId, sender: &ComponentSender<Self>) {
        let Some(ref cache) = self.artwork_cache else {
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
            // Resolve the item's owning source so cross-source CW cards load
            // artwork from their own server.
            let Some(source) = self.source_for_item(item) else {
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
        } else if self.connecting {
            self.stack.set_visible_child(&self.connecting_page);
        } else if self.loading {
            self.stack.set_visible_child(&self.loading_page);
        } else if self.last_error.is_some() {
            self.stack.set_visible_child(&self.error_page);
        } else {
            self.stack.set_visible_child(&self.empty_page);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::media::{MediaType, SourceType};

    #[test]
    fn not_supported_is_not_a_real_error() {
        // A source that doesn't offer a row (NotSupported) is empty-success and
        // must not mark a Home load incomplete.
        let r: Result<Vec<MediaItem>, SourceError> =
            Err(SourceError::NotSupported("no hubs".into()));
        assert!(!is_real_source_error(&r));
    }

    #[test]
    fn connection_failure_is_a_real_error() {
        let r: Result<Vec<MediaItem>, SourceError> = Err(SourceError::Connection("timeout".into()));
        assert!(is_real_source_error(&r));
    }

    #[test]
    fn auth_and_not_found_are_real_errors() {
        let auth: Result<Vec<MediaItem>, SourceError> = Err(SourceError::Auth("401".into()));
        let nf: Result<Vec<MediaItem>, SourceError> = Err(SourceError::NotFound("gone".into()));
        let other: Result<Vec<MediaItem>, SourceError> = Err(SourceError::Other("x".into()));
        assert!(is_real_source_error(&auth));
        assert!(is_real_source_error(&nf));
        assert!(is_real_source_error(&other));
    }

    #[test]
    fn empty_success_is_not_a_real_error() {
        let r: Result<Vec<MediaItem>, SourceError> = Ok(Vec::new());
        assert!(!is_real_source_error(&r));
    }

    fn cw_item(source_type: SourceType, source_id: &str, external_id: &str) -> MediaItem {
        MediaItem {
            id: format!("{}:{source_id}:{external_id}", source_type.as_str()),
            source_type,
            source_id: source_id.into(),
            external_id: external_id.into(),
            media_type: MediaType::Movie,
            title: format!("Title {external_id}"),
            year: None,
            overview: None,
            content_rating: None,
            rating: None,
            runtime_minutes: None,
            poster_path: None,
            series_poster_path: None,
            backdrop_path: None,
            genres: Vec::new(),
            parent_id: None,
            season_number: None,
            episode_number: None,
            air_date: None,
            file_path: None,
            video_resolution: None,
            hdr: None,
            added_at: String::new(),
            updated_at: String::new(),
            playback_position_ms: None,
            watched: false,
            library_section_id: None,
        }
    }

    #[test]
    fn merge_continue_watching_across_sources() {
        let plex = vec![
            cw_item(SourceType::Plex, "p", "1"),
            cw_item(SourceType::Plex, "p", "2"),
        ];
        let jelly = vec![cw_item(SourceType::Jellyfin, "j", "9")];
        let merged =
            merge_continue_watching(vec![("Plex".into(), plex), ("Jellyfin".into(), jelly)]);
        let ids: Vec<&str> = merged.iter().map(|(i, _)| i.id.as_str()).collect();
        assert_eq!(merged.len(), 3);
        assert!(ids.contains(&"plex:p:1"));
        assert!(ids.contains(&"plex:p:2"));
        assert!(ids.contains(&"jellyfin:j:9"));
    }

    #[test]
    fn merge_dedupes_by_composite_id() {
        let a = vec![cw_item(SourceType::Plex, "p", "1")];
        let b = vec![
            cw_item(SourceType::Plex, "p", "1"),
            cw_item(SourceType::Plex, "p", "2"),
        ];
        let merged = merge_continue_watching(vec![("A".into(), a), ("B".into(), b)]);
        assert_eq!(merged.len(), 2);
        // First occurrence wins: the duplicate keeps source A's label.
        let dup = merged.iter().find(|(i, _)| i.id == "plex:p:1").unwrap();
        assert_eq!(dup.1, "A");
    }

    #[test]
    fn merged_items_carry_source_label() {
        let plex = vec![cw_item(SourceType::Plex, "p", "1")];
        let jelly = vec![cw_item(SourceType::Jellyfin, "j", "9")];
        let merged = merge_continue_watching(vec![
            ("Living Room".into(), plex),
            ("Basement".into(), jelly),
        ]);
        let p = merged.iter().find(|(i, _)| i.id == "plex:p:1").unwrap();
        let j = merged.iter().find(|(i, _)| i.id == "jellyfin:j:9").unwrap();
        assert_eq!(p.1, "Living Room");
        assert_eq!(j.1, "Basement");
    }

    #[test]
    fn source_without_continue_watching_contributes_nothing() {
        let plex = vec![cw_item(SourceType::Plex, "p", "1")];
        let merged =
            merge_continue_watching(vec![("Empty".into(), Vec::new()), ("Plex".into(), plex)]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].0.id, "plex:p:1");
    }

    #[test]
    fn retain_visible_merged_preserves_labels() {
        let visible = cw_item(SourceType::Plex, "p", "1");
        let mut hidden_item = cw_item(SourceType::Plex, "p", "2");
        hidden_item.library_section_id = Some("42".into());
        let merged = vec![
            (visible, "Living Room".to_string()),
            (hidden_item, "Living Room".to_string()),
        ];

        let mut hidden = HashSet::new();
        hidden.insert("plex:p:42".to_string());

        let kept = retain_visible_merged(merged, &hidden);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].0.id, "plex:p:1");
        assert_eq!(kept[0].1, "Living Room");
    }
}
