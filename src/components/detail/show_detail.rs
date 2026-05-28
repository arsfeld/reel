use std::collections::HashMap;
use std::sync::Arc;

use adw;
use adw::prelude::*;
use relm4::prelude::*;
use tracing::info;

use crate::db::database::Database;
use crate::db::watch_progress_repo::WatchProgressRepo;
use crate::models::detail::MediaDetail;
use crate::models::media::{MediaItem, MediaType};
use crate::models::watch::WatchProgress;
use crate::services::artwork::ArtworkCache;
use crate::services::media_source::MediaSource;

#[allow(dead_code)]
pub struct ShowDetail {
    show: Option<MediaItem>,
    seasons: Vec<MediaItem>,
    episodes: Vec<MediaItem>,
    selected_season: u32,
    source: Option<Arc<dyn MediaSource>>,
    artwork_cache: Option<Arc<ArtworkCache>>,
    db: Option<Database>,
    watch_progress: HashMap<String, WatchProgress>,
    // Widgets
    title_label: gtk::Label,
    meta_box: gtk::Box,
    year_label: gtk::Label,
    rating_label: gtk::Label,
    content_rating_label: gtk::Label,
    overview_label: gtk::Label,
    backdrop: gtk::Picture,
    poster: gtk::Picture,
    // Genre chips + actions
    genres_box: gtk::FlowBox,
    play_button: gtk::Button,
    // Director and writer credits
    director_label: gtk::Label,
    writer_label: gtk::Label,
    // Show info panel
    show_info_panel: gtk::Box,
    show_info_label: gtk::Label,
    // Season cards
    season_scroll: gtk::ScrolledWindow,
    season_cards_box: gtk::Box,
    season_section: gtk::Box,
    // Episode cards (horizontal row)
    episode_section: gtk::Box,
    episode_scroll: gtk::ScrolledWindow,
    episode_cards_box: gtk::Box,
    // Cast section
    cast_section: gtk::Box,
    cast_scroll: gtk::ScrolledWindow,
    cast_box: gtk::Box,
    // Tech info panel
    tech_label: gtk::Label,
    tech_panel: gtk::Box,
    // Collections panel
    collections_box: gtk::Box,
    collections_panel: gtk::Box,
}

#[allow(clippy::large_enum_variant)]
pub enum ShowDetailMsg {
    LoadShow(MediaItem),
    SetSource(Arc<dyn MediaSource>, Arc<ArtworkCache>),
    SetDb(Database),
    SelectSeason(u32),
    PlayEpisode(usize),
    /// Download the currently-loaded season's episodes as a group.
    DownloadSeason,
    SeasonsLoaded(Vec<MediaItem>),
    EpisodesLoaded(Vec<MediaItem>),
    LoadError(String),
}

impl std::fmt::Debug for ShowDetailMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadShow(item) => write!(f, "LoadShow({})", item.title),
            Self::SetSource(..) => write!(f, "SetSource(..)"),
            Self::SetDb(..) => write!(f, "SetDb(..)"),
            Self::SelectSeason(idx) => write!(f, "SelectSeason({idx})"),
            Self::PlayEpisode(idx) => write!(f, "PlayEpisode({idx})"),
            Self::DownloadSeason => write!(f, "DownloadSeason"),
            Self::SeasonsLoaded(s) => write!(f, "SeasonsLoaded({} seasons)", s.len()),
            Self::EpisodesLoaded(e) => write!(f, "EpisodesLoaded({} episodes)", e.len()),
            Self::LoadError(msg) => write!(f, "LoadError({msg})"),
        }
    }
}

#[derive(Debug)]
pub enum ShowDetailOutput {
    PlayMedia {
        url: String,
        media_item: Box<Option<crate::models::media::MediaItem>>,
    },
    /// Enqueue a show/season download: snapshot the current episode set.
    DownloadGroup {
        parent: Box<MediaItem>,
        episodes: Vec<MediaItem>,
        scope: crate::models::download::GroupScope,
    },
    Error(String),
}

#[allow(clippy::large_enum_variant)]
pub enum ShowDetailCmd {
    SeasonsReady(Vec<MediaItem>),
    EpisodesReady(Vec<MediaItem>),
    BackdropReady(gtk::gdk::Texture),
    PosterReady(gtk::gdk::Texture),
    SeasonArtworkReady(usize, gtk::gdk::Texture),
    EpisodeThumbReady(usize, gtk::gdk::Texture),
    DetailLoaded(MediaDetail),
    CastPhotoReady(usize, gtk::gdk::Texture),
    Error(String),
    Noop,
}

impl std::fmt::Debug for ShowDetailCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ShowDetailCmd")
    }
}

#[relm4::component(pub)]
impl Component for ShowDetail {
    type Init = ();
    type Input = ShowDetailMsg;
    type Output = ShowDetailOutput;
    type CommandOutput = ShowDetailCmd;

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

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();

        let main_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .hexpand(true)
            .build();

        // ═══ HERO: backdrop + gradient + headline (poster, title, meta, play) ═══

        let hero_overlay = gtk::Overlay::builder()
            .height_request(420)
            .hexpand(true)
            .build();

        let backdrop = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .css_classes(["detail-hero"])
            .vexpand(true)
            .hexpand(true)
            .build();

        let hero_gradient = gtk::Box::builder()
            .css_classes(["detail-hero-overlay"])
            .vexpand(true)
            .hexpand(true)
            .build();

        // Headline: poster + text column, anchored to the bottom of the hero and
        // clamped to match the content width below it.
        let headline_clamp = adw::Clamp::builder()
            .maximum_size(960)
            .valign(gtk::Align::End)
            .build();
        let headline_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(28)
            .valign(gtk::Align::End)
            .margin_start(28)
            .margin_end(28)
            .margin_bottom(24)
            .css_classes(["detail-hero-headline"])
            .build();

        // Poster floats at bottom-left. Hidden until art loads; GTK4 skips hidden
        // children in box layout so the text column reclaims the room.
        let poster = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .width_request(170)
            .height_request(255)
            .css_classes(["detail-poster-hero"])
            .halign(gtk::Align::Start)
            .valign(gtk::Align::End)
            .visible(false)
            .build();

        let text_column = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(10)
            .hexpand(true)
            .valign(gtk::Align::End)
            .build();

        // Title — capped at 2 lines so long titles don't push the badges and Play
        // button out of the fixed-height hero.
        let title_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .wrap(true)
            .lines(2)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .css_classes(["title-1", "detail-hero-title"])
            .build();

        let meta_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .halign(gtk::Align::Start)
            .build();

        let year_label = gtk::Label::builder()
            .css_classes(["detail-badge"])
            .visible(false)
            .build();
        let rating_label = gtk::Label::builder()
            .css_classes(["detail-badge", "accent"])
            .visible(false)
            .build();
        let content_rating_label = gtk::Label::builder()
            .css_classes(["detail-badge"])
            .visible(false)
            .build();

        meta_box.append(&year_label);
        meta_box.append(&rating_label);
        meta_box.append(&content_rating_label);

        // Credits (populated when MediaDetail arrives)
        let director_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .wrap(true)
            .css_classes(["dim-label", "detail-hero-credit"])
            .visible(false)
            .build();
        let writer_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .wrap(true)
            .css_classes(["dim-label", "detail-hero-credit"])
            .visible(false)
            .build();

        // Action buttons
        let actions_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .halign(gtk::Align::Start)
            .css_classes(["detail-actions"])
            .build();

        let play_button = gtk::Button::builder()
            .label("▶  Play")
            .css_classes(["suggested-action", "pill"])
            .halign(gtk::Align::Start)
            .build();

        let sender_play = sender.input_sender().clone();
        play_button.connect_clicked(move |_| {
            let _ = sender_play.send(ShowDetailMsg::PlayEpisode(0));
        });

        let download_button = gtk::Button::builder()
            .label("⤓  Download Season")
            .css_classes(["pill"])
            .halign(gtk::Align::Start)
            .build();
        let sender_dl = sender.input_sender().clone();
        download_button.connect_clicked(move |_| {
            let _ = sender_dl.send(ShowDetailMsg::DownloadSeason);
        });

        actions_row.append(&play_button);
        actions_row.append(&download_button);

        text_column.append(&title_label);
        text_column.append(&meta_box);
        text_column.append(&director_label);
        text_column.append(&writer_label);
        text_column.append(&actions_row);

        headline_row.append(&poster);
        headline_row.append(&text_column);
        headline_clamp.set_child(Some(&headline_row));

        hero_overlay.set_child(Some(&backdrop));
        hero_overlay.add_overlay(&hero_gradient);
        hero_overlay.add_overlay(&headline_clamp);

        main_box.append(&hero_overlay);

        // ═══ Clamped content below hero ═══

        let clamp = adw::Clamp::builder().maximum_size(960).build();
        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_start(20)
            .margin_end(20)
            .margin_top(20)
            .margin_bottom(32)
            .build();

        // ═══ Overview ═══

        let overview_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .wrap(true)
            .visible(false)
            .build();
        content_box.append(&overview_label);

        // ═══ Genre chips ═══

        let genres_box = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .halign(gtk::Align::Start)
            .max_children_per_line(10)
            .row_spacing(6)
            .column_spacing(6)
            .build();
        content_box.append(&genres_box);

        // ═══ Show info panel (frosted glass) ═══

        let show_info_panel = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["detail-panel"])
            .visible(false)
            .build();
        let show_info_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        show_info_panel.append(&show_info_label);
        content_box.append(&show_info_panel);

        // ═══ Season cards (horizontal scrolling row) ═══

        let season_section = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .visible(false)
            .build();

        let season_heading = gtk::Label::builder()
            .label("Seasons")
            .halign(gtk::Align::Start)
            .css_classes(["detail-section-title"])
            .build();

        let season_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .height_request(248)
            .build();

        let season_cards_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .build();

        season_scroll.set_child(Some(&season_cards_box));
        season_section.append(&season_heading);
        season_section.append(&season_scroll);
        content_box.append(&season_section);

        // ═══ Episode cards (horizontal scrolling row) ═══

        let episode_section = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .visible(false)
            .build();

        let episode_heading = gtk::Label::builder()
            .label("Episodes")
            .halign(gtk::Align::Start)
            .css_classes(["detail-section-title"])
            .build();

        let episode_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .height_request(310)
            .build();

        let episode_cards_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        episode_scroll.set_child(Some(&episode_cards_box));
        episode_section.append(&episode_heading);
        episode_section.append(&episode_scroll);
        content_box.append(&episode_section);

        // ═══ Cast section ═══

        let cast_section = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .visible(false)
            .build();
        let cast_heading = gtk::Label::builder()
            .label("Cast")
            .halign(gtk::Align::Start)
            .css_classes(["detail-section-title"])
            .build();
        let cast_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .height_request(140)
            .build();
        let cast_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(10)
            .build();
        cast_scroll.set_child(Some(&cast_box));
        cast_section.append(&cast_heading);
        cast_section.append(&cast_scroll);
        content_box.append(&cast_section);

        // ═══ Technical info panel ═══

        let tech_panel = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["detail-panel"])
            .visible(false)
            .build();
        let tech_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        tech_panel.append(&tech_label);
        content_box.append(&tech_panel);

        // ═══ Collections panel ═══

        let collections_panel = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["detail-panel"])
            .visible(false)
            .build();
        let collections_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .halign(gtk::Align::Start)
            .build();
        collections_panel.append(&collections_box);
        content_box.append(&collections_panel);

        clamp.set_child(Some(&content_box));
        main_box.append(&clamp);
        scrolled.set_child(Some(&main_box));
        toolbar.set_content(Some(&scrolled));
        root.append(&toolbar);

        let model = Self {
            show: None,
            seasons: Vec::new(),
            episodes: Vec::new(),
            selected_season: 0,
            source: None,
            artwork_cache: None,
            db: None,
            watch_progress: HashMap::new(),
            title_label,
            meta_box,
            year_label,
            rating_label,
            content_rating_label,
            overview_label,
            backdrop,
            poster,
            genres_box,
            play_button,
            director_label,
            writer_label,
            show_info_panel,
            show_info_label,
            season_scroll,
            season_cards_box,
            season_section,
            episode_section,
            episode_scroll,
            episode_cards_box,
            cast_section,
            cast_scroll,
            cast_box,
            tech_label,
            tech_panel,
            collections_box,
            collections_panel,
        };

        ComponentParts { model, widgets }
    }

    #[allow(clippy::too_many_lines)]
    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            ShowDetailMsg::SetSource(source, artwork_cache) => {
                self.source = Some(source);
                self.artwork_cache = Some(artwork_cache);
            }
            ShowDetailMsg::SetDb(db) => {
                self.db = Some(db);
            }
            ShowDetailMsg::LoadShow(show) => {
                info!("Loading show detail: {}", show.title);

                self.title_label.set_label(&show.display_title());

                if let Some(year) = show.year {
                    self.year_label.set_label(&year.to_string());
                    self.year_label.set_visible(true);
                } else {
                    self.year_label.set_visible(false);
                }

                if let Some(rating) = show.rating {
                    self.rating_label.set_label(&format!("★ {rating:.1}"));
                    self.rating_label.set_visible(true);
                } else {
                    self.rating_label.set_visible(false);
                }

                if let Some(ref cr) = show.content_rating {
                    self.content_rating_label.set_label(cr);
                    self.content_rating_label.set_visible(true);
                } else {
                    self.content_rating_label.set_visible(false);
                }

                // Genre chips
                while let Some(child) = self.genres_box.first_child() {
                    self.genres_box.remove(&child);
                }
                for genre in &show.genres {
                    let label = gtk::Label::builder()
                        .label(genre)
                        .css_classes(["genre-chip"])
                        .build();
                    self.genres_box.insert(&label, -1);
                }

                if let Some(ref overview) = show.overview {
                    self.overview_label.set_label(overview);
                    self.overview_label.set_visible(true);
                } else {
                    self.overview_label.set_visible(false);
                }

                self.play_button.set_sensitive(show.file_path.is_some());

                // Reset enriched sections while loading
                self.director_label.set_visible(false);
                self.writer_label.set_visible(false);
                self.show_info_panel.set_visible(false);
                self.cast_section.set_visible(false);
                self.tech_panel.set_visible(false);
                self.collections_panel.set_visible(false);

                self.poster.set_visible(false);
                self.episode_section.set_visible(false);

                // Clear any previous show's backdrop so it doesn't linger while
                // the new one loads (or stays empty when the new show has none).
                self.backdrop.set_paintable(None::<&gtk::gdk::Texture>);
                if show.backdrop_path.is_none() {
                    self.backdrop.add_css_class("detail-hero-empty");
                } else {
                    self.backdrop.remove_css_class("detail-hero-empty");
                }

                // Load artwork
                if let (Some(source), Some(cache)) = (&self.source, &self.artwork_cache) {
                    if let Some(art_path) = &show.backdrop_path {
                        let url = source.artwork_url(art_path, 1280, 400);
                        let cache = Arc::clone(cache);
                        sender.oneshot_command(async move {
                            match cache.get_or_download(&url).await {
                                Ok(path) => match gtk::gdk::Texture::from_filename(&path) {
                                    Ok(tex) => ShowDetailCmd::BackdropReady(tex),
                                    Err(_) => ShowDetailCmd::Noop,
                                },
                                Err(_) => ShowDetailCmd::Noop,
                            }
                        });
                    }

                    if let Some(poster_path) = &show.poster_path {
                        let url = source.artwork_url(poster_path, 340, 510);
                        let cache = Arc::clone(cache);
                        sender.oneshot_command(async move {
                            match cache.get_or_download(&url).await {
                                Ok(path) => match gtk::gdk::Texture::from_filename(&path) {
                                    Ok(tex) => ShowDetailCmd::PosterReady(tex),
                                    Err(_) => ShowDetailCmd::Noop,
                                },
                                Err(_) => ShowDetailCmd::Noop,
                            }
                        });
                    }
                }

                // Fetch enriched metadata (cast, crew, technical info, collections)
                if let Some(source) = self.source.clone() {
                    let external_id = show.external_id.clone();
                    sender.oneshot_command(async move {
                        match source.metadata(&external_id).await {
                            Ok(detail) => ShowDetailCmd::DetailLoaded(detail),
                            Err(_) => ShowDetailCmd::Noop,
                        }
                    });
                }

                let external_id = show.external_id.clone();
                self.show = Some(show);

                if let Some(source) = self.source.clone() {
                    sender.oneshot_command(async move {
                        match source.children(&external_id).await {
                            Ok(items) => {
                                let seasons: Vec<MediaItem> = items
                                    .into_iter()
                                    .filter(|i| i.media_type == MediaType::Season)
                                    .collect();
                                ShowDetailCmd::SeasonsReady(seasons)
                            }
                            Err(e) => ShowDetailCmd::Error(e.to_string()),
                        }
                    });
                }
            }
            ShowDetailMsg::SeasonsLoaded(seasons) => {
                self.seasons = seasons.clone();
                self.rebuild_season_cards(&sender);

                // Update show info panel with season count
                if !seasons.is_empty() {
                    self.show_info_label
                        .set_label(&format!("{} Seasons", seasons.len()));
                    self.show_info_panel.set_visible(true);

                    self.selected_season = 0;
                    sender.input(ShowDetailMsg::SelectSeason(0));
                } else {
                    self.show_info_panel.set_visible(false);
                }
            }
            ShowDetailMsg::SelectSeason(index) => {
                let idx = index as usize;
                if idx >= self.seasons.len() {
                    return;
                }

                self.selected_season = index;
                self.update_season_card_highlight();

                let season = &self.seasons[idx];
                let external_id = season.external_id.clone();

                if let Some(source) = self.source.clone() {
                    sender.oneshot_command(async move {
                        match source.children(&external_id).await {
                            Ok(items) => {
                                let episodes: Vec<MediaItem> = items
                                    .into_iter()
                                    .filter(|i| i.media_type == MediaType::Episode)
                                    .collect();
                                ShowDetailCmd::EpisodesReady(episodes)
                            }
                            Err(e) => ShowDetailCmd::Error(e.to_string()),
                        }
                    });
                }
            }
            ShowDetailMsg::EpisodesLoaded(episodes) => {
                // Load watch progress for each episode from the shared connection.
                self.watch_progress.clear();
                if let Some(db) = &self.db {
                    let loaded: Vec<(String, WatchProgress)> = db.with(|conn| {
                        let mut repo = WatchProgressRepo::new(conn);
                        episodes
                            .iter()
                            .filter_map(|ep| {
                                repo.find_by_media_id(&ep.id)
                                    .ok()
                                    .flatten()
                                    .map(|p| (ep.id.clone(), p))
                            })
                            .collect()
                    });
                    for (id, progress) in loaded {
                        self.watch_progress.insert(id, progress);
                    }
                }

                self.rebuild_episode_cards(&episodes, &sender);
                self.episodes = episodes;
            }
            ShowDetailMsg::PlayEpisode(index) => {
                if let Some(ep) = self.episodes.get(index)
                    && let Some(ref file_path) = ep.file_path
                    && let Some(ref source) = self.source
                {
                    let url = source.playback_url(file_path);
                    info!("Playing episode: {}", ep.title);
                    let _ = sender.output(ShowDetailOutput::PlayMedia {
                        url,
                        media_item: Box::new(Some(ep.clone())),
                    });
                }
            }
            ShowDetailMsg::DownloadSeason => {
                if let Some(ref show) = self.show {
                    if self.episodes.is_empty() {
                        let _ = sender
                            .output(ShowDetailOutput::Error("No episodes to download".into()));
                    } else {
                        info!(
                            "Queuing season download for {}: {} episodes",
                            show.title,
                            self.episodes.len()
                        );
                        let _ = sender.output(ShowDetailOutput::DownloadGroup {
                            parent: Box::new(show.clone()),
                            episodes: self.episodes.clone(),
                            scope: crate::models::download::GroupScope::Season,
                        });
                    }
                }
            }
            ShowDetailMsg::LoadError(msg) => {
                let _ = sender.output(ShowDetailOutput::Error(msg));
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
            ShowDetailCmd::SeasonsReady(seasons) => {
                sender.input(ShowDetailMsg::SeasonsLoaded(seasons));
            }
            ShowDetailCmd::EpisodesReady(episodes) => {
                sender.input(ShowDetailMsg::EpisodesLoaded(episodes));
            }
            ShowDetailCmd::BackdropReady(texture) => {
                self.backdrop.set_paintable(Some(&texture));
            }
            ShowDetailCmd::PosterReady(texture) => {
                self.poster.set_paintable(Some(&texture));
                self.poster.set_visible(true);
            }
            ShowDetailCmd::DetailLoaded(detail) => {
                self.populate_enrichment(&detail, &sender);
            }
            ShowDetailCmd::CastPhotoReady(idx, texture) => {
                if let Some(child) = self.cast_box.observe_children().item(idx as u32)
                    && let Ok(card) = child.downcast::<gtk::Box>()
                    && let Some(picture) = card.first_child()
                    && let Some(inner) = picture.first_child()
                    && let Ok(picture) = inner.downcast::<gtk::Picture>()
                {
                    picture.set_paintable(Some(&texture));
                }
            }
            ShowDetailCmd::SeasonArtworkReady(idx, texture) => {
                if let Some(card) = self.season_cards_box.observe_children().item(idx as u32)
                    && let Ok(card) = card.downcast::<gtk::Box>()
                    && let Some(picture) = card.first_child()
                    && let Ok(picture) = picture.downcast::<gtk::Picture>()
                {
                    picture.set_paintable(Some(&texture));
                }
            }
            ShowDetailCmd::EpisodeThumbReady(idx, texture) => {
                // The overlay's base child is the placeholder Box; the picture is
                // an overlay child. Search the overlay's children for the Picture
                // rather than assuming first_child.
                if let Some(card) = self.episode_cards_box.observe_children().item(idx as u32)
                    && let Ok(card) = card.downcast::<gtk::Box>()
                    && let Some(overlay) = card.first_child()
                    && let Ok(overlay) = overlay.downcast::<gtk::Overlay>()
                {
                    let children = overlay.observe_children();
                    for i in 0..children.n_items() {
                        if let Some(child) = children.item(i)
                            && let Ok(picture) = child.downcast::<gtk::Picture>()
                        {
                            picture.set_paintable(Some(&texture));
                            break;
                        }
                    }
                }
            }
            ShowDetailCmd::Error(msg) => {
                sender.input(ShowDetailMsg::LoadError(msg));
            }
            ShowDetailCmd::Noop => {}
        }
    }
}

impl ShowDetail {
    fn rebuild_season_cards(&self, sender: &ComponentSender<Self>) {
        while let Some(child) = self.season_cards_box.first_child() {
            self.season_cards_box.remove(&child);
        }

        if self.seasons.is_empty() {
            self.season_section.set_visible(false);
            return;
        }
        self.season_section.set_visible(true);

        for (i, season) in self.seasons.iter().enumerate() {
            let card = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .width_request(130)
                .css_classes(["season-card"])
                .build();

            let poster_pic = gtk::Picture::builder()
                .content_fit(gtk::ContentFit::Cover)
                .width_request(130)
                .height_request(195)
                .css_classes(["season-card-poster"])
                .build();

            let name = if season.season_number == Some(0) {
                "Specials".to_string()
            } else {
                season.title.clone()
            };

            let name_label = gtk::Label::builder()
                .label(&name)
                .halign(gtk::Align::Center)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(14)
                .css_classes(["season-card-label"])
                .build();

            // Episode count hint (will be populated after season select)
            let ep_count_label = gtk::Label::builder()
                .label("")
                .halign(gtk::Align::Center)
                .css_classes(["season-card-ep-count"])
                .build();

            card.append(&poster_pic);
            card.append(&name_label);
            card.append(&ep_count_label);

            // Load season poster
            if let (Some(poster_path), Some(source), Some(cache)) =
                (&season.poster_path, &self.source, &self.artwork_cache)
            {
                let url = source.artwork_url(poster_path, 260, 390);
                let cache = Arc::clone(cache);
                sender.oneshot_command(async move {
                    match cache.get_or_download(&url).await {
                        Ok(path) => match gtk::gdk::Texture::from_filename(&path) {
                            Ok(tex) => ShowDetailCmd::SeasonArtworkReady(i, tex),
                            Err(_) => ShowDetailCmd::Noop,
                        },
                        Err(_) => ShowDetailCmd::Noop,
                    }
                });
            }

            let click = gtk::GestureClick::new();
            let sender_clone = sender.input_sender().clone();
            let idx = i;
            click.connect_pressed(move |_, _, _, _| {
                let _ = sender_clone.send(ShowDetailMsg::SelectSeason(idx as u32));
            });
            card.add_controller(click);

            self.season_cards_box.append(&card);
        }

        self.update_season_card_highlight();
    }

    fn update_season_card_highlight(&self) {
        let children = self.season_cards_box.observe_children();
        for i in 0..children.n_items() {
            if let Some(card) = children.item(i)
                && let Ok(card) = card.downcast::<gtk::Box>()
            {
                let class_refs: Vec<&str> = if i == self.selected_season {
                    vec!["season-card", "season-card-selected"]
                } else {
                    vec!["season-card"]
                };
                card.set_css_classes(&class_refs);
            }
        }
    }

    /// Build enhanced horizontal episode cards with overview, air date, and watch progress.
    #[allow(clippy::too_many_lines)]
    fn rebuild_episode_cards(&self, episodes: &[MediaItem], sender: &ComponentSender<Self>) {
        while let Some(child) = self.episode_cards_box.first_child() {
            self.episode_cards_box.remove(&child);
        }

        if episodes.is_empty() {
            self.episode_section.set_visible(false);
            return;
        }
        self.episode_section.set_visible(true);

        // Update season card episode counts
        let season_children = self.season_cards_box.observe_children();
        for i in 0..season_children.n_items() {
            if i == self.selected_season
                && let Some(card) = season_children.item(i)
                && let Ok(card) = card.downcast::<gtk::Box>()
            {
                // The ep count label is the 3rd child (poster, name, count)
                if let Some(count_label) = card.observe_children().item(2)
                    && let Ok(count_label) = count_label.downcast::<gtk::Label>()
                {
                    let label = if episodes.len() == 1 {
                        "1 episode".to_string()
                    } else {
                        format!("{} episodes", episodes.len())
                    };
                    count_label.set_label(&label);
                }
            }
        }

        for (i, ep) in episodes.iter().enumerate() {
            let ep_num = ep.episode_number.unwrap_or(0);

            // Card: vertical box containing thumbnail overlay + text + progress
            let card = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .width_request(290)
                .css_classes(["episode-card"])
                .build();

            // Thumbnail: picture over a placeholder backing. The overlay's base
            // child is the placeholder (dark fill + centered icon shown when no
            // art is available); the picture layers on top and covers it once a
            // texture loads. The number badge floats in the top-left corner.
            let thumb_overlay = gtk::Overlay::new();

            let thumb_placeholder = gtk::Box::builder()
                .width_request(290)
                .height_request(163)
                .css_classes(["episode-card-thumb"])
                .build();
            let placeholder_icon = gtk::Image::builder()
                .icon_name("video-display-symbolic")
                .pixel_size(32)
                .hexpand(true)
                .vexpand(true)
                .halign(gtk::Align::Center)
                .valign(gtk::Align::Center)
                .css_classes(["episode-card-thumb-placeholder"])
                .build();
            thumb_placeholder.append(&placeholder_icon);

            let thumb_picture = gtk::Picture::builder()
                .content_fit(gtk::ContentFit::Cover)
                .width_request(290)
                .height_request(163)
                .css_classes(["episode-card-thumb-img"])
                .build();

            let ep_badge = gtk::Label::builder()
                .label(format!("{ep_num}"))
                .css_classes(["episode-number-badge"])
                .halign(gtk::Align::Start)
                .valign(gtk::Align::Start)
                .margin_start(6)
                .margin_top(6)
                .build();

            thumb_overlay.set_child(Some(&thumb_placeholder));
            thumb_overlay.add_overlay(&thumb_picture);
            thumb_overlay.add_overlay(&ep_badge);
            card.append(&thumb_overlay);

            // Title below thumbnail
            let title_label = gtk::Label::builder()
                .label(&ep.title)
                .halign(gtk::Align::Start)
                .wrap(true)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(30)
                .css_classes(["episode-card-title"])
                .build();
            card.append(&title_label);

            // Episode number + air date + runtime
            let mut meta_parts = Vec::new();
            meta_parts.push(format!("Episode {ep_num}"));
            if let Some(ref air_date) = ep.air_date {
                meta_parts.push(air_date.clone());
            }
            if let Some(ref runtime) = ep.format_runtime() {
                meta_parts.push(runtime.clone());
            }
            let meta_text = meta_parts.join(" · ");
            let meta_label = gtk::Label::builder()
                .label(&meta_text)
                .halign(gtk::Align::Start)
                .css_classes(["episode-card-meta"])
                .build();
            card.append(&meta_label);

            // Overview snippet
            if let Some(ref overview) = ep.overview {
                let overview_label = gtk::Label::builder()
                    .label(overview)
                    .halign(gtk::Align::Start)
                    .wrap(true)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .max_width_chars(38)
                    .lines(3)
                    .css_classes(["episode-card-overview"])
                    .build();
                card.append(&overview_label);
            }

            // Watch progress indicator
            if let Some(progress) = self.watch_progress.get(&ep.id) {
                if progress.watched {
                    let watched_label = gtk::Label::builder()
                        .label("✓ Watched")
                        .halign(gtk::Align::Start)
                        .css_classes(["episode-card-watched"])
                        .build();
                    card.append(&watched_label);
                } else if progress.position_seconds > 0.0 {
                    let fraction = progress.progress_fraction();
                    let progress_bar = gtk::ProgressBar::builder()
                        .fraction(fraction)
                        .show_text(false)
                        .css_classes(["episode-card-progress-bar"])
                        .build();
                    card.append(&progress_bar);
                }
            }

            // Load thumbnail
            if let (Some(thumb_path), Some(source), Some(cache)) =
                (&ep.poster_path, &self.source, &self.artwork_cache)
            {
                let url = source.artwork_url(thumb_path, 580, 326);
                let cache = Arc::clone(cache);
                let idx = i;
                sender.oneshot_command(async move {
                    match cache.get_or_download(&url).await {
                        Ok(path) => match gtk::gdk::Texture::from_filename(&path) {
                            Ok(tex) => ShowDetailCmd::EpisodeThumbReady(idx, tex),
                            Err(_) => ShowDetailCmd::Noop,
                        },
                        Err(_) => ShowDetailCmd::Noop,
                    }
                });
            }

            // Click to play (only if file available)
            if ep.file_path.is_some() {
                let click = gtk::GestureClick::new();
                let sender_clone = sender.input_sender().clone();
                let idx = i;
                click.connect_pressed(move |_, _, _, _| {
                    let _ = sender_clone.send(ShowDetailMsg::PlayEpisode(idx));
                });
                card.add_controller(click);
            } else {
                card.set_opacity(0.5);
            }

            self.episode_cards_box.append(&card);
        }
    }

    /// Populate enriched detail sections from a MediaDetail response.
    #[allow(clippy::too_many_lines)]
    fn populate_enrichment(&self, detail: &MediaDetail, sender: &ComponentSender<Self>) {
        // Directors
        if !detail.directors.is_empty() {
            let text = format!("Created by {}", detail.directors.join(", "));
            self.director_label.set_label(&text);
            self.director_label.set_visible(true);
        }

        // Writers
        if !detail.writers.is_empty() {
            let text = format!("Written by {}", detail.writers.join(", "));
            self.writer_label.set_label(&text);
            self.writer_label.set_visible(true);
        }

        // Cast
        if !detail.cast.is_empty() {
            while let Some(child) = self.cast_box.first_child() {
                self.cast_box.remove(&child);
            }

            for (idx, member) in detail.cast.iter().enumerate() {
                let card = gtk::Box::builder()
                    .orientation(gtk::Orientation::Vertical)
                    .css_classes(["cast-card"])
                    .build();

                let inner = gtk::Box::builder()
                    .orientation(gtk::Orientation::Vertical)
                    .spacing(4)
                    .width_request(80)
                    .build();

                let picture = gtk::Picture::builder()
                    .content_fit(gtk::ContentFit::Cover)
                    .width_request(72)
                    .height_request(72)
                    .css_classes(["cast-photo"])
                    .build();

                let name_label = gtk::Label::builder()
                    .label(&member.name)
                    .halign(gtk::Align::Center)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .max_width_chars(12)
                    .css_classes(["caption"])
                    .build();

                inner.append(&picture);
                inner.append(&name_label);

                if let Some(ref character) = member.character {
                    let char_label = gtk::Label::builder()
                        .label(character)
                        .halign(gtk::Align::Center)
                        .ellipsize(gtk::pango::EllipsizeMode::End)
                        .max_width_chars(12)
                        .css_classes(["caption", "dim-label"])
                        .build();
                    inner.append(&char_label);
                }

                card.append(&inner);
                self.cast_box.append(&card);

                if let (Some(photo_path), Some(source), Some(cache)) =
                    (&member.photo_path, &self.source, &self.artwork_cache)
                {
                    let url = source.artwork_url(photo_path, 80, 80);
                    let cache = Arc::clone(cache);
                    sender.oneshot_command(async move {
                        match cache.get_or_download(&url).await {
                            Ok(path) => match gtk::gdk::Texture::from_filename(&path) {
                                Ok(tex) => ShowDetailCmd::CastPhotoReady(idx, tex),
                                Err(_) => ShowDetailCmd::Noop,
                            },
                            Err(_) => ShowDetailCmd::Noop,
                        }
                    });
                }
            }

            self.cast_section.set_visible(true);
        }

        // Technical info (frosted panel)
        if let Some(ref tech) = detail.technical {
            let mut parts: Vec<String> = Vec::new();
            if let Some(res) = tech.display_resolution() {
                parts.push(res);
            }
            if let Some(ref codec) = tech.video_codec {
                parts.push(codec.to_uppercase());
            }
            if let Some(ref audio) = tech.audio_codec {
                let channels = tech
                    .display_audio_channels()
                    .map(|ch| format!(" {ch}"))
                    .unwrap_or_default();
                parts.push(format!("{}{channels}", audio.to_uppercase()));
            }
            if let Some(ref container) = tech.container {
                parts.push(container.to_uppercase());
            }
            if let Some(size) = tech.display_file_size() {
                parts.push(size);
            }

            if !parts.is_empty() {
                self.tech_label
                    .set_label(&format!("Technical Details\n{}", parts.join(" · ")));
                self.tech_panel.set_visible(true);
            }
        }

        // Collections (frosted panel)
        if !detail.collections.is_empty() {
            while let Some(child) = self.collections_box.first_child() {
                self.collections_box.remove(&child);
            }

            let prefix = gtk::Label::builder()
                .label("Part of:")
                .css_classes(["dim-label"])
                .margin_end(6)
                .build();
            self.collections_box.append(&prefix);

            for col in &detail.collections {
                let label = gtk::Label::builder()
                    .label(&col.name)
                    .css_classes(["caption"])
                    .build();
                self.collections_box.append(&label);
            }

            self.collections_panel.set_visible(true);
        }
    }
}
