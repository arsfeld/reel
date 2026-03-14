use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use relm4::prelude::*;
use tracing::info;

use crate::components::player::shortcuts::{self, PlayerAction};
use crate::components::player::video_area::{VideoArea, VideoAreaMsg, VideoAreaOutput};
use crate::player::backend;

pub struct App {
    video_area: Controller<VideoArea>,
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
    SetVolume(f64),
    ToggleMute,
    SetSpeed(f64),
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

            gtk4::Box {
                set_orientation: gtk4::Orientation::Vertical,

                model.video_area.widget() -> &gtk4::Overlay {
                    set_vexpand: true,
                },
            },

            add_controller = gtk4::EventControllerKey {
                connect_key_pressed[sender] => move |_controller, key, _code, mods| {
                    // TODO: detect text input focus and pass is_text_input_focused=true
                    if let Some(action) = shortcuts::map_key_to_action(key, mods, false) {
                        match action {
                            PlayerAction::TogglePause => sender.input(AppMsg::TogglePause),
                            PlayerAction::SeekForward(s) => sender.input(AppMsg::SeekRelative(s)),
                            PlayerAction::SeekBackward(s) => sender.input(AppMsg::SeekRelative(-s)),
                            PlayerAction::VolumeUp(v) => sender.input(AppMsg::SetVolume(v)),
                            PlayerAction::VolumeDown(v) => sender.input(AppMsg::SetVolume(-v)),
                            PlayerAction::ToggleMute => sender.input(AppMsg::ToggleMute),
                            PlayerAction::ToggleFullscreen => sender.input(AppMsg::ToggleFullscreen),
                            PlayerAction::ExitFullscreen => sender.input(AppMsg::ExitFullscreen),
                            PlayerAction::SpeedUp => {
                                // Speed stepping handled via backend::next_speed
                                sender.input(AppMsg::SetSpeed(1.0)); // placeholder, will be refined
                            }
                            PlayerAction::SpeedDown => {
                                sender.input(AppMsg::SetSpeed(1.0)); // placeholder
                            }
                            PlayerAction::SpeedReset => sender.input(AppMsg::SetSpeed(1.0)),
                        }
                        glib::Propagation::Stop
                    } else {
                        glib::Propagation::Proceed
                    }
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

        let model = Self {
            video_area,
            file_to_load: file_arg.clone(),
        };

        let widgets = view_output!();

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
                self.video_area.emit(VideoAreaMsg::LoadFile(path));
            }
            AppMsg::SeekRelative(offset) => {
                self.video_area.emit(VideoAreaMsg::SeekRelative(offset));
            }
            AppMsg::SetVolume(delta) => {
                // Volume delta from keyboard: +5 or -5
                // TODO: track current volume and clamp
                self.video_area.emit(VideoAreaMsg::SetVolume(delta));
            }
            AppMsg::ToggleMute => {
                self.video_area.emit(VideoAreaMsg::ToggleMute);
            }
            AppMsg::SetSpeed(speed) => {
                self.video_area.emit(VideoAreaMsg::SetSpeed(speed));
            }
            AppMsg::ToggleFullscreen => {
                root.set_fullscreened(!root.is_fullscreen());
            }
            AppMsg::ExitFullscreen => {
                if root.is_fullscreen() {
                    root.set_fullscreened(false);
                }
            }
            AppMsg::VideoOutput(output) => match output {
                VideoAreaOutput::FileLoaded => {
                    info!("File loaded in app");
                }
                VideoAreaOutput::PositionChanged { .. } => {}
                VideoAreaOutput::StateChanged(state) => {
                    root.set_title(Some(backend::window_title_for_state(state)));
                }
                VideoAreaOutput::EndOfFile(reason) => {
                    info!("Playback ended: {:?}", reason);
                }
                VideoAreaOutput::VolumeChanged { .. } | VideoAreaOutput::SpeedChanged(_) => {}
                VideoAreaOutput::ToggleFullscreen => {
                    root.set_fullscreened(!root.is_fullscreen());
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
