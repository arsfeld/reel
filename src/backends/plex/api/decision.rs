use anyhow::{Result, anyhow};
use serde::Deserialize;

use super::client::PlexApi;
use crate::models::{Resolution, StreamInfo};

#[derive(Debug, Deserialize)]
struct DecisionResponse {
    #[serde(rename = "MediaContainer")]
    media_container: DecisionMediaContainer,
}

#[derive(Debug, Deserialize)]
struct DecisionMediaContainer {
    #[serde(default)]
    #[serde(rename = "Metadata")]
    metadata: Vec<DecisionMetadata>,
}

#[derive(Debug, Deserialize)]
struct DecisionMetadata {
    #[serde(rename = "Media", default)]
    media: Vec<DecisionMedia>,
}

#[derive(Debug, Deserialize)]
struct DecisionMedia {
    #[serde(rename = "Part", default)]
    parts: Vec<DecisionPart>,

    #[serde(rename = "videoDecision")]
    video_decision: Option<String>,

    #[serde(rename = "audioDecision")]
    audio_decision: Option<String>,

    #[serde(rename = "protocol")]
    protocol: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DecisionPart {
    #[serde(rename = "key")]
    key: String,

    #[serde(rename = "decision")]
    decision: Option<String>,
}

impl PlexApi {
    /// Get stream URL via decision endpoint (for remote connections or transcoding)
    pub async fn get_stream_url_via_decision(
        &self,
        media_id: &str,
        direct_play: bool,
        max_bitrate_kbps: Option<u64>,
        resolution: Option<Resolution>,
        is_local: bool,
    ) -> Result<StreamInfo> {
        let path = format!("/library/metadata/{}", media_id);

        // Build query parameters
        let mut params = vec![
            ("path", path.as_str()),
            ("mediaIndex", "0"),
            ("partIndex", "0"),
            ("protocol", "http"),
            ("hasMDE", "1"),
            ("location", if is_local { "lan" } else { "wan" }),
        ];

        // Direct play settings
        let direct_play_str = if direct_play { "1" } else { "0" };
        params.push(("directPlay", direct_play_str));
        params.push(("directStream", direct_play_str));

        // Quality parameters (for transcoding)
        let bitrate_str;
        let resolution_str;
        if !direct_play {
            if let Some(bitrate) = max_bitrate_kbps {
                bitrate_str = bitrate.to_string();
                params.push(("maxVideoBitrate", &bitrate_str));
            }

            if let Some(ref res) = resolution {
                resolution_str = format!("{}x{}", res.width, res.height);
                params.push(("videoResolution", &resolution_str));
            }

            params.push(("fastSeek", "1"));
        }

        // Make decision request
        let url = self.build_url("/video/:/transcode/universal/decision");

        let response = self
            .client
            .get(&url)
            .query(&params)
            .headers(self.standard_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Decision endpoint failed: {}", response.status()));
        }

        let decision_response: DecisionResponse = response.json().await?;

        // Extract stream URL from decision response
        if let Some(metadata) = decision_response.media_container.metadata.first()
            && let Some(media) = metadata.media.first()
            && let Some(part) = media.parts.first()
        {
            // Log the decision made by Plex
            let video_decision = media.video_decision.as_deref().unwrap_or("unknown");
            let audio_decision = media.audio_decision.as_deref().unwrap_or("unknown");
            let protocol = media.protocol.as_deref().unwrap_or("unknown");

            tracing::info!(
                "Plex decision endpoint response - video: {}, audio: {}, protocol: {}, location: {}",
                video_decision,
                audio_decision,
                protocol,
                if is_local { "lan" } else { "wan" }
            );

            // Construct the full stream URL
            let stream_url = if part.key.starts_with("http://") || part.key.starts_with("https://")
            {
                part.key.clone()
            } else {
                let separator = if part.key.contains('?') { "&" } else { "?" };
                format!(
                    "{}{}{}X-Plex-Token={}",
                    self.base_url, part.key, separator, self.auth_token
                )
            };

            tracing::debug!("Decision endpoint stream URL: {}", stream_url);

            // Build StreamInfo from decision
            Ok(StreamInfo {
                url: stream_url,
                direct_play,
                video_codec: String::new(), // Would need to fetch from metadata
                audio_codec: String::new(),
                container: String::new(),
                bitrate: max_bitrate_kbps.unwrap_or(0) * 1000,
                resolution: resolution.unwrap_or(Resolution {
                    width: 1920,
                    height: 1080,
                }),
                quality_options: Vec::new(), // Populated by get_stream_url
            })
        } else {
            Err(anyhow!("Invalid decision response structure"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_response_parsing() {
        let json_response = r#"{
            "MediaContainer": {
                "Metadata": [{
                    "Media": [{
                        "Part": [{
                            "key": "/library/parts/123/file.mp4",
                            "decision": "directplay"
                        }],
                        "videoDecision": "directplay",
                        "audioDecision": "directplay",
                        "protocol": "http"
                    }]
                }]
            }
        }"#;

        let result: Result<DecisionResponse, _> = serde_json::from_str(json_response);
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.media_container.metadata.len(), 1);
        assert_eq!(response.media_container.metadata[0].media.len(), 1);
        assert_eq!(response.media_container.metadata[0].media[0].parts.len(), 1);
        assert_eq!(
            response.media_container.metadata[0].media[0].parts[0].key,
            "/library/parts/123/file.mp4"
        );
    }

    #[test]
    fn test_decision_response_transcode_parsing() {
        let json_response = r#"{
            "MediaContainer": {
                "Metadata": [{
                    "Media": [{
                        "Part": [{
                            "key": "/video/:/transcode/universal/start.m3u8",
                            "decision": "transcode"
                        }],
                        "videoDecision": "transcode",
                        "audioDecision": "copy",
                        "protocol": "hls"
                    }]
                }]
            }
        }"#;

        let result: Result<DecisionResponse, _> = serde_json::from_str(json_response);
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(
            response.media_container.metadata[0].media[0].video_decision,
            Some("transcode".to_string())
        );
        assert_eq!(
            response.media_container.metadata[0].media[0].audio_decision,
            Some("copy".to_string())
        );
        assert_eq!(
            response.media_container.metadata[0].media[0].protocol,
            Some("hls".to_string())
        );
    }
}
