//! OSD chrome helpers extracted from `video_player.rs` to keep that file under
//! the 2000-line hard cap (KTD9): time formatting, the volume/center icons, the
//! keyboard-shortcut filter, the center-indicator flash, and the audio/subtitle
//! track popover rebuild. These are free functions (not methods), so they only
//! depend on their parameters' types.

use gtk::glib;
use gtk::prelude::*;
use relm4::ComponentSender;
use relm4::gtk;

use super::video_player::{VideoPlayer, VideoPlayerMsg, VideoPlayerWidgets};
use crate::player::gst_pipeline::PlaybackPipeline;
use crate::player::tracks::{MediaTrack, TrackKind};

/// Whether a key is one the player consumes (so it doesn't bubble to the shell).
pub(super) fn is_player_shortcut(key: gtk::gdk::Key) -> bool {
    use gtk::gdk::Key;
    matches!(
        key,
        Key::space
            | Key::k
            | Key::K
            | Key::Left
            | Key::Right
            | Key::Up
            | Key::Down
            | Key::j
            | Key::J
            | Key::l
            | Key::L
            | Key::Home
            | Key::End
            | Key::m
            | Key::M
            | Key::_9
            | Key::_0
            | Key::f
            | Key::F
            | Key::s
            | Key::S
            | Key::Escape
    )
}

/// Briefly flash the center play/pause indicator.
pub(super) fn flash_center(image: &gtk::Image, playing: bool) {
    image.set_icon_name(Some(if playing {
        "media-playback-start-symbolic"
    } else {
        "media-playback-pause-symbolic"
    }));
    image.set_visible(true);
    image.remove_css_class("video-center-indicator-flash");
    image.add_css_class("video-center-indicator-flash");
    let weak = image.downgrade();
    glib::timeout_add_local_once(std::time::Duration::from_millis(550), move || {
        if let Some(img) = weak.upgrade() {
            img.set_visible(false);
            img.remove_css_class("video-center-indicator-flash");
        }
    });
}

/// Format microseconds as `h:mm:ss` or `m:ss`.
pub(super) fn format_us(us: i64) -> String {
    let total = (us.max(0) / 1_000_000) as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Volume icon name for the current mute/volume state.
pub(super) fn volume_icon(muted: bool, volume: f64) -> &'static str {
    if muted || volume <= 0.001 {
        "audio-volume-muted-symbolic"
    } else if volume < 0.34 {
        "audio-volume-low-symbolic"
    } else if volume < 0.67 {
        "audio-volume-medium-symbolic"
    } else {
        "audio-volume-high-symbolic"
    }
}

/// A stable signature of the track set + subtitle-enabled flag, used to skip
/// rebuilding the popovers when nothing changed.
fn track_ui_signature(tracks: &[MediaTrack], subtitles_enabled: bool) -> String {
    let mut parts: Vec<String> = tracks
        .iter()
        .map(|t| format!("{}:{}:{}", t.stream_id, t.selected as u8, t.label.as_str()))
        .collect();
    parts.push(format!("subs:{subtitles_enabled}"));
    parts.join("|")
}

/// Rebuild the audio/subtitle track radio popovers when the track set changes.
pub(super) fn rebuild_track_popovers(
    widgets: &VideoPlayerWidgets,
    tracks: &[MediaTrack],
    media: Option<&PlaybackPipeline>,
    sender: &ComponentSender<VideoPlayer>,
    signature: &mut String,
) {
    let subtitles_enabled = media.is_some_and(PlaybackPipeline::subtitles_enabled);
    let sig = track_ui_signature(tracks, subtitles_enabled);
    if sig == *signature {
        return;
    }
    *signature = sig;

    while let Some(child) = widgets.audio_tracks_box.first_child() {
        widgets.audio_tracks_box.remove(&child);
    }
    while let Some(child) = widgets.subtitle_tracks_box.first_child() {
        widgets.subtitle_tracks_box.remove(&child);
    }

    let audio_tracks: Vec<_> = tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Audio)
        .collect();
    if audio_tracks.is_empty() {
        let label = gtk::Label::new(Some("No audio tracks"));
        label.add_css_class("dim-label");
        widgets.audio_tracks_box.append(&label);
    } else {
        let mut first_btn: Option<gtk::CheckButton> = None;
        for track in audio_tracks {
            let btn = gtk::CheckButton::builder()
                .label(&track.label)
                .active(track.selected)
                .build();
            if let Some(ref group) = first_btn {
                btn.set_group(Some(group));
            } else {
                first_btn = Some(btn.clone());
            }
            let id = track.stream_id.clone();
            let sender = sender.clone();
            btn.connect_toggled(move |b| {
                if b.is_active() {
                    sender.input(VideoPlayerMsg::SelectAudio(id.clone()));
                }
            });
            widgets.audio_tracks_box.append(&btn);
        }
    }

    let off_btn = gtk::CheckButton::builder()
        .label("Off")
        .active(!subtitles_enabled)
        .build();
    let first_sub = Some(off_btn.clone());
    let sender_off = sender.clone();
    off_btn.connect_toggled(move |b| {
        if b.is_active() {
            sender_off.input(VideoPlayerMsg::SelectSubtitle(None));
        }
    });
    widgets.subtitle_tracks_box.append(&off_btn);

    let subtitle_tracks: Vec<_> = tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Subtitle)
        .collect();
    for track in subtitle_tracks {
        let btn = gtk::CheckButton::builder()
            .label(&track.label)
            .active(subtitles_enabled && track.selected)
            .build();
        if let Some(ref group) = first_sub {
            btn.set_group(Some(group));
        }
        let id = track.stream_id.clone();
        let sender = sender.clone();
        btn.connect_toggled(move |b| {
            if b.is_active() {
                sender.input(VideoPlayerMsg::SelectSubtitle(Some(id.clone())));
            }
        });
        widgets.subtitle_tracks_box.append(&btn);
    }

    let load_btn = gtk::Button::builder()
        .label("Load subtitle file…")
        .css_classes(["flat"])
        .build();
    let sender_load = sender.clone();
    load_btn.connect_clicked(move |_| {
        sender_load.input(VideoPlayerMsg::LoadSubtitleFile);
    });
    widgets.subtitle_tracks_box.append(&load_btn);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_us_hours_and_minutes() {
        assert_eq!(format_us(0), "0:00");
        assert_eq!(format_us(65_000_000), "1:05");
        assert_eq!(format_us(3_661_000_000), "1:01:01");
        assert_eq!(format_us(-5), "0:00");
    }

    #[test]
    fn volume_icon_buckets() {
        assert_eq!(volume_icon(true, 0.9), "audio-volume-muted-symbolic");
        assert_eq!(volume_icon(false, 0.0), "audio-volume-muted-symbolic");
        assert_eq!(volume_icon(false, 0.2), "audio-volume-low-symbolic");
        assert_eq!(volume_icon(false, 0.5), "audio-volume-medium-symbolic");
        assert_eq!(volume_icon(false, 0.9), "audio-volume-high-symbolic");
    }
}
