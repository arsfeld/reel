use std::sync::Arc;

use gtk::prelude::*;
use relm4::prelude::*;
use tracing::info;

use crate::models::detail::MediaDetail;
use crate::models::media::MediaItem;
use crate::services::artwork::ArtworkCache;
use crate::services::download::download_eligible;
use crate::services::media_source::MediaSource;

#[allow(dead_code)]
pub struct MovieDetail {
    item: Option<MediaItem>,
    source: Option<Arc<dyn MediaSource>>,
    artwork_cache: Option<Arc<ArtworkCache>>,
    // Widgets
    title_label: gtk::Label,
    meta_box: gtk::Box,
    year_label: gtk::Label,
    runtime_label: gtk::Label,
    rating_label: gtk::Label,
    content_rating_label: gtk::Label,
    director_label: gtk::Label,
    writer_label: gtk::Label,
    genres_box: gtk::FlowBox,
    overview_label: gtk::Label,
    play_button: gtk::Button,
    download_button: gtk::Button,
    downloaded_badge: gtk::Label,
    backdrop: gtk::Picture,
    poster: gtk::Picture,
    // Enriched sections
    cast_section: gtk::Box,
    cast_scroll: gtk::ScrolledWindow,
    cast_box: gtk::Box,
    tech_label: gtk::Label,
    tech_panel: gtk::Box,
    collections_box: gtk::Box,
    collections_panel: gtk::Box,
}

#[allow(clippy::large_enum_variant)]
pub enum MovieDetailMsg {
    LoadMovie(MediaItem),
    SetSource(Arc<dyn MediaSource>, Arc<ArtworkCache>),
    Play,
    Download,
    /// Reflect whether a completed local download exists for the shown item.
    SetDownloaded(bool),
}

impl std::fmt::Debug for MovieDetailMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadMovie(item) => write!(f, "LoadMovie({})", item.title),
            Self::SetSource(..) => write!(f, "SetSource(..)"),
            Self::Play => write!(f, "Play"),
            Self::Download => write!(f, "Download"),
            Self::SetDownloaded(v) => write!(f, "SetDownloaded({v})"),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum MovieDetailOutput {
    PlayMedia {
        url: String,
        media_item: Box<Option<crate::models::media::MediaItem>>,
    },
    /// Enqueue this movie for offline download.
    DownloadMedia(Box<MediaItem>),
    Error(String),
}

#[allow(clippy::large_enum_variant)]
pub enum MovieDetailCmd {
    BackdropReady(gtk::gdk::Texture),
    PosterReady(gtk::gdk::Texture),
    DetailLoaded(MediaDetail),
    CastPhotoReady(usize, gtk::gdk::Texture),
    Noop,
}

impl std::fmt::Debug for MovieDetailCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MovieDetailCmd")
    }
}

#[relm4::component(pub)]
impl Component for MovieDetail {
    type Init = ();
    type Input = MovieDetailMsg;
    type Output = MovieDetailOutput;
    type CommandOutput = MovieDetailCmd;

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

        // Backdrop image (base layer of the overlay)
        let backdrop = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Cover)
            .css_classes(["detail-hero"])
            .vexpand(true)
            .hexpand(true)
            .build();

        // Gradient scrim for text readability over the backdrop
        let hero_gradient = gtk::Box::builder()
            .css_classes(["detail-hero-overlay"])
            .vexpand(true)
            .hexpand(true)
            .build();

        // Headline: poster + text column, anchored to the bottom of the hero and
        // clamped to match the content width below it.
        let headline_clamp = adw::Clamp::builder()
            .maximum_size(1400)
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

        // Poster floats at bottom-left of the hero. Hidden until art loads; when
        // hidden GTK4 skips it in box layout so the text column reclaims the room.
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
        let runtime_label = gtk::Label::builder()
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
        meta_box.append(&runtime_label);
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
            let _ = sender_play.send(MovieDetailMsg::Play);
        });

        let download_button = gtk::Button::builder()
            .label("⤓  Download")
            .css_classes(["pill"])
            .halign(gtk::Align::Start)
            .build();
        let sender_dl = sender.input_sender().clone();
        download_button.connect_clicked(move |_| {
            let _ = sender_dl.send(MovieDetailMsg::Download);
        });

        let downloaded_badge = gtk::Label::builder()
            .label("✓ Downloaded")
            .css_classes(["pill", "success"])
            .valign(gtk::Align::Center)
            .visible(false)
            .build();

        actions_row.append(&play_button);
        actions_row.append(&download_button);
        actions_row.append(&downloaded_badge);

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

        let clamp = adw::Clamp::builder().maximum_size(1400).build();
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
            item: None,
            source: None,
            artwork_cache: None,
            title_label,
            meta_box,
            year_label,
            runtime_label,
            rating_label,
            content_rating_label,
            director_label,
            writer_label,
            genres_box,
            overview_label,
            play_button,
            download_button,
            downloaded_badge,
            backdrop,
            poster,
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
            MovieDetailMsg::SetSource(source, artwork_cache) => {
                self.source = Some(source);
                self.artwork_cache = Some(artwork_cache);
            }
            MovieDetailMsg::LoadMovie(item) => {
                info!("Loading movie detail: {}", item.title);

                self.title_label.set_label(&item.display_title());

                // Metadata badges (visibility toggled; children are permanent)

                if let Some(year) = item.year {
                    self.year_label.set_label(&year.to_string());
                    self.year_label.set_visible(true);
                } else {
                    self.year_label.set_visible(false);
                }

                if let Some(ref runtime) = item.format_runtime() {
                    self.runtime_label.set_label(runtime);
                    self.runtime_label.set_visible(true);
                } else {
                    self.runtime_label.set_visible(false);
                }

                if let Some(rating) = item.rating {
                    self.rating_label.set_label(&format!("★ {rating:.1}"));
                    self.rating_label.set_visible(true);
                } else {
                    self.rating_label.set_visible(false);
                }

                if let Some(ref cr) = item.content_rating {
                    self.content_rating_label.set_label(cr);
                    self.content_rating_label.set_visible(true);
                } else {
                    self.content_rating_label.set_visible(false);
                }

                // Genre chips
                while let Some(child) = self.genres_box.first_child() {
                    self.genres_box.remove(&child);
                }
                for genre in &item.genres {
                    let label = gtk::Label::builder()
                        .label(genre)
                        .css_classes(["genre-chip"])
                        .build();
                    self.genres_box.insert(&label, -1);
                }

                // Overview
                if let Some(ref overview) = item.overview {
                    self.overview_label.set_label(overview);
                    self.overview_label.set_visible(true);
                } else {
                    self.overview_label.set_visible(false);
                }

                self.play_button.set_sensitive(item.file_path.is_some());
                // Gate the Download action on eligibility (remote source with a
                // part key; hidden for local sources). Badge is hidden until the
                // app reports an existing completed download (SetDownloaded).
                let eligible = download_eligible(item.source_type, item.file_path.as_deref());
                self.download_button.set_visible(eligible);
                self.download_button.set_sensitive(eligible);
                self.downloaded_badge.set_visible(false);

                // Reset enriched sections while loading
                self.director_label.set_visible(false);
                self.writer_label.set_visible(false);
                self.cast_section.set_visible(false);
                self.tech_panel.set_visible(false);
                self.collections_panel.set_visible(false);

                // Reset poster
                self.poster.set_visible(false);

                // Clear any previous item's backdrop so it doesn't linger while
                // the new one loads (or stays empty when the new item has none).
                self.backdrop.set_paintable(None::<&gtk::gdk::Texture>);
                if item.backdrop_path.is_none() {
                    self.backdrop.add_css_class("detail-hero-empty");
                } else {
                    self.backdrop.remove_css_class("detail-hero-empty");
                }

                // Load backdrop (prefer backdrop, fall back to poster for hero)
                if let (Some(source), Some(cache)) = (&self.source, &self.artwork_cache) {
                    // Backdrop
                    if let Some(art_path) = &item.backdrop_path {
                        let url = source.artwork_url(art_path, 1280, 420);
                        let cache = Arc::clone(cache);
                        sender.oneshot_command(async move {
                            match cache.get_or_download(&url).await {
                                Ok(path) => match gtk::gdk::Texture::from_filename(&path) {
                                    Ok(tex) => MovieDetailCmd::BackdropReady(tex),
                                    Err(e) => {
                                        tracing::debug!("Failed to load backdrop: {e}");
                                        MovieDetailCmd::Noop
                                    }
                                },
                                Err(e) => {
                                    tracing::debug!("Failed to download backdrop: {e}");
                                    MovieDetailCmd::Noop
                                }
                            }
                        });
                    }

                    // Poster
                    if let Some(poster_path) = &item.poster_path {
                        let url = source.artwork_url(poster_path, 340, 510);
                        let cache = Arc::clone(cache);
                        sender.oneshot_command(async move {
                            match cache.get_or_download(&url).await {
                                Ok(path) => match gtk::gdk::Texture::from_filename(&path) {
                                    Ok(tex) => MovieDetailCmd::PosterReady(tex),
                                    Err(e) => {
                                        tracing::debug!("Failed to load poster: {e}");
                                        MovieDetailCmd::Noop
                                    }
                                },
                                Err(e) => {
                                    tracing::debug!("Failed to download poster: {e}");
                                    MovieDetailCmd::Noop
                                }
                            }
                        });
                    }
                }

                // Fetch enriched metadata (cast, crew, technical info)
                if let Some(source) = self.source.clone() {
                    let external_id = item.external_id.clone();
                    sender.oneshot_command(async move {
                        match source.metadata(&external_id).await {
                            Ok(detail) => MovieDetailCmd::DetailLoaded(detail),
                            Err(e) => {
                                tracing::debug!("Failed to fetch movie detail: {e}");
                                MovieDetailCmd::Noop
                            }
                        }
                    });
                }

                self.item = Some(item);
            }
            MovieDetailMsg::Play => {
                if let Some(ref item) = self.item
                    && let Some(ref file_path) = item.file_path
                    && let Some(ref source) = self.source
                {
                    let url = source.playback_url(file_path);
                    info!("Playing: {}", item.title);
                    let _ = sender.output(MovieDetailOutput::PlayMedia {
                        url,
                        media_item: Box::new(Some(item.clone())),
                    });
                }
            }
            MovieDetailMsg::Download => {
                if let Some(ref item) = self.item
                    && download_eligible(item.source_type, item.file_path.as_deref())
                {
                    info!("Queuing download: {}", item.title);
                    let _ = sender.output(MovieDetailOutput::DownloadMedia(Box::new(item.clone())));
                    // Optimistic: the queue runs; the badge flips on completion
                    // when the app re-reports state.
                    self.download_button.set_sensitive(false);
                }
            }
            MovieDetailMsg::SetDownloaded(downloaded) => {
                self.downloaded_badge.set_visible(downloaded);
                // A completed local copy: no point offering Download again.
                self.download_button.set_visible(!downloaded);
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
            MovieDetailCmd::BackdropReady(texture) => {
                self.backdrop.set_paintable(Some(&texture));
            }
            MovieDetailCmd::PosterReady(texture) => {
                self.poster.set_paintable(Some(&texture));
                self.poster.set_visible(true);
            }
            MovieDetailCmd::DetailLoaded(detail) => {
                self.populate_enrichment(&detail, &sender);
            }
            MovieDetailCmd::CastPhotoReady(idx, texture) => {
                if let Some(child) = self.cast_box.observe_children().item(idx as u32)
                    && let Ok(card) = child.downcast::<gtk::Box>()
                    && let Some(picture) = card.first_child()
                {
                    // Navigate: cast-card box → inner box → picture
                    if let Some(inner) = picture.first_child()
                        && let Ok(picture) = inner.downcast::<gtk::Picture>()
                    {
                        picture.set_paintable(Some(&texture));
                    }
                }
            }
            MovieDetailCmd::Noop => {}
        }
    }
}

impl MovieDetail {
    /// Populate enriched detail sections from a MediaDetail response.
    #[allow(clippy::too_many_lines)]
    fn populate_enrichment(&self, detail: &MediaDetail, sender: &ComponentSender<Self>) {
        // Directors
        if !detail.directors.is_empty() {
            let text = format!("Directed by {}", detail.directors.join(", "));
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
                // Outer glass card wrapper
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

                // Load cast photo asynchronously
                if let (Some(photo_path), Some(source), Some(cache)) =
                    (&member.photo_path, &self.source, &self.artwork_cache)
                {
                    let url = source.artwork_url(photo_path, 80, 80);
                    let cache = Arc::clone(cache);
                    sender.oneshot_command(async move {
                        match cache.get_or_download(&url).await {
                            Ok(path) => match gtk::gdk::Texture::from_filename(&path) {
                                Ok(tex) => MovieDetailCmd::CastPhotoReady(idx, tex),
                                Err(_) => MovieDetailCmd::Noop,
                            },
                            Err(_) => MovieDetailCmd::Noop,
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
