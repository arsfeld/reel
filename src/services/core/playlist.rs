use crate::db::connection::DatabaseConnection;
use crate::db::repository::{MediaRepository, MediaRepositoryImpl, Repository};
use crate::models::{EpisodeInfo, MediaItemId, PlaylistContext, ShowId};
use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, error, info};

/// Service for managing playlist contexts and episode navigation
pub struct PlaylistService;

impl PlaylistService {
    /// Build playlist context for a TV show episode
    pub async fn build_show_context(
        db: &DatabaseConnection,
        episode_id: &MediaItemId,
    ) -> Result<PlaylistContext> {
        let repo = MediaRepositoryImpl::new(db.clone());

        // Get the current episode details
        let current_episode = repo
            .find_by_id(episode_id.as_ref())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Episode not found: {}", episode_id))?;

        // Make sure it's an episode
        if current_episode.media_type != "episode" {
            // Not an episode, return single item context
            return Ok(PlaylistContext::SingleItem);
        }

        // Get the parent show ID
        let show_id = current_episode
            .parent_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Episode has no parent show"))?;

        // Get the show details
        let show = repo
            .find_by_id(show_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Parent show not found: {}", show_id))?;

        // Get all episodes for the show
        let episodes = repo.find_episode_playlist(show_id).await?;

        if episodes.is_empty() {
            // No episodes found, return single item context
            return Ok(PlaylistContext::SingleItem);
        }

        // Convert episodes to EpisodeInfo
        let episode_infos: Vec<EpisodeInfo> = episodes
            .into_iter()
            .map(|ep| EpisodeInfo {
                id: MediaItemId::new(&ep.id),
                title: ep.title.clone(),
                season_number: ep.season_number.unwrap_or(0) as u32,
                episode_number: ep.episode_number.unwrap_or(0) as u32,
                duration_ms: ep.duration_ms,
                watched: false,             // TODO: Get from playback_progress
                playback_position_ms: None, // TODO: Get from playback_progress
            })
            .collect();

        // Find the current episode index
        let current_index = episode_infos
            .iter()
            .position(|e| &e.id == episode_id)
            .unwrap_or(0);

        info!(
            "Built playlist context for show '{}' with {} episodes, current index: {}",
            show.title,
            episode_infos.len(),
            current_index
        );

        Ok(PlaylistContext::TvShow {
            show_id: ShowId::new(show_id),
            show_title: show.title,
            current_index,
            episodes: episode_infos,
            auto_play_next: true, // TODO: Get from user preferences
        })
    }

    /// Get next item in playlist
    pub async fn get_next_item(
        db: &DatabaseConnection,
        current_id: &MediaItemId,
        context: &PlaylistContext,
    ) -> Result<Option<MediaItemId>> {
        match context {
            PlaylistContext::SingleItem => Ok(None),
            PlaylistContext::TvShow { .. } => {
                // The context already knows the next item
                Ok(context.get_next_item())
            }
        }
    }

    /// Get previous item in playlist
    pub async fn get_previous_item(
        db: &DatabaseConnection,
        current_id: &MediaItemId,
        context: &PlaylistContext,
    ) -> Result<Option<MediaItemId>> {
        match context {
            PlaylistContext::SingleItem => Ok(None),
            PlaylistContext::TvShow { .. } => {
                // The context already knows the previous item
                Ok(context.get_previous_item())
            }
        }
    }

    /// Get next unwatched episode for continue watching
    pub async fn get_next_unwatched_episode(
        db: &DatabaseConnection,
        show_id: &MediaItemId,
        after_season: i32,
        after_episode: i32,
    ) -> Result<Option<MediaItemId>> {
        let repo = MediaRepositoryImpl::new(db.clone());

        let next_episode = repo
            .find_next_unwatched_episode(show_id.as_ref(), after_season, after_episode)
            .await?;

        Ok(next_episode.map(|ep| MediaItemId::new(&ep.id)))
    }

    /// Check if an episode is near completion (>90% watched)
    pub fn is_episode_near_completion(position_ms: i64, duration_ms: i64) -> bool {
        if duration_ms <= 0 {
            return false;
        }

        let progress_percent = (position_ms as f64 / duration_ms as f64) * 100.0;
        progress_percent >= 90.0
    }

    /// Calculate remaining time until episode ends
    pub fn calculate_remaining_time_ms(position_ms: i64, duration_ms: i64) -> i64 {
        if duration_ms <= position_ms {
            0
        } else {
            duration_ms - position_ms
        }
    }
}
