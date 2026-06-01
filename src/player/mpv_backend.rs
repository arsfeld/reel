#![allow(dead_code)]

use crate::player::backend::PlayerEvent;
use crate::player::tracks::{MediaTrack, TrackKind};
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

#[link(name = "epoxy")]
unsafe extern "C" {
    static epoxy_eglGetProcAddress:
        Option<unsafe extern "C" fn(*const std::os::raw::c_char) -> *mut std::ffi::c_void>;
    static epoxy_glXGetProcAddressARB:
        Option<unsafe extern "C" fn(*const u8) -> *mut std::ffi::c_void>;
}

fn get_proc_address(_ctx: &(), name: &str) -> *mut std::ffi::c_void {
    let cname = std::ffi::CString::new(name).unwrap();
    unsafe {
        if let Some(egl_get_proc_address) = epoxy_eglGetProcAddress {
            let ptr = egl_get_proc_address(cname.as_ptr());
            if !ptr.is_null() {
                return ptr;
            }
        }
        epoxy_glXGetProcAddressARB.map_or(std::ptr::null_mut(), |glx_get_proc_address| {
            glx_get_proc_address(cname.as_ptr().cast::<u8>())
        })
    }
}

pub(crate) struct MpvRenderBridge {
    render_context: libmpv2::render::RenderContext<'static>,
    _redraw_source: glib::SourceId,
}

pub(crate) struct MpvInner {
    mpv: libmpv2::Mpv,
    wants_play: Cell<bool>,
    playing: Cell<bool>,
    prepared: Cell<bool>,
    seeking: Cell<bool>,
    error: RefCell<Option<String>>,
    tracks: RefCell<Vec<MediaTrack>>,
    subtitles_enabled: Cell<bool>,
}

pub(crate) struct MpvBackend {
    inner: Rc<MpvInner>,
    _wakeup_channel: glib::SourceId,
}

impl MpvBackend {
    pub(crate) fn new(
        url: &str,
        autoplay: bool,
        preferred_subtitle_lang: Option<String>,
        local_path: Option<&str>,
        hdr_mode: crate::settings::ResolvedHdrMode,
        hwdec_mode: &str,
        sender: Rc<dyn Fn(PlayerEvent)>,
    ) -> Option<Self> {
        let mpv = libmpv2::Mpv::with_initializer(|init| {
            init.set_option("vo", "libmpv")?;
            init.set_option("gpu-context", "auto")?;
            init.set_option(
                "hwdec",
                if hwdec_mode == "auto-safe" {
                    "auto"
                } else {
                    hwdec_mode
                },
            )?;

            match hdr_mode {
                crate::settings::ResolvedHdrMode::MpvAuto => {}
                crate::settings::ResolvedHdrMode::MpvForceSdr
                | crate::settings::ResolvedHdrMode::Transcode => {
                    init.set_option("target-trc", "bt.1886")?;
                    init.set_option("target-peak", "100.0")?;
                }
                crate::settings::ResolvedHdrMode::DirectLimited => {}
            }

            init.set_option("terminal", "no")?;
            init.set_option("osc", "no")?;
            init.set_option("input-default-bindings", "no")?;
            init.set_option("input-vo-keyboard", "no")?;
            Ok(())
        })
        .ok()?;

        // Observe events/properties
        let _ = mpv.observe_property("track-list", libmpv2::Format::String, 100);
        let _ = mpv.observe_property("pause", libmpv2::Format::Flag, 101);
        let _ = mpv.observe_property("cache-buffering-state", libmpv2::Format::Int64, 102);

        // Set up local subtitle if provided
        if let Some(path) = local_path {
            if let Some(sub) =
                crate::player::subtitles::find_matching_subtitles(std::path::Path::new(path))
                    .first()
            {
                let sub_uri = format!("file://{}", sub.to_string_lossy());
                let _ = mpv.set_property("sub-files", sub_uri);
            }
        }

        let inner = Rc::new(MpvInner {
            mpv,
            wants_play: Cell::new(autoplay),
            playing: Cell::new(false),
            prepared: Cell::new(false),
            seeking: Cell::new(false),
            error: RefCell::new(None),
            tracks: RefCell::new(Vec::new()),
            subtitles_enabled: Cell::new(true),
        });

        let inner_weak = Rc::downgrade(&inner);
        let sender_clone = sender.clone();
        let preferred_subtitle_lang_clone = preferred_subtitle_lang.clone();

        let wakeup_channel =
            glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
                let Some(inner) = inner_weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };

                while let Some(event_res) = inner.mpv.wait_event(0.0) {
                    match event_res {
                        Ok(event) => match event {
                            libmpv2::events::Event::FileLoaded => {
                                inner.prepared.set(true);
                                inner.playing.set(inner.wants_play.get());
                                let t = query_tracks(&inner.mpv);
                                *inner.tracks.borrow_mut() = t.clone();
                                sender_clone(PlayerEvent::TracksChanged(t));

                                // Apply preferred subtitle lang if available
                                if let Some(ref lang) = preferred_subtitle_lang_clone {
                                    if let Some(track) = inner.tracks.borrow().iter().find(|tr| {
                                        tr.kind == TrackKind::Subtitle
                                            && tr.language.as_ref().is_some_and(|l| {
                                                l.to_lowercase() == lang.to_lowercase()
                                            })
                                    }) {
                                        if let Ok(id) = track.stream_id.parse::<i64>() {
                                            let _ = inner.mpv.set_property("sid", id);
                                        }
                                    }
                                }
                            }
                            libmpv2::events::Event::PropertyChange { name, change, .. } => {
                                match name {
                                    "track-list" => {
                                        let t = query_tracks(&inner.mpv);
                                        *inner.tracks.borrow_mut() = t.clone();
                                        sender_clone(PlayerEvent::TracksChanged(t));
                                    }
                                    "pause" => {
                                        if let libmpv2::events::PropertyData::Flag(paused) = change
                                        {
                                            inner.playing.set(!paused);
                                        }
                                    }
                                    "cache-buffering-state" => {
                                        if let libmpv2::events::PropertyData::Int64(val) = change {
                                            sender_clone(PlayerEvent::Buffering(val as i32));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            libmpv2::events::Event::EndFile(reason) => {
                                inner.prepared.set(false);
                                inner.playing.set(false);
                                if reason == libmpv2::mpv_end_file_reason::Error {
                                    *inner.error.borrow_mut() =
                                        Some("Playback failed in mpv backend".to_string());
                                }
                            }
                            libmpv2::events::Event::Seek => {
                                inner.seeking.set(true);
                            }
                            libmpv2::events::Event::PlaybackRestart => {
                                inner.seeking.set(false);
                            }
                            _ => {}
                        },
                        Err(err) => {
                            *inner.error.borrow_mut() = Some(sanitize_error(&format!("{err:?}")));
                        }
                    }
                }
                glib::ControlFlow::Continue
            });

        // Load URL
        let mode = "replace";
        if inner.mpv.command("loadfile", &[url, mode]).is_err() {
            *inner.error.borrow_mut() = Some("mpv failed to load the playback URL".to_string());
            return None;
        }

        Some(Self {
            inner,
            _wakeup_channel: wakeup_channel,
        })
    }

    pub(crate) fn mpv_handle(&self) -> &libmpv2::Mpv {
        &self.inner.mpv
    }

    pub(crate) fn attach_render_context(
        &self,
        area: &gtk::GLArea,
    ) -> Result<MpvRenderBridge, String> {
        area.make_current();
        if let Some(err) = area.error() {
            return Err(format!("mpv GL surface is unavailable: {err}"));
        }

        let mut render_context = self
            .inner
            .mpv
            .create_render_context(vec![
                libmpv2::render::RenderParam::ApiType(libmpv2::render::RenderParamApiType::OpenGl),
                libmpv2::render::RenderParam::InitParams(libmpv2::render::OpenGLInitParams {
                    get_proc_address,
                    ctx: (),
                }),
            ])
            .map_err(|err| format!("mpv render initialization failed: {err}"))?;

        let (redraw_tx, redraw_rx) = std::sync::mpsc::channel::<()>();
        render_context.set_update_callback(move || {
            let _ = redraw_tx.send(());
        });

        let area_weak = area.downgrade();
        let redraw_source = glib::timeout_add_local(Duration::from_millis(16), move || {
            let mut should_redraw = false;
            while redraw_rx.try_recv().is_ok() {
                should_redraw = true;
            }

            let Some(area) = area_weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            if should_redraw {
                area.queue_render();
            }
            glib::ControlFlow::Continue
        });

        // libmpv2 models RenderContext as borrowing Mpv, but the C render
        // context owns a raw mpv handle. VideoPlayer drops this bridge before
        // dropping the backend, so the raw handle remains valid.
        let render_context = unsafe {
            std::mem::transmute::<
                libmpv2::render::RenderContext<'_>,
                libmpv2::render::RenderContext<'static>,
            >(render_context)
        };

        Ok(MpvRenderBridge {
            render_context,
            _redraw_source: redraw_source,
        })
    }

    pub(crate) fn play(&self) {
        self.inner.wants_play.set(true);
        let _ = self.inner.mpv.set_property("pause", false);
    }

    pub(crate) fn pause(&self) {
        self.inner.wants_play.set(false);
        let _ = self.inner.mpv.set_property("pause", true);
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.inner.playing.get()
    }

    pub(crate) fn wants_play(&self) -> bool {
        self.inner.wants_play.get()
    }

    pub(crate) fn is_prepared(&self) -> bool {
        self.inner.prepared.get()
    }

    pub(crate) fn is_seeking(&self) -> bool {
        self.inner.seeking.get()
    }

    pub(crate) fn error_message(&self) -> Option<String> {
        self.inner.error.borrow().clone()
    }

    pub(crate) fn tracks(&self) -> Vec<MediaTrack> {
        self.inner.tracks.borrow().clone()
    }

    pub(crate) fn subtitles_enabled(&self) -> bool {
        self.inner.subtitles_enabled.get()
    }

    pub(crate) fn select_audio(&self, id: &str) {
        if let Ok(track_id) = id.parse::<i64>() {
            let _ = self.inner.mpv.set_property("aid", track_id);
        }
    }

    pub(crate) fn select_subtitle(&self, id: Option<&str>) {
        if let Some(track_id_str) = id {
            if let Ok(track_id) = track_id_str.parse::<i64>() {
                self.inner.subtitles_enabled.set(true);
                let _ = self.inner.mpv.set_property("sid", track_id);
            }
        } else {
            self.inner.subtitles_enabled.set(false);
            let _ = self.inner.mpv.set_property("sid", "no");
        }
    }

    pub(crate) fn load_external_subtitle(&self, uri: &str) {
        let _ = self.inner.mpv.command("sub-add", &[uri, "select"]);
        self.inner.subtitles_enabled.set(true);
    }

    pub(crate) fn duration_us(&self) -> i64 {
        let duration: f64 = self.inner.mpv.get_property("duration").unwrap_or(0.0);
        (duration * 1_000_000.0) as i64
    }

    pub(crate) fn position_us(&self) -> i64 {
        let pos: f64 = self.inner.mpv.get_property("time-pos").unwrap_or(0.0);
        (pos * 1_000_000.0) as i64
    }

    pub(crate) fn buffered_fraction(&self) -> f64 {
        let cache_percent: i64 = self.inner.mpv.get_property("cache-percent").unwrap_or(0);
        (cache_percent as f64 / 100.0).clamp(0.0, 1.0)
    }

    pub(crate) fn set_volume(&self, v: f64) {
        let _ = self.inner.mpv.set_property("volume", v * 100.0);
    }

    pub(crate) fn volume(&self) -> f64 {
        let vol: f64 = self.inner.mpv.get_property("volume").unwrap_or(100.0);
        vol / 100.0
    }

    pub(crate) fn set_muted(&self, m: bool) {
        let _ = self.inner.mpv.set_property("mute", m);
    }

    pub(crate) fn is_muted(&self) -> bool {
        self.inner.mpv.get_property("mute").unwrap_or(false)
    }

    pub(crate) fn set_speed(&self, speed: f64) {
        let _ = self.inner.mpv.set_property("speed", speed);
    }

    pub(crate) fn speed(&self) -> f64 {
        self.inner.mpv.get_property("speed").unwrap_or(1.0)
    }

    pub(crate) fn seek(&self, target_us: i64) {
        let target_secs = target_us as f64 / 1_000_000.0;
        let _ = self
            .inner
            .mpv
            .command("seek", &[&format!("{:.6}", target_secs), "absolute"]);
    }
}

impl MpvRenderBridge {
    pub(crate) fn render(&self, area: &gtk::GLArea) -> Result<(), String> {
        let scale = area.scale_factor();
        let width = area.width() * scale;
        let height = area.height() * scale;
        if width <= 0 || height <= 0 {
            return Ok(());
        }

        self.render_context
            .update()
            .map_err(|err| format!("mpv render update failed: {err}"))?;
        self.render_context
            .render::<()>(0, width, height, true)
            .map_err(|err| format!("mpv frame render failed: {err}"))?;
        self.render_context.report_swap();
        Ok(())
    }
}

fn query_tracks(mpv: &libmpv2::Mpv) -> Vec<MediaTrack> {
    let mut tracks = Vec::new();
    let count: i64 = mpv.get_property("track-list/count").unwrap_or(0);
    for i in 0..count {
        let id: i64 = mpv
            .get_property(&format!("track-list/{}/id", i))
            .unwrap_or(0);
        let type_str: String = mpv
            .get_property(&format!("track-list/{}/type", i))
            .unwrap_or_default();
        let lang: String = mpv
            .get_property(&format!("track-list/{}/lang", i))
            .unwrap_or_default();
        let title: String = mpv
            .get_property(&format!("track-list/{}/title", i))
            .unwrap_or_default();
        let selected: bool = mpv
            .get_property(&format!("track-list/{}/selected", i))
            .unwrap_or(false);

        if let Some(track) = media_track_from_mpv_fields(id, &type_str, &lang, &title, selected) {
            tracks.push(track);
        }
    }
    tracks
}

fn media_track_from_mpv_fields(
    id: i64,
    type_str: &str,
    lang: &str,
    title: &str,
    selected: bool,
) -> Option<MediaTrack> {
    let kind = match type_str {
        "audio" => TrackKind::Audio,
        "sub" => TrackKind::Subtitle,
        _ => return None,
    };

    let label = if !title.is_empty() {
        title.to_string()
    } else if !lang.is_empty() {
        lang.to_string()
    } else {
        format!("Track {id}")
    };

    Some(MediaTrack {
        stream_id: id.to_string(),
        kind,
        label,
        language: if lang.is_empty() {
            None
        } else {
            Some(lang.to_string())
        },
        codec: None,
        selected,
        forced: false,
        external: false,
    })
}

fn sanitize_error(message: &str) -> String {
    crate::models::playback::redact_plex_token(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mpv_track_fields_map_audio_with_stable_label() {
        let track = media_track_from_mpv_fields(2, "audio", "eng", "English DTS", true).unwrap();
        assert_eq!(track.kind, TrackKind::Audio);
        assert_eq!(track.stream_id, "2");
        assert_eq!(track.label, "English DTS");
        assert_eq!(track.language.as_deref(), Some("eng"));
        assert!(track.selected);
    }

    #[test]
    fn mpv_track_fields_fall_back_to_language_then_track_id() {
        let subtitle = media_track_from_mpv_fields(3, "sub", "spa", "", false).unwrap();
        assert_eq!(subtitle.kind, TrackKind::Subtitle);
        assert_eq!(subtitle.label, "spa");

        let audio = media_track_from_mpv_fields(4, "audio", "", "", false).unwrap();
        assert_eq!(audio.label, "Track 4");
    }

    #[test]
    fn mpv_track_fields_ignore_unknown_types() {
        assert!(media_track_from_mpv_fields(1, "video", "", "", false).is_none());
    }

    #[test]
    fn sanitizes_token_bearing_errors() {
        let error = sanitize_error("failed https://host/video?X-Plex-Token=secret&x=1");
        assert!(error.contains("X-Plex-Token=REDACTED"));
        assert!(!error.contains("secret"));
    }
}
