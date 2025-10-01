use anyhow::Result;
use async_trait::async_trait;
use std::time::Duration;

use crate::models::{
    Credentials, Episode, HomeSection, Library, LibraryId, MediaItemId, Movie, Season, Show,
    ShowId, StreamInfo, User,
};

#[async_trait]
pub trait MediaBackend: Send + Sync + std::fmt::Debug {
    /// Initialize the backend with stored credentials
    /// Returns Ok(Some(user)) if successfully connected, Ok(None) if no credentials, Err if failed
    async fn initialize(&self) -> Result<Option<User>>;

    // is_initialized and is_playback_ready removed - never used

    /// Get the backend as Any for downcasting
    fn as_any(&self) -> &dyn std::any::Any;

    async fn authenticate(&self, credentials: Credentials) -> Result<User>;

    async fn get_libraries(&self) -> Result<Vec<Library>>;

    async fn get_movies(&self, library_id: &LibraryId) -> Result<Vec<Movie>>;

    async fn get_shows(&self, library_id: &LibraryId) -> Result<Vec<Show>>;

    async fn get_seasons(&self, show_id: &ShowId) -> Result<Vec<Season>>;

    async fn get_episodes(&self, show_id: &ShowId, season: u32) -> Result<Vec<Episode>>;

    async fn get_stream_url(&self, media_id: &MediaItemId) -> Result<StreamInfo>;

    async fn update_progress(
        &self,
        media_id: &MediaItemId,
        position: Duration,
        duration: Duration,
    ) -> Result<()>;

    // Watch status methods removed - never used in production
    // Search method removed - never used in production

    /// Get homepage sections with suggested content, recently added, etc.
    async fn get_home_sections(&self) -> Result<Vec<HomeSection>> {
        // Default implementation returns empty sections
        // Backends should override this to provide homepage data
        Ok(Vec::new())
    }

    // Marker and navigation methods removed - never used in production

    // get_library_items removed - never used in production

    // Music and photo methods removed - never implemented

    // get_backend_info removed - never used

    // Sync support methods

    // get_last_sync_time and supports_offline removed - never used
    // get_backend_id removed - never used
}
