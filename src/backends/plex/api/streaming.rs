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

        let plex_response: PlexMediaResponse = response.json().await?;

        if let Some(metadata) = plex_response.media_container.metadata.first()
            && let Some(media) = metadata.media.first()
            && let Some(part) = media.parts.as_ref().and_then(|p| p.first())
        {
            let stream_url = format!(
                "{}{}?X-Plex-Token={}",
                self.base_url, part.key, self.auth_token
            );

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
            let transcode_qualities = vec![
                ("1080p", 1920, 1080, 8000000),
                ("720p", 1280, 720, 4000000),
                ("480p", 854, 480, 2000000),
                ("360p", 640, 360, 1000000),
            ];

            for (name, width, height, bitrate) in transcode_qualities {
                // Only add qualities lower than original
                if height < original_height {
                    let path = format!("/library/metadata/{}", media_id);
                    let transcode_url = format!(
                        "{}/video/:/transcode/universal/start.m3u8?path={}&mediaIndex=0&partIndex=0&protocol=hls&directPlay=0&directStream=0&fastSeek=1&maxVideoBitrate={}&videoResolution={}x{}&X-Plex-Token={}",
                        self.base_url,
                        percent_encoding::utf8_percent_encode(
                            &path,
                            percent_encoding::NON_ALPHANUMERIC
                        )
                        .to_string(),
                        bitrate / 1000, // Convert to kbps
                        width,
                        height,
                        self.auth_token
                    );

                    quality_options.push(QualityOption {
                        name: name.to_string(),
                        resolution: Resolution { width, height },
                        bitrate: bitrate as u64,
                        url: transcode_url,
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

        Err(anyhow!("Failed to get stream info for media"))
    }
}
