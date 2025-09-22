use crate::backends::plex::PlexBackend;
use crate::backends::plex::api::playqueue::{PlayQueueContainer, PlayQueueItem, PlayQueueResponse};
use crate::db::connection::DatabaseConnection;
use crate::db::repository::{
    MediaRepository, MediaRepositoryImpl, PlaybackRepository, PlaybackRepositoryImpl, Repository,
};
use crate::models::{EpisodeInfo, MediaItemId, PlayQueueInfo, PlaylistContext, QueueItem, ShowId};
use anyhow::{Result, anyhow};
use std::any::Any;
use tracing::{debug, error, info, warn};

/// Service for managing Plex PlayQueue contexts
pub struct PlayQueueService;

impl PlayQueueService {
    /// Create a PlayQueue context from a media item
    pub async fn create_from_media(
        backend: &dyn Any,
        db: &DatabaseConnection,
        media_id: &MediaItemId,
        media_type: &str,
    ) -> Result<PlaylistContext> {
        // Try to downcast to PlexBackend
        if let Some(plex_backend) = backend.downcast_ref::<PlexBackend>() {
            if let Some(api) = plex_backend.get_api_for_playqueue().await {
                debug!(
                    "Creating PlayQueue for media_id: {}, type: {}",
                    media_id, media_type
                );

                // Create the PlayQueue on the Plex server
                let play_queue_response =
                    api.create_play_queue(media_id.as_ref(), media_type).await?;

                // Convert the response to a PlaylistContext
                return Self::build_context_from_response(
                    db,
                    play_queue_response,
                    media_id,
                    media_type,
                )
                .await;
            }
        }

        // Fall back to regular playlist context for non-Plex or if API not available
        warn!("PlayQueue creation not available, falling back to standard context");
        PlaylistService::build_local_show_context(db, media_id).await
    }

    /// Create a PlayQueue context from a playlist
    pub async fn create_from_playlist(
        backend: &dyn Any,
        db: &DatabaseConnection,
        playlist_id: &str,
    ) -> Result<PlaylistContext> {
        // Try to downcast to PlexBackend
        if let Some(plex_backend) = backend.downcast_ref::<PlexBackend>() {
            if let Some(api) = plex_backend.get_api_for_playqueue().await {
                debug!("Creating PlayQueue from playlist: {}", playlist_id);

                // Create the PlayQueue on the Plex server
                let play_queue_response = api.create_play_queue_from_playlist(playlist_id).await?;

                // Build generic PlayQueue context
                return Self::build_playqueue_context(db, play_queue_response).await;
            }
        }

        Err(anyhow!(
            "PlayQueue creation from playlist not available for this backend"
        ))
    }

    /// Retrieve an existing PlayQueue by ID
    pub async fn get_play_queue(
        backend: &dyn Any,
        db: &DatabaseConnection,
        play_queue_id: i64,
    ) -> Result<PlaylistContext> {
        // Try to downcast to PlexBackend
        if let Some(plex_backend) = backend.downcast_ref::<PlexBackend>() {
            if let Some(api) = plex_backend.get_api_for_playqueue().await {
                debug!("Retrieving PlayQueue: {}", play_queue_id);

                // Get the PlayQueue from the Plex server
                let play_queue_response = api.get_play_queue(play_queue_id).await?;

                // Build generic PlayQueue context
                return Self::build_playqueue_context(db, play_queue_response).await;
            }
        }

        Err(anyhow!(
            "PlayQueue retrieval not available for this backend"
        ))
    }

    /// Build a PlaylistContext from a PlayQueueResponse
    async fn build_context_from_response(
        db: &DatabaseConnection,
        response: PlayQueueResponse,
        media_id: &MediaItemId,
        media_type: &str,
    ) -> Result<PlaylistContext> {
        let container = &response.media_container;

        // Check if this is an episode and we should build a TvShow context
        if media_type == "episode" {
            // Try to build a TV show context with PlayQueue info
            if let Ok(mut context) = PlaylistService::build_local_show_context(db, media_id).await {
                // Add PlayQueue info to the TV show context
                if let PlaylistContext::TvShow { .. } = &mut context {
                    if let Some(play_queue_id) = container.play_queue_id {
                        let queue_info = Self::extract_queue_info(container)?;

                        // Update episode info with PlayQueue item IDs
                        if let PlaylistContext::TvShow {
                            ref mut episodes,
                            ref mut play_queue_info,
                            ..
                        } = context
                        {
                            for (episode, queue_item) in
                                episodes.iter_mut().zip(&container.metadata)
                            {
                                if episode.id.as_ref() == queue_item.rating_key {
                                    episode.play_queue_item_id =
                                        Some(queue_item.play_queue_item_id);
                                }
                            }
                            *play_queue_info = Some(queue_info);
                        }

                        info!(
                            "Created TvShow PlayQueue context with ID: {}",
                            play_queue_id
                        );
                    }
                }
                return Ok(context);
            }
        }

        // Build generic PlayQueue context
        Self::build_playqueue_context(db, response).await
    }

    /// Build a generic PlayQueue context
    async fn build_playqueue_context(
        db: &DatabaseConnection,
        response: PlayQueueResponse,
    ) -> Result<PlaylistContext> {
        let container = &response.media_container;

        let play_queue_info = Self::extract_queue_info(container)?;

        // Convert PlayQueue items to QueueItems
        let repo = MediaRepositoryImpl::new(db.clone());
        let mut items = Vec::new();

        for queue_item in &container.metadata {
            // Try to get media type from database
            let media_type = if let Ok(Some(media)) = repo.find_by_id(&queue_item.rating_key).await
            {
                media.media_type.clone()
            } else {
                // Default to video if not found
                "video".to_string()
            };

            items.push(QueueItem {
                id: MediaItemId::new(&queue_item.rating_key),
                title: queue_item.title.clone(),
                media_type,
                duration_ms: queue_item.duration,
                play_queue_item_id: Some(queue_item.play_queue_item_id),
            });
        }

        // Find current index based on selected item ID
        let current_index = if let Some(selected_id) = container.play_queue_selected_item_id {
            container
                .metadata
                .iter()
                .position(|item| item.play_queue_item_id == selected_id)
                .unwrap_or(0)
        } else {
            0
        };

        info!(
            "Created generic PlayQueue context with {} items, current index: {}",
            items.len(),
            current_index
        );

        Ok(PlaylistContext::PlayQueue {
            play_queue_info,
            current_index,
            items,
            auto_play_next: true, // PlayQueues always support auto-play
        })
    }

    /// Extract PlayQueueInfo from container
    fn extract_queue_info(container: &PlayQueueContainer) -> Result<PlayQueueInfo> {
        let play_queue_id = container
            .play_queue_id
            .ok_or_else(|| anyhow!("PlayQueue has no ID"))?;

        let play_queue_version = container.play_queue_version.unwrap_or(1);

        let play_queue_item_id = container
            .play_queue_selected_item_id
            .or_else(|| {
                container
                    .metadata
                    .first()
                    .map(|item| item.play_queue_item_id)
            })
            .ok_or_else(|| anyhow!("PlayQueue has no items"))?;

        Ok(PlayQueueInfo {
            play_queue_id,
            play_queue_version,
            play_queue_item_id,
            source_uri: container.play_queue_source_uri.clone(),
            shuffled: container.play_queue_shuffled.unwrap_or(false),
        })
    }

    /// Add an item to an existing PlayQueue
    pub async fn add_to_queue(
        backend: &dyn Any,
        play_queue_id: i64,
        media_id: &MediaItemId,
    ) -> Result<()> {
        if let Some(plex_backend) = backend.downcast_ref::<PlexBackend>() {
            if let Some(api) = plex_backend.get_api_for_playqueue().await {
                debug!("Adding media {} to PlayQueue {}", media_id, play_queue_id);

                api.add_to_play_queue(play_queue_id, media_id.as_ref())
                    .await?;

                info!("Successfully added item to PlayQueue");
                return Ok(());
            }
        }

        Err(anyhow!(
            "PlayQueue modification not available for this backend"
        ))
    }

    /// Remove an item from a PlayQueue
    pub async fn remove_from_queue(
        backend: &dyn Any,
        play_queue_id: i64,
        play_queue_item_id: i64,
    ) -> Result<()> {
        if let Some(plex_backend) = backend.downcast_ref::<PlexBackend>() {
            if let Some(api) = plex_backend.get_api_for_playqueue().await {
                debug!(
                    "Removing item {} from PlayQueue {}",
                    play_queue_item_id, play_queue_id
                );

                api.remove_from_play_queue(play_queue_id, play_queue_item_id)
                    .await?;

                info!("Successfully removed item from PlayQueue");
                return Ok(());
            }
        }

        Err(anyhow!(
            "PlayQueue modification not available for this backend"
        ))
    }

    /// Update playback progress with PlayQueue context
    pub async fn update_progress_with_queue(
        backend: &dyn Any,
        context: &PlaylistContext,
        media_id: &MediaItemId,
        position: std::time::Duration,
        duration: std::time::Duration,
        state: &str,
    ) -> Result<()> {
        if let Some(queue_info) = context.get_play_queue_info() {
            if let Some(plex_backend) = backend.downcast_ref::<PlexBackend>() {
                if let Some(api) = plex_backend.get_api_for_playqueue().await {
                    debug!(
                        "Updating PlayQueue progress - queue: {}, item: {}, media: {}",
                        queue_info.play_queue_id, queue_info.play_queue_item_id, media_id
                    );

                    // Use the PlayQueue-aware timeline update
                    api.update_play_queue_progress(
                        queue_info.play_queue_id,
                        queue_info.play_queue_item_id,
                        media_id.as_ref(),
                        position,
                        duration,
                        state,
                    )
                    .await?;

                    return Ok(());
                }
            }
        }

        // Fall back to regular progress update
        debug!("No PlayQueue context, using regular progress update");
        Ok(())
    }
}

/// Service for managing playlist contexts and episode navigation
pub struct PlaylistService;

impl PlaylistService {
    /// Build playlist context for a TV show episode
    /// Automatically tries PlayQueue for Plex sources, falls back to local context
    pub async fn build_show_context(
        db: &DatabaseConnection,
        episode_id: &MediaItemId,
    ) -> Result<PlaylistContext> {
        use crate::db::repository::source_repository::SourceRepositoryImpl;
        use crate::services::core::BackendService;

        // First, get the media item to check its source
        let media_repo = MediaRepositoryImpl::new(db.clone());
        if let Ok(Some(media)) = media_repo.find_by_id(episode_id.as_ref()).await {
            if let Ok(source_id_num) = media.source_id.parse::<i32>() {
                // Check if this is a Plex source
                let source_repo = SourceRepositoryImpl::new(db.clone());
                if let Ok(Some(source)) = source_repo.find_by_id(&media.source_id).await {
                    if source.source_type == "plex" || source.source_type == "PlexServer" {
                        // Try to create a backend and get PlayQueue context
                        match BackendService::create_backend_for_source(db, &source).await {
                            Ok(backend) => {
                                // Try to create PlayQueue context
                                // We need to pass the backend as Any, which requires downcasting
                                // For now, we skip PlayQueue integration here and rely on direct calls
                                if let Ok(context) = PlayQueueService::create_from_media(
                                    backend.as_any(),
                                    db,
                                    episode_id,
                                    "episode",
                                )
                                .await
                                {
                                    info!("Created PlayQueue context for episode {}", episode_id);

                                    // Save PlayQueue state in database for resume
                                    if let Some(queue_info) = context.get_play_queue_info() {
                                        let playback_repo =
                                            crate::db::repository::PlaybackRepositoryImpl::new(
                                                db.clone(),
                                            );
                                        if let Err(e) = playback_repo
                                            .save_playqueue_state(
                                                episode_id.as_ref(),
                                                None, // TODO: Get actual user ID
                                                queue_info.play_queue_id,
                                                queue_info.play_queue_version,
                                                queue_info.play_queue_item_id,
                                                source_id_num,
                                            )
                                            .await
                                        {
                                            warn!("Failed to save PlayQueue state: {}", e);
                                        }
                                    }

                                    return Ok(context);
                                }
                            }
                            Err(e) => {
                                debug!("Failed to create backend for PlayQueue: {}", e);
                            }
                        }
                    }
                }
            }
        }

        // Fall back to building regular playlist context
        Self::build_local_show_context(db, episode_id).await
    }

    /// Build playlist context for a TV show episode without PlayQueue (internal)
    async fn build_local_show_context(
        db: &DatabaseConnection,
        episode_id: &MediaItemId,
    ) -> Result<PlaylistContext> {
        let repo = MediaRepositoryImpl::new(db.clone());

        // Get the current episode details
        let current_episode = repo
            .find_by_id(episode_id.as_ref())
            .await?
            .ok_or_else(|| anyhow!("Episode not found: {}", episode_id))?;

        // Make sure it's an episode
        if current_episode.media_type != "episode" {
            // Not an episode, return single item context
            return Ok(PlaylistContext::SingleItem);
        }

        // Get the parent show ID
        let show_id = current_episode
            .parent_id
            .as_ref()
            .ok_or_else(|| anyhow!("Episode has no parent show"))?;

        // Get the show details
        let show = repo
            .find_by_id(show_id)
            .await?
            .ok_or_else(|| anyhow!("Parent show not found: {}", show_id))?;

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
                play_queue_item_id: None,   // Will be set if using PlayQueue
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
            auto_play_next: true,  // TODO: Get from user preferences
            play_queue_info: None, // Will be set by PlayQueueService if applicable
        })
    }
}
