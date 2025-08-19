use gtk4::{self, prelude::*, glib};
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::player::GStreamerPlayer;
use crate::state::AppState;
use crate::models::{Movie, Show, Episode, MediaItem};
use crate::backends::traits::MediaBackend;

#[derive(Clone)]
pub struct PlayerPage {
    widget: gtk4::Box,
    player: Arc<RwLock<GStreamerPlayer>>,
    controls: PlayerControls,
    overlay: gtk4::Overlay,
    video_container: gtk4::Box,
}

impl std::fmt::Debug for PlayerPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerPage")
            .field("widget", &"gtk4::Box")
            .field("player", &"Arc<RwLock<GStreamerPlayer>>")
            .finish()
    }
}

impl PlayerPage {
    pub fn new(_state: Arc<AppState>) -> Self {
        // Create main container
        let widget = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .build();
        widget.add_css_class("player-page");
        
        // Create overlay for video and controls
        let overlay = gtk4::Overlay::new();
        overlay.set_vexpand(true);
        overlay.set_hexpand(true);
        
        // Video container
        let video_container = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        video_container.set_vexpand(true);
        video_container.set_hexpand(true);
        video_container.add_css_class("video-container");
        overlay.set_child(Some(&video_container));
        
        // Create player
        let player = Arc::new(RwLock::new(
            GStreamerPlayer::new().expect("Failed to create GStreamer player")
        ));
        
        // Create controls
        let controls = PlayerControls::new(player.clone());
        controls.widget.set_valign(gtk4::Align::End);
        controls.widget.set_margin_bottom(20);
        // Hide controls by default - they'll show on mouse movement
        controls.widget.set_visible(false);
        overlay.add_overlay(&controls.widget);
        
        // Set up hover detection for showing/hiding controls
        let controls_widget = controls.widget.clone();
        let hide_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let hover_controller = gtk4::EventControllerMotion::new();
        
        let hide_timer_clone = hide_timer.clone();
        hover_controller.connect_motion(move |_, _, _| {
            // Show controls
            controls_widget.set_visible(true);
            controls_widget.add_css_class("osd");
            
            // Cancel previous timer if exists
            if let Some(timer_id) = hide_timer_clone.borrow_mut().take() {
                timer_id.remove();
            }
            
            // Hide again after 3 seconds of no movement
            let controls_widget_inner = controls_widget.clone();
            let hide_timer_inner = hide_timer_clone.clone();
            let timer_id = glib::timeout_add_local(std::time::Duration::from_secs(3), move || {
                controls_widget_inner.set_visible(false);
                // Clear the timer reference since it's done
                hide_timer_inner.borrow_mut().take();
                glib::ControlFlow::Break
            });
            hide_timer_clone.borrow_mut().replace(timer_id);
        });
        
        // Add controller to the overlay (covers the whole video area)
        overlay.add_controller(hover_controller);
        
        widget.append(&overlay);
        
        Self {
            widget,
            player,
            controls,
            overlay,
            video_container,
        }
    }
    
    pub async fn load_media(&self, media_item: &MediaItem, state: Arc<AppState>) -> anyhow::Result<()> {
        info!("Loading media item: {}", media_item.title());
        
        // Get the backend manager
        let backend_manager = state.backend_manager.read().await;
        
        if let Some((backend_id, backend)) = backend_manager.get_active_backend() {
            info!("Using backend: {}", backend_id);
            // Get stream URL from backend
            let stream_info = backend.get_stream_url(media_item.id()).await?;
            info!("Got stream URL: {}", stream_info.url);
            
            // Create video widget
            let mut player = self.player.write().await;
            let video_widget = player.create_video_widget();
            
            // Add video widget to container
            self.video_container.append(&video_widget);
            
            // Load the media (sink is already set up in create_video_widget)
            player.load_media(&stream_info.url, None).await?;
            
            // Update controls with media info
            self.controls.set_media_info(media_item.title(), None).await;
            
            // Start playback
            player.play().await?;
        } else {
            error!("No active backend found");
        }
        
        Ok(())
    }
    
    pub fn widget(&self) -> &gtk4::Box {
        &self.widget
    }
    
    pub async fn stop(&self) {
        let player = self.player.read().await;
        if let Err(e) = player.stop().await {
            error!("Failed to stop player: {}", e);
        }
    }
    
    pub async fn get_video_dimensions(&self) -> Option<(i32, i32)> {
        let player = self.player.read().await;
        player.get_video_dimensions().await
    }
}

#[derive(Clone)]
struct PlayerControls {
    widget: gtk4::Box,
    play_button: gtk4::Button,
    progress_bar: gtk4::Scale,
    volume_button: gtk4::Scale,
    fullscreen_button: gtk4::Button,
    title_label: gtk4::Label,
    time_label: gtk4::Label,
    player: Arc<RwLock<GStreamerPlayer>>,
    is_seeking: Arc<RwLock<bool>>,
}

impl PlayerControls {
    fn new(player: Arc<RwLock<GStreamerPlayer>>) -> Self {
        // Main controls container
        let widget = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(10)
            .margin_start(20)
            .margin_end(20)
            .build();
        widget.add_css_class("player-controls");
        widget.add_css_class("osd");
        
        // Title label
        let title_label = gtk4::Label::new(None);
        title_label.add_css_class("title-2");
        title_label.set_halign(gtk4::Align::Start);
        widget.append(&title_label);
        
        // Progress bar
        let progress_bar = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 100.0, 0.1);
        progress_bar.set_draw_value(false);
        progress_bar.add_css_class("progress-bar");
        widget.append(&progress_bar);
        
        // Controls row
        let controls_row = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(10)
            .build();
        
        // Play/pause button
        let play_button = gtk4::Button::from_icon_name("media-playback-start-symbolic");
        play_button.add_css_class("circular");
        controls_row.append(&play_button);
        
        // Time label
        let time_label = gtk4::Label::new(Some("0:00 / 0:00"));
        time_label.add_css_class("dim-label");
        controls_row.append(&time_label);
        
        // Spacer
        let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        controls_row.append(&spacer);
        
        // Volume button - using Scale as VolumeButton is deprecated
        let volume_button = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 1.0, 0.01);
        volume_button.set_value(1.0);
        volume_button.set_draw_value(false);
        volume_button.set_size_request(100, -1);
        controls_row.append(&volume_button);
        
        // Fullscreen button
        let fullscreen_button = gtk4::Button::from_icon_name("view-fullscreen-symbolic");
        fullscreen_button.add_css_class("flat");
        controls_row.append(&fullscreen_button);
        
        widget.append(&controls_row);
        
        let controls = Self {
            widget,
            play_button: play_button.clone(),
            progress_bar: progress_bar.clone(),
            volume_button: volume_button.clone(),
            fullscreen_button: fullscreen_button.clone(),
            title_label,
            time_label: time_label.clone(),
            player: player.clone(),
            is_seeking: Arc::new(RwLock::new(false)),
        };
        
        // Set up event handlers
        controls.setup_handlers();
        
        // Start position update timer
        controls.start_position_timer();
        
        controls
    }
    
    fn setup_handlers(&self) {
        let player = self.player.clone();
        let button = self.play_button.clone();
        
        // Play/pause button
        self.play_button.connect_clicked(move |_| {
            let player = player.clone();
            let button = button.clone();
            glib::spawn_future_local(async move {
                let player = player.read().await;
                // Toggle play/pause
                // This is simplified - you'd check the actual state
                if button.icon_name() == Some("media-playback-start-symbolic".into()) {
                    if let Err(e) = player.play().await {
                        error!("Failed to play: {}", e);
                    }
                    button.set_icon_name("media-playback-pause-symbolic");
                } else {
                    if let Err(e) = player.pause().await {
                        error!("Failed to pause: {}", e);
                    }
                    button.set_icon_name("media-playback-start-symbolic");
                }
            });
        });
        
        // Volume control
        let player = self.player.clone();
        self.volume_button.connect_value_changed(move |scale| {
            let player = player.clone();
            let volume = scale.value();
            glib::spawn_future_local(async move {
                let player = player.read().await;
                if let Err(e) = player.set_volume(volume).await {
                    error!("Failed to set volume: {}", e);
                }
            });
        });
        
        // Progress bar seek - only seek when user drags, not programmatic updates
        let player = self.player.clone();
        let is_seeking = self.is_seeking.clone();
        self.progress_bar.connect_change_value(move |scale, _, value| {
            let player = player.clone();
            let is_seeking = is_seeking.clone();
            glib::spawn_future_local(async move {
                // Mark that we're seeking
                *is_seeking.write().await = true;
                
                let player = player.read().await;
                if let Some(duration) = player.get_duration().await {
                    let seek_position = Duration::from_secs_f64(
                        value * duration.as_secs_f64() / 100.0
                    );
                    if let Err(e) = player.seek(seek_position).await {
                        error!("Failed to seek: {}", e);
                    }
                }
                
                // Clear seeking flag after a short delay
                let is_seeking = is_seeking.clone();
                glib::timeout_add_local(Duration::from_millis(100), move || {
                    let is_seeking = is_seeking.clone();
                    glib::spawn_future_local(async move {
                        *is_seeking.write().await = false;
                    });
                    glib::ControlFlow::Break
                });
            });
            
            glib::Propagation::Proceed
        });
        
        // Fullscreen button
        self.fullscreen_button.connect_clicked(|button| {
            if let Some(window) = button.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) {
                if window.is_fullscreen() {
                    window.unfullscreen();
                    button.set_icon_name("view-fullscreen-symbolic");
                } else {
                    window.fullscreen();
                    button.set_icon_name("view-restore-symbolic");
                }
            }
        });
    }
    
    fn start_position_timer(&self) {
        let player = self.player.clone();
        let progress_bar = self.progress_bar.clone();
        let time_label = self.time_label.clone();
        let is_seeking = self.is_seeking.clone();
        
        glib::timeout_add_local(Duration::from_millis(500), move || {
            let player = player.clone();
            let progress_bar = progress_bar.clone();
            let time_label = time_label.clone();
            let is_seeking = is_seeking.clone();
            
            glib::spawn_future_local(async move {
                // Don't update progress bar if user is seeking
                let is_seeking = *is_seeking.read().await;
                
                let player = player.read().await;
                
                if let (Some(position), Some(duration)) = 
                    (player.get_position().await, player.get_duration().await) {
                    
                    // Only update progress bar if not seeking
                    if !is_seeking {
                        let progress = (position.as_secs_f64() / duration.as_secs_f64()) * 100.0;
                        progress_bar.set_value(progress);
                    }
                    
                    // Always update time label
                    let pos_str = format_duration(position);
                    let dur_str = format_duration(duration);
                    time_label.set_text(&format!("{} / {}", pos_str, dur_str));
                }
            });
            
            glib::ControlFlow::Continue
        });
    }
    
    async fn set_media_info(&self, title: &str, _subtitle: Option<&str>) {
        self.title_label.set_text(title);
    }
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    
    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}