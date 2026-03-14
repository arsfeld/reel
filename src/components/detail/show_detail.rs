use std::sync::Arc;

use adw::prelude::*;
use gtk4::prelude::*;
use libadwaita as adw;
use relm4::prelude::*;
use tracing::info;

use crate::models::media::{MediaItem, MediaType};
use crate::services::artwork::ArtworkCache;
use crate::services::media_source::MediaSource;

pub struct ShowDetail {
    show: Option<MediaItem>,
    seasons: Vec<MediaItem>,
    episodes: Vec<MediaItem>,
    source: Option<Arc<dyn MediaSource>>,
    artwork_cache: Option<Arc<ArtworkCache>>,
    // Widgets
    title_label: gtk4::Label,
    year_label: gtk4::Label,
    rating_label: gtk4::Label,
    content_rating_label: gtk4::Label,
    overview_label: gtk4::Label,
    season_dropdown: gtk4::DropDown,
    episodes_list: gtk4::ListBox,
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
    PlayMedia(String),
    Error(String),
}

pub enum ShowDetailCmd {
    SeasonsReady(Vec<MediaItem>),
    EpisodesReady(Vec<MediaItem>),
    Error(String),
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
        let rating_label = gtk4::Label::builder()
            .css_classes(["dim-label"])
            .visible(false)
            .build();
        let content_rating_label = gtk4::Label::builder()
            .css_classes(["dim-label"])
            .visible(false)
            .build();

        meta_box.append(&year_label);
        meta_box.append(&rating_label);
        meta_box.append(&content_rating_label);

        let overview_label = gtk4::Label::builder()
            .halign(gtk4::Align::Start)
            .wrap(true)
            .visible(false)
            .build();

        let season_dropdown = gtk4::DropDown::builder()
            .halign(gtk4::Align::Start)
            .margin_top(8)
            .build();

        let sender_season = sender.input_sender().clone();
        season_dropdown.connect_selected_notify(move |dd| {
            let _ = sender_season.send(ShowDetailMsg::SelectSeason(dd.selected()));
        });

        let episodes_list = gtk4::ListBox::builder()
            .css_classes(["boxed-list"])
            .selection_mode(gtk4::SelectionMode::None)
            .build();

        content_box.append(&title_label);
        content_box.append(&meta_box);
        content_box.append(&overview_label);
        content_box.append(&season_dropdown);
        content_box.append(&episodes_list);

        clamp.set_child(Some(&content_box));
        scrolled.set_child(Some(&clamp));
        toolbar.set_content(Some(&scrolled));
        root.append(&toolbar);

        let model = Self {
            show: None,
            seasons: Vec::new(),
            episodes: Vec::new(),
            source: None,
            artwork_cache: None,
            title_label,
            year_label,
            rating_label,
            content_rating_label,
            overview_label,
            season_dropdown,
            episodes_list,
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
                    self.rating_label.set_label(&format!("{rating:.1}"));
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

                let string_list = gtk4::StringList::new(&[] as &[&str]);
                for s in &seasons {
                    let name = if s.season_number == Some(0) {
                        "Specials".to_string()
                    } else {
                        s.title.clone()
                    };
                    string_list.append(&name);
                }
                self.season_dropdown.set_model(Some(&string_list));

                if !seasons.is_empty() {
                    self.season_dropdown.set_selected(0);
                    sender.input(ShowDetailMsg::SelectSeason(0));
                }
            }
            ShowDetailMsg::SelectSeason(index) => {
                let idx = index as usize;
                if idx >= self.seasons.len() {
                    return;
                }

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
                while let Some(child) = self.episodes_list.first_child() {
                    self.episodes_list.remove(&child);
                }

                self.episodes = episodes.clone();

                for (i, ep) in episodes.iter().enumerate() {
                    let ep_num = ep.episode_number.unwrap_or(0);
                    let title = format!("{ep_num}. {}", ep.title);

                    let row = adw::ActionRow::builder()
                        .title(&title)
                        .activatable(ep.file_path.is_some())
                        .build();

                    if let Some(ref air_date) = ep.air_date {
                        row.set_subtitle(air_date);
                    }

                    if let Some(ref runtime) = ep.format_runtime() {
                        let duration_label = gtk4::Label::builder()
                            .label(runtime)
                            .css_classes(["dim-label"])
                            .valign(gtk4::Align::Center)
                            .build();
                        row.add_suffix(&duration_label);
                    }

                    if ep.file_path.is_some() {
                        let play_icon = gtk4::Image::builder()
                            .icon_name("media-playback-start-symbolic")
                            .valign(gtk4::Align::Center)
                            .build();
                        row.add_suffix(&play_icon);

                        let sender_clone = sender.input_sender().clone();
                        let idx = i;
                        row.connect_activated(move |_| {
                            let _ = sender_clone.send(ShowDetailMsg::PlayEpisode(idx));
                        });
                    } else {
                        row.set_sensitive(false);
                    }

                    self.episodes_list.append(&row);
                }
            }
            ShowDetailMsg::PlayEpisode(index) => {
                if let Some(ep) = self.episodes.get(index)
                    && let Some(ref file_path) = ep.file_path
                    && let Some(ref source) = self.source
                {
                    let url = source.playback_url(file_path);
                    info!("Playing episode: {}", ep.title);
                    let _ = sender.output(ShowDetailOutput::PlayMedia(url));
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
            ShowDetailCmd::Error(msg) => {
                sender.input(ShowDetailMsg::LoadError(msg));
            }
        }
    }
}
