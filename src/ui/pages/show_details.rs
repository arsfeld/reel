use gtk4::{self, prelude::*, glib};
use libadwaita as adw;
use adw::prelude::*;
use std::sync::Arc;
use std::cell::RefCell;
use std::rc::Rc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::state::AppState;
use crate::models::{Show, Season, Episode, MediaItem};
use crate::backends::traits::MediaBackend;
use crate::utils::{OptimizedImageLoader, ImageSize};

// Global optimized image loader instance
use once_cell::sync::Lazy;
static IMAGE_LOADER: Lazy<OptimizedImageLoader> = Lazy::new(|| {
    OptimizedImageLoader::new().expect("Failed to create OptimizedImageLoader")
});

#[derive(Clone)]
pub struct ShowDetailsPage {
    widget: gtk4::Box,
    header_box: gtk4::Box,
    season_dropdown: gtk4::DropDown,
    episodes_carousel: gtk4::ScrolledWindow,
    episodes_box: gtk4::Box,
    show_poster: gtk4::Picture,
    show_info_box: gtk4::Box,
    state: Arc<AppState>,
    current_show: Arc<RwLock<Option<Show>>>,
    current_season: Arc<RwLock<Option<u32>>>,
    on_episode_selected: Arc<RwLock<Option<Box<dyn Fn(&Episode)>>>>,
}

impl std::fmt::Debug for ShowDetailsPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShowDetailsPage")
            .field("widget", &"gtk4::Box")
            .finish()
    }
}

impl ShowDetailsPage {
    pub fn new(state: Arc<AppState>) -> Self {
        // Main container with dark background
        let widget = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .css_classes(vec!["view"])
            .build();
        
        // Header section with show info and backdrop
        let header_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .build();
        
        // Show info container (poster + details)
        let info_container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(24)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        
        // Show poster with rounded corners
        let poster_frame = gtk4::Frame::builder()
            .width_request(200)
            .height_request(300)
            .css_classes(vec!["card"])
            .build();
        
        let show_poster = gtk4::Picture::builder()
            .content_fit(gtk4::ContentFit::Cover)
            .build();
        poster_frame.set_child(Some(&show_poster));
        info_container.append(&poster_frame);
        
        // Show details box
        let show_info_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(12)
            .hexpand(true)
            .valign(gtk4::Align::Center)
            .build();
        info_container.append(&show_info_box);
        
        header_box.append(&info_container);
        
        // Season selector section
        let season_section = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .margin_start(24)
            .margin_end(24)
            .margin_bottom(12)
            .build();
        
        let season_label = gtk4::Label::builder()
            .label("Season")
            .css_classes(vec!["heading"])
            .build();
        season_section.append(&season_label);
        
        // Modern dropdown for season selection
        let season_dropdown = gtk4::DropDown::builder()
            .enable_search(false)
            .build();
        season_section.append(&season_dropdown);
        
        header_box.append(&season_section);
        widget.append(&header_box);
        
        // Episodes carousel section
        let episodes_section = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .margin_start(24)
            .margin_end(24)
            .margin_bottom(24)
            .vexpand(true)
            .build();
        
        let episodes_label = gtk4::Label::builder()
            .label("Episodes")
            .css_classes(vec!["title-2"])
            .xalign(0.0)
            .margin_bottom(12)
            .build();
        episodes_section.append(&episodes_label);
        
        // Horizontal scrolled window for episode carousel
        let episodes_carousel = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Never)
            .height_request(280)
            .vexpand(false)
            .build();
        
        // Container for episode cards
        let episodes_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(16)
            .build();
        
        episodes_carousel.set_child(Some(&episodes_box));
        episodes_section.append(&episodes_carousel);
        widget.append(&episodes_section);
        
        let page = Self {
            widget,
            header_box,
            season_dropdown: season_dropdown.clone(),
            episodes_carousel,
            episodes_box: episodes_box.clone(),
            show_poster,
            show_info_box,
            state: state.clone(),
            current_show: Arc::new(RwLock::new(None)),
            current_season: Arc::new(RwLock::new(None)),
            on_episode_selected: Arc::new(RwLock::new(None)),
        };
        
        // Connect season dropdown selection
        let page_weak = page.clone();
        season_dropdown.connect_selected_notify(move |dropdown| {
            let selected = dropdown.selected();
            let page = page_weak.clone();
            glib::spawn_future_local(async move {
                if let Some(show) = page.current_show.read().await.as_ref() {
                    if let Some(season) = show.seasons.get(selected as usize) {
                        page.load_episodes(season.season_number).await;
                    }
                }
            });
        });
        
        page
    }
    
    pub async fn load_show(&self, show: Show) {
        info!("Loading show details: {}", show.title);
        
        // Clear existing content
        self.clear_episodes();
        
        // Display show info
        self.display_show_info(&show).await;
        
        // Store current show
        *self.current_show.write().await = Some(show.clone());
        
        // Setup season dropdown
        let season_labels: Vec<String> = show.seasons
            .iter()
            .map(|s| format!("Season {}", s.season_number))
            .collect();
        
        let string_list = gtk4::StringList::new(&season_labels.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        self.season_dropdown.set_model(Some(&string_list));
        
        // Select first season if available
        if !show.seasons.is_empty() {
            self.season_dropdown.set_selected(0);
            if let Some(first_season) = show.seasons.first() {
                self.load_episodes(first_season.season_number).await;
            }
        }
    }
    
    async fn display_show_info(&self, show: &Show) {
        // Clear previous info
        while let Some(child) = self.show_info_box.first_child() {
            self.show_info_box.remove(&child);
        }
        
        // Load poster image
        if let Some(poster_url) = &show.poster_url {
            let picture = self.show_poster.clone();
            let url = poster_url.clone();
            
            glib::spawn_future_local(async move {
                match IMAGE_LOADER.load_image(&url, ImageSize::Large).await {
                    Ok(texture) => {
                        picture.set_paintable(Some(&texture));
                    }
                    Err(e) => {
                        error!("Failed to load show poster: {}", e);
                    }
                }
            });
        }
        
        // Show title
        let title = gtk4::Label::builder()
            .label(&show.title)
            .css_classes(vec!["title-1"])
            .xalign(0.0)
            .wrap(true)
            .build();
        self.show_info_box.append(&title);
        
        // Show metadata row (year, rating, etc.)
        let metadata_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(16)
            .build();
        
        if let Some(year) = show.year {
            let year_label = gtk4::Label::builder()
                .label(&format!("{}", year))
                .css_classes(vec!["dim-label"])
                .build();
            metadata_box.append(&year_label);
        }
        
        if let Some(rating) = show.rating {
            let rating_box = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .spacing(4)
                .build();
            
            let star_icon = gtk4::Image::from_icon_name("starred-symbolic");
            star_icon.add_css_class("warning");
            rating_box.append(&star_icon);
            
            let rating_label = gtk4::Label::builder()
                .label(&format!("{:.1}", rating))
                .build();
            rating_box.append(&rating_label);
            metadata_box.append(&rating_box);
        }
        
        let seasons_label = gtk4::Label::builder()
            .label(&format!("{} seasons", show.seasons.len()))
            .css_classes(vec!["accent"])
            .build();
        metadata_box.append(&seasons_label);
        
        self.show_info_box.append(&metadata_box);
        
        // Show overview
        if let Some(overview) = &show.overview {
            let overview_label = gtk4::Label::builder()
                .label(overview)
                .css_classes(vec!["body"])
                .wrap(true)
                .xalign(0.0)
                .margin_top(12)
                .build();
            self.show_info_box.append(&overview_label);
        }
        
        // Genres
        if !show.genres.is_empty() {
            let genres_box = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .spacing(8)
                .margin_top(12)
                .build();
            
            for genre in &show.genres {
                let genre_chip = adw::Bin::builder()
                    .css_classes(vec!["card", "compact"])
                    .build();
                
                let genre_label = gtk4::Label::builder()
                    .label(genre)
                    .css_classes(vec!["caption"])
                    .margin_top(4)
                    .margin_bottom(4)
                    .margin_start(8)
                    .margin_end(8)
                    .build();
                
                genre_chip.set_child(Some(&genre_label));
                genres_box.append(&genre_chip);
            }
            
            self.show_info_box.append(&genres_box);
        }
    }
    
    async fn load_episodes(&self, season_number: u32) {
        info!("Loading episodes for season {}", season_number);
        
        // Clear existing episodes
        self.clear_episodes();
        
        // Store current season
        *self.current_season.write().await = Some(season_number);
        
        // Get the show
        let show = self.current_show.read().await;
        if let Some(show) = show.as_ref() {
            // Get backend and fetch episodes
            let backend_manager = self.state.backend_manager.read().await;
            if let Some((_, backend)) = backend_manager.get_active_backend() {
                match backend.get_episodes(&show.id, season_number).await {
                    Ok(episodes) => {
                        for episode in episodes {
                            self.add_episode_card(episode);
                        }
                    }
                    Err(e) => {
                        error!("Failed to load episodes: {}", e);
                        // Show error message
                        let error_label = gtk4::Label::builder()
                            .label(&format!("Failed to load episodes: {}", e))
                            .css_classes(vec!["error"])
                            .build();
                        self.episodes_box.append(&error_label);
                    }
                }
            }
        }
    }
    
    fn add_episode_card(&self, episode: Episode) {
        // Create episode card
        let card = gtk4::Button::builder()
            .css_classes(vec!["card", "activatable"])
            .width_request(280)
            .build();
        
        let card_content = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(8)
            .build();
        
        // Episode thumbnail placeholder
        let thumbnail_frame = gtk4::Frame::builder()
            .height_request(157)  // 16:9 aspect ratio for 280px width
            .css_classes(vec!["view"])
            .build();
        
        let thumbnail = gtk4::Picture::builder()
            .content_fit(gtk4::ContentFit::Cover)
            .build();
        
        // Load episode thumbnail if available
        if let Some(thumb_url) = &episode.thumbnail_url {
            let picture = thumbnail.clone();
            let url = thumb_url.clone();
            
            glib::spawn_future_local(async move {
                match IMAGE_LOADER.load_image(&url, ImageSize::Medium).await {
                    Ok(texture) => {
                        picture.set_paintable(Some(&texture));
                    }
                    Err(e) => {
                        debug!("Failed to load episode thumbnail: {}", e);
                    }
                }
            });
        } else {
            // Show play icon as placeholder
            let placeholder_box = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Vertical)
                .valign(gtk4::Align::Center)
                .halign(gtk4::Align::Center)
                .build();
            
            let play_icon = gtk4::Image::builder()
                .icon_name("media-playback-start-symbolic")
                .pixel_size(48)
                .css_classes(vec!["dim-label"])
                .build();
            
            placeholder_box.append(&play_icon);
            thumbnail_frame.set_child(Some(&placeholder_box));
        }
        
        if thumbnail_frame.child().is_none() {
            thumbnail_frame.set_child(Some(&thumbnail));
        }
        
        // Add overlay for episode number
        let overlay = gtk4::Overlay::new();
        overlay.set_child(Some(&thumbnail_frame));
        
        let episode_number_label = gtk4::Label::builder()
            .label(&format!("E{}", episode.episode_number))
            .css_classes(vec!["osd", "numeric"])
            .halign(gtk4::Align::Start)
            .valign(gtk4::Align::Start)
            .margin_top(8)
            .margin_start(8)
            .build();
        overlay.add_overlay(&episode_number_label);
        
        // Add watched indicator if episode is watched
        if episode.view_count > 0 {
            let watched_icon = gtk4::Image::builder()
                .icon_name("object-select-symbolic")
                .css_classes(vec!["success"])
                .halign(gtk4::Align::End)
                .valign(gtk4::Align::Start)
                .margin_top(8)
                .margin_end(8)
                .build();
            overlay.add_overlay(&watched_icon);
        }
        
        // Add progress bar if partially watched
        if let Some(position) = episode.playback_position {
            if position.as_secs() > 0 && position < episode.duration {
                let progress = position.as_secs_f64() / episode.duration.as_secs_f64();
                let progress_bar = gtk4::ProgressBar::builder()
                    .fraction(progress)
                    .css_classes(vec!["osd"])
                    .valign(gtk4::Align::End)
                    .margin_bottom(4)
                    .margin_start(4)
                    .margin_end(4)
                    .build();
                overlay.add_overlay(&progress_bar);
            }
        }
        
        card_content.append(&overlay);
        
        // Episode info
        let info_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(4)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(12)
            .margin_end(12)
            .build();
        
        // Episode title
        let title_label = gtk4::Label::builder()
            .label(&episode.title)
            .css_classes(vec!["heading"])
            .xalign(0.0)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .single_line_mode(true)
            .build();
        info_box.append(&title_label);
        
        // Episode duration
        let duration_mins = episode.duration.as_secs() / 60;
        let duration_label = gtk4::Label::builder()
            .label(&format!("{} min", duration_mins))
            .css_classes(vec!["dim-label", "caption"])
            .xalign(0.0)
            .build();
        info_box.append(&duration_label);
        
        card_content.append(&info_box);
        card.set_child(Some(&card_content));
        
        // Connect click handler
        let self_clone = self.clone();
        let episode_clone = episode.clone();
        card.connect_clicked(move |_| {
            let page = self_clone.clone();
            let episode = episode_clone.clone();
            glib::spawn_future_local(async move {
                if let Some(callback) = page.on_episode_selected.read().await.as_ref() {
                    callback(&episode);
                }
            });
        });
        
        self.episodes_box.append(&card);
    }
    
    fn clear_episodes(&self) {
        while let Some(child) = self.episodes_box.first_child() {
            self.episodes_box.remove(&child);
        }
    }
    
    pub fn set_on_episode_selected<F>(&self, callback: F)
    where
        F: Fn(&Episode) + 'static,
    {
        let on_episode_selected = self.on_episode_selected.clone();
        glib::spawn_future_local(async move {
            *on_episode_selected.write().await = Some(Box::new(callback));
        });
    }
    
    pub fn widget(&self) -> &gtk4::Box {
        &self.widget
    }
}