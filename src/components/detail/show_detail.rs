use std::sync::Arc;

use adw;
use adw::prelude::*;
use relm4::prelude::*;
use tracing::info;

use crate::models::media::{MediaItem, MediaType};
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
    // Widgets
    title_label: gtk::Label,
    meta_box: gtk::Box,
    year_label: gtk::Label,
    rating_label: gtk::Label,
    content_rating_label: gtk::Label,
    overview_label: gtk::Label,
    backdrop: gtk::Picture,
    poster: gtk::Picture,
    poster_spacer: gtk::Box,
    // Season cards
    season_scroll: gtk::ScrolledWindow,
    season_cards_box: gtk::Box,
    season_section: gtk::Box,
    // Episode cards (horizontal row)
    episode_section: gtk::Box,
    episode_scroll: gtk::ScrolledWindow,
    episode_cards_box: gtk::Box,
}

#[allow(clippy::large_enum_variant)]
pub enum ShowDetailMsg {
    LoadShow(MediaItem),
    SetSource(Arc<dyn MediaSource>, Arc<ArtworkCache>),
    SelectSeason(u32),
    PlayEpisode(usize),
    SeasonsLoaded(Vec<MediaItem>),
    EpisodesLoaded(Vec<MediaItem>),
    LoadError(String),
}

impl std::fmt::Debug for ShowDetailMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadShow(item) => write!(f, "LoadShow({})", item.title),
            Self::SetSource(..) => write!(f, "SetSource(..)"),
            Self::SelectSeason(idx) => write!(f, "SelectSeason({idx})"),
            Self::PlayEpisode(idx) => write!(f, "PlayEpisode({idx})"),
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
    Error(String),
}

pub enum ShowDetailCmd {
    SeasonsReady(Vec<MediaItem>),
    EpisodesReady(Vec<MediaItem>),
    BackdropReady(gtk::gdk::Texture),
    PosterReady(gtk::gdk::Texture),
    SeasonArtworkReady(usize, gtk::gdk::Texture),
    EpisodeThumbReady(usize, gtk::gdk::Texture),
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
        _sender: ComponentSender<Self>,
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

        // ═══ HERO: backdrop + gradient overlay + floating poster ═══

        let hero_overlay = gtk::Overlay::builder()
            .height_request(400)
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

        // Poster art — clamped to match content width, floats at bottom-left overlapping hero edge
        let poster_clamp = adw::Clamp::builder()
            .maximum_size(960)
            .valign(gtk::Align::End)
            .build();
        let poster = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .width_request(170)
            .height_request(255)
            .css_classes(["detail-poster-hero"])
            .halign(gtk::Align::Start)
            .valign(gtk::Align::End)
            .margin_start(28)
            .visible(false)
            .build();
        poster_clamp.set_child(Some(&poster));

        hero_overlay.add_overlay(&backdrop);
        hero_overlay.add_overlay(&hero_gradient);
        hero_overlay.add_overlay(&poster_clamp);

        main_box.append(&hero_overlay);

        // ═══ Clamped content below hero ═══

        let clamp = adw::Clamp::builder().maximum_size(960).build();
        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_start(20)
            .margin_end(20)
            .margin_top(16)
            .margin_bottom(32)
            .build();

        // ═══ Title + metadata (indented for poster) ═══

        let title_meta_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(0)
            .build();

        let poster_spacer = gtk::Box::builder()
            .width_request(198)
            .visible(false)
            .build();

        let title_meta_content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .hexpand(true)
            .build();

        let title_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .wrap(true)
            .css_classes(["title-1"])
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

        title_meta_content.append(&title_label);
        title_meta_content.append(&meta_box);

        title_meta_row.append(&poster_spacer);
        title_meta_row.append(&title_meta_content);
        content_box.append(&title_meta_row);

        // ═══ Overview ═══

        let overview_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .wrap(true)
            .visible(false)
            .build();
        content_box.append(&overview_label);

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
            .height_request(190)
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
            .height_request(210)
            .build();

        let episode_cards_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();

        episode_scroll.set_child(Some(&episode_cards_box));
        episode_section.append(&episode_heading);
        episode_section.append(&episode_scroll);
        content_box.append(&episode_section);

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
            title_label,
            meta_box,
            year_label,
            rating_label,
            content_rating_label,
            overview_label,
            backdrop,
            poster,
            poster_spacer,
            season_scroll,
            season_cards_box,
            season_section,
            episode_section,
            episode_scroll,
            episode_cards_box,
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

                if let Some(ref overview) = show.overview {
                    self.overview_label.set_label(overview);
                    self.overview_label.set_visible(true);
                } else {
                    self.overview_label.set_visible(false);
                }

                self.poster.set_visible(false);
                self.poster_spacer.set_visible(false);
                self.episode_section.set_visible(false);

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

                if !seasons.is_empty() {
                    self.selected_season = 0;
                    sender.input(ShowDetailMsg::SelectSeason(0));
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
                self.poster_spacer.set_visible(true);
            }
            ShowDetailCmd::SeasonArtworkReady(idx, texture) => {
                // Fix: picture is the direct first child of the card box
                if let Some(card) = self.season_cards_box.observe_children().item(idx as u32)
                    && let Ok(card) = card.downcast::<gtk::Box>()
                    && let Some(picture) = card.first_child()
                    && let Ok(picture) = picture.downcast::<gtk::Picture>()
                {
                    picture.set_paintable(Some(&texture));
                }
            }
            ShowDetailCmd::EpisodeThumbReady(idx, texture) => {
                // Find the episode card at index and set its overlay thumbnail
                if let Some(card) = self.episode_cards_box.observe_children().item(idx as u32)
                    && let Ok(card) = card.downcast::<gtk::Box>()
                {
                    // Card structure: overlay → picture (first overlay child)
                    if let Some(overlay) = card.first_child()
                        && let Ok(overlay) = overlay.downcast::<gtk::Overlay>()
                        && let Some(picture) = overlay.first_child()
                        && let Ok(picture) = picture.downcast::<gtk::Picture>()
                    {
                        picture.set_paintable(Some(&texture));
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
                .width_request(100)
                .css_classes(["season-card"])
                .build();

            let poster_pic = gtk::Picture::builder()
                .content_fit(gtk::ContentFit::Cover)
                .width_request(100)
                .height_request(150)
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
                .max_width_chars(12)
                .css_classes(["season-card-label"])
                .build();

            card.append(&poster_pic);
            card.append(&name_label);

            // Load season poster
            if let (Some(poster_path), Some(source), Some(cache)) =
                (&season.poster_path, &self.source, &self.artwork_cache)
            {
                let url = source.artwork_url(poster_path, 200, 300);
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

    /// Build horizontal scrollable episode cards — Infuse-style wide thumbnails.
    fn rebuild_episode_cards(&self, episodes: &[MediaItem], sender: &ComponentSender<Self>) {
        while let Some(child) = self.episode_cards_box.first_child() {
            self.episode_cards_box.remove(&child);
        }

        if episodes.is_empty() {
            self.episode_section.set_visible(false);
            return;
        }
        self.episode_section.set_visible(true);

        for (i, ep) in episodes.iter().enumerate() {
            let ep_num = ep.episode_number.unwrap_or(0);

            // Card: vertical box containing thumbnail overlay + text
            let card = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .width_request(240)
                .css_classes(["episode-card"])
                .build();

            // Thumbnail overlay: picture + episode number badge
            let thumb_overlay = gtk::Overlay::new();

            let thumb_picture = gtk::Picture::builder()
                .content_fit(gtk::ContentFit::Cover)
                .width_request(240)
                .height_request(135)
                .css_classes(["episode-card-thumb"])
                .build();

            let ep_badge = gtk::Label::builder()
                .label(format!("{ep_num}"))
                .css_classes(["episode-number-badge"])
                .halign(gtk::Align::Start)
                .valign(gtk::Align::End)
                .build();

            thumb_overlay.add_overlay(&thumb_picture);
            thumb_overlay.add_overlay(&ep_badge);
            card.append(&thumb_overlay);

            // Title below thumbnail
            let title_label = gtk::Label::builder()
                .label(&ep.title)
                .halign(gtk::Align::Start)
                .wrap(true)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(28)
                .css_classes(["episode-card-title"])
                .build();
            card.append(&title_label);

            // Episode number + runtime
            let mut meta_str = format!("Episode {ep_num}");
            if let Some(ref runtime) = ep.format_runtime() {
                meta_str.push_str(&format!(" · {runtime}"));
            }
            let meta_label = gtk::Label::builder()
                .label(&meta_str)
                .halign(gtk::Align::Start)
                .css_classes(["dim-label", "caption"])
                .build();
            card.append(&meta_label);

            // Load thumbnail
            if let (Some(thumb_path), Some(source), Some(cache)) =
                (&ep.poster_path, &self.source, &self.artwork_cache)
            {
                let url = source.artwork_url(thumb_path, 480, 270);
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
}
