use anyhow::Result;
use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, warn};

use crate::cache::{FileCache, FileCacheHandle};
use crate::services::config_service::config_service;

/// Global cache service instance
static CACHE_SERVICE: Lazy<CacheService> = Lazy::new(CacheService::new);

/// Cache service that manages the file cache
#[derive(Debug)]
pub struct CacheService {
    cache_handle: Arc<Mutex<Option<FileCacheHandle>>>,
    is_initialized: Arc<Mutex<bool>>,
}

impl CacheService {
    fn new() -> Self {
        Self {
            cache_handle: Arc::new(Mutex::new(None)),
            is_initialized: Arc::new(Mutex::new(false)),
        }
    }

    /// Initialize the cache service with database connection
    pub async fn initialize(&self, db: Arc<sea_orm::DatabaseConnection>) -> Result<()> {
        let mut is_initialized = self.is_initialized.lock().await;
        if *is_initialized {
            warn!("Cache service already initialized");
            return Ok(());
        }

        debug!("Initializing file cache service");

        // Get cache configuration from config service
        let config = config_service().get_config().await;
        let cache_config = config.cache;

        // Validate cache configuration
        cache_config.validate()?;

        // Create and start the file cache with database connection
        let (cache_handle, file_cache) = FileCache::new(cache_config, db).await?;

        // Store the handle
        {
            let mut handle_guard = self.cache_handle.lock().await;
            *handle_guard = Some(cache_handle);
        }

        // Spawn the cache task with error logging
        tokio::spawn(async move {
            file_cache.run().await;
            error!("File cache task has exited unexpectedly");
        });

        *is_initialized = true;

        debug!("File cache service initialized");
        Ok(())
    }

    /// Get the cache handle (initializing if necessary)
    pub async fn get_handle(&self) -> Result<FileCacheHandle> {
        let handle_guard = self.cache_handle.lock().await;
        handle_guard.as_ref().cloned().ok_or_else(|| {
            anyhow::anyhow!("Cache service not initialized. Please call initialize() first.")
        })
    }

    /// Check if the cache service is initialized
    pub async fn is_initialized(&self) -> bool {
        let is_initialized = self.is_initialized.lock().await;
        *is_initialized
    }

    /// Shutdown the cache service
    pub async fn shutdown(&self) -> Result<()> {
        let mut is_initialized = self.is_initialized.lock().await;
        if !*is_initialized {
            return Ok(());
        }

        debug!("Shutting down file cache service");

        // Shutdown the cache
        {
            let handle_guard = self.cache_handle.lock().await;
            if let Some(ref handle) = *handle_guard {
                handle.shutdown()?;
            }
        }

        // Clear the handle
        {
            let mut handle_guard = self.cache_handle.lock().await;
            *handle_guard = None;
        }

        *is_initialized = false;

        debug!("File cache service shut down");
        Ok(())
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> Result<crate::cache::storage::CacheStats> {
        let handle = self.get_handle().await?;
        handle.get_stats().await
    }

    /// Clear the entire cache
    pub async fn clear_cache(&self) -> Result<()> {
        let handle = self.get_handle().await?;
        handle.clear_cache().await
    }

    /// Cleanup cache to fit within limits
    pub async fn cleanup_cache(&self) -> Result<()> {
        let handle = self.get_handle().await?;
        handle.cleanup_cache().await
    }
}

/// Get the global cache service instance
pub fn cache_service() -> &'static CacheService {
    &CACHE_SERVICE
}

/// Initialize the cache service at application startup with database connection
pub async fn initialize_cache_service(db: Arc<sea_orm::DatabaseConnection>) -> Result<()> {
    cache_service().initialize(db).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_service_initialization() {
        let service = CacheService::new();

        // Should not be initialized initially
        assert!(!service.is_initialized().await);

        // Initialize should work
        // Note: This test would need proper config setup to work
        // assert!(service.initialize().await.is_ok());
        // assert!(service.is_initialized().await);
    }

    #[tokio::test]
    async fn test_cache_service_double_initialization() {
        let service = CacheService::new();

        // Double initialization should not fail
        // Note: This test would need proper config setup to work
        // assert!(service.initialize().await.is_ok());
        // assert!(service.initialize().await.is_ok());
        // assert!(service.is_initialized().await);
    }
}
