use anyhow::Result;
use async_trait::async_trait;

use crate::backends::traits::MediaBackend;
use crate::db::connection::DatabaseConnection;
use crate::models::{Library, SourceId};
use crate::services::commands::Command;
use crate::services::core::sync::{SyncResult, SyncService};

/// Sync all libraries for a source
pub struct SyncSourceCommand<'a> {
    pub db: DatabaseConnection,
    pub backend: &'a dyn MediaBackend,
    pub source_id: SourceId,
}

#[async_trait]
impl<'a> Command<SyncResult> for SyncSourceCommand<'a> {
    async fn execute(&self) -> Result<SyncResult> {
        SyncService::sync_source(&self.db, self.backend, &self.source_id).await
    }
}

/// Sync a single library
pub struct SyncLibraryCommand<'a> {
    pub db: DatabaseConnection,
    pub backend: &'a dyn MediaBackend,
    pub source_id: SourceId,
    pub library: Library,
}

#[async_trait]
impl<'a> Command<usize> for SyncLibraryCommand<'a> {
    async fn execute(&self) -> Result<usize> {
        SyncService::sync_library(&self.db, self.backend, &self.source_id, &self.library).await
    }
}
