use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create people table
        manager
            .create_table(
                Table::create()
                    .table(People::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(People::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(People::Name).string().not_null())
                    .col(ColumnDef::new(People::ImageUrl).string())
                    .col(
                        ColumnDef::new(People::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(People::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create media_people junction table
        manager
            .create_table(
                Table::create()
                    .table(MediaPeople::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MediaPeople::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MediaPeople::MediaItemId).string().not_null())
                    .col(ColumnDef::new(MediaPeople::PersonId).string().not_null())
                    .col(ColumnDef::new(MediaPeople::PersonType).string().not_null()) // 'actor', 'director', 'writer', 'producer'
                    .col(ColumnDef::new(MediaPeople::Role).string()) // Character name for actors
                    .col(ColumnDef::new(MediaPeople::SortOrder).integer()) // Display order
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_people_media_item")
                            .from(MediaPeople::Table, MediaPeople::MediaItemId)
                            .to(MediaItems::Table, MediaItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_people_person")
                            .from(MediaPeople::Table, MediaPeople::PersonId)
                            .to(People::Table, People::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes for efficient queries
        manager
            .create_index(
                Index::create()
                    .name("idx_media_people_media_item")
                    .table(MediaPeople::Table)
                    .col(MediaPeople::MediaItemId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_media_people_person")
                    .table(MediaPeople::Table)
                    .col(MediaPeople::PersonId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_media_people_type")
                    .table(MediaPeople::Table)
                    .col(MediaPeople::PersonType)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MediaPeople::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(People::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum People {
    Table,
    Id,
    Name,
    ImageUrl,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum MediaPeople {
    Table,
    Id,
    MediaItemId,
    PersonId,
    PersonType,
    Role,
    SortOrder,
}

#[derive(DeriveIden)]
enum MediaItems {
    Table,
    Id,
}
