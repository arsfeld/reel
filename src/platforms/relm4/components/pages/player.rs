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
    // UI state
    show_controls: bool,
    is_fullscreen: bool,
    controls_timer: Option<SourceId>,
    cursor_timer: Option<SourceId>,
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
    current_audio_track: i32,
    current_subtitle_track: i32,
}

impl PlayerPage {
    fn populate_audio_menu(&self, sender: AsyncComponentSender<Self>) {
        if let Some(player) = &self.player {
            let player_clone = player.clone();
            let audio_menu_button = self.audio_menu_button.clone();
            let current_track = self.current_audio_track;
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
                        let item = gtk::gio::MenuItem::new(Some(&track_name), None);
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
            let current_track = self.current_subtitle_track;
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
                        let item = gtk::gio::MenuItem::new(Some(&track_name), None);
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
    ShowControls,
    HideControls,
    ResetControlsTimer,
    Previous,
    Next,
    ShowCursor,
    HideCursor,
    ResetCursorTimer,
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
    // Control visibility
    ToggleControlsVisibility,
    // Relative seeking
    SeekRelative(i64), // Positive for forward, negative for backward
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
                set_visible: model.show_controls && model.error_message.is_none(),
                #[watch]
                set_opacity: if model.show_controls && model.error_message.is_none() { 1.0 } else { 0.0 },
                add_css_class: "osd",
                add_css_class: "player-controls",
                add_css_class: "minimal",

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
                        gtk::MenuButton {
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
                if !window_clone.is_fullscreen() {
                    if let Some(surface) = window_clone.surface() {
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
            let width = scale.allocated_width() as f64;
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
            show_controls: true,
            is_fullscreen: false,
            controls_timer: None,
            cursor_timer: None,
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
            current_audio_track: -1,
            current_subtitle_track: -1,
        };

        // Initialize the player controller
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

                model.player = Some(handle);
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
            let sender_for_fullscreen_check = sender.clone();
            let key_controller = gtk::EventControllerKey::new();
            key_controller.connect_key_pressed(move |controller, key, _keycode, modifiers| {
                use gtk::gdk::ModifierType;

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
                // Clear context when loading without context
                self.playlist_context = None;
                // No navigation available for single items
                self.can_go_previous = false;
                self.can_go_next = false;
                // Clear playlist position label
                self.playlist_position_label.set_text("");
                // Clear any existing error message
                self.error_message = None;

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
                                let user_message = match e.to_string().as_str() {
                                    s if s.contains("network") || s.contains("connection") =>
                                        "Network connection error. Please check your internet connection.".to_string(),
                                    s if s.contains("unauthorized") || s.contains("401") =>
                                        "Authentication failed. Please check your server credentials.".to_string(),
                                    s if s.contains("not found") || s.contains("404") =>
                                        "Media not found on server. It may have been removed.".to_string(),
                                    s if s.contains("timeout") =>
                                        "Server connection timed out. Please try again.".to_string(),
                                    _ => format!("Failed to load media: {}", e)
                                };
                                return PlayerCommandOutput::LoadError(user_message);
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
                                    // Get saved progress
                                    if let Ok(Some((position_ms, _duration_ms))) =
                                        (GetPlaybackProgressCommand {
                                            db: db_clone.as_ref().clone(),
                                            media_id: media_id_for_resume.clone(),
                                            user_id: "default".to_string(), // TODO: Get actual user ID
                                        }).execute().await
                                    {
                                        // Only resume if we've watched more than the threshold
                                        let threshold_ms = (resume_threshold_seconds as i64) * 1000;
                                        if position_ms > threshold_ms {
                                            let resume_position = std::time::Duration::from_millis(position_ms as u64);
                                            info!("Resuming playback from {:?}", resume_position);

                                            // Seek to saved position
                                            if let Err(e) = player_handle.seek(resume_position).await {
                                                warn!("Failed to seek to saved position: {}", e);
                                            }
                                        }
                                    }
                                }

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

                                // Get the actual state from the player after loading
                                let actual_state = player_handle
                                    .get_state()
                                    .await
                                    .unwrap_or(PlayerState::Idle);
                                PlayerCommandOutput::StateChanged(actual_state)
                            }
                            Err(e) => {
                                error!("Failed to load media: {}", e);
                                let user_message = match e.to_string().as_str() {
                                    s if s.contains("codec") || s.contains("decoder") =>
                                        "Media format not supported. The file may use an incompatible codec.".to_string(),
                                    s if s.contains("permission") || s.contains("access") =>
                                        "Permission denied. Check file or server access rights.".to_string(),
                                    s if s.contains("memory") =>
                                        "Not enough memory to play this media.".to_string(),
                                    _ => format!("Playback error: {}", e)
                                };
                                PlayerCommandOutput::LoadError(user_message)
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
                                let user_message = match e.to_string().as_str() {
                                    s if s.contains("network") || s.contains("connection") =>
                                        "Network connection error. Please check your internet connection.".to_string(),
                                    s if s.contains("unauthorized") || s.contains("401") =>
                                        "Authentication failed. Please check your server credentials.".to_string(),
                                    s if s.contains("not found") || s.contains("404") =>
                                        "Media not found on server. It may have been removed.".to_string(),
                                    s if s.contains("timeout") =>
                                        "Server connection timed out. Please try again.".to_string(),
                                    _ => format!("Failed to load media: {}", e)
                                };
                                return PlayerCommandOutput::LoadError(user_message);
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
                                    // Get saved progress
                                    if let Ok(Some((position_ms, _duration_ms))) =
                                        (GetPlaybackProgressCommand {
                                            db: db_clone.as_ref().clone(),
                                            media_id: media_id_for_resume.clone(),
                                            user_id: "default".to_string(), // TODO: Get actual user ID
                                        }).execute().await
                                    {
                                        // Only resume if we've watched more than the threshold
                                        let threshold_ms = (resume_threshold_seconds as i64) * 1000;
                                        if position_ms > threshold_ms {
                                            let resume_position = std::time::Duration::from_millis(position_ms as u64);
                                            info!("Resuming playback from {:?}", resume_position);

                                            // Seek to saved position
                                            if let Err(e) = player_handle.seek(resume_position).await {
                                                warn!("Failed to seek to saved position: {}", e);
                                            }
                                        }
                                    }
                                }

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

                                // Get the actual state from the player after loading
                                let actual_state = player_handle
                                    .get_state()
                                    .await
                                    .unwrap_or(PlayerState::Idle);
                                PlayerCommandOutput::StateChanged(actual_state)
                            }
                            Err(e) => {
                                error!("Failed to load media: {}", e);
                                let user_message = match e.to_string().as_str() {
                                    s if s.contains("codec") || s.contains("decoder") =>
                                        "Media format not supported. The file may use an incompatible codec.".to_string(),
                                    s if s.contains("permission") || s.contains("access") =>
                                        "Permission denied. Check file or server access rights.".to_string(),
                                    s if s.contains("memory") =>
                                        "Not enough memory to play this media.".to_string(),
                                    _ => format!("Playback error: {}", e)
                                };
                                PlayerCommandOutput::LoadError(user_message)
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
                if self.player_state == PlayerState::Paused {
                    if let Some(player) = &self.player {
                        let player_handle = player.clone();
                        sender.oneshot_command(async move {
                            player_handle.frame_step_forward().await.ok();
                            PlayerCommandOutput::StateChanged(PlayerState::Paused)
                        });
                    }
                }
            }
            PlayerInput::FrameStepBackward => {
                // Step one frame backward (while paused)
                if self.player_state == PlayerState::Paused {
                    if let Some(player) = &self.player {
                        let player_handle = player.clone();
                        sender.oneshot_command(async move {
                            player_handle.frame_step_backward().await.ok();
                            PlayerCommandOutput::StateChanged(PlayerState::Paused)
                        });
                    }
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

                // Also get current track selections
                if let Some(player) = &self.player {
                    let player_handle = player.clone();
                    let sender_clone = sender.clone();
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
                    self.current_audio_track = track_id;
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        let _ = player_handle.set_audio_track(track_id).await;
                        PlayerCommandOutput::StateChanged(PlayerState::Playing)
                    });
                }
            }
            PlayerInput::SetSubtitleTrack(track_id) => {
                if let Some(player) = &self.player {
                    self.current_subtitle_track = track_id;
                    let player_handle = player.clone();
                    sender.oneshot_command(async move {
                        let _ = player_handle.set_subtitle_track(track_id).await;
                        PlayerCommandOutput::StateChanged(PlayerState::Playing)
                    });
                }
            }
            PlayerInput::ToggleControlsVisibility => {
                // Toggle controls visibility
                self.show_controls = !self.show_controls;
                if self.show_controls {
                    // Reset timer when showing controls
                    sender.input(PlayerInput::ResetControlsTimer);
                    sender.input(PlayerInput::ResetCursorTimer);
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
                // Clear cursor timer and show cursor before navigating back
                if let Some(timer) = self.cursor_timer.take() {
                    timer.remove();
                }
                if let Some(timer) = self.controls_timer.take() {
                    timer.remove();
                }
                sender.input(PlayerInput::ShowCursor);
                // Navigate back
                sender.output(PlayerOutput::NavigateBack).unwrap();
            }
        }
    }

    async fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            PlayerCommandOutput::StateChanged(state) => {
                self.player_state = state.clone();
                // Clear error on successful state change
                if !matches!(&state, PlayerState::Error) {
                    self.error_message = None;
                    self.retry_count = 0;
                }
            }
            PlayerCommandOutput::LoadError(error_msg) => {
                sender.input(PlayerInput::ShowError(error_msg));
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

                        if watched || elapsed >= save_interval_secs {
                            self.last_progress_save = std::time::Instant::now();

                            let db = (*self.db).clone();
                            let media_id = media_id.clone();
                            let position_ms = pos.as_millis() as i64;
                            let duration_ms = dur.as_millis() as i64;

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
