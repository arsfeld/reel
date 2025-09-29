use super::{BaseRepository, Repository};
use crate::db::entities::{AuthToken, AuthTokenActiveModel, AuthTokenModel, auth_tokens};
use anyhow::Result;
use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};
use std::sync::Arc;

/// Repository trait for AuthToken entities
#[async_trait]
pub trait AuthTokenRepository: Repository<AuthTokenModel> {
    /// Find a token by source ID and token type
    async fn find_by_source_and_type(
        &self,
        source_id: &str,
        token_type: &str,
    ) -> Result<Option<AuthTokenModel>>;

    /// Delete all tokens for a source
    async fn delete_by_source(&self, source_id: &str) -> Result<u64>;

    /// Upsert a token (insert if not exists, update if exists)
    async fn upsert(&self, entity: AuthTokenModel) -> Result<AuthTokenModel>;

    /// Find expired tokens
    async fn find_expired(&self) -> Result<Vec<AuthTokenModel>>;

    /// Delete expired tokens
    async fn delete_expired(&self) -> Result<u64>;
}

#[derive(Debug)]
pub struct AuthTokenRepositoryImpl {
    base: BaseRepository,
}

impl AuthTokenRepositoryImpl {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            base: BaseRepository::new(db),
        }
    }
}

#[async_trait]
impl Repository<AuthTokenModel> for AuthTokenRepositoryImpl {
    type Entity = AuthToken;

    async fn find_by_id(&self, id: &str) -> Result<Option<AuthTokenModel>> {
        let id = id.parse::<i32>()?;
        Ok(AuthToken::find_by_id(id).one(self.base.db.as_ref()).await?)
    }

    async fn find_all(&self) -> Result<Vec<AuthTokenModel>> {
        Ok(AuthToken::find().all(self.base.db.as_ref()).await?)
    }

    async fn insert(&self, entity: AuthTokenModel) -> Result<AuthTokenModel> {
        let active_model = AuthTokenActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            source_id: Set(entity.source_id.clone()),
            token_type: Set(entity.token_type.clone()),
            token: Set(entity.token.clone()),
            created_at: Set(entity.created_at),
            updated_at: Set(chrono::Utc::now().naive_utc()),
            expires_at: Set(entity.expires_at),
        };

        Ok(active_model.insert(self.base.db.as_ref()).await?)
    }

    async fn update(&self, entity: AuthTokenModel) -> Result<AuthTokenModel> {
        let active_model = AuthTokenActiveModel {
            id: Set(entity.id),
            source_id: Set(entity.source_id.clone()),
            token_type: Set(entity.token_type.clone()),
            token: Set(entity.token.clone()),
            created_at: Set(entity.created_at),
            updated_at: Set(chrono::Utc::now().naive_utc()),
            expires_at: Set(entity.expires_at),
        };

        Ok(active_model.update(self.base.db.as_ref()).await?)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let id = id.parse::<i32>()?;
        AuthToken::delete_by_id(id)
            .exec(self.base.db.as_ref())
            .await?;
        Ok(())
    }

    async fn count(&self) -> Result<u64> {
        Ok(AuthToken::find().count(self.base.db.as_ref()).await?)
    }
}

#[async_trait]
impl AuthTokenRepository for AuthTokenRepositoryImpl {
    async fn find_by_source_and_type(
        &self,
        source_id: &str,
        token_type: &str,
    ) -> Result<Option<AuthTokenModel>> {
        Ok(AuthToken::find()
            .filter(auth_tokens::Column::SourceId.eq(source_id))
            .filter(auth_tokens::Column::TokenType.eq(token_type))
            .one(self.base.db.as_ref())
            .await?)
    }

    async fn delete_by_source(&self, source_id: &str) -> Result<u64> {
        let result = AuthToken::delete_many()
            .filter(auth_tokens::Column::SourceId.eq(source_id))
            .exec(self.base.db.as_ref())
            .await?;
        Ok(result.rows_affected)
    }

    async fn upsert(&self, entity: AuthTokenModel) -> Result<AuthTokenModel> {
        // Try to find existing token
        if let Some(existing) = self
            .find_by_source_and_type(&entity.source_id, &entity.token_type)
            .await?
        {
            // Update existing token
            let mut updated = existing;
            updated.token = entity.token;
            updated.expires_at = entity.expires_at;
            self.update(updated).await
        } else {
            // Insert new token
            self.insert(entity).await
        }
    }

    async fn find_expired(&self) -> Result<Vec<AuthTokenModel>> {
        Ok(AuthToken::find()
            .filter(auth_tokens::Column::ExpiresAt.lt(chrono::Utc::now().naive_utc()))
            .all(self.base.db.as_ref())
            .await?)
    }

    async fn delete_expired(&self) -> Result<u64> {
        let result = AuthToken::delete_many()
            .filter(auth_tokens::Column::ExpiresAt.lt(chrono::Utc::now().naive_utc()))
            .exec(self.base.db.as_ref())
            .await?;
        Ok(result.rows_affected)
    }
}
