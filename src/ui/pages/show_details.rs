use gtk4::{self, prelude::*, glib};
use libadwaita as adw;
use adw::prelude::*;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::state::AppState;
use crate::models::{Show, Season, Episode, MediaItem};
use crate::backends::traits::MediaBackend;

#[derive(Clone)]
pub struct ShowDetailsPage {
    widget: gtk4::Box,
    content_box: adw::Clamp,
    seasons_list: gtk4::ListBox,
    episodes_list: gtk4::ListBox,
    show_info: gtk4::Box,
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
        // Main container
        let widget = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .build();
        
        // Header with show info
        let show_info = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(20)
            .margin_top(20)
            .margin_bottom(20)
            .margin_start(20)
            .margin_end(20)
            .build();
        
        widget.append(&show_info);
        
        // Content area with clamp for max width
        let content_box = adw::Clamp::builder()
            .maximum_size(1200)
            .tightening_threshold(800)
            .margin_start(20)
            .margin_end(20)
            .build();
        
        // Create a horizontal paned view for seasons and episodes
        let paned = gtk4::Paned::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .position(250)
            .shrink_start_child(false)
            .shrink_end_child(false)
            .build();
        
        // Seasons list (left side)
        let seasons_frame = gtk4::Frame::builder()
            .label("Seasons")
            .build();
        
        let seasons_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .min_content_width(200)
            .build();
        
        let seasons_list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::Single)
            .build();
        seasons_list.add_css_class("navigation-sidebar");
        
        seasons_scroll.set_child(Some(&seasons_list));
        seasons_frame.set_child(Some(&seasons_scroll));
        paned.set_start_child(Some(&seasons_frame));
        
        // Episodes list (right side)
        let episodes_frame = gtk4::Frame::builder()
            .label("Episodes")
            .build();
        
        let episodes_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .build();
        
        let episodes_list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::Single)
            .activate_on_single_click(true)
            .build();
        episodes_list.add_css_class("boxed-list");
        
        episodes_scroll.set_child(Some(&episodes_list));
        episodes_frame.set_child(Some(&episodes_scroll));
        paned.set_end_child(Some(&episodes_frame));
        
        content_box.set_child(Some(&paned));
        widget.append(&content_box);
        
        let page = Self {
            widget,
            content_box,
            seasons_list: seasons_list.clone(),
            episodes_list: episodes_list.clone(),
            show_info,
            state: state.clone(),
            current_show: Arc::new(RwLock::new(None)),
            current_season: Arc::new(RwLock::new(None)),
            on_episode_selected: Arc::new(RwLock::new(None)),
        };
        
        // Connect season selection
        let page_weak = page.clone();
        
        // We'll set up season and episode handlers when loading content
        
        page
    }
    
    pub async fn load_show(&self, show: Show) {
        info!("Loading show details: {}", show.title);
        
        // Clear existing content
        self.clear_show_info();
        self.clear_seasons();
        self.clear_episodes();
        
        // Display show info
        self.display_show_info(&show);
        
        // Store current show
        *self.current_show.write().await = Some(show.clone());
        
        // Display seasons
        for season in &show.seasons {
            self.add_season_row(season);
        }
        
        // Select first season if available
        if let Some(first_season) = show.seasons.first() {
            self.load_episodes(first_season.season_number).await;
            
            // Select the first row in seasons list
            if let Some(first_row) = self.seasons_list.row_at_index(0) {
                self.seasons_list.select_row(Some(&first_row));
            }
        }
    }
    
    fn display_show_info(&self, show: &Show) {
        // Show poster
        let poster = gtk4::Picture::builder()
            .width_request(150)
            .height_request(225)
            .content_fit(gtk4::ContentFit::Cover)
            .build();
        
        if let Some(poster_url) = &show.poster_url {
            // TODO: Load actual poster image
            debug!("Would load poster from: {}", poster_url);
        }
        
        self.show_info.append(&poster);
        
        // Show details
        let details_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(10)
            .hexpand(true)
            .build();
        
        let title = gtk4::Label::builder()
            .label(&show.title)
            .xalign(0.0)
            .build();
        title.add_css_class("title-1");
        details_box.append(&title);
        
        if let Some(year) = show.year {
            let year_label = gtk4::Label::builder()
                .label(&format!("{}", year))
                .xalign(0.0)
                .build();
            year_label.add_css_class("dim-label");
            details_box.append(&year_label);
        }
        
        if let Some(overview) = &show.overview {
            let overview_label = gtk4::Label::builder()
                .label(overview)
                .wrap(true)
                .xalign(0.0)
                .build();
            details_box.append(&overview_label);
        }
        
        let seasons_label = gtk4::Label::builder()
            .label(&format!("{} seasons", show.seasons.len()))
            .xalign(0.0)
            .build();
        seasons_label.add_css_class("accent");
        details_box.append(&seasons_label);
        
        self.show_info.append(&details_box);
    }
    
    fn add_season_row(&self, season: &Season) {
        let row_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(10)
            .margin_top(5)
            .margin_bottom(5)
            .margin_start(10)
            .margin_end(10)
            .build();
        
        let season_label = gtk4::Label::builder()
            .label(&format!("Season {}", season.season_number))
            .hexpand(true)
            .xalign(0.0)
            .build();
        row_box.append(&season_label);
        
        let episode_count = gtk4::Label::builder()
            .label(&format!("{} episodes", season.episode_count))
            .build();
        episode_count.add_css_class("dim-label");
        episode_count.add_css_class("caption");
        row_box.append(&episode_count);
        
        let row = gtk4::ListBoxRow::new();
        row.set_child(Some(&row_box));
        
        // Connect row activation directly with the season number
        let season_number = season.season_number;
        let self_clone = self.clone();
        row.connect_activate(move |_| {
            let page = self_clone.clone();
            glib::spawn_future_local(async move {
                page.load_episodes(season_number).await;
            });
        });
        
        self.seasons_list.append(&row);
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
                            self.add_episode_row(episode);
                        }
                    }
                    Err(e) => {
                        error!("Failed to load episodes: {}", e);
                        // Show error message
                        let error_label = gtk4::Label::builder()
                            .label(&format!("Failed to load episodes: {}", e))
                            .build();
                        self.episodes_list.append(&error_label);
                    }
                }
            }
        }
    }
    
    fn add_episode_row(&self, episode: Episode) {
        let row = adw::ActionRow::builder()
            .title(&format!("{}. {}", episode.episode_number, episode.title))
            .activatable(true)
            .build();
        
        // Add subtitle with duration
        if let Some(overview) = &episode.overview {
            let truncated = if overview.len() > 100 {
                format!("{}...", &overview[..100])
            } else {
                overview.clone()
            };
            row.set_subtitle(&truncated);
        }
        
        // Add duration as suffix
        let duration_mins = episode.duration.as_secs() / 60;
        let duration_label = gtk4::Label::builder()
            .label(&format!("{} min", duration_mins))
            .build();
        duration_label.add_css_class("dim-label");
        row.add_suffix(&duration_label);
        
        // Add play icon
        let play_icon = gtk4::Image::from_icon_name("media-playback-start-symbolic");
        row.add_suffix(&play_icon);
        
        // Connect activation directly with the episode
        let self_clone = self.clone();
        let episode_clone = episode.clone();
        row.connect_activated(move |_| {
            let page = self_clone.clone();
            let episode = episode_clone.clone();
            glib::spawn_future_local(async move {
                if let Some(callback) = page.on_episode_selected.read().await.as_ref() {
                    callback(&episode);
                }
            });
        });
        
        self.episodes_list.append(&row);
    }
    
    fn clear_show_info(&self) {
        while let Some(child) = self.show_info.first_child() {
            self.show_info.remove(&child);
        }
    }
    
    fn clear_seasons(&self) {
        while let Some(child) = self.seasons_list.first_child() {
            self.seasons_list.remove(&child);
        }
    }
    
    fn clear_episodes(&self) {
        while let Some(child) = self.episodes_list.first_child() {
            self.episodes_list.remove(&child);
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