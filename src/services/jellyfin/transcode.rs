//! Jellyfin transcode-decision client (U3). The analog of Plex's `/decision`
//! flow: `POST /Items/{id}/PlaybackInfo` with a `DeviceProfile` body returns the
//! server's playability verdict, a streaming URL, the per-stream list, and the
//! `PlaySessionId`. We map that into the shared [`PlaybackDecision`] so the
//! existing quality menu, decision indicator, keepalive, and track menus drive
//! Jellyfin unchanged.
//!
//! No GStreamer/GTK imports (services layer is pure Rust). The decision call
//! uses a single-attempt short timeout (KTD4c) rather than the retrying
//! `request_factory` loop, because it sits on the play hot-path.

use std::time::Duration;

use serde_json::json;

use crate::models::playback::{
    DecisionStream, PlaybackDecision, PlaybackDecisionKind, PlaybackRequest,
};
use crate::services::media_source::SourceError;

use super::api::JellyfinClient;
use super::error::JellyfinTranscodeError;
use super::models::{MediaSourceInfo, MediaStream, PlaybackInfoResponse};
use super::transcode_profile::device_profile;

/// Single-attempt decision timeout. Deliberately short — the call blocks the
/// Play action, so a slow server should fail fast to the caller's fallback
/// rather than retry (KTD4c).
const DECISION_TIMEOUT: Duration = Duration::from_secs(5);

impl From<JellyfinTranscodeError> for SourceError {
    fn from(e: JellyfinTranscodeError) -> Self {
        match e {
            // Retryable / fallback-to-direct-play conditions.
            JellyfinTranscodeError::Timeout | JellyfinTranscodeError::Request(_) => {
                SourceError::Connection(e.to_string())
            }
            // Loud conditions the user should see.
            JellyfinTranscodeError::Server { .. }
            | JellyfinTranscodeError::Parse(_)
            | JellyfinTranscodeError::NoDecision => SourceError::Other(e.to_string()),
        }
    }
}

impl JellyfinClient {
    /// Resolve a playback decision via `POST /Items/{id}/PlaybackInfo`.
    pub async fn resolve_decision(
        &self,
        req: &PlaybackRequest,
    ) -> Result<PlaybackDecision, JellyfinTranscodeError> {
        // The composite part key is "{item_id}|{media_source_id}". A bare id
        // (no '|') means we don't know the source — omit MediaSourceId so the
        // server picks the default (KTD2/U3).
        let item_id = req.rating_key.as_str();
        let requested_source_id = req.part_key.split_once('|').map(|(_, src)| src.to_string());

        let (profile, max_bitrate_bps) = device_profile(req.quality, self.is_remote());
        let body = self.build_body(
            req,
            requested_source_id.as_deref(),
            profile,
            max_bitrate_bps,
        );

        let url = format!(
            "{}/Items/{}/PlaybackInfo?userId={}",
            self.base_url(),
            item_id,
            self.user_id()
        );

        let resp = self
            .http()
            .post(&url)
            .header("Content-Type", "application/json")
            .timeout(DECISION_TIMEOUT)
            .body(
                serde_json::to_vec(&body)
                    .map_err(|e| JellyfinTranscodeError::Parse(e.to_string()))?,
            )
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    JellyfinTranscodeError::Timeout
                } else {
                    JellyfinTranscodeError::Request(e.to_string())
                }
            })?;

        if !resp.status().is_success() {
            return Err(JellyfinTranscodeError::Server {
                status: resp.status().as_u16(),
            });
        }

        let info: PlaybackInfoResponse = resp
            .json()
            .await
            .map_err(|e| JellyfinTranscodeError::Parse(e.to_string()))?;

        self.decision_from_response(req, requested_source_id.as_deref(), info)
    }

    /// Build the `PlaybackInfoDto` request body.
    fn build_body(
        &self,
        req: &PlaybackRequest,
        requested_source_id: Option<&str>,
        profile: serde_json::Value,
        max_bitrate_bps: Option<u64>,
    ) -> serde_json::Value {
        let mut body = json!({
            "UserId": self.user_id(),
            "StartTimeTicks": (req.offset_secs * 10_000_000.0) as i64,
            "EnableDirectPlay": !req.force_transcode,
            "EnableDirectStream": !req.force_transcode,
            "EnableTranscoding": true,
            "DeviceProfile": profile,
        });
        let obj = body.as_object_mut().expect("json! built an object");
        if let Some(bps) = max_bitrate_bps {
            obj.insert("MaxStreamingBitrate".into(), json!(bps));
        }
        if let Some(src) = requested_source_id {
            obj.insert("MediaSourceId".into(), json!(src));
        }
        if let Some(idx) = req.audio_stream_id {
            obj.insert("AudioStreamIndex".into(), json!(idx));
        }
        if let Some(idx) = req.subtitle_stream_id {
            obj.insert("SubtitleStreamIndex".into(), json!(idx));
        }
        body
    }

    /// Map a `PlaybackInfoResponse` into a [`PlaybackDecision`].
    fn decision_from_response(
        &self,
        req: &PlaybackRequest,
        requested_source_id: Option<&str>,
        info: PlaybackInfoResponse,
    ) -> Result<PlaybackDecision, JellyfinTranscodeError> {
        // Prefer the source matching the requested id; else the first.
        let source = requested_source_id
            .and_then(|want| {
                info.media_sources
                    .iter()
                    .find(|s| s.id.as_deref() == Some(want))
            })
            .or_else(|| info.media_sources.first())
            .ok_or(JellyfinTranscodeError::NoDecision)?;

        let kind = if source.supports_direct_play {
            PlaybackDecisionKind::DirectPlay
        } else if source.supports_direct_stream {
            PlaybackDecisionKind::DirectStream
        } else if source.supports_transcoding && source.transcoding_url.is_some() {
            PlaybackDecisionKind::Transcode
        } else {
            return Err(JellyfinTranscodeError::NoDecision);
        };

        let source_id = source.id.clone().unwrap_or_default();
        let (url, session) = match kind {
            PlaybackDecisionKind::DirectPlay => {
                (self.stream_url(&req.rating_key, &source_id), None)
            }
            PlaybackDecisionKind::DirectStream | PlaybackDecisionKind::Transcode => {
                let rel = source
                    .transcoding_url
                    .as_deref()
                    .ok_or(JellyfinTranscodeError::NoDecision)?;
                (
                    self.absolute_transcode_url(rel),
                    info.play_session_id.clone(),
                )
            }
        };

        let (video_resolution, video_bitrate_kbps) = video_output(source);
        let audio_streams = decision_streams(
            source,
            "Audio",
            req.audio_stream_id,
            source.default_audio_stream_index,
        );
        let subtitle_streams = decision_streams(
            source,
            "Subtitle",
            req.subtitle_stream_id,
            source.default_subtitle_stream_index,
        );

        Ok(PlaybackDecision {
            kind,
            url,
            session,
            video_resolution,
            video_bitrate_kbps,
            // Jellyfin's PlaybackInfo gives no throttle signal at decision time.
            throttled: false,
            audio_streams,
            subtitle_streams,
        })
    }

    /// Resolve a server-returned (relative) `TranscodingUrl` to an absolute URL,
    /// guaranteeing the in-URL `api_key` GStreamer needs for segment auth
    /// (KTD2b). Refuses an absolute/cross-origin URL (SSRF guard) by treating
    /// only the leading-slash relative form as valid.
    fn absolute_transcode_url(&self, relative: &str) -> String {
        let rel = if relative.starts_with('/') {
            relative.to_string()
        } else {
            format!("/{relative}")
        };
        let mut url = format!("{}{}", self.base_url(), rel);
        if !url.contains("api_key=") && !url.contains("ApiKey=") {
            let sep = if url.contains('?') { '&' } else { '?' };
            url.push(sep);
            url.push_str("api_key=");
            url.push_str(self.token());
        }
        url
    }
}

/// Derive `(video_resolution, video_bitrate_kbps)` for the indicator from the
/// selected source: resolution as the bare height ("1080"), bitrate from the
/// source's overall bitrate (bits/sec → kbps).
fn video_output(source: &MediaSourceInfo) -> (Option<String>, Option<i64>) {
    let resolution = source
        .media_streams
        .iter()
        .find(|s| s.type_.as_deref() == Some("Video"))
        .and_then(|v| v.height)
        .map(|h| h.to_string());
    let bitrate_kbps = source.bitrate.map(|b| b / 1000);
    (resolution, bitrate_kbps)
}

/// Build the [`DecisionStream`] list for one stream type (Audio/Subtitle) from
/// the source's media streams. `requested` is the index the user asked for this
/// resolve; `default` is the source's default index — a stream is marked
/// `selected` when it matches the requested index, or the default when none was
/// requested.
fn decision_streams(
    source: &MediaSourceInfo,
    stream_type: &str,
    requested: Option<i64>,
    default: Option<i32>,
) -> Vec<DecisionStream> {
    source
        .media_streams
        .iter()
        .filter(|s| s.type_.as_deref() == Some(stream_type))
        .map(|s| {
            let id = s.index as i64;
            let selected = match requested {
                Some(want) => want == id,
                None => default.map(|d| d as i64) == Some(id),
            };
            DecisionStream {
                id,
                label: stream_label(s),
                selected,
            }
        })
        .collect()
}

/// Human-readable label for a stream, with a fallback chain.
fn stream_label(s: &MediaStream) -> String {
    if let Some(t) = s.display_title.as_deref().filter(|t| !t.is_empty()) {
        return t.to_string();
    }
    if let Some(lang) = s.language.as_deref().filter(|l| !l.is_empty()) {
        return lang.to_string();
    }
    if let Some(codec) = s.codec.as_deref().filter(|c| !c.is_empty()) {
        return codec.to_string();
    }
    format!("Track {}", s.index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::playback::QualitySelection;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn req(part_key: &str) -> PlaybackRequest {
        PlaybackRequest {
            rating_key: "item9".into(),
            part_key: part_key.into(),
            media_index: 0,
            part_index: 0,
            quality: QualitySelection::Auto,
            force_transcode: false,
            audio_stream_id: None,
            subtitle_stream_id: None,
            offset_secs: 0.0,
        }
    }

    fn client(base: &str) -> JellyfinClient {
        JellyfinClient::new(base, "secret-token", "user1", "dev1")
    }

    fn direct_play_body() -> serde_json::Value {
        json!({
            "PlaySessionId": "ps-direct",
            "MediaSources": [{
                "Id": "src1",
                "SupportsDirectPlay": true,
                "SupportsDirectStream": true,
                "SupportsTranscoding": true,
                "Bitrate": 8000000,
                "MediaStreams": [
                    {"Index": 0, "Type": "Video", "Height": 1080},
                    {"Index": 1, "Type": "Audio", "Codec": "aac", "DisplayTitle": "English (AAC)"}
                ]
            }]
        })
    }

    fn transcode_body() -> serde_json::Value {
        json!({
            "PlaySessionId": "ps-trans",
            "MediaSources": [{
                "Id": "src1",
                "SupportsDirectPlay": false,
                "SupportsDirectStream": false,
                "SupportsTranscoding": true,
                "TranscodingUrl": "/videos/item9/master.m3u8?api_key=secret-token&mediaSourceId=src1",
                "TranscodingSubProtocol": "hls",
                "Bitrate": 18000000,
                "DefaultAudioStreamIndex": 1,
                "MediaStreams": [
                    {"Index": 0, "Type": "Video", "Codec": "hevc", "Height": 2160},
                    {"Index": 1, "Type": "Audio", "Codec": "truehd", "DisplayTitle": "English (TrueHD 7.1)"},
                    {"Index": 2, "Type": "Audio", "Codec": "aac", "DisplayTitle": "Commentary"},
                    {"Index": 3, "Type": "Subtitle", "Codec": "ass", "DisplayTitle": "English"}
                ]
            }]
        })
    }

    #[tokio::test]
    async fn direct_play_maps_to_static_stream_url() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/Items/item9/PlaybackInfo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(direct_play_body()))
            .mount(&server)
            .await;
        let c = client(&server.uri());
        let d = c.resolve_decision(&req("item9|src1")).await.unwrap();
        assert_eq!(d.kind, PlaybackDecisionKind::DirectPlay);
        assert!(d.url.contains("/Videos/item9/stream?static=true"));
        assert!(d.url.contains("api_key=secret-token"));
        // Direct-play has no transcode session to keep alive.
        assert!(d.session.is_none());
    }

    #[tokio::test]
    async fn transcode_maps_absolute_url_and_session() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/Items/item9/PlaybackInfo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(transcode_body()))
            .mount(&server)
            .await;
        let c = client(&server.uri());
        let d = c.resolve_decision(&req("item9|src1")).await.unwrap();
        assert_eq!(d.kind, PlaybackDecisionKind::Transcode);
        assert_eq!(
            d.url,
            format!(
                "{}/videos/item9/master.m3u8?api_key=secret-token&mediaSourceId=src1",
                server.uri()
            )
        );
        assert_eq!(d.session.as_deref(), Some("ps-trans"));
        assert_eq!(d.video_resolution.as_deref(), Some("2160"));
    }

    #[tokio::test]
    async fn transcode_populates_audio_and_subtitle_streams() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/Items/item9/PlaybackInfo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(transcode_body()))
            .mount(&server)
            .await;
        let c = client(&server.uri());
        let d = c.resolve_decision(&req("item9|src1")).await.unwrap();
        assert_eq!(d.audio_streams.len(), 2);
        assert_eq!(d.audio_streams[0].id, 1);
        assert_eq!(d.audio_streams[0].label, "English (TrueHD 7.1)");
        // Default audio index 1 is marked selected when none requested.
        assert!(d.audio_streams[0].selected);
        assert!(!d.audio_streams[1].selected);
        assert_eq!(d.subtitle_streams.len(), 1);
        assert_eq!(d.subtitle_streams[0].id, 3);
    }

    #[tokio::test]
    async fn requested_audio_index_marks_selected() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/Items/item9/PlaybackInfo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(transcode_body()))
            .mount(&server)
            .await;
        let c = client(&server.uri());
        let mut r = req("item9|src1");
        r.audio_stream_id = Some(2);
        let d = c.resolve_decision(&r).await.unwrap();
        assert!(!d.audio_streams[0].selected);
        assert!(d.audio_streams.iter().find(|s| s.id == 2).unwrap().selected);
    }

    #[tokio::test]
    async fn no_supported_mode_is_no_decision() {
        let server = MockServer::start().await;
        let body = json!({
            "PlaySessionId": "x",
            "MediaSources": [{"Id": "src1", "SupportsDirectPlay": false, "SupportsDirectStream": false, "SupportsTranscoding": false}]
        });
        Mock::given(method("POST"))
            .and(path("/Items/item9/PlaybackInfo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        let c = client(&server.uri());
        let err = c.resolve_decision(&req("item9|src1")).await.unwrap_err();
        assert!(matches!(err, JellyfinTranscodeError::NoDecision));
    }

    #[tokio::test]
    async fn server_500_is_server_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/Items/item9/PlaybackInfo"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let c = client(&server.uri());
        let err = c.resolve_decision(&req("item9|src1")).await.unwrap_err();
        assert!(matches!(
            err,
            JellyfinTranscodeError::Server { status: 500 }
        ));
        // Loud, not retryable.
        assert!(matches!(SourceError::from(err), SourceError::Other(_)));
    }
}
