use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
#[allow(dead_code)] // Used by SeaORM migration system
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add auth_status to sources table
        manager
            .alter_table(
                Table::alter()
                    .table(Sources::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Sources::AuthStatus)
                            .string()
                            .not_null()
                            .default("unknown"),
                    )
                    .to_owned(),
            )
            .await?;

        // Add last_auth_check timestamp to sources table
        manager
            .alter_table(
                Table::alter()
                    .table(Sources::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Sources::LastAuthCheck).timestamp().null(),
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
                    .drop_column(Sources::AuthStatus)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Sources::Table)
                    .drop_column(Sources::LastAuthCheck)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Sources {
    Table,
    AuthStatus,
    LastAuthCheck,
}
