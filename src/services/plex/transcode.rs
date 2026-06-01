//! Plex universal-transcoder client methods: build the `start.m3u8` URL, derive
//! and run the `/decision`, and manage session lifecycle (stop / keepalive).
//!
//! Kept out of `api.rs` to stay under the file-size cap. All HTTP goes through
//! the shared [`PlexClient`] so the `X-Plex-Token` / `X-Plex-Client-Identifier`
//! default headers apply.
//!
//! ## Live-server traps encoded here (U1 findings)
//! - **`X-Plex-Client-Identifier` must NOT be a query param** — it 400s the
//!   universal transcoder on PMS 1.43. It travels as a default header instead.
//! - **`hasMDE=1` must be omitted** — it also 400s the decision endpoint.
//! - **`X-Plex-Token` MUST be in the playback URL query**, because GStreamer
//!   fetches HLS segments from the URL and cannot send the client's headers.
//! - The decision URL is the `start.m3u8` URL with the endpoint segment swapped
//!   to `decision`, identical query (KTD3, minus the traps above).
//! - `/stop` on an already-reaped session returns 404 — treated as success.

use std::time::Duration;

use reqwest::Url;

use super::api::PlexClient;
use super::models::{PlexDecisionResponse, PlexStream};
use crate::models::playback::{
    DecisionStream, PlaybackDecision, PlaybackDecisionKind, PlaybackRequest, QualityPreset,
    QualitySelection, redact_plex_token,
};

/// Build the [`DecisionStream`] list for one stream type (2 = audio, 3 =
/// subtitle) from a decision response's annotated source streams (AE6). Streams
/// without an id are skipped — they can't be re-requested by `audioStreamID`.
fn decision_streams(streams: &[PlexStream], stream_type: i32) -> Vec<DecisionStream> {
    streams
        .iter()
        .filter(|s| s.stream_type == Some(stream_type))
        .filter_map(|s| {
            let id = s.id?;
            let label = s
                .extended_display_title
                .clone()
                .or_else(|| s.display_title.clone())
                .or_else(|| s.language.clone())
                .or_else(|| s.codec.clone())
                .unwrap_or_else(|| format!("Track {id}"));
            Some(DecisionStream {
                id,
                label,
                selected: s.selected.unwrap_or(false),
            })
        })
        .collect()
}

/// Transcode decision/lifecycle errors. All variants are loud and typed so the
/// play path (U7) can surface an actionable notice rather than silently
/// direct-playing an incompatible file. Any string carrying a URL is redacted.
#[derive(Debug, thiserror::Error)]
pub enum TranscodeError {
    /// The decision request timed out within the short single-attempt budget.
    #[error("transcode decision timed out")]
    Timeout,
    /// A transport error (connection reset, TLS, etc.). Redacted.
    #[error("transcode decision request failed: {0}")]
    Request(String),
    /// The server returned a non-success HTTP status.
    #[error("transcode decision server error: HTTP {status}")]
    Server { status: u16 },
    /// The response body did not deserialize.
    #[error("malformed transcode decision response: {0}")]
    Parse(String),
    /// The response parsed but carried neither a recognizable `Part@decision`
    /// nor a container `generalDecisionCode` we can map — fail loud rather than
    /// defaulting to a kind that would route playback wrong.
    #[error("transcode decision response had no decision field")]
    NoDecision,
    /// Building the request URL failed.
    #[error("could not build transcode URL")]
    Url,
}

/// Short per-call budget for the decision request. Deliberately NOT the shared
/// `request()` retry helper (3× over 15s ≈ 45s worst case would block the play
/// action).
const DECISION_TIMEOUT: Duration = Duration::from_secs(5);
/// Bounded retry budget for `/stop` and `/ping`.
const LIFECYCLE_TIMEOUT: Duration = Duration::from_secs(3);
const STOP_ATTEMPTS: u32 = 3;

const UNIVERSAL: &str = "/video/:/transcode/universal";

/// `X-Plex-Platform` value sent on transcode requests. Required by the universal
/// transcoder (omitting it 400s), and the **value is validated**: `Generic` and
/// `Chrome` are accepted but `Linux` is rejected with 400 (U1 live finding). It
/// must be a query param, not a header, because GStreamer fetches `start.m3u8`
/// and its segments from the URL without the client's headers.
const TRANSCODE_PLATFORM: &str = "Generic";

/// Clamp an offset of ≤ 12s to 0 — Plex 404s the first segment otherwise
/// (documented PFK trap; did not reproduce on PMS 1.43 but kept as cheap
/// insurance, KTD2).
fn clamp_offset(offset_secs: f64) -> i64 {
    if offset_secs <= 12.0 {
        0
    } else {
        offset_secs.round() as i64
    }
}

/// Generate a fresh, unique session id per reload (KTD7). The server honors a
/// client-chosen id (U1(i)). Uses the persistent client identifier as a prefix
/// plus 8 random bytes from `/dev/urandom`.
fn new_session_id(client_prefix: &str) -> String {
    use std::io::Read;
    let mut buf = [0u8; 8];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut buf);
    }
    let hex: String = buf.iter().map(|b| format!("{b:02x}")).collect();
    format!("{client_prefix}-{hex}")
}

/// The effective quality cap for a request: a manual rung, the remote default
/// when Auto on a remote connection, or `None` (uncapped) when Auto on local.
fn effective_preset(quality: QualitySelection, is_remote: bool) -> Option<QualityPreset> {
    match quality {
        QualitySelection::Manual(p) => Some(p),
        QualitySelection::Auto if is_remote => Some(QualityPreset::REMOTE_DEFAULT),
        QualitySelection::Auto => None,
    }
}

impl PlexClient {
    /// Build the ordered `start.m3u8` query parameters for a request. The same
    /// params drive the decision URL (string-replace endpoint). `session` is the
    /// per-reload id; `client_id_prefix` is unused here (the id travels as a
    /// header, never a query param — U1 trap).
    fn transcode_params(&self, req: &PlaybackRequest, session: &str) -> Vec<(String, String)> {
        let mut p: Vec<(String, String)> = Vec::new();
        p.push((
            "path".into(),
            format!("/library/metadata/{}", req.rating_key),
        ));
        p.push(("mediaIndex".into(), req.media_index.to_string()));
        p.push(("partIndex".into(), req.part_index.to_string()));
        p.push(("protocol".into(), "hls".into()));
        p.push(("fastSeek".into(), "1".into()));
        p.push(("copyts".into(), "1".into()));
        p.push(("offset".into(), clamp_offset(req.offset_secs).to_string()));
        p.push(("session".into(), session.to_string()));

        // KTD11: forced transcode disables both direct paths so the decision
        // cannot come back directplay; Auto lets the server decide.
        if req.force_transcode {
            p.push(("directPlay".into(), "0".into()));
            p.push(("directStream".into(), "0".into()));
        } else {
            p.push(("directPlay".into(), "1".into()));
            p.push(("directStream".into(), "1".into()));
        }

        // Quality cap triple (KTD4 / R6).
        if let Some(preset) = effective_preset(req.quality, self.is_remote()) {
            p.push(("videoQuality".into(), preset.video_quality().to_string()));
            if let Some(br) = preset.max_video_bitrate_kbps() {
                p.push(("maxVideoBitrate".into(), br.to_string()));
            }
            if let Some(res) = preset.video_resolution() {
                p.push(("videoResolution".into(), res.into()));
            }
        }

        if let Some(a) = req.audio_stream_id {
            p.push(("audioStreamID".into(), a.to_string()));
        }
        if let Some(s) = req.subtitle_stream_id {
            p.push(("subtitleStreamID".into(), s.to_string()));
            // `auto` lets the server burn image subs / soft-mux text subs; the
            // burn refinement for known image codecs lands in U10.
            p.push(("subtitles".into(), "auto".into()));
        }

        // `location=lan` only on a local connection (KTD6/U2).
        if !self.is_remote() {
            p.push(("location".into(), "lan".into()));
        }

        // Required by the transcoder; value is validated (Generic, not Linux).
        p.push(("X-Plex-Platform".into(), TRANSCODE_PLATFORM.into()));
        p.push((
            "X-Plex-Client-Profile-Extra".into(),
            super::transcode_profile::client_profile_extra(req.can_direct_play_10bit, req.can_direct_play_hdr),
        ));
        // Token MUST be in the query so GStreamer's segment fetches inherit it.
        p.push(("X-Plex-Token".into(), self.token().to_string()));
        p
    }

    /// Build a universal-transcoder URL for `endpoint` (e.g. `start.m3u8`,
    /// `decision`) with the given params.
    fn universal_url(
        &self,
        endpoint: &str,
        params: &[(String, String)],
    ) -> Result<String, TranscodeError> {
        let base = format!("{}{}/{}", self.base_url(), UNIVERSAL, endpoint);
        Url::parse_with_params(&base, params)
            .map(|u| u.to_string())
            .map_err(|_| TranscodeError::Url)
    }

    /// Build the `start.m3u8` transcode URL for a request (the URL handed to the
    /// pipeline when the decision is transcode/direct-stream).
    pub fn transcode_start_url(
        &self,
        req: &PlaybackRequest,
        session: &str,
    ) -> Result<String, TranscodeError> {
        let params = self.transcode_params(req, session);
        self.universal_url("start.m3u8", &params)
    }

    /// Run the transcode decision for a request and resolve a [`PlaybackDecision`].
    /// Generates a fresh session id; returns it on transcode/direct-stream.
    pub async fn resolve_decision(
        &self,
        req: &PlaybackRequest,
    ) -> Result<PlaybackDecision, TranscodeError> {
        let session = new_session_id("reel");
        let params = self.transcode_params(req, &session);
        let decision_url = self.universal_url("decision", &params)?;

        let resp = self
            .http()
            .get(&decision_url)
            .timeout(DECISION_TIMEOUT)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    TranscodeError::Timeout
                } else {
                    TranscodeError::Request(redact_plex_token(&e.to_string()))
                }
            })?;

        if !resp.status().is_success() {
            return Err(TranscodeError::Server {
                status: resp.status().as_u16(),
            });
        }

        let body = resp
            .text()
            .await
            .map_err(|e| TranscodeError::Request(redact_plex_token(&e.to_string())))?;
        let decision: PlexDecisionResponse = serde_json::from_str(&body)
            .map_err(|e| TranscodeError::Parse(redact_plex_token(&e.to_string())))?;

        let result = self.decision_to_playback(req, &session, decision);
        if let Err(TranscodeError::NoDecision) = &result {
            // The response parsed but carried no mappable decision — typically a
            // server-side reject (e.g. transcodeDecisionCode 4005, "No conversion
            // profile found"). Log the redacted body so the actual decision codes
            // are recoverable from logs rather than opaque behind a generic error.
            tracing::warn!(
                rating_key = %req.rating_key,
                body = %redact_plex_token(&body),
                "transcode decision had no mappable decision; raw body follows"
            );
        }
        result
    }

    /// Map a parsed decision response into a [`PlaybackDecision`], building the
    /// right URL for the decided kind.
    fn decision_to_playback(
        &self,
        req: &PlaybackRequest,
        session: &str,
        decision: PlexDecisionResponse,
    ) -> Result<PlaybackDecision, TranscodeError> {
        let mc = decision.media_container;
        let media = mc.metadata.first().and_then(|m| {
            m.media
                .iter()
                .find(|m| m.selected == Some(true))
                .or(m.media.first())
        });
        let part = media.and_then(|m| m.parts.first());

        // The per-Part decision is authoritative when present. Some Plex server
        // versions omit it even on a perfectly valid response, in which case the
        // container-level decision codes carry the verdict: generalDecisionCode
        // 1000 == "Direct play OK". Fall back to that rather than erroring (which
        // would force a noisy direct-play fallback for content the server already
        // cleared for direct play). Anything we still can't classify is a loud
        // error, not a silent default that would route playback wrong.
        let kind = match part.and_then(|p| p.decision.as_deref()) {
            Some("directplay") => PlaybackDecisionKind::DirectPlay,
            Some("copy") => PlaybackDecisionKind::DirectStream,
            Some("transcode") => PlaybackDecisionKind::Transcode,
            Some(_) => return Err(TranscodeError::NoDecision),
            None => match mc.general_decision_code {
                Some(1000) => PlaybackDecisionKind::DirectPlay,
                _ => return Err(TranscodeError::NoDecision),
            },
        };

        // Output res/bitrate come from the selected Media, never the request
        // (U1: TranscodeSession is absent from the decision response).
        let video_resolution = media.and_then(|m| m.video_resolution.clone());
        let video_bitrate_kbps = media.and_then(|m| m.bitrate);
        let throttled = mc
            .transcode_session
            .as_ref()
            .and_then(|s| s.throttled)
            .unwrap_or(false);

        let (url, session_out) = match kind {
            PlaybackDecisionKind::DirectPlay => {
                // Raw file URL — direct-play timeline 0 == content 0.
                (self.playback_url(&req.part_key), None)
            }
            PlaybackDecisionKind::DirectStream | PlaybackDecisionKind::Transcode => (
                self.transcode_start_url(req, session)?,
                Some(session.to_string()),
            ),
        };

        // Available audio/subtitle tracks for the transcode-aware track menu
        // (AE6) — the annotated source stream list, with Plex stream ids.
        let streams = part.map(|p| p.streams.as_slice()).unwrap_or(&[]);
        let audio_streams = decision_streams(streams, 2);
        let subtitle_streams = decision_streams(streams, 3);

        Ok(PlaybackDecision {
            kind,
            url,
            session: session_out,
            video_resolution,
            video_bitrate_kbps,
            throttled,
            audio_streams,
            subtitle_streams,
        })
    }

    /// Stop a transcode session (R14). Bounded retry with a short timeout; a 404
    /// means the session already reaped — treated as success (U1).
    pub async fn stop_transcode(&self, session: &str) -> Result<(), TranscodeError> {
        let url = self.universal_url(
            "stop",
            &[
                ("session".into(), session.to_string()),
                ("X-Plex-Token".into(), self.token().to_string()),
            ],
        )?;
        let mut last: Option<TranscodeError> = None;
        for _ in 0..STOP_ATTEMPTS {
            match self
                .http()
                .get(&url)
                .timeout(LIFECYCLE_TIMEOUT)
                .send()
                .await
            {
                Ok(resp) => {
                    let s = resp.status();
                    if s.is_success() || s.as_u16() == 404 {
                        return Ok(());
                    }
                    last = Some(TranscodeError::Server { status: s.as_u16() });
                }
                Err(e) if e.is_timeout() => last = Some(TranscodeError::Timeout),
                Err(e) => last = Some(TranscodeError::Request(redact_plex_token(&e.to_string()))),
            }
        }
        Err(last.unwrap_or(TranscodeError::Timeout))
    }

    /// Keep a transcode session alive (R15). Fired on a fixed-interval timer by
    /// U10. Single short attempt — a missed ping is recovered by the next tick.
    pub async fn ping_transcode(&self, session: &str) -> Result<(), TranscodeError> {
        let url = self.universal_url(
            "ping",
            &[
                ("session".into(), session.to_string()),
                ("X-Plex-Token".into(), self.token().to_string()),
            ],
        )?;
        let resp = self
            .http()
            .get(&url)
            .timeout(LIFECYCLE_TIMEOUT)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    TranscodeError::Timeout
                } else {
                    TranscodeError::Request(redact_plex_token(&e.to_string()))
                }
            })?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(TranscodeError::Server {
                status: resp.status().as_u16(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> PlaybackRequest {
        PlaybackRequest {
            rating_key: "136798".into(),
            part_key: "/library/parts/950874/1772297784/file.mkv".into(),
            media_index: 0,
            part_index: 0,
            quality: QualitySelection::Auto,
            force_transcode: false,
            can_direct_play_10bit: false,
            can_direct_play_hdr: false,
            backend_kind: crate::models::playback::PlaybackBackendKind::GStreamer,
            audio_stream_id: None,
            subtitle_stream_id: None,
            offset_secs: 0.0,
        }
    }

    fn parse_query(url: &str) -> std::collections::HashMap<String, String> {
        Url::parse(url)
            .unwrap()
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect()
    }

    #[test]
    fn clamp_offset_below_threshold_is_zero() {
        assert_eq!(clamp_offset(5.0), 0);
        assert_eq!(clamp_offset(12.0), 0);
        assert_eq!(clamp_offset(90.4), 90);
    }

    #[test]
    fn effective_preset_auto_remote_uses_default_cap() {
        // Covers R6.
        assert_eq!(
            effective_preset(QualitySelection::Auto, true),
            Some(QualityPreset::REMOTE_DEFAULT)
        );
        assert_eq!(effective_preset(QualitySelection::Auto, false), None);
        assert_eq!(
            effective_preset(QualitySelection::Manual(QualityPreset::P720Mbps4), false),
            Some(QualityPreset::P720Mbps4)
        );
    }

    #[test]
    fn remote_source_url_carries_default_cap() {
        // Covers R6: remote Auto -> default maxVideoBitrate; local -> none.
        let remote = PlexClient::new("http://h:32400", "tok").with_remote(true);
        let q = parse_query(&remote.transcode_start_url(&req(), "s1").unwrap());
        assert_eq!(q.get("maxVideoBitrate").map(String::as_str), Some("8000"));
        assert_eq!(q.get("location"), None); // remote: no location=lan

        let local = PlexClient::new("http://h:32400", "tok").with_remote(false);
        let q = parse_query(&local.transcode_start_url(&req(), "s1").unwrap());
        assert_eq!(q.get("maxVideoBitrate"), None);
        assert_eq!(q.get("location").map(String::as_str), Some("lan"));
    }

    #[test]
    fn forced_transcode_emits_directplay_zero() {
        // Covers R10 / KTD11.
        let c = PlexClient::new("http://h:32400", "tok");
        let mut r = req();
        r.force_transcode = true;
        let q = parse_query(&c.transcode_start_url(&r, "s1").unwrap());
        assert_eq!(q.get("directPlay").map(String::as_str), Some("0"));
        assert_eq!(q.get("directStream").map(String::as_str), Some("0"));

        let q = parse_query(&c.transcode_start_url(&req(), "s1").unwrap());
        assert_eq!(q.get("directPlay").map(String::as_str), Some("1"));
        assert_eq!(q.get("directStream").map(String::as_str), Some("1"));
    }

    #[test]
    fn manual_preset_overrides_cap() {
        let c = PlexClient::new("http://h:32400", "tok").with_remote(true);
        let mut r = req();
        r.quality = QualitySelection::Manual(QualityPreset::P720Mbps4);
        let q = parse_query(&c.transcode_start_url(&r, "s1").unwrap());
        assert_eq!(q.get("maxVideoBitrate").map(String::as_str), Some("4000"));
        assert_eq!(
            q.get("videoResolution").map(String::as_str),
            Some("1280x720")
        );
    }

    #[test]
    fn original_preset_has_no_bitrate_cap() {
        let c = PlexClient::new("http://h:32400", "tok");
        let mut r = req();
        r.quality = QualitySelection::Manual(QualityPreset::Original);
        let q = parse_query(&c.transcode_start_url(&r, "s1").unwrap());
        assert_eq!(q.get("maxVideoBitrate"), None);
        assert_eq!(q.get("videoQuality").map(String::as_str), Some("100"));
    }

    #[test]
    fn url_never_contains_client_identifier_or_hasmde_in_query() {
        // U1 traps: both 400 the universal transcoder as query params.
        let c = PlexClient::new("http://h:32400", "tok");
        let url = c.transcode_start_url(&req(), "s1").unwrap();
        let q = parse_query(&url);
        assert!(!q.contains_key("X-Plex-Client-Identifier"));
        assert!(!q.contains_key("hasMDE"));
        // copyts + fastSeek + token present.
        assert_eq!(q.get("copyts").map(String::as_str), Some("1"));
        assert_eq!(q.get("X-Plex-Token").map(String::as_str), Some("tok"));
    }

    #[test]
    fn url_carries_required_validated_platform() {
        // U1 trap: the transcoder 400s without X-Plex-Platform, and rejects
        // "Linux" — it must be a recognized value sent as a query param.
        let c = PlexClient::new("http://h:32400", "tok");
        let q = parse_query(&c.transcode_start_url(&req(), "s1").unwrap());
        assert_eq!(
            q.get("X-Plex-Platform").map(String::as_str),
            Some("Generic")
        );
    }

    #[test]
    fn offset_clamped_in_url() {
        let c = PlexClient::new("http://h:32400", "tok");
        let mut r = req();
        r.offset_secs = 5.0;
        let q = parse_query(&c.transcode_start_url(&r, "s1").unwrap());
        assert_eq!(q.get("offset").map(String::as_str), Some("0"));
        r.offset_secs = 90.0;
        let q = parse_query(&c.transcode_start_url(&r, "s1").unwrap());
        assert_eq!(q.get("offset").map(String::as_str), Some("90"));
    }

    #[test]
    fn selected_streams_in_url() {
        let c = PlexClient::new("http://h:32400", "tok");
        let mut r = req();
        r.audio_stream_id = Some(3799553);
        r.subtitle_stream_id = Some(3799555);
        let q = parse_query(&c.transcode_start_url(&r, "s1").unwrap());
        assert_eq!(q.get("audioStreamID").map(String::as_str), Some("3799553"));
        assert_eq!(
            q.get("subtitleStreamID").map(String::as_str),
            Some("3799555")
        );
        assert_eq!(q.get("subtitles").map(String::as_str), Some("auto"));
    }

    #[test]
    fn multi_part_index_in_url() {
        let c = PlexClient::new("http://h:32400", "tok");
        let mut r = req();
        r.media_index = 1;
        r.part_index = 2;
        let q = parse_query(&c.transcode_start_url(&r, "s1").unwrap());
        assert_eq!(q.get("mediaIndex").map(String::as_str), Some("1"));
        assert_eq!(q.get("partIndex").map(String::as_str), Some("2"));
    }

    #[test]
    fn decision_maps_transcode_fixture() {
        let c = PlexClient::new("http://h:32400", "tok");
        let json = include_str!("../../../tests/fixtures/plex/decision_transcode.json");
        let decision: PlexDecisionResponse = serde_json::from_str(json).unwrap();
        let pd = c.decision_to_playback(&req(), "sess1", decision).unwrap();
        assert_eq!(pd.kind, PlaybackDecisionKind::Transcode);
        assert_eq!(pd.session.as_deref(), Some("sess1"));
        assert!(pd.url.contains("/transcode/universal/start.m3u8"));
        // Output metadata from the selected Media, not the request.
        assert_eq!(pd.video_resolution.as_deref(), Some("720p"));
        assert_eq!(pd.video_bitrate_kbps, Some(1877));
    }

    #[test]
    fn decision_maps_directplay_fixture_to_raw_url() {
        let c = PlexClient::new("http://h:32400", "tok");
        let json = include_str!("../../../tests/fixtures/plex/decision_directplay.json");
        let decision: PlexDecisionResponse = serde_json::from_str(json).unwrap();
        let pd = c.decision_to_playback(&req(), "sess1", decision).unwrap();
        assert_eq!(pd.kind, PlaybackDecisionKind::DirectPlay);
        assert_eq!(pd.session, None);
        assert!(pd.url.contains("download=1"));
        assert!(pd.url.contains(&req().part_key));
    }

    #[test]
    fn decision_maps_copy_fixture_to_direct_stream() {
        let c = PlexClient::new("http://h:32400", "tok");
        let json = include_str!("../../../tests/fixtures/plex/decision_copy.json");
        let decision: PlexDecisionResponse = serde_json::from_str(json).unwrap();
        let pd = c.decision_to_playback(&req(), "sess1", decision).unwrap();
        assert_eq!(pd.kind, PlaybackDecisionKind::DirectStream);
        assert!(pd.url.contains("start.m3u8"));
    }

    #[test]
    fn decision_exposes_audio_and_subtitle_tracks() {
        // AE6: the decision carries the annotated source streams with Plex ids
        // and selection, for the transcode-aware track menus.
        let c = PlexClient::new("http://h:32400", "tok");
        let json = include_str!("../../../tests/fixtures/plex/decision_transcode.json");
        let decision: PlexDecisionResponse = serde_json::from_str(json).unwrap();
        let pd = c.decision_to_playback(&req(), "sess1", decision).unwrap();
        assert_eq!(pd.audio_streams.len(), 1);
        assert_eq!(pd.audio_streams[0].id, 3799553);
        assert!(pd.audio_streams[0].selected);
        assert_eq!(pd.audio_streams[0].label, "English (EAC3 5.1)");
        assert_eq!(pd.subtitle_streams.len(), 1);
        assert_eq!(pd.subtitle_streams[0].id, 3799555);
        assert_eq!(pd.subtitle_streams[0].label, "English (SRT)");
        // Video streams (type 1) are not offered as audio/subtitle tracks.
        assert!(pd.audio_streams.iter().all(|s| s.id != 3799552));
    }

    #[test]
    fn decision_streams_skips_idless_and_filters_by_type() {
        let streams = vec![
            PlexStream {
                id: Some(10),
                stream_type: Some(2),
                extended_display_title: Some("English (AAC)".into()),
                selected: Some(true),
                ..Default::default()
            },
            PlexStream {
                id: None, // no id → cannot be re-requested → skipped
                stream_type: Some(2),
                extended_display_title: Some("French".into()),
                ..Default::default()
            },
            PlexStream {
                id: Some(20),
                stream_type: Some(3), // subtitle, not audio
                ..Default::default()
            },
        ];
        let audio = decision_streams(&streams, 2);
        assert_eq!(audio.len(), 1);
        assert_eq!(audio[0].id, 10);
        assert!(audio[0].selected);
    }

    #[test]
    fn decision_without_decision_field_is_loud_error() {
        let c = PlexClient::new("http://h:32400", "tok");
        // A decision body with a Part but no `decision` field and no container
        // decision code to fall back on.
        let json = r#"{"MediaContainer":{"Metadata":[{"ratingKey":"1","title":"x","type":"movie",
            "Media":[{"Part":[{"key":"/p/1"}]}]}]}}"#;
        let decision: PlexDecisionResponse = serde_json::from_str(json).unwrap();
        let err = c.decision_to_playback(&req(), "s", decision).unwrap_err();
        assert!(matches!(err, TranscodeError::NoDecision));
    }

    #[test]
    fn missing_part_decision_falls_back_to_general_decision_code() {
        let c = PlexClient::new("http://h:32400", "tok");
        // Some Plex server versions omit the per-Part `decision` but set the
        // container `generalDecisionCode` to 1000 ("Direct play OK").
        let json = r#"{"MediaContainer":{"generalDecisionCode":1000,
            "Metadata":[{"ratingKey":"1","title":"x","type":"movie",
            "Media":[{"Part":[{"key":"/p/1"}]}]}]}}"#;
        let decision: PlexDecisionResponse = serde_json::from_str(json).unwrap();
        let pd = c.decision_to_playback(&req(), "s", decision).unwrap();
        assert!(matches!(pd.kind, PlaybackDecisionKind::DirectPlay));
        assert!(pd.session.is_none());
    }

    #[test]
    fn missing_part_decision_with_conversion_code_is_loud_error() {
        let c = PlexClient::new("http://h:32400", "tok");
        // generalDecisionCode 1001 == conversion needed, but with no per-Part
        // decision we can't tell copy from transcode — stay loud.
        let json = r#"{"MediaContainer":{"generalDecisionCode":1001,
            "Metadata":[{"ratingKey":"1","title":"x","type":"movie",
            "Media":[{"Part":[{"key":"/p/1"}]}]}]}}"#;
        let decision: PlexDecisionResponse = serde_json::from_str(json).unwrap();
        let err = c.decision_to_playback(&req(), "s", decision).unwrap_err();
        assert!(matches!(err, TranscodeError::NoDecision));
    }
}
