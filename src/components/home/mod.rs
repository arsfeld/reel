use std::sync::Arc;

use adw;
use gtk::prelude::*;
use relm4::prelude::*;

use crate::models::media::MediaItem;
use crate::models::watch::WatchProgress;
use crate::services::artwork::ArtworkCache;
use crate::services::media_source::MediaSource;

mod shelf;

use shelf::{Generation, HomeCard, Shelf, ShelfId, poster_result_is_current};

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
}

pub enum HomeViewMsg {
    SetSource(Arc<dyn MediaSource>, Arc<ArtworkCache>),
    /// Show/hide the "Connecting to Plex…" loading page.
    SetConnecting(bool),
    /// Load home page data: watch_progress items from local DB + recently_added from source.
    LoadHome {
        in_progress: Vec<(MediaItem, WatchProgress)>,
    },
    LoadError(String),
    /// A card was activated. `resume` is set for Continue Watching cards, which
    /// resume playback rather than opening a detail page.
    CardActivated { item: MediaItem, resume: bool },
}

impl std::fmt::Debug for HomeViewMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SetSource(..) => write!(f, "SetSource(..)"),
            Self::SetConnecting(v) => write!(f, "SetConnecting({v})"),
            Self::LoadHome { in_progress } => {
                write!(f, "LoadHome({} in progress)", in_progress.len())
            }
            Self::LoadError(msg) => write!(f, "LoadError({msg})"),
            Self::CardActivated { item, resume } => {
                write!(f, "CardActivated({}, resume={resume})", item.title)
            }
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
    },
    PosterLoaded {
        generation: Generation,
        shelf_id: ShelfId,
        index: usize,
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

        // Shelves container
        let scroll = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();

        let shelves_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .margin_start(24)
            .margin_end(24)
            .margin_top(16)
            .margin_bottom(16)
            .build();

        scroll.set_child(Some(&shelves_box));

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
            scroll,
            empty_page,
            loading_page,
            error_page,
            connecting_page,
            stack,
            last_error: None,
            loading: false,
        };

        ComponentParts { model, widgets }
    }

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
                                let mut items =
                                    src.recently_added_in_library(&key).await.unwrap_or_default();
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

                        HomeViewCmd::HomeData {
                            continue_watching,
                            recently_added,
                            collections,
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
                    && (resume
                        || item.media_type == crate::models::media::MediaType::Episode);
                if should_play {
                    if let (Some(source), Some(part)) = (self.source.as_ref(), item.file_path.as_ref())
                    {
                        let url = source.playback_url(part);
                        let _ = sender.output(HomeViewOutput::PlayMedia {
                            url,
                            media_item: item,
                        });
                        return;
                    }
                }
                let _ = sender.output(HomeViewOutput::ShowDetail(item));
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
            } => {
                self.loading = false;

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
                if let Some(shelf) = self.shelves.iter().find(|s| s.id == shelf_id) {
                    if let Some((card, _)) = shelf.cards.get(index) {
                        card.set_poster(&texture);
                    }
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
            let Some(poster_path) = item.poster_path.clone() else {
                continue;
            };
            let artwork_url = source.artwork_url(&poster_path, 320, 480);
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
