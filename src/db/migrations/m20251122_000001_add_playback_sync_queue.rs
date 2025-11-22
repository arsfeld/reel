use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create playback_sync_queue table
        manager
            .create_table(
                Table::create()
                    .table(PlaybackSyncQueue::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlaybackSyncQueue::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PlaybackSyncQueue::MediaItemId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlaybackSyncQueue::SourceId)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PlaybackSyncQueue::UserId).string())
                    .col(
                        ColumnDef::new(PlaybackSyncQueue::ChangeType)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PlaybackSyncQueue::PositionMs).big_integer())
                    .col(ColumnDef::new(PlaybackSyncQueue::Completed).boolean())
                    .col(
                        ColumnDef::new(PlaybackSyncQueue::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(PlaybackSyncQueue::LastAttemptAt).timestamp())
                    .col(
                        ColumnDef::new(PlaybackSyncQueue::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(PlaybackSyncQueue::ErrorMessage).string())
                    .col(
                        ColumnDef::new(PlaybackSyncQueue::Status)
                            .string()
                            .not_null()
                            .default("pending"),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_playback_sync_queue_media_item")
                            .from(PlaybackSyncQueue::Table, PlaybackSyncQueue::MediaItemId)
                            .to(MediaItems::Table, MediaItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_playback_sync_queue_source")
                            .from(PlaybackSyncQueue::Table, PlaybackSyncQueue::SourceId)
                            .to(Sources::Table, Sources::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes for efficient querying
        manager
            .create_index(
                Index::create()
                    .name("idx_playback_sync_queue_media_item")
                    .table(PlaybackSyncQueue::Table)
                    .col(PlaybackSyncQueue::MediaItemId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_playback_sync_queue_source")
                    .table(PlaybackSyncQueue::Table)
                    .col(PlaybackSyncQueue::SourceId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_playback_sync_queue_status")
                    .table(PlaybackSyncQueue::Table)
                    .col(PlaybackSyncQueue::Status)
                    .to_owned(),
            )
            .await?;

        // Composite index for efficient pending queue queries
        manager
            .create_index(
                Index::create()
                    .name("idx_playback_sync_queue_status_created")
                    .table(PlaybackSyncQueue::Table)
                    .col(PlaybackSyncQueue::Status)
                    .col(PlaybackSyncQueue::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PlaybackSyncQueue::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum PlaybackSyncQueue {
    Table,
    Id,
    MediaItemId,
    SourceId,
    UserId,
    ChangeType,
    PositionMs,
    Completed,
    CreatedAt,
    LastAttemptAt,
    AttemptCount,
    ErrorMessage,
    Status,
}

#[derive(DeriveIden)]
enum MediaItems {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Sources {
    Table,
    Id,
}
