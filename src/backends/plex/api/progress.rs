use anyhow::{Result, anyhow};
use std::time::Duration;
use tracing::{debug, warn};

use super::client::PlexApi;

impl PlexApi {
    /// Update playback progress
    /// Note: state should be "playing" for active playback or "paused" when paused
    pub async fn update_progress(
        &self,
        media_id: &str,
        position: Duration,
        duration: Duration,
    ) -> Result<()> {
        self.update_progress_with_state(media_id, position, duration, "playing")
            .await
    }

    /// Update playback progress with explicit state
    pub async fn update_progress_with_state(
        &self,
        media_id: &str,
        position: Duration,
        duration: Duration,
        state: &str,
    ) -> Result<()> {
        // For simple progress tracking without a playQueue, we can update the viewOffset directly
        // by "scrobbling" with the current position
        let position_ms = position.as_millis() as u64;

        // If we're more than 90% through, mark as watched
        let duration_ms = duration.as_millis() as u64;
        if duration_ms > 0 && position_ms > (duration_ms * 9 / 10) {
            debug!("Position is >90% of duration, marking as watched");
            return self.mark_watched(media_id).await;
        }

        // Otherwise update the viewOffset using timeline endpoint with proper headers
        // The timeline endpoint is more reliable for position updates
        let timeline_url = self.build_url("/:/timeline");

        debug!(
            "Updating progress via timeline - media_id: {}, position: {}ms",
            media_id, position_ms
        );

        let response = self
            .client
            .get(&timeline_url)
            .headers(self.standard_headers())
            .query(&[
                ("ratingKey", media_id),
                ("key", &format!("/library/metadata/{}", media_id)),
                ("identifier", "com.plexapp.plugins.library"),
                ("state", state),
                ("time", &position_ms.to_string()),
                ("duration", &duration_ms.to_string()),
                ("playbackTime", &position_ms.to_string()),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            debug!("Timeline update response: {} - {}", status, text);
            // Timeline endpoint often returns 200 with empty response, which is OK
            if status != 200 {
                return Err(anyhow!("Failed to update progress: {}", status));
            }
        } else {
            debug!("Timeline update successful for media_id: {}", media_id);
        }

        Ok(())
    }

    /// Mark media as watched
    pub async fn mark_watched(&self, media_id: &str) -> Result<()> {
        let url = self.build_url("/:/scrobble");

        let response = self
            .client
            .get(&url)
            .headers(self.standard_headers())
            .query(&[
                ("key", media_id),
                ("identifier", "com.plexapp.plugins.library"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to mark as watched: {}", response.status()));
        }

        Ok(())
    }

    /// Mark media as unwatched
    pub async fn mark_unwatched(&self, media_id: &str) -> Result<()> {
        let url = self.build_url("/:/unscrobble");

        let response = self
            .client
            .get(&url)
            .headers(self.standard_headers())
            .query(&[
                ("key", media_id),
                ("identifier", "com.plexapp.plugins.library"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to mark as unwatched: {}",
                response.status()
            ));
        }

        Ok(())
    }
}
