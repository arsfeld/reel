use gtk4::{self, prelude::*, glib, gdk, gio};
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use chrono;

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
    current_stream_info: Arc<RwLock<Option<crate::models::StreamInfo>>>,
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
        info!("PlayerPage::new() - Creating new player page");
        // Create main container
        let widget = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .build();
        widget.add_css_class("player-page");
        debug!("PlayerPage::new() - Created main widget container");
        
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
        info!("PlayerPage::new() - Creating GStreamer player");
        let player = Arc::new(RwLock::new(
            GStreamerPlayer::new().expect("Failed to create GStreamer player")
        ));
        info!("PlayerPage::new() - GStreamer player created successfully");
        
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
        
        // Add keyboard event controller for fullscreen and playback controls
        let key_controller = gtk4::EventControllerKey::new();
        let controls_for_key = controls.clone();
        let overlay_for_key = overlay.clone();
        
        key_controller.connect_key_pressed(move |controller, keyval, _keycode, _state| {
            match keyval {
                // F or F11 for fullscreen toggle
                gdk::Key::f | gdk::Key::F | gdk::Key::F11 => {
                    if let Some(widget) = controller.widget() {
                        if let Some(window) = widget.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) {
                            if window.is_fullscreen() {
                                window.unfullscreen();
                                controls_for_key.fullscreen_button.set_icon_name("view-fullscreen-symbolic");
                            } else {
                                window.fullscreen();
                                controls_for_key.fullscreen_button.set_icon_name("view-restore-symbolic");
                            }
                        }
                    }
                    glib::Propagation::Stop
                }
                // Escape to exit fullscreen
                gdk::Key::Escape => {
                    if let Some(widget) = controller.widget() {
                        if let Some(window) = widget.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) {
                            if window.is_fullscreen() {
                                window.unfullscreen();
                                controls_for_key.fullscreen_button.set_icon_name("view-fullscreen-symbolic");
                            }
                        }
                    }
                    glib::Propagation::Stop
                }
                // Space for play/pause
                gdk::Key::space => {
                    controls_for_key.play_button.emit_clicked();
                    glib::Propagation::Stop
                }
                // Arrow keys for seeking
                gdk::Key::Left => {
                    // Seek backward 10 seconds
                    let player = controls_for_key.player.clone();
                    glib::spawn_future_local(async move {
                        let player = player.read().await;
                        if let Some(position) = player.get_position().await {
                            let new_position = position.saturating_sub(Duration::from_secs(10));
                            if let Err(e) = player.seek(new_position).await {
                                error!("Failed to seek backward: {}", e);
                            }
                        }
                    });
                    glib::Propagation::Stop
                }
                gdk::Key::Right => {
                    // Seek forward 10 seconds
                    let player = controls_for_key.player.clone();
                    glib::spawn_future_local(async move {
                        let player = player.read().await;
                        if let Some(position) = player.get_position().await {
                            let new_position = position + Duration::from_secs(10);
                            if let Err(e) = player.seek(new_position).await {
                                error!("Failed to seek forward: {}", e);
                            }
                        }
                    });
                    glib::Propagation::Stop
                }
                // M for mute toggle
                gdk::Key::m | gdk::Key::M => {
                    if controls_for_key.volume_button.value() > 0.0 {
                        controls_for_key.volume_button.set_value(0.0);
                    } else {
                        controls_for_key.volume_button.set_value(1.0);
                    }
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed
            }
        });
        
        // Add key controller to the overlay
        overlay.add_controller(key_controller);
        
        widget.append(&overlay);
        
        info!("PlayerPage::new() - Player page initialization complete");
        
        Self {
            widget,
            player,
            controls,
            overlay,
            video_container,
            current_stream_info: Arc::new(RwLock::new(None)),
        }
    }
    
    pub async fn load_media(&self, media_item: &MediaItem, state: Arc<AppState>) -> anyhow::Result<()> {
        info!("PlayerPage::load_media() - Starting to load media: {}", media_item.title());
        info!("PlayerPage::load_media() - Media ID: {}", media_item.id());
        
        // Get the backend manager
        debug!("PlayerPage::load_media() - Getting backend manager");
        let backend_manager = state.backend_manager.read().await;
        
        if let Some((backend_id, backend)) = backend_manager.get_active_backend() {
            info!("PlayerPage::load_media() - Using backend: {}", backend_id);
            // Get stream URL from backend
            debug!("PlayerPage::load_media() - Requesting stream URL from backend");
            let stream_info = backend.get_stream_url(media_item.id()).await?;
            info!("PlayerPage::load_media() - Got stream URL: {}", stream_info.url);
            debug!("PlayerPage::load_media() - Stream info: resolution={}x{}, bitrate={}, codec={}", 
                stream_info.resolution.width, stream_info.resolution.height, 
                stream_info.bitrate, stream_info.video_codec);
            
            // Store stream info for quality selection
            *self.current_stream_info.write().await = Some(stream_info.clone());
            
            // Clear any existing video widget first
            debug!("PlayerPage::load_media() - Clearing existing video widgets");
            while let Some(child) = self.video_container.first_child() {
                self.video_container.remove(&child);
            }
            info!("PlayerPage::load_media() - Existing widgets cleared");
            
            // Create video widget
            debug!("PlayerPage::load_media() - Creating video widget");
            let mut player = self.player.write().await;
            let video_widget = player.create_video_widget();
            info!("PlayerPage::load_media() - Video widget created");
            
            // Add video widget to container
            debug!("PlayerPage::load_media() - Adding video widget to container");
            self.video_container.append(&video_widget);
            info!("PlayerPage::load_media() - Video widget added to container");
            
            // Load the media (sink is already set up in create_video_widget)
            debug!("PlayerPage::load_media() - Loading media into player");
            player.load_media(&stream_info.url, None).await?;
            info!("PlayerPage::load_media() - Media loaded into player");
            
            // Update controls with media info and stream options
            debug!("PlayerPage::load_media() - Updating controls with media info");
            self.controls.set_media_info(media_item.title(), Some(&stream_info)).await;
            
            // Start playback
            debug!("PlayerPage::load_media() - Starting playback");
            player.play().await?;
            info!("PlayerPage::load_media() - Playback started successfully");
        } else {
            error!("PlayerPage::load_media() - No active backend found!");
            return Err(anyhow::anyhow!("No active backend available"));
        }
        
        info!("PlayerPage::load_media() - Media loading complete");
        Ok(())
    }
    
    pub fn widget(&self) -> &gtk4::Box {
        &self.widget
    }
    
    pub async fn stop(&self) {
        debug!("PlayerPage::stop() - Stopping player");
        let player = self.player.read().await;
        if let Err(e) = player.stop().await {
            error!("PlayerPage::stop() - Failed to stop player: {}", e);
        } else {
            info!("PlayerPage::stop() - Player stopped");
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
    rewind_button: gtk4::Button,
    forward_button: gtk4::Button,
    progress_bar: gtk4::Scale,
    volume_button: gtk4::Scale,
    fullscreen_button: gtk4::Button,
    audio_button: gtk4::MenuButton,
    subtitle_button: gtk4::MenuButton,
    quality_button: gtk4::MenuButton,
    title_label: gtk4::Label,
    time_label: gtk4::Label,
    end_time_label: gtk4::Label,
    time_display_mode: Arc<RwLock<TimeDisplayMode>>,
    player: Arc<RwLock<GStreamerPlayer>>,
    is_seeking: Arc<RwLock<bool>>,
}

#[derive(Clone, Copy, Debug)]
enum TimeDisplayMode {
    TotalDuration,   // Shows total duration (e.g., "1:45:00")
    TimeRemaining,   // Shows time remaining (e.g., "-45:00")
    EndTime,         // Shows when it will end (e.g., "11:45 PM")
}

impl PlayerControls {
    fn new(player: Arc<RwLock<GStreamerPlayer>>) -> Self {
        // Main controls container - minimalistic and tight
        let widget = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(8)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::End)
            .margin_bottom(20)
            .margin_start(20)
            .margin_end(20)
            .width_request(600)
            .build();
        widget.add_css_class("player-controls");
        widget.add_css_class("osd");
        widget.add_css_class("minimal");
        
        // Add custom CSS for minimalistic look
        let css_provider = gtk4::CssProvider::new();
        css_provider.load_from_string(
            ".player-controls.minimal {
                background-color: rgba(0, 0, 0, 0.75);
                border-radius: 10px;
                padding: 12px 16px;
                box-shadow: 0 2px 8px rgba(0, 0, 0, 0.4);
            }
            
            .player-controls .progress-bar {
                min-height: 4px;
            }
            
            .player-controls .progress-bar trough {
                background-color: rgba(255, 255, 255, 0.2);
                border-radius: 2px;
            }
            
            .player-controls .progress-bar highlight {
                background-color: rgba(255, 255, 255, 0.9);
                border-radius: 2px;
            }
            
            .player-controls .dim-label {
                font-size: 0.85em;
                color: rgba(255, 255, 255, 0.8);
            }
            
            .player-controls button.flat {
                min-width: 32px;
                min-height: 32px;
                padding: 2px;
                margin: 0;
                color: rgba(255, 255, 255, 0.9);
            }
            
            .player-controls button.flat:hover {
                background-color: rgba(255, 255, 255, 0.1);
            }
            
            .player-controls button.circular {
                border-radius: 50%;
            }"
        );
        
        if let Some(display) = gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &css_provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        
        // Hidden title label (kept for compatibility but not shown)
        let title_label = gtk4::Label::new(None);
        title_label.set_visible(false);
        
        // Progress bar with time labels
        let progress_container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .build();
        
        // Current time label (left side)
        let time_label = gtk4::Label::new(Some("0:00"));
        time_label.add_css_class("dim-label");
        time_label.set_width_request(45);
        progress_container.append(&time_label);
        
        // Progress bar
        let progress_bar = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 100.0, 0.1);
        progress_bar.set_draw_value(false);
        progress_bar.add_css_class("progress-bar");
        progress_bar.set_hexpand(true);
        progress_container.append(&progress_bar);
        
        // End time label (right side) - clickable to cycle modes
        let end_time_label = gtk4::Label::new(Some("0:00"));
        end_time_label.add_css_class("dim-label");
        end_time_label.set_width_request(65);
        end_time_label.set_tooltip_text(Some("Click to cycle time display"));
        
        // Make end time label clickable
        let end_time_button = gtk4::Button::new();
        end_time_button.set_child(Some(&end_time_label));
        end_time_button.add_css_class("flat");
        progress_container.append(&end_time_button);
        
        widget.append(&progress_container);
        
        // Main controls row with three sections
        let controls_row = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(0)
            .build();
        
        // Left section: Volume control
        let left_section = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .hexpand(true)
            .halign(gtk4::Align::Start)
            .spacing(4)
            .build();
        
        let volume_button = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 1.0, 0.01);
        volume_button.set_value(1.0);
        volume_button.set_draw_value(false);
        volume_button.set_size_request(70, -1);
        left_section.append(&volume_button);
        
        controls_row.append(&left_section);
        
        // Center section: Playback controls
        let center_section = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(4)
            .halign(gtk4::Align::Center)
            .build();
        
        // Rewind button (seek backward 10s)
        let rewind_button = gtk4::Button::from_icon_name("media-seek-backward-symbolic");
        rewind_button.add_css_class("flat");
        center_section.append(&rewind_button);
        
        // Play/pause button (center, slightly larger)
        let play_button = gtk4::Button::from_icon_name("media-playback-start-symbolic");
        play_button.add_css_class("circular");
        play_button.set_size_request(40, 40);
        center_section.append(&play_button);
        
        // Forward button (seek forward 10s)
        let forward_button = gtk4::Button::from_icon_name("media-seek-forward-symbolic");
        forward_button.add_css_class("flat");
        center_section.append(&forward_button);
        
        controls_row.append(&center_section);
        
        // Right section: Track selection and fullscreen
        let right_section = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .hexpand(true)
            .halign(gtk4::Align::End)
            .spacing(2)
            .build();
        
        // Audio tracks button
        let audio_button = gtk4::MenuButton::new();
        audio_button.set_icon_name("audio-x-generic-symbolic");
        audio_button.add_css_class("flat");
        audio_button.set_tooltip_text(Some("Audio Track"));
        right_section.append(&audio_button);
        
        // Subtitle tracks button
        let subtitle_button = gtk4::MenuButton::new();
        subtitle_button.set_icon_name("media-view-subtitles-symbolic");
        subtitle_button.add_css_class("flat");
        subtitle_button.set_tooltip_text(Some("Subtitles"));
        right_section.append(&subtitle_button);
        
        // Quality/Resolution button
        let quality_button = gtk4::MenuButton::new();
        quality_button.set_icon_name("preferences-system-symbolic");
        quality_button.add_css_class("flat");
        quality_button.set_tooltip_text(Some("Video Quality"));
        right_section.append(&quality_button);
        
        // Fullscreen button
        let fullscreen_button = gtk4::Button::from_icon_name("view-fullscreen-symbolic");
        fullscreen_button.add_css_class("flat");
        right_section.append(&fullscreen_button);
        
        controls_row.append(&right_section);
        
        widget.append(&controls_row);
        
        let controls = Self {
            widget,
            play_button: play_button.clone(),
            rewind_button: rewind_button.clone(),
            forward_button: forward_button.clone(),
            progress_bar: progress_bar.clone(),
            volume_button: volume_button.clone(),
            fullscreen_button: fullscreen_button.clone(),
            audio_button: audio_button.clone(),
            subtitle_button: subtitle_button.clone(),
            quality_button: quality_button.clone(),
            title_label,
            time_label: time_label.clone(),
            end_time_label: end_time_label.clone(),
            time_display_mode: Arc::new(RwLock::new(TimeDisplayMode::TotalDuration)),
            player: player.clone(),
            is_seeking: Arc::new(RwLock::new(false)),
        };
        
        // Set up click handler for end time label to cycle display modes
        let mode = controls.time_display_mode.clone();
        end_time_button.connect_clicked(move |_| {
            let mode = mode.clone();
            glib::spawn_future_local(async move {
                let mut current_mode = mode.write().await;
                *current_mode = match *current_mode {
                    TimeDisplayMode::TotalDuration => TimeDisplayMode::TimeRemaining,
                    TimeDisplayMode::TimeRemaining => TimeDisplayMode::EndTime,
                    TimeDisplayMode::EndTime => TimeDisplayMode::TotalDuration,
                };
                debug!("Time display mode changed to: {:?}", *current_mode);
            });
        });
        
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
        
        // Rewind button (seek backward 10s)
        let player = self.player.clone();
        self.rewind_button.connect_clicked(move |_| {
            let player = player.clone();
            glib::spawn_future_local(async move {
                let player = player.read().await;
                if let Some(position) = player.get_position().await {
                    let new_position = position.saturating_sub(Duration::from_secs(10));
                    if let Err(e) = player.seek(new_position).await {
                        error!("Failed to seek backward: {}", e);
                    }
                }
            });
        });
        
        // Forward button (seek forward 10s)
        let player = self.player.clone();
        self.forward_button.connect_clicked(move |_| {
            let player = player.clone();
            glib::spawn_future_local(async move {
                let player = player.read().await;
                if let Some(position) = player.get_position().await {
                    let new_position = position + Duration::from_secs(10);
                    if let Err(e) = player.seek(new_position).await {
                        error!("Failed to seek forward: {}", e);
                    }
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
        let end_time_label = self.end_time_label.clone();
        let is_seeking = self.is_seeking.clone();
        let time_display_mode = self.time_display_mode.clone();
        
        glib::timeout_add_local(Duration::from_millis(500), move || {
            let player = player.clone();
            let progress_bar = progress_bar.clone();
            let time_label = time_label.clone();
            let end_time_label = end_time_label.clone();
            let is_seeking = is_seeking.clone();
            let time_display_mode = time_display_mode.clone();
            
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
                    
                    // Update current time label (always shows current position)
                    let pos_str = format_duration(position);
                    time_label.set_text(&pos_str);
                    
                    // Update end time label based on display mode
                    let mode = *time_display_mode.read().await;
                    let end_str = match mode {
                        TimeDisplayMode::TotalDuration => {
                            format_duration(duration)
                        }
                        TimeDisplayMode::TimeRemaining => {
                            let remaining = duration.saturating_sub(position);
                            format!("-{}", format_duration(remaining))
                        }
                        TimeDisplayMode::EndTime => {
                            // Calculate when the video will end
                            let remaining = duration.saturating_sub(position);
                            let now = chrono::Local::now();
                            let end_time = now + chrono::Duration::from_std(remaining).unwrap_or_default();
                            end_time.format("%-I:%M %p").to_string()
                        }
                    };
                    end_time_label.set_text(&end_str);
                }
            });
            
            glib::ControlFlow::Continue
        });
    }
    
    async fn set_media_info(&self, title: &str, stream_info: Option<&crate::models::StreamInfo>) {
        debug!("PlayerControls::set_media_info() - Setting media title: {}", title);
        self.title_label.set_text(title);
        
        // Skip populating track menus for now - they require the playbin to be in Playing state
        // We'll populate them after playback starts
        debug!("PlayerControls::set_media_info() - Skipping track menu population (will do after playback starts)");
        // self.populate_track_menus().await;
        
        // Populate quality menu if stream info is available
        if let Some(info) = stream_info {
            debug!("PlayerControls::set_media_info() - Populating quality menu");
            self.populate_quality_menu(info).await;
            debug!("PlayerControls::set_media_info() - Quality menu populated");
        }
        
        info!("PlayerControls::set_media_info() - Media info set successfully");
    }
    
    async fn populate_track_menus(&self) {
        // Create audio tracks menu
        let audio_menu = gio::Menu::new();
        let audio_tracks = self.player.read().await.get_audio_tracks().await;
        let current_audio = self.player.read().await.get_current_audio_track().await;
        
        for (index, name) in audio_tracks {
            let action_name = format!("player.set-audio-track-{}", index);
            audio_menu.append(Some(&name), Some(&action_name));
        }
        
        let audio_popover = gtk4::PopoverMenu::from_model(Some(&audio_menu));
        self.audio_button.set_popover(Some(&audio_popover));
        
        // Create subtitle tracks menu
        let subtitle_menu = gio::Menu::new();
        let subtitle_tracks = self.player.read().await.get_subtitle_tracks().await;
        let current_subtitle = self.player.read().await.get_current_subtitle_track().await;
        
        for (index, name) in subtitle_tracks {
            let action_name = if index < 0 {
                "player.disable-subtitles".to_string()
            } else {
                format!("player.set-subtitle-track-{}", index)
            };
            subtitle_menu.append(Some(&name), Some(&action_name));
        }
        
        let subtitle_popover = gtk4::PopoverMenu::from_model(Some(&subtitle_menu));
        self.subtitle_button.set_popover(Some(&subtitle_popover));
        
        // Set up actions for track selection
        self.setup_track_actions().await;
    }
    
    async fn setup_track_actions(&self) {
        // Get the action group from the widget's root
        if let Some(window) = self.widget.root() {
            let action_group = gio::SimpleActionGroup::new();
            
            // Add audio track actions
            let audio_tracks = self.player.read().await.get_audio_tracks().await;
            for (index, _name) in audio_tracks {
                let action = gio::SimpleAction::new(
                    &format!("set-audio-track-{}", index),
                    None
                );
                let player = self.player.clone();
                action.connect_activate(move |_, _| {
                    let player = player.clone();
                    glib::spawn_future_local(async move {
                        if let Err(e) = player.read().await.set_audio_track(index).await {
                            error!("Failed to set audio track: {}", e);
                        }
                    });
                });
                action_group.add_action(&action);
            }
            
            // Add subtitle track actions
            let subtitle_tracks = self.player.read().await.get_subtitle_tracks().await;
            for (index, _name) in subtitle_tracks {
                if index < 0 {
                    let action = gio::SimpleAction::new("disable-subtitles", None);
                    let player = self.player.clone();
                    action.connect_activate(move |_, _| {
                        let player = player.clone();
                        glib::spawn_future_local(async move {
                            if let Err(e) = player.read().await.set_subtitle_track(-1).await {
                                error!("Failed to disable subtitles: {}", e);
                            }
                        });
                    });
                    action_group.add_action(&action);
                } else {
                    let action = gio::SimpleAction::new(
                        &format!("set-subtitle-track-{}", index),
                        None
                    );
                    let player = self.player.clone();
                    action.connect_activate(move |_, _| {
                        let player = player.clone();
                        glib::spawn_future_local(async move {
                            if let Err(e) = player.read().await.set_subtitle_track(index).await {
                                error!("Failed to set subtitle track: {}", e);
                            }
                        });
                    });
                    action_group.add_action(&action);
                }
            }
            
            window.insert_action_group("player", Some(&action_group));
        }
    }
    
    async fn populate_quality_menu(&self, stream_info: &crate::models::StreamInfo) {
        debug!("PlayerControls::populate_quality_menu() - Starting");
        // Create quality menu
        let quality_menu = gio::Menu::new();
        
        // Add quality options from stream info
        debug!("PlayerControls::populate_quality_menu() - Found {} quality options", stream_info.quality_options.len());
        for (index, option) in stream_info.quality_options.iter().enumerate() {
            let action_name = format!("player.set-quality-{}", index);
            let label = if option.requires_transcode {
                format!("{} (Transcode)", option.name)
            } else {
                option.name.clone()
            };
            quality_menu.append(Some(&label), Some(&action_name));
        }
        
        // If no quality options, add current quality
        if stream_info.quality_options.is_empty() {
            let label = format!("{}p", stream_info.resolution.height);
            quality_menu.append(Some(&label), None);
        }
        
        let quality_popover = gtk4::PopoverMenu::from_model(Some(&quality_menu));
        self.quality_button.set_popover(Some(&quality_popover));
        
        // Set up actions for quality selection
        debug!("PlayerControls::populate_quality_menu() - Getting window root");
        if let Some(window) = self.widget.root() {
            debug!("PlayerControls::populate_quality_menu() - Got window root, setting up actions");
            let action_group = gio::SimpleActionGroup::new();
            
            for (index, option) in stream_info.quality_options.iter().enumerate() {
                let action = gio::SimpleAction::new(
                    &format!("set-quality-{}", index),
                    None
                );
                let player = self.player.clone();
                let url = option.url.clone();
                action.connect_activate(move |_, _| {
                    let player = player.clone();
                    let url = url.clone();
                    glib::spawn_future_local(async move {
                        // Get current position before switching
                        let position = {
                            let player = player.read().await;
                            player.get_position().await
                        };
                        
                        // Load new quality URL
                        let player = player.read().await;
                        if let Err(e) = player.load_media(&url, None).await {
                            error!("Failed to switch quality: {}", e);
                            return;
                        }
                        
                        // Seek to previous position if available
                        if let Some(pos) = position {
                            if let Err(e) = player.seek(pos).await {
                                error!("Failed to seek after quality switch: {}", e);
                            }
                        }
                        
                        // Resume playback
                        if let Err(e) = player.play().await {
                            error!("Failed to resume playback: {}", e);
                        }
                    });
                });
                action_group.add_action(&action);
            }
            
            window.insert_action_group("player", Some(&action_group));
            debug!("PlayerControls::populate_quality_menu() - Actions set up successfully");
        } else {
            debug!("PlayerControls::populate_quality_menu() - Window root not available yet, skipping action setup");
        }
        
        debug!("PlayerControls::populate_quality_menu() - Complete");
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