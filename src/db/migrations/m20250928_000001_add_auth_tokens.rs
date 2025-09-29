use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
#[allow(dead_code)] // Used by SeaORM migration system
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create auth_tokens table
        manager
            .create_table(
                Table::create()
                    .table(AuthTokens::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuthTokens::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AuthTokens::SourceId).string().not_null())
                    .col(ColumnDef::new(AuthTokens::TokenType).string().not_null())
                    .col(ColumnDef::new(AuthTokens::Token).text().not_null())
                    .col(
                        ColumnDef::new(AuthTokens::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(AuthTokens::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(AuthTokens::ExpiresAt).timestamp().null())
                    .to_owned(),
            )
            .await?;

        // Create index on source_id for fast lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_auth_tokens_source_id")
                    .table(AuthTokens::Table)
                    .col(AuthTokens::SourceId)
                    .to_owned(),
            )
            .await?;

        // Create unique index on source_id and token_type to prevent duplicates
        manager
            .create_index(
                Index::create()
                    .name("idx_auth_tokens_source_token_type")
                    .table(AuthTokens::Table)
                    .col(AuthTokens::SourceId)
                    .col(AuthTokens::TokenType)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop the auth_tokens table
        manager
            .drop_table(Table::drop().table(AuthTokens::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum AuthTokens {
    Table,
    Id,
    SourceId,
    TokenType,
    Token,
    CreatedAt,
    UpdatedAt,
    ExpiresAt,
}
