use std::time::{SystemTime, UNIX_EPOCH};

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
    use crate::models::playback::{PlaybackDecision, PlaybackDecisionKind};

    fn decision(kind: PlaybackDecisionKind) -> PlaybackDecision {
        PlaybackDecision {
            kind,
            url: "https://h/x?X-Plex-Token=t".into(),
            session: Some("s".into()),
            video_resolution: Some("720p".into()),
            video_bitrate_kbps: Some(1877),
            throttled: false,
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
}
