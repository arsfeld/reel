use chrono;
use gdk::prelude::ToplevelExt;
use gtk4::{self, gdk, gio, glib, prelude::*};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace};

use crate::backends::traits::MediaBackend;
use crate::config::Config;
use crate::constants::PLAYER_CONTROLS_HIDE_DELAY_SECS;
use crate::models::{Episode, MediaItem, Movie};
use crate::platforms::gtk::ui::viewmodels::player_view_model::PlayerViewModel;
use crate::platforms::gtk::ui::widgets::player_overlay::ReelPlayerOverlayHost;
use crate::player::Player;
use crate::state::AppState;

#[derive(Clone)]
pub struct PlayerPage {
    widget: gtk4::Box,
    player: Arc<RwLock<Player>>,
    controls: PlayerControls,
    overlay: gtk4::Overlay,
    video_container: gtk4::Box,
    controls_container: gtk4::Box,
    top_left_osd: gtk4::Box,
    top_right_osd: gtk4::Box,
    back_button: gtk4::Button,
    close_button: gtk4::Button,
    current_stream_info: Arc<RwLock<Option<crate::models::StreamInfo>>>,
    current_media_item: Arc<RwLock<Option<MediaItem>>>,
    state: Arc<AppState>,
    hover_controller: Rc<gtk4::EventControllerMotion>,
    inhibit_cookie: Arc<RwLock<Option<u32>>>,
    skip_intro_button: gtk4::Button,
    skip_credits_button: gtk4::Button,
    auto_play_overlay: gtk4::Box,
    pip_container: gtk4::Box,
    next_episode_info: Arc<RwLock<Option<Episode>>>,
    auto_play_countdown: Arc<RwLock<Option<glib::SourceId>>>,
    chapter_monitor_id: Arc<RwLock<Option<glib::SourceId>>>,
    config: Config,
    position_sync_timer: Arc<RwLock<Option<glib::SourceId>>>,
    last_synced_position: Arc<RwLock<Option<Duration>>>,
    loading_overlay: gtk4::Box,
    loading_spinner: gtk4::Spinner,
    loading_label: gtk4::Label,
    error_overlay: gtk4::Box,
    error_label: gtk4::Label,
    view_model: Arc<PlayerViewModel>,
}

impl std::fmt::Debug for PlayerPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerPage")
            .field("widget", &"gtk4::Box")
            .field("player", &"Arc<RwLock<Player>>")
            .finish()
    }
}

impl PlayerPage {
    pub async fn get_backend_type(&self) -> String {
        let player = self.player.read().await;
        match &*player {
            Player::GStreamer(_) => "gstreamer".to_string(),
            Player::Mpv(_) => "mpv".to_string(),
        }
    }

    pub async fn cleanup(&self) {
        // Stop playback and cleanup resources
        info!("PlayerPage::cleanup() - Cleaning up player resources");

        // Stop any ongoing playback
        if let Ok(player) = self.player.try_read() {
            let _ = player.stop().await;
        }

        // Cancel any timers
        if let Some(timer) = self.position_sync_timer.write().await.take() {
            timer.remove();
        }

        if let Some(timer) = self.auto_play_countdown.write().await.take() {
            timer.remove();
        }

        if let Some(timer) = self.chapter_monitor_id.write().await.take() {
            timer.remove();
        }

        // Uninhibit screensaver
        self.uninhibit_suspend().await;

        info!("PlayerPage::cleanup() - Cleanup complete");
    }

    async fn seek_with_retries(&self, position: Duration) {
        use std::time::Duration as StdDuration;
        let max_attempts = 8;
        let mut attempt = 0;
        // Backoff sequence in ms
        let delays = [150, 250, 400, 600, 800, 1000, 1200, 1500];

        loop {
            attempt += 1;
            let player = self.player.read().await;
            match player.seek(position).await {
                Ok(_) => {
                    info!(
                        "Seek successful on attempt {} at position {}s",
                        attempt,
                        position.as_secs()
                    );
                    break;
                }
                Err(e) => {
                    if attempt >= max_attempts {
                        error!("Failed to seek after {} attempts: {}", attempt, e);
                        break;
                    } else {
                        debug!(
                            "Seek attempt {} failed: {}. Retrying in {}ms",
                            attempt,
                            e,
                            delays[attempt - 1]
                        );
                        drop(player);
                        glib::timeout_future(StdDuration::from_millis(delays[attempt - 1] as u64))
                            .await;
                        continue;
                    }
                }
            }
        }
    }
    pub fn new(state: Arc<AppState>) -> Self {
        info!("PlayerPage::new() - Creating new player page");
        // Create main container
        let widget = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .build();
        widget.add_css_class("player-page");
        debug!("PlayerPage::new() - Created main widget container");

        // Create host widget from CompositeTemplate and fetch children
        let host = ReelPlayerOverlayHost::new();
        let overlay = host.overlay().clone();
        let video_container = host.video_container().clone();
        let controls_container = host.controls_container().clone();
        let top_left_osd = host.top_left_osd().clone();
        let top_right_osd = host.top_right_osd().clone();
        let back_button = host.back_button().clone();
        let close_button = host.close_button().clone();

        // Create player based on config from AppState
        info!("PlayerPage::new() - Creating player");
        let config_arc = state.config.clone();
        let config = tokio::task::block_in_place(|| {
            let config_guard = tokio::runtime::Handle::current().block_on(config_arc.read());
            config_guard.clone()
        });
        info!(
            "PlayerPage::new() - Using player backend: {}",
            config.playback.player_backend
        );
        let player = Arc::new(RwLock::new(
            Player::new(&config).expect("Failed to create player"),
        ));
        info!("PlayerPage::new() - Player created successfully");

        // Create inhibit cookie that will be shared with controls
        let inhibit_cookie = Arc::new(RwLock::new(None));

        // Grab overlay elements from blueprint
        let skip_intro_button = host.skip_intro_button().clone();
        let skip_credits_button = host.skip_credits_button().clone();
        let auto_play_overlay = host.auto_play_overlay().clone();
        let pip_container = host.pip_container().clone();
        let play_now_button = host.play_now_button().clone();
        let cancel_button = host.cancel_button().clone();

        // Wire up auto-play actions to move video back
        let auto_play_overlay_for_play = auto_play_overlay.clone();
        let pip_for_play = pip_container.clone();
        let video_for_play = video_container.clone();
        play_now_button.connect_clicked(move |_| {
            info!("Play Now clicked - would load next episode");
            auto_play_overlay_for_play.set_visible(false);

            if let Some(video_widget) = pip_for_play.first_child() {
                pip_for_play.remove(&video_widget);
                video_widget.set_size_request(-1, -1);
                video_for_play.append(&video_widget);
            }
        });

        let auto_play_overlay_for_cancel = auto_play_overlay.clone();
        let pip_for_cancel = pip_container.clone();
        let video_for_cancel = video_container.clone();
        cancel_button.connect_clicked(move |_| {
            info!("Cancel auto-play clicked");
            auto_play_overlay_for_cancel.set_visible(false);

            if let Some(video_widget) = pip_for_cancel.first_child() {
                pip_for_cancel.remove(&video_widget);
                video_widget.set_size_request(-1, -1);
                video_for_cancel.append(&video_widget);
            }
        });

        // Loading and error overlays from blueprint
        let loading_overlay = host.loading_overlay().clone();
        let loading_spinner = host.loading_spinner().clone();
        let loading_label = host.loading_label().clone();

        let error_overlay = host.error_overlay().clone();
        let error_label = host.error_label().clone();
        let retry_button = host.retry_button().clone();

        // Connect retry button to go back to the previous page
        let error_overlay_for_retry = error_overlay.clone();
        let widget_for_retry = widget.clone();
        retry_button.connect_clicked(move |_| {
            error_overlay_for_retry.set_visible(false);
            // Navigate back
            if let Some(window) = widget_for_retry
                .root()
                .and_then(|r| r.downcast::<gtk4::Window>().ok())
            {
                window.close();
            }
        });

        // Initialize PlayerViewModel with app state and event bus (create early so controls can use it)
        let data_service = state.data_service.clone();
        let view_model = Arc::new(PlayerViewModel::new(
            data_service,
            state.clone(),
            state.event_bus.clone(),
        ));

        // Initialize ViewModel subscriptions
        glib::spawn_future_local({
            let vm = view_model.clone();
            let event_bus = state.event_bus.clone();
            async move {
                use crate::platforms::gtk::ui::viewmodels::ViewModel;
                vm.initialize(event_bus).await;
            }
        });

        // Create controls (backend and media item will be set when loading media)
        let controls = PlayerControls::new(
            player.clone(),
            inhibit_cookie.clone(),
            Arc::new(RwLock::new(None)),
            Arc::new(RwLock::new(None)),
        );
        // Inject controls widget into blueprint container
        controls_container.append(&controls.widget);

        // Set up hover detection for showing/hiding OSD (controls + corner buttons)
        let controls_container_for_hover = controls_container.clone();
        let top_left_osd_for_hover = top_left_osd.clone();
        let top_right_osd_for_hover = top_right_osd.clone();
        let hide_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let hover_controller = gtk4::EventControllerMotion::new();
        let widget_for_motion = widget.clone();

        let hide_timer_clone = hide_timer.clone();
        hover_controller.connect_motion(move |_, _, _| {
            // Fade in OSD quickly (200ms)
            controls_container_for_hover.set_visible(true);
            controls_container_for_hover.set_opacity(1.0);
            top_left_osd_for_hover.set_visible(true);
            top_left_osd_for_hover.set_opacity(1.0);
            top_right_osd_for_hover.set_visible(true);
            top_right_osd_for_hover.set_opacity(1.0);

            // Show cursor on movement while in fullscreen
            if let Some(window) = widget_for_motion
                .root()
                .and_then(|r| r.downcast::<gtk4::Window>().ok())
                && window.is_fullscreen()
            {
                // Restore default cursor
                if let Some(cursor) = gdk::Cursor::from_name("default", None) {
                    widget_for_motion.set_cursor(Some(&cursor));
                } else {
                    widget_for_motion.set_cursor(None);
                }
            }

            // Cancel previous timer if exists
            if let Some(timer_id) = hide_timer_clone.borrow_mut().take() {
                timer_id.remove();
            }

            // Hide again after configured delay of no movement
            let controls_container_inner = controls_container_for_hover.clone();
            let top_left_osd_inner = top_left_osd_for_hover.clone();
            let top_right_osd_inner = top_right_osd_for_hover.clone();
            let hide_timer_inner = hide_timer_clone.clone();
            // Clone widget reference for the timer closure to avoid moving outer capture
            let widget_for_motion_for_timer = widget_for_motion.clone();
            let timer_id = glib::timeout_add_local(
                std::time::Duration::from_secs(PLAYER_CONTROLS_HIDE_DELAY_SECS),
                move || {
                    // Fade out animation
                    let fade_start_time = std::time::Instant::now();
                    let controls_for_fade = controls_container_inner.clone();
                    let top_left_for_fade = top_left_osd_inner.clone();
                    let top_right_for_fade = top_right_osd_inner.clone();
                    let widget_for_fade = widget_for_motion_for_timer.clone();

                    glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
                        let elapsed = fade_start_time.elapsed().as_millis() as f64;
                        let fade_duration = 200.0; // 200ms fade out

                        if elapsed >= fade_duration {
                            controls_for_fade.set_opacity(0.0);
                            controls_for_fade.set_visible(false);
                            top_left_for_fade.set_opacity(0.0);
                            top_left_for_fade.set_visible(false);
                            top_right_for_fade.set_opacity(0.0);
                            top_right_for_fade.set_visible(false);
                            // Hide cursor when controls hidden and in fullscreen
                            if let Some(window) = widget_for_fade
                                .root()
                                .and_then(|r| r.downcast::<gtk4::Window>().ok())
                                && window.is_fullscreen()
                                && let Ok(texture) =
                                    gdk::Texture::from_bytes(&glib::Bytes::from_static(&[0u8; 64]))
                            {
                                let cursor = gdk::Cursor::from_texture(&texture, 0, 0, None);
                                widget_for_fade.set_cursor(Some(&cursor));
                            }
                            glib::ControlFlow::Break
                        } else {
                            let opacity = 1.0 - (elapsed / fade_duration);
                            controls_for_fade.set_opacity(opacity);
                            top_left_for_fade.set_opacity(opacity);
                            top_right_for_fade.set_opacity(opacity);
                            glib::ControlFlow::Continue
                        }
                    });

                    // Clear the timer reference since it's done
                    hide_timer_inner.borrow_mut().take();
                    glib::ControlFlow::Break
                },
            );
            hide_timer_clone.borrow_mut().replace(timer_id);
        });

        // Store the hover controller as we'll add it after playback starts
        let hover_controller_rc = Rc::new(hover_controller);

        // Add keyboard event controller for fullscreen and playback controls
        let key_controller = gtk4::EventControllerKey::new();
        let controls_for_key = controls.clone();
        let overlay_for_key = overlay.clone();

        key_controller.connect_key_pressed(move |controller, keyval, _keycode, _state| {
            match keyval {
                // F or F11 for fullscreen toggle
                gdk::Key::f | gdk::Key::F | gdk::Key::F11 => {
                    // This needs to be handled differently since we can't call self methods here
                    if let Some(widget) = controller.widget()
                        && let Some(window) = widget
                            .root()
                            .and_then(|r| r.downcast::<gtk4::Window>().ok())
                    {
                        if window.is_fullscreen() {
                            window.unfullscreen();
                            controls_for_key
                                .fullscreen_button
                                .set_icon_name("view-fullscreen-symbolic");
                            overlay_for_key.remove_css_class("fullscreen");
                        } else {
                            window.fullscreen();
                            controls_for_key
                                .fullscreen_button
                                .set_icon_name("view-restore-symbolic");
                            overlay_for_key.add_css_class("fullscreen");
                        }
                    }
                    glib::Propagation::Stop
                }
                // Escape to exit fullscreen
                gdk::Key::Escape => {
                    if let Some(widget) = controller.widget()
                        && let Some(window) = widget
                            .root()
                            .and_then(|r| r.downcast::<gtk4::Window>().ok())
                        && window.is_fullscreen()
                    {
                        window.unfullscreen();
                        controls_for_key
                            .fullscreen_button
                            .set_icon_name("view-fullscreen-symbolic");
                        overlay_for_key.remove_css_class("fullscreen");
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
                            let new_position = position + Duration::from_secs(30);
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
                // Q to quit the application
                gdk::Key::q | gdk::Key::Q => {
                    if let Some(widget) = controller.widget()
                        && let Some(window) = widget
                            .root()
                            .and_then(|r| r.downcast::<gtk4::Window>().ok())
                    {
                        window.close();
                    }
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });

        // Add key controller to the overlay
        overlay.add_controller(key_controller);

        // Add double-click gesture for fullscreen toggle
        let double_click_gesture = gtk4::GestureClick::new();
        double_click_gesture.set_button(gdk::BUTTON_PRIMARY);
        let controls_for_double_click = controls.clone();
        let overlay_for_double_click = overlay.clone();

        double_click_gesture.connect_pressed(move |gesture, n_press, _x, _y| {
            if n_press == 2 {
                // Double-click detected
                if let Some(widget) = gesture.widget()
                    && let Some(window) = widget
                        .root()
                        .and_then(|r| r.downcast::<gtk4::Window>().ok())
                {
                    if window.is_fullscreen() {
                        window.unfullscreen();
                        controls_for_double_click
                            .fullscreen_button
                            .set_icon_name("view-fullscreen-symbolic");
                        overlay_for_double_click.remove_css_class("fullscreen");
                    } else {
                        window.fullscreen();
                        controls_for_double_click
                            .fullscreen_button
                            .set_icon_name("view-restore-symbolic");
                        overlay_for_double_click.add_css_class("fullscreen");
                    }
                }
            }
        });

        video_container.add_controller(double_click_gesture);

        // Add drag gesture for moving the window - only on video container, not overlay
        // This prevents it from interfering with control buttons
        let drag_gesture = gtk4::GestureDrag::new();
        drag_gesture.set_button(gdk::BUTTON_PRIMARY); // Left mouse button

        drag_gesture.connect_drag_begin(|gesture, start_x, start_y| {
            // Only start window drag if we're not over a button
            if let Some(widget) = gesture.widget()
                && let Some(window) = widget
                    .root()
                    .and_then(|r| r.downcast::<gtk4::Window>().ok())
            {
                // Start the window drag operation
                if let Some(surface) = window.surface()
                    && let Some(toplevel) = surface.downcast_ref::<gdk::Toplevel>()
                    && let Some(device) = gesture.device()
                {
                    toplevel.begin_move(
                        &device,
                        gdk::BUTTON_PRIMARY as i32,
                        start_x,
                        start_y,
                        gtk4::gdk::CURRENT_TIME,
                    );
                }
            }
        });

        // Add to video_container, not overlay - this way controls remain clickable
        video_container.add_controller(drag_gesture);

        // Use the host as the visual widget for the page
        widget.append(&host);

        info!("PlayerPage::new() - Player page initialization complete");

        // Reactive UI bindings to ViewModel properties

        // is_loading -> show/hide loading overlay
        {
            use crate::platforms::gtk::ui::viewmodels::ViewModel;
            let mut sub = view_model
                .subscribe_to_property("is_loading")
                .unwrap_or_else(|| view_model.is_loading().subscribe());
            let loading_overlay = loading_overlay.clone();
            let error_overlay = error_overlay.clone();
            let controls_container = controls_container.clone();
            let vm = view_model.clone();
            glib::spawn_future_local(async move {
                while sub.wait_for_change().await {
                    let is_loading = vm.is_loading().get().await;
                    if is_loading {
                        loading_overlay.set_visible(true);
                        error_overlay.set_visible(false);
                        controls_container.set_visible(false);
                    } else {
                        loading_overlay.set_visible(false);
                    }
                }
            });
        }

        // error -> show error overlay
        {
            use crate::platforms::gtk::ui::viewmodels::ViewModel;
            let mut sub = view_model
                .subscribe_to_property("error")
                .unwrap_or_else(|| view_model.error().subscribe());
            let error_overlay = error_overlay.clone();
            let error_label = error_label.clone();
            let loading_overlay = loading_overlay.clone();
            let controls_container = controls_container.clone();
            let vm = view_model.clone();
            glib::spawn_future_local(async move {
                while sub.wait_for_change().await {
                    if let Some(msg) = vm.error().get().await {
                        error_label.set_text(&msg);
                        error_overlay.set_visible(true);
                        loading_overlay.set_visible(false);
                        controls_container.set_visible(false);
                    } else {
                        error_overlay.set_visible(false);
                    }
                }
            });
        }

        // Set up controls event handlers and position timer now that VM is available
        controls.setup_handlers(view_model.clone());
        controls.start_position_timer();

        Self {
            widget,
            player,
            controls,
            overlay,
            video_container,
            controls_container,
            top_left_osd,
            top_right_osd,
            back_button,
            close_button,
            current_stream_info: Arc::new(RwLock::new(None)),
            current_media_item: Arc::new(RwLock::new(None)),
            state,
            hover_controller: hover_controller_rc,
            inhibit_cookie,
            skip_intro_button,
            skip_credits_button,
            auto_play_overlay,
            pip_container,
            next_episode_info: Arc::new(RwLock::new(None)),
            auto_play_countdown: Arc::new(RwLock::new(None)),
            chapter_monitor_id: Arc::new(RwLock::new(None)),
            config,
            position_sync_timer: Arc::new(RwLock::new(None)),
            last_synced_position: Arc::new(RwLock::new(None)),
            loading_overlay,
            loading_spinner,
            loading_label,
            error_overlay,
            error_label,
            view_model,
        }
    }

    fn show_loading_state(&self, message: &str) {
        let loading_label = self.loading_label.clone();
        let loading_overlay = self.loading_overlay.clone();
        let error_overlay = self.error_overlay.clone();
        let controls = self.controls_container.clone();
        let msg = message.to_string();

        glib::MainContext::default().spawn_local(async move {
            loading_label.set_text(&msg);
            loading_overlay.set_visible(true);
            error_overlay.set_visible(false);
            controls.set_visible(false);
        });
    }

    fn show_error_state(&self, message: &str) {
        let error_label = self.error_label.clone();
        let error_overlay = self.error_overlay.clone();
        let loading_overlay = self.loading_overlay.clone();
        let controls = self.controls_container.clone();
        let msg = message.to_string();

        glib::MainContext::default().spawn_local(async move {
            error_label.set_text(&msg);
            error_overlay.set_visible(true);
            loading_overlay.set_visible(false);
            controls.set_visible(false);
        });
    }

    fn hide_overlays(&self) {
        let loading_overlay = self.loading_overlay.clone();
        let error_overlay = self.error_overlay.clone();
        let controls = self.controls_container.clone();

        glib::MainContext::default().spawn_local(async move {
            loading_overlay.set_visible(false);
            error_overlay.set_visible(false);
            controls.set_visible(true);
        });
    }

    pub async fn load_media(
        &self,
        media_item: &MediaItem,
        state: Arc<AppState>,
    ) -> anyhow::Result<()> {
        info!(
            "PlayerPage::load_media() - Starting to load media: {}",
            media_item.title()
        );
        info!("PlayerPage::load_media() - Media ID: {}", media_item.id());

        // Show loading state
        self.show_loading_state("Loading media...");

        // Store the current media item
        *self.current_media_item.write().await = Some(media_item.clone());

        // Update controls' media item reference
        *self.controls.current_media_item.write().await = Some(media_item.clone());

        // Get the backend for this media item
        let backend_id = media_item.backend_id();
        debug!("PlayerPage::load_media() - Getting backend: {}", backend_id);

        if let Some(backend) = state.source_coordinator.get_backend(backend_id).await {
            info!("PlayerPage::load_media() - Using backend: {}", backend_id);

            // Update controls' backend reference
            *self.controls.backend.write().await = Some(backend.clone());

            // Use ViewModel to resolve stream URL and markers
            self.show_loading_state("Fetching stream URL...");
            self.view_model.set_media_item(media_item.clone()).await;
            if let Err(e) = self.view_model.load_stream_and_metadata().await {
                // Prefer VM error message if present
                if let Some(msg) = self.view_model.error().get().await {
                    self.show_error_state(&msg);
                } else {
                    self.show_error_state("Failed to load media from server");
                }
                return Err(e);
            }
            // Retrieve stream info from VM
            let stream_info = match self.view_model.stream_info().get().await {
                Some(info) => info,
                None => {
                    let err = anyhow::anyhow!("Stream information unavailable");
                    error!("PlayerPage::load_media() - VM did not provide stream info");
                    self.show_error_state("Failed to load media stream");
                    return Err(err);
                }
            };
            info!(
                "PlayerPage::load_media() - Got stream URL: {}",
                stream_info.url
            );
            debug!(
                "PlayerPage::load_media() - Stream info: resolution={}x{}, bitrate={}, codec={}",
                stream_info.resolution.width,
                stream_info.resolution.height,
                stream_info.bitrate,
                stream_info.video_codec
            );

            // Store stream info for quality selection
            *self.current_stream_info.write().await = Some(stream_info.clone());

            // Update loading message
            self.show_loading_state("Preparing video player...");

            // Clear any existing video widget first
            debug!("PlayerPage::load_media() - Clearing existing video widgets");
            while let Some(child) = self.video_container.first_child() {
                self.video_container.remove(&child);
            }
            info!("PlayerPage::load_media() - Existing widgets cleared");

            // Create video widget
            debug!("PlayerPage::load_media() - Creating video widget");
            let player = self.player.write().await;
            let video_widget = player.create_video_widget();
            info!("PlayerPage::load_media() - Video widget created");

            // Add video widget to container
            debug!("PlayerPage::load_media() - Adding video widget to container");

            // Only use GraphicsOffload for GStreamer backend
            // MPV uses GLArea which manages its own OpenGL context and doesn't work well with offload
            let using_mpv = self.config.playback.player_backend.to_lowercase() == "mpv";

            if using_mpv {
                // Direct append for MPV - it manages its own GL rendering
                debug!("PlayerPage::load_media() - Using direct rendering for MPV player");
                self.video_container.append(&video_widget);
            } else {
                // Use GraphicsOffload for GStreamer for better performance (GTK 4.14+)
                // This offloads video rendering to a dedicated GPU subsurface
                let offload = gtk4::GraphicsOffload::builder()
                    .child(&video_widget)
                    .build();

                // Enable offload - this can reduce CPU usage and improve performance
                offload.set_enabled(gtk4::GraphicsOffloadEnabled::Enabled);

                debug!(
                    "PlayerPage::load_media() - Using GraphicsOffload for GStreamer video rendering"
                );
                info!("GraphicsOffload enabled for improved video performance");
                self.video_container.append(&offload);
            }

            info!("PlayerPage::load_media() - Video widget added to container");

            // Update loading message
            self.show_loading_state("Loading video stream...");

            // Load the media (sink is already set up in create_video_widget)
            debug!("PlayerPage::load_media() - Loading media into player");
            match player.load_media(&stream_info.url).await {
                Ok(_) => {
                    info!("PlayerPage::load_media() - Media loaded into player");
                }
                Err(e) => {
                    error!("Failed to load media into player: {}", e);
                    self.show_error_state(&format!("Failed to play video: {}", e));
                    return Err(e);
                }
            }

            // Do not seek yet; MPV may not have loaded the file. We'll retry after playback starts.

            // Update controls with media info and stream options
            debug!("PlayerPage::load_media() - Updating controls with media info");
            self.controls
                .set_media_info(media_item.title(), Some(&stream_info))
                .await;

            // Start playback
            debug!("PlayerPage::load_media() - Starting playback");
            self.show_loading_state("Starting playback...");
            player.play().await?;
            info!("PlayerPage::load_media() - Playback started successfully");

            // Hide loading overlay now that playback has started
            self.hide_overlays();

            // Start position sync timer
            self.start_position_sync_timer().await;

            // Resume from saved position with retries for MPV/slow backends
            let resume_position = match media_item {
                MediaItem::Movie(movie) => movie.playback_position,
                MediaItem::Episode(episode) => episode.playback_position,
                _ => None,
            };

            if let Some(position) = resume_position {
                info!(
                    "PlayerPage::load_media() - Resuming from saved position: {:?} ({}s) with retries",
                    position,
                    position.as_secs()
                );
                self.seek_with_retries(position).await;
            } else {
                debug!("PlayerPage::load_media() - No saved position, starting from beginning");
            }

            // Update play button to show pause icon since we're now playing
            self.controls
                .play_button
                .set_icon_name("media-playback-pause-symbolic");

            // Add the hover controller after a delay to prevent initial control flash
            let overlay = self.overlay.clone();
            let hover_controller = self.hover_controller.clone();
            // Clone fields needed for initial idle auto-hide
            let controls_for_idle_init = self.controls_container.clone();
            let tl_for_idle_init = self.top_left_osd.clone();
            let tr_for_idle_init = self.top_right_osd.clone();
            let widget_for_idle_init = self.widget.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(1000), move || {
                // Check if the controller's widget is null before adding
                // This prevents the "controller already has a widget" assertion error
                if gtk4::prelude::EventControllerExt::widget(&*hover_controller).is_none() {
                    overlay.add_controller(hover_controller.as_ref().clone());
                }
                // Schedule an initial auto-hide if user is idle after entering playback/fullscreen
                let controls = controls_for_idle_init.clone();
                let tl = tl_for_idle_init.clone();
                let tr = tr_for_idle_init.clone();
                let widget_for_idle = widget_for_idle_init.clone();
                glib::timeout_add_local(
                    std::time::Duration::from_secs(PLAYER_CONTROLS_HIDE_DELAY_SECS),
                    move || {
                        // Only hide if currently fullscreen to avoid confusing windowed mode
                        if let Some(window) = widget_for_idle
                            .root()
                            .and_then(|r| r.downcast::<gtk4::Window>().ok())
                            && window.is_fullscreen()
                        {
                            controls.set_opacity(0.0);
                            controls.set_visible(false);
                            tl.set_opacity(0.0);
                            tl.set_visible(false);
                            tr.set_opacity(0.0);
                            tr.set_visible(false);

                            // Hide cursor on initial idle
                            if let Ok(texture) =
                                gdk::Texture::from_bytes(&glib::Bytes::from_static(&[0u8; 64]))
                            {
                                let cursor = gdk::Cursor::from_texture(&texture, 0, 0, None);
                                widget_for_idle.set_cursor(Some(&cursor));
                            }
                        }
                        glib::ControlFlow::Break
                    },
                );
                glib::ControlFlow::Break
            });

            // Grab focus on the overlay to ensure keyboard shortcuts work
            self.overlay.grab_focus();

            // Inhibit suspend/screensaver while playing
            self.inhibit_suspend().await;

            // Populate track menus after playback starts (requires Playing state)
            // Add a delay to ensure the playbin has discovered all tracks
            // On macOS, GStreamer may need more time to initialize
            let controls = self.controls.clone();
            let delay_ms = if cfg!(target_os = "macos") { 1500 } else { 500 };
            glib::spawn_future_local(async move {
                debug!(
                    "PlayerPage::load_media() - Waiting {}ms before populating track menus",
                    delay_ms
                );
                glib::timeout_future(std::time::Duration::from_millis(delay_ms)).await;
                debug!("PlayerPage::load_media() - Populating track menus after playback start");
                controls.populate_track_menus().await;
                info!("PlayerPage::load_media() - Track menus populated");
            });

            // Start monitoring for playback completion
            self.monitor_playback_completion(backend_id.to_string(), backend.clone());

            // Setup skip handlers using markers from ViewModel (already fetched)
            match media_item.clone() {
                MediaItem::Episode(mut episode) => {
                    let (intro, credits) = self.view_model.markers().get().await;
                    episode.intro_marker = intro;
                    episode.credits_marker = credits;

                    self.setup_episode_features(episode);
                }
                MediaItem::Movie(mut movie) => {
                    let (intro, credits) = self.view_model.markers().get().await;
                    if intro.is_some() || credits.is_some() {
                        info!(
                            "Markers for movie '{}': intro={:?} credits={:?}",
                            movie.title, intro, credits
                        );
                    } else {
                        info!("No markers found for movie '{}'", movie.title);
                    }
                    movie.intro_marker = intro;
                    movie.credits_marker = credits;

                    self.setup_movie_features(movie);
                }
                _ => {
                    // No marker support for other media types yet
                }
            }
        } else {
            error!(
                "PlayerPage::load_media() - Backend not found for ID: {}",
                backend_id
            );
            return Err(anyhow::anyhow!("Backend not found for ID: {}", backend_id));
        }

        info!("PlayerPage::load_media() - Media loading complete");
        Ok(())
    }

    pub fn widget(&self) -> &gtk4::Box {
        &self.widget
    }

    pub fn set_on_back_clicked<F: Fn() + 'static>(&self, f: F) {
        self.back_button.connect_clicked(move |_| f());
    }

    pub fn set_on_close_clicked<F: Fn() + 'static>(&self, f: F) {
        self.close_button.connect_clicked(move |_| f());
    }

    pub async fn stop(&self) {
        debug!("PlayerPage::stop() - Stopping player");

        // Sync final position before stopping
        self.sync_playback_position().await;

        // Stop the position sync timer
        self.stop_position_sync_timer().await;

        let player = self.player.read().await;
        if let Err(e) = player.stop().await {
            error!("PlayerPage::stop() - Failed to stop player: {}", e);
        } else {
            info!("PlayerPage::stop() - Player stopped");
        }

        // Remove suspend/screensaver inhibit when stopping
        self.uninhibit_suspend().await;
    }

    pub async fn get_video_dimensions(&self) -> Option<(i32, i32)> {
        let player = self.player.read().await;
        player.get_video_dimensions().await
    }

    pub fn toggle_fullscreen(&self) {
        if let Some(window) = self
            .widget
            .root()
            .and_then(|r| r.downcast::<gtk4::Window>().ok())
        {
            if window.is_fullscreen() {
                self.exit_fullscreen(&window);
            } else {
                self.enter_fullscreen(&window);
            }
        }
    }

    fn enter_fullscreen(&self, window: &gtk4::Window) {
        window.fullscreen();
        self.controls
            .fullscreen_button
            .set_icon_name("view-restore-symbolic");

        // Add fullscreen CSS class for special styling
        self.widget.add_css_class("fullscreen");
        self.overlay.add_css_class("fullscreen");

        // Hide cursor after inactivity in fullscreen
        self.setup_cursor_hiding();
    }

    fn exit_fullscreen(&self, window: &gtk4::Window) {
        window.unfullscreen();
        self.controls
            .fullscreen_button
            .set_icon_name("view-fullscreen-symbolic");

        // Remove fullscreen CSS class
        self.widget.remove_css_class("fullscreen");
        self.overlay.remove_css_class("fullscreen");

        // Show cursor
        if let Some(cursor) = gdk::Cursor::from_name("default", None) {
            self.widget.set_cursor(Some(&cursor));
        }
    }

    fn setup_cursor_hiding(&self) {
        // Hide cursor when idle in fullscreen
        let widget = self.widget.clone();
        glib::timeout_add_local(std::time::Duration::from_secs(3), move || {
            if let Some(window) = widget
                .root()
                .and_then(|r| r.downcast::<gtk4::Window>().ok())
            {
                if window.is_fullscreen() {
                    // Create blank cursor to hide it
                    let _display = widget.display();
                    if let Ok(texture) =
                        gdk::Texture::from_bytes(&glib::Bytes::from_static(&[0u8; 64]))
                    {
                        let cursor = gdk::Cursor::from_texture(&texture, 0, 0, None);
                        widget.set_cursor(Some(&cursor));
                    }
                    glib::ControlFlow::Continue
                } else {
                    glib::ControlFlow::Break
                }
            } else {
                glib::ControlFlow::Break
            }
        });
    }

    async fn inhibit_suspend(&self) {
        // Uninhibit any existing inhibit first
        self.uninhibit_suspend().await;

        if let Some(window) = self
            .widget
            .root()
            .and_then(|r| r.downcast::<gtk4::Window>().ok())
            && let Some(app) = window
                .application()
                .and_then(|a| a.downcast::<gtk4::Application>().ok())
        {
            // Inhibit suspend and idle with reason
            let cookie = app.inhibit(
                Some(&window),
                gtk4::ApplicationInhibitFlags::SUSPEND | gtk4::ApplicationInhibitFlags::IDLE,
                Some("Playing video"),
            );

            *self.inhibit_cookie.write().await = Some(cookie);
            info!("Inhibited system suspend/screensaver (cookie: {})", cookie);
        }
    }

    async fn uninhibit_suspend(&self) {
        if let Some(cookie) = self.inhibit_cookie.write().await.take()
            && let Some(window) = self
                .widget
                .root()
                .and_then(|r| r.downcast::<gtk4::Window>().ok())
            && let Some(app) = window
                .application()
                .and_then(|a| a.downcast::<gtk4::Application>().ok())
        {
            app.uninhibit(cookie);
            info!(
                "Removed system suspend/screensaver inhibit (cookie: {})",
                cookie
            );
        }
    }

    fn setup_movie_features(&self, movie: Movie) {
        // Only setup handlers if we have actual markers

        // Setup skip intro button handler if intro marker exists
        if let Some(intro_marker) = movie.intro_marker.as_ref() {
            let player = self.player.clone();
            let button = self.skip_intro_button.clone();
            let intro_end = intro_marker.end_time;

            self.skip_intro_button.connect_clicked(move |_| {
                let player = player.clone();
                let button = button.clone();

                glib::spawn_future_local(async move {
                    // Skip to intro end time
                    let player = player.read().await;
                    if let Err(e) = player.seek(intro_end).await {
                        error!("Failed to skip intro: {}", e);
                    }
                    button.set_visible(false);
                });
            });
        }

        // Setup skip credits button handler (simpler for movies - just skip to end)
        if let Some(credits_marker) = movie.credits_marker.as_ref() {
            let skip_credits = self.skip_credits_button.clone();
            let skip_credits_hide = skip_credits.clone();
            let player = self.player.clone();
            let credits_end = credits_marker.end_time;

            skip_credits.connect_clicked(move |_| {
                let player = player.clone();
                let button = skip_credits_hide.clone();

                glib::spawn_future_local(async move {
                    // For movies, just skip to the end of credits
                    let player = player.read().await;
                    if let Err(e) = player.seek(credits_end).await {
                        error!("Failed to skip credits: {}", e);
                    }
                    button.set_visible(false);
                });
            });
        }

        // Monitor playback position only if we have markers
        let has_markers = movie.intro_marker.is_some() || movie.credits_marker.is_some();

        if has_markers {
            let skip_intro_btn = self.skip_intro_button.clone();
            let skip_credits_btn = self.skip_credits_button.clone();
            let player = self.player.clone();
            let config = self.config.clone();
            let view_model = self.view_model.clone();

            glib::timeout_add_local(Duration::from_millis(500), move || {
                let skip_intro_btn = skip_intro_btn.clone();
                let skip_credits_btn = skip_credits_btn.clone();
                let player = player.clone();
                let config = config.clone();
                // Markers will be read from ViewModel each tick
                let vm = view_model.clone();

                glib::spawn_future_local(async move {
                    let player = player.read().await;
                    if let Some(position) = player.get_position().await {
                        let (intro_marker, credits_marker) = vm.markers().get().await;

                        if config.playback.skip_intro {
                            if let Some(marker) = &intro_marker {
                                if position >= marker.start_time && position < marker.end_time {
                                    skip_intro_btn.set_visible(true);
                                } else {
                                    skip_intro_btn.set_visible(false);
                                }
                            } else {
                                skip_intro_btn.set_visible(false);
                            }
                        }

                        if config.playback.skip_credits {
                            if let Some(marker) = &credits_marker {
                                if position >= marker.start_time {
                                    skip_credits_btn.set_visible(true);
                                } else {
                                    skip_credits_btn.set_visible(false);
                                }
                            } else {
                                skip_credits_btn.set_visible(false);
                            }
                        }
                    }
                });

                glib::ControlFlow::Continue
            });
        }
    }

    fn setup_episode_features(&self, episode: Episode) {
        // Only setup handlers if we have actual markers

        // Setup skip intro button handler if intro marker exists
        if let Some(intro_marker) = episode.intro_marker.as_ref() {
            let player = self.player.clone();
            let button = self.skip_intro_button.clone();
            let intro_end = intro_marker.end_time;

            self.skip_intro_button.connect_clicked(move |_| {
                let player = player.clone();
                let button = button.clone();

                glib::spawn_future_local(async move {
                    // Skip to intro end time
                    let player = player.read().await;
                    if let Err(e) = player.seek(intro_end).await {
                        error!("Failed to skip intro: {}", e);
                    }
                    button.set_visible(false);
                });
            });
        }

        // Setup skip credits button handler
        let skip_credits = self.skip_credits_button.clone();
        let skip_credits_hide = skip_credits.clone();
        let auto_play_overlay = self.auto_play_overlay.clone();
        let pip_container = self.pip_container.clone();
        let video_container = self.video_container.clone();

        let player_page_for_skip = self.clone();
        skip_credits.connect_clicked(move |_| {
            skip_credits_hide.set_visible(false);
            info!("Skip credits clicked - triggering auto-play preview");

            // Show auto-play overlay with PiP
            // Move current video to PiP container
            if let Some(video_widget) = video_container.first_child() {
                video_container.remove(&video_widget);
                video_widget.set_size_request(320, 180);
                pip_container.append(&video_widget);
            }

            // Show the auto-play overlay
            auto_play_overlay.set_visible(true);

            // Find and display actual next episode info
            let player_page = player_page_for_skip.clone();
            let auto_play_overlay = auto_play_overlay.clone();
            glib::spawn_future_local(async move {
                if let Some(next_episode) = player_page.find_next_episode().await {
                    // Update the next episode info with actual data
                    if let Some(container) = auto_play_overlay.first_child()
                        && let Some(next_container) = container.next_sibling()
                    {
                        // Update title label with actual episode info
                        if let Some(label) = next_container
                            .first_child()
                            .and_then(|w| w.next_sibling())
                            .and_then(|w| w.downcast::<gtk4::Label>().ok())
                        {
                            let title = format!(
                                "S{:02}E{:02} - {}",
                                next_episode.season_number,
                                next_episode.episode_number,
                                next_episode.title
                            );
                            label.set_text(&title);
                        }

                        // Update countdown label (still using demo countdown for now)
                        if let Some(label) = next_container
                            .first_child()
                            .and_then(|w| w.next_sibling())
                            .and_then(|w| w.next_sibling())
                            .and_then(|w| w.downcast::<gtk4::Label>().ok())
                        {
                            label.set_text("Playing in 10 seconds");
                        }
                    }

                    // Store the next episode info for later use
                    *player_page.next_episode_info.write().await = Some(next_episode);
                } else {
                    // No next episode found - still show overlay but with different message
                    if let Some(container) = auto_play_overlay.first_child()
                        && let Some(next_container) = container.next_sibling()
                    {
                        // Update title label to indicate no next episode
                        if let Some(label) = next_container
                            .first_child()
                            .and_then(|w| w.next_sibling())
                            .and_then(|w| w.downcast::<gtk4::Label>().ok())
                        {
                            label.set_text("No next episode available");
                        }

                        // Hide countdown since there's nothing to play
                        if let Some(label) = next_container
                            .first_child()
                            .and_then(|w| w.next_sibling())
                            .and_then(|w| w.next_sibling())
                            .and_then(|w| w.downcast::<gtk4::Label>().ok())
                        {
                            label.set_text("");
                        }
                    }
                }
            });
        });

        // Monitor playback position only if we have markers
        let has_markers = episode.intro_marker.is_some() || episode.credits_marker.is_some();

        if has_markers {
            let skip_intro_btn = self.skip_intro_button.clone();
            let skip_credits_btn = self.skip_credits_button.clone();
            let player = self.player.clone();
            let config = self.config.clone();
            let view_model = self.view_model.clone();

            glib::timeout_add_local(Duration::from_millis(500), move || {
                let skip_intro_btn = skip_intro_btn.clone();
                let skip_credits_btn = skip_credits_btn.clone();
                let player = player.clone();
                let config = config.clone();
                // Markers will be read from ViewModel each tick
                let vm = view_model.clone();

                glib::spawn_future_local(async move {
                    let player = player.read().await;
                    if let Some(position) = player.get_position().await {
                        let (intro_marker, credits_marker) = vm.markers().get().await;

                        if config.playback.skip_intro {
                            if let Some(marker) = &intro_marker {
                                if position >= marker.start_time && position < marker.end_time {
                                    skip_intro_btn.set_visible(true);
                                } else {
                                    skip_intro_btn.set_visible(false);
                                }
                            } else {
                                skip_intro_btn.set_visible(false);
                            }
                        }

                        if config.playback.skip_credits {
                            if let Some(marker) = &credits_marker {
                                if position >= marker.start_time {
                                    skip_credits_btn.set_visible(true);
                                } else {
                                    skip_credits_btn.set_visible(false);
                                }
                            } else {
                                skip_credits_btn.set_visible(false);
                            }
                        }
                    }
                });

                glib::ControlFlow::Continue
            });
        }
    }

    fn monitor_playback_completion(&self, _backend_id: String, backend: Arc<dyn MediaBackend>) {
        let player = self.player.clone();
        let current_media_item = self.current_media_item.clone();
        let view_model = self.view_model.clone();

        // Start periodic position syncing (delegates to ViewModel persistence)
        self.start_position_sync(backend.clone());

        // Spawn a task to monitor player state
        glib::spawn_future_local(async move {
            // Add a small delay to let playback start
            glib::timeout_future(std::time::Duration::from_secs(2)).await;

            loop {
                // Check player state every second
                glib::timeout_future(std::time::Duration::from_secs(1)).await;

                let state = {
                    let player = player.read().await;
                    player.get_state().await
                };

                match state {
                    crate::player::PlayerState::Stopped => {
                        // Playback has ended, check if we should mark as watched
                        if let Some(media_item) = current_media_item.read().await.as_ref() {
                            // Get current position and duration
                            let player = player.read().await;
                            let position = player.get_position().await;
                            let duration = player.get_duration().await;

                            // Sync final position before marking as watched
                            if let (Some(pos), Some(dur)) = (position, duration) {
                                view_model
                                    .save_progress_throttled(media_item.id(), pos, dur)
                                    .await;
                            }

                            // If we've watched more than 90% of the content, mark as watched
                            if let (Some(pos), Some(dur)) = (position, duration) {
                                let watched_percentage = pos.as_secs_f64() / dur.as_secs_f64();
                                if watched_percentage > 0.9 {
                                    info!(
                                        "Marking {} as watched ({}% watched)",
                                        media_item.title(),
                                        (watched_percentage * 100.0) as i32
                                    );

                                    // Mark as watched on the backend
                                    if let Err(e) = backend.mark_watched(media_item.id()).await {
                                        error!("Failed to mark as watched: {}", e);
                                    }
                                }
                            }
                        }
                        break; // Exit monitoring loop
                    }
                    crate::player::PlayerState::Error(_) => {
                        // Playback error, exit monitoring
                        break;
                    }
                    _ => {
                        // Continue monitoring
                    }
                }
            }
        });
    }

    fn start_position_sync(&self, backend: Arc<dyn MediaBackend>) {
        let player = self.player.clone();
        let current_media_item = self.current_media_item.clone();
        let mut last_sync_position = std::time::Duration::ZERO;
        let view_model = self.view_model.clone();

        // Sync position every 10 seconds during playback
        glib::spawn_future_local(async move {
            let backend = backend.clone();
            loop {
                // Wait 10 seconds between syncs
                glib::timeout_future(std::time::Duration::from_secs(10)).await;

                // Get current state
                let state = {
                    let player = player.read().await;
                    player.get_state().await
                };

                // Only sync if playing or paused
                match state {
                    crate::player::PlayerState::Playing | crate::player::PlayerState::Paused => {
                        if let Some(media_item) = current_media_item.read().await.as_ref() {
                            let position = {
                                let player = player.read().await;
                                player.get_position().await
                            };
                            let duration = {
                                let player = player.read().await;
                                player.get_duration().await
                            };

                            if let (Some(pos), Some(dur)) = (position, duration) {
                                // Only sync if position has changed significantly (more than 5 seconds)
                                let pos_duration =
                                    std::time::Duration::from_secs_f64(pos.as_secs_f64());
                                if (pos_duration.as_secs() as i64
                                    - last_sync_position.as_secs() as i64)
                                    .abs()
                                    > 5
                                {
                                    debug!(
                                        "Syncing playback position: {:?} ({}s) / {:?} ({}s) for {} (id: {})",
                                        pos,
                                        pos.as_secs(),
                                        dur,
                                        dur.as_secs(),
                                        media_item.title(),
                                        media_item.id()
                                    );

                                    // Persist progress via ViewModel (debounced)
                                    view_model
                                        .save_progress_throttled(media_item.id(), pos, dur)
                                        .await;
                                    debug!("Position sync successful (VM)");

                                    // Push progress to backend (Plex/Jellyfin) as well
                                    if let Err(e) =
                                        backend.update_progress(media_item.id(), pos, dur).await
                                    {
                                        tracing::debug!(
                                            "Backend progress sync failed for {}: {}",
                                            media_item.id(),
                                            e
                                        );
                                    } else {
                                        tracing::debug!(
                                            "Position sync successful (backend) id={} pos={}s",
                                            media_item.id(),
                                            pos.as_secs()
                                        );
                                    }
                                    last_sync_position = pos_duration;
                                } else {
                                    trace!(
                                        "Skipping sync - position change too small: {}s vs {}s",
                                        pos_duration.as_secs(),
                                        last_sync_position.as_secs()
                                    );
                                }
                            } else {
                                trace!("Cannot sync - missing position or duration");
                            }
                        }
                    }
                    crate::player::PlayerState::Stopped | crate::player::PlayerState::Error(_) => {
                        // Stop syncing if playback has stopped
                        break;
                    }
                    _ => {
                        // Continue for other states
                    }
                }
            }
        });
    }

    /// Find the next episode after the current one using the backend
    async fn find_next_episode(&self) -> Option<Episode> {
        // Get the current media item
        let current_media = self.current_media_item.read().await;
        let current_media = current_media.as_ref()?;

        // Only works for episodes, not movies
        if let MediaItem::Episode(current_episode) = current_media {
            let backend_id = current_media.backend_id();
            let backend = self
                .state
                .source_coordinator
                .get_backend(backend_id)
                .await?;

            // Use the backend's find_next_episode method
            match backend.find_next_episode(current_episode).await {
                Ok(Some(next_episode)) => Some(next_episode),
                Ok(None) => None,
                Err(e) => {
                    error!("Failed to find next episode: {}", e);
                    None
                }
            }
        } else {
            None
        }
    }

    /// Load the next episode (to be called from the Play Now button)
    pub async fn load_next_episode(&self) {
        if let Some(next_episode) = self.next_episode_info.read().await.as_ref() {
            let next_media_item = MediaItem::Episode(next_episode.clone());
            match self.load_media(&next_media_item, self.state.clone()).await {
                Ok(_) => {}
                Err(e) => {
                    error!("Failed to load next episode: {}", e);
                }
            }
        }
    }
    async fn start_position_sync_timer(&self) {
        // Stop any existing timer
        self.stop_position_sync_timer().await;

        let player = self.player.clone();
        let current_media_item = self.current_media_item.clone();
        let last_synced_position = self.last_synced_position.clone();
        let timer_ref = self.position_sync_timer.clone();
        let view_model = self.view_model.clone();

        // Start a timer to sync position every 10 seconds
        let timer_id = glib::timeout_add_local(Duration::from_secs(10), move || {
            let player = player.clone();
            let current_media_item = current_media_item.clone();
            let last_synced_position = last_synced_position.clone();
            let vm = view_model.clone();

            glib::spawn_future_local(async move {
                // Get current position
                let player = player.read().await;
                if let Some(position) = player.get_position().await {
                    // Only sync if position has changed significantly (> 5 seconds)
                    let last_pos = *last_synced_position.read().await;
                    let should_sync = match last_pos {
                        None => true,
                        Some(last) => {
                            let diff = position.abs_diff(last);
                            diff > Duration::from_secs(5)
                        }
                    };

                    if should_sync {
                        // Get media item and persist progress through ViewModel
                        if let Some(media_item) = &*current_media_item.read().await {
                            let duration = media_item.duration().unwrap_or(Duration::ZERO);
                            vm.save_progress_throttled(media_item.id(), position, duration)
                                .await;
                            *last_synced_position.write().await = Some(position);
                        }
                    }
                }
            });

            glib::ControlFlow::Continue
        });

        *timer_ref.write().await = Some(timer_id);
        info!("Started playback position sync timer");
    }

    async fn stop_position_sync_timer(&self) {
        if let Some(timer_id) = self.position_sync_timer.write().await.take() {
            timer_id.remove();
            info!("Stopped playback position sync timer");
        }
    }

    async fn sync_playback_position(&self) {
        // Get current position and sync immediately
        let player = self.player.read().await;
        if let Some(position) = player.get_position().await
            && let Some(media_item) = &*self.current_media_item.read().await
        {
            let duration = media_item.duration().unwrap_or(Duration::ZERO);
            self.view_model
                .save_progress_throttled(media_item.id(), position, duration)
                .await;
        }
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
    upscaling_button: gtk4::MenuButton,
    title_label: gtk4::Label,
    time_label: gtk4::Label,
    end_time_label: gtk4::Label,
    time_display_mode: Arc<RwLock<TimeDisplayMode>>,
    player: Arc<RwLock<Player>>,
    is_seeking: Arc<RwLock<bool>>,
    inhibit_cookie: Arc<RwLock<Option<u32>>>,
    backend: Arc<RwLock<Option<Arc<dyn MediaBackend>>>>,
    current_media_item: Arc<RwLock<Option<MediaItem>>>,
    action_group: gio::SimpleActionGroup,
    track_menu_retry_count: Arc<RwLock<u8>>,
}

#[derive(Clone, Copy, Debug)]
enum TimeDisplayMode {
    TotalDuration, // Shows total duration (e.g., "1:45:00")
    TimeRemaining, // Shows time remaining (e.g., "-45:00")
    EndTime,       // Shows when it will end (e.g., "11:45 PM")
}

impl PlayerControls {
    fn new(
        player: Arc<RwLock<Player>>,
        inhibit_cookie: Arc<RwLock<Option<u32>>>,
        backend: Arc<RwLock<Option<Arc<dyn MediaBackend>>>>,
        current_media_item: Arc<RwLock<Option<MediaItem>>>,
    ) -> Self {
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
            
            /* Single progress bar with clean styling */
            .player-controls .progress-bar {
                min-height: 6px;
            }
            
            .player-controls .progress-bar trough {
                background-color: rgba(255, 255, 255, 0.15);
                border-radius: 3px;
                min-height: 6px;
            }
            
            .player-controls .progress-bar highlight {
                background-color: rgba(255, 255, 255, 0.9);
                border-radius: 3px;
                min-height: 6px;
            }
            
            .player-controls .progress-bar slider {
                min-width: 12px;
                min-height: 12px;
                background-color: white;
                border-radius: 50%;
                margin: -3px 0;
                box-shadow: 0 1px 3px rgba(0, 0, 0, 0.4);
            }
            
            .player-controls .progress-bar:hover slider {
                min-width: 14px;
                min-height: 14px;
                margin: -4px 0;
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
            }
            
            /* Fullscreen styling */
            .fullscreen {
                background-color: black;
            }
            
            .fullscreen .video-container {
                background-color: black;
            }
            
            .fullscreen .player-controls {
                margin-bottom: 40px;
                width: 80%;
                max-width: 1200px;
            }",
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

        // Simple single progress bar
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
            .width_request(150)
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
            .hexpand(true)
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
            .width_request(150)
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

        // Upscaling button
        let upscaling_button = gtk4::MenuButton::new();
        upscaling_button.set_icon_name("view-reveal-symbolic");
        upscaling_button.add_css_class("flat");
        upscaling_button.set_tooltip_text(Some("Video Upscaling"));
        right_section.append(&upscaling_button);

        // Fullscreen button
        let fullscreen_button = gtk4::Button::from_icon_name("view-fullscreen-symbolic");
        fullscreen_button.add_css_class("flat");
        right_section.append(&fullscreen_button);

        controls_row.append(&right_section);

        widget.append(&controls_row);

        // Create the action group that will be shared by all menus
        let action_group = gio::SimpleActionGroup::new();

        let controls = Self {
            widget: widget.clone(),
            play_button: play_button.clone(),
            rewind_button: rewind_button.clone(),
            forward_button: forward_button.clone(),
            progress_bar: progress_bar.clone(),
            volume_button: volume_button.clone(),
            fullscreen_button: fullscreen_button.clone(),
            audio_button: audio_button.clone(),
            subtitle_button: subtitle_button.clone(),
            quality_button: quality_button.clone(),
            upscaling_button: upscaling_button.clone(),
            title_label,
            time_label: time_label.clone(),
            end_time_label: end_time_label.clone(),
            time_display_mode: Arc::new(RwLock::new(TimeDisplayMode::TotalDuration)),
            player: player.clone(),
            is_seeking: Arc::new(RwLock::new(false)),
            inhibit_cookie,
            backend,
            current_media_item,
            action_group: action_group.clone(),
            track_menu_retry_count: Arc::new(RwLock::new(0)),
        };

        // Insert the action group into the widget hierarchy
        widget.insert_action_group("player", Some(&action_group));

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

        // Return controls; caller sets up handlers and timers
        controls
    }

    fn setup_handlers(&self, view_model: Arc<PlayerViewModel>) {
        let player = self.player.clone();
        let button = self.play_button.clone();
        let inhibit_cookie = self.inhibit_cookie.clone();
        let backend = self.backend.clone();
        let current_media_item = self.current_media_item.clone();

        // Play/pause button
        self.play_button.connect_clicked(move |btn| {
            let view_model = view_model.clone();
            let player = player.clone();
            let button = button.clone();
            let inhibit_cookie = inhibit_cookie.clone();
            let backend = backend.clone();
            let current_media_item = current_media_item.clone();
            let widget = btn.clone().upcast::<gtk4::Widget>();
            glib::spawn_future_local(async move {
                let player = player.read().await;
                // Toggle play/pause and manage inhibit
                if button.icon_name() == Some("media-playback-start-symbolic".into()) {
                    if let Err(e) = player.play().await {
                        error!("Failed to play: {}", e);
                    }
                    button.set_icon_name("media-playback-pause-symbolic");

                    // Re-inhibit suspend when resuming playback
                    Self::inhibit_suspend_static(&widget, inhibit_cookie).await;
                } else {
                    if let Err(e) = player.pause().await {
                        error!("Failed to pause: {}", e);
                    }
                    button.set_icon_name("media-playback-start-symbolic");

                    // Remove inhibit when pausing
                    Self::uninhibit_suspend_static(&widget, inhibit_cookie).await;

                    // Sync position when pausing
                    if let Some(media_item) = current_media_item.read().await.as_ref()
                        && let (Some(position), Some(duration)) =
                            (player.get_position().await, player.get_duration().await)
                    {
                        debug!(
                            "Syncing position on pause: {:?} for {}",
                            position,
                            media_item.title()
                        );
                        view_model
                            .save_progress_throttled(media_item.id(), position, duration)
                            .await;
                    }
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
        self.progress_bar
            .connect_change_value(move |scale, _, value| {
                let player = player.clone();
                let is_seeking = is_seeking.clone();
                glib::spawn_future_local(async move {
                    // Mark that we're seeking
                    *is_seeking.write().await = true;

                    let player = player.read().await;
                    if let Some(duration) = player.get_duration().await {
                        let seek_position =
                            Duration::from_secs_f64(value * duration.as_secs_f64() / 100.0);
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
            if let Some(window) = button
                .root()
                .and_then(|r| r.downcast::<gtk4::Window>().ok())
            {
                if window.is_fullscreen() {
                    window.unfullscreen();
                    button.set_icon_name("view-fullscreen-symbolic");
                    // Remove fullscreen class from parent containers
                    if let Some(parent) = button.parent() {
                        let mut widget = Some(parent);
                        while let Some(w) = widget {
                            if w.has_css_class("fullscreen") {
                                w.remove_css_class("fullscreen");
                            }
                            widget = w.parent();
                        }
                    }
                } else {
                    window.fullscreen();
                    button.set_icon_name("view-restore-symbolic");
                    // Add fullscreen class to parent containers
                    if let Some(parent) = button.parent() {
                        let mut widget = Some(parent);
                        while let Some(w) = widget {
                            if w.has_css_class("player-page") || w.is::<gtk4::Overlay>() {
                                w.add_css_class("fullscreen");
                            }
                            widget = w.parent();
                        }
                    }
                }
            }
        });

        // Setup upscaling menu
        self.setup_upscaling_menu();
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
                    (player.get_position().await, player.get_duration().await)
                {
                    // Only update progress bar if not seeking
                    if !is_seeking {
                        let progress = (position.as_secs_f64() / duration.as_secs_f64()) * 100.0;
                        progress_bar.set_value(progress);
                    }

                    // No need to show buffer info - MPV always maintains ~10 seconds
                    // which is not useful to display

                    // Update current time label (always shows current position)
                    let pos_str = format_duration(position);
                    time_label.set_text(&pos_str);

                    // Update end time label based on display mode
                    let mode = *time_display_mode.read().await;
                    let end_str = match mode {
                        TimeDisplayMode::TotalDuration => format_duration(duration),
                        TimeDisplayMode::TimeRemaining => {
                            let remaining = duration.saturating_sub(position);
                            format!("-{}", format_duration(remaining))
                        }
                        TimeDisplayMode::EndTime => {
                            // Calculate when the video will end
                            let remaining = duration.saturating_sub(position);
                            let now = chrono::Local::now();
                            let end_time =
                                now + chrono::Duration::from_std(remaining).unwrap_or_default();
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
        debug!(
            "PlayerControls::set_media_info() - Setting media title: {}",
            title
        );
        self.title_label.set_text(title);

        // Skip populating track menus for now - they require the playbin to be in Playing state
        // We'll populate them after playback starts
        debug!(
            "PlayerControls::set_media_info() - Skipping track menu population (will do after playback starts)"
        );
        // self.populate_track_menus().await;

        // Populate quality menu if stream info is available
        if let Some(info) = stream_info {
            debug!("PlayerControls::set_media_info() - Populating quality menu");
            self.populate_quality_menu(info).await;
            debug!("PlayerControls::set_media_info() - Quality menu populated");
        }

        info!("PlayerControls::set_media_info() - Media info set successfully");
    }

    pub async fn populate_track_menus(&self) {
        // Create audio tracks menu
        // Log backend + state for context
        let player_locked = self.player.read().await;
        let backend_name = match &*player_locked {
            crate::player::Player::GStreamer(_) => "gstreamer",
            crate::player::Player::Mpv(_) => "mpv",
        };
        let state = player_locked.get_state().await;
        drop(player_locked);
        debug!(
            "populate_track_menus(): backend={} state={:?}",
            backend_name, state
        );

        let audio_menu = gio::Menu::new();
        let audio_tracks = self.player.read().await.get_audio_tracks().await;
        let _current_audio = self.player.read().await.get_current_audio_track().await;

        debug!(
            "PlayerControls::populate_track_menus() - Found {} audio tracks",
            audio_tracks.len()
        );

        if audio_tracks.is_empty() {
            // Add a disabled message if no tracks found
            audio_menu.append(Some("No audio tracks available"), None);
        } else {
            for (index, name) in &audio_tracks {
                let action_name = format!("player.set-audio-track-{}", index);
                audio_menu.append(Some(name), Some(&action_name));
                debug!("  Audio track {}: {}", index, name);
            }
        }

        let audio_popover = gtk4::PopoverMenu::from_model(Some(&audio_menu));
        self.audio_button.set_popover(Some(&audio_popover));

        // Enable/disable button based on track availability
        let audio_enabled = !audio_tracks.is_empty();
        self.audio_button.set_sensitive(audio_enabled);
        debug!(
            "populate_track_menus(): audio_button sensitive={} (tracks={})",
            audio_enabled,
            audio_tracks.len()
        );

        // Create subtitle tracks menu
        let subtitle_menu = gio::Menu::new();
        let subtitle_tracks = self.player.read().await.get_subtitle_tracks().await;
        let _current_subtitle = self.player.read().await.get_current_subtitle_track().await;

        debug!(
            "PlayerControls::populate_track_menus() - Found {} subtitle tracks",
            subtitle_tracks.len()
        );

        if subtitle_tracks.is_empty() || (subtitle_tracks.len() == 1 && subtitle_tracks[0].0 == -1)
        {
            // Add a disabled message if no real subtitle tracks found (only "None" option)
            subtitle_menu.append(Some("No subtitles available"), None);
            self.subtitle_button.set_sensitive(false);
        } else {
            for (index, name) in &subtitle_tracks {
                let action_name = if *index < 0 {
                    "player.disable-subtitles".to_string()
                } else {
                    format!("player.set-subtitle-track-{}", index)
                };
                subtitle_menu.append(Some(name), Some(&action_name));
                debug!("  Subtitle track {}: {}", index, name);
            }
            self.subtitle_button.set_sensitive(true);
        }

        let subtitle_popover = gtk4::PopoverMenu::from_model(Some(&subtitle_menu));
        self.subtitle_button.set_popover(Some(&subtitle_popover));

        debug!(
            "populate_track_menus(): subtitle_button sensitive={} (tracks={})",
            self.subtitle_button.is_sensitive(),
            subtitle_tracks.len()
        );

        // Set up actions for track selection
        self.setup_track_actions().await;

        // If MPV hasn’t exposed tracks yet, retry a few times with small delays
        let needs_retry_audio = audio_tracks.is_empty();
        let needs_retry_subs = subtitle_tracks.is_empty()
            || (subtitle_tracks.len() == 1 && subtitle_tracks[0].0 == -1);

        if needs_retry_audio || needs_retry_subs {
            let retries = *self.track_menu_retry_count.read().await;
            if retries < 8 {
                let self_clone = self.clone();
                let next_retry = retries + 1;
                *self.track_menu_retry_count.write().await = next_retry;
                let delay_ms = 250u64;
                debug!(
                    "populate_track_menus(): scheduling retry {} in {}ms (audio_empty={} subs_empty_or_none={})",
                    next_retry, delay_ms, needs_retry_audio, needs_retry_subs
                );
                glib::timeout_add_local(std::time::Duration::from_millis(delay_ms), move || {
                    let again = self_clone.clone();
                    glib::spawn_future_local(async move {
                        again.populate_track_menus().await;
                    });
                    glib::ControlFlow::Break
                });
            } else {
                debug!("populate_track_menus(): retries exhausted; leaving buttons as-is");
            }
        } else {
            // Tracks found; reset retry counter
            *self.track_menu_retry_count.write().await = 0;
        }
    }

    async fn setup_track_actions(&self) {
        // Use the shared action group from the struct
        let action_group = &self.action_group;

        // Add audio track actions
        let audio_tracks = self.player.read().await.get_audio_tracks().await;
        for (index, _name) in &audio_tracks {
            let action = gio::SimpleAction::new(&format!("set-audio-track-{}", index), None);
            let player = self.player.clone();
            let track_index = *index;
            action.connect_activate(move |_, _| {
                let player = player.clone();
                glib::spawn_future_local(async move {
                    if let Err(e) = player.read().await.set_audio_track(track_index).await {
                        error!("Failed to set audio track: {}", e);
                    }
                });
            });
            action_group.add_action(&action);
        }

        // Add subtitle track actions
        let subtitle_tracks = self.player.read().await.get_subtitle_tracks().await;
        for (index, _name) in &subtitle_tracks {
            if *index < 0 {
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
                let action = gio::SimpleAction::new(&format!("set-subtitle-track-{}", index), None);
                let player = self.player.clone();
                let track_index = *index;
                action.connect_activate(move |_, _| {
                    let player = player.clone();
                    glib::spawn_future_local(async move {
                        if let Err(e) = player.read().await.set_subtitle_track(track_index).await {
                            error!("Failed to set subtitle track: {}", e);
                        }
                    });
                });
                action_group.add_action(&action);
            }
        }
    }

    async fn inhibit_suspend_static(
        widget: &gtk4::Widget,
        inhibit_cookie: Arc<RwLock<Option<u32>>>,
    ) {
        // Uninhibit any existing inhibit first
        Self::uninhibit_suspend_static(widget, inhibit_cookie.clone()).await;

        if let Some(window) = widget
            .root()
            .and_then(|r| r.downcast::<gtk4::Window>().ok())
            && let Some(app) = window
                .application()
                .and_then(|a| a.downcast::<gtk4::Application>().ok())
        {
            // Inhibit suspend and idle with reason
            let cookie = app.inhibit(
                Some(&window),
                gtk4::ApplicationInhibitFlags::SUSPEND | gtk4::ApplicationInhibitFlags::IDLE,
                Some("Playing video"),
            );

            *inhibit_cookie.write().await = Some(cookie);
            info!(
                "Inhibited system suspend/screensaver from controls (cookie: {})",
                cookie
            );
        }
    }

    async fn uninhibit_suspend_static(
        widget: &gtk4::Widget,
        inhibit_cookie: Arc<RwLock<Option<u32>>>,
    ) {
        if let Some(cookie) = inhibit_cookie.write().await.take()
            && let Some(window) = widget
                .root()
                .and_then(|r| r.downcast::<gtk4::Window>().ok())
            && let Some(app) = window
                .application()
                .and_then(|a| a.downcast::<gtk4::Application>().ok())
        {
            app.uninhibit(cookie);
            info!(
                "Removed system suspend/screensaver inhibit from controls (cookie: {})",
                cookie
            );
        }
    }

    async fn populate_quality_menu(&self, stream_info: &crate::models::StreamInfo) {
        debug!("PlayerControls::populate_quality_menu() - Starting");
        // Create quality menu
        let quality_menu = gio::Menu::new();

        // Add quality options from stream info
        debug!(
            "PlayerControls::populate_quality_menu() - Found {} quality options",
            stream_info.quality_options.len()
        );
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
        debug!("PlayerControls::populate_quality_menu() - Setting up quality actions");
        let action_group = &self.action_group;

        for (index, option) in stream_info.quality_options.iter().enumerate() {
            let action = gio::SimpleAction::new(&format!("set-quality-{}", index), None);
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
                    if let Err(e) = player.load_media(&url).await {
                        error!("Failed to switch quality: {}", e);
                        return;
                    }

                    // Seek to previous position if available
                    if let Some(pos) = position
                        && let Err(e) = player.seek(pos).await
                    {
                        error!("Failed to seek after quality switch: {}", e);
                    }

                    // Resume playback
                    if let Err(e) = player.play().await {
                        error!("Failed to resume playback: {}", e);
                    }
                });
            });
            action_group.add_action(&action);
        }
        debug!("PlayerControls::populate_quality_menu() - Actions set up successfully");

        debug!("PlayerControls::populate_quality_menu() - Complete");
    }

    fn setup_upscaling_menu(&self) {
        // Create upscaling menu
        let upscaling_menu = gio::Menu::new();

        // Add upscaling options
        upscaling_menu.append(Some("None"), Some("player.set-upscaling-none"));
        upscaling_menu.append(Some("High Quality"), Some("player.set-upscaling-hq"));
        upscaling_menu.append(Some("FSR"), Some("player.set-upscaling-fsr"));
        upscaling_menu.append(Some("Anime4K"), Some("player.set-upscaling-anime"));

        let upscaling_popover = gtk4::PopoverMenu::from_model(Some(&upscaling_menu));
        self.upscaling_button.set_popover(Some(&upscaling_popover));

        // Set up actions for upscaling selection
        let action_group = &self.action_group;

        // Add upscaling actions
        use crate::player::UpscalingMode;
        let modes = vec![
            ("set-upscaling-none", UpscalingMode::None),
            ("set-upscaling-hq", UpscalingMode::HighQuality),
            ("set-upscaling-fsr", UpscalingMode::FSR),
            ("set-upscaling-anime", UpscalingMode::Anime),
        ];

        for (action_name, mode) in modes {
            let action = gio::SimpleAction::new(action_name, None);
            let player = self.player.clone();
            let upscaling_btn = self.upscaling_button.clone();

            action.connect_activate(move |_, _| {
                let player = player.clone();
                let upscaling_btn = upscaling_btn.clone();
                let mode = mode;

                glib::spawn_future_local(async move {
                    let player = player.read().await;

                    // Check if we're using MPV player
                    if let Player::Mpv(mpv_player) = &*player {
                        match mpv_player.set_upscaling_mode(mode).await {
                            Ok(_) => {
                                let tooltip = format!("Upscaling: {}", mode.to_string());
                                upscaling_btn.set_tooltip_text(Some(&tooltip));

                                // Update icon based on mode
                                let icon = match mode {
                                    UpscalingMode::None => "view-reveal-symbolic",
                                    UpscalingMode::HighQuality => "view-continuous-symbolic",
                                    UpscalingMode::FSR => "view-fullscreen-symbolic",
                                    UpscalingMode::Anime => "view-restore-symbolic",
                                };
                                upscaling_btn.set_icon_name(icon);

                                info!("Set upscaling mode to: {}", mode.to_string());
                            }
                            Err(e) => {
                                error!("Failed to set upscaling mode: {}", e);
                            }
                        }
                    } else {
                        debug!("Upscaling only supported with MPV player");
                    }
                });
            });
            action_group.add_action(&action);
        }
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
