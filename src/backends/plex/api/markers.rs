use anyhow::Result;
use std::time::Duration;
use tracing::{error, info, warn};

use super::client::PlexApi;
use super::types::*;
use crate::models::{ChapterMarker, ChapterType};

impl PlexApi {
    /// Fetch intro and credit markers for any media (episode or movie)
    pub async fn fetch_episode_markers(
        &self,
        rating_key: &str,
    ) -> Result<(Option<ChapterMarker>, Option<ChapterMarker>)> {
        // Include additional parameters to ensure markers are returned
        // includeChapters=1 ensures chapter/marker data is included
        info!("Fetching markers for media ID: {}", rating_key);
        let url = self.build_url(&format!("/library/metadata/{}", rating_key));

        let response = self
            .client
            .get(&url)
            .headers(self.standard_headers())
            .query(&[
                ("includeChapters", "1"),
                ("includeMarkers", "1"),
                ("includeOnDeck", "1"),
                ("includeRelated", "1"),
                ("includeExtras", "1"),
                ("includeGeolocation", "1"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            warn!(
                "Failed to fetch markers for episode {}: {}",
                rating_key,
                response.status()
            );
            return Ok((None, None));
        }

        let response_text = response.text().await?;

        // Try to parse the response
        let data: PlexMetadataResponse = match serde_json::from_str(&response_text) {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to parse Plex metadata response: {}", e);
                return Ok((None, None));
            }
        };

        let mut intro_marker = None;
        let mut credits_marker = None;

        if let Some(metadata) = data.media_container.metadata.first() {
            if let Some(markers) = &metadata.marker {
                info!(
                    "Found {} markers for media ID: {}",
                    markers.len(),
                    rating_key
                );
                for marker in markers.iter() {
                    info!(
                        "Marker type: '{}', start: {}ms, end: {}ms",
                        marker.type_, marker.start_time_offset, marker.end_time_offset
                    );
                    match marker.type_.as_str() {
                        "intro" => {
                            intro_marker = Some(ChapterMarker {
                                start_time: Duration::from_millis(marker.start_time_offset as u64),
                                end_time: Duration::from_millis(marker.end_time_offset as u64),
                                marker_type: ChapterType::Intro,
                            });
                        }
                        "credits" => {
                            credits_marker = Some(ChapterMarker {
                                start_time: Duration::from_millis(marker.start_time_offset as u64),
                                end_time: Duration::from_millis(marker.end_time_offset as u64),
                                marker_type: ChapterType::Credits,
                            });
                        }
                        _ => {}
                    }
                }
            } else {
                info!("No markers found for media ID: {}", rating_key);
            }
        } else {
            warn!("No metadata found in response for media ID: {}", rating_key);
        }

        Ok((intro_marker, credits_marker))
    }
}
