use crate::events::{
    event_bus::EventBus,
    types::{DatabaseEvent, EventPayload, EventType},
};
use anyhow::{Context, Result};
use sea_orm::{ConnectOptions, Database as SeaOrmDatabase, DatabaseConnection as SeaOrmConnection};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

pub type DatabaseConnection = Arc<SeaOrmConnection>;

pub struct Database {
    connection: DatabaseConnection,
}

impl Database {
    /// Create a new database connection
    pub async fn new() -> Result<Self> {
        let db_path = Self::db_path()?;
        Self::connect(&db_path).await
    }

    /// Connect to a specific database path
    pub async fn connect(path: &PathBuf) -> Result<Self> {
        // Ensure database directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create database directory")?;
        }

        // Build connection URL
        let db_url = format!("sqlite://{}?mode=rwc", path.display());
        info!("Connecting to database at: {}", db_url);

        // Configure connection options
        let mut opt = ConnectOptions::new(db_url);
        opt.max_connections(10)
            .min_connections(1)
            .connect_timeout(Duration::from_secs(8))
            .acquire_timeout(Duration::from_secs(8))
            .idle_timeout(Duration::from_secs(8))
            .max_lifetime(Duration::from_secs(8))
            .sqlx_logging(false); // Disable SQLx logging (we'll use SeaORM's)

        // Connect to database
        let connection = SeaOrmDatabase::connect(opt)
            .await
            .context("Failed to connect to database")?;

        // Enable foreign key constraints for SQLite
        use sea_orm::{ConnectionTrait, Statement};
        connection
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "PRAGMA foreign_keys = ON",
            ))
            .await
            .context("Failed to enable foreign key constraints")?;

        // Enable WAL mode for better concurrent access
        connection
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "PRAGMA journal_mode = WAL",
            ))
            .await
            .context("Failed to enable WAL mode")?;

        info!("Successfully connected to database");

        Ok(Self {
            connection: Arc::new(connection),
        })
    }

    /// Get a clone of the database connection
    pub fn get_connection(&self) -> DatabaseConnection {
        self.connection.clone()
    }

    /// Get the default database path
    fn db_path() -> Result<PathBuf> {
        let cache_dir = dirs::cache_dir().context("Failed to get cache directory")?;
        Ok(cache_dir.join("reel").join("data.db"))
    }

    /// Run migrations
    pub async fn migrate(&self) -> Result<()> {
        self.migrate_with_events(None).await
    }

    /// Run migrations with optional event bus for notifications
    pub async fn migrate_with_events(&self, event_bus: Option<Arc<EventBus>>) -> Result<()> {
        use crate::db::migrations::Migrator;
        use sea_orm_migration::MigratorTrait;

        info!("Running database migrations");

        // Get pending migrations count for the event
        let pending_count = Migrator::get_pending_migrations(&*self.connection)
            .await
            .context("Failed to get pending migrations")?
            .len();

        if pending_count > 0 {
            Migrator::up(&*self.connection, None)
                .await
                .context("Failed to run migrations")?;

            info!("Database migrations completed successfully");

            // Emit DatabaseMigrated event
            if let Some(bus) = event_bus {
                let event = DatabaseEvent::new(
                    EventType::DatabaseMigrated,
                    EventPayload::System {
                        message: format!("Applied {} database migrations", pending_count),
                        details: Some(serde_json::json!({
                            "migrations_applied": pending_count,
                            "database_path": Self::db_path().unwrap_or_default().to_string_lossy()
                        })),
                    },
                );

                if let Err(e) = bus.publish(event).await {
                    tracing::warn!("Failed to publish DatabaseMigrated event: {}", e);
                }
            }
        } else {
            info!("No pending migrations to apply");
        }

        Ok(())
    }

    /// Check if database needs migration
    pub async fn needs_migration(&self) -> Result<bool> {
        use crate::db::migrations::Migrator;
        use sea_orm_migration::MigratorTrait;

        let applied = Migrator::get_applied_migrations(&*self.connection)
            .await
            .context("Failed to get applied migrations")?;

        let pending = Migrator::get_pending_migrations(&*self.connection)
            .await
            .context("Failed to get pending migrations")?;

        Ok(!pending.is_empty())
    }
}
