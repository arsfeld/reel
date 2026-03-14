use std::sync::Arc;

use gtk4::prelude::*;
use libadwaita as adw;
use relm4::prelude::*;
use tracing::info;

use crate::models::media::MediaItem;
use crate::services::artwork::ArtworkCache;
use crate::services::media_source::MediaSource;

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
    genres_box: gtk4::FlowBox,
    overview_label: gtk4::Label,
    play_button: gtk4::Button,
    backdrop: gtk4::Picture,
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
    PlayMedia(String),
    Error(String),
}

pub enum MovieDetailCmd {
    BackdropReady(gtk4::gdk::Texture),
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

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = view_output!();

        // Build widget hierarchy manually
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

        content_box.append(&backdrop);
        content_box.append(&title_label);
        content_box.append(&meta_box);
        content_box.append(&genres_box);
        content_box.append(&play_button);
        content_box.append(&overview_label);

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
            genres_box,
            overview_label,
            play_button,
            backdrop,
        };

        ComponentParts { model, widgets }
    }

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

                self.item = Some(item);
            }
            MovieDetailMsg::Play => {
                if let Some(ref item) = self.item
                    && let Some(ref file_path) = item.file_path
                    && let Some(ref source) = self.source
                {
                    let url = source.playback_url(file_path);
                    info!("Playing: {}", item.title);
                    let _ = sender.output(MovieDetailOutput::PlayMedia(url));
                }
            }
        }
    }

    fn update_cmd(
        &mut self,
        cmd: Self::CommandOutput,
        _sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match cmd {
            MovieDetailCmd::BackdropReady(texture) => {
                self.backdrop.set_paintable(Some(&texture));
            }
            MovieDetailCmd::Noop => {}
        }
    }
}
