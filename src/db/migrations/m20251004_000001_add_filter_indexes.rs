use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
#[allow(dead_code)] // Used by SeaORM migration system
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add single-column indexes for filtering

        // Index on year for year filtering
        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_year")
                    .table(MediaItems::Table)
                    .col(MediaItems::Year)
                    .to_owned(),
            )
            .await?;

        // Index on rating for rating filtering
        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_rating")
                    .table(MediaItems::Table)
                    .col(MediaItems::Rating)
                    .to_owned(),
            )
            .await?;

        // Index on added_at for "Recently Added" sorting
        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_added_at")
                    .table(MediaItems::Table)
                    .col(MediaItems::AddedAt)
                    .to_owned(),
            )
            .await?;

        // Index on duration_ms for duration filtering/sorting
        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_duration")
                    .table(MediaItems::Table)
                    .col(MediaItems::DurationMs)
                    .to_owned(),
            )
            .await?;

        // Add composite indexes for common filter+sort combinations

        // Library + Sort Title (most common: browsing a library sorted by title)
        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_library_sort_title")
                    .table(MediaItems::Table)
                    .col(MediaItems::LibraryId)
                    .col(MediaItems::SortTitle)
                    .to_owned(),
            )
            .await?;

        // Library + Year (browsing a library sorted by year)
        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_library_year")
                    .table(MediaItems::Table)
                    .col(MediaItems::LibraryId)
                    .col(MediaItems::Year)
                    .to_owned(),
            )
            .await?;

        // Library + Rating (browsing a library sorted by rating)
        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_library_rating")
                    .table(MediaItems::Table)
                    .col(MediaItems::LibraryId)
                    .col(MediaItems::Rating)
                    .to_owned(),
            )
            .await?;

        // Library + Added At (browsing a library sorted by recently added)
        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_library_added_at")
                    .table(MediaItems::Table)
                    .col(MediaItems::LibraryId)
                    .col(MediaItems::AddedAt)
                    .to_owned(),
            )
            .await?;

        // Library + Media Type + Sort Title (filtered by type in library, sorted by title)
        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_library_type_title")
                    .table(MediaItems::Table)
                    .col(MediaItems::LibraryId)
                    .col(MediaItems::MediaType)
                    .col(MediaItems::SortTitle)
                    .to_owned(),
            )
            .await?;

        // Parent ID + Season + Episode (for TV show episode queries)
        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_parent_season_episode")
                    .table(MediaItems::Table)
                    .col(MediaItems::ParentId)
                    .col(MediaItems::SeasonNumber)
                    .col(MediaItems::EpisodeNumber)
                    .to_owned(),
            )
            .await?;

        // Index on last_watched_at in playback_progress for "Last Watched" sorting
        manager
            .create_index(
                Index::create()
                    .name("idx_playback_progress_last_watched")
                    .table(PlaybackProgress::Table)
                    .col(PlaybackProgress::LastWatchedAt)
                    .to_owned(),
            )
            .await?;

        // Media ID + Watched for filtering watched/unwatched items
        manager
            .create_index(
                Index::create()
                    .name("idx_playback_progress_media_watched")
                    .table(PlaybackProgress::Table)
                    .col(PlaybackProgress::MediaId)
                    .col(PlaybackProgress::Watched)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop all indexes in reverse order
        manager
            .drop_index(
                Index::drop()
                    .name("idx_playback_progress_media_watched")
                    .table(PlaybackProgress::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_playback_progress_last_watched")
                    .table(PlaybackProgress::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_media_items_parent_season_episode")
                    .table(MediaItems::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_media_items_library_type_title")
                    .table(MediaItems::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_media_items_library_added_at")
                    .table(MediaItems::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_media_items_library_rating")
                    .table(MediaItems::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_media_items_library_year")
                    .table(MediaItems::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_media_items_library_sort_title")
                    .table(MediaItems::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_media_items_duration")
                    .table(MediaItems::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_media_items_added_at")
                    .table(MediaItems::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_media_items_rating")
                    .table(MediaItems::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_media_items_year")
                    .table(MediaItems::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

// Define table and column identifiers
#[derive(Iden)]
enum MediaItems {
    Table,
    LibraryId,
    MediaType,
    SortTitle,
    Year,
    Rating,
    AddedAt,
    DurationMs,
    ParentId,
    SeasonNumber,
    EpisodeNumber,
}

#[derive(Iden)]
enum PlaybackProgress {
    Table,
    MediaId,
    LastWatchedAt,
    Watched,
}
