use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add intro marker fields
        manager
            .alter_table(
                Table::alter()
                    .table(MediaItems::Table)
                    .add_column(ColumnDef::new(MediaItems::IntroMarkerStartMs).big_integer())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MediaItems::Table)
                    .add_column(ColumnDef::new(MediaItems::IntroMarkerEndMs).big_integer())
                    .to_owned(),
            )
            .await?;

        // Add credits marker fields
        manager
            .alter_table(
                Table::alter()
                    .table(MediaItems::Table)
                    .add_column(ColumnDef::new(MediaItems::CreditsMarkerStartMs).big_integer())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MediaItems::Table)
                    .add_column(ColumnDef::new(MediaItems::CreditsMarkerEndMs).big_integer())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(MediaItems::Table)
                    .drop_column(MediaItems::IntroMarkerStartMs)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MediaItems::Table)
                    .drop_column(MediaItems::IntroMarkerEndMs)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MediaItems::Table)
                    .drop_column(MediaItems::CreditsMarkerStartMs)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MediaItems::Table)
                    .drop_column(MediaItems::CreditsMarkerEndMs)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum MediaItems {
    Table,
    IntroMarkerStartMs,
    IntroMarkerEndMs,
    CreditsMarkerStartMs,
    CreditsMarkerEndMs,
}
