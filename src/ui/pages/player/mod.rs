use crate::config::Config;
use crate::models::{ChapterMarker, MediaItemId, PlaylistContext};
use crate::player::{PlayerController, PlayerHandle, PlayerState};
use crate::services::commands::Command;
use crate::services::config_service::CONFIG_SERVICE;
use crate::ui::shared::broker::{BROKER, BrokerMessage, ConfigMessage};
use adw::prelude::*;
use gtk::glib::{self, SourceId};
use libadwaita as adw;
use relm4::gtk;
use relm4::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

mod sleep_inhibition;
use sleep_inhibition::SleepInhibitor;
mod controls_visibility;
mod menu_builders;
use controls_visibility::ControlState;
mod backend_manager;
mod playlist_navigation;
mod skip_markers;
use skip_markers::SkipMarkerManager;
mod buffering_overlay;
use buffering_overlay::{BufferingOverlay, BufferingOverlayInput};
mod buffering_warnings;

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
    // Zoom control state
    zoom_menu_button: gtk::MenuButton,
    current_zoom_mode: crate::player::ZoomMode,
    zoom_label: gtk::Label,
    // Control widgets for bounds detection
    controls_overlay: Option<gtk::Box>,
    // Popover state tracking to prevent control hiding when popover is open
    active_popover_count: std::rc::Rc<std::cell::RefCell<usize>>,
    // Timing configuration
    inactivity_timeout_secs: u64,
    mouse_move_threshold: f64,
    window_event_debounce_ms: u64,
    // Skip intro/credits management
    skip_marker_manager: SkipMarkerManager,
    // Sleep inhibition
    sleep_inhibitor: SleepInhibitor,
    // Buffering overlay component
    buffering_overlay: Controller<BufferingOverlay>,
}

impl PlayerPage {
    // Configuration constants for control visibility behavior
    const DEFAULT_INACTIVITY_TIMEOUT_SECS: u64 = 3;
    const DEFAULT_MOUSE_MOVE_THRESHOLD: f64 = 5.0; // pixels
    const DEFAULT_WINDOW_EVENT_DEBOUNCE_MS: u64 = 50; // milliseconds
    const CONTROL_FADE_ANIMATION_MS: u64 = 200; // milliseconds for fade transition
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
    ClearAutoPlayTimeout,
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
    // Skip intro/credits
    SkipIntro,
    SkipCredits,
    UpdateSkipButtonsVisibility,
    HideSkipIntro,
    HideSkipCredits,
    LoadedMarkers {
        intro: Option<ChapterMarker>,
        credits: Option<ChapterMarker>,
    },
    // Message broker messages
    BrokerMsg(BrokerMessage),
    // Zoom controls
    SetZoomMode(crate::player::ZoomMode),
    CycleZoom,
    ZoomIn,
    ZoomOut,
    ZoomReset,
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

#[cfg(test)]
mod tests {
    use super::PlayerPage;

    #[test]
    fn gstreamer_selection_is_never_mpv() {
        assert!(!PlayerPage::backend_prefers_mpv("gstreamer"));
    }

    #[cfg(all(feature = "mpv", not(target_os = "macos")))]
    #[test]
    fn mpv_selection_detects_mpv() {
        assert!(PlayerPage::backend_prefers_mpv("mpv"));
        assert!(PlayerPage::backend_prefers_mpv("MPV"));
        assert!(PlayerPage::backend_prefers_mpv(""));
    }

    #[cfg(any(not(feature = "mpv"), target_os = "macos"))]
    #[test]
    fn mpv_selection_forced_false_when_unavailable() {
        assert!(!PlayerPage::backend_prefers_mpv("mpv"));
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

            // Buffering overlay
            add_overlay = model.buffering_overlay.widget(),

            // Skip intro button overlay
            add_overlay = &gtk::Box {
                set_halign: gtk::Align::End,
                set_valign: gtk::Align::End,
                set_margin_bottom: 140,
                set_margin_end: 20,
                #[watch]
                set_visible: model.skip_marker_manager.is_skip_intro_visible(),

                gtk::Button {
                    set_label: "Skip Intro",
                    add_css_class: "osd",
                    add_css_class: "pill",
                    connect_clicked => PlayerInput::SkipIntro,
                },
            },

            // Skip credits button overlay
            add_overlay = &gtk::Box {
                set_halign: gtk::Align::End,
                set_valign: gtk::Align::End,
                set_margin_bottom: 140,
                set_margin_end: 20,
                #[watch]
                set_visible: model.skip_marker_manager.is_skip_credits_visible(),

                gtk::Button {
                    set_label: "Skip Credits",
                    add_css_class: "osd",
                    add_css_class: "pill",
                    connect_clicked => PlayerInput::SkipCredits,
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

                        // Quality/Resolution button (hidden - doesn't work with current MPV embed)
                        model.quality_menu_button.clone() {
                            set_icon_name: "preferences-system-symbolic",
                            add_css_class: "flat",
                            set_tooltip_text: Some("Video Quality"),
                            set_visible: false,
                        },

                        // Zoom button
                        model.zoom_menu_button.clone() {
                            set_icon_name: "zoom-in-symbolic",
                            add_css_class: "flat",
                            set_tooltip_text: Some("Video Zoom"),
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
        css_provider.load_from_string(include_str!("../../../styles/player.css"));
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
        let zoom_menu_button = gtk::MenuButton::new();
        let zoom_label = gtk::Label::new(Some("Fit"));

        // Load config via the shared ConfigService so runtime updates stay in sync
        let config = CONFIG_SERVICE.get_config().await;

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
            current_upscaling_mode: Self::mpv_upscaling_mode_from_config(&config),
            is_mpv_backend: Self::backend_prefers_mpv(&config.playback.player_backend),
            zoom_menu_button: zoom_menu_button.clone(),
            current_zoom_mode: crate::player::ZoomMode::default(),
            zoom_label: zoom_label.clone(),
            controls_overlay: None, // Will be set when controls are created
            active_popover_count: std::rc::Rc::new(std::cell::RefCell::new(0)),
            inactivity_timeout_secs: Self::DEFAULT_INACTIVITY_TIMEOUT_SECS,
            mouse_move_threshold: Self::DEFAULT_MOUSE_MOVE_THRESHOLD,
            window_event_debounce_ms: Self::DEFAULT_WINDOW_EVENT_DEBOUNCE_MS,
            // Skip intro/credits management
            skip_marker_manager: SkipMarkerManager::new(
                config.playback.skip_intro_enabled,
                config.playback.skip_credits_enabled,
                config.playback.auto_skip_intro,
                config.playback.auto_skip_credits,
                config.playback.minimum_marker_duration_seconds as u64,
            ),
            // Sleep inhibition
            sleep_inhibitor: SleepInhibitor::new(),
            // Buffering overlay
            buffering_overlay: BufferingOverlay::builder().launch(()).detach(),
        };

        // Initialize the player controller
        match PlayerController::new(&config) {
            Ok((handle, controller)) => {
                model.attach_player_controller(handle, controller, &sender);
                model.error_message = None;
            }
            Err(e) => {
                error!("Failed to initialize player controller: {}", e);
                model.error_message = Some(format!("Failed to initialize player: {}", e));
                model.player_state = PlayerState::Error;
            }
        }

        // Setup seek bar handlers - handle clicks and drags directly for video seeking behavior
        {
            let sender_start = sender.clone();
            let sender_end = sender.clone();
            let seek_bar_for_click = model.seek_bar.clone();

            // Handle direct clicks and drags for video seeking
            let click_gesture = gtk::GestureClick::new();
            click_gesture.set_button(gtk::gdk::BUTTON_PRIMARY);

            // Start seeking on press
            click_gesture.connect_pressed(move |_gesture, _n_press, x, _y| {
                sender_start.input(PlayerInput::StartSeeking);

                // Calculate position from click location
                let widget_width = seek_bar_for_click.width() as f64;
                let adjustment = seek_bar_for_click.adjustment();
                let range = adjustment.upper() - adjustment.lower();
                let value = adjustment.lower() + (x / widget_width) * range;

                // Update the scale value and seek
                seek_bar_for_click.set_value(value);
                sender_start.input(PlayerInput::Seek(Duration::from_secs_f64(value.max(0.0))));
            });

            // End seeking on release
            click_gesture.connect_released(move |_gesture, _n_press, _x, _y| {
                sender_end.input(PlayerInput::StopSeeking);
            });

            model.seek_bar.add_controller(click_gesture);

            // Handle dragging
            let sender_drag = sender.clone();
            let seek_bar_for_drag = model.seek_bar.clone();
            let drag_gesture = gtk::GestureDrag::new();
            drag_gesture.set_button(gtk::gdk::BUTTON_PRIMARY);

            drag_gesture.connect_drag_update(move |_gesture, offset_x, _offset_y| {
                // Calculate position from drag location
                let widget_width = seek_bar_for_drag.width() as f64;
                let adjustment = seek_bar_for_drag.adjustment();
                let range = adjustment.upper() - adjustment.lower();

                // Get current position and add offset
                let current_value = seek_bar_for_drag.value();
                let value_per_pixel = range / widget_width;
                let new_value = (current_value + offset_x * value_per_pixel)
                    .clamp(adjustment.lower(), adjustment.upper());

                // Update the scale value and seek
                seek_bar_for_drag.set_value(new_value);
                sender_drag.input(PlayerInput::Seek(Duration::from_secs_f64(
                    new_value.max(0.0),
                )));
            });

            model.seek_bar.add_controller(drag_gesture);
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
                    gtk::gdk::Key::_0 if ctrl_pressed => {
                        // Ctrl+0: reset zoom
                        sender.input(PlayerInput::ZoomReset);
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
                    // Zoom controls
                    gtk::gdk::Key::z => {
                        if shift_pressed {
                            // Shift+Z: zoom out
                            sender.input(PlayerInput::ZoomOut);
                        } else {
                            // z: cycle zoom modes
                            sender.input(PlayerInput::CycleZoom);
                        }
                        glib::Propagation::Stop
                    }
                    gtk::gdk::Key::plus | gtk::gdk::Key::equal => {
                        // +/=: zoom in
                        sender.input(PlayerInput::ZoomIn);
                        glib::Propagation::Stop
                    }
                    gtk::gdk::Key::minus | gtk::gdk::Key::underscore => {
                        // -/_: zoom out
                        sender.input(PlayerInput::ZoomOut);
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

        // Subscribe to MessageBroker for config updates
        {
            let broker_sender = sender.input_sender().clone();
            relm4::spawn(async move {
                let (tx, rx) = relm4::channel::<BrokerMessage>();
                BROKER.subscribe("PlayerPage".to_string(), tx).await;

                while let Some(msg) = rx.recv().await {
                    broker_sender.send(PlayerInput::BrokerMsg(msg)).unwrap();
                }
            });
        }

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

                // Reset scrubber UI to prevent showing previous video's position
                self.seek_bar.set_value(0.0);
                self.position_label.set_text("0:00");
                self.duration_label.set_text("--:--");

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
                    // Timeout may have already fired and been auto-removed by GLib
                    // Use catch_unwind to handle the panic gracefully
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| timeout.remove()))
                        .is_err()
                    {
                        debug!("Auto-play timeout already removed (likely fired)");
                    }
                }
                // Clear skip button state
                self.skip_marker_manager.clear_markers();

                // Load marker data from database and fetch from backend if missing
                let db_clone_for_markers = self.db.clone();
                let media_id_for_markers = id.clone();
                let sender_for_markers = sender.clone();
                glib::spawn_future_local(async move {
                    use crate::db::repository::{MediaRepository, MediaRepositoryImpl, Repository};
                    use crate::models::{MediaItem, MediaItemId};
                    use crate::services::core::backend::BackendService;

                    let media_repo =
                        MediaRepositoryImpl::new(db_clone_for_markers.as_ref().clone());
                    if let Ok(Some(mut db_media)) =
                        media_repo.find_by_id(media_id_for_markers.as_ref()).await
                    {
                        // Check if markers are missing in database
                        let markers_missing = db_media.intro_marker_start_ms.is_none()
                            && db_media.credits_marker_start_ms.is_none();

                        if markers_missing {
                            // Fetch markers from backend
                            let media_id_typed = MediaItemId::new(media_id_for_markers.to_string());
                            match BackendService::fetch_markers(
                                &db_clone_for_markers,
                                &media_id_typed,
                            )
                            .await
                            {
                                Ok((intro_marker, credits_marker)) => {
                                    // Store markers in database
                                    let intro_tuple =
                                        intro_marker.as_ref().map(|(start, end)| (*start, *end));
                                    let credits_tuple =
                                        credits_marker.as_ref().map(|(start, end)| (*start, *end));

                                    if let Err(e) = media_repo
                                        .update_markers(
                                            media_id_for_markers.as_ref(),
                                            intro_tuple,
                                            credits_tuple,
                                        )
                                        .await
                                    {
                                        tracing::warn!(
                                            "Failed to store markers in database: {}",
                                            e
                                        );
                                    } else {
                                        // Update local db_media with the new markers
                                        if let Some((start, end)) = intro_tuple {
                                            db_media.intro_marker_start_ms = Some(start);
                                            db_media.intro_marker_end_ms = Some(end);
                                        }
                                        if let Some((start, end)) = credits_tuple {
                                            db_media.credits_marker_start_ms = Some(start);
                                            db_media.credits_marker_end_ms = Some(end);
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::debug!(
                                        "Could not fetch markers from backend: {} (this is normal if markers aren't available)",
                                        e
                                    );
                                }
                            }
                        }

                        // Convert to domain model to get markers
                        if let Ok(media_item) = MediaItem::try_from(db_media) {
                            match media_item {
                                MediaItem::Movie(movie) => {
                                    sender_for_markers.input(PlayerInput::LoadedMarkers {
                                        intro: movie.intro_marker,
                                        credits: movie.credits_marker,
                                    });
                                }
                                MediaItem::Episode(episode) => {
                                    sender_for_markers.input(PlayerInput::LoadedMarkers {
                                        intro: episode.intro_marker,
                                        credits: episode.credits_marker,
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                });

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
                        use crate::ui::shared::commands::{
                            AppCommand, CommandResult, execute_command,
                        };

                        // Use the proper StartPlayback command which includes cache integration
                        let command_result = execute_command(
                            AppCommand::StartPlayback {
                                media_id: media_id.to_string(),
                            },
                            &db_clone,
                        )
                        .await;

                        let stream_url = match command_result {
                            CommandResult::PlaybackStarted { url, .. } => url,
                            CommandResult::Error(e) => {
                                error!("Failed to start playback: {}", e);
                                return PlayerCommandOutput::LoadError(format!(
                                    "Failed to load media: {}",
                                    e
                                ));
                            }
                        };

                        info!("Got stream URL (potentially cached): {}", stream_url);

                        // Load the media into the player using channel-based API
                        match player_handle.load_media(&stream_url).await {
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
                                    if let Ok(Some(progress)) = (GetPlaybackProgressCommand {
                                        db: db_clone.as_ref().clone(),
                                        media_id: media_id_for_resume.clone(),
                                        user_id: "default".to_string(), // TODO: Get actual user ID
                                    })
                                    .execute()
                                    .await
                                    {
                                        // Only resume if:
                                        // 1. Position is above threshold (e.g., 5 seconds)
                                        // 2. Progress is less than 95% (not near completion)
                                        // 3. Media is not marked as watched
                                        let threshold_ms = (resume_threshold_seconds as i64) * 1000;
                                        let progress_percentage =
                                            progress.get_progress_percentage();

                                        if progress.position_ms > threshold_ms
                                            && progress_percentage < 0.95
                                            && !progress.watched
                                        {
                                            let resume_position = std::time::Duration::from_millis(
                                                progress.position_ms as u64,
                                            );
                                            info!(
                                                "Resuming playback from {:?} ({:.1}% complete)",
                                                resume_position,
                                                progress_percentage * 100.0
                                            );

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

                                // Start playback automatically after loading
                                if let Err(e) = player_handle.play().await {
                                    warn!("Failed to auto-start playback: {}", e);
                                }

                                // Get the actual state from the player after loading and playing
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

                // Reset scrubber UI to prevent showing previous video's position
                self.seek_bar.set_value(0.0);
                self.position_label.set_text("0:00");
                self.duration_label.set_text("--:--");

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
                    // Timeout may have already fired and been auto-removed by GLib
                    // Use catch_unwind to handle the panic gracefully
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| timeout.remove()))
                        .is_err()
                    {
                        debug!("Auto-play timeout already removed (likely fired)");
                    }
                }
                // Clear skip button state
                self.skip_marker_manager.clear_markers();

                // Load marker data from database and fetch from backend if missing
                let db_clone_for_markers = self.db.clone();
                let media_id_for_markers = media_id.clone();
                let sender_for_markers = sender.clone();
                glib::spawn_future_local(async move {
                    use crate::db::repository::{MediaRepository, MediaRepositoryImpl, Repository};
                    use crate::models::{MediaItem, MediaItemId};
                    use crate::services::core::backend::BackendService;

                    let media_repo =
                        MediaRepositoryImpl::new(db_clone_for_markers.as_ref().clone());
                    if let Ok(Some(mut db_media)) =
                        media_repo.find_by_id(media_id_for_markers.as_ref()).await
                    {
                        // Check if markers are missing in database
                        let markers_missing = db_media.intro_marker_start_ms.is_none()
                            && db_media.credits_marker_start_ms.is_none();

                        if markers_missing {
                            // Fetch markers from backend
                            let media_id_typed = MediaItemId::new(media_id_for_markers.to_string());
                            match BackendService::fetch_markers(
                                &db_clone_for_markers,
                                &media_id_typed,
                            )
                            .await
                            {
                                Ok((intro_marker, credits_marker)) => {
                                    // Store markers in database
                                    let intro_tuple =
                                        intro_marker.as_ref().map(|(start, end)| (*start, *end));
                                    let credits_tuple =
                                        credits_marker.as_ref().map(|(start, end)| (*start, *end));

                                    if let Err(e) = media_repo
                                        .update_markers(
                                            media_id_for_markers.as_ref(),
                                            intro_tuple,
                                            credits_tuple,
                                        )
                                        .await
                                    {
                                        tracing::warn!(
                                            "Failed to store markers in database: {}",
                                            e
                                        );
                                    } else {
                                        // Update local db_media with the new markers
                                        if let Some((start, end)) = intro_tuple {
                                            db_media.intro_marker_start_ms = Some(start);
                                            db_media.intro_marker_end_ms = Some(end);
                                        }
                                        if let Some((start, end)) = credits_tuple {
                                            db_media.credits_marker_start_ms = Some(start);
                                            db_media.credits_marker_end_ms = Some(end);
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::debug!(
                                        "Could not fetch markers from backend: {} (this is normal if markers aren't available)",
                                        e
                                    );
                                }
                            }
                        }

                        // Convert to domain model to get markers
                        if let Ok(media_item) = MediaItem::try_from(db_media) {
                            match media_item {
                                MediaItem::Movie(movie) => {
                                    sender_for_markers.input(PlayerInput::LoadedMarkers {
                                        intro: movie.intro_marker,
                                        credits: movie.credits_marker,
                                    });
                                }
                                MediaItem::Episode(episode) => {
                                    sender_for_markers.input(PlayerInput::LoadedMarkers {
                                        intro: episode.intro_marker,
                                        credits: episode.credits_marker,
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                });

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
                        use crate::ui::shared::commands::{
                            AppCommand, CommandResult, execute_command,
                        };

                        // Use the proper StartPlayback command which includes cache integration
                        let command_result = execute_command(
                            AppCommand::StartPlayback {
                                media_id: media_id_clone.to_string(),
                            },
                            &db_clone,
                        )
                        .await;

                        let stream_url = match command_result {
                            CommandResult::PlaybackStarted { url, .. } => url,
                            CommandResult::Error(e) => {
                                error!("Failed to start playback: {}", e);
                                return PlayerCommandOutput::LoadError(format!(
                                    "Failed to load media: {}",
                                    e
                                ));
                            }
                        };

                        info!("Got stream URL (potentially cached): {}", stream_url);

                        // Load the media into the player using channel-based API
                        match player_handle.load_media(&stream_url).await {
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
                                    if let Ok(Some(progress)) = (GetPlaybackProgressCommand {
                                        db: db_clone.as_ref().clone(),
                                        media_id: media_id_for_resume.clone(),
                                        user_id: "default".to_string(), // TODO: Get actual user ID
                                    })
                                    .execute()
                                    .await
                                    {
                                        // Only resume if:
                                        // 1. Position is above threshold (e.g., 5 seconds)
                                        // 2. Progress is less than 95% (not near completion)
                                        // 3. Media is not marked as watched
                                        let threshold_ms = (resume_threshold_seconds as i64) * 1000;
                                        let progress_percentage =
                                            progress.get_progress_percentage();

                                        if progress.position_ms > threshold_ms
                                            && progress_percentage < 0.95
                                            && !progress.watched
                                        {
                                            let resume_position = std::time::Duration::from_millis(
                                                progress.position_ms as u64,
                                            );
                                            info!(
                                                "Resuming playback from {:?} ({:.1}% complete)",
                                                resume_position,
                                                progress_percentage * 100.0
                                            );

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

                                // Start playback automatically after loading
                                if let Err(e) = player_handle.play().await {
                                    warn!("Failed to auto-start playback: {}", e);
                                }

                                // Get the actual state from the player after loading and playing
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
                    let _ = timer.remove();
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
                    let _ = timer.remove();
                }

                // Don't hide if a popover is open
                if *self.active_popover_count.borrow() > 0 {
                    debug!("Popover is open, not hiding controls on window leave");
                    return;
                }

                // Process immediately - hide when leaving window (unless popover is open)
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
                self.handle_previous_navigation(&sender);
            }
            PlayerInput::Next => {
                self.handle_next_navigation(&sender);
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
                self.populate_zoom_menu(sender.clone());

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
            PlayerInput::UpdateSkipButtonsVisibility => {
                self.skip_marker_manager
                    .update_visibility(self.position, &sender);
            }
            PlayerInput::HideSkipIntro => {
                self.skip_marker_manager.hide_skip_intro();
            }
            PlayerInput::HideSkipCredits => {
                self.skip_marker_manager.hide_skip_credits();
            }
            PlayerInput::SkipIntro => {
                self.skip_marker_manager.skip_intro(&sender);
            }
            PlayerInput::SkipCredits => {
                self.skip_marker_manager.skip_credits(&sender);
            }
            PlayerInput::LoadedMarkers { intro, credits } => {
                self.skip_marker_manager.load_markers(intro, credits);
                // Trigger visibility check
                sender.input(PlayerInput::UpdateSkipButtonsVisibility);
            }
            PlayerInput::BrokerMsg(msg) => {
                match msg {
                    BrokerMessage::Config(ConfigMessage::Updated { config }) => {
                        self.handle_config_update(config.as_ref(), &sender).await;
                    }
                    BrokerMessage::Config(ConfigMessage::PlayerBackendChanged { backend }) => {
                        self.ensure_backend_alignment(&backend, &sender).await;
                    }
                    _ => {
                        // Ignore other broker messages
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
                    let _ = timer.remove();
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
                    let _ = timer.remove();
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
                    let _ = timer.remove();
                }

                // Cancel any state timer
                if let ControlState::Visible { timer_id } = &mut self.control_state
                    && let Some(timer) = timer_id.take()
                {
                    let _ = timer.remove();
                }

                // Exit fullscreen mode and reset state before navigating back
                if self.is_fullscreen {
                    debug!("Exiting fullscreen before navigating back from player");
                    self.window.unfullscreen();
                    self.is_fullscreen = false;
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
            PlayerInput::ClearAutoPlayTimeout => {
                // Clear the auto-play timeout reference without trying to remove it
                // (it's about to be removed automatically by GLib when it fires)
                self.auto_play_timeout = None;
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
            PlayerInput::SetZoomMode(mode) => {
                if let Some(player) = &self.player {
                    self.current_zoom_mode = mode;
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        let _ = player_handle.set_zoom_mode(mode).await;
                        PlayerCommandOutput::StateChanged(PlayerState::Playing)
                    });
                    // Update menu to reflect new selection
                    self.populate_zoom_menu(sender.clone());
                    // Update zoom label
                    self.zoom_label.set_text(&mode.to_string());
                }
            }
            PlayerInput::CycleZoom => {
                // Cycle through common zoom modes
                let next_mode = match self.current_zoom_mode {
                    crate::player::ZoomMode::Fit => crate::player::ZoomMode::Fill,
                    crate::player::ZoomMode::Fill => crate::player::ZoomMode::Zoom16_9,
                    crate::player::ZoomMode::Zoom16_9 => crate::player::ZoomMode::Zoom4_3,
                    crate::player::ZoomMode::Zoom4_3 => crate::player::ZoomMode::Zoom2_35,
                    crate::player::ZoomMode::Zoom2_35 => crate::player::ZoomMode::Fit,
                    crate::player::ZoomMode::Custom(_) => crate::player::ZoomMode::Fit,
                };
                sender.input(PlayerInput::SetZoomMode(next_mode));
            }
            PlayerInput::ZoomIn => {
                let new_level = match self.current_zoom_mode {
                    crate::player::ZoomMode::Custom(level) => (level + 0.1).min(3.0),
                    _ => 1.1,
                };
                sender.input(PlayerInput::SetZoomMode(crate::player::ZoomMode::Custom(
                    new_level,
                )));
            }
            PlayerInput::ZoomOut => {
                let new_level = match self.current_zoom_mode {
                    crate::player::ZoomMode::Custom(level) => (level - 0.1).max(0.5),
                    _ => 0.9,
                };
                sender.input(PlayerInput::SetZoomMode(crate::player::ZoomMode::Custom(
                    new_level,
                )));
            }
            PlayerInput::ZoomReset => {
                sender.input(PlayerInput::SetZoomMode(crate::player::ZoomMode::Fit));
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
                    // Enable sleep inhibition when playback starts
                    self.sleep_inhibitor.setup(&self.window);
                }

                // Release sleep inhibition when playback stops, pauses, or errors
                if matches!(
                    &state,
                    PlayerState::Paused | PlayerState::Stopped | PlayerState::Error
                ) {
                    self.sleep_inhibitor.release(&self.window);
                }

                // Send immediate timeline update on play, pause, or stop state changes
                if matches!(
                    &state,
                    PlayerState::Playing | PlayerState::Paused | PlayerState::Stopped
                ) {
                    if let (Some(media_id), Some(player)) = (&self.media_item_id, &self.player) {
                        let player_handle = player.clone();
                        let media_id_clone = media_id.clone();
                        let state_clone = state.clone();
                        let db_clone = (*self.db).clone();
                        let context_clone = self.playlist_context.clone();

                        glib::spawn_future_local(async move {
                            // Get current position and duration
                            if let Ok(Some(position)) = player_handle.get_position().await
                                && let Ok(Some(duration)) = player_handle.get_duration().await
                            {
                                use crate::db::repository::source_repository::SourceRepositoryImpl;
                                use crate::db::repository::{MediaRepositoryImpl, Repository};
                                use crate::services::core::BackendService;
                                use crate::services::core::playqueue::PlayQueueService;

                                // Map player state to Plex state
                                let plex_state = match state_clone {
                                    PlayerState::Playing => "playing",
                                    PlayerState::Paused => "paused",
                                    PlayerState::Stopped => "stopped",
                                    _ => return,
                                };

                                // If we have a PlayQueue context, sync with server
                                if let Some(ref context) = context_clone
                                    && let Some(_queue_info) = context.get_play_queue_info()
                                {
                                    let media_repo = MediaRepositoryImpl::new(db_clone.clone());
                                    if let Ok(Some(media)) =
                                        media_repo.find_by_id(media_id_clone.as_ref()).await
                                    {
                                        let source_repo =
                                            SourceRepositoryImpl::new(db_clone.clone());
                                        if let Ok(Some(source)) =
                                            source_repo.find_by_id(&media.source_id).await
                                        {
                                            if let Ok(backend) =
                                                BackendService::create_backend_for_source(
                                                    &db_clone, &source,
                                                )
                                                .await
                                            {
                                                let _ =
                                                    PlayQueueService::update_progress_with_queue(
                                                        backend.as_any(),
                                                        &context,
                                                        &media_id_clone,
                                                        position,
                                                        duration,
                                                        plex_state,
                                                    )
                                                    .await;
                                            }
                                        }
                                    }
                                }
                            }
                        });
                    }
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

                    // Check skip button visibility based on position
                    sender.input(PlayerInput::UpdateSkipButtonsVisibility);

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
                            if let Some(ref context) = self.playlist_context {
                                if context.is_auto_play_enabled() {
                                    if context.has_next() {
                                        info!("Auto-play triggered, loading next episode");

                                        // Load next item after a short delay to let current one finish
                                        let sender_clone = sender.clone();
                                        let timeout_id =
                                            glib::timeout_add_seconds_local(3, move || {
                                                // Clear the timeout reference before it's auto-removed by GLib
                                                sender_clone
                                                    .input(PlayerInput::ClearAutoPlayTimeout);
                                                sender_clone.input(PlayerInput::Next);
                                                glib::ControlFlow::Break
                                            });

                                        // Store timeout ID in case we need to cancel (e.g., user manually navigates)
                                        self.auto_play_timeout = Some(timeout_id);
                                    } else {
                                        info!(
                                            "Episode ending without next episode, will navigate back"
                                        );

                                        // No next episode available - navigate back after a delay
                                        // This delay allows watch status to be saved and synced before navigation
                                        let sender_clone = sender.clone();
                                        let timeout_id =
                                            glib::timeout_add_seconds_local(5, move || {
                                                // Clear the timeout reference before it's auto-removed by GLib
                                                sender_clone
                                                    .input(PlayerInput::ClearAutoPlayTimeout);
                                                sender_clone.input(PlayerInput::NavigateBack);
                                                glib::ControlFlow::Break
                                            });

                                        // Store timeout ID in case we need to cancel (e.g., user manually navigates)
                                        self.auto_play_timeout = Some(timeout_id);

                                        // Show toast notification to user
                                        sender
                                            .output(PlayerOutput::ShowToast(
                                                "End of season".to_string(),
                                            ))
                                            .unwrap();
                                    }
                                } else {
                                    debug!(
                                        "Episode ending with auto-play disabled, letting video finish naturally"
                                    );
                                    // Auto-play is disabled, let the video finish naturally
                                    // User can manually navigate back when they're ready
                                }
                            } else {
                                debug!(
                                    "Episode ending without playlist context, letting video finish naturally"
                                );
                                // No playlist context - this is a standalone video
                                // Let it finish naturally, user can manually navigate back
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
                                let player_state_clone = self.player_state.clone();

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
                                                // Map player state to Plex state
                                                let plex_state = if watched {
                                                    "stopped"
                                                } else {
                                                    match player_state_clone {
                                                        PlayerState::Playing => "playing",
                                                        PlayerState::Paused => "paused",
                                                        PlayerState::Stopped => "stopped",
                                                        PlayerState::Loading => "buffering",
                                                        _ => "playing",
                                                    }
                                                };

                                                if let Err(e) =
                                                    PlayQueueService::update_progress_with_queue(
                                                        backend.as_any(),
                                                        &context_clone,
                                                        &media_id_clone,
                                                        position,
                                                        duration,
                                                        plex_state,
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

    fn shutdown(&mut self, _widgets: &mut Self::Widgets, _output: relm4::Sender<Self::Output>) {
        // Unsubscribe from MessageBroker
        relm4::spawn(async move {
            BROKER.unsubscribe("PlayerPage").await;
        });

        // Restore cursor visibility when player is destroyed
        if let Some(surface) = self.window.surface()
            && let Some(cursor) = gtk::gdk::Cursor::from_name("default", None)
        {
            surface.set_cursor(Some(&cursor));
        }

        // Clean up any active timers
        if let Some(timer) = self.cursor_timer.take() {
            let _ = timer.remove();
        }

        // Cancel any visible state timer
        if let ControlState::Visible { timer_id } = &mut self.control_state
            && let Some(timer) = timer_id.take()
        {
            let _ = timer.remove();
        }

        // Clean up window event debounce timer
        if let Some(timer) = self.window_event_debounce.take() {
            let _ = timer.remove();
        }

        // Clean up retry timer
        if let Some(timer) = self.retry_timer.take() {
            let _ = timer.remove();
        }

        // Clean up auto-play timeout
        if let Some(timer) = self.auto_play_timeout.take() {
            // Timeout may have already fired and been auto-removed by GLib
            // Use catch_unwind to handle the panic gracefully
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| timer.remove())).is_err() {
                debug!("Auto-play timeout already removed (likely fired)");
            }
        }

        // Release sleep inhibition
        self.sleep_inhibitor.release(&self.window);

        tracing::debug!("PlayerPage shutdown: restored cursor and cleaned up timers");
    }
}
