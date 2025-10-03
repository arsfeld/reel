use crate::db::connection::DatabaseConnection;
use crate::models::{MediaItemId, SourceId};
use crate::services::core::backend::BackendService;
use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub enum AppCommand {
    StartPlayback {
        media_id: String,
    },
    ChangeQuality {
        media_id: String,
        quality: crate::models::QualityOption,
    },
}

#[derive(Debug, Clone)]
pub enum CommandResult {
    PlaybackStarted { media_id: String, url: String },
    QualityChanged { media_id: String, url: String },
    Error(String),
}

pub async fn execute_command(command: AppCommand, db: &DatabaseConnection) -> CommandResult {
    match command {
        AppCommand::StartPlayback { media_id } => match start_playback(db, &media_id).await {
            Ok(url) => CommandResult::PlaybackStarted { media_id, url },
            Err(e) => CommandResult::Error(e.to_string()),
        },
        AppCommand::ChangeQuality { media_id, quality } => {
            match change_quality(db, &media_id, &quality).await {
                Ok(url) => CommandResult::QualityChanged { media_id, url },
                Err(e) => CommandResult::Error(e.to_string()),
            }
        }
    }
}

async fn start_playback(db: &DatabaseConnection, media_id: &str) -> Result<String> {
    use crate::db::repository::{MediaRepositoryImpl, Repository};
    use crate::services::cache_service::cache_service;

    // Get actual stream URL from backend using stateless BackendService
    let media_item_id = MediaItemId::new(media_id.to_string());

    // Get source_id from the media item
    let media_repo = MediaRepositoryImpl::new(db.clone());
    let media_entity = media_repo
        .find_by_id(media_item_id.as_ref())
        .await?
        .ok_or_else(|| anyhow::anyhow!("Media item not found: {}", media_item_id))?;
    let source_id = SourceId::new(media_entity.source_id);

    // BackendService::get_stream_url handles all the backend creation and URL fetching
    let stream_info = BackendService::get_stream_url(db, &media_item_id).await?;

    // Get cached stream - no fallback
    let cache_handle = cache_service()
        .get_handle()
        .await
        .context("Cache service is not available")?;

    let cached_stream = cache_handle
        .get_cached_stream(source_id, media_item_id, stream_info)
        .await
        .context("Failed to get cached stream")?;

    let playback_url = cached_stream.playback_url();
    tracing::info!(
        "Using cached stream for media: {} (cached: {}, complete: {}, playback_url: {})",
        media_id,
        cached_stream.cached_url.is_some(),
        cached_stream.is_complete,
        playback_url
    );

    Ok(playback_url.to_string())
}

async fn change_quality(
    db: &DatabaseConnection,
    media_id: &str,
    quality: &crate::models::QualityOption,
) -> Result<String> {
    use crate::db::repository::{MediaRepositoryImpl, Repository};
    use crate::services::cache_service::cache_service;

    let media_item_id = MediaItemId::new(media_id.to_string());

    // Get source_id from the media item
    let media_repo = MediaRepositoryImpl::new(db.clone());
    let media_entity = media_repo
        .find_by_id(media_item_id.as_ref())
        .await?
        .ok_or_else(|| anyhow::anyhow!("Media item not found: {}", media_item_id))?;
    let source_id = SourceId::new(media_entity.source_id);

    // Get stream URL for the specific quality
    let stream_url = BackendService::get_stream_with_quality(db, &media_item_id, quality).await?;

    // Determine cache key based on quality
    let quality_key = if quality.requires_transcode {
        quality.name.clone() // "1080p", "720p", etc.
    } else {
        "original".to_string()
    };

    // Get cached stream with quality-aware key
    let cache_handle = cache_service()
        .get_handle()
        .await
        .context("Cache service is not available")?;

    let playback_url = cache_handle
        .get_cached_stream_with_quality(source_id, media_item_id, quality_key.clone(), stream_url)
        .await
        .context("Failed to get cached stream with quality")?;

    tracing::info!(
        "Quality changed to {} for media: {} (quality_key: {}, playback_url: {})",
        quality.name,
        media_id,
        quality_key,
        playback_url
    );

    Ok(playback_url)
}
