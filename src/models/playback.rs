//! Playback-decision domain types shared across the source-abstraction
//! boundary (the `resolve_playback` trait method), the Plex transcode client,
//! and the player's quality menu UI. Pure data — no GStreamer/GTK/HTTP.

/// A user-selectable quality rung. `Original` keeps full resolution and lifts
/// the bitrate cap (but still permits a *compatibility* transcode — it is not
/// "never transcode", KTD4). The remaining rungs map to a
/// `maxVideoBitrate × videoResolution × videoQuality` triple the transcoder
/// honors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityPreset {
    Original,
    P1080Mbps20,
    P1080Mbps12,
    P1080Mbps8,
    P720Mbps4,
    P720Mbps3,
    P480Mbps2,
    P480Kbps720,
}

impl QualityPreset {
    /// The ladder, highest quality first. `Original` leads; descending caps
    /// follow. This order drives the menu and the watchdog step-down (U8).
    pub const LADDER: &'static [QualityPreset] = &[
        QualityPreset::Original,
        QualityPreset::P1080Mbps20,
        QualityPreset::P1080Mbps12,
        QualityPreset::P1080Mbps8,
        QualityPreset::P720Mbps4,
        QualityPreset::P720Mbps3,
        QualityPreset::P480Mbps2,
        QualityPreset::P480Kbps720,
    ];

    /// The default cap applied automatically on a remote/relay connection (R6).
    /// A mid-ladder rung (1080p · 8 Mbps) — a sane default, not a measurement.
    pub const REMOTE_DEFAULT: QualityPreset = QualityPreset::P1080Mbps8;

    /// `maxVideoBitrate` in kbps, or `None` for `Original` (uncapped).
    pub fn max_video_bitrate_kbps(self) -> Option<u32> {
        match self {
            QualityPreset::Original => None,
            QualityPreset::P1080Mbps20 => Some(20_000),
            QualityPreset::P1080Mbps12 => Some(12_000),
            QualityPreset::P1080Mbps8 => Some(8_000),
            QualityPreset::P720Mbps4 => Some(4_000),
            QualityPreset::P720Mbps3 => Some(3_000),
            QualityPreset::P480Mbps2 => Some(2_000),
            QualityPreset::P480Kbps720 => Some(720),
        }
    }

    /// `videoResolution` as Plex's `WxH`, or `None` for `Original` (source res).
    pub fn video_resolution(self) -> Option<&'static str> {
        match self {
            QualityPreset::Original => None,
            QualityPreset::P1080Mbps20 | QualityPreset::P1080Mbps12 | QualityPreset::P1080Mbps8 => {
                Some("1920x1080")
            }
            QualityPreset::P720Mbps4 | QualityPreset::P720Mbps3 => Some("1280x720"),
            QualityPreset::P480Mbps2 | QualityPreset::P480Kbps720 => Some("720x480"),
        }
    }

    /// Plex `videoQuality` (0–100). `Original` is the most permissive.
    pub fn video_quality(self) -> u32 {
        match self {
            QualityPreset::Original | QualityPreset::P1080Mbps20 | QualityPreset::P1080Mbps12 => {
                100
            }
            QualityPreset::P1080Mbps8 | QualityPreset::P720Mbps4 => 90,
            QualityPreset::P720Mbps3 | QualityPreset::P480Mbps2 => 80,
            QualityPreset::P480Kbps720 => 60,
        }
    }

    /// Human-readable menu label.
    pub fn label(self) -> &'static str {
        match self {
            QualityPreset::Original => "Original",
            QualityPreset::P1080Mbps20 => "1080p · 20 Mbps",
            QualityPreset::P1080Mbps12 => "1080p · 12 Mbps",
            QualityPreset::P1080Mbps8 => "1080p · 8 Mbps",
            QualityPreset::P720Mbps4 => "720p · 4 Mbps",
            QualityPreset::P720Mbps3 => "720p · 3 Mbps",
            QualityPreset::P480Mbps2 => "480p · 2 Mbps",
            QualityPreset::P480Kbps720 => "480p · 720 kbps",
        }
    }

    /// The next lower rung, for the buffering watchdog's auto-step-down (U8).
    /// Returns `None` at the bottom of the ladder.
    pub fn step_down(self) -> Option<QualityPreset> {
        let idx = Self::LADDER.iter().position(|&p| p == self)?;
        Self::LADDER.get(idx + 1).copied()
    }
}

/// What the user picked in the quality menu: `Auto` lets the server decide
/// (with the remote default cap applied when remote), or a `Manual` rung.
/// `Auto` is distinct from `Manual(Original)` — see U9.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualitySelection {
    Auto,
    Manual(QualityPreset),
}

/// The kind of playback the server decided on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackDecisionKind {
    /// Original file served as-is (raw download URL).
    DirectPlay,
    /// Remux without re-encoding video (HLS).
    DirectStream,
    /// Full transcode (HLS).
    Transcode,
}

impl PlaybackDecisionKind {
    /// Direct-play seeks client-side on the raw timeline; transcode/direct-stream
    /// reload at a new offset (KTD1/KTD2).
    pub fn is_transcode_like(self) -> bool {
        matches!(self, Self::DirectStream | Self::Transcode)
    }
}

/// Redact the `X-Plex-Token` value in a URL or arbitrary string so it never
/// reaches logs, `Debug` output, or error dialogs. Replaces the token value
/// with `REDACTED` while leaving the rest intact.
pub fn redact_plex_token(s: &str) -> String {
    // Match `X-Plex-Token=<value>` up to the next `&`, whitespace, or end.
    let needle = "X-Plex-Token=";
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find(needle) {
        let (before, after) = rest.split_at(pos + needle.len());
        out.push_str(before);
        out.push_str("REDACTED");
        // Skip the token value: everything until the next delimiter.
        let end = after
            .find(|c: char| c == '&' || c.is_whitespace())
            .unwrap_or(after.len());
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// An audio or subtitle track available for a transcode re-decision (AE6).
/// Sourced from the decision response's annotated source stream list, so it
/// carries the Plex stream id (passed back as `audioStreamID`/`subtitleStreamID`)
/// and a human-readable label, regardless of what the HLS output actually muxed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionStream {
    pub id: i64,
    pub label: String,
    /// Whether the server currently has this stream selected.
    pub selected: bool,
}

/// The resolved playback decision: the URL to hand to the pipeline plus the
/// server-actual output metadata for the indicator (R16). Resolution/bitrate
/// come from the server's selected `Media`, never echoed from the request.
///
/// `Debug` is hand-written to redact the `X-Plex-Token` carried in `url`
/// (token hygiene — the URL must keep the token for the pipeline's segment
/// fetches, but it must never leak to logs).
#[derive(Clone, PartialEq)]
pub struct PlaybackDecision {
    pub kind: PlaybackDecisionKind,
    /// Playback URL for the pipeline. For transcode/direct-stream this carries
    /// `X-Plex-Token` in the query so GStreamer's segment fetches inherit it.
    pub url: String,
    /// The transcode session id (for keepalive/stop). `None` for direct-play.
    pub session: Option<String>,
    /// Server-actual output resolution (e.g. "720p"), from the selected Media.
    pub video_resolution: Option<String>,
    /// Server-actual output bitrate in kbps, from the selected Media.
    pub video_bitrate_kbps: Option<i64>,
    /// Whether the transcoder reported itself throttled at decision time.
    pub throttled: bool,
    /// Audio tracks the server reported for this title, for the transcode-aware
    /// audio menu (AE6). Empty for direct-play (live GStreamer selection wins).
    pub audio_streams: Vec<DecisionStream>,
    /// Subtitle tracks, same as `audio_streams` (AE6).
    pub subtitle_streams: Vec<DecisionStream>,
}

impl PlaybackDecision {
    /// Human-readable decision indicator for the OSD (R16). Direct-play reads
    /// "Direct Play"; a transcode/direct-stream reads "Transcoding" with the
    /// server-actual output resolution and bitrate appended when known (sourced
    /// from the decision response, never the requested preset — so an "Original"
    /// that compatibility-transcodes shows the real res/bitrate).
    pub fn indicator_text(&self) -> String {
        if self.kind == PlaybackDecisionKind::DirectPlay {
            return "Direct Play".to_string();
        }
        let mut s = "Transcoding".to_string();
        if let Some(res) = self.video_resolution.as_deref().filter(|r| !r.is_empty()) {
            s.push(' ');
            s.push_str(&format_resolution(res));
        }
        if let Some(kbps) = self.video_bitrate_kbps.filter(|b| *b > 0) {
            s.push_str(" · ");
            s.push_str(&format_bitrate_kbps(kbps));
        }
        s
    }
}

/// Normalize Plex's `videoResolution` (e.g. "1080", "720", "4k", "sd") for
/// display: bare digit strings gain a "p" suffix, named tiers are upper-cased.
fn format_resolution(res: &str) -> String {
    let r = res.trim();
    if !r.is_empty() && r.chars().all(|c| c.is_ascii_digit()) {
        format!("{r}p")
    } else {
        r.to_uppercase()
    }
}

/// Format a kbps bitrate as "8 Mbps" / "1.5 Mbps" (≥ 1000 kbps) or "720 kbps".
fn format_bitrate_kbps(kbps: i64) -> String {
    if kbps >= 1000 {
        let mbps = kbps as f64 / 1000.0;
        if (mbps.fract()).abs() < 0.05 {
            format!("{} Mbps", mbps.round() as i64)
        } else {
            format!("{mbps:.1} Mbps")
        }
    } else {
        format!("{kbps} kbps")
    }
}

impl std::fmt::Debug for PlaybackDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlaybackDecision")
            .field("kind", &self.kind)
            .field("url", &redact_plex_token(&self.url))
            .field("session", &self.session)
            .field("video_resolution", &self.video_resolution)
            .field("video_bitrate_kbps", &self.video_bitrate_kbps)
            .field("throttled", &self.throttled)
            .field("audio_streams", &self.audio_streams)
            .field("subtitle_streams", &self.subtitle_streams)
            .finish()
    }
}

/// Everything needed to ask the source to resolve a playback URL.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackRequest {
    /// The item's rating key (Plex `external_id`), e.g. "136798". The transcode
    /// `path` is `/library/metadata/{rating_key}`.
    pub rating_key: String,
    /// The part key / file path (Plex `file_path`), e.g.
    /// "/library/parts/950874/…/file.mkv" — used to build the direct-play URL.
    pub part_key: String,
    /// Positional media/part indices for multi-version / multi-part media.
    pub media_index: u32,
    pub part_index: u32,
    /// Auto or a manual rung.
    pub quality: QualitySelection,
    /// Force a transcode even if the file looks compatible (R10/KTD11).
    pub force_transcode: bool,
    pub audio_stream_id: Option<i64>,
    pub subtitle_stream_id: Option<i64>,
    /// Part-relative resume offset in seconds.
    pub offset_secs: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ladder_is_ordered_descending_by_bitrate() {
        // Original (uncapped) leads; the rest strictly descend in bitrate.
        assert_eq!(QualityPreset::LADDER[0], QualityPreset::Original);
        let caps: Vec<u32> = QualityPreset::LADDER
            .iter()
            .skip(1)
            .map(|p| p.max_video_bitrate_kbps().unwrap())
            .collect();
        let mut sorted = caps.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(caps, sorted, "ladder bitrates must strictly descend");
    }

    #[test]
    fn preset_maps_to_expected_bitrate() {
        assert_eq!(
            QualityPreset::P1080Mbps8.max_video_bitrate_kbps(),
            Some(8_000)
        );
        assert_eq!(QualityPreset::Original.max_video_bitrate_kbps(), None);
    }

    #[test]
    fn remote_default_is_a_mid_ladder_cap() {
        let d = QualityPreset::REMOTE_DEFAULT;
        assert_eq!(d, QualityPreset::P1080Mbps8);
        assert!(d.max_video_bitrate_kbps().is_some());
    }

    #[test]
    fn step_down_walks_ladder_and_stops_at_bottom() {
        assert_eq!(
            QualityPreset::P720Mbps4.step_down(),
            Some(QualityPreset::P720Mbps3)
        );
        assert_eq!(QualityPreset::P480Kbps720.step_down(), None);
    }

    #[test]
    fn original_label_distinct() {
        assert_eq!(QualityPreset::Original.label(), "Original");
        assert!(QualityPreset::P720Mbps4.label().contains("720p"));
    }

    #[test]
    fn transcode_kinds_are_transcode_like() {
        assert!(PlaybackDecisionKind::Transcode.is_transcode_like());
        assert!(PlaybackDecisionKind::DirectStream.is_transcode_like());
        assert!(!PlaybackDecisionKind::DirectPlay.is_transcode_like());
    }

    #[test]
    fn redact_plex_token_masks_value_in_url() {
        let masked = redact_plex_token(
            "https://h/video/:/transcode/universal/start.m3u8?session=s&X-Plex-Token=secret123&offset=0",
        );
        assert!(!masked.contains("secret123"));
        assert!(masked.contains("X-Plex-Token=REDACTED"));
        assert!(masked.contains("offset=0")); // rest intact
        assert!(masked.contains("session=s"));
    }

    #[test]
    fn redact_plex_token_at_end_of_string() {
        let masked = redact_plex_token("https://h/x?X-Plex-Token=abc");
        assert_eq!(masked, "https://h/x?X-Plex-Token=REDACTED");
    }

    fn decision(
        kind: PlaybackDecisionKind,
        res: Option<&str>,
        kbps: Option<i64>,
    ) -> PlaybackDecision {
        PlaybackDecision {
            kind,
            url: "https://h/x".into(),
            session: None,
            video_resolution: res.map(str::to_string),
            video_bitrate_kbps: kbps,
            throttled: false,
            audio_streams: Vec::new(),
            subtitle_streams: Vec::new(),
        }
    }

    #[test]
    fn indicator_direct_play() {
        assert_eq!(
            decision(PlaybackDecisionKind::DirectPlay, Some("1080"), Some(20_000)).indicator_text(),
            "Direct Play"
        );
    }

    #[test]
    fn indicator_transcode_with_res_and_bitrate() {
        // Covers R16: server-actual res/bitrate from the response, formatted.
        assert_eq!(
            decision(PlaybackDecisionKind::Transcode, Some("1080"), Some(8_000)).indicator_text(),
            "Transcoding 1080p · 8 Mbps"
        );
        assert_eq!(
            decision(PlaybackDecisionKind::Transcode, Some("720"), Some(1_877)).indicator_text(),
            "Transcoding 720p · 1.9 Mbps"
        );
    }

    #[test]
    fn indicator_transcode_named_tier_and_kbps() {
        assert_eq!(
            decision(PlaybackDecisionKind::Transcode, Some("4k"), Some(720)).indicator_text(),
            "Transcoding 4K · 720 kbps"
        );
    }

    #[test]
    fn indicator_transcode_without_metadata() {
        // Original that compatibility-transcodes with no reported res/bitrate.
        assert_eq!(
            decision(PlaybackDecisionKind::DirectStream, None, None).indicator_text(),
            "Transcoding"
        );
        // Empty/zero are treated as absent, not rendered.
        assert_eq!(
            decision(PlaybackDecisionKind::Transcode, Some(""), Some(0)).indicator_text(),
            "Transcoding"
        );
    }

    #[test]
    fn playback_decision_debug_redacts_token() {
        let d = PlaybackDecision {
            kind: PlaybackDecisionKind::Transcode,
            url: "https://h/start.m3u8?X-Plex-Token=secret123&offset=0".into(),
            session: Some("sess".into()),
            video_resolution: Some("720p".into()),
            video_bitrate_kbps: Some(1877),
            throttled: false,
            audio_streams: Vec::new(),
            subtitle_streams: Vec::new(),
        };
        let dbg = format!("{d:?}");
        assert!(!dbg.contains("secret123"));
        assert!(dbg.contains("REDACTED"));
    }
}
