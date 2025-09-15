use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add last_connection_test timestamp to sources table
        manager
            .alter_table(
                Table::alter()
                    .table(Sources::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Sources::LastConnectionTest)
                            .timestamp()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Add connection_failure_count to sources table
        manager
            .alter_table(
                Table::alter()
                    .table(Sources::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Sources::ConnectionFailureCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        // Add connection_quality to sources table (local, remote, relay)
        manager
            .alter_table(
                Table::alter()
                    .table(Sources::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Sources::ConnectionQuality).string().null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Remove the added columns
        manager
            .alter_table(
                Table::alter()
                    .table(Sources::Table)
                    .drop_column(Sources::LastConnectionTest)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Sources::Table)
                    .drop_column(Sources::ConnectionFailureCount)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Sources::Table)
                    .drop_column(Sources::ConnectionQuality)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Sources {
    Table,
    LastConnectionTest,
    ConnectionFailureCount,
    ConnectionQuality,
}
