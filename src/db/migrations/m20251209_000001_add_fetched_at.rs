use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add fetched_at column to media_items table
        // This tracks when metadata was last fetched from the backend (separate from updated_at)
        manager
            .alter_table(
                Table::alter()
                    .table(MediaItems::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(MediaItems::FetchedAt).timestamp().null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Add index for efficient TTL queries
        manager
            .create_index(
                Index::create()
                    .name("idx_media_items_fetched_at")
                    .table(MediaItems::Table)
                    .col(MediaItems::FetchedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Remove the index first
        manager
            .drop_index(
                Index::drop()
                    .name("idx_media_items_fetched_at")
                    .table(MediaItems::Table)
                    .to_owned(),
            )
            .await?;

        // Remove the column
        manager
            .alter_table(
                Table::alter()
                    .table(MediaItems::Table)
                    .drop_column(MediaItems::FetchedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum MediaItems {
    Table,
    FetchedAt,
}
