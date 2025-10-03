use anyhow::{Result, anyhow};

use super::client::PlexApi;
use super::types::*;
use crate::models::{QualityOption, Resolution, StreamInfo};

impl PlexApi {
    pub async fn get_stream_url(&self, media_id: &str) -> Result<StreamInfo> {
        // For Plex, we can usually direct play
        // This is a simplified version - real implementation would check transcoding needs
        let url = self.build_url(&format!("/library/metadata/{}", media_id));

        let response = self
            .client
            .get(&url)
            .headers(self.standard_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get media info: {}", response.status()));
        }

        // Get response text first so we can provide context on error
        let response_text = response.text().await?;

        // Try to parse the response and provide better error context
        let plex_response: PlexMediaResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                tracing::error!("Failed to decode Plex response: {}", e);
                tracing::error!("Response was: {}", response_text);
                anyhow!("Failed to decode Plex stream response: {}", e)
            })?;

        if let Some(metadata) = plex_response.media_container.metadata.first()
            && let Some(media) = metadata.media.first()
            && let Some(part) = media.parts.as_ref().and_then(|p| p.first())
        {
            // Properly construct the stream URL with authentication
            // The part.key already contains the full path with session parameters
            let stream_url = if part.key.starts_with("http://") || part.key.starts_with("https://")
            {
                // part.key is a full URL, use it directly
                part.key.clone()
            } else {
                // part.key is a path, combine with base URL
                let separator = if part.key.contains('?') { "&" } else { "?" };
                format!(
                    "{}{}{}X-Plex-Token={}",
                    self.base_url, part.key, separator, self.auth_token
                )
            };

            tracing::debug!("Stream URL constructed: {}", stream_url);

            // Generate quality options for transcoding
            let mut quality_options = Vec::new();

            // Add original quality (direct play)
            let original_bitrate = media.bitrate.unwrap_or(0);
            let original_width = media.width.unwrap_or(1920);
            let original_height = media.height.unwrap_or(1080);

            quality_options.push(QualityOption {
                name: format!("Original ({}p)", original_height),
                resolution: Resolution {
                    width: original_width,
                    height: original_height,
                },
                bitrate: original_bitrate,
                url: stream_url.clone(),
                requires_transcode: false,
            });

            // Add transcoding options
            // Note: URLs will be generated on-demand via decision endpoint
            let transcode_qualities = vec![
                ("1080p", 1920, 1080, 8000000),
                ("720p", 1280, 720, 4000000),
                ("480p", 854, 480, 2000000),
                ("360p", 640, 360, 1000000),
            ];

            for (name, width, height, bitrate) in transcode_qualities {
                // Only add qualities lower than original
                if height < original_height {
                    quality_options.push(QualityOption {
                        name: name.to_string(),
                        resolution: Resolution { width, height },
                        bitrate: bitrate as u64,
                        url: String::new(), // Generated on-demand via get_stream_url_for_quality()
                        requires_transcode: true,
                    });
                }
            }

            return Ok(StreamInfo {
                url: stream_url,
                direct_play: true,
                video_codec: media.video_codec.clone().unwrap_or_default(),
                audio_codec: media.audio_codec.clone().unwrap_or_default(),
                container: part.container.clone().unwrap_or_default(),
                bitrate: original_bitrate,
                resolution: Resolution {
                    width: original_width,
                    height: original_height,
                },
                quality_options,
            });
        }

        // Provide more detailed error information
        tracing::error!(
            "Failed to get stream info for media {}: metadata={:?}, container={:?}",
            media_id,
            plex_response.media_container.metadata.is_empty(),
            plex_response
                .media_container
                .metadata
                .first()
                .map(|m| m.media.is_empty())
        );

        Err(anyhow!(
            "Failed to get stream info for media {}. Response parsed but missing required media/parts data. Metadata count: {}",
            media_id,
            plex_response.media_container.metadata.len()
        ))
    }

    /// Get stream URL for a specific quality option
    /// Routes to direct URL for original quality or decision endpoint for transcoded qualities
    pub async fn get_stream_url_for_quality(
        &self,
        media_id: &str,
        quality: &QualityOption,
        is_local: bool,
    ) -> Result<String> {
        if quality.requires_transcode {
            // Use decision endpoint for transcoded streams
            let stream_info = self
                .get_stream_url_via_decision(
                    media_id,
                    false,                        // not direct play
                    Some(quality.bitrate / 1000), // convert to kbps
                    Some(quality.resolution.clone()),
                    is_local,
                )
                .await?;

            Ok(stream_info.url)
        } else {
            // Use direct URL for original quality
            Ok(quality.url.clone())
        }
    }
}
