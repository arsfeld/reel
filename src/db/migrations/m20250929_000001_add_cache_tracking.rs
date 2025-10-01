use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create cache_entries table - Main cache entry tracking
        manager
            .create_table(
                Table::create()
                    .table(CacheEntries::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CacheEntries::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CacheEntries::SourceId).string().not_null())
                    .col(ColumnDef::new(CacheEntries::MediaId).string().not_null())
                    .col(ColumnDef::new(CacheEntries::Quality).string().not_null())
                    .col(
                        ColumnDef::new(CacheEntries::OriginalUrl)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CacheEntries::FilePath).string().not_null())
                    .col(
                        ColumnDef::new(CacheEntries::FileSize)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(CacheEntries::ExpectedTotalSize).big_integer())
                    .col(
                        ColumnDef::new(CacheEntries::DownloadedBytes)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(CacheEntries::IsComplete)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(CacheEntries::Priority)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(CacheEntries::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CacheEntries::LastAccessed)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CacheEntries::LastModified)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CacheEntries::AccessCount)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(CacheEntries::MimeType).string())
                    .col(ColumnDef::new(CacheEntries::VideoCodec).string())
                    .col(ColumnDef::new(CacheEntries::AudioCodec).string())
                    .col(ColumnDef::new(CacheEntries::Container).string())
                    .col(ColumnDef::new(CacheEntries::ResolutionWidth).integer())
                    .col(ColumnDef::new(CacheEntries::ResolutionHeight).integer())
                    .col(ColumnDef::new(CacheEntries::Bitrate).big_integer())
                    .col(ColumnDef::new(CacheEntries::DurationSecs).double())
                    .col(ColumnDef::new(CacheEntries::Etag).string())
                    .col(ColumnDef::new(CacheEntries::ExpiresAt).timestamp())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cache_entries_media")
                            .from(CacheEntries::Table, CacheEntries::MediaId)
                            .to(MediaItems::Table, MediaItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cache_entries_source")
                            .from(CacheEntries::Table, CacheEntries::SourceId)
                            .to(Sources::Table, Sources::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes for cache_entries
        manager
            .create_index(
                Index::create()
                    .name("idx_cache_entries_source_media_quality")
                    .table(CacheEntries::Table)
                    .col(CacheEntries::SourceId)
                    .col(CacheEntries::MediaId)
                    .col(CacheEntries::Quality)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cache_entries_media")
                    .table(CacheEntries::Table)
                    .col(CacheEntries::MediaId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cache_entries_source")
                    .table(CacheEntries::Table)
                    .col(CacheEntries::SourceId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cache_entries_last_accessed")
                    .table(CacheEntries::Table)
                    .col(CacheEntries::LastAccessed)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cache_entries_priority")
                    .table(CacheEntries::Table)
                    .col(CacheEntries::Priority)
                    .col(CacheEntries::LastAccessed)
                    .to_owned(),
            )
            .await?;

        // Create cache_chunks table - Track downloaded byte ranges for partial downloads
        manager
            .create_table(
                Table::create()
                    .table(CacheChunks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CacheChunks::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CacheChunks::CacheEntryId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CacheChunks::StartByte)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CacheChunks::EndByte)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CacheChunks::DownloadedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cache_chunks_entry")
                            .from(CacheChunks::Table, CacheChunks::CacheEntryId)
                            .to(CacheEntries::Table, CacheEntries::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index for cache_chunks
        manager
            .create_index(
                Index::create()
                    .name("idx_cache_chunks_entry")
                    .table(CacheChunks::Table)
                    .col(CacheChunks::CacheEntryId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_cache_chunks_range")
                    .table(CacheChunks::Table)
                    .col(CacheChunks::CacheEntryId)
                    .col(CacheChunks::StartByte)
                    .col(CacheChunks::EndByte)
                    .to_owned(),
            )
            .await?;

        // Create cache_download_queue table - Track download priorities and user preferences
        manager
            .create_table(
                Table::create()
                    .table(CacheDownloadQueue::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CacheDownloadQueue::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CacheDownloadQueue::MediaId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CacheDownloadQueue::SourceId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CacheDownloadQueue::Quality)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CacheDownloadQueue::Priority)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(CacheDownloadQueue::Status)
                            .string()
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(CacheDownloadQueue::RetryCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(CacheDownloadQueue::LastRetryAt).timestamp())
                    .col(
                        ColumnDef::new(CacheDownloadQueue::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(CacheDownloadQueue::ScheduledFor).timestamp())
                    .col(ColumnDef::new(CacheDownloadQueue::ExpiresAt).timestamp())
                    .col(
                        ColumnDef::new(CacheDownloadQueue::UserRequested)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_download_queue_media")
                            .from(CacheDownloadQueue::Table, CacheDownloadQueue::MediaId)
                            .to(MediaItems::Table, MediaItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_download_queue_source")
                            .from(CacheDownloadQueue::Table, CacheDownloadQueue::SourceId)
                            .to(Sources::Table, Sources::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes for cache_download_queue
        manager
            .create_index(
                Index::create()
                    .name("idx_download_queue_status_priority")
                    .table(CacheDownloadQueue::Table)
                    .col(CacheDownloadQueue::Status)
                    .col(CacheDownloadQueue::Priority)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_download_queue_media_source")
                    .table(CacheDownloadQueue::Table)
                    .col(CacheDownloadQueue::MediaId)
                    .col(CacheDownloadQueue::SourceId)
                    .to_owned(),
            )
            .await?;

        // Create cache_quality_variants table - Track available quality options
        manager
            .create_table(
                Table::create()
                    .table(CacheQualityVariants::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CacheQualityVariants::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CacheQualityVariants::MediaId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CacheQualityVariants::SourceId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CacheQualityVariants::Quality)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CacheQualityVariants::ResolutionWidth).integer())
                    .col(ColumnDef::new(CacheQualityVariants::ResolutionHeight).integer())
                    .col(ColumnDef::new(CacheQualityVariants::Bitrate).big_integer())
                    .col(ColumnDef::new(CacheQualityVariants::FileSize).big_integer())
                    .col(ColumnDef::new(CacheQualityVariants::Container).string())
                    .col(ColumnDef::new(CacheQualityVariants::VideoCodec).string())
                    .col(ColumnDef::new(CacheQualityVariants::AudioCodec).string())
                    .col(ColumnDef::new(CacheQualityVariants::StreamUrl).string())
                    .col(
                        ColumnDef::new(CacheQualityVariants::IsDefault)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(CacheQualityVariants::DiscoveredAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_quality_variants_media")
                            .from(CacheQualityVariants::Table, CacheQualityVariants::MediaId)
                            .to(MediaItems::Table, MediaItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_quality_variants_source")
                            .from(CacheQualityVariants::Table, CacheQualityVariants::SourceId)
                            .to(Sources::Table, Sources::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes for cache_quality_variants
        manager
            .create_index(
                Index::create()
                    .name("idx_quality_variants_media_source")
                    .table(CacheQualityVariants::Table)
                    .col(CacheQualityVariants::MediaId)
                    .col(CacheQualityVariants::SourceId)
                    .to_owned(),
            )
            .await?;

        // Create cache_statistics table - Track cache usage statistics
        manager
            .create_table(
                Table::create()
                    .table(CacheStatistics::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CacheStatistics::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CacheStatistics::TotalSize)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(CacheStatistics::FileCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(CacheStatistics::MaxSizeBytes)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CacheStatistics::MaxFileCount)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CacheStatistics::HitCount)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(CacheStatistics::MissCount)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(CacheStatistics::BytesServed)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(CacheStatistics::BytesDownloaded)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(CacheStatistics::LastCleanupAt).timestamp())
                    .col(
                        ColumnDef::new(CacheStatistics::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create cache_headers table - Store HTTP headers for cache validation
        manager
            .create_table(
                Table::create()
                    .table(CacheHeaders::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CacheHeaders::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CacheHeaders::CacheEntryId)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CacheHeaders::HeaderName).string().not_null())
                    .col(
                        ColumnDef::new(CacheHeaders::HeaderValue)
                            .string()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_cache_headers_entry")
                            .from(CacheHeaders::Table, CacheHeaders::CacheEntryId)
                            .to(CacheEntries::Table, CacheEntries::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index for cache_headers
        manager
            .create_index(
                Index::create()
                    .name("idx_cache_headers_entry")
                    .table(CacheHeaders::Table)
                    .col(CacheHeaders::CacheEntryId)
                    .to_owned(),
            )
            .await?;

        // Note: Triggers and initial data would need to be added via direct database access
        // after migration, as SeaORM migrations don't support raw SQL execution.
        // The application code will need to handle cache statistics updates.

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop tables in reverse order
        manager
            .drop_table(Table::drop().table(CacheHeaders::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CacheStatistics::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CacheQualityVariants::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CacheDownloadQueue::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CacheChunks::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CacheEntries::Table).to_owned())
            .await?;

        Ok(())
    }
}

// Define table and column identifiers
#[derive(Iden)]
enum CacheEntries {
    Table,
    Id,
    SourceId,
    MediaId,
    Quality,
    OriginalUrl,
    FilePath,
    FileSize,
    ExpectedTotalSize,
    DownloadedBytes,
    IsComplete,
    Priority,
    CreatedAt,
    LastAccessed,
    LastModified,
    AccessCount,
    MimeType,
    VideoCodec,
    AudioCodec,
    Container,
    ResolutionWidth,
    ResolutionHeight,
    Bitrate,
    DurationSecs,
    Etag,
    ExpiresAt,
}

#[derive(Iden)]
enum CacheChunks {
    Table,
    Id,
    CacheEntryId,
    StartByte,
    EndByte,
    DownloadedAt,
}

#[derive(Iden)]
enum CacheDownloadQueue {
    Table,
    Id,
    MediaId,
    SourceId,
    Quality,
    Priority,
    Status,
    RetryCount,
    LastRetryAt,
    CreatedAt,
    ScheduledFor,
    ExpiresAt,
    UserRequested,
}

#[derive(Iden)]
enum CacheQualityVariants {
    Table,
    Id,
    MediaId,
    SourceId,
    Quality,
    ResolutionWidth,
    ResolutionHeight,
    Bitrate,
    FileSize,
    Container,
    VideoCodec,
    AudioCodec,
    StreamUrl,
    IsDefault,
    DiscoveredAt,
}

#[derive(Iden)]
enum CacheStatistics {
    Table,
    Id,
    TotalSize,
    FileCount,
    MaxSizeBytes,
    MaxFileCount,
    HitCount,
    MissCount,
    BytesServed,
    BytesDownloaded,
    LastCleanupAt,
    UpdatedAt,
}

#[derive(Iden)]
enum CacheHeaders {
    Table,
    Id,
    CacheEntryId,
    HeaderName,
    HeaderValue,
}

// Reference to existing tables
#[derive(Iden)]
enum MediaItems {
    Table,
    Id,
}

#[derive(Iden)]
enum Sources {
    Table,
    Id,
}
