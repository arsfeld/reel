//! Common utilities for integration tests
//!
//! This module provides test database helpers and utilities for integration testing.
//! Docker container fixtures using testcontainers can be added here for E2E testing.

#![allow(dead_code)]

use anyhow::Result;

/// Test database helper
pub struct TestDb {
    pub connection: std::sync::Arc<sea_orm::DatabaseConnection>,
    _temp_dir: tempfile::TempDir,
}

impl TestDb {
    /// Create a new test database with migrations
    pub async fn new() -> Result<Self> {
        use reel::db::connection::Database;

        let temp_dir = tempfile::TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");

        let db = Database::connect(&db_path).await?;
        db.migrate().await?;

        Ok(Self {
            connection: db.get_connection(),
            _temp_dir: temp_dir,
        })
    }
}

// Note: Docker container fixtures using testcontainers can be added here
// for true E2E testing with real Plex/Jellyfin servers.
// Example:
// pub struct PlexContainer { ... }
// pub struct JellyfinContainer { ... }
//
// These would use testcontainers::GenericImage to spin up Docker containers
// during tests. For now, integration tests use mockito for fast, reliable testing.
