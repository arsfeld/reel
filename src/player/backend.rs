use crate::player::gst_pipeline::PlaybackPipeline;
use crate::player::tracks::MediaTrack;

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    /// Track list changed (includes initial collection load and subsequent selections).
    TracksChanged(Vec<MediaTrack>),
    /// Buffering progress percent (0-100).
    Buffering(i32),
    /// The stream negotiation/render failure (gated direct-play transcode fallback).
    RenderFailed,
}

pub enum PlaybackBackend {
    GStreamer(PlaybackPipeline),
}

impl PlaybackBackend {
    pub(crate) fn play(&self) {
        match self {
            Self::GStreamer(p) => p.play(),
        }
    }

    pub(crate) fn pause(&self) {
        match self {
            Self::GStreamer(p) => p.pause(),
        }
    }

    pub(crate) fn is_playing(&self) -> bool {
        match self {
            Self::GStreamer(p) => p.is_playing(),
        }
    }

    pub(crate) fn wants_play(&self) -> bool {
        match self {
            Self::GStreamer(p) => p.wants_play(),
        }
    }

    pub(crate) fn is_prepared(&self) -> bool {
        match self {
            Self::GStreamer(p) => p.is_prepared(),
        }
    }

    pub(crate) fn is_seeking(&self) -> bool {
        match self {
            Self::GStreamer(p) => p.is_seeking(),
        }
    }

    pub(crate) fn error_message(&self) -> Option<String> {
        match self {
            Self::GStreamer(p) => p.error_message(),
        }
    }

    pub(crate) fn tracks(&self) -> Vec<MediaTrack> {
        match self {
            Self::GStreamer(p) => p.tracks(),
        }
    }

    pub(crate) fn subtitles_enabled(&self) -> bool {
        match self {
            Self::GStreamer(p) => p.subtitles_enabled(),
        }
    }

    pub(crate) fn select_audio(&self, id: &str) {
        match self {
            Self::GStreamer(p) => p.select_audio(id),
        }
    }

    pub(crate) fn select_subtitle(&self, id: Option<&str>) {
        match self {
            Self::GStreamer(p) => p.select_subtitle(id),
        }
    }

    pub(crate) fn load_external_subtitle(&self, uri: &str) {
        match self {
            Self::GStreamer(p) => p.load_external_subtitle(uri),
        }
    }

    pub(crate) fn duration_us(&self) -> i64 {
        match self {
            Self::GStreamer(p) => p.duration_us(),
        }
    }

    pub(crate) fn position_us(&self) -> i64 {
        match self {
            Self::GStreamer(p) => p.position_us(),
        }
    }

    pub(crate) fn buffered_fraction(&self) -> f64 {
        match self {
            Self::GStreamer(p) => p.buffered_fraction(),
        }
    }

    pub(crate) fn set_volume(&self, v: f64) {
        match self {
            Self::GStreamer(p) => p.set_volume(v),
        }
    }

    pub(crate) fn volume(&self) -> f64 {
        match self {
            Self::GStreamer(p) => p.volume(),
        }
    }

    pub(crate) fn set_muted(&self, m: bool) {
        match self {
            Self::GStreamer(p) => p.set_muted(m),
        }
    }

    pub(crate) fn is_muted(&self) -> bool {
        match self {
            Self::GStreamer(p) => p.is_muted(),
        }
    }

    pub(crate) fn set_speed(&self, speed: f64) {
        match self {
            Self::GStreamer(p) => p.set_speed(speed),
        }
    }

    pub(crate) fn speed(&self) -> f64 {
        match self {
            Self::GStreamer(p) => p.speed(),
        }
    }

    pub(crate) fn seek(&self, target_us: i64) {
        match self {
            Self::GStreamer(p) => p.seek(target_us),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn gstreamer_paintable(&self) -> Option<gtk::gdk::Paintable> {
        match self {
            Self::GStreamer(p) => Some(p.paintable().clone()),
        }
    }
}
