//! Metadata Refresh Service
//!
//! This service handles TTL-based metadata refresh logic. It decides when to refresh
//! and queues refresh requests via the MessageBroker.
//!
//! ## Usage Pattern
//!
//! 1. Check if content needs refresh using `needs_refresh()`
//! 2. If stale, queue a refresh using `queue_library_refresh()` or `queue_items_refresh()`
//! 3. Workers listen for `MetadataRefreshMessage` and process requests
//! 4. After refresh, the cache is updated with new `fetched_at` timestamps

use anyhow::Result;
use chrono::{DateTime, Utc};
use tracing::{debug, info};

use crate::backends::traits::MediaBackend;
use crate::db::connection::DatabaseConnection;
use crate::db::repository::{MediaRepository, MediaRepositoryImpl};
use crate::models::{LibraryId, MediaItem, MediaItemId, SourceId};
use crate::services::core::cache_config::{CacheConfig, ContentType, cache_config};
use crate::services::core::media::MediaService;
use crate::ui::shared::broker::{BROKER, BrokerMessage, MetadataRefreshMessage, RefreshPriority};

/// Stateless service for metadata refresh operations
pub struct MetadataRefreshService;

impl MetadataRefreshService {
    /// Check if content needs refresh based on TTL
    ///
    /// Returns true if the content should be refreshed in the background.
    /// Uses the global cache config by default.
    pub fn needs_refresh(fetched_at: Option<DateTime<Utc>>, content_type: ContentType) -> bool {
        cache_config().is_stale(fetched_at, content_type)
    }

    /// Check if content needs refresh with a custom config
    pub fn needs_refresh_with_config(
        fetched_at: Option<DateTime<Utc>>,
        content_type: ContentType,
        config: &CacheConfig,
    ) -> bool {
        config.is_stale(fetched_at, content_type)
    }

    /// Check if a naive datetime (from DB) needs refresh
    pub fn needs_refresh_naive(
        fetched_at: Option<chrono::NaiveDateTime>,
        content_type: ContentType,
    ) -> bool {
        cache_config().is_stale_naive(fetched_at, content_type)
    }

    /// Queue a background refresh for a library
    ///
    /// This broadcasts a message to the MessageBroker which workers can subscribe to.
    /// The refresh happens asynchronously - this method returns immediately.
    pub async fn queue_library_refresh(
        source_id: &SourceId,
        library_id: &LibraryId,
        priority: RefreshPriority,
    ) -> Result<()> {
        info!(
            "Queueing library refresh: source={}, library={}, priority={:?}",
            source_id, library_id, priority
        );

        BROKER
            .broadcast(BrokerMessage::MetadataRefresh(
                MetadataRefreshMessage::RefreshLibrary {
                    source_id: source_id.to_string(),
                    library_id: library_id.to_string(),
                    priority,
                },
            ))
            .await;

        Ok(())
    }

    /// Queue refresh for specific items
    ///
    /// Useful when only certain items need updating (e.g., after viewing a details page).
    pub async fn queue_items_refresh(
        source_id: &SourceId,
        item_ids: &[MediaItemId],
        priority: RefreshPriority,
    ) -> Result<()> {
        if item_ids.is_empty() {
            return Ok(());
        }

        debug!(
            "Queueing items refresh: source={}, count={}, priority={:?}",
            source_id,
            item_ids.len(),
            priority
        );

        BROKER
            .broadcast(BrokerMessage::MetadataRefresh(
                MetadataRefreshMessage::RefreshItems {
                    source_id: source_id.to_string(),
                    item_ids: item_ids.iter().map(|id| id.to_string()).collect(),
                    priority,
                },
            ))
            .await;

        Ok(())
    }

    /// Queue refresh for a single item's full metadata (cast/crew)
    ///
    /// This is typically called when a user opens a details page and we need
    /// complete metadata including cast, crew, etc.
    pub async fn queue_item_metadata_refresh(
        source_id: &SourceId,
        item_id: &MediaItemId,
    ) -> Result<()> {
        debug!(
            "Queueing item metadata refresh: source={}, item={}",
            source_id, item_id
        );

        BROKER
            .broadcast(BrokerMessage::MetadataRefresh(
                MetadataRefreshMessage::RefreshItemMetadata {
                    source_id: source_id.to_string(),
                    item_id: item_id.to_string(),
                },
            ))
            .await;

        Ok(())
    }

    /// Refresh a single movie's full metadata from the backend
    ///
    /// This fetches complete metadata including cast/crew.
    /// Called by workers when processing refresh requests.
    pub async fn refresh_movie_metadata(
        db: &DatabaseConnection,
        backend: &dyn MediaBackend,
        movie_id: &MediaItemId,
        source_id: &SourceId,
        library_id: &LibraryId,
    ) -> Result<MediaItem> {
        info!("Refreshing movie metadata: {}", movie_id);

        // Fetch fresh metadata from backend
        let movie = backend.get_movie_metadata(movie_id).await?;
        let item = MediaItem::Movie(movie);

        // Save to database (this updates fetched_at)
        MediaService::save_media_item(db, item.clone(), library_id, source_id).await?;

        // Notify completion
        BROKER
            .broadcast(BrokerMessage::MetadataRefresh(
                MetadataRefreshMessage::ItemRefreshCompleted {
                    item_id: movie_id.to_string(),
                },
            ))
            .await;

        Ok(item)
    }

    /// Refresh a single show's full metadata from the backend
    ///
    /// This fetches complete metadata including cast/crew and seasons.
    /// Called by workers when processing refresh requests.
    pub async fn refresh_show_metadata(
        db: &DatabaseConnection,
        backend: &dyn MediaBackend,
        show_id: &crate::models::ShowId,
        source_id: &SourceId,
        library_id: &LibraryId,
    ) -> Result<MediaItem> {
        info!("Refreshing show metadata: {}", show_id);

        // Fetch fresh metadata from backend
        let show = backend.get_show_metadata(show_id).await?;
        let item = MediaItem::Show(show);

        // Save to database (this updates fetched_at)
        MediaService::save_media_item(db, item.clone(), library_id, source_id).await?;

        // Notify completion
        BROKER
            .broadcast(BrokerMessage::MetadataRefresh(
                MetadataRefreshMessage::ItemRefreshCompleted {
                    item_id: show_id.to_string(),
                },
            ))
            .await;

        Ok(item)
    }

    /// Get stale items in a library that need refresh
    ///
    /// Returns items where fetched_at is older than the TTL for their content type.
    pub async fn get_stale_items(
        db: &DatabaseConnection,
        library_id: &LibraryId,
        content_type: ContentType,
    ) -> Result<Vec<crate::db::entities::MediaItemModel>> {
        let config = cache_config();
        let ttl = config.ttl_for(content_type);

        let repo = MediaRepositoryImpl::new(db.clone());
        repo.find_stale_items(library_id.as_ref(), ttl).await
    }

    /// Check if a media item model (from DB) needs refresh
    pub fn model_needs_refresh(
        model: &crate::db::entities::MediaItemModel,
        content_type: ContentType,
    ) -> bool {
        Self::needs_refresh_naive(model.fetched_at, content_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_refresh_with_none() {
        // None should always need refresh
        assert!(MetadataRefreshService::needs_refresh(
            None,
            ContentType::Libraries
        ));
        assert!(MetadataRefreshService::needs_refresh(
            None,
            ContentType::MediaItems
        ));
    }

    #[test]
    fn test_needs_refresh_fresh_content() {
        let now = Utc::now();
        // Just fetched should not need refresh
        assert!(!MetadataRefreshService::needs_refresh(
            Some(now),
            ContentType::Libraries
        ));
        assert!(!MetadataRefreshService::needs_refresh(
            Some(now),
            ContentType::MediaItems
        ));
    }

    #[test]
    fn test_needs_refresh_stale_content() {
        // 2 hours ago - stale for libraries (1h TTL)
        let two_hours_ago = Utc::now() - chrono::Duration::hours(2);
        assert!(MetadataRefreshService::needs_refresh(
            Some(two_hours_ago),
            ContentType::Libraries
        ));

        // But not stale for media_items (4h TTL)
        assert!(!MetadataRefreshService::needs_refresh(
            Some(two_hours_ago),
            ContentType::MediaItems
        ));
    }
}
