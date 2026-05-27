use adw;
use adw::prelude::*;
use gtk;
use gtk::gio;
use relm4::Sender;

use super::AppMsg;
use crate::components::player::video_player::VideoPlayerMsg;

pub fn show_subtitle_chooser(window: &adw::ApplicationWindow, sender: Sender<AppMsg>) {
    let dialog = gtk::FileDialog::builder()
        .title("Open Subtitle File")
        .modal(true)
        .build();

    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Subtitle Files"));
    for ext in ["srt", "ass", "ssa", "vtt", "sub"] {
        filter.add_suffix(ext);
    }

    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    dialog.set_filters(Some(&filters));

    let window_clone = window.clone();
    dialog.open(
        Some(&window_clone),
        gio::Cancellable::NONE,
        move |result| {
            if let Ok(file) = result
                && let Some(path) = file.path()
            {
                let uri = format!("file://{}", path.to_string_lossy());
                let _ = sender.send(AppMsg::PlayerAction(
                    VideoPlayerMsg::LoadExternalSubtitle(uri),
                ));
            }
        },
    );
}

pub fn show_file_chooser(window: &adw::ApplicationWindow, sender: Sender<AppMsg>) {
    let dialog = gtk::FileDialog::builder()
        .title("Open Video File")
        .modal(true)
        .build();

    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Video Files"));
    filter.add_mime_type("video/*");

    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    dialog.set_filters(Some(&filters));

    let window_clone = window.clone();
    dialog.open(
        Some(&window_clone),
        gio::Cancellable::NONE,
        move |result| {
            if let Ok(file) = result
                && let Some(path) = file.path()
            {
                let _ =
                    sender.send(AppMsg::OpenFile(path.to_string_lossy().to_string()));
            }
        },
    );
}
