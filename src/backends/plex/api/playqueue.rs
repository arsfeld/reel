use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, warn};

use super::client::PlexApi;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlayQueueResponse {
    pub media_container: PlayQueueContainer,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayQueueContainer {
    #[serde(rename = "playQueueID")]
    pub play_queue_id: Option<i64>,
    #[serde(rename = "playQueueVersion")]
    pub play_queue_version: Option<i32>,
    #[serde(rename = "playQueueSelectedItemID")]
    pub play_queue_selected_item_id: Option<i64>,
    #[serde(rename = "playQueueSelectedItemOffset")]
    pub play_queue_selected_item_offset: Option<i32>,
    #[serde(rename = "playQueueTotalCount")]
    pub play_queue_total_count: Option<i32>,
    #[serde(rename = "playQueueShuffled")]
    pub play_queue_shuffled: Option<bool>,
    #[serde(rename = "playQueueSourceURI")]
    pub play_queue_source_uri: Option<String>,
    #[serde(rename = "Metadata", default)]
    pub metadata: Vec<PlayQueueItem>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayQueueItem {
    pub rating_key: String,
    #[serde(rename = "playQueueItemID")]
    pub play_queue_item_id: i64,
    pub title: String,
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(default)]
    pub view_offset: Option<i64>,
}

impl PlexApi {
    /// Create a new PlayQueue from a media item
    pub async fn create_play_queue(
        &self,
        media_id: &str,
        media_type: &str,
    ) -> Result<PlayQueueResponse> {
        let url = self.build_url("/playQueues");

        // Get machine identifier
        let machine_id = self.get_machine_id().await?;

        // Construct the URI for the media item
        let uri = format!(
            "server://{}/com.plexapp.plugins.library/library/metadata/{}",
            machine_id, media_id
        );

        debug!(
            "Creating PlayQueue for media_id: {}, type: {}, uri: {}",
            media_id, media_type, uri
        );

        // Map media type to Plex type number
        let type_num = match media_type.to_lowercase().as_str() {
            "movie" => "video",
            "episode" => "video",
            "track" => "audio",
            _ => "video",
        };

        let response = self
            .client
            .post(&url)
            .headers(self.standard_headers())
            .query(&[
                ("type", type_num),
                ("uri", &uri),
                ("continuous", "1"), // Enable continuous playback for episodes
                ("repeat", "0"),
                ("includeChapters", "1"),
                ("includeRelated", "1"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to create PlayQueue: {} - {}", status, text));
        }

        let play_queue: PlayQueueResponse = response.json().await?;

        if let Some(id) = play_queue.media_container.play_queue_id {
            debug!("Created PlayQueue with ID: {}", id);
        }

        Ok(play_queue)
    }

    /// Create a PlayQueue from a playlist
    pub async fn create_play_queue_from_playlist(
        &self,
        playlist_id: &str,
    ) -> Result<PlayQueueResponse> {
        let url = self.build_url("/playQueues");

        // Get machine identifier
        let machine_id = self.get_machine_id().await?;

        let playlist_key = format!("/playlists/{}", playlist_id);
        let uri = format!(
            "server://{}/com.plexapp.plugins.library{}",
            machine_id, playlist_key
        );

        debug!("Creating PlayQueue from playlist: {}", playlist_id);

        let response = self
            .client
            .post(&url)
            .headers(self.standard_headers())
            .query(&[
                ("type", "video"),
                ("playlistID", playlist_id),
                ("uri", &uri),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Failed to create PlayQueue from playlist: {} - {}",
                status,
                text
            ));
        }

        response.json().await.map_err(Into::into)
    }

    /// Retrieve an existing PlayQueue
    pub async fn get_play_queue(&self, play_queue_id: i64) -> Result<PlayQueueResponse> {
        let url = self.build_url(&format!("/playQueues/{}", play_queue_id));

        debug!("Retrieving PlayQueue: {}", play_queue_id);

        let response = self
            .client
            .get(&url)
            .headers(self.standard_headers())
            .query(&[
                ("own", "1"), // Take ownership of the queue
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Failed to retrieve PlayQueue {}: {} - {}",
                play_queue_id,
                status,
                text
            ));
        }

        response.json().await.map_err(Into::into)
    }

    /// Add item to PlayQueue
    pub async fn add_to_play_queue(
        &self,
        play_queue_id: i64,
        media_id: &str,
    ) -> Result<PlayQueueResponse> {
        let url = self.build_url(&format!("/playQueues/{}", play_queue_id));

        // Get machine identifier
        let machine_id = self.get_machine_id().await?;

        let uri = format!(
            "server://{}/com.plexapp.plugins.library/library/metadata/{}",
            machine_id, media_id
        );

        debug!("Adding media {} to PlayQueue {}", media_id, play_queue_id);

        let response = self
            .client
            .put(&url)
            .headers(self.standard_headers())
            .query(&[("uri", uri.as_str())])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to add to PlayQueue: {} - {}", status, text));
        }

        response.json().await.map_err(Into::into)
    }

    /// Remove item from PlayQueue
    pub async fn remove_from_play_queue(
        &self,
        play_queue_id: i64,
        play_queue_item_id: i64,
    ) -> Result<()> {
        let url = self.build_url(&format!(
            "/playQueues/{}/items/{}",
            play_queue_id, play_queue_item_id
        ));

        debug!(
            "Removing item {} from PlayQueue {}",
            play_queue_item_id, play_queue_id
        );

        let response = self
            .client
            .delete(&url)
            .headers(self.standard_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Failed to remove from PlayQueue: {} - {}",
                status,
                text
            ));
        }

        Ok(())
    }

    /// Move item in PlayQueue
    pub async fn move_play_queue_item(
        &self,
        play_queue_id: i64,
        play_queue_item_id: i64,
        after_item_id: Option<i64>,
    ) -> Result<PlayQueueResponse> {
        let url = self.build_url(&format!(
            "/playQueues/{}/items/{}/move",
            play_queue_id, play_queue_item_id
        ));

        debug!(
            "Moving item {} in PlayQueue {} after {:?}",
            play_queue_item_id, play_queue_id, after_item_id
        );

        let mut query_params = vec![];
        if let Some(after_id) = after_item_id {
            query_params.push(("after", after_id.to_string()));
        }

        let response = self
            .client
            .put(&url)
            .headers(self.standard_headers())
            .query(&query_params)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Failed to move item in PlayQueue: {} - {}",
                status,
                text
            ));
        }

        response.json().await.map_err(Into::into)
    }

    /// Update progress using PlayQueue
    pub async fn update_play_queue_progress(
        &self,
        play_queue_id: i64,
        play_queue_item_id: i64,
        media_id: &str,
        position: Duration,
        duration: Duration,
        state: &str,
    ) -> Result<()> {
        // Use the timeline endpoint but with PlayQueue context
        let timeline_url = self.build_url("/:/timeline");

        let position_ms = position.as_millis() as u64;
        let duration_ms = duration.as_millis() as u64;

        debug!(
            "Updating PlayQueue progress - queue_id: {}, item_id: {}, media_id: {}, position: {}ms",
            play_queue_id, play_queue_item_id, media_id, position_ms
        );

        // If we're more than 90% through, mark as watched
        if duration_ms > 0 && position_ms > (duration_ms * 9 / 10) {
            debug!("Position is >90% of duration, marking as watched");
            return self.mark_watched(media_id).await;
        }

        let response = self
            .client
            .get(&timeline_url)
            .headers(self.standard_headers())
            .query(&[
                ("ratingKey", media_id),
                ("key", &format!("/library/metadata/{}", media_id)),
                ("playQueueID", &play_queue_id.to_string()),
                ("playQueueVersion", "1"), // We should track this
                ("playQueueItemID", &play_queue_item_id.to_string()),
                ("state", state),
                ("time", &position_ms.to_string()),
                ("duration", &duration_ms.to_string()),
                ("playbackTime", &position_ms.to_string()),
                ("identifier", "com.plexapp.plugins.library"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            warn!("PlayQueue timeline update failed: {} - {}", status, text);

            // Fall back to regular timeline update
            return self
                .update_progress_with_state(media_id, position, duration, state)
                .await;
        }

        debug!("PlayQueue timeline update successful");
        Ok(())
    }
}
