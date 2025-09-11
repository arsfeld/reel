use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Check if columns already exist by attempting to query them
        // This is a workaround for SQLite not properly handling add_column_if_not_exists
        let _db = manager.get_connection();

        // Try to add parent_id column - will fail silently if it exists
        match manager
            .alter_table(
                Table::alter()
                    .table(MediaItems::Table)
                    .add_column(ColumnDef::new(MediaItems::ParentId).string().null())
                    .to_owned(),
            )
            .await
        {
            Ok(_) => {}
            Err(e) if e.to_string().contains("duplicate column name") => {
                // Column already exists, continue
            }
            Err(e) => return Err(e),
        }

        // Try to add season_number column - will fail silently if it exists
        match manager
            .alter_table(
                Table::alter()
                    .table(MediaItems::Table)
                    .add_column(ColumnDef::new(MediaItems::SeasonNumber).integer().null())
                    .to_owned(),
            )
            .await
        {
            Ok(_) => {}
            Err(e) if e.to_string().contains("duplicate column name") => {
                // Column already exists, continue
            }
            Err(e) => return Err(e),
        }

        // Try to add episode_number column - will fail silently if it exists
        match manager
            .alter_table(
                Table::alter()
                    .table(MediaItems::Table)
                    .add_column(ColumnDef::new(MediaItems::EpisodeNumber).integer().null())
                    .to_owned(),
            )
            .await
        {
            Ok(_) => {}
            Err(e) if e.to_string().contains("duplicate column name") => {
                // Column already exists, continue
            }
            Err(e) => return Err(e),
        }

        // Note: SQLite doesn't support adding foreign keys to existing tables
        // The foreign key constraint for parent_id would need to be handled
        // by recreating the table, which is too complex for this migration.
        // We'll rely on application-level referential integrity.

        // Create index for parent_id for fast episode lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_parent")
                    .table(MediaItems::Table)
                    .col(MediaItems::ParentId)
                    .to_owned(),
            )
            .await?;

        // Create composite index for parent_id + season_number for season queries
        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_parent_season")
                    .table(MediaItems::Table)
                    .col(MediaItems::ParentId)
                    .col(MediaItems::SeasonNumber)
                    .to_owned(),
            )
            .await?;

        // Create unique index for episode identification
        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_episode_unique")
                    .table(MediaItems::Table)
                    .col(MediaItems::ParentId)
                    .col(MediaItems::SeasonNumber)
                    .col(MediaItems::EpisodeNumber)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop indexes
        manager
            .drop_index(
                Index::drop()
                    .name("idx_media_items_episode_unique")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_media_items_parent_season")
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(Index::drop().name("idx_media_items_parent").to_owned())
            .await?;

        // Note: No foreign key to drop since SQLite doesn't support adding them to existing tables

        // Drop columns
        manager
            .alter_table(
                Table::alter()
                    .table(MediaItems::Table)
                    .drop_column(MediaItems::EpisodeNumber)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MediaItems::Table)
                    .drop_column(MediaItems::SeasonNumber)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MediaItems::Table)
                    .drop_column(MediaItems::ParentId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

// Define table identifiers
#[derive(Iden)]
enum MediaItems {
    Table,
    #[allow(dead_code)]
    Id,
    ParentId,
    SeasonNumber,
    EpisodeNumber,
}
