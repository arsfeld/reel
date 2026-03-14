use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use relm4::prelude::*;
use tracing::info;

use crate::components::player::drop_target;
use crate::components::player::shortcuts::{self, PlayerAction};
use crate::components::player::video_area::{VideoArea, VideoAreaMsg, VideoAreaOutput};
use crate::player::backend::{self, PlayState};
use crate::services::screensaver::ScreensaverInhibitor;
use crate::services::window_state::{self, WindowState};

pub struct App {
    video_area: Controller<VideoArea>,
    screensaver: ScreensaverInhibitor,
    current_speed: f64,
    current_volume: f64,
    has_video: bool,
    #[allow(dead_code)]
    file_to_load: Option<String>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum AppMsg {
    TogglePause,
    OpenFile(String),
    VideoOutput(VideoAreaOutput),
    ShowFileChooser,
    ToggleFullscreen,
    ExitFullscreen,
    SeekRelative(f64),
    VolumeStep(f64),
    ToggleMute,
    SetSpeed(f64),
    SpeedUp,
    SpeedDown,
    SpeedReset,
    FilesDropped(String),
}

#[relm4::component(pub)]
impl Component for App {
    type Init = Option<String>;
    type Input = AppMsg;
    type Output = ();
    type CommandOutput = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("Reel"),
            set_default_width: 1280,
            set_default_height: 720,

            #[name = "toast_overlay"]
            adw::ToastOverlay {
                gtk4::Box {
                    set_orientation: gtk4::Orientation::Vertical,

                    model.video_area.widget() -> &gtk4::Overlay {
                        set_vexpand: true,
                    },
                },
            },

            add_controller = gtk4::EventControllerKey {
                connect_key_pressed[sender] => move |_controller, key, _code, mods| {
                    if let Some(action) = shortcuts::map_key_to_action(key, mods, false) {
                        match action {
                            PlayerAction::TogglePause => sender.input(AppMsg::TogglePause),
                            PlayerAction::SeekForward(s) => sender.input(AppMsg::SeekRelative(s)),
                            PlayerAction::SeekBackward(s) => sender.input(AppMsg::SeekRelative(-s)),
                            PlayerAction::VolumeUp(v) => sender.input(AppMsg::VolumeStep(v)),
                            PlayerAction::VolumeDown(v) => sender.input(AppMsg::VolumeStep(-v)),
                            PlayerAction::ToggleMute => sender.input(AppMsg::ToggleMute),
                            PlayerAction::ToggleFullscreen => sender.input(AppMsg::ToggleFullscreen),
                            PlayerAction::ExitFullscreen => sender.input(AppMsg::ExitFullscreen),
                            PlayerAction::SpeedUp => sender.input(AppMsg::SpeedUp),
                            PlayerAction::SpeedDown => sender.input(AppMsg::SpeedDown),
                            PlayerAction::SpeedReset => sender.input(AppMsg::SpeedReset),
                        }
                        glib::Propagation::Stop
                    } else {
                        glib::Propagation::Proceed
                    }
                },
            },

            // Drag-and-drop support
            add_controller = gtk4::DropTarget {
                set_actions: gtk4::gdk::DragAction::COPY,
                set_types: &[gtk4::glib::types::Type::STRING],
                connect_drop[sender] => move |_target, value, _x, _y| {
                    if let Ok(text) = value.get::<String>() {
                        for uri in drop_target::parse_uri_list(&text) {
                            sender.input(AppMsg::FilesDropped(uri));
                        }
                        return true;
                    }
                    false
                },
            },
        }
    }

    fn init(
        file_arg: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let video_area = VideoArea::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::VideoOutput);

        // Load window state
        let ws = window_state::load();

        let model = Self {
            video_area,
            screensaver: ScreensaverInhibitor::new(),
            current_speed: 1.0,
            current_volume: ws.volume,
            has_video: false,
            file_to_load: file_arg.clone(),
        };

        let widgets = view_output!();

        // Apply saved window state
        root.set_default_size(ws.width, ws.height);
        if ws.maximized {
            root.maximize();
        }

        // Save window state on close
        let root_close = root.clone();
        root.connect_close_request(move |_window| {
            let (width, height) = root_close.default_size();
            let state = WindowState {
                width,
                height,
                maximized: root_close.is_maximized(),
                volume: 100.0, // TODO: track actual volume
            };
            if let Err(e) = window_state::save(&state) {
                tracing::warn!("Failed to save window state: {}", e);
            }
            glib::Propagation::Proceed
        });

        // Load file from CLI arg or show file chooser after a short delay
        let sender_init = sender.clone();
        let root_clone = root.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
            if let Some(path) = file_arg {
                sender_init.input(AppMsg::OpenFile(path));
            } else {
                show_file_chooser(&root_clone, sender_init.input_sender().clone());
            }
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            AppMsg::TogglePause => {
                self.video_area.emit(VideoAreaMsg::TogglePause);
            }
            AppMsg::OpenFile(path) => {
                info!("Opening file: {}", path);
                self.current_speed = 1.0;
                self.video_area.emit(VideoAreaMsg::LoadFile(path));
            }
            AppMsg::SeekRelative(offset) => {
                self.video_area.emit(VideoAreaMsg::SeekRelative(offset));
            }
            AppMsg::VolumeStep(delta) => {
                let new_vol = (self.current_volume + delta).clamp(0.0, 150.0);
                self.current_volume = new_vol;
                self.video_area.emit(VideoAreaMsg::SetVolume(new_vol));
            }
            AppMsg::ToggleMute => {
                self.video_area.emit(VideoAreaMsg::ToggleMute);
            }
            AppMsg::SetSpeed(speed) => {
                self.current_speed = speed;
                self.video_area.emit(VideoAreaMsg::SetSpeed(speed));
            }
            AppMsg::SpeedUp => {
                let new_speed = backend::next_speed(self.current_speed);
                self.current_speed = new_speed;
                self.video_area.emit(VideoAreaMsg::SetSpeed(new_speed));
            }
            AppMsg::SpeedDown => {
                let new_speed = backend::prev_speed(self.current_speed);
                self.current_speed = new_speed;
                self.video_area.emit(VideoAreaMsg::SetSpeed(new_speed));
            }
            AppMsg::SpeedReset => {
                self.current_speed = 1.0;
                self.video_area.emit(VideoAreaMsg::SetSpeed(1.0));
            }
            AppMsg::ToggleFullscreen => {
                let new_fs = !root.is_fullscreen();
                root.set_fullscreened(new_fs);
                self.video_area
                    .emit(VideoAreaMsg::FullscreenChanged(new_fs));
            }
            AppMsg::ExitFullscreen => {
                if root.is_fullscreen() {
                    root.set_fullscreened(false);
                    self.video_area.emit(VideoAreaMsg::FullscreenChanged(false));
                }
            }
            AppMsg::FilesDropped(uri) => {
                let action = drop_target::classify_drop(&uri, self.has_video);
                match action {
                    drop_target::DropAction::PlayVideo(path) => {
                        sender.input(AppMsg::OpenFile(path));
                    }
                    drop_target::DropAction::LoadSubtitle(path) => {
                        self.video_area.emit(VideoAreaMsg::AddSubtitleFile(path));
                    }
                    drop_target::DropAction::Unsupported => {
                        // Show toast via the ToastOverlay
                    }
                }
            }
            AppMsg::VideoOutput(output) => match output {
                VideoAreaOutput::FileLoaded => {
                    info!("File loaded in app");
                    self.has_video = true;
                    self.screensaver.inhibit(root);
                }
                VideoAreaOutput::PositionChanged { .. } => {}
                VideoAreaOutput::StateChanged(state) => {
                    root.set_title(Some(backend::window_title_for_state(state)));
                    match state {
                        PlayState::Playing => self.screensaver.inhibit(root),
                        PlayState::Paused | PlayState::Stopped => {
                            self.screensaver.uninhibit(root);
                        }
                    }
                }
                VideoAreaOutput::EndOfFile(reason) => {
                    info!("Playback ended: {:?}", reason);
                    self.screensaver.uninhibit(root);
                }
                VideoAreaOutput::VolumeChanged { volume, .. } => {
                    self.current_volume = volume;
                }
                VideoAreaOutput::SpeedChanged(speed) => {
                    self.current_speed = speed;
                }
                VideoAreaOutput::ToggleFullscreen => {
                    let new_fs = !root.is_fullscreen();
                    root.set_fullscreened(new_fs);
                    self.video_area
                        .emit(VideoAreaMsg::FullscreenChanged(new_fs));
                }
            },
            AppMsg::ShowFileChooser => {
                show_file_chooser(root, sender.input_sender().clone());
            }
        }
    }
}

fn show_file_chooser(window: &adw::ApplicationWindow, sender: relm4::Sender<AppMsg>) {
    let dialog = gtk4::FileDialog::builder()
        .title("Open Video File")
        .modal(true)
        .build();

    let filter = gtk4::FileFilter::new();
    filter.set_name(Some("Video Files"));
    filter.add_mime_type("video/*");

    let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&filter);
    dialog.set_filters(Some(&filters));

    let window_clone = window.clone();
    dialog.open(
        Some(&window_clone),
        gtk4::gio::Cancellable::NONE,
        move |result| {
            if let Ok(file) = result
                && let Some(path) = file.path()
            {
                let _ = sender.send(AppMsg::OpenFile(path.to_string_lossy().to_string()));
            }
        },
    );
}
