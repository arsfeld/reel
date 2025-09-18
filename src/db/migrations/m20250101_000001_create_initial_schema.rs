use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
#[allow(dead_code)] // Used by SeaORM migration system
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create sources table
        manager
            .create_table(
                Table::create()
                    .table(Sources::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Sources::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Sources::Name).string().not_null())
                    .col(ColumnDef::new(Sources::SourceType).string().not_null())
                    .col(ColumnDef::new(Sources::AuthProviderId).string())
                    .col(ColumnDef::new(Sources::ConnectionUrl).string())
                    .col(ColumnDef::new(Sources::IsOnline).boolean().default(false))
                    .col(ColumnDef::new(Sources::LastSync).timestamp())
                    .col(
                        ColumnDef::new(Sources::CreatedAt)
                            .timestamp()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Sources::UpdatedAt)
                            .timestamp()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create libraries table
        manager
            .create_table(
                Table::create()
                    .table(Libraries::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Libraries::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Libraries::SourceId).string().not_null())
                    .col(ColumnDef::new(Libraries::Title).string().not_null())
                    .col(ColumnDef::new(Libraries::LibraryType).string().not_null())
                    .col(ColumnDef::new(Libraries::Icon).string())
                    .col(ColumnDef::new(Libraries::ItemCount).integer().default(0))
                    .col(
                        ColumnDef::new(Libraries::CreatedAt)
                            .timestamp()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Libraries::UpdatedAt)
                            .timestamp()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_libraries_source")
                            .from(Libraries::Table, Libraries::SourceId)
                            .to(Sources::Table, Sources::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create media_items table
        manager
            .create_table(
                Table::create()
                    .table(MediaItems::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MediaItems::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MediaItems::LibraryId).string().not_null())
                    .col(ColumnDef::new(MediaItems::SourceId).string().not_null())
                    .col(ColumnDef::new(MediaItems::MediaType).string().not_null())
                    .col(ColumnDef::new(MediaItems::Title).string().not_null())
                    .col(ColumnDef::new(MediaItems::SortTitle).string())
                    .col(ColumnDef::new(MediaItems::Year).integer())
                    .col(ColumnDef::new(MediaItems::DurationMs).big_integer())
                    .col(ColumnDef::new(MediaItems::Rating).float())
                    .col(ColumnDef::new(MediaItems::PosterUrl).string())
                    .col(ColumnDef::new(MediaItems::BackdropUrl).string())
                    .col(ColumnDef::new(MediaItems::Overview).text())
                    .col(ColumnDef::new(MediaItems::Genres).json())
                    .col(ColumnDef::new(MediaItems::AddedAt).timestamp())
                    .col(
                        ColumnDef::new(MediaItems::UpdatedAt)
                            .timestamp()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(MediaItems::Metadata).json())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_items_library")
                            .from(MediaItems::Table, MediaItems::LibraryId)
                            .to(Libraries::Table, Libraries::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_items_source")
                            .from(MediaItems::Table, MediaItems::SourceId)
                            .to(Sources::Table, Sources::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create playback_progress table
        manager
            .create_table(
                Table::create()
                    .table(PlaybackProgress::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlaybackProgress::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PlaybackProgress::MediaId)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PlaybackProgress::UserId).string())
                    .col(
                        ColumnDef::new(PlaybackProgress::PositionMs)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlaybackProgress::DurationMs)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlaybackProgress::Watched)
                            .boolean()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(PlaybackProgress::ViewCount)
                            .integer()
                            .default(0),
                    )
                    .col(ColumnDef::new(PlaybackProgress::LastWatchedAt).timestamp())
                    .col(
                        ColumnDef::new(PlaybackProgress::UpdatedAt)
                            .timestamp()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_playback_progress_media")
                            .from(PlaybackProgress::Table, PlaybackProgress::MediaId)
                            .to(MediaItems::Table, MediaItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create unique index for playback_progress
        manager
            .create_index(
                Index::create()
                    .name("idx_playback_progress_unique")
                    .table(PlaybackProgress::Table)
                    .col(PlaybackProgress::MediaId)
                    .col(PlaybackProgress::UserId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Create sync_status table
        manager
            .create_table(
                Table::create()
                    .table(SyncStatus::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SyncStatus::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SyncStatus::SourceId).string().not_null())
                    .col(ColumnDef::new(SyncStatus::SyncType).string().not_null())
                    .col(ColumnDef::new(SyncStatus::Status).string().not_null())
                    .col(ColumnDef::new(SyncStatus::StartedAt).timestamp())
                    .col(ColumnDef::new(SyncStatus::CompletedAt).timestamp())
                    .col(ColumnDef::new(SyncStatus::ItemsSynced).integer().default(0))
                    .col(ColumnDef::new(SyncStatus::ErrorMessage).text())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_sync_status_source")
                            .from(SyncStatus::Table, SyncStatus::SourceId)
                            .to(Sources::Table, Sources::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create offline_content table
        manager
            .create_table(
                Table::create()
                    .table(OfflineContent::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OfflineContent::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(OfflineContent::MediaId).string().not_null())
                    .col(ColumnDef::new(OfflineContent::FilePath).string().not_null())
                    .col(ColumnDef::new(OfflineContent::FileSizeBytes).big_integer())
                    .col(ColumnDef::new(OfflineContent::Quality).string())
                    .col(
                        ColumnDef::new(OfflineContent::DownloadedAt)
                            .timestamp()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(OfflineContent::LastAccessed).timestamp())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_offline_content_media")
                            .from(OfflineContent::Table, OfflineContent::MediaId)
                            .to(MediaItems::Table, MediaItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes for performance
        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_library")
                    .table(MediaItems::Table)
                    .col(MediaItems::LibraryId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_source")
                    .table(MediaItems::Table)
                    .col(MediaItems::SourceId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_type")
                    .table(MediaItems::Table)
                    .col(MediaItems::MediaType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_title")
                    .table(MediaItems::Table)
                    .col(MediaItems::SortTitle)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_sync_status_source")
                    .table(SyncStatus::Table)
                    .col(SyncStatus::SourceId)
                    .col(SyncStatus::Status)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop tables in reverse order of creation (due to foreign keys)
        manager
            .drop_table(Table::drop().table(OfflineContent::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SyncStatus::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(PlaybackProgress::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(MediaItems::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Libraries::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Sources::Table).to_owned())
            .await?;

        Ok(())
    }
}

// Define table identifiers
#[derive(Iden)]
enum Sources {
    Table,
    Id,
    Name,
    SourceType,
    AuthProviderId,
    ConnectionUrl,
    IsOnline,
    LastSync,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Libraries {
    Table,
    Id,
    SourceId,
    Title,
    LibraryType,
    Icon,
    ItemCount,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum MediaItems {
    Table,
    Id,
    LibraryId,
    SourceId,
    MediaType,
    Title,
    SortTitle,
    Year,
    DurationMs,
    Rating,
    PosterUrl,
    BackdropUrl,
    Overview,
    Genres,
    AddedAt,
    UpdatedAt,
    Metadata,
}

#[derive(Iden)]
enum PlaybackProgress {
    Table,
    Id,
    MediaId,
    UserId,
    PositionMs,
    DurationMs,
    Watched,
    ViewCount,
    LastWatchedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum SyncStatus {
    Table,
    Id,
    SourceId,
    SyncType,
    Status,
    StartedAt,
    CompletedAt,
    ItemsSynced,
    ErrorMessage,
}

#[derive(Iden)]
enum OfflineContent {
    Table,
    Id,
    MediaId,
    FilePath,
    FileSizeBytes,
    Quality,
    DownloadedAt,
    LastAccessed,
}
