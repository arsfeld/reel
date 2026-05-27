use std::sync::Arc;

use gtk4::prelude::*;
use libadwaita as adw;
use relm4::prelude::*;

use crate::models::media::MediaItem;
use crate::models::watch::WatchProgress;
use crate::services::artwork::ArtworkCache;
use crate::services::media_source::MediaSource;

/// A shelf card on the home screen. Smaller than grid cards (~160×240).
struct HomeCard {
    container: gtk4::Box,
    picture: gtk4::Picture,
    title_label: gtk4::Label,
    subtitle_label: gtk4::Label,
    progress_bar: gtk4::ProgressBar,
    /// Click gesture controller for this card.
    gesture: gtk4::GestureClick,
}

impl HomeCard {
    fn new() -> Self {
        let container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(4)
            .width_request(160)
            .css_classes(["home-card"])
            .build();

        let picture = gtk4::Picture::builder()
            .content_fit(gtk4::ContentFit::Cover)
            .width_request(160)
            .height_request(240)
            .css_classes(["home-card-poster", "loading"])
            .build();

        let progress_bar = gtk4::ProgressBar::builder()
            .halign(gtk4::Align::Fill)
            .valign(gtk4::Align::End)
            .css_classes(["watch-progress"])
            .visible(false)
            .build();

        let overlay = gtk4::Overlay::new();
        overlay.set_child(Some(&picture));
        overlay.add_overlay(&progress_bar);

        let title_label = gtk4::Label::builder()
            .halign(gtk4::Align::Start)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .max_width_chars(18)
            .lines(2)
            .wrap(true)
            .wrap_mode(gtk4::pango::WrapMode::WordChar)
            .css_classes(["home-card-title"])
            .build();

        let subtitle_label = gtk4::Label::builder()
            .halign(gtk4::Align::Start)
            .css_classes(["home-card-subtitle", "dim-label"])
            .visible(false)
            .build();

        container.append(&overlay);
        container.append(&title_label);
        container.append(&subtitle_label);

        let gesture = gtk4::GestureClick::new();
        container.add_controller(gesture.clone());

        Self {
            container,
            picture,
            title_label,
            subtitle_label,
            progress_bar,
            gesture,
        }
    }

    fn set_media(&self, item: &MediaItem, progress: Option<f64>) {
        self.title_label.set_label(&item.title);

        // Subtitle: year for movies, "S1 E5" for episodes
        match item.media_type {
            crate::models::media::MediaType::Episode => {
                let sub = match (item.season_number, item.episode_number) {
                    (Some(s), Some(e)) => format!("S{} E{}", s, e),
                    _ => {
                        if let Some(ref parent) = item.parent_id {
                            parent.clone()
                        } else {
                            String::new()
                        }
                    }
                };
                if sub.is_empty() {
                    self.subtitle_label.set_visible(false);
                } else {
                    self.subtitle_label.set_label(&sub);
                    self.subtitle_label.set_visible(true);
                }
            }
            _ => {
                if let Some(year) = item.year {
                    self.subtitle_label.set_label(&year.to_string());
                    self.subtitle_label.set_visible(true);
                } else {
                    self.subtitle_label.set_visible(false);
                }
            }
        }

        // Progress bar for continue watching
        if let Some(frac) = progress {
            if frac > 0.0 && frac < 1.0 {
                self.progress_bar.set_fraction(frac);
                self.progress_bar.set_visible(true);
            } else {
                self.progress_bar.set_visible(false);
            }
        } else {
            self.progress_bar.set_visible(false);
        }
    }

    fn set_poster(&self, texture: &gtk4::gdk::Texture) {
        self.picture.set_paintable(Some(texture));
        self.picture.remove_css_class("loading");
        self.picture.set_opacity(1.0);
    }
}

pub struct HomeView {
    source: Option<Arc<dyn MediaSource>>,
    artwork_cache: Option<Arc<ArtworkCache>>,
    /// The vertical container holding all shelf sections.
    #[allow(dead_code)]
    shelves_box: gtk4::Box,
    /// Continue Watching shelf.
    cw_section: gtk4::Box,
    cw_row: gtk4::Box,
    cw_cards: Vec<(HomeCard, MediaItem)>,
    /// Recently Added shelf.
    ra_section: gtk4::Box,
    ra_row: gtk4::Box,
    ra_cards: Vec<(HomeCard, MediaItem)>,
    /// Empty state page (shown when no data).
    empty_page: adw::StatusPage,
    /// Stack switches between shelves and empty page.
    stack: gtk4::Stack,
    /// Prevent concurrent loads.
    loading: bool,
}

pub enum HomeViewMsg {
    SetSource(Arc<dyn MediaSource>, Arc<ArtworkCache>),
    /// Load home page data: watch_progress items from local DB + recently_added from source.
    LoadHome {
        in_progress: Vec<(MediaItem, WatchProgress)>,
    },
    HomeLoaded {
        recently_added: Vec<MediaItem>,
    },
    LoadError(String),
    CardActivated(MediaItem),
}

impl std::fmt::Debug for HomeViewMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SetSource(..) => write!(f, "SetSource(..)"),
            Self::LoadHome { in_progress } => {
                write!(f, "LoadHome({} in progress)", in_progress.len())
            }
            Self::HomeLoaded { recently_added } => {
                write!(f, "HomeLoaded({} recent)", recently_added.len())
            }
            Self::LoadError(msg) => write!(f, "LoadError({msg})"),
            Self::CardActivated(item) => write!(f, "CardActivated({})", item.title),
        }
    }
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
}

#[derive(Debug)]
pub enum HomeViewCmd {
    Fetched(Vec<MediaItem>),
    PosterLoaded {
        index: usize,
        row: PosterRow,
        texture: gtk4::gdk::Texture,
    },
    Error(String),
}

/// Which shelf row a poster belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PosterRow {
    ContinueWatching,
    RecentlyAdded,
}

#[relm4::component(pub)]
impl Component for HomeView {
    type Init = ();
    type Input = HomeViewMsg;
    type Output = HomeViewOutput;
    type CommandOutput = HomeViewCmd;

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

        // Empty state
        let empty_page = adw::StatusPage::builder()
            .title("Welcome to Reel")
            .description("Connect a Plex server to see your Continue Watching and Recently Added")
            .icon_name("folder-videos-symbolic")
            .build();

        // "Connect to Plex" button on the empty page
        let connect_btn = gtk4::Button::builder()
            .label("Connect to Plex")
            .halign(gtk4::Align::Center)
            .css_classes(["pill", "suggested-action"])
            .build();
        let sender_btn = sender.output_sender().clone();
        connect_btn.connect_clicked(move |_| {
            let _ = sender_btn.send(HomeViewOutput::ShowConnectionDialog);
        });
        empty_page.set_child(Some(&connect_btn));

        // Shelves container
        let scroll = gtk4::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .build();

        let shelves_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(16)
            .margin_start(24)
            .margin_end(24)
            .margin_top(16)
            .margin_bottom(16)
            .build();

        // --- Continue Watching shelf ---
        let cw_section = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(8)
            .visible(false)
            .build();

        let cw_label = gtk4::Label::builder()
            .label("Continue Watching")
            .halign(gtk4::Align::Start)
            .css_classes(["title-2"])
            .build();

        let cw_row = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .build();

        let cw_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Never)
            .max_content_height(300)
            .child(&cw_row)
            .build();

        cw_section.append(&cw_label);
        cw_section.append(&cw_scroll);

        // --- Recently Added shelf ---
        let ra_section = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(8)
            .visible(false)
            .build();

        let ra_label = gtk4::Label::builder()
            .label("Recently Added")
            .halign(gtk4::Align::Start)
            .css_classes(["title-2"])
            .build();

        let ra_row = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .build();

        let ra_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Never)
            .max_content_height(300)
            .child(&ra_row)
            .build();

        ra_section.append(&ra_label);
        ra_section.append(&ra_scroll);

        shelves_box.append(&cw_section);
        shelves_box.append(&ra_section);
        scroll.set_child(Some(&shelves_box));

        stack.add_child(&empty_page);
        stack.add_child(&scroll);
        stack.set_visible_child(&empty_page);

        root.append(&stack);

        let model = Self {
            source: None,
            artwork_cache: None,
            shelves_box,
            cw_section,
            cw_row,
            cw_cards: Vec::new(),
            ra_section,
            ra_row,
            ra_cards: Vec::new(),
            empty_page,
            stack,
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
            HomeViewMsg::LoadHome { in_progress } => {
                if self.loading {
                    return;
                }
                self.loading = true;

                // Clear existing cards
                self.clear_cards();

                // Build Continue Watching row from local DB data
                self.build_cw_row(&in_progress, &sender);

                // Fetch Recently Added from source (async) — use library items sorted by date
                if let Some(ref source) = self.source {
                    let src = source.clone();
                    sender.oneshot_command(async move {
                        // Fetch all movie + show libraries then take most recent
                        match src.libraries().await {
                            Ok(libs) => {
                                let mut all = Vec::new();
                                for lib in &libs {
                                    if let Ok(items) = src.library_items(&lib.key).await {
                                        all.extend(items);
                                    }
                                }
                                // Sort by added_at descending, take top 20
                                all.sort_by(|a, b| b.added_at.cmp(&a.added_at));
                                all.truncate(20);
                                HomeViewCmd::Fetched(all)
                            }
                            Err(e) => HomeViewCmd::Error(e.to_string()),
                        }
                    });
                } else {
                    self.loading = false;
                    self.update_visibility();
                }
            }
            HomeViewMsg::HomeLoaded { recently_added } => {
                self.loading = false;

                // Build Recently Added row
                self.build_ra_row(&recently_added, &sender);
                self.update_visibility();
            }
            HomeViewMsg::LoadError(msg) => {
                self.loading = false;
                tracing::warn!("HomeView load error: {msg}");
                self.update_visibility();
            }
            HomeViewMsg::CardActivated(item) => {
                // For now, navigate to detail page (same as library grid click)
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
            HomeViewCmd::Fetched(items) => {
                sender.input(HomeViewMsg::HomeLoaded {
                    recently_added: items,
                });
            }
            HomeViewCmd::Error(msg) => {
                sender.input(HomeViewMsg::LoadError(msg));
            }
            HomeViewCmd::PosterLoaded {
                index,
                row,
                texture,
            } => {
                let cards = match row {
                    PosterRow::ContinueWatching => &self.cw_cards,
                    PosterRow::RecentlyAdded => &self.ra_cards,
                };
                if index < cards.len() {
                    cards[index].0.set_poster(&texture);
                }
            }
        }
    }
}

impl HomeView {
    fn clear_cards(&mut self) {
        // Remove all card widgets from rows
        while let Some(child) = self.cw_row.first_child() {
            self.cw_row.remove(&child);
        }
        while let Some(child) = self.ra_row.first_child() {
            self.ra_row.remove(&child);
        }
        self.cw_cards.clear();
        self.ra_cards.clear();
    }

    fn build_cw_row(
        &mut self,
        in_progress: &[(MediaItem, WatchProgress)],
        sender: &ComponentSender<Self>,
    ) {
        if in_progress.is_empty() {
            self.cw_section.set_visible(false);
            return;
        }

        for (item, progress) in in_progress {
            let card = HomeCard::new();
            let frac = progress.progress_fraction();
            card.set_media(item, Some(frac));

            // Wire click
            let sender_card = sender.input_sender().clone();
            let item_clone = item.clone();
            card.gesture.connect_released(move |_, _, _, _| {
                let _ = sender_card.send(HomeViewMsg::CardActivated(item_clone.clone()));
            });

            self.cw_row.append(&card.container);
            self.cw_cards.push((card, item.clone()));
        }

        self.cw_section.set_visible(true);

        // Kick off poster downloads
        self.fetch_posters(PosterRow::ContinueWatching, sender);
    }

    fn build_ra_row(&mut self, items: &[MediaItem], sender: &ComponentSender<Self>) {
        if items.is_empty() {
            self.ra_section.set_visible(false);
            return;
        }

        for item in items {
            let card = HomeCard::new();
            card.set_media(item, None);

            // Wire click
            let sender_card = sender.input_sender().clone();
            let item_clone = item.clone();
            card.gesture.connect_released(move |_, _, _, _| {
                let _ = sender_card.send(HomeViewMsg::CardActivated(item_clone.clone()));
            });

            self.ra_row.append(&card.container);
            self.ra_cards.push((card, item.clone()));
        }

        self.ra_section.set_visible(true);

        // Kick off poster downloads
        self.fetch_posters(PosterRow::RecentlyAdded, sender);
    }

    fn fetch_posters(&self, row: PosterRow, sender: &ComponentSender<Self>) {
        let Some(ref cache) = self.artwork_cache else {
            return;
        };

        let cards = match row {
            PosterRow::ContinueWatching => &self.cw_cards,
            PosterRow::RecentlyAdded => &self.ra_cards,
        };

        for (idx, (_, item)) in cards.iter().enumerate() {
            let poster_path = match &item.poster_path {
                Some(p) => p.clone(),
                None => continue,
            };
            let artwork_url = match &self.source {
                Some(s) => s.artwork_url(&poster_path, 320, 480),
                None => continue,
            };

            let cache_clone = cache.clone();
            let url_clone = artwork_url.clone();
            let sender_clone = sender.command_sender().clone();

            gtk4::glib::spawn_future_local(async move {
                match cache_clone.get_or_download(&url_clone).await {
                    Ok(path) => {
                        if let Ok(texture) = gtk4::gdk::Texture::from_filename(&path) {
                            let _ = sender_clone.send(HomeViewCmd::PosterLoaded {
                                index: idx,
                                row,
                                texture,
                            });
                        }
                    }
                    Err(e) => {
                        tracing::debug!("Poster load failed for {}: {e}", url_clone);
                    }
                }
            });
        }
    }

    fn update_visibility(&mut self) {
        let has_cw = !self.cw_cards.is_empty();
        let has_ra = !self.ra_cards.is_empty();

        if has_cw || has_ra {
            // Switch to the scroll (shelves) page
            if let Some(scroll) = self.stack.last_child() {
                self.stack.set_visible_child(&scroll);
            }
        } else {
            self.stack.set_visible_child(&self.empty_page);
        }
    }
}
