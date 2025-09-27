use crate::config::Config;
use crate::models::{MediaItemId, PlaylistContext};
use crate::player::{PlayerController, PlayerHandle, PlayerState};
use adw::prelude::*;
use gtk::glib::{self, SourceId};
use libadwaita as adw;
use relm4::gtk;
use relm4::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

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

/// Control visibility state machine states
#[derive(Debug, PartialEq)]
enum ControlState {
    /// Controls and cursor are completely hidden
    Hidden,
    /// Controls and cursor are visible with inactivity timer running
    Visible { timer_id: Option<SourceId> },
    /// Controls and cursor are visible because mouse is over controls
    Hovering,
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
    video_placeholder: Option<gtk::Label>,
    // Playlist context
    playlist_context: Option<PlaylistContext>,
    // Navigation state
    can_go_previous: bool,
    can_go_next: bool,
    // UI state - Control visibility state machine
    control_state: ControlState,
    is_fullscreen: bool,
    cursor_timer: Option<SourceId>,
    // Mouse tracking for threshold detection
    last_mouse_position: Option<(f64, f64)>,
    // Debouncing for window events
    window_event_debounce: Option<SourceId>,
    // Widgets for seeking
    seek_bar: gtk::Scale,
    position_label: gtk::Label,
    duration_label: gtk::Label,
    volume_slider: gtk::Scale,
    playlist_position_label: gtk::Label,
    // Window reference for cursor management
    window: adw::ApplicationWindow,
    // Seek bar drag state
    is_seeking: bool,
    // Error handling
    error_message: Option<String>,
    retry_count: u32,
    max_retries: u32,
    retry_timer: Option<SourceId>,
    // Progress save tracking
    last_progress_save: std::time::Instant,
    // Cached config values to avoid reloading config file every second
    config_auto_resume: bool,
    config_resume_threshold_seconds: u64,
    config_progress_update_interval_seconds: u64,
    // Playback state
    playback_speed: f64,
    // Track selection menus
    audio_menu_button: gtk::MenuButton,
    subtitle_menu_button: gtk::MenuButton,
    current_audio_track: Option<i32>,
    current_subtitle_track: Option<i32>,
    // Auto-play state
    auto_play_triggered: bool,
    auto_play_timeout: Option<SourceId>,
    // Video quality (upscaling) state
    quality_menu_button: gtk::MenuButton,
    current_upscaling_mode: crate::player::UpscalingMode,
    is_mpv_backend: bool,
    // Control widgets for bounds detection
    controls_overlay: Option<gtk::Box>,
    // Timing configuration
    inactivity_timeout_secs: u64,
    mouse_move_threshold: f64,
    window_event_debounce_ms: u64,
}

impl PlayerPage {
    // Configuration constants for control visibility behavior
    const DEFAULT_INACTIVITY_TIMEOUT_SECS: u64 = 3;
    const DEFAULT_MOUSE_MOVE_THRESHOLD: f64 = 5.0; // pixels
    const DEFAULT_WINDOW_EVENT_DEBOUNCE_MS: u64 = 50; // milliseconds
    const CONTROL_FADE_ANIMATION_MS: u64 = 200; // milliseconds for fade transition

    /// Transition to the Hidden state
    fn transition_to_hidden(&mut self, _sender: AsyncComponentSender<Self>, from_timer: bool) {
        // Only try to cancel timer if not called from the timer itself
        if !from_timer {
            if let ControlState::Visible { timer_id } = &mut self.control_state {
                if let Some(timer) = timer_id.take() {
                    timer.remove();
                }
            }
        }

        self.control_state = ControlState::Hidden;

        // Hide cursor
        if let Some(surface) = self.window.surface() {
            if let Some(cursor) = gtk::gdk::Cursor::from_name("none", None) {
                surface.set_cursor(Some(&cursor));
            } else {
                surface.set_cursor(None);
            }
        }
    }

    /// Transition to the Visible state
    fn transition_to_visible(&mut self, sender: AsyncComponentSender<Self>) {
        // Cancel any existing timer first
        if let ControlState::Visible { timer_id } = &mut self.control_state {
            if let Some(timer) = timer_id.take() {
                timer.remove();
            }
        }

        // Show cursor
        if let Some(surface) = self.window.surface()
            && let Some(cursor) = gtk::gdk::Cursor::from_name("default", None)
        {
            surface.set_cursor(Some(&cursor));
        }

        // Start inactivity timer
        let timeout_secs = self.inactivity_timeout_secs;
        let sender_clone = sender.clone();
        let timer_id = glib::timeout_add_seconds_local(timeout_secs as u32, move || {
            sender_clone.input(PlayerInput::HideControls);
            glib::ControlFlow::Break
        });

        self.control_state = ControlState::Visible {
            timer_id: Some(timer_id),
        };
    }

    /// Transition to the Hovering state
    fn transition_to_hovering(&mut self, _sender: AsyncComponentSender<Self>) {
        // Cancel any existing timer
        if let ControlState::Visible { timer_id } = &mut self.control_state {
            if let Some(timer) = timer_id.take() {
                timer.remove();
            }
        }

        self.control_state = ControlState::Hovering;

        // Ensure cursor is visible
        if let Some(surface) = self.window.surface()
            && let Some(cursor) = gtk::gdk::Cursor::from_name("default", None)
        {
            surface.set_cursor(Some(&cursor));
        }
    }

    /// Check if controls should be visible
    fn controls_visible(&self) -> bool {
        !matches!(self.control_state, ControlState::Hidden)
    }

    /// Check if mouse movement exceeds threshold
    fn mouse_movement_exceeds_threshold(&self, x: f64, y: f64) -> bool {
        if let Some((last_x, last_y)) = self.last_mouse_position {
            let dx = (x - last_x).abs();
            let dy = (y - last_y).abs();
            let distance = (dx * dx + dy * dy).sqrt();
            distance >= self.mouse_move_threshold
        } else {
            true // First movement always exceeds threshold
        }
    }

    /// Check if mouse is over control widgets
    fn is_mouse_over_controls(&self, _x: f64, y: f64) -> bool {
        // For now use the heuristic approach, but this should be replaced
        // with actual widget bounds checking when controls_overlay is properly set
        if let Some(controls) = &self.controls_overlay {
            // Get the allocation of the controls overlay
            let allocation = controls.allocation();
            let controls_height = allocation.height() as f64;
            let window_height = self.window.allocated_height() as f64;

            // Check if y position is within control bounds
            // Controls are at the bottom of the window
            y >= (window_height - controls_height - 50.0) // Add some padding
        } else {
            // Fallback to heuristic: bottom 20% of window
            let window_height = self.window.allocated_height() as f64;
            y >= window_height * 0.8
        }
    }

    fn populate_audio_menu(&self, sender: AsyncComponentSender<Self>) {
        if let Some(player) = &self.player {
            let player_clone = player.clone();
            let audio_menu_button = self.audio_menu_button.clone();
            let _current_track = self.current_audio_track;
            let sender = sender.clone();

            glib::spawn_future_local(async move {
                let tracks = player_clone.get_audio_tracks().await.unwrap_or_default();

                if tracks.is_empty() {
                    // No audio tracks available, disable the button
                    audio_menu_button.set_sensitive(false);
                    audio_menu_button.set_popover(None::<&gtk::Popover>);
                } else {
                    audio_menu_button.set_sensitive(true);

                    // Create menu
                    let menu = gtk::gio::Menu::new();

                    for (track_id, track_name) in &tracks {
                        let item = gtk::gio::MenuItem::new(Some(track_name), None);
                        let action_name = format!("player.audio-track-{}", track_id);
                        item.set_action_and_target_value(Some(&action_name), None);
                        menu.append_item(&item);
                    }

                    // Create popover from menu model
                    let popover = gtk::PopoverMenu::from_model(Some(&menu));

                    // Add actions for each track
                    let action_group = gtk::gio::SimpleActionGroup::new();
                    for (track_id, _) in &tracks {
                        let action_name = format!("audio-track-{}", track_id);
                        let action = gtk::gio::SimpleAction::new(&action_name, None);
                        let sender_clone = sender.clone();
                        let track_id_copy = *track_id;
                        action.connect_activate(move |_, _| {
                            sender_clone.input(PlayerInput::SetAudioTrack(track_id_copy));
                        });
                        action_group.add_action(&action);
                    }

                    // Insert the action group
                    audio_menu_button.insert_action_group("player", Some(&action_group));
                    audio_menu_button.set_popover(Some(&popover));
                }
            });
        }
    }

    fn populate_subtitle_menu(&self, sender: AsyncComponentSender<Self>) {
        if let Some(player) = &self.player {
            let player_clone = player.clone();
            let subtitle_menu_button = self.subtitle_menu_button.clone();
            let _current_track = self.current_subtitle_track;
            let sender = sender.clone();

            glib::spawn_future_local(async move {
                let tracks = player_clone.get_subtitle_tracks().await.unwrap_or_default();

                if tracks.is_empty() || tracks.len() == 1 {
                    // No subtitle tracks available (only "None" option), disable the button
                    subtitle_menu_button.set_sensitive(false);
                    subtitle_menu_button.set_popover(None::<&gtk::Popover>);
                } else {
                    subtitle_menu_button.set_sensitive(true);

                    // Create menu
                    let menu = gtk::gio::Menu::new();

                    for (track_id, track_name) in &tracks {
                        let item = gtk::gio::MenuItem::new(Some(track_name), None);
                        let action_name = format!("player.subtitle-track-{}", track_id);
                        item.set_action_and_target_value(Some(&action_name), None);
                        menu.append_item(&item);
                    }

                    // Create popover from menu model
                    let popover = gtk::PopoverMenu::from_model(Some(&menu));

                    // Add actions for each track
                    let action_group = gtk::gio::SimpleActionGroup::new();
                    for (track_id, _) in &tracks {
                        let action_name = format!("subtitle-track-{}", track_id);
                        let action = gtk::gio::SimpleAction::new(&action_name, None);
                        let sender_clone = sender.clone();
                        let track_id_copy = *track_id;
                        action.connect_activate(move |_, _| {
                            sender_clone.input(PlayerInput::SetSubtitleTrack(track_id_copy));
                        });
                        action_group.add_action(&action);
                    }

                    // Insert the action group
                    subtitle_menu_button.insert_action_group("player", Some(&action_group));
                    subtitle_menu_button.set_popover(Some(&popover));
                }
            });
        }
    }

    fn populate_quality_menu(&self, sender: AsyncComponentSender<Self>) {
        let quality_menu_button = self.quality_menu_button.clone();
        let current_mode = self.current_upscaling_mode;
        let is_mpv = self.is_mpv_backend;

        if !is_mpv {
            // Disable button for non-MPV backends
            quality_menu_button.set_sensitive(false);
            quality_menu_button.set_tooltip_text(Some("Upscaling only available with MPV player"));
            return;
        }

        quality_menu_button.set_sensitive(true);
        quality_menu_button.set_tooltip_text(Some("Video Quality"));

        // Create menu
        let menu = gtk::gio::Menu::new();

        // Add upscaling modes
        let modes = [
            (crate::player::UpscalingMode::None, "None", "No upscaling"),
            (
                crate::player::UpscalingMode::HighQuality,
                "High Quality",
                "Enhanced quality upscaling",
            ),
            (
                crate::player::UpscalingMode::FSR,
                "FSR",
                "AMD FidelityFX Super Resolution",
            ),
            (
                crate::player::UpscalingMode::Anime,
                "Anime",
                "Optimized for anime content",
            ),
        ];

        for (mode, label, _description) in modes {
            let item = gtk::gio::MenuItem::new(Some(label), None);
            let action_name = format!("player.quality-{}", label.to_lowercase().replace(' ', "-"));
            item.set_action_and_target_value(Some(&action_name), None);

            // Add checkmark for current mode
            if mode == current_mode {
                item.set_attribute_value("icon", Some(&"object-select-symbolic".to_variant()));
            }

            menu.append_item(&item);
        }

        // Create popover from menu model
        let popover = gtk::PopoverMenu::from_model(Some(&menu));

        // Add actions for each mode
        let action_group = gtk::gio::SimpleActionGroup::new();
        for (mode, label, _) in modes {
            let action_name = format!("quality-{}", label.to_lowercase().replace(' ', "-"));
            let action = gtk::gio::SimpleAction::new(&action_name, None);
            let sender_clone = sender.clone();
            let mode_copy = mode;
            action.connect_activate(move |_, _| {
                sender_clone.input(PlayerInput::SetUpscalingMode(mode_copy));
            });
            action_group.add_action(&action);
        }

        // Insert the action group
        quality_menu_button.insert_action_group("player", Some(&action_group));
        quality_menu_button.set_popover(Some(&popover));
    }

    fn update_playlist_position_label(&self, context: &PlaylistContext) {
        match context {
            PlaylistContext::SingleItem => {
                self.playlist_position_label.set_text("");
            }
            PlaylistContext::TvShow {
                show_title,
                current_index,
                episodes,
                ..
            } => {
                if let Some(current_episode) = episodes.get(*current_index) {
                    let text = format!(
                        "{} - S{}E{} - Episode {} of {}",
                        show_title,
                        current_episode.season_number,
                        current_episode.episode_number,
                        current_index + 1,
                        episodes.len()
                    );
                    self.playlist_position_label.set_text(&text);
                }
            }
            PlaylistContext::PlayQueue {
                current_index,
                items,
                ..
            } => {
                if let Some(current_item) = items.get(*current_index) {
                    let text = format!(
                        "{} - Item {} of {}",
                        current_item.title,
                        current_index + 1,
                        items.len()
                    );
                    self.playlist_position_label.set_text(&text);
                }
            }
        }
    }
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
    LoadMediaWithContext {
        media_id: MediaItemId,
        context: PlaylistContext,
    },
    UpdateTrackMenus,
    SetAudioTrack(i32),
    SetSubtitleTrack(i32),
    PlayPause,
    Stop,
    Seek(Duration),
    SetVolume(f64),
    UpdatePosition,
    ToggleFullscreen,
    // State machine events
    MouseEnterWindow,
    MouseLeaveWindow,
    MouseMove {
        x: f64,
        y: f64,
    },
    HideControls, // Triggered by inactivity timeout
    Previous,
    Next,
    StartSeeking,
    StopSeeking,
    UpdateSeekPreview(Duration),
    Rewind,
    Forward,
    RetryLoad,
    ClearError,
    ShowError(String),
    EscapePressed,
    NavigateBack,
    // Speed controls
    SpeedUp,
    SpeedDown,
    SpeedReset,
    // Frame stepping
    FrameStepForward,
    FrameStepBackward,
    // Audio controls
    ToggleMute,
    VolumeUp,
    VolumeDown,
    // Track cycling
    CycleSubtitleTrack,
    CycleAudioTrack,
    // Control visibility (for keyboard toggle)
    ToggleControlsVisibility,
    // Relative seeking
    SeekRelative(i64), // Positive for forward, negative for backward
    // Upscaling mode
    SetUpscalingMode(crate::player::UpscalingMode),
    UpdateQualityMenu,
}

#[derive(Debug, Clone)]
pub enum PlayerOutput {
    NavigateBack,
    MediaLoaded,
    Error(String),
    ShowToast(String),
    WindowStateChanged { width: i32, height: i32 },
}

pub enum PlayerCommandOutput {
    StateChanged(PlayerState),
    PositionUpdate {
        position: Option<Duration>,
        duration: Option<Duration>,
        state: PlayerState,
    },
    LoadError(String),
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
            Self::LoadError(msg) => write!(f, "LoadError({})", msg),
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
            set_focusable: true,
            set_can_focus: true,

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
                add_css_class: "osd-overlay",
                add_css_class: "controls-visible",
                #[watch]
                set_visible: model.controls_visible(),

                gtk::Button {
                    set_icon_name: "go-previous-symbolic",
                    set_tooltip_text: Some("Back"),
                    add_css_class: "osd",
                    add_css_class: "circular",
                    connect_clicked[sender] => move |_| {
                        sender.input(PlayerInput::NavigateBack);
                    },
                },
            },

            // Top right OSD controls (fullscreen button)
            add_overlay = &gtk::Box {
                set_halign: gtk::Align::End,
                set_valign: gtk::Align::Start,
                set_margin_top: 12,
                set_margin_end: 12,
                add_css_class: "osd-overlay",
                add_css_class: "controls-visible",
                #[watch]
                set_visible: model.controls_visible(),

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

            // Error message overlay
            add_overlay = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
                set_spacing: 20,
                #[watch]
                set_visible: model.error_message.is_some(),
                add_css_class: "osd",
                add_css_class: "error-overlay",

                gtk::Image {
                    set_icon_name: Some("dialog-error-symbolic"),
                    set_pixel_size: 64,
                    add_css_class: "error-icon",
                },

                gtk::Label {
                    #[watch]
                    set_label: model.error_message.as_deref().unwrap_or("An error occurred"),
                    set_wrap: true,
                    set_max_width_chars: 50,
                    add_css_class: "title-2",
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 12,
                    set_halign: gtk::Align::Center,

                    gtk::Button {
                        set_label: "Retry",
                        add_css_class: "suggested-action",
                        add_css_class: "pill",
                        connect_clicked => PlayerInput::RetryLoad,
                    },

                    gtk::Button {
                        set_label: "Go Back",
                        add_css_class: "pill",
                        connect_clicked[sender] => move |_| {
                            sender.input(PlayerInput::NavigateBack);
                        },
                    },
                },
            },

            // Bottom controls overlay with full control layout
            add_overlay = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::End,
                set_margin_all: 20,
                set_width_request: 700,
                #[watch]
                set_visible: model.controls_visible() && model.error_message.is_none(),
                add_css_class: "osd",
                add_css_class: "player-controls",
                add_css_class: "minimal",
                #[watch]
                add_css_class: if model.controls_visible() { "fade-in" } else { "fade-out" },

                // Playlist position indicator
                model.playlist_position_label.clone() {
                    add_css_class: "dim-label",
                    set_margin_bottom: 4,
                },

                // Progress bar with time labels
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 8,
                    set_margin_bottom: 8,

                    model.position_label.clone() {
                        add_css_class: "dim-label",
                        set_width_chars: 7,
                    },

                    model.seek_bar.clone() {
                        set_hexpand: true,
                        set_draw_value: false,
                        add_css_class: "progress-bar",
                    },

                    model.duration_label.clone() {
                        add_css_class: "dim-label",
                        set_width_chars: 7,
                    },
                },

                // Main controls row with three sections
                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: 0,

                    // Left section: Volume control
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_width_request: 150,
                        set_halign: gtk::Align::Start,
                        set_spacing: 4,

                        gtk::Image {
                            set_icon_name: Some("audio-volume-high-symbolic"),
                            set_pixel_size: 16,
                            add_css_class: "dim-label",
                        },

                        model.volume_slider.clone() {
                            set_size_request: (100, -1),
                            set_draw_value: false,
                            add_css_class: "volume-slider",
                            set_value: 1.0,
                        },
                    },

                    // Center section: Playback controls
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 4,
                        set_halign: gtk::Align::Center,
                        set_hexpand: true,

                        // Previous track button
                        gtk::Button {
                            set_icon_name: "media-skip-backward-symbolic",
                            add_css_class: "flat",
                            set_tooltip_text: Some("Previous"),
                            #[watch]
                            set_sensitive: model.can_go_previous,
                            connect_clicked => PlayerInput::Previous,
                        },

                        // Rewind button (seek backward 10s)
                        gtk::Button {
                            set_icon_name: "media-seek-backward-symbolic",
                            add_css_class: "flat",
                            set_tooltip_text: Some("Rewind 10 seconds"),
                            connect_clicked => PlayerInput::Rewind,
                        },

                        // Play/pause button (center, compact)
                        gtk::Box {
                            set_size_request: (36, 36),
                            set_halign: gtk::Align::Center,
                            set_valign: gtk::Align::Center,
                            add_css_class: "play-pause-container",

                            gtk::Button {
                                #[watch]
                                set_icon_name: if matches!(model.player_state, PlayerState::Playing) {
                                    "media-playback-pause-symbolic"
                                } else {
                                    "media-playback-start-symbolic"
                                },
                                add_css_class: "circular",
                                add_css_class: "play-pause-button",
                                set_can_shrink: false,
                                connect_clicked => PlayerInput::PlayPause,
                            },
                        },

                        // Forward button (seek forward 10s)
                        gtk::Button {
                            set_icon_name: "media-seek-forward-symbolic",
                            add_css_class: "flat",
                            set_tooltip_text: Some("Forward 10 seconds"),
                            connect_clicked => PlayerInput::Forward,
                        },

                        // Next track button
                        gtk::Button {
                            set_icon_name: "media-skip-forward-symbolic",
                            add_css_class: "flat",
                            set_tooltip_text: Some("Next"),
                            #[watch]
                            set_sensitive: model.can_go_next,
                            connect_clicked => PlayerInput::Next,
                        },
                    },

                    // Right section: Track selection and fullscreen
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_width_request: 150,
                        set_halign: gtk::Align::End,
                        set_spacing: 2,

                        // Audio tracks button
                        model.audio_menu_button.clone() {
                            set_icon_name: "audio-x-generic-symbolic",
                            add_css_class: "flat",
                            set_tooltip_text: Some("Audio Track"),
                        },

                        // Subtitle tracks button
                        model.subtitle_menu_button.clone() {
                            set_icon_name: "media-view-subtitles-symbolic",
                            add_css_class: "flat",
                            set_tooltip_text: Some("Subtitles"),
                        },

                        // Quality/Resolution button
                        model.quality_menu_button.clone() {
                            set_icon_name: "preferences-system-symbolic",
                            add_css_class: "flat",
                            set_tooltip_text: Some("Video Quality"),
                        },

                        // Fullscreen button
                        gtk::Button {
                            #[watch]
                            set_icon_name: if model.is_fullscreen {
                                "view-restore-symbolic"
                            } else {
                                "view-fullscreen-symbolic"
                            },
                            add_css_class: "flat",
                            set_tooltip_text: Some("Fullscreen"),
                            connect_clicked => PlayerInput::ToggleFullscreen,
                        },
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

        // Load player CSS styles
        let css_provider = gtk::CssProvider::new();
        css_provider.load_from_string(include_str!("../../styles/player.css"));
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().expect("Could not get default display"),
            &css_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        // Create a container for the video widget that will be populated when player initializes
        let video_container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();

        // Add drag gesture to make the window draggable by clicking on the video area
        let drag_gesture = gtk::GestureDrag::new();
        let window_clone = window.clone();
        drag_gesture.connect_drag_begin(move |gesture, start_x, start_y| {
            // Only start drag if it's a primary button (left click) and not in fullscreen
            if let Some(event) = gesture.current_event() {
                if event.triggers_context_menu() {
                    return; // Don't drag on right-click
                }

                // Get the surface and start the drag
                if !window_clone.is_fullscreen()
                    && let Some(surface) = window_clone.surface()
                {
                    // Check if surface implements Toplevel interface
                    use gtk::gdk::prelude::ToplevelExt;
                    if let Some(toplevel) = surface.downcast_ref::<gtk::gdk::Toplevel>() {
                        toplevel.begin_move(
                            &event.device().unwrap(),
                            gesture.current_button() as i32,
                            start_x,
                            start_y,
                            event.time(),
                        );
                    }
                }
            }
        });
        video_container.add_controller(drag_gesture);

        // Add a placeholder initially
        let placeholder = gtk::Label::new(Some("Initializing player..."));
        placeholder.add_css_class("title-1");
        placeholder.set_valign(gtk::Align::Center);
        placeholder.set_halign(gtk::Align::Center);
        video_container.append(&placeholder);

        // Create seek bar
        let seek_bar = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
        seek_bar.set_draw_value(false);
        seek_bar.set_has_tooltip(true);

        // Add tooltip to show time at cursor position
        seek_bar.connect_query_tooltip(|scale, x, _y, _keyboard_mode, tooltip| {
            let adjustment = scale.adjustment();
            // x is already relative to the widget
            let width = scale.width() as f64;
            let ratio = x as f64 / width;
            let max = adjustment.upper();
            let value = ratio * max;
            // Clamp value to ensure it's non-negative
            let duration = Duration::from_secs_f64(value.max(0.0));
            tooltip.set_text(Some(&format_duration(duration)));
            true
        });

        // Create time labels
        let position_label = gtk::Label::new(Some("0:00"));
        let duration_label = gtk::Label::new(Some("0:00"));
        let playlist_position_label = gtk::Label::new(None);

        // Create volume slider
        let volume_slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.01);
        volume_slider.set_value(1.0);
        volume_slider.set_draw_value(false);

        // Create menu buttons for track selection
        let audio_menu_button = gtk::MenuButton::new();
        let subtitle_menu_button = gtk::MenuButton::new();
        let quality_menu_button = gtk::MenuButton::new();

        // Load config once at initialization
        let config = Config::load().unwrap_or_default();

        let mut model = Self {
            media_item_id,
            player: None,
            player_state: PlayerState::Idle,
            position: Duration::from_secs(0),
            duration: Duration::from_secs(0),
            volume: 1.0,
            db,
            video_container: video_container.clone(),
            video_placeholder: Some(placeholder.clone()),
            playlist_context: None,
            can_go_previous: false,
            can_go_next: false,
            control_state: ControlState::Visible {
                timer_id: None, // Will be set after initialization
            },
            is_fullscreen: false,
            cursor_timer: None,
            last_mouse_position: None,
            window_event_debounce: None,
            seek_bar: seek_bar.clone(),
            position_label: position_label.clone(),
            duration_label: duration_label.clone(),
            volume_slider: volume_slider.clone(),
            playlist_position_label: playlist_position_label.clone(),
            window: window.clone(),
            is_seeking: false,
            error_message: None,
            retry_count: 0,
            max_retries: 3,
            retry_timer: None,
            last_progress_save: std::time::Instant::now(),
            // Cache config values to avoid reloading every second
            config_auto_resume: config.playback.auto_resume,
            config_resume_threshold_seconds: config.playback.resume_threshold_seconds as u64,
            config_progress_update_interval_seconds: config
                .playback
                .progress_update_interval_seconds
                as u64,
            playback_speed: 1.0,
            audio_menu_button: audio_menu_button.clone(),
            subtitle_menu_button: subtitle_menu_button.clone(),
            current_audio_track: None,
            current_subtitle_track: None,
            auto_play_triggered: false,
            auto_play_timeout: None,
            quality_menu_button: quality_menu_button.clone(),
            current_upscaling_mode: match config.playback.mpv_upscaling_mode.as_str() {
                "High Quality" => crate::player::UpscalingMode::HighQuality,
                "FSR" => crate::player::UpscalingMode::FSR,
                "Anime" => crate::player::UpscalingMode::Anime,
                _ => crate::player::UpscalingMode::None,
            },
            is_mpv_backend: config.playback.player_backend == "mpv"
                || config.playback.player_backend.is_empty(),
            controls_overlay: None, // Will be set when controls are created
            inactivity_timeout_secs: Self::DEFAULT_INACTIVITY_TIMEOUT_SECS,
            mouse_move_threshold: Self::DEFAULT_MOUSE_MOVE_THRESHOLD,
            window_event_debounce_ms: Self::DEFAULT_WINDOW_EVENT_DEBOUNCE_MS,
        };

        // Initialize the player controller
        match PlayerController::new(&config) {
            Ok((handle, controller)) => {
                info!("Player controller initialized successfully");

                // Set up error monitoring
                if let Some(mut error_receiver) = handle.take_error_receiver() {
                    let sender_clone = sender.clone();
                    glib::spawn_future_local(async move {
                        while let Some(error_msg) = error_receiver.recv().await {
                            error!("Player error received: {}", error_msg);
                            // Send toast notification
                            sender_clone
                                .output(PlayerOutput::ShowToast(error_msg.clone()))
                                .unwrap();
                            // Show error in player UI
                            sender_clone.input(PlayerInput::ShowError(error_msg));
                        }
                    });
                }

                // Spawn the controller task on the main thread's executor
                // This is needed because Player contains raw pointers that are not Send
                glib::spawn_future_local(async move {
                    controller.run().await;
                });

                // Create video widget synchronously on main thread
                // Note: We need to spawn this as a local future too
                let handle_clone = handle.clone();
                let video_container_clone = model.video_container.clone();
                let placeholder_clone = model.video_placeholder.take();
                glib::spawn_future_local(async move {
                    if let Ok(video_widget) = handle_clone.create_video_widget().await {
                        // Remove placeholder if it exists
                        if let Some(placeholder) = placeholder_clone {
                            video_container_clone.remove(&placeholder);
                        }

                        // Set proper expansion for the video widget
                        video_widget.set_vexpand(true);
                        video_widget.set_hexpand(true);
                        video_widget.set_valign(gtk::Align::Fill);
                        video_widget.set_halign(gtk::Align::Fill);

                        // Add the video widget
                        video_container_clone.append(&video_widget);

                        info!("Video widget successfully attached to container");
                    }
                });

                model.player = Some(handle.clone());

                // Apply saved upscaling mode if MPV backend
                if model.is_mpv_backend {
                    let saved_mode = model.current_upscaling_mode;
                    let handle_for_upscaling = handle;
                    glib::spawn_future_local(async move {
                        let _ = handle_for_upscaling.set_upscaling_mode(saved_mode).await;
                    });
                }
            }
            Err(e) => {
                error!("Failed to initialize player controller: {}", e);
                model.error_message = Some(format!("Failed to initialize player: {}", e));
                model.player_state = PlayerState::Error;
            }
        }

        // Setup seek bar handlers
        {
            // Track when user starts dragging
            let sender_press = sender.clone();
            let button_controller = gtk::GestureClick::new();
            button_controller.set_button(gtk::gdk::BUTTON_PRIMARY);
            button_controller.connect_pressed(move |_, _, _, _| {
                sender_press.input(PlayerInput::StartSeeking);
            });
            model.seek_bar.add_controller(button_controller);

            // Track when user releases drag and perform seek
            let sender_release = sender.clone();
            let seek_bar_clone = model.seek_bar.clone();
            let button_release_controller = gtk::GestureClick::new();
            button_release_controller.set_button(gtk::gdk::BUTTON_PRIMARY);
            button_release_controller.connect_released(move |_, _, _, _| {
                let position = seek_bar_clone.value();
                sender_release.input(PlayerInput::StopSeeking);
                // Ensure position is non-negative before creating Duration
                sender_release.input(PlayerInput::Seek(Duration::from_secs_f64(
                    position.max(0.0),
                )));
            });
            model.seek_bar.add_controller(button_release_controller);

            // Also handle value changes during drag for smooth preview
            let sender_changed = sender.clone();
            let seek_bar = model.seek_bar.clone();
            seek_bar.connect_value_changed(move |scale| {
                // Update time labels during drag for preview
                // Clamp scale value to prevent negative Duration
                let position = Duration::from_secs_f64(scale.value().max(0.0));
                sender_changed.input(PlayerInput::UpdateSeekPreview(position));
            });
        }

        // Setup volume slider handler
        {
            let sender = sender.clone();
            let volume_slider = model.volume_slider.clone();
            volume_slider.connect_value_changed(move |scale| {
                sender.input(PlayerInput::SetVolume(scale.value()));
            });
        }

        // Setup motion detection with state machine
        {
            let sender_motion = sender.clone();
            let sender_enter = sender.clone();
            let sender_leave = sender.clone();
            let motion_controller = gtk::EventControllerMotion::new();

            // Track mouse enter event for the window
            motion_controller.connect_enter(move |_, x, y| {
                debug!("Mouse entered window at ({}, {})", x, y);
                // Transition based on current state
                sender_enter.input(PlayerInput::MouseEnterWindow);
            });

            // Track mouse leave event for the window
            motion_controller.connect_leave(move |_| {
                debug!("Mouse left window");
                // Transition to hidden immediately
                sender_leave.input(PlayerInput::MouseLeaveWindow);
            });

            // Track mouse motion within the window
            motion_controller.connect_motion(move |_, x, y| {
                // Send position for state machine to handle
                sender_motion.input(PlayerInput::MouseMove { x, y });
            });

            root.add_controller(motion_controller);
        }

        // Setup keyboard shortcuts
        {
            let sender = sender.clone();
            let _sender_for_escape = sender.clone();
            let sender_for_fullscreen_check = sender.clone();
            let key_controller = gtk::EventControllerKey::new();
            key_controller.connect_key_pressed(move |_controller, key, _keycode, modifiers| {
                use gtk::gdk::ModifierType;

                // Show controls on any keyboard input
                sender.input(PlayerInput::MouseMove { x: 0.0, y: 0.0 });

                // Check modifier keys
                let shift_pressed = modifiers.contains(ModifierType::SHIFT_MASK);
                let ctrl_pressed = modifiers.contains(ModifierType::CONTROL_MASK);

                match key {
                    // Fullscreen controls
                    gtk::gdk::Key::F11 | gtk::gdk::Key::f => {
                        sender.input(PlayerInput::ToggleFullscreen);
                        glib::Propagation::Stop
                    }
                    // Playback controls
                    gtk::gdk::Key::space => {
                        sender.input(PlayerInput::PlayPause);
                        glib::Propagation::Stop
                    }
                    // Quit/Stop
                    gtk::gdk::Key::Escape | gtk::gdk::Key::q => {
                        sender_for_fullscreen_check.input(PlayerInput::EscapePressed);
                        glib::Propagation::Stop
                    }
                    // Seeking controls
                    gtk::gdk::Key::Left => {
                        if ctrl_pressed {
                            // Ctrl+Left: seek back 10s
                            sender.input(PlayerInput::SeekRelative(-10));
                        } else if shift_pressed {
                            // Shift+Left: seek back 1s
                            sender.input(PlayerInput::SeekRelative(-1));
                        } else {
                            // Left: seek back 5s (default)
                            sender.input(PlayerInput::SeekRelative(-5));
                        }
                        glib::Propagation::Stop
                    }
                    gtk::gdk::Key::Right => {
                        if ctrl_pressed {
                            // Ctrl+Right: seek forward 10s
                            sender.input(PlayerInput::SeekRelative(10));
                        } else if shift_pressed {
                            // Shift+Right: seek forward 1s
                            sender.input(PlayerInput::SeekRelative(1));
                        } else {
                            // Right: seek forward 5s (default)
                            sender.input(PlayerInput::SeekRelative(5));
                        }
                        glib::Propagation::Stop
                    }
                    // Speed controls
                    gtk::gdk::Key::bracketleft => {
                        // [ key: speed down
                        sender.input(PlayerInput::SpeedDown);
                        glib::Propagation::Stop
                    }
                    gtk::gdk::Key::bracketright => {
                        // ] key: speed up
                        sender.input(PlayerInput::SpeedUp);
                        glib::Propagation::Stop
                    }
                    gtk::gdk::Key::BackSpace => {
                        // Backspace: reset speed
                        sender.input(PlayerInput::SpeedReset);
                        glib::Propagation::Stop
                    }
                    // Volume controls
                    gtk::gdk::Key::_9 => {
                        // 9: volume down by 10%
                        sender.input(PlayerInput::VolumeDown);
                        glib::Propagation::Stop
                    }
                    gtk::gdk::Key::_0 => {
                        // 0: volume up by 10%
                        sender.input(PlayerInput::VolumeUp);
                        glib::Propagation::Stop
                    }
                    gtk::gdk::Key::m => {
                        // m: toggle mute
                        sender.input(PlayerInput::ToggleMute);
                        glib::Propagation::Stop
                    }
                    // Frame stepping
                    gtk::gdk::Key::period => {
                        // . key: frame step forward
                        sender.input(PlayerInput::FrameStepForward);
                        glib::Propagation::Stop
                    }
                    gtk::gdk::Key::comma => {
                        // , key: frame step backward
                        sender.input(PlayerInput::FrameStepBackward);
                        glib::Propagation::Stop
                    }
                    // Subtitle controls
                    gtk::gdk::Key::v => {
                        // v: cycle subtitles
                        sender.input(PlayerInput::CycleSubtitleTrack);
                        glib::Propagation::Stop
                    }
                    gtk::gdk::Key::j => {
                        if shift_pressed {
                            // Shift+J: cycle subtitle track backward (same as v for simplicity)
                            sender.input(PlayerInput::CycleSubtitleTrack);
                        } else {
                            // j: cycle subtitle track forward
                            sender.input(PlayerInput::CycleSubtitleTrack);
                        }
                        glib::Propagation::Stop
                    }
                    // Audio track cycling
                    gtk::gdk::Key::numbersign => {
                        // # key: cycle audio track
                        sender.input(PlayerInput::CycleAudioTrack);
                        glib::Propagation::Stop
                    }
                    gtk::gdk::Key::_3 if shift_pressed => {
                        // Shift+3 (also #): cycle audio track
                        sender.input(PlayerInput::CycleAudioTrack);
                        glib::Propagation::Stop
                    }
                    // Controls visibility
                    gtk::gdk::Key::Tab => {
                        // Tab: toggle controls visibility
                        sender.input(PlayerInput::ToggleControlsVisibility);
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                }
            });
            root.add_controller(key_controller);
        }

        // Populate quality menu
        model.populate_quality_menu(sender.clone());

        // Start position update timer (1Hz)
        {
            let sender = sender.clone();
            glib::timeout_add_seconds_local(1, move || {
                sender.input(PlayerInput::UpdatePosition);
                glib::ControlFlow::Continue
            });
        }

        // Start with controls visible with timer
        model.transition_to_visible(sender.clone());

        // Load media if provided
        if let Some(id) = &model.media_item_id {
            sender.input(PlayerInput::LoadMedia(id.clone()));
        }

        let widgets = view_output!();

        // Grab focus to ensure keyboard shortcuts work
        root.grab_focus();

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
                // Clear context when loading without context
                self.playlist_context = None;
                // No navigation available for single items
                self.can_go_previous = false;
                self.can_go_next = false;
                // Clear playlist position label
                self.playlist_position_label.set_text("");
                // Clear any existing error message
                self.error_message = None;
                // Reset auto-play state
                self.auto_play_triggered = false;
                if let Some(timeout) = self.auto_play_timeout.take() {
                    timeout.remove();
                }

                // Get actual media URL from backend using GetStreamUrlCommand
                let db_clone = self.db.clone();
                let media_id = id.clone();
                let media_id_for_resume = media_id.clone();
                let sender_clone = sender.clone();
                // Capture cached config values to avoid reloading config in async closure
                let auto_resume = self.config_auto_resume;
                let resume_threshold_seconds = self.config_resume_threshold_seconds;

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
                                return PlayerCommandOutput::LoadError(format!(
                                    "Failed to load media: {}",
                                    e
                                ));
                            }
                        };

                        info!("Got stream URL: {}", stream_info.url);

                        // Load the media into the player using channel-based API
                        match player_handle.load_media(&stream_info.url).await {
                            Ok(_) => {
                                info!("Media loaded successfully");

                                // Populate track menus after media loads
                                sender_clone.input(PlayerInput::UpdateTrackMenus);

                                // Check for saved playback progress and resume if configured
                                use crate::services::commands::GetPlaybackProgressCommand;

                                // Use cached config values
                                if auto_resume {
                                    // TODO: Restore PlayQueue state when loading without context
                                    // This requires restructuring to avoid Send/Sync issues with backend.as_any()

                                    // Get saved progress
                                    if let Ok(Some((position_ms, _duration_ms))) =
                                        (GetPlaybackProgressCommand {
                                            db: db_clone.as_ref().clone(),
                                            media_id: media_id_for_resume.clone(),
                                            user_id: "default".to_string(), // TODO: Get actual user ID
                                        })
                                        .execute()
                                        .await
                                    {
                                        // Only resume if we've watched more than the threshold
                                        let threshold_ms = (resume_threshold_seconds as i64) * 1000;
                                        if position_ms > threshold_ms {
                                            let resume_position = std::time::Duration::from_millis(
                                                position_ms as u64,
                                            );
                                            info!("Resuming playback from {:?}", resume_position);

                                            // Seek to saved position
                                            if let Err(e) =
                                                player_handle.seek(resume_position).await
                                            {
                                                warn!("Failed to seek to saved position: {}", e);
                                            }
                                        }
                                    }
                                }

                                // Try to get video dimensions and calculate appropriate window size
                                if let Ok(Some((width, height))) =
                                    player_handle.get_video_dimensions().await
                                    && width > 0
                                    && height > 0
                                {
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

                                // Get the actual state from the player after loading
                                let actual_state =
                                    player_handle.get_state().await.unwrap_or(PlayerState::Idle);
                                PlayerCommandOutput::StateChanged(actual_state)
                            }
                            Err(e) => {
                                error!("Failed to load media: {}", e);
                                PlayerCommandOutput::LoadError(format!("Playback error: {}", e))
                            }
                        }
                    });
                }
            }
            PlayerInput::LoadMediaWithContext { media_id, context } => {
                self.media_item_id = Some(media_id.clone());
                self.player_state = PlayerState::Loading;

                // Update navigation state based on context
                self.can_go_previous = context.has_previous();
                self.can_go_next = context.has_next();

                // Update playlist position label
                self.update_playlist_position_label(&context);

                self.playlist_context = Some(context);
                // Clear any existing error message
                self.error_message = None;
                // Reset auto-play state
                self.auto_play_triggered = false;
                if let Some(timeout) = self.auto_play_timeout.take() {
                    timeout.remove();
                }

                // Get actual media URL from backend using GetStreamUrlCommand
                let db_clone = self.db.clone();
                let media_id_clone = media_id.clone();
                let media_id_for_resume = media_id_clone.clone();
                let sender_clone = sender.clone();
                // Capture cached config values to avoid reloading config in async closure
                let auto_resume = self.config_auto_resume;
                let resume_threshold_seconds = self.config_resume_threshold_seconds;

                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        use crate::services::commands::Command;
                        use crate::services::commands::media_commands::GetStreamUrlCommand;

                        // Get the stream info from the backend using stateless command
                        let stream_info = match (GetStreamUrlCommand {
                            db: db_clone.as_ref().clone(),
                            media_item_id: media_id_clone,
                        })
                        .execute()
                        .await
                        {
                            Ok(info) => info,
                            Err(e) => {
                                error!("Failed to get stream URL: {}", e);
                                return PlayerCommandOutput::LoadError(format!(
                                    "Failed to load media: {}",
                                    e
                                ));
                            }
                        };

                        info!("Got stream URL: {}", stream_info.url);

                        // Load the media into the player using channel-based API
                        match player_handle.load_media(&stream_info.url).await {
                            Ok(_) => {
                                info!("Media loaded successfully with playlist context");

                                // Populate track menus after media loads
                                sender_clone.input(PlayerInput::UpdateTrackMenus);

                                // Check for saved playback progress and resume if configured
                                use crate::services::commands::GetPlaybackProgressCommand;

                                // Use cached config values
                                if auto_resume {
                                    // TODO: Restore PlayQueue state when loading without context
                                    // This requires restructuring to avoid Send/Sync issues with backend.as_any()

                                    // Get saved progress
                                    if let Ok(Some((position_ms, _duration_ms))) =
                                        (GetPlaybackProgressCommand {
                                            db: db_clone.as_ref().clone(),
                                            media_id: media_id_for_resume.clone(),
                                            user_id: "default".to_string(), // TODO: Get actual user ID
                                        })
                                        .execute()
                                        .await
                                    {
                                        // Only resume if we've watched more than the threshold
                                        let threshold_ms = (resume_threshold_seconds as i64) * 1000;
                                        if position_ms > threshold_ms {
                                            let resume_position = std::time::Duration::from_millis(
                                                position_ms as u64,
                                            );
                                            info!("Resuming playback from {:?}", resume_position);

                                            // Seek to saved position
                                            if let Err(e) =
                                                player_handle.seek(resume_position).await
                                            {
                                                warn!("Failed to seek to saved position: {}", e);
                                            }
                                        }
                                    }
                                }

                                // Try to get video dimensions and calculate appropriate window size
                                if let Ok(Some((width, height))) =
                                    player_handle.get_video_dimensions().await
                                    && width > 0
                                    && height > 0
                                {
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

                                // Get the actual state from the player after loading
                                let actual_state =
                                    player_handle.get_state().await.unwrap_or(PlayerState::Idle);
                                PlayerCommandOutput::StateChanged(actual_state)
                            }
                            Err(e) => {
                                error!("Failed to load media: {}", e);
                                PlayerCommandOutput::LoadError(format!("Playback error: {}", e))
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
                        // Execute the play/pause command based on current state
                        match current_state {
                            PlayerState::Playing => {
                                player_handle.pause().await.ok();
                            }
                            _ => {
                                player_handle.play().await.ok();
                            }
                        };

                        // Get the actual state from the player after the command
                        let actual_state = player_handle
                            .get_state()
                            .await
                            .unwrap_or(PlayerState::Error);

                        PlayerCommandOutput::StateChanged(actual_state)
                    });
                }
            }
            PlayerInput::Stop => {
                // Save current progress before stopping
                if let Some(media_id) = &self.media_item_id {
                    let db = (*self.db).clone();
                    let media_id = media_id.clone();
                    let position_ms = self.position.as_millis() as i64;
                    let duration_ms = self.duration.as_millis() as i64;
                    let watched = position_ms as f64 / duration_ms as f64 > 0.9;

                    relm4::spawn(async move {
                        use crate::services::commands::{Command, UpdatePlaybackProgressCommand};

                        let command = UpdatePlaybackProgressCommand {
                            db,
                            media_id,
                            position_ms,
                            duration_ms,
                            watched,
                        };

                        if let Err(e) = command.execute().await {
                            debug!("Failed to save final playback progress: {}", e);
                        }
                    });
                }

                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        player_handle.stop().await.ok();
                        // Get the actual state from the player after stopping
                        let actual_state = player_handle
                            .get_state()
                            .await
                            .unwrap_or(PlayerState::Stopped);
                        PlayerCommandOutput::StateChanged(actual_state)
                    });
                }
            }
            PlayerInput::Seek(position) => {
                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        player_handle.seek(position).await.ok();
                        // Get the actual state from the player after seeking
                        let actual_state = player_handle
                            .get_state()
                            .await
                            .unwrap_or(PlayerState::Error);
                        PlayerCommandOutput::StateChanged(actual_state)
                    });
                }
            }
            PlayerInput::SetVolume(volume) => {
                self.volume = volume;
                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        player_handle.set_volume(volume).await.ok();
                        // Get the actual state from the player after volume change
                        let actual_state = player_handle
                            .get_state()
                            .await
                            .unwrap_or(PlayerState::Error);
                        PlayerCommandOutput::StateChanged(actual_state)
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
                debug!("Toggling fullscreen to: {}", self.is_fullscreen);
                if self.is_fullscreen {
                    self.window.fullscreen();
                    // In fullscreen mode: transition to visible state
                    self.transition_to_visible(sender.clone());
                } else {
                    self.window.unfullscreen();
                    // When exiting fullscreen: show controls and cursor
                    self.transition_to_visible(sender.clone());
                }
                // Ensure focus after fullscreen toggle
                root.grab_focus();
            }
            PlayerInput::MouseEnterWindow => {
                // Handle window enter with debouncing
                if let Some(timer) = self.window_event_debounce.take() {
                    timer.remove();
                }

                // Process immediately for now (can add debouncing later if needed)
                match self.control_state {
                    ControlState::Hidden => {
                        self.transition_to_visible(sender.clone());
                    }
                    _ => {} // Already visible or hovering
                }
            }
            PlayerInput::MouseLeaveWindow => {
                // Handle window leave with debouncing
                if let Some(timer) = self.window_event_debounce.take() {
                    timer.remove();
                }

                // Process immediately - always hide when leaving window
                self.transition_to_hidden(sender.clone(), false);
            }
            PlayerInput::MouseMove { x, y } => {
                // Check if movement exceeds threshold
                if self.mouse_movement_exceeds_threshold(x, y) {
                    // Update last position
                    self.last_mouse_position = Some((x, y));

                    // Check if mouse is over controls
                    let over_controls = self.is_mouse_over_controls(x, y);

                    // Handle state transitions based on current state and mouse position
                    match &self.control_state {
                        ControlState::Hidden => {
                            // Any significant movement shows controls
                            self.transition_to_visible(sender.clone());
                        }
                        ControlState::Visible { .. } => {
                            if over_controls {
                                // Mouse entered controls area
                                self.transition_to_hovering(sender.clone());
                            } else {
                                // Reset timer on movement outside controls
                                self.transition_to_visible(sender.clone());
                            }
                        }
                        ControlState::Hovering => {
                            if !over_controls {
                                // Mouse left controls area
                                self.transition_to_visible(sender.clone());
                            }
                            // If still hovering, no action needed
                        }
                    }
                }
            }
            PlayerInput::HideControls => {
                // Called by inactivity timer
                if matches!(self.control_state, ControlState::Visible { .. }) {
                    // Only hide if we're in Visible state (not Hovering)
                    self.transition_to_hidden(sender.clone(), true);
                }
            }
            PlayerInput::Previous => {
                debug!("Previous track requested");

                if let Some(ref context) = self.playlist_context {
                    if let Some(prev_id) = context.get_previous_item() {
                        // Keep the context and just load the previous media
                        let mut new_context = context.clone();
                        new_context.update_current_index(&prev_id);

                        sender.input(PlayerInput::LoadMediaWithContext {
                            media_id: prev_id,
                            context: new_context,
                        });
                    } else {
                        debug!("No previous episode available");
                    }
                } else {
                    debug!("No playlist context available for previous navigation");
                }
            }
            PlayerInput::Next => {
                debug!("Next track requested");

                if let Some(ref context) = self.playlist_context {
                    if let Some(next_id) = context.get_next_item() {
                        // Keep the context and just load the next media
                        let mut new_context = context.clone();
                        new_context.update_current_index(&next_id);

                        sender.input(PlayerInput::LoadMediaWithContext {
                            media_id: next_id,
                            context: new_context,
                        });
                    } else {
                        debug!("No next episode available");
                    }
                } else {
                    debug!("No playlist context available for next navigation");
                }
            }
            PlayerInput::StartSeeking => {
                self.is_seeking = true;
            }
            PlayerInput::StopSeeking => {
                self.is_seeking = false;
            }
            PlayerInput::UpdateSeekPreview(position) => {
                // Update position label to show preview time during seek
                self.position_label.set_text(&format_duration(position));
            }
            PlayerInput::Rewind => {
                // Seek backward 10 seconds
                let new_position = self.position.saturating_sub(Duration::from_secs(10));
                sender.input(PlayerInput::Seek(new_position));
            }
            PlayerInput::Forward => {
                // Seek forward 10 seconds
                let new_position = self.position + Duration::from_secs(10);
                // Clamp to duration if we have it
                let final_position = if new_position > self.duration {
                    self.duration
                } else {
                    new_position
                };
                sender.input(PlayerInput::Seek(final_position));
            }
            PlayerInput::SeekRelative(seconds) => {
                // Relative seeking: positive for forward, negative for backward
                let duration = if seconds >= 0 {
                    Duration::from_secs(seconds as u64)
                } else {
                    Duration::from_secs((-seconds) as u64)
                };

                let new_position = if seconds >= 0 {
                    // Forward seek
                    let pos = self.position + duration;
                    // Clamp to duration
                    if pos > self.duration {
                        self.duration
                    } else {
                        pos
                    }
                } else {
                    // Backward seek
                    self.position.saturating_sub(duration)
                };

                sender.input(PlayerInput::Seek(new_position));
            }
            PlayerInput::SpeedUp => {
                // Increase playback speed by 10%
                self.playback_speed = (self.playback_speed * 1.1).min(4.0); // Cap at 4x speed
                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    let speed = self.playback_speed;
                    sender.oneshot_command(async move {
                        player_handle.set_playback_speed(speed).await.ok();
                        PlayerCommandOutput::StateChanged(PlayerState::Playing)
                    });
                }
            }
            PlayerInput::SpeedDown => {
                // Decrease playback speed by 10%
                self.playback_speed = (self.playback_speed * 0.9).max(0.25); // Min 0.25x speed
                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    let speed = self.playback_speed;
                    sender.oneshot_command(async move {
                        player_handle.set_playback_speed(speed).await.ok();
                        PlayerCommandOutput::StateChanged(PlayerState::Playing)
                    });
                }
            }
            PlayerInput::SpeedReset => {
                // Reset playback speed to normal
                self.playback_speed = 1.0;
                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        player_handle.set_playback_speed(1.0).await.ok();
                        PlayerCommandOutput::StateChanged(PlayerState::Playing)
                    });
                }
            }
            PlayerInput::FrameStepForward => {
                // Step one frame forward (while paused)
                if self.player_state == PlayerState::Paused
                    && let Some(player) = &self.player
                {
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        player_handle.frame_step_forward().await.ok();
                        PlayerCommandOutput::StateChanged(PlayerState::Paused)
                    });
                }
            }
            PlayerInput::FrameStepBackward => {
                // Step one frame backward (while paused)
                if self.player_state == PlayerState::Paused
                    && let Some(player) = &self.player
                {
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        player_handle.frame_step_backward().await.ok();
                        PlayerCommandOutput::StateChanged(PlayerState::Paused)
                    });
                }
            }
            PlayerInput::ToggleMute => {
                // Toggle mute state
                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        player_handle.toggle_mute().await.ok();
                        PlayerCommandOutput::StateChanged(PlayerState::Playing)
                    });
                }
            }
            PlayerInput::VolumeUp => {
                // Increase volume by 10%
                let new_volume = (self.volume + 0.1).min(1.0);
                self.volume = new_volume;
                self.volume_slider.set_value(new_volume);
                sender.input(PlayerInput::SetVolume(new_volume));
            }
            PlayerInput::VolumeDown => {
                // Decrease volume by 10%
                let new_volume = (self.volume - 0.1).max(0.0);
                self.volume = new_volume;
                self.volume_slider.set_value(new_volume);
                sender.input(PlayerInput::SetVolume(new_volume));
            }
            PlayerInput::CycleSubtitleTrack => {
                // Cycle through available subtitle tracks
                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        player_handle.cycle_subtitle_track().await.ok();
                        PlayerCommandOutput::StateChanged(PlayerState::Playing)
                    });
                }
            }
            PlayerInput::CycleAudioTrack => {
                // Cycle through available audio tracks
                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        player_handle.cycle_audio_track().await.ok();
                        PlayerCommandOutput::StateChanged(PlayerState::Playing)
                    });
                }
            }
            PlayerInput::UpdateTrackMenus => {
                // Populate the track selection menus
                self.populate_audio_menu(sender.clone());
                self.populate_subtitle_menu(sender.clone());
                self.populate_quality_menu(sender.clone());

                // Also get current track selections
                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    let _sender_clone = sender.clone();
                    glib::spawn_future_local(async move {
                        let _audio_track =
                            player_handle.get_current_audio_track().await.unwrap_or(-1);
                        let _subtitle_track = player_handle
                            .get_current_subtitle_track()
                            .await
                            .unwrap_or(-1);
                        // Store the current tracks (we'll update the model in a moment)
                    });
                }
            }
            PlayerInput::SetAudioTrack(track_id) => {
                if let Some(player) = &self.player {
                    self.current_audio_track = Some(track_id);
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        let _ = player_handle.set_audio_track(track_id).await;
                        PlayerCommandOutput::StateChanged(PlayerState::Playing)
                    });
                }
            }
            PlayerInput::SetSubtitleTrack(track_id) => {
                if let Some(player) = &self.player {
                    self.current_subtitle_track = Some(track_id);
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        let _ = player_handle.set_subtitle_track(track_id).await;
                        PlayerCommandOutput::StateChanged(PlayerState::Playing)
                    });
                }
            }
            PlayerInput::ToggleControlsVisibility => {
                // Toggle between Hidden and Visible states
                match self.control_state {
                    ControlState::Hidden => {
                        self.transition_to_visible(sender.clone());
                    }
                    _ => {
                        self.transition_to_hidden(sender.clone(), false);
                    }
                }
            }
            PlayerInput::RetryLoad => {
                // Clear the error and retry loading the media
                self.error_message = None;

                // Check if we've exceeded max retries
                if self.retry_count >= self.max_retries {
                    self.error_message = Some(
                        "Failed to load media after multiple attempts. Please check your connection and try again.".to_string()
                    );
                    self.retry_count = 0;
                    return;
                }

                // Increment retry count
                self.retry_count += 1;

                // Calculate exponential backoff delay (1s, 2s, 4s)
                let delay = Duration::from_secs(2_u64.pow(self.retry_count - 1));

                // Schedule retry after delay
                if let Some(timer) = self.retry_timer.take() {
                    timer.remove();
                }

                let sender_clone = sender.clone();
                let media_id = self.media_item_id.clone();
                let context = self.playlist_context.clone();

                info!("Scheduling retry #{} after {:?}", self.retry_count, delay);

                self.retry_timer = Some(glib::timeout_add_local(delay, move || {
                    if let Some(id) = &media_id {
                        if let Some(ctx) = &context {
                            sender_clone.input(PlayerInput::LoadMediaWithContext {
                                media_id: id.clone(),
                                context: ctx.clone(),
                            });
                        } else {
                            sender_clone.input(PlayerInput::LoadMedia(id.clone()));
                        }
                    }
                    glib::ControlFlow::Break
                }));
            }
            PlayerInput::ClearError => {
                self.error_message = None;
                self.retry_count = 0;
                if let Some(timer) = self.retry_timer.take() {
                    timer.remove();
                }
            }
            PlayerInput::ShowError(msg) => {
                error!("Player error: {}", msg);
                self.error_message = Some(msg);
                self.player_state = PlayerState::Error;
            }
            PlayerInput::EscapePressed => {
                // ESC key behavior: exit fullscreen if in fullscreen, otherwise navigate back
                if self.is_fullscreen {
                    sender.input(PlayerInput::ToggleFullscreen);
                } else {
                    sender.input(PlayerInput::NavigateBack);
                }
            }
            PlayerInput::NavigateBack => {
                // Clear any timers and show cursor before navigating back
                if let Some(timer) = self.cursor_timer.take() {
                    timer.remove();
                }

                // Cancel any state timer
                if let ControlState::Visible { timer_id } = &mut self.control_state {
                    if let Some(timer) = timer_id.take() {
                        timer.remove();
                    }
                }

                // Show cursor before navigating
                if let Some(surface) = self.window.surface()
                    && let Some(cursor) = gtk::gdk::Cursor::from_name("default", None)
                {
                    surface.set_cursor(Some(&cursor));
                }

                // Navigate back
                sender.output(PlayerOutput::NavigateBack).unwrap();
            }
            PlayerInput::SetUpscalingMode(mode) => {
                if let Some(player) = &self.player {
                    self.current_upscaling_mode = mode;
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        let _ = player_handle.set_upscaling_mode(mode).await;
                        PlayerCommandOutput::StateChanged(PlayerState::Playing)
                    });

                    // Save preference to config
                    if let Ok(mut config) = Config::load() {
                        config.playback.mpv_upscaling_mode = mode.to_string().to_string();
                        let _ = config.save();
                    }

                    // Update menu to reflect new selection
                    self.populate_quality_menu(sender.clone());
                }
            }
            PlayerInput::UpdateQualityMenu => {
                self.populate_quality_menu(sender.clone());
            }
        }
    }

    async fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root,
    ) {
        match message {
            PlayerCommandOutput::StateChanged(state) => {
                self.player_state = state.clone();
                // Clear error on successful state change
                if !matches!(&state, PlayerState::Error) {
                    self.error_message = None;
                    self.retry_count = 0;
                }
                // Grab focus when media starts playing to ensure keyboard shortcuts work
                if matches!(&state, PlayerState::Playing) {
                    root.grab_focus();
                }
            }
            PlayerCommandOutput::LoadError(error_msg) => {
                // Send toast notification for immediate feedback
                sender
                    .output(PlayerOutput::ShowToast(error_msg.clone()))
                    .unwrap();

                // Show error in overlay
                sender.input(PlayerInput::ShowError(error_msg.clone()));

                // Navigate back after a small delay to ensure navigation stack is ready
                let sender_clone = sender.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                    sender_clone.output(PlayerOutput::NavigateBack).unwrap();
                });
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
                    if !self.is_seeking {
                        self.seek_bar.set_value(pos.as_secs_f64());
                    }

                    // Save playback progress to database at configured interval
                    if let (Some(media_id), Some(dur)) = (&self.media_item_id, duration) {
                        // Use cached config value instead of reloading config file
                        let save_interval_secs = self.config_progress_update_interval_seconds;

                        // Check if enough time has passed since last save
                        let elapsed = self.last_progress_save.elapsed().as_secs();

                        // Always save if watched (>90%) or if interval has passed
                        let watched = pos.as_secs_f64() / dur.as_secs_f64() > 0.9;

                        // Check for auto-play when episode is nearly complete (>95%)
                        let should_auto_play = pos.as_secs_f64() / dur.as_secs_f64() > 0.95;
                        if should_auto_play && !self.auto_play_triggered {
                            self.auto_play_triggered = true;

                            // Check if we have a playlist context with auto-play enabled
                            if let Some(ref context) = self.playlist_context
                                && context.is_auto_play_enabled()
                                && context.has_next()
                            {
                                info!("Auto-play triggered, loading next episode");

                                // Load next item after a short delay to let current one finish
                                let sender_clone = sender.clone();
                                let timeout_id = glib::timeout_add_seconds_local(3, move || {
                                    sender_clone.input(PlayerInput::Next);
                                    glib::ControlFlow::Break
                                });

                                // Store timeout ID in case we need to cancel (e.g., user manually navigates)
                                self.auto_play_timeout = Some(timeout_id);
                            }
                        }

                        if watched || elapsed >= save_interval_secs {
                            self.last_progress_save = std::time::Instant::now();

                            let db = (*self.db).clone();
                            let media_id = media_id.clone();
                            let position_ms = pos.as_millis() as i64;
                            let duration_ms = dur.as_millis() as i64;

                            // If we have a PlayQueue context, also sync with Plex server
                            if let Some(ref context) = self.playlist_context
                                && let Some(_queue_info) = context.get_play_queue_info()
                            {
                                // Clone the context for the async task
                                let context_clone = context.clone();
                                let db_clone = db.clone();
                                let media_id_clone = media_id.clone();
                                let position = pos;
                                let duration = dur;

                                glib::spawn_future_local(async move {
                                    use crate::db::repository::source_repository::SourceRepositoryImpl;
                                    use crate::db::repository::{MediaRepositoryImpl, Repository};
                                    use crate::services::core::BackendService;
                                    use crate::services::core::playqueue::PlayQueueService;

                                    // Get the media's source
                                    let media_repo = MediaRepositoryImpl::new(db_clone.clone());
                                    if let Ok(Some(media)) =
                                        media_repo.find_by_id(media_id_clone.as_ref()).await
                                        && let Ok(_source_id) = media.source_id.parse::<i32>()
                                    {
                                        let source_repo =
                                            SourceRepositoryImpl::new(db_clone.clone());
                                        if let Ok(Some(source)) =
                                            source_repo.find_by_id(&media.source_id).await
                                        {
                                            // Create backend and sync PlayQueue progress
                                            if let Ok(backend) =
                                                BackendService::create_backend_for_source(
                                                    &db_clone, &source,
                                                )
                                                .await
                                            {
                                                // PlayQueueService will handle the sync
                                                if let Err(e) =
                                                    PlayQueueService::update_progress_with_queue(
                                                        backend.as_any(),
                                                        &context_clone,
                                                        &media_id_clone,
                                                        position,
                                                        duration,
                                                        if watched { "stopped" } else { "playing" },
                                                    )
                                                    .await
                                                {
                                                    debug!(
                                                        "Failed to sync PlayQueue progress: {}",
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                    }
                                });
                            }

                            relm4::spawn(async move {
                                use crate::services::commands::{
                                    Command, UpdatePlaybackProgressCommand,
                                };

                                let command = UpdatePlaybackProgressCommand {
                                    db,
                                    media_id,
                                    position_ms,
                                    duration_ms,
                                    watched,
                                };

                                if let Err(e) = command.execute().await {
                                    debug!("Failed to save playback progress: {}", e);
                                }
                            });
                        }
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
