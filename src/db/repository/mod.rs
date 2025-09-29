pub mod auth_token_repository;
pub mod home_section_repository;
pub mod library_repository;
pub mod media_repository;
pub mod playback_repository;
pub mod source_repository;
pub mod sync_repository;

use anyhow::Result;
use async_trait::async_trait;
use sea_orm::{DatabaseConnection, EntityTrait};
use std::sync::Arc;

/// Base repository trait that all repositories should implement
#[async_trait]
pub trait Repository<T> {
    type Entity: EntityTrait;

    /// Find an entity by ID
    async fn find_by_id(&self, id: &str) -> Result<Option<T>>;

    /// Find all entities
    async fn find_all(&self) -> Result<Vec<T>>;

    /// Insert a new entity
    async fn insert(&self, entity: T) -> Result<T>;

    /// Update an existing entity
    async fn update(&self, entity: T) -> Result<T>;

    /// Delete an entity by ID
    async fn delete(&self, id: &str) -> Result<()>;

    /// Count all entities

    async fn count(&self) -> Result<u64>;
}

/// Base repository implementation holder
#[derive(Debug)]
pub struct BaseRepository {
    pub db: Arc<DatabaseConnection>,
}

impl BaseRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

// Re-export specific repositories
pub use auth_token_repository::{AuthTokenRepository, AuthTokenRepositoryImpl};
pub use home_section_repository::{HomeSectionRepository, HomeSectionRepositoryImpl};
pub use library_repository::{LibraryRepository, LibraryRepositoryImpl};
pub use media_repository::{MediaRepository, MediaRepositoryImpl};
pub use playback_repository::{PlaybackRepository, PlaybackRepositoryImpl};
pub use source_repository::SourceRepositoryImpl;
