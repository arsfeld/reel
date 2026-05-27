//! Pure track parsing and display helpers for GStreamer stream collections.

use gst::{Stream, StreamCollection, StreamFlags, StreamType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Audio,
    Subtitle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaTrack {
    pub stream_id: String,
    pub kind: TrackKind,
    pub label: String,
    pub language: Option<String>,
    pub codec: Option<String>,
    pub selected: bool,
    pub forced: bool,
    pub external: bool,
}

/// Build a human-readable label for a track row in the selector popover.
pub fn format_track_label(
    kind: TrackKind,
    language: Option<&str>,
    codec: Option<&str>,
    forced: bool,
) -> String {
    let mut parts = Vec::new();
    if let Some(lang) = language.filter(|s| !s.is_empty()) {
        parts.push(lang.to_string());
    } else {
        parts.push(match kind {
            TrackKind::Audio => "Unknown audio".to_string(),
            TrackKind::Subtitle => "Unknown subtitle".to_string(),
        });
    }
    if let Some(codec) = codec.filter(|s| !s.is_empty()) {
        parts.push(format!("({codec})"));
    }
    if forced {
        parts.push("[Forced]".to_string());
    }
    parts.join(" ")
}

/// Parse audio and subtitle streams from a GStreamer collection.
pub fn tracks_from_collection(
    collection: &StreamCollection,
    active_ids: &[String],
) -> Vec<MediaTrack> {
    collection
        .iter()
        .filter_map(|stream| media_track_from_stream(&stream, active_ids))
        .collect()
}

fn media_track_from_stream(stream: &Stream, active_ids: &[String]) -> Option<MediaTrack> {
    let stream_type = stream.stream_type();
    let kind = if stream_type.contains(StreamType::AUDIO) {
        TrackKind::Audio
    } else if stream_type.contains(StreamType::TEXT) {
        TrackKind::Subtitle
    } else {
        return None;
    };

    let stream_id = stream.stream_id()?.to_string();
    let flags = stream.stream_flags();
    if flags.contains(StreamFlags::UNSELECT) {
        return None;
    }

    let language = stream_language(stream);
    let codec = stream_codec(stream);
    let forced = stream_id.to_lowercase().contains("forced");
    let external = stream_id.contains("external") || stream_id.contains("file");
    let selected = active_ids.iter().any(|id| id == &stream_id);

    Some(MediaTrack {
        label: format_track_label(kind, language.as_deref(), codec.as_deref(), forced),
        stream_id,
        kind,
        language,
        codec,
        selected,
        forced,
        external,
    })
}

fn stream_language(stream: &Stream) -> Option<String> {
    let tags = stream.tags()?;
    tags.get::<gst::tags::LanguageName>()
        .map(|v| v.get().to_string())
        .or_else(|| {
            tags.get::<gst::tags::LanguageCode>()
                .map(|v| v.get().to_string())
        })
}

fn stream_codec(stream: &Stream) -> Option<String> {
    let caps = stream.caps()?;
    caps.structure(0).map(|s| s.name().to_string())
}

pub fn audio_tracks(tracks: &[MediaTrack]) -> Vec<&MediaTrack> {
    tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Audio)
        .collect()
}

pub fn subtitle_tracks(tracks: &[MediaTrack]) -> Vec<&MediaTrack> {
    tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Subtitle)
        .collect()
}

/// Pick the best subtitle stream for a preferred ISO 639-1 language code.
pub fn pick_preferred_subtitle(
    tracks: &[MediaTrack],
    preferred_lang: Option<&str>,
) -> Option<String> {
    let subs = subtitle_tracks(tracks);
    if subs.is_empty() {
        return None;
    }
    let Some(pref) = preferred_lang.filter(|s| !s.is_empty()) else {
        return subs
            .iter()
            .find(|t| t.selected)
            .map(|t| t.stream_id.clone())
            .or_else(|| subs.first().map(|t| t.stream_id.clone()));
    };
    let pref_lower = pref.to_lowercase();
    subs.iter()
        .find(|t| {
            t.language
                .as_ref()
                .is_some_and(|l| l.to_lowercase().starts_with(&pref_lower))
        })
        .or_else(|| {
            subs.iter()
                .find(|t| t.stream_id.to_lowercase().contains(&pref_lower))
        })
        .map(|t| t.stream_id.clone())
        .or_else(|| subs.first().map(|t| t.stream_id.clone()))
}

/// Build the stream-id list to send with `GST_EVENT_SELECT_STREAMS`.
pub fn build_stream_selection(
    tracks: &[MediaTrack],
    active_ids: &[String],
    audio_id: Option<&str>,
    subtitle_id: Option<&str>,
    subtitles_enabled: bool,
) -> Vec<String> {
    let mut selected = Vec::new();
    let is_audio_or_sub = |id: &str| {
        tracks
            .iter()
            .any(|t| t.stream_id == id && matches!(t.kind, TrackKind::Audio | TrackKind::Subtitle))
    };

    // Preserve currently active non-audio/subtitle streams (typically video).
    for id in active_ids {
        if !is_audio_or_sub(id) {
            selected.push(id.clone());
        }
    }

    if let Some(id) = audio_id {
        selected.push(id.to_string());
    } else if let Some(track) = audio_tracks(tracks).into_iter().find(|t| t.selected) {
        selected.push(track.stream_id.clone());
    } else if let Some(track) = audio_tracks(tracks).first() {
        selected.push(track.stream_id.clone());
    }

    if subtitles_enabled {
        if let Some(id) = subtitle_id {
            selected.push(id.to_string());
        } else if let Some(track) = subtitle_tracks(tracks).into_iter().find(|t| t.selected) {
            selected.push(track.stream_id.clone());
        }
    }

    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_track_label_with_language_and_codec() {
        assert_eq!(
            format_track_label(TrackKind::Audio, Some("English"), Some("AAC"), false),
            "English (AAC)"
        );
    }

    #[test]
    fn format_track_label_forced_subtitle() {
        assert_eq!(
            format_track_label(TrackKind::Subtitle, Some("Japanese"), None, true),
            "Japanese [Forced]"
        );
    }

    #[test]
    fn format_track_label_unknown_audio() {
        assert_eq!(
            format_track_label(TrackKind::Audio, None, Some("AC3"), false),
            "Unknown audio (AC3)"
        );
    }

    #[test]
    fn pick_preferred_subtitle_matches_language_code() {
        let tracks = vec![
            MediaTrack {
                stream_id: "sub-en".into(),
                kind: TrackKind::Subtitle,
                label: "English".into(),
                language: Some("en".into()),
                codec: None,
                selected: false,
                forced: false,
                external: false,
            },
            MediaTrack {
                stream_id: "sub-es".into(),
                kind: TrackKind::Subtitle,
                label: "Spanish".into(),
                language: Some("es".into()),
                codec: None,
                selected: true,
                forced: false,
                external: false,
            },
        ];
        assert_eq!(
            pick_preferred_subtitle(&tracks, Some("en")),
            Some("sub-en".into())
        );
    }

    #[test]
    fn pick_preferred_subtitle_none_picks_selected_or_first() {
        let tracks = vec![MediaTrack {
            stream_id: "sub-1".into(),
            kind: TrackKind::Subtitle,
            label: "One".into(),
            language: None,
            codec: None,
            selected: false,
            forced: false,
            external: false,
        }];
        assert_eq!(pick_preferred_subtitle(&tracks, None), Some("sub-1".into()));
    }

    #[test]
    fn build_stream_selection_swaps_audio_keeps_video() {
        let tracks = vec![
            MediaTrack {
                stream_id: "audio-en".into(),
                kind: TrackKind::Audio,
                label: String::new(),
                language: None,
                codec: None,
                selected: true,
                forced: false,
                external: false,
            },
            MediaTrack {
                stream_id: "audio-jp".into(),
                kind: TrackKind::Audio,
                label: String::new(),
                language: None,
                codec: None,
                selected: false,
                forced: false,
                external: false,
            },
        ];
        let active = vec!["video0".into(), "audio-en".into()];
        let sel = build_stream_selection(&tracks, &active, Some("audio-jp"), None, false);
        assert!(sel.contains(&"video0".to_string()));
        assert!(sel.contains(&"audio-jp".to_string()));
        assert!(!sel.contains(&"audio-en".to_string()));
    }
}
