use super::BaseRepository;
use crate::db::entities::{
    CacheChunk, CacheChunkActiveModel, CacheChunkModel, CacheDownloadQueue,
    CacheDownloadQueueActiveModel, CacheDownloadQueueModel, CacheEntry, CacheEntryActiveModel,
    CacheEntryModel, CacheHeader, CacheHeaderActiveModel, CacheHeaderModel, CacheQualityVariant,
    CacheQualityVariantActiveModel, CacheQualityVariantModel, CacheStatistics,
    CacheStatisticsActiveModel, CacheStatisticsModel, cache_chunks, cache_download_queue,
    cache_entries, cache_headers, cache_quality_variants, cache_statistics,
};
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set, TransactionTrait,
};
use std::sync::Arc;

/// Repository for cache-related database operations
#[async_trait]
pub trait CacheRepository: Send + Sync {
    // CacheEntry operations
    async fn find_cache_entry(
        &self,
        source_id: &str,
        media_id: &str,
        quality: &str,
    ) -> Result<Option<CacheEntryModel>>;
    async fn insert_cache_entry(&self, entry: CacheEntryModel) -> Result<CacheEntryModel>;
    async fn update_cache_entry(&self, entry: CacheEntryModel) -> Result<CacheEntryModel>;
    async fn delete_cache_entry(&self, id: i32) -> Result<()>;
    async fn list_cache_entries(&self) -> Result<Vec<CacheEntryModel>>;
    async fn mark_cache_accessed(&self, id: i32) -> Result<()>;
    async fn update_download_progress(
        &self,
        id: i32,
        downloaded_bytes: i64,
        is_complete: bool,
    ) -> Result<()>;
    async fn find_cache_entries_by_media(&self, media_id: &str) -> Result<Vec<CacheEntryModel>>;
    async fn find_cache_entries_by_source(&self, source_id: &str) -> Result<Vec<CacheEntryModel>>;

    // CacheChunk operations
    async fn add_cache_chunk(&self, chunk: CacheChunkModel) -> Result<CacheChunkModel>;
    async fn get_chunks_for_entry(&self, cache_entry_id: i32) -> Result<Vec<CacheChunkModel>>;
    async fn delete_chunks_for_entry(&self, cache_entry_id: i32) -> Result<()>;
    async fn has_byte_range(&self, cache_entry_id: i32, start: i64, end: i64) -> Result<bool>;

    // State derivation methods (for database-driven state computation)
    async fn get_downloaded_bytes(&self, cache_entry_id: i32) -> Result<i64>;
    async fn get_chunk_count(&self, cache_entry_id: i32) -> Result<usize>;
    async fn has_pending_downloads(&self, source_id: &str, media_id: &str) -> Result<bool>;

    // CacheDownloadQueue operations
    async fn add_to_download_queue(
        &self,
        item: CacheDownloadQueueModel,
    ) -> Result<CacheDownloadQueueModel>;
    async fn get_pending_downloads(&self) -> Result<Vec<CacheDownloadQueueModel>>;
    async fn update_download_status(&self, id: i32, status: String) -> Result<()>;
    async fn increment_retry_count(&self, id: i32) -> Result<()>;
    async fn remove_from_queue(&self, id: i32) -> Result<()>;
    async fn find_in_queue(
        &self,
        media_id: &str,
        source_id: &str,
    ) -> Result<Option<CacheDownloadQueueModel>>;

    // CacheStatistics operations
    async fn get_cache_statistics(&self) -> Result<Option<CacheStatisticsModel>>;
    async fn update_cache_statistics(
        &self,
        stats: CacheStatisticsModel,
    ) -> Result<CacheStatisticsModel>;
    async fn increment_cache_hit(&self) -> Result<()>;
    async fn increment_cache_miss(&self) -> Result<()>;
    async fn update_cache_size(&self, total_size: i64, file_count: i32) -> Result<()>;

    // CacheHeader operations
    async fn add_cache_headers(&self, headers: Vec<CacheHeaderModel>) -> Result<()>;
    async fn get_headers_for_entry(&self, cache_entry_id: i32) -> Result<Vec<CacheHeaderModel>>;
    async fn delete_headers_for_entry(&self, cache_entry_id: i32) -> Result<()>;

    // CacheQualityVariant operations
    async fn add_quality_variant(
        &self,
        variant: CacheQualityVariantModel,
    ) -> Result<CacheQualityVariantModel>;
    async fn find_quality_variants(
        &self,
        media_id: &str,
        source_id: &str,
    ) -> Result<Vec<CacheQualityVariantModel>>;
    async fn delete_quality_variants(&self, media_id: &str, source_id: &str) -> Result<()>;

    // Cleanup operations
    async fn get_entries_for_cleanup(&self, limit: usize) -> Result<Vec<CacheEntryModel>>;
    async fn delete_old_entries(&self, days_old: i64) -> Result<u64>;
}

#[derive(Debug)]
pub struct CacheRepositoryImpl {
    base: BaseRepository,
}

impl CacheRepositoryImpl {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            base: BaseRepository::new(db),
        }
    }
}

#[async_trait]
impl CacheRepository for CacheRepositoryImpl {
    // CacheEntry operations
    async fn find_cache_entry(
        &self,
        source_id: &str,
        media_id: &str,
        quality: &str,
    ) -> Result<Option<CacheEntryModel>> {
        Ok(CacheEntry::find()
            .filter(cache_entries::Column::SourceId.eq(source_id))
            .filter(cache_entries::Column::MediaId.eq(media_id))
            .filter(cache_entries::Column::Quality.eq(quality))
            .one(self.base.db.as_ref())
            .await?)
    }

    async fn insert_cache_entry(&self, entry: CacheEntryModel) -> Result<CacheEntryModel> {
        let active_model = CacheEntryActiveModel {
            id: sea_orm::NotSet,
            source_id: Set(entry.source_id.clone()),
            media_id: Set(entry.media_id.clone()),
            quality: Set(entry.quality.clone()),
            original_url: Set(entry.original_url.clone()),
            file_path: Set(entry.file_path.clone()),
            file_size: Set(entry.file_size),
            expected_total_size: Set(entry.expected_total_size),
            downloaded_bytes: Set(entry.downloaded_bytes),
            is_complete: Set(entry.is_complete),
            priority: Set(entry.priority),
            created_at: Set(Utc::now().naive_utc()),
            last_accessed: Set(Utc::now().naive_utc()),
            last_modified: Set(Utc::now().naive_utc()),
            access_count: Set(0),
            mime_type: Set(entry.mime_type),
            video_codec: Set(entry.video_codec),
            audio_codec: Set(entry.audio_codec),
            container: Set(entry.container),
            resolution_width: Set(entry.resolution_width),
            resolution_height: Set(entry.resolution_height),
            bitrate: Set(entry.bitrate),
            duration_secs: Set(entry.duration_secs),
            etag: Set(entry.etag),
            expires_at: Set(entry.expires_at),
        };

        Ok(active_model.insert(self.base.db.as_ref()).await?)
    }

    async fn update_cache_entry(&self, entry: CacheEntryModel) -> Result<CacheEntryModel> {
        let mut active_model: CacheEntryActiveModel = entry.clone().into();
        active_model.last_modified = Set(Utc::now().naive_utc());

        Ok(active_model.update(self.base.db.as_ref()).await?)
    }

    async fn delete_cache_entry(&self, id: i32) -> Result<()> {
        CacheEntry::delete_by_id(id)
            .exec(self.base.db.as_ref())
            .await?;
        Ok(())
    }

    async fn list_cache_entries(&self) -> Result<Vec<CacheEntryModel>> {
        Ok(CacheEntry::find()
            .order_by_desc(cache_entries::Column::LastAccessed)
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn mark_cache_accessed(&self, id: i32) -> Result<()> {
        if let Some(entry) = CacheEntry::find_by_id(id)
            .one(self.base.db.as_ref())
            .await?
        {
            let mut active_model: CacheEntryActiveModel = entry.into();
            active_model.last_accessed = Set(Utc::now().naive_utc());
            active_model.access_count = Set(active_model.access_count.unwrap() + 1);
            active_model.update(self.base.db.as_ref()).await?;
        }
        Ok(())
    }

    async fn update_download_progress(
        &self,
        id: i32,
        downloaded_bytes: i64,
        is_complete: bool,
    ) -> Result<()> {
        if let Some(entry) = CacheEntry::find_by_id(id)
            .one(self.base.db.as_ref())
            .await?
        {
            let mut active_model: CacheEntryActiveModel = entry.into();
            active_model.downloaded_bytes = Set(downloaded_bytes);
            active_model.is_complete = Set(is_complete);
            active_model.last_modified = Set(Utc::now().naive_utc());
            active_model.update(self.base.db.as_ref()).await?;
        }
        Ok(())
    }

    async fn find_cache_entries_by_media(&self, media_id: &str) -> Result<Vec<CacheEntryModel>> {
        Ok(CacheEntry::find()
            .filter(cache_entries::Column::MediaId.eq(media_id))
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn find_cache_entries_by_source(&self, source_id: &str) -> Result<Vec<CacheEntryModel>> {
        Ok(CacheEntry::find()
            .filter(cache_entries::Column::SourceId.eq(source_id))
            .all(self.base.db.as_ref())
            .await?)
    }

    // CacheChunk operations
    async fn add_cache_chunk(&self, chunk: CacheChunkModel) -> Result<CacheChunkModel> {
        // Use transaction to ensure atomicity when merging chunks
        let tx = self.base.db.begin().await?;

        // Get all existing chunks for this entry
        let chunks = CacheChunk::find()
            .filter(cache_chunks::Column::CacheEntryId.eq(chunk.cache_entry_id))
            .all(&tx)
            .await?;

        // Find chunks adjacent to the new chunk
        // Two chunks are adjacent if one ends exactly where the other starts
        let before_chunk = chunks.iter().find(|c| c.end_byte == chunk.start_byte - 1);
        let after_chunk = chunks.iter().find(|c| c.start_byte == chunk.end_byte + 1);

        let result = match (before_chunk, after_chunk) {
            (Some(before), Some(after)) => {
                // New chunk fills gap between two chunks - merge all three
                // Update before_chunk to span from before.start to after.end
                let mut active_model: CacheChunkActiveModel = before.clone().into();
                active_model.end_byte = Set(after.end_byte);
                active_model.downloaded_at = Set(Utc::now().naive_utc());

                let updated = active_model.update(&tx).await?;

                // Delete the after_chunk since it's now merged
                CacheChunk::delete_by_id(after.id).exec(&tx).await?;

                updated
            }
            (Some(before), None) => {
                // Extend before_chunk forward to include new chunk
                let mut active_model: CacheChunkActiveModel = before.clone().into();
                active_model.end_byte = Set(chunk.end_byte);
                active_model.downloaded_at = Set(Utc::now().naive_utc());

                active_model.update(&tx).await?
            }
            (None, Some(after)) => {
                // Extend after_chunk backward to include new chunk
                let mut active_model: CacheChunkActiveModel = after.clone().into();
                active_model.start_byte = Set(chunk.start_byte);
                active_model.downloaded_at = Set(Utc::now().naive_utc());

                active_model.update(&tx).await?
            }
            (None, None) => {
                // No adjacent chunks, insert new chunk as-is
                let active_model = CacheChunkActiveModel {
                    id: sea_orm::NotSet,
                    cache_entry_id: Set(chunk.cache_entry_id),
                    start_byte: Set(chunk.start_byte),
                    end_byte: Set(chunk.end_byte),
                    downloaded_at: Set(Utc::now().naive_utc()),
                };

                active_model.insert(&tx).await?
            }
        };

        tx.commit().await?;
        Ok(result)
    }

    async fn get_chunks_for_entry(&self, cache_entry_id: i32) -> Result<Vec<CacheChunkModel>> {
        Ok(CacheChunk::find()
            .filter(cache_chunks::Column::CacheEntryId.eq(cache_entry_id))
            .order_by_asc(cache_chunks::Column::StartByte)
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn delete_chunks_for_entry(&self, cache_entry_id: i32) -> Result<()> {
        CacheChunk::delete_many()
            .filter(cache_chunks::Column::CacheEntryId.eq(cache_entry_id))
            .exec(self.base.db.as_ref())
            .await?;
        Ok(())
    }

    async fn has_byte_range(&self, cache_entry_id: i32, start: i64, end: i64) -> Result<bool> {
        let chunks = self.get_chunks_for_entry(cache_entry_id).await?;

        // Check if the requested range is covered by any chunk
        for chunk in chunks {
            if chunk.start_byte <= start && chunk.end_byte >= end {
                return Ok(true);
            }
        }
        Ok(false)
    }

    // State derivation methods
    async fn get_downloaded_bytes(&self, cache_entry_id: i32) -> Result<i64> {
        let chunks = self.get_chunks_for_entry(cache_entry_id).await?;

        // Sum up all chunk sizes: SUM(end_byte - start_byte + 1)
        let total: i64 = chunks
            .iter()
            .map(|chunk| chunk.end_byte - chunk.start_byte + 1)
            .sum();

        Ok(total)
    }

    async fn get_chunk_count(&self, cache_entry_id: i32) -> Result<usize> {
        Ok(CacheChunk::find()
            .filter(cache_chunks::Column::CacheEntryId.eq(cache_entry_id))
            .count(self.base.db.as_ref())
            .await? as usize)
    }

    async fn has_pending_downloads(&self, source_id: &str, media_id: &str) -> Result<bool> {
        let count = CacheDownloadQueue::find()
            .filter(cache_download_queue::Column::SourceId.eq(source_id))
            .filter(cache_download_queue::Column::MediaId.eq(media_id))
            .filter(cache_download_queue::Column::Status.eq("pending"))
            .count(self.base.db.as_ref())
            .await?;

        Ok(count > 0)
    }

    // CacheDownloadQueue operations
    async fn add_to_download_queue(
        &self,
        item: CacheDownloadQueueModel,
    ) -> Result<CacheDownloadQueueModel> {
        let active_model = CacheDownloadQueueActiveModel {
            id: sea_orm::NotSet,
            media_id: Set(item.media_id.clone()),
            source_id: Set(item.source_id.clone()),
            quality: Set(item.quality.clone()),
            priority: Set(item.priority),
            status: Set("pending".to_string()),
            retry_count: Set(0),
            last_retry_at: Set(None),
            created_at: Set(Utc::now().naive_utc()),
            scheduled_for: Set(item.scheduled_for),
            expires_at: Set(item.expires_at),
            user_requested: Set(item.user_requested),
        };

        Ok(active_model.insert(self.base.db.as_ref()).await?)
    }

    async fn get_pending_downloads(&self) -> Result<Vec<CacheDownloadQueueModel>> {
        Ok(CacheDownloadQueue::find()
            .filter(cache_download_queue::Column::Status.eq("pending"))
            .order_by_desc(cache_download_queue::Column::Priority)
            .order_by_asc(cache_download_queue::Column::CreatedAt)
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn update_download_status(&self, id: i32, status: String) -> Result<()> {
        if let Some(item) = CacheDownloadQueue::find_by_id(id)
            .one(self.base.db.as_ref())
            .await?
        {
            let mut active_model: CacheDownloadQueueActiveModel = item.into();
            active_model.status = Set(status);
            active_model.update(self.base.db.as_ref()).await?;
        }
        Ok(())
    }

    async fn increment_retry_count(&self, id: i32) -> Result<()> {
        if let Some(item) = CacheDownloadQueue::find_by_id(id)
            .one(self.base.db.as_ref())
            .await?
        {
            let mut active_model: CacheDownloadQueueActiveModel = item.into();
            active_model.retry_count = Set(active_model.retry_count.unwrap() + 1);
            active_model.last_retry_at = Set(Some(Utc::now().naive_utc()));
            active_model.update(self.base.db.as_ref()).await?;
        }
        Ok(())
    }

    async fn remove_from_queue(&self, id: i32) -> Result<()> {
        CacheDownloadQueue::delete_by_id(id)
            .exec(self.base.db.as_ref())
            .await?;
        Ok(())
    }

    async fn find_in_queue(
        &self,
        media_id: &str,
        source_id: &str,
    ) -> Result<Option<CacheDownloadQueueModel>> {
        Ok(CacheDownloadQueue::find()
            .filter(cache_download_queue::Column::MediaId.eq(media_id))
            .filter(cache_download_queue::Column::SourceId.eq(source_id))
            .one(self.base.db.as_ref())
            .await?)
    }

    // CacheStatistics operations
    async fn get_cache_statistics(&self) -> Result<Option<CacheStatisticsModel>> {
        Ok(CacheStatistics::find().one(self.base.db.as_ref()).await?)
    }

    async fn update_cache_statistics(
        &self,
        stats: CacheStatisticsModel,
    ) -> Result<CacheStatisticsModel> {
        let mut active_model: CacheStatisticsActiveModel = stats.into();
        active_model.updated_at = Set(Utc::now().naive_utc());

        Ok(active_model.update(self.base.db.as_ref()).await?)
    }

    async fn increment_cache_hit(&self) -> Result<()> {
        let tx = self.base.db.begin().await?;

        if let Some(stats) = CacheStatistics::find().one(&tx).await? {
            let mut active_model: CacheStatisticsActiveModel = stats.into();
            active_model.hit_count = Set(active_model.hit_count.unwrap() + 1);
            active_model.updated_at = Set(Utc::now().naive_utc());
            active_model.update(&tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn increment_cache_miss(&self) -> Result<()> {
        let tx = self.base.db.begin().await?;

        if let Some(stats) = CacheStatistics::find().one(&tx).await? {
            let mut active_model: CacheStatisticsActiveModel = stats.into();
            active_model.miss_count = Set(active_model.miss_count.unwrap() + 1);
            active_model.updated_at = Set(Utc::now().naive_utc());
            active_model.update(&tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn update_cache_size(&self, total_size: i64, file_count: i32) -> Result<()> {
        let tx = self.base.db.begin().await?;

        if let Some(stats) = CacheStatistics::find().one(&tx).await? {
            let mut active_model: CacheStatisticsActiveModel = stats.into();
            active_model.total_size = Set(total_size);
            active_model.file_count = Set(file_count);
            active_model.updated_at = Set(Utc::now().naive_utc());
            active_model.update(&tx).await?;
        } else {
            // Create initial statistics record if it doesn't exist
            let active_model = CacheStatisticsActiveModel {
                id: sea_orm::NotSet,
                total_size: Set(total_size),
                file_count: Set(file_count),
                max_size_bytes: Set(1024 * 1024 * 1024 * 10), // 10GB default
                max_file_count: Set(1000),                    // 1000 files default
                hit_count: Set(0),
                miss_count: Set(0),
                bytes_served: Set(0),
                bytes_downloaded: Set(0),
                last_cleanup_at: Set(None),
                updated_at: Set(Utc::now().naive_utc()),
            };
            active_model.insert(&tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    // CacheHeader operations
    async fn add_cache_headers(&self, headers: Vec<CacheHeaderModel>) -> Result<()> {
        let tx = self.base.db.begin().await?;

        for header in headers {
            let active_model = CacheHeaderActiveModel {
                id: sea_orm::NotSet,
                cache_entry_id: Set(header.cache_entry_id),
                header_name: Set(header.header_name),
                header_value: Set(header.header_value),
            };
            active_model.insert(&tx).await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn get_headers_for_entry(&self, cache_entry_id: i32) -> Result<Vec<CacheHeaderModel>> {
        Ok(CacheHeader::find()
            .filter(cache_headers::Column::CacheEntryId.eq(cache_entry_id))
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn delete_headers_for_entry(&self, cache_entry_id: i32) -> Result<()> {
        CacheHeader::delete_many()
            .filter(cache_headers::Column::CacheEntryId.eq(cache_entry_id))
            .exec(self.base.db.as_ref())
            .await?;
        Ok(())
    }

    // CacheQualityVariant operations
    async fn add_quality_variant(
        &self,
        variant: CacheQualityVariantModel,
    ) -> Result<CacheQualityVariantModel> {
        let active_model = CacheQualityVariantActiveModel {
            id: sea_orm::NotSet,
            media_id: Set(variant.media_id.clone()),
            source_id: Set(variant.source_id.clone()),
            quality: Set(variant.quality.clone()),
            resolution_width: Set(variant.resolution_width),
            resolution_height: Set(variant.resolution_height),
            bitrate: Set(variant.bitrate),
            file_size: Set(variant.file_size),
            container: Set(variant.container),
            video_codec: Set(variant.video_codec),
            audio_codec: Set(variant.audio_codec),
            stream_url: Set(variant.stream_url),
            is_default: Set(variant.is_default),
            discovered_at: Set(Utc::now().naive_utc()),
        };

        Ok(active_model.insert(self.base.db.as_ref()).await?)
    }

    async fn find_quality_variants(
        &self,
        media_id: &str,
        source_id: &str,
    ) -> Result<Vec<CacheQualityVariantModel>> {
        Ok(CacheQualityVariant::find()
            .filter(cache_quality_variants::Column::MediaId.eq(media_id))
            .filter(cache_quality_variants::Column::SourceId.eq(source_id))
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn delete_quality_variants(&self, media_id: &str, source_id: &str) -> Result<()> {
        CacheQualityVariant::delete_many()
            .filter(cache_quality_variants::Column::MediaId.eq(media_id))
            .filter(cache_quality_variants::Column::SourceId.eq(source_id))
            .exec(self.base.db.as_ref())
            .await?;
        Ok(())
    }

    // Cleanup operations
    async fn get_entries_for_cleanup(&self, limit: usize) -> Result<Vec<CacheEntryModel>> {
        use sea_orm::QuerySelect;

        Ok(CacheEntry::find()
            .order_by_asc(cache_entries::Column::LastAccessed)
            .limit(limit as u64)
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn delete_old_entries(&self, days_old: i64) -> Result<u64> {
        use chrono::Duration;
        let cutoff = (Utc::now() - Duration::days(days_old)).naive_utc();

        let result = CacheEntry::delete_many()
            .filter(cache_entries::Column::LastAccessed.lt(cutoff))
            .exec(self.base.db.as_ref())
            .await?;

        Ok(result.rows_affected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection;
    use chrono::Utc;
    use sea_orm::Database;

    async fn create_test_db() -> Arc<DatabaseConnection> {
        use sea_orm::{ConnectionTrait, Statement};

        // Create in-memory SQLite database without foreign keys for simplified testing
        let db_conn = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");

        // Create only the tables we need for chunk merging tests (without FK constraints)
        db_conn
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "CREATE TABLE cache_entries (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    source_id TEXT NOT NULL,
                    media_id TEXT NOT NULL,
                    quality TEXT NOT NULL,
                    original_url TEXT NOT NULL,
                    file_path TEXT NOT NULL,
                    file_size INTEGER NOT NULL DEFAULT 0,
                    expected_total_size INTEGER,
                    downloaded_bytes INTEGER NOT NULL DEFAULT 0,
                    is_complete BOOLEAN NOT NULL DEFAULT 0,
                    priority INTEGER NOT NULL DEFAULT 0,
                    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    last_accessed TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    last_modified TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    access_count INTEGER NOT NULL DEFAULT 0,
                    mime_type TEXT,
                    video_codec TEXT,
                    audio_codec TEXT,
                    container TEXT,
                    resolution_width INTEGER,
                    resolution_height INTEGER,
                    bitrate INTEGER,
                    duration_secs REAL,
                    etag TEXT,
                    expires_at TIMESTAMP
                )",
            ))
            .await
            .expect("Failed to create cache_entries table");

        db_conn
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "CREATE TABLE cache_chunks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    cache_entry_id INTEGER NOT NULL,
                    start_byte INTEGER NOT NULL,
                    end_byte INTEGER NOT NULL,
                    downloaded_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
                )",
            ))
            .await
            .expect("Failed to create cache_chunks table");

        Arc::new(db_conn)
    }

    async fn create_test_cache_entry(
        repo: &CacheRepositoryImpl,
        media_id_suffix: i32,
    ) -> CacheEntryModel {
        // Foreign keys are disabled for tests, so we can create entries without parent records
        let entry = CacheEntryModel {
            id: 0, // Will be auto-generated by database
            source_id: "test".to_string(),
            media_id: format!("media_{}", media_id_suffix),
            quality: "1080p".to_string(),
            original_url: "http://test.com/video.mp4".to_string(),
            file_path: format!("/tmp/test_{}.cache", media_id_suffix),
            file_size: 0,
            expected_total_size: Some(10 * 1024 * 1024), // 10MB
            downloaded_bytes: 0,
            is_complete: false,
            priority: 0,
            created_at: Utc::now().naive_utc(),
            last_accessed: Utc::now().naive_utc(),
            last_modified: Utc::now().naive_utc(),
            access_count: 0,
            mime_type: Some("video/mp4".to_string()),
            video_codec: None,
            audio_codec: None,
            container: None,
            resolution_width: None,
            resolution_height: None,
            bitrate: None,
            duration_secs: None,
            etag: None,
            expires_at: None,
        };

        repo.insert_cache_entry(entry)
            .await
            .expect("Failed to insert test cache entry")
    }

    fn create_chunk(entry_id: i32, start: i64, end: i64) -> CacheChunkModel {
        CacheChunkModel {
            id: 0,
            cache_entry_id: entry_id,
            start_byte: start,
            end_byte: end,
            downloaded_at: Utc::now().naive_utc(),
        }
    }

    #[tokio::test]
    async fn test_chunk_merging_sequential_forward() {
        let db = create_test_db().await;
        let repo = CacheRepositoryImpl::new(db);
        let entry = create_test_cache_entry(&repo, 1).await;

        // Add chunks sequentially: 0-999, 1000-1999, 2000-2999
        repo.add_cache_chunk(create_chunk(entry.id, 0, 999))
            .await
            .expect("Failed to add chunk 0");
        repo.add_cache_chunk(create_chunk(entry.id, 1000, 1999))
            .await
            .expect("Failed to add chunk 1");
        repo.add_cache_chunk(create_chunk(entry.id, 2000, 2999))
            .await
            .expect("Failed to add chunk 2");

        // Should be merged into single chunk
        let chunks = repo
            .get_chunks_for_entry(entry.id)
            .await
            .expect("Failed to get chunks");

        assert_eq!(chunks.len(), 1, "Chunks should be merged into one");
        assert_eq!(chunks[0].start_byte, 0);
        assert_eq!(chunks[0].end_byte, 2999);
    }

    #[tokio::test]
    async fn test_chunk_merging_sequential_backward() {
        let db = create_test_db().await;
        let repo = CacheRepositoryImpl::new(db);

        let entry = create_test_cache_entry(&repo, 2).await;

        // Add chunks in reverse order: 2000-2999, 1000-1999, 0-999
        repo.add_cache_chunk(create_chunk(entry.id, 2000, 2999))
            .await
            .expect("Failed to add chunk 2");
        repo.add_cache_chunk(create_chunk(entry.id, 1000, 1999))
            .await
            .expect("Failed to add chunk 1");
        repo.add_cache_chunk(create_chunk(entry.id, 0, 999))
            .await
            .expect("Failed to add chunk 0");

        // Should be merged into single chunk
        let chunks = repo
            .get_chunks_for_entry(entry.id)
            .await
            .expect("Failed to get chunks");
        assert_eq!(chunks.len(), 1, "Chunks should be merged into one");
        assert_eq!(chunks[0].start_byte, 0);
        assert_eq!(chunks[0].end_byte, 2999);
    }

    #[tokio::test]
    async fn test_chunk_merging_gap_filling() {
        let db = create_test_db().await;
        let repo = CacheRepositoryImpl::new(db);

        let entry = create_test_cache_entry(&repo, 3).await;

        // Add chunks with gap: 0-999, 2000-2999, then fill gap with 1000-1999
        repo.add_cache_chunk(create_chunk(entry.id, 0, 999))
            .await
            .expect("Failed to add chunk 0");
        repo.add_cache_chunk(create_chunk(entry.id, 2000, 2999))
            .await
            .expect("Failed to add chunk 2");

        // Should have 2 separate chunks
        let chunks = repo
            .get_chunks_for_entry(entry.id)
            .await
            .expect("Failed to get chunks");
        assert_eq!(chunks.len(), 2, "Should have 2 separate chunks with gap");

        // Fill the gap
        repo.add_cache_chunk(create_chunk(entry.id, 1000, 1999))
            .await
            .expect("Failed to add chunk 1");

        // Should be merged into single chunk
        let chunks = repo
            .get_chunks_for_entry(entry.id)
            .await
            .expect("Failed to get chunks");
        assert_eq!(chunks.len(), 1, "Gap should be filled, chunks merged");
        assert_eq!(chunks[0].start_byte, 0);
        assert_eq!(chunks[0].end_byte, 2999);
    }

    #[tokio::test]
    async fn test_chunk_non_adjacent_remain_separate() {
        let db = create_test_db().await;
        let repo = CacheRepositoryImpl::new(db);

        let entry = create_test_cache_entry(&repo, 4).await;

        // Add non-adjacent chunks with gaps: 0-999, 2000-2999, 4000-4999
        repo.add_cache_chunk(create_chunk(entry.id, 0, 999))
            .await
            .expect("Failed to add chunk 0");
        repo.add_cache_chunk(create_chunk(entry.id, 2000, 2999))
            .await
            .expect("Failed to add chunk 1");
        repo.add_cache_chunk(create_chunk(entry.id, 4000, 4999))
            .await
            .expect("Failed to add chunk 2");

        // Should remain as 3 separate chunks
        let chunks = repo
            .get_chunks_for_entry(entry.id)
            .await
            .expect("Failed to get chunks");
        assert_eq!(
            chunks.len(),
            3,
            "Non-adjacent chunks should remain separate"
        );
        assert_eq!(chunks[0].start_byte, 0);
        assert_eq!(chunks[0].end_byte, 999);
        assert_eq!(chunks[1].start_byte, 2000);
        assert_eq!(chunks[1].end_byte, 2999);
        assert_eq!(chunks[2].start_byte, 4000);
        assert_eq!(chunks[2].end_byte, 4999);
    }

    #[tokio::test]
    async fn test_downloaded_bytes_with_merged_chunks() {
        let db = create_test_db().await;
        let repo = CacheRepositoryImpl::new(db);

        let entry = create_test_cache_entry(&repo, 5).await;

        // Add sequential chunks that will be merged
        repo.add_cache_chunk(create_chunk(entry.id, 0, 999))
            .await
            .expect("Failed to add chunk");
        repo.add_cache_chunk(create_chunk(entry.id, 1000, 1999))
            .await
            .expect("Failed to add chunk");
        repo.add_cache_chunk(create_chunk(entry.id, 2000, 2999))
            .await
            .expect("Failed to add chunk");

        // Check downloaded bytes (should be 3000 bytes: 0-2999 inclusive)
        let downloaded = repo
            .get_downloaded_bytes(entry.id)
            .await
            .expect("Failed to get downloaded bytes");
        assert_eq!(downloaded, 3000, "Downloaded bytes should be correct");

        // Check chunk count (should be 1 after merging)
        let count = repo
            .get_chunk_count(entry.id)
            .await
            .expect("Failed to get chunk count");
        assert_eq!(count, 1, "Should have 1 merged chunk");
    }

    #[tokio::test]
    async fn test_chunk_merging_random_order() {
        let db = create_test_db().await;
        let repo = CacheRepositoryImpl::new(db);

        let entry = create_test_cache_entry(&repo, 6).await;

        // Add chunks in random order: 1, 3, 0, 2, 4
        repo.add_cache_chunk(create_chunk(entry.id, 1000, 1999))
            .await
            .expect("Failed to add chunk");
        repo.add_cache_chunk(create_chunk(entry.id, 3000, 3999))
            .await
            .expect("Failed to add chunk");
        repo.add_cache_chunk(create_chunk(entry.id, 0, 999))
            .await
            .expect("Failed to add chunk");
        repo.add_cache_chunk(create_chunk(entry.id, 2000, 2999))
            .await
            .expect("Failed to add chunk");
        repo.add_cache_chunk(create_chunk(entry.id, 4000, 4999))
            .await
            .expect("Failed to add chunk");

        // Should be merged into single chunk
        let chunks = repo
            .get_chunks_for_entry(entry.id)
            .await
            .expect("Failed to get chunks");
        assert_eq!(
            chunks.len(),
            1,
            "All adjacent chunks should be merged regardless of order"
        );
        assert_eq!(chunks[0].start_byte, 0);
        assert_eq!(chunks[0].end_byte, 4999);
    }
}
