use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
#[allow(dead_code)] // Used by SeaORM migration system
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add connections JSON column to store all discovered server connections
        manager
            .alter_table(
                Table::alter()
                    .table(Sources::Table)
                    .add_column_if_not_exists(ColumnDef::new(Sources::Connections).json().null())
                    .to_owned(),
            )
            .await?;

        // Add machine_id column for Plex servers
        manager
            .alter_table(
                Table::alter()
                    .table(Sources::Table)
                    .add_column_if_not_exists(ColumnDef::new(Sources::MachineId).string().null())
                    .to_owned(),
            )
            .await?;

        // Add owned flag for Plex servers (owned vs shared)
        manager
            .alter_table(
                Table::alter()
                    .table(Sources::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Sources::IsOwned).boolean().default(true),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop the added columns
        manager
            .alter_table(
                Table::alter()
                    .table(Sources::Table)
                    .drop_column(Sources::Connections)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Sources::Table)
                    .drop_column(Sources::MachineId)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Sources::Table)
                    .drop_column(Sources::IsOwned)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Sources {
    Table,
    Connections,
    MachineId,
    IsOwned,
}
