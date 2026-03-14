use std::sync::Arc;

use gtk4::prelude::*;
use libadwaita as adw;
use relm4::prelude::*;
use tracing::info;

use crate::models::detail::MediaDetail;
use crate::models::media::MediaItem;
use crate::services::artwork::ArtworkCache;
use crate::services::media_source::MediaSource;

#[allow(dead_code)]
pub struct MovieDetail {
    item: Option<MediaItem>,
    source: Option<Arc<dyn MediaSource>>,
    artwork_cache: Option<Arc<ArtworkCache>>,
    // Widgets we need to update
    title_label: gtk4::Label,
    year_label: gtk4::Label,
    runtime_label: gtk4::Label,
    rating_label: gtk4::Label,
    content_rating_label: gtk4::Label,
    director_label: gtk4::Label,
    writer_label: gtk4::Label,
    genres_box: gtk4::FlowBox,
    overview_label: gtk4::Label,
    play_button: gtk4::Button,
    backdrop: gtk4::Picture,
    // Enriched sections
    cast_section: gtk4::Box,
    cast_scroll: gtk4::ScrolledWindow,
    cast_box: gtk4::Box,
    tech_label: gtk4::Label,
    collections_box: gtk4::Box,
}

#[allow(clippy::large_enum_variant)]
pub enum MovieDetailMsg {
    LoadMovie(MediaItem),
    SetSource(Arc<dyn MediaSource>, Arc<ArtworkCache>),
    Play,
}

impl std::fmt::Debug for MovieDetailMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadMovie(item) => write!(f, "LoadMovie({})", item.title),
            Self::SetSource(..) => write!(f, "SetSource(..)"),
            Self::Play => write!(f, "Play"),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum MovieDetailOutput {
    PlayMedia {
        url: String,
        media_item: Option<crate::models::media::MediaItem>,
    },
    Error(String),
}

#[allow(clippy::large_enum_variant)]
pub enum MovieDetailCmd {
    BackdropReady(gtk4::gdk::Texture),
    DetailLoaded(MediaDetail),
    CastPhotoReady(usize, gtk4::gdk::Texture),
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

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());

        let scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .build();

        let clamp = adw::Clamp::builder().maximum_size(900).build();
        let content_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(16)
            .margin_start(16)
            .margin_end(16)
            .margin_top(16)
            .margin_bottom(16)
            .build();

        let backdrop = gtk4::Picture::builder()
            .content_fit(gtk4::ContentFit::Cover)
            .height_request(360)
            .css_classes(["media-backdrop"])
            .build();

        let title_label = gtk4::Label::builder()
            .halign(gtk4::Align::Start)
            .wrap(true)
            .css_classes(["title-1"])
            .build();

        let meta_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .halign(gtk4::Align::Start)
            .build();

        let year_label = gtk4::Label::builder()
            .css_classes(["dim-label"])
            .visible(false)
            .build();
        let runtime_label = gtk4::Label::builder()
            .css_classes(["dim-label"])
            .visible(false)
            .build();
        let rating_label = gtk4::Label::builder()
            .css_classes(["dim-label"])
            .visible(false)
            .build();
        let content_rating_label = gtk4::Label::builder()
            .css_classes(["dim-label"])
            .visible(false)
            .build();

        meta_box.append(&year_label);
        meta_box.append(&runtime_label);
        meta_box.append(&rating_label);
        meta_box.append(&content_rating_label);

        // Credits labels (populated when MediaDetail arrives)
        let director_label = gtk4::Label::builder()
            .halign(gtk4::Align::Start)
            .wrap(true)
            .css_classes(["dim-label"])
            .visible(false)
            .build();
        let writer_label = gtk4::Label::builder()
            .halign(gtk4::Align::Start)
            .wrap(true)
            .css_classes(["dim-label"])
            .visible(false)
            .build();

        let genres_box = gtk4::FlowBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .halign(gtk4::Align::Start)
            .max_children_per_line(10)
            .build();

        let play_button = gtk4::Button::builder()
            .label("Play")
            .css_classes(["suggested-action", "pill"])
            .halign(gtk4::Align::Start)
            .margin_top(8)
            .build();

        let sender_play = sender.input_sender().clone();
        play_button.connect_clicked(move |_| {
            let _ = sender_play.send(MovieDetailMsg::Play);
        });

        let overview_label = gtk4::Label::builder()
            .halign(gtk4::Align::Start)
            .wrap(true)
            .margin_top(8)
            .visible(false)
            .build();

        // Cast section (populated when MediaDetail arrives)
        let cast_section = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(8)
            .visible(false)
            .build();
        let cast_heading = gtk4::Label::builder()
            .label("Cast")
            .halign(gtk4::Align::Start)
            .css_classes(["title-4"])
            .build();
        let cast_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Never)
            .height_request(130)
            .build();
        let cast_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .build();
        cast_scroll.set_child(Some(&cast_box));
        cast_section.append(&cast_heading);
        cast_section.append(&cast_scroll);

        // Technical info label (populated when MediaDetail arrives)
        let tech_label = gtk4::Label::builder()
            .halign(gtk4::Align::Start)
            .wrap(true)
            .css_classes(["dim-label", "caption"])
            .visible(false)
            .build();

        // Collections section (populated when MediaDetail arrives)
        let collections_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .halign(gtk4::Align::Start)
            .visible(false)
            .build();

        content_box.append(&backdrop);
        content_box.append(&title_label);
        content_box.append(&meta_box);
        content_box.append(&director_label);
        content_box.append(&writer_label);
        content_box.append(&genres_box);
        content_box.append(&play_button);
        content_box.append(&overview_label);
        content_box.append(&cast_section);
        content_box.append(&tech_label);
        content_box.append(&collections_box);

        clamp.set_child(Some(&content_box));
        scrolled.set_child(Some(&clamp));
        toolbar.set_content(Some(&scrolled));
        root.append(&toolbar);

        let model = Self {
            item: None,
            source: None,
            artwork_cache: None,
            title_label,
            year_label,
            runtime_label,
            rating_label,
            content_rating_label,
            director_label,
            writer_label,
            genres_box,
            overview_label,
            play_button,
            backdrop,
            cast_section,
            cast_scroll,
            cast_box,
            tech_label,
            collections_box,
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
                    self.rating_label.set_label(&format!("{rating:.1}"));
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

                // Genres
                while let Some(child) = self.genres_box.first_child() {
                    self.genres_box.remove(&child);
                }
                for genre in &item.genres {
                    let label = gtk4::Label::builder()
                        .label(genre)
                        .css_classes(["caption", "dim-label"])
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

                // Reset enriched sections while loading
                self.director_label.set_visible(false);
                self.writer_label.set_visible(false);
                self.cast_section.set_visible(false);
                self.tech_label.set_visible(false);
                self.collections_box.set_visible(false);

                // Load backdrop
                if let (Some(art_path), Some(source), Some(cache)) =
                    (&item.backdrop_path, &self.source, &self.artwork_cache)
                {
                    let url = source.artwork_url(art_path, 900, 360);
                    let cache = Arc::clone(cache);
                    sender.oneshot_command(async move {
                        match cache.get_or_download(&url).await {
                            Ok(path) => match gtk4::gdk::Texture::from_filename(&path) {
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
                        media_item: Some(item.clone()),
                    });
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
            MovieDetailCmd::BackdropReady(texture) => {
                self.backdrop.set_paintable(Some(&texture));
            }
            MovieDetailCmd::DetailLoaded(detail) => {
                self.populate_enrichment(&detail, &sender);
            }
            MovieDetailCmd::CastPhotoReady(idx, texture) => {
                // Find the cast card at this index and set its picture
                if let Some(child) = self.cast_box.observe_children().item(idx as u32)
                    && let Ok(card) = child.downcast::<gtk4::Box>()
                    && let Some(picture) = card.first_child()
                    && let Ok(picture) = picture.downcast::<gtk4::Picture>()
                {
                    picture.set_paintable(Some(&texture));
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
            // Clear existing cast cards
            while let Some(child) = self.cast_box.first_child() {
                self.cast_box.remove(&child);
            }

            for (idx, member) in detail.cast.iter().enumerate() {
                let card = gtk4::Box::builder()
                    .orientation(gtk4::Orientation::Vertical)
                    .spacing(4)
                    .width_request(80)
                    .build();

                let picture = gtk4::Picture::builder()
                    .content_fit(gtk4::ContentFit::Cover)
                    .width_request(80)
                    .height_request(80)
                    .css_classes(["cast-photo"])
                    .build();

                let name_label = gtk4::Label::builder()
                    .label(&member.name)
                    .halign(gtk4::Align::Center)
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .max_width_chars(12)
                    .css_classes(["caption"])
                    .build();

                card.append(&picture);
                card.append(&name_label);

                if let Some(ref character) = member.character {
                    let char_label = gtk4::Label::builder()
                        .label(character)
                        .halign(gtk4::Align::Center)
                        .ellipsize(gtk4::pango::EllipsizeMode::End)
                        .max_width_chars(12)
                        .css_classes(["caption", "dim-label"])
                        .build();
                    card.append(&char_label);
                }

                self.cast_box.append(&card);

                // Load cast photo asynchronously
                if let (Some(photo_path), Some(source), Some(cache)) =
                    (&member.photo_path, &self.source, &self.artwork_cache)
                {
                    let url = source.artwork_url(photo_path, 80, 80);
                    let cache = Arc::clone(cache);
                    sender.oneshot_command(async move {
                        match cache.get_or_download(&url).await {
                            Ok(path) => match gtk4::gdk::Texture::from_filename(&path) {
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

        // Technical info
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
                self.tech_label.set_label(&parts.join(" · "));
                self.tech_label.set_visible(true);
            }
        }

        // Collections
        if !detail.collections.is_empty() {
            while let Some(child) = self.collections_box.first_child() {
                self.collections_box.remove(&child);
            }

            let prefix = gtk4::Label::builder()
                .label("Part of:")
                .css_classes(["dim-label"])
                .build();
            self.collections_box.append(&prefix);

            for col in &detail.collections {
                let label = gtk4::Label::builder()
                    .label(&col.name)
                    .css_classes(["caption"])
                    .build();
                self.collections_box.append(&label);
            }

            self.collections_box.set_visible(true);
        }
    }
}
