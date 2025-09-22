use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
#[allow(dead_code)] // Used by SeaORM migration system
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add play_queue_id to playback_progress table for Plex PlayQueue persistence
        manager
            .alter_table(
                Table::alter()
                    .table(PlaybackProgress::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(PlaybackProgress::PlayQueueId)
                            .big_integer()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Add play_queue_version for PlayQueue sync tracking
        manager
            .alter_table(
                Table::alter()
                    .table(PlaybackProgress::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(PlaybackProgress::PlayQueueVersion)
                            .integer()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Add play_queue_item_id to track current position in queue
        manager
            .alter_table(
                Table::alter()
                    .table(PlaybackProgress::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(PlaybackProgress::PlayQueueItemId)
                            .big_integer()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Add source_id to link PlayQueue to specific backend
        manager
            .alter_table(
                Table::alter()
                    .table(PlaybackProgress::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(PlaybackProgress::SourceId).integer().null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Add index for faster PlayQueue lookups
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_playback_playqueue")
                    .table(PlaybackProgress::Table)
                    .col(PlaybackProgress::PlayQueueId)
                    .col(PlaybackProgress::SourceId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop the index
        manager
            .drop_index(
                Index::drop()
                    .name("idx_playback_playqueue")
                    .table(PlaybackProgress::Table)
                    .to_owned(),
            )
            .await?;

        // Remove the added columns
        manager
            .alter_table(
                Table::alter()
                    .table(PlaybackProgress::Table)
                    .drop_column(PlaybackProgress::PlayQueueId)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(PlaybackProgress::Table)
                    .drop_column(PlaybackProgress::PlayQueueVersion)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(PlaybackProgress::Table)
                    .drop_column(PlaybackProgress::PlayQueueItemId)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(PlaybackProgress::Table)
                    .drop_column(PlaybackProgress::SourceId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum PlaybackProgress {
    Table,
    PlayQueueId,
    PlayQueueVersion,
    PlayQueueItemId,
    SourceId,
}
