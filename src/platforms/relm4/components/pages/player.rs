use crate::config::Config;
use crate::models::MediaItemId;
use crate::player::{PlayerController, PlayerHandle, PlayerState};
use adw::prelude::*;
use gtk::glib::{self, SourceId};
use gtk::prelude::*;
use libadwaita as adw;
use relm4::gtk;
use relm4::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info};

fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}

pub struct PlayerPage {
    media_item_id: Option<MediaItemId>,
    player: Option<PlayerHandle>,
    player_state: PlayerState,
    position: Duration,
    duration: Duration,
    volume: f64,
    db: Arc<crate::db::connection::DatabaseConnection>,
    video_container: gtk::Box,
    // UI state
    show_controls: bool,
    is_fullscreen: bool,
    controls_timer: Option<SourceId>,
    cursor_timer: Option<SourceId>,
    // Widgets for seeking
    seek_bar: gtk::Scale,
    position_label: gtk::Label,
    duration_label: gtk::Label,
    volume_button: gtk::VolumeButton,
    // Window reference for cursor management
    window: adw::ApplicationWindow,
}

impl std::fmt::Debug for PlayerPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerPage")
            .field("media_item_id", &self.media_item_id)
            .field("player_state", &self.player_state)
            .field("position", &self.position)
            .field("duration", &self.duration)
            .field("volume", &self.volume)
            .finish()
    }
}

#[derive(Debug)]
pub enum PlayerInput {
    LoadMedia(MediaItemId),
    PlayPause,
    Stop,
    Seek(Duration),
    SetVolume(f64),
    UpdatePosition,
    ToggleFullscreen,
    ShowControls,
    HideControls,
    ResetControlsTimer,
    Previous,
    Next,
    ShowCursor,
    HideCursor,
    ResetCursorTimer,
}

#[derive(Debug, Clone)]
pub enum PlayerOutput {
    NavigateBack,
    MediaLoaded,
    Error(String),
    WindowStateChanged { width: i32, height: i32 },
}

pub enum PlayerCommandOutput {
    StateChanged(PlayerState),
    PositionUpdate {
        position: Option<Duration>,
        duration: Option<Duration>,
        state: PlayerState,
    },
}

impl std::fmt::Debug for PlayerCommandOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StateChanged(state) => write!(f, "StateChanged({:?})", state),
            Self::PositionUpdate {
                position,
                duration,
                state,
            } => {
                write!(
                    f,
                    "PositionUpdate {{ position: {:?}, duration: {:?}, state: {:?} }}",
                    position, duration, state
                )
            }
        }
    }
}

#[relm4::component(pub async)]
impl AsyncComponent for PlayerPage {
    type Init = (
        Option<MediaItemId>,
        Arc<crate::db::connection::DatabaseConnection>,
        adw::ApplicationWindow,
    );
    type Input = PlayerInput;
    type Output = PlayerOutput;
    type CommandOutput = PlayerCommandOutput;

    view! {
        gtk::Overlay {
            set_vexpand: true,
            set_hexpand: true,

            // Video container as the main child
            model.video_container.clone() {
                set_vexpand: true,
                set_hexpand: true,
                set_valign: gtk::Align::Fill,
                set_halign: gtk::Align::Fill,
                add_css_class: "video-area",
            },

            // Top left OSD controls (back button)
            add_overlay = &gtk::Box {
                set_halign: gtk::Align::Start,
                set_valign: gtk::Align::Start,
                set_margin_top: 12,
                set_margin_start: 12,
                #[watch]
                set_visible: model.show_controls,
                #[watch]
                set_opacity: if model.show_controls { 1.0 } else { 0.0 },

                gtk::Button {
                    set_icon_name: "go-previous-symbolic",
                    set_tooltip_text: Some("Back"),
                    add_css_class: "osd",
                    add_css_class: "circular",
                    connect_clicked[sender] => move |_| {
                        sender.output(PlayerOutput::NavigateBack).unwrap();
                    },
                },
            },

            // Top right OSD controls (fullscreen button)
            add_overlay = &gtk::Box {
                set_halign: gtk::Align::End,
                set_valign: gtk::Align::Start,
                set_margin_top: 12,
                set_margin_end: 12,
                #[watch]
                set_visible: model.show_controls,
                #[watch]
                set_opacity: if model.show_controls { 1.0 } else { 0.0 },

                gtk::Button {
                    #[watch]
                    set_icon_name: if model.is_fullscreen {
                        "view-restore-symbolic"
                    } else {
                        "view-fullscreen-symbolic"
                    },
                    set_tooltip_text: Some("Toggle Fullscreen"),
                    add_css_class: "osd",
                    add_css_class: "circular",
                    connect_clicked => PlayerInput::ToggleFullscreen,
                },
            },

            // Bottom controls overlay
            add_overlay = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_halign: gtk::Align::Fill,
                set_valign: gtk::Align::End,
                set_margin_all: 20,
                #[watch]
                set_visible: model.show_controls,
                #[watch]
                set_opacity: if model.show_controls { 1.0 } else { 0.0 },
                add_css_class: "osd",
                add_css_class: "player-controls",

                // Seek bar with time labels
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 12,
                    set_margin_bottom: 12,

                    model.position_label.clone() {
                        add_css_class: "numeric",
                        set_width_chars: 8,
                    },

                    model.seek_bar.clone() {
                        set_hexpand: true,
                        set_draw_value: false,
                        add_css_class: "seek-bar",
                    },

                    model.duration_label.clone() {
                        add_css_class: "numeric",
                        set_width_chars: 8,
                    },
                },

                // Playback controls
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_halign: gtk::Align::Center,
                    set_spacing: 12,

                    gtk::Button {
                        set_icon_name: "media-skip-backward-symbolic",
                        add_css_class: "circular",
                        set_size_request: (36, 36),
                        set_tooltip_text: Some("Previous"),
                        connect_clicked => PlayerInput::Previous,
                    },

                    gtk::Button {
                        #[watch]
                        set_icon_name: if matches!(model.player_state, PlayerState::Playing) {
                            "media-playback-pause-symbolic"
                        } else {
                            "media-playback-start-symbolic"
                        },
                        add_css_class: "circular",
                        add_css_class: "suggested-action",
                        set_size_request: (48, 48),
                        connect_clicked => PlayerInput::PlayPause,
                    },

                    gtk::Button {
                        set_icon_name: "media-skip-forward-symbolic",
                        add_css_class: "circular",
                        set_size_request: (36, 36),
                        set_tooltip_text: Some("Next"),
                        connect_clicked => PlayerInput::Next,
                    },

                    gtk::Separator {
                        set_orientation: gtk::Orientation::Vertical,
                        set_margin_start: 12,
                        set_margin_end: 12,
                    },

                    model.volume_button.clone() {
                        set_size_request: (36, 36),
                    },
                },
            },
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let (media_item_id, db, window) = init;

        // Create a container for the video widget that will be populated when player initializes
        let video_container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();

        // Add a placeholder initially
        let placeholder = gtk::Label::new(Some("Initializing player..."));
        placeholder.add_css_class("title-1");
        placeholder.set_valign(gtk::Align::Center);
        placeholder.set_halign(gtk::Align::Center);
        video_container.append(&placeholder);

        // Create seek bar
        let seek_bar = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
        seek_bar.set_draw_value(false);

        // Create time labels
        let position_label = gtk::Label::new(Some("0:00"));
        let duration_label = gtk::Label::new(Some("0:00"));

        // Create volume button
        let volume_button = gtk::VolumeButton::new();
        volume_button.set_value(1.0);

        let mut model = Self {
            media_item_id,
            player: None,
            player_state: PlayerState::Idle,
            position: Duration::from_secs(0),
            duration: Duration::from_secs(0),
            volume: 1.0,
            db,
            video_container: video_container.clone(),
            show_controls: true,
            is_fullscreen: false,
            controls_timer: None,
            cursor_timer: None,
            seek_bar: seek_bar.clone(),
            position_label: position_label.clone(),
            duration_label: duration_label.clone(),
            volume_button: volume_button.clone(),
            window: window.clone(),
        };

        // Initialize the player controller
        let config = Config::default();
        match PlayerController::new(&config) {
            Ok((handle, controller)) => {
                info!("Player controller initialized successfully");

                // Spawn the controller task on the main thread's executor
                // This is needed because Player contains raw pointers that are not Send
                glib::spawn_future_local(async move {
                    controller.run().await;
                });

                // Create video widget synchronously on main thread
                // Note: We need to spawn this as a local future too
                let handle_clone = handle.clone();
                let video_container_clone = model.video_container.clone();
                glib::spawn_future_local(async move {
                    if let Ok(video_widget) = handle_clone.create_video_widget().await {
                        video_container_clone.append(&video_widget);
                    }
                });

                model.player = Some(handle);
            }
            Err(e) => {
                error!("Failed to initialize player controller: {}", e);
                sender
                    .output(PlayerOutput::Error(format!(
                        "Failed to initialize player: {}",
                        e
                    )))
                    .unwrap();
            }
        }

        // Setup seek bar handler
        {
            let sender = sender.clone();
            let seek_bar = model.seek_bar.clone();
            seek_bar.connect_value_changed(move |scale| {
                if scale.has_focus() {
                    // Only send seek if user is dragging
                    let position = scale.value();
                    sender.input(PlayerInput::Seek(Duration::from_secs_f64(position)));
                }
            });
        }

        // Setup volume button handler
        {
            let sender = sender.clone();
            let volume_button = model.volume_button.clone();
            volume_button.connect_value_changed(move |button, _value| {
                sender.input(PlayerInput::SetVolume(button.value()));
            });
        }

        // Setup mouse motion handler for showing controls and cursor
        {
            let sender = sender.clone();
            let motion_controller = gtk::EventControllerMotion::new();
            motion_controller.connect_motion(move |_, _, _| {
                sender.input(PlayerInput::ShowControls);
                sender.input(PlayerInput::ShowCursor);
            });
            root.add_controller(motion_controller);
        }

        // Setup keyboard shortcuts
        {
            let sender = sender.clone();
            let sender_for_escape = sender.clone();
            let is_fullscreen = model.is_fullscreen;
            let key_controller = gtk::EventControllerKey::new();
            key_controller.connect_key_pressed(move |_, key, _, _| {
                match key {
                    gtk::gdk::Key::F11 | gtk::gdk::Key::f => {
                        sender.input(PlayerInput::ToggleFullscreen);
                        glib::Propagation::Stop
                    }
                    gtk::gdk::Key::space => {
                        sender.input(PlayerInput::PlayPause);
                        glib::Propagation::Stop
                    }
                    gtk::gdk::Key::Escape => {
                        // ESC key behavior: exit fullscreen if fullscreen, otherwise go back
                        // We need to check the actual state, not the captured value
                        // Send navigate back - MainWindow will handle chrome restoration
                        sender_for_escape
                            .output(PlayerOutput::NavigateBack)
                            .unwrap();
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                }
            });
            root.add_controller(key_controller);
        }

        // Start position update timer (1Hz)
        {
            let sender = sender.clone();
            glib::timeout_add_seconds_local(1, move || {
                sender.input(PlayerInput::UpdatePosition);
                glib::ControlFlow::Continue
            });
        }

        // Start controls and cursor timers
        sender.input(PlayerInput::ResetControlsTimer);
        sender.input(PlayerInput::ResetCursorTimer);

        // Load media if provided
        if let Some(id) = &model.media_item_id {
            sender.input(PlayerInput::LoadMedia(id.clone()));
        }

        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root,
    ) {
        match msg {
            PlayerInput::LoadMedia(id) => {
                self.media_item_id = Some(id.clone());
                self.player_state = PlayerState::Loading;

                // Get actual media URL from backend using GetStreamUrlCommand
                let db_clone = self.db.clone();
                let media_id = id.clone();
                let sender_clone = sender.clone();

                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        use crate::services::commands::Command;
                        use crate::services::commands::media_commands::GetStreamUrlCommand;

                        // Get the stream info from the backend using stateless command
                        let stream_info = match (GetStreamUrlCommand {
                            db: db_clone.as_ref().clone(),
                            media_item_id: media_id,
                        })
                        .execute()
                        .await
                        {
                            Ok(info) => info,
                            Err(e) => {
                                error!("Failed to get stream URL: {}", e);
                                return PlayerCommandOutput::StateChanged(PlayerState::Error);
                            }
                        };

                        info!("Got stream URL: {}", stream_info.url);

                        // Load the media into the player using channel-based API
                        match player_handle.load_media(&stream_info.url).await {
                            Ok(_) => {
                                info!("Media loaded successfully");

                                // Try to get video dimensions and calculate appropriate window size
                                if let Ok(Some((width, height))) =
                                    player_handle.get_video_dimensions().await
                                {
                                    if width > 0 && height > 0 {
                                        // Calculate window size based on video aspect ratio
                                        // Keep width reasonable (max 1920) and scale height accordingly
                                        let max_width = 1920.0_f32.min(width as f32);
                                        let scale = max_width / width as f32;
                                        let window_width = max_width as i32;
                                        let window_height = (height as f32 * scale) as i32;

                                        // Add some padding for controls
                                        let final_height = window_height + 100; // Extra space for controls

                                        info!(
                                            "Video dimensions: {}x{}, window size: {}x{}",
                                            width, height, window_width, final_height
                                        );

                                        // Request window resize through output
                                        sender_clone
                                            .output(PlayerOutput::WindowStateChanged {
                                                width: window_width,
                                                height: final_height,
                                            })
                                            .ok();
                                    }
                                }

                                PlayerCommandOutput::StateChanged(PlayerState::Idle)
                            }
                            Err(e) => {
                                error!("Failed to load media: {}", e);
                                PlayerCommandOutput::StateChanged(PlayerState::Error)
                            }
                        }
                    });
                }
            }
            PlayerInput::PlayPause => {
                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    let current_state = self.player_state.clone();

                    sender.oneshot_command(async move {
                        let result = match current_state {
                            PlayerState::Playing => {
                                player_handle.pause().await.ok();
                                PlayerCommandOutput::StateChanged(PlayerState::Paused)
                            }
                            _ => {
                                player_handle.play().await.ok();
                                PlayerCommandOutput::StateChanged(PlayerState::Playing)
                            }
                        };
                        result
                    });
                }
            }
            PlayerInput::Stop => {
                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        player_handle.stop().await.ok();
                        PlayerCommandOutput::StateChanged(PlayerState::Stopped)
                    });
                }
            }
            PlayerInput::Seek(position) => {
                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        player_handle.seek(position).await.ok();
                        // Return current state after seek
                        let state = player_handle
                            .get_state()
                            .await
                            .unwrap_or(PlayerState::Error);
                        PlayerCommandOutput::StateChanged(state)
                    });
                }
            }
            PlayerInput::SetVolume(volume) => {
                self.volume = volume;
                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        player_handle.set_volume(volume).await.ok();
                        // Return current state after volume change
                        let state = player_handle
                            .get_state()
                            .await
                            .unwrap_or(PlayerState::Error);
                        PlayerCommandOutput::StateChanged(state)
                    });
                }
            }
            PlayerInput::UpdatePosition => {
                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        let position = player_handle.get_position().await.unwrap_or(None);
                        let duration = player_handle.get_duration().await.unwrap_or(None);
                        let state = player_handle
                            .get_state()
                            .await
                            .unwrap_or(PlayerState::Error);
                        PlayerCommandOutput::PositionUpdate {
                            position,
                            duration,
                            state,
                        }
                    });
                }
            }
            PlayerInput::ToggleFullscreen => {
                self.is_fullscreen = !self.is_fullscreen;
                if self.is_fullscreen {
                    self.window.fullscreen();
                    // Hide cursor immediately in fullscreen
                    sender.input(PlayerInput::HideCursor);
                } else {
                    self.window.unfullscreen();
                    // Show cursor when exiting fullscreen
                    sender.input(PlayerInput::ShowCursor);
                }
            }
            PlayerInput::ShowControls => {
                self.show_controls = true;
                sender.input(PlayerInput::ResetControlsTimer);
                // Also reset cursor timer when controls are shown
                sender.input(PlayerInput::ResetCursorTimer);
            }
            PlayerInput::HideControls => {
                self.show_controls = false;
                self.controls_timer = None;
            }
            PlayerInput::ResetControlsTimer => {
                // Cancel existing timer
                if let Some(timer) = self.controls_timer.take() {
                    timer.remove();
                }

                // Start new 3-second timer
                let sender = sender.clone();
                self.controls_timer = Some(glib::timeout_add_seconds_local(3, move || {
                    sender.input(PlayerInput::HideControls);
                    glib::ControlFlow::Break
                }));
            }
            PlayerInput::Previous => {
                // TODO: Implement previous track/episode logic
                debug!("Previous track requested");
            }
            PlayerInput::Next => {
                // TODO: Implement next track/episode logic
                debug!("Next track requested");
            }
            PlayerInput::ShowCursor => {
                // Set cursor to default (visible)
                if let Some(surface) = self.window.surface() {
                    if let Some(cursor) = gtk::gdk::Cursor::from_name("default", None) {
                        surface.set_cursor(Some(&cursor));
                    }
                }
                sender.input(PlayerInput::ResetCursorTimer);
            }
            PlayerInput::HideCursor => {
                // Set cursor to blank/none (invisible)
                if let Some(surface) = self.window.surface() {
                    if let Some(cursor) = gtk::gdk::Cursor::from_name("none", None) {
                        surface.set_cursor(Some(&cursor));
                    } else {
                        // If 'none' cursor doesn't exist, try 'blank' or just hide it
                        surface.set_cursor(None);
                    }
                }
                self.cursor_timer = None;
            }
            PlayerInput::ResetCursorTimer => {
                // Cancel existing timer
                if let Some(timer) = self.cursor_timer.take() {
                    timer.remove();
                }

                // Start new 3-second timer for cursor hiding
                let sender = sender.clone();
                self.cursor_timer = Some(glib::timeout_add_seconds_local(3, move || {
                    sender.input(PlayerInput::HideCursor);
                    glib::ControlFlow::Break
                }));
            }
        }
    }

    async fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            PlayerCommandOutput::StateChanged(state) => {
                self.player_state = state;
            }
            PlayerCommandOutput::PositionUpdate {
                position,
                duration,
                state,
            } => {
                if let Some(pos) = position {
                    self.position = pos;
                    // Update position label
                    self.position_label.set_text(&format_duration(pos));

                    // Update seek bar position (only if not being dragged)
                    if !self.seek_bar.has_focus() {
                        self.seek_bar.set_value(pos.as_secs_f64());
                    }
                }
                if let Some(dur) = duration {
                    self.duration = dur;
                    // Update duration label
                    self.duration_label.set_text(&format_duration(dur));
                    // Update seek bar range
                    self.seek_bar.set_range(0.0, dur.as_secs_f64());
                }
                self.player_state = state;
            }
        }
    }
}
