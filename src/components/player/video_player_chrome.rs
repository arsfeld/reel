//! OSD chrome helpers extracted from `video_player.rs` to keep that file under
//! the 2000-line hard cap (KTD9): time formatting, the volume/center icons, the
//! keyboard-shortcut filter, the center-indicator flash, and the audio/subtitle
//! track popover rebuild. These are free functions (not methods), so they only
//! depend on their parameters' types.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use gtk::glib;
use gtk::prelude::*;
use relm4::ComponentSender;
use relm4::gtk;

use super::video_player::{VideoPlayer, VideoPlayerMsg, VideoPlayerWidgets};
use crate::models::playback::DecisionStream;
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
/// rebuilding the popovers when nothing changed. `transcode` (AE6) folds the
/// Plex stream lists in so the menus rebuild when the decision changes them.
fn track_ui_signature(
    tracks: &[MediaTrack],
    subtitles_enabled: bool,
    transcode: Option<(&[DecisionStream], &[DecisionStream])>,
) -> String {
    if let Some((audio, subtitle)) = transcode {
        let fmt = |s: &DecisionStream| format!("{}:{}:{}", s.id, s.selected as u8, s.label);
        let mut parts: Vec<String> = vec!["tc".into()];
        parts.extend(audio.iter().map(fmt));
        parts.push("|sub|".into());
        parts.extend(subtitle.iter().map(fmt));
        return parts.join("|");
    }
    let mut parts: Vec<String> = tracks
        .iter()
        .map(|t| format!("{}:{}:{}", t.stream_id, t.selected as u8, t.label.as_str()))
        .collect();
    parts.push(format!("subs:{subtitles_enabled}"));
    parts.join("|")
}

/// Rebuild the audio/subtitle track radio popovers when the track set changes.
/// `transcode` (AE6): when the active decision is a transcode, the menus are
/// built from the Plex stream list and a change issues a re-decision; otherwise
/// the live GStreamer tracks drive the menus (and live selection).
pub(super) fn rebuild_track_popovers(
    widgets: &VideoPlayerWidgets,
    tracks: &[MediaTrack],
    media: Option<&PlaybackPipeline>,
    transcode: Option<(&[DecisionStream], &[DecisionStream])>,
    sender: &ComponentSender<VideoPlayer>,
    signature: &mut String,
) {
    let subtitles_enabled = media.is_some_and(PlaybackPipeline::subtitles_enabled);
    let sig = track_ui_signature(tracks, subtitles_enabled, transcode);
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

    if let Some((audio, subtitle)) = transcode {
        build_transcode_track_menus(widgets, audio, subtitle, sender);
    } else {
        build_gstreamer_track_menus(widgets, tracks, subtitles_enabled, sender);
    }
}

/// Build the audio/subtitle menus from the live GStreamer track set, wiring each
/// toggle to a live in-pipeline selection (direct-play path).
fn build_gstreamer_track_menus(
    widgets: &VideoPlayerWidgets,
    tracks: &[MediaTrack],
    subtitles_enabled: bool,
    sender: &ComponentSender<VideoPlayer>,
) {
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
    let sender_off = sender.clone();
    off_btn.connect_toggled(move |b| {
        if b.is_active() {
            sender_off.input(VideoPlayerMsg::SelectSubtitle(None));
        }
    });
    widgets.subtitle_tracks_box.append(&off_btn);

    for track in tracks.iter().filter(|t| t.kind == TrackKind::Subtitle) {
        let btn = gtk::CheckButton::builder()
            .label(&track.label)
            .active(subtitles_enabled && track.selected)
            .build();
        btn.set_group(Some(&off_btn));
        let id = track.stream_id.clone();
        let sender = sender.clone();
        btn.connect_toggled(move |b| {
            if b.is_active() {
                sender.input(VideoPlayerMsg::SelectSubtitle(Some(id.clone())));
            }
        });
        widgets.subtitle_tracks_box.append(&btn);
    }

    append_load_subtitle_button(widgets, sender);
}

/// Build the audio/subtitle menus from the Plex decision stream list (AE6).
/// Each toggle issues a fresh decision carrying the Plex stream id instead of a
/// live GStreamer selection, so a track change during a transcode reloads.
fn build_transcode_track_menus(
    widgets: &VideoPlayerWidgets,
    audio: &[DecisionStream],
    subtitle: &[DecisionStream],
    sender: &ComponentSender<VideoPlayer>,
) {
    if audio.is_empty() {
        let label = gtk::Label::new(Some("No audio tracks"));
        label.add_css_class("dim-label");
        widgets.audio_tracks_box.append(&label);
    } else {
        let mut first_btn: Option<gtk::CheckButton> = None;
        for stream in audio {
            let btn = gtk::CheckButton::builder()
                .label(&stream.label)
                .active(stream.selected)
                .build();
            if let Some(ref group) = first_btn {
                btn.set_group(Some(group));
            } else {
                first_btn = Some(btn.clone());
            }
            let id = stream.id;
            let sender = sender.clone();
            btn.connect_toggled(move |b| {
                if b.is_active() {
                    sender.input(VideoPlayerMsg::SelectAudioTrack(id));
                }
            });
            widgets.audio_tracks_box.append(&btn);
        }
    }

    let any_sub_selected = subtitle.iter().any(|s| s.selected);
    let off_btn = gtk::CheckButton::builder()
        .label("Off")
        .active(!any_sub_selected)
        .build();
    let sender_off = sender.clone();
    off_btn.connect_toggled(move |b| {
        if b.is_active() {
            sender_off.input(VideoPlayerMsg::SelectSubtitleTrack(None));
        }
    });
    widgets.subtitle_tracks_box.append(&off_btn);

    for stream in subtitle {
        let btn = gtk::CheckButton::builder()
            .label(&stream.label)
            .active(stream.selected)
            .build();
        btn.set_group(Some(&off_btn));
        let id = stream.id;
        let sender = sender.clone();
        btn.connect_toggled(move |b| {
            if b.is_active() {
                sender.input(VideoPlayerMsg::SelectSubtitleTrack(Some(id)));
            }
        });
        widgets.subtitle_tracks_box.append(&btn);
    }

    append_load_subtitle_button(widgets, sender);
}

/// Append the "Load subtitle file…" button to the subtitle popover.
fn append_load_subtitle_button(
    widgets: &VideoPlayerWidgets,
    sender: &ComponentSender<VideoPlayer>,
) {
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

/// Hook the seek + volume scales: anything that isn't our own programmatic
/// write becomes a UserSeek/SetVolume; the seek slider also stamps an
/// `Instant` so `refresh_widgets` doesn't yank the thumb back during a drag.
pub(super) fn wire_slider_handlers(
    widgets: &VideoPlayerWidgets,
    sender: &ComponentSender<VideoPlayer>,
    suppress_scale: &Rc<Cell<bool>>,
    suppress_volume: &Rc<Cell<bool>>,
    last_user_seek: &Rc<Cell<Option<Instant>>>,
) {
    {
        let sender = sender.clone();
        let suppress = suppress_scale.clone();
        let stamp = last_user_seek.clone();
        widgets.seek_scale.connect_value_changed(move |s| {
            if suppress.get() {
                return;
            }
            stamp.set(Some(Instant::now()));
            sender.input(VideoPlayerMsg::UserSeek(s.value() as i64));
        });
    }
    let sender = sender.clone();
    let suppress = suppress_volume.clone();
    widgets.volume_scale.connect_value_changed(move |s| {
        if suppress.get() {
            return;
        }
        sender.input(VideoPlayerMsg::SetVolume(s.value()));
    });
}

/// Pointer motion → wake controls. Single click anywhere on the surface
/// toggles play/pause; double click toggles fullscreen. Both paths also
/// keep the OSD visible (PointerActive).
pub(super) fn wire_pointer_handlers(
    widgets: &VideoPlayerWidgets,
    sender: &ComponentSender<VideoPlayer>,
) {
    let motion = gtk::EventControllerMotion::new();
    let sender_m = sender.clone();
    motion.connect_motion(move |_, _, _| {
        sender_m.input(VideoPlayerMsg::PointerActive);
    });
    widgets.stack_overlay.add_controller(motion);

    let click = gtk::GestureClick::new();
    click.set_button(gtk::gdk::BUTTON_PRIMARY);
    let sender_c = sender.clone();
    let root_for_focus = widgets.root_box.clone();
    click.connect_pressed(move |_, n, _, _| {
        root_for_focus.grab_focus();
        if n == 1 {
            sender_c.input(VideoPlayerMsg::TogglePlay);
        } else if n == 2 {
            sender_c.input(VideoPlayerMsg::ToggleFullscreen);
        }
        sender_c.input(VideoPlayerMsg::PointerActive);
    });
    // Attach to the picture (not the controls bar) so clicks on the OSD
    // don't steal play/pause toggles.
    widgets.picture.add_controller(click);
}

/// Keyboard shortcuts: capture phase so the slider (and any other focused
/// descendant) doesn't eat arrow keys before we see them. Return `Stop`
/// for keys we recognise so we don't double-handle (e.g. focused seek
/// slider + our own seek-by-5s).
pub(super) fn wire_keyboard_handlers(
    widgets: &VideoPlayerWidgets,
    sender: &ComponentSender<VideoPlayer>,
) {
    let key = gtk::EventControllerKey::new();
    key.set_propagation_phase(gtk::PropagationPhase::Capture);
    let sender_k = sender.clone();
    key.connect_key_pressed(move |_, keyval, _, mods| {
        if is_player_shortcut(keyval) {
            sender_k.input(VideoPlayerMsg::KeyPressed(keyval, mods));
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    widgets.root_box.add_controller(key);
}

/// Wire the OSD popover menus (audio / subtitle / quality) so the chrome stays
/// revealed while any is open.
pub(super) fn wire_popover_handlers(
    widgets: &VideoPlayerWidgets,
    sender: &ComponentSender<VideoPlayer>,
) {
    for menu in [
        &widgets.audio_menu,
        &widgets.subtitle_menu,
        &widgets.quality_menu,
    ] {
        let Some(popover) = menu.popover() else {
            continue;
        };
        let sender_show = sender.clone();
        popover.connect_show(move |_| {
            sender_show.input(VideoPlayerMsg::PopoverVisibilityChanged(true));
        });
        let sender_hide = sender.clone();
        popover.connect_closed(move |_| {
            sender_hide.input(VideoPlayerMsg::PopoverVisibilityChanged(false));
        });
    }
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
