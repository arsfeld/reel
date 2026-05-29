use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::media::MediaItem;
use crate::models::watch::WatchProgress;

/// Position (seconds) to resume playback from, honoring **server-wins**.
///
/// The source's own offset wins first ([`MediaItem::resume_position_secs`]).
/// When the source has no resume opinion, we normally fall back to local
/// progress — but a server-*watched* item must NOT resume from a stale local
/// offset (AE6): the inverse of "server wins". So if the owning server says the
/// item is watched, the local fallback is suppressed entirely; local is only
/// consulted when the server has no opinion (e.g. an unreachable server or a
/// `Local` source).
///
/// Superseded at the live Play call-site by [`super::watch_events::resume_position`]
/// (which also folds in offline-download progress); retained for its server-wins /
/// AE6 coverage of the source-watched fallback path.
#[allow(dead_code)]
pub fn resume_position_for(item: &MediaItem, local: Option<&WatchProgress>) -> Option<f64> {
    if let Some(secs) = item.resume_position_secs() {
        return Some(secs);
    }
    // Server gave no resume offset. If the owning server says watched, treat it
    // as watched and ignore any stale local offset.
    if item.watched && item.source_type.reports_watch_state() {
        return None;
    }
    local
        .filter(|progress| progress.should_show_resume())
        .map(WatchProgress::resume_position)
}

/// Generate a UTC ISO 8601 timestamp string.
pub fn iso_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple ISO format sufficient for sort ordering
    let secs_per_day = 86400;
    let days_since_epoch = now / secs_per_day;
    let time_of_day = now % secs_per_day;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    // Approximate date calculation (sufficient for timestamping)
    let (year, month, day) = days_to_ymd(days_since_epoch);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

/// Convert days since Unix epoch to (year, month, day).
pub fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Parse a text/uri-list string into individual file:// URIs (or plain paths).
pub fn parse_uri_list(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            if let Some(stripped) = line.strip_prefix("file://") {
                // URL-decode the path (basic: replace %XX)
                urlencoding_decode(stripped)
            } else {
                line.to_string()
            }
        })
        .collect()
}

/// Minimal percent-decode for file:// URLs.
pub fn urlencoding_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hi = chars.next().and_then(|c| c.to_digit(16));
            let lo = chars.next().and_then(|c| c.to_digit(16));
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push(char::from_u32((hi << 4) | lo).unwrap_or('\u{FFFD}'));
            } else {
                out.push('%');
                if let Some(c) = chars.next() {
                    out.push(c);
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Map a resolved [`PlaybackDecision`] plus the desired resume position into the
/// `SetUrl` parameters and the transcode base offset. Pure so the play-path
/// resume policy (the load-bearing KTD1 decision) is unit-testable.
///
/// Returns `(url, resume_secs, base_offset_secs)`:
/// - **Direct-play:** the raw file timeline 0 == content 0, so the player seeks
///   client-side to `position` (`resume_secs = position`); `base_offset = 0`.
/// - **Transcode / direct-stream:** the stream is built at `offset = position`
///   and `playbin3` reports position from 0 (U1 finding), so `resume_secs = 0`
///   (no double-seek) and the player adds `base_offset = position` for the
///   displayed position / seek bar / scrobble. (Within U5's ≤12s offset clamp,
///   `base_offset` is effectively the start.)
pub fn set_url_for_decision(
    decision: &crate::models::playback::PlaybackDecision,
    resume_secs: Option<f64>,
) -> (String, Option<f64>, f64) {
    use crate::models::playback::PlaybackDecisionKind;
    let position = resume_secs.unwrap_or(0.0);
    match decision.kind {
        PlaybackDecisionKind::DirectPlay => (decision.url.clone(), resume_secs, 0.0),
        PlaybackDecisionKind::Transcode | PlaybackDecisionKind::DirectStream => {
            (decision.url.clone(), Some(0.0), position)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::media::{MediaType, SourceType};
    use crate::models::playback::{PlaybackDecision, PlaybackDecisionKind};

    fn decision(kind: PlaybackDecisionKind) -> PlaybackDecision {
        PlaybackDecision {
            kind,
            url: "https://h/x?X-Plex-Token=t".into(),
            session: Some("s".into()),
            video_resolution: Some("720p".into()),
            video_bitrate_kbps: Some(1877),
            throttled: false,
            audio_streams: Vec::new(),
            subtitle_streams: Vec::new(),
        }
    }

    fn item(source_type: SourceType, watched: bool, position_ms: Option<i64>) -> MediaItem {
        MediaItem {
            id: "id".into(),
            source_type,
            source_id: "s".into(),
            external_id: "e".into(),
            media_type: MediaType::Movie,
            title: "T".into(),
            year: None,
            overview: None,
            content_rating: None,
            rating: None,
            runtime_minutes: Some(100),
            poster_path: None,
            series_poster_path: None,
            backdrop_path: None,
            genres: vec![],
            parent_id: None,
            season_number: None,
            episode_number: None,
            air_date: None,
            file_path: None,
            video_resolution: None,
            hdr: None,
            added_at: String::new(),
            updated_at: String::new(),
            playback_position_ms: position_ms,
            watched,
            library_section_id: None,
        }
    }

    fn local(position: f64) -> WatchProgress {
        WatchProgress {
            media_item_id: "id".into(),
            position_seconds: position,
            duration_seconds: 6000.0,
            watched: false,
            last_watched_at: String::new(),
        }
    }

    #[test]
    fn direct_play_seeks_to_position_no_base_offset() {
        let (_url, resume, base) =
            set_url_for_decision(&decision(PlaybackDecisionKind::DirectPlay), Some(2530.0));
        assert_eq!(resume, Some(2530.0));
        assert_eq!(base, 0.0);
    }

    #[test]
    fn transcode_resumes_at_zero_with_base_offset() {
        // KTD1: server offset already applied; resume_secs=0 avoids a double
        // jump (a switch at 2530s lands at content 2530s, not ~5060s/EOF).
        let (_url, resume, base) =
            set_url_for_decision(&decision(PlaybackDecisionKind::Transcode), Some(2530.0));
        assert_eq!(resume, Some(0.0));
        assert_eq!(base, 2530.0);
    }

    #[test]
    fn direct_stream_uses_transcode_resume_policy() {
        let (_url, resume, base) =
            set_url_for_decision(&decision(PlaybackDecisionKind::DirectStream), Some(100.0));
        assert_eq!(resume, Some(0.0));
        assert_eq!(base, 100.0);
    }

    #[test]
    fn no_resume_position_is_zero_everywhere() {
        let (_u, resume, base) =
            set_url_for_decision(&decision(PlaybackDecisionKind::Transcode), None);
        assert_eq!(resume, Some(0.0));
        assert_eq!(base, 0.0);
    }

    #[test]
    fn server_offset_wins_over_local() {
        // In-progress server offset (40 min into a 100 min film).
        let it = item(SourceType::Jellyfin, false, Some(40 * 60_000));
        let got = resume_position_for(&it, Some(&local(310.0)));
        assert_eq!(got, Some(40.0 * 60.0 - 10.0));
    }

    #[test]
    fn server_watched_item_ignores_stale_local_offset() {
        // Server says watched; a stale local in-progress offset must be ignored.
        let it = item(SourceType::Jellyfin, true, None);
        assert_eq!(resume_position_for(&it, Some(&local(310.0))), None);
    }

    #[test]
    fn falls_back_to_local_when_server_silent() {
        // Server has no offset and isn't watched → use local progress.
        let it = item(SourceType::Plex, false, None);
        assert_eq!(resume_position_for(&it, Some(&local(310.0))), Some(300.0));
    }

    #[test]
    fn unreachable_server_with_no_opinion_uses_local() {
        // No server opinion at all (None offset, not watched) falls back to local
        // — this is the AE3 path.
        let it = item(SourceType::Jellyfin, false, None);
        assert_eq!(resume_position_for(&it, Some(&local(310.0))), Some(300.0));
    }

    #[test]
    fn no_local_and_no_server_offset_is_none() {
        let it = item(SourceType::Plex, false, None);
        assert_eq!(resume_position_for(&it, None), None);
    }
}
