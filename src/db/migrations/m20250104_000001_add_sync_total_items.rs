use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add total_items column to sync_status table
        manager
            .alter_table(
                Table::alter()
                    .table(SyncStatus::Table)
                    .add_column(ColumnDef::new(SyncStatus::TotalItems).integer().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Remove the total_items column
        manager
            .alter_table(
                Table::alter()
                    .table(SyncStatus::Table)
                    .drop_column(SyncStatus::TotalItems)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum SyncStatus {
    Table,
    TotalItems,
}
