use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
#[allow(dead_code)] // Used by SeaORM migration system
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create home_sections table
        manager
            .create_table(
                Table::create()
                    .table(HomeSections::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(HomeSections::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(HomeSections::SourceId).string().not_null())
                    .col(
                        ColumnDef::new(HomeSections::HubIdentifier)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(HomeSections::Title).string().not_null())
                    .col(
                        ColumnDef::new(HomeSections::SectionType)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(HomeSections::Position).integer().not_null())
                    .col(ColumnDef::new(HomeSections::Context).string())
                    .col(ColumnDef::new(HomeSections::Style).string())
                    .col(ColumnDef::new(HomeSections::HubType).string())
                    .col(ColumnDef::new(HomeSections::Size).integer())
                    .col(
                        ColumnDef::new(HomeSections::LastUpdated)
                            .timestamp()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(HomeSections::IsStale)
                            .boolean()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(HomeSections::CreatedAt)
                            .timestamp()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(HomeSections::UpdatedAt)
                            .timestamp()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_home_sections_source")
                            .from(HomeSections::Table, HomeSections::SourceId)
                            .to(Sources::Table, Sources::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create home_section_items junction table
        manager
            .create_table(
                Table::create()
                    .table(HomeSectionItems::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(HomeSectionItems::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(HomeSectionItems::SectionId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(HomeSectionItems::MediaItemId)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(HomeSectionItems::Position)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(HomeSectionItems::CreatedAt)
                            .timestamp()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_home_section_items_section")
                            .from(HomeSectionItems::Table, HomeSectionItems::SectionId)
                            .to(HomeSections::Table, HomeSections::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_home_section_items_media")
                            .from(HomeSectionItems::Table, HomeSectionItems::MediaItemId)
                            .to(MediaItems::Table, MediaItems::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index for faster lookups by source and hub identifier
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_home_sections_source_hub")
                    .table(HomeSections::Table)
                    .col(HomeSections::SourceId)
                    .col(HomeSections::HubIdentifier)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Create index for ordered retrieval
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_home_sections_source_position")
                    .table(HomeSections::Table)
                    .col(HomeSections::SourceId)
                    .col(HomeSections::Position)
                    .to_owned(),
            )
            .await?;

        // Create index for section items retrieval
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_home_section_items_section_position")
                    .table(HomeSectionItems::Table)
                    .col(HomeSectionItems::SectionId)
                    .col(HomeSectionItems::Position)
                    .to_owned(),
            )
            .await?;

        // Create unique index to prevent duplicate items in a section
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_home_section_items_unique")
                    .table(HomeSectionItems::Table)
                    .col(HomeSectionItems::SectionId)
                    .col(HomeSectionItems::MediaItemId)
                    .unique()
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
                    .name("idx_home_section_items_unique")
                    .table(HomeSectionItems::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_home_section_items_section_position")
                    .table(HomeSectionItems::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_home_sections_source_position")
                    .table(HomeSections::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_home_sections_source_hub")
                    .table(HomeSections::Table)
                    .to_owned(),
            )
            .await?;

        // Drop tables in reverse order due to foreign keys
        manager
            .drop_table(Table::drop().table(HomeSectionItems::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(HomeSections::Table).to_owned())
            .await?;

        Ok(())
    }
}

// Table identifiers for home_sections
#[derive(Iden)]
enum HomeSections {
    Table,
    Id,
    SourceId,
    HubIdentifier,
    Title,
    SectionType,
    Position,
    Context,
    Style,
    HubType,
    Size,
    LastUpdated,
    IsStale,
    CreatedAt,
    UpdatedAt,
}

// Table identifiers for home_section_items
#[derive(Iden)]
enum HomeSectionItems {
    Table,
    Id,
    SectionId,
    MediaItemId,
    Position,
    CreatedAt,
}

// Referenced tables
#[derive(Iden)]
enum Sources {
    Table,
    Id,
}

#[derive(Iden)]
enum MediaItems {
    Table,
    Id,
}
