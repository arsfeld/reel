use anyhow::{Context, Result};
use dirs;
use serde::{Serialize, de::DeserializeOwned};
use sqlx::{Row, sqlite::SqlitePool};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug)]
pub struct CacheManager {
    db: Arc<SqlitePool>,
}

impl CacheManager {
    pub fn new() -> Result<Self> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async { Self::new_async().await })
    }

    async fn new_async() -> Result<Self> {
        let db_path = Self::db_path()?;

        // Ensure cache directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create cache directory")?;
        }

        // Use proper SQLite connection string with create flag
        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
        let db = SqlitePool::connect(&db_url)
            .await
            .context("Failed to connect to cache database")?;

        // Initialize database schema
        Self::initialize_schema(&db).await?;

        Ok(Self { db: Arc::new(db) })
    }

    async fn initialize_schema(db: &SqlitePool) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS media_cache (
                id TEXT PRIMARY KEY,
                type TEXT NOT NULL,
                data TEXT NOT NULL,
                updated_at INTEGER DEFAULT (unixepoch())
            )
            "#,
        )
        .execute(db)
        .await
        .context("Failed to create media_cache table")?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS playback_progress (
                media_id TEXT PRIMARY KEY,
                position INTEGER NOT NULL,
                duration INTEGER NOT NULL,
                watched BOOLEAN DEFAULT FALSE,
                view_count INTEGER DEFAULT 0,
                last_watched_at INTEGER,
                updated_at INTEGER DEFAULT (unixepoch())
            )
            "#,
        )
        .execute(db)
        .await
        .context("Failed to create playback_progress table")?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS preferences (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )
            "#,
        )
        .execute(db)
        .await
        .context("Failed to create preferences table")?;

        Ok(())
    }

    pub async fn get_media<T>(&self, id: &str) -> Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let result = sqlx::query("SELECT data FROM media_cache WHERE id = ?")
            .bind(id)
            .fetch_optional(self.db.as_ref())
            .await?;

        match result {
            Some(row) => {
                let data: String = row.try_get("data")?;
                let item = serde_json::from_str(&data)?;
                Ok(Some(item))
            }
            None => Ok(None),
        }
    }

    pub async fn set_media<T>(&self, id: &str, media_type: &str, data: &T) -> Result<()>
    where
        T: Serialize,
    {
        let json = serde_json::to_string(data)?;

        sqlx::query(
            r#"
            INSERT INTO media_cache (id, type, data)
            VALUES (?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                data = excluded.data,
                updated_at = unixepoch()
            "#,
        )
        .bind(id)
        .bind(media_type)
        .bind(json)
        .execute(self.db.as_ref())
        .await?;

        Ok(())
    }

    pub async fn get_playback_progress(&self, media_id: &str) -> Result<Option<(u64, u64)>> {
        let result =
            sqlx::query("SELECT position, duration FROM playback_progress WHERE media_id = ?")
                .bind(media_id)
                .fetch_optional(self.db.as_ref())
                .await?;

        match result {
            Some(row) => {
                let position: i64 = row.try_get("position")?;
                let duration: i64 = row.try_get("duration")?;
                Ok(Some((position as u64, duration as u64)))
            }
            None => Ok(None),
        }
    }

    pub async fn set_playback_progress(
        &self,
        media_id: &str,
        position: u64,
        duration: u64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO playback_progress (media_id, position, duration)
            VALUES (?, ?, ?)
            ON CONFLICT(media_id) DO UPDATE SET
                position = excluded.position,
                duration = excluded.duration,
                updated_at = unixepoch()
            "#,
        )
        .bind(media_id)
        .bind(position as i64)
        .bind(duration as i64)
        .execute(self.db.as_ref())
        .await?;

        Ok(())
    }

    pub async fn clear_backend_cache(&self, backend_id: &str) -> Result<()> {
        // Delete all entries for this backend
        sqlx::query("DELETE FROM media_cache WHERE id LIKE ?")
            .bind(format!("{}:%", backend_id))
            .execute(self.db.as_ref())
            .await?;

        Ok(())
    }

    fn db_path() -> Result<PathBuf> {
        let cache_dir = dirs::cache_dir().context("Failed to get cache directory")?;
        Ok(cache_dir.join("reel").join("cache.db"))
    }
}
