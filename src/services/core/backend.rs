use crate::backends::{jellyfin::JellyfinBackend, plex::PlexBackend, traits::MediaBackend};
use crate::db::connection::DatabaseConnection;
use crate::db::repository::{
    Repository,
    media_repository::{MediaRepository, MediaRepositoryImpl},
    source_repository::{SourceRepository, SourceRepositoryImpl},
};
use crate::models::{
    AuthProvider, ConnectionInfo, Credentials, Episode, HomeSection, MediaItem, MediaItemId, Movie,
    Show, Source, SourceId, SourceType, StreamInfo,
};
use crate::services::core::auth::AuthService;
use anyhow::{Context, Result};
use sea_orm::{ActiveModelTrait, Set};

/// Stateless backend service following Relm4's pure function pattern
/// All backend operations are pure functions that take dependencies as parameters
pub struct BackendService;

impl BackendService {
    /// Get stream URL for a media item - pure function that creates backend on demand
    pub async fn get_stream_url(
        db: &DatabaseConnection,
        media_item_id: &MediaItemId,
    ) -> Result<StreamInfo> {
        // Load media item to find its source
        let media_repo = MediaRepositoryImpl::new(db.clone());
        let media_item = media_repo
            .find_by_id(media_item_id.as_str())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Media item not found"))?;

        // Load source configuration
        let source_repo = SourceRepositoryImpl::new(db.clone());
        let source_entity = source_repo
            .find_by_id(&media_item.source_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Source not found"))?;

        // Create backend and get stream URL
        let backend = Self::create_backend_for_source(db, &source_entity).await?;
        backend.get_stream_url(media_item_id).await
    }

    /// Create a backend instance for a source - stateless factory
    async fn create_backend_for_source(
        db: &DatabaseConnection,
        source_entity: &crate::db::entities::sources::Model,
    ) -> Result<Box<dyn MediaBackend>> {
        // Load credentials from secure storage
        let source_id = SourceId::new(source_entity.id.clone());
        let credentials = AuthService::load_credentials(&source_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No credentials found for source"))?;

        // Create AuthProvider based on credentials and source type
        let auth_provider = Self::create_auth_provider(&credentials, source_entity)?;

        // Create Source struct from entity
        let source = Self::entity_to_source(source_entity);

        // Create and initialize the appropriate backend
        let backend: Box<dyn MediaBackend> = match source_entity.source_type.as_str() {
            "plex" | "PlexServer" => {
                let backend = PlexBackend::from_auth(auth_provider, source)
                    .context("Failed to create Plex backend")?;
                backend.initialize().await?;

                // Update the source with the best connection URL if it changed
                if backend.has_url_changed().await {
                    if let Some(new_url) = backend.get_current_url().await {
                        tracing::info!(
                            "Updating source {} with new URL: {}",
                            source_entity.id,
                            new_url
                        );
                        let source_repo = SourceRepositoryImpl::new(db.clone());
                        source_repo
                            .update_connection_url(&source_entity.id, Some(new_url))
                            .await
                            .context("Failed to update source URL")?;
                    }
                }

                Box::new(backend)
            }
            "jellyfin" | "JellyfinServer" => {
                let backend = JellyfinBackend::from_auth(auth_provider, source)
                    .context("Failed to create Jellyfin backend")?;
                backend.initialize().await?;
                Box::new(backend)
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported source type: {}",
                    source_entity.source_type
                ));
            }
        };

        Ok(backend)
    }

    /// Create AuthProvider from credentials - pure transformation
    fn create_auth_provider(
        credentials: &Credentials,
        source: &crate::db::entities::sources::Model,
    ) -> Result<AuthProvider> {
        let auth_provider = match (credentials, source.source_type.as_str()) {
            // Plex with token
            (Credentials::Token { token }, "plex" | "PlexServer") => AuthProvider::PlexAccount {
                id: source.auth_provider_id.clone().unwrap_or_default(),
                username: String::new(),
                email: String::new(),
                token: token.clone(),
                refresh_token: None,
                token_expiry: None,
            },
            // Jellyfin with token (Quick Connect)
            (Credentials::Token { token }, "jellyfin" | "JellyfinServer") => {
                // Parse token to check if it contains user_id (format: token|user_id)
                let parts: Vec<&str> = token.split('|').collect();
                let (access_token, user_id) = if parts.len() == 2 {
                    (parts[0].to_string(), parts[1].to_string())
                } else {
                    (token.clone(), String::new())
                };

                AuthProvider::JellyfinAuth {
                    id: source.auth_provider_id.clone().unwrap_or_default(),
                    server_url: source.connection_url.clone().unwrap_or_default(),
                    username: String::new(),
                    user_id,
                    access_token,
                }
            }
            // Jellyfin with username/password
            (Credentials::UsernamePassword { username, .. }, "jellyfin" | "JellyfinServer") => {
                AuthProvider::JellyfinAuth {
                    id: source.auth_provider_id.clone().unwrap_or_default(),
                    server_url: source.connection_url.clone().unwrap_or_default(),
                    username: username.clone(),
                    user_id: String::new(),
                    access_token: String::new(), // Will be populated during initialization
                }
            }
            _ => return Err(anyhow::anyhow!("Unsupported credential type for source")),
        };

        Ok(auth_provider)
    }

    /// Convert database entity to Source model - pure transformation
    fn entity_to_source(entity: &crate::db::entities::sources::Model) -> Source {
        Source {
            id: entity.id.clone(),
            name: entity.name.clone(),
            source_type: match entity.source_type.as_str() {
                "plex" | "PlexServer" => SourceType::PlexServer {
                    // Use the actual machine_id field from the database
                    machine_id: entity.machine_id.clone().unwrap_or_default(),
                    owned: entity.is_owned,
                },
                "jellyfin" | "JellyfinServer" => SourceType::JellyfinServer,
                _ => SourceType::LocalFolder {
                    path: std::path::PathBuf::new(),
                },
            },
            auth_provider_id: entity.auth_provider_id.clone(),
            connection_info: ConnectionInfo {
                primary_url: entity.connection_url.clone(),
                is_online: entity.is_online,
                last_check: Some(chrono::Utc::now()),
            },
            enabled: true,
            last_sync: entity.last_sync.map(|dt| dt.and_utc()),
            library_count: 0,
        }
    }

    /// Sync a source - creates backend on demand, performs sync, then discards
    pub async fn sync_source(
        db: &DatabaseConnection,
        source_id: &SourceId,
    ) -> Result<crate::backends::traits::SyncResult> {
        use crate::services::core::sync::SyncService;

        // Load source configuration
        let source_repo = SourceRepositoryImpl::new(db.clone());
        let source_entity = source_repo
            .find_by_id(source_id.as_str())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Source not found"))?;

        // Create backend and perform sync
        let backend = Self::create_backend_for_source(db, &source_entity).await?;

        // Use SyncService to perform the actual sync with all content
        let result = SyncService::sync_source(db, backend.as_ref(), source_id).await?;

        // Convert the SyncService result to the expected return type
        Ok(crate::backends::traits::SyncResult {
            backend_id: crate::models::BackendId::new(source_id.as_str()),
            success: result.errors.is_empty(),
            items_synced: result.items_synced,
            duration: std::time::Duration::from_secs(0), // SyncService doesn't track duration
            errors: result.errors,
        })
    }

    /// Test connection for a source - stateless connection test
    pub async fn test_connection(db: &DatabaseConnection, source_id: &SourceId) -> Result<bool> {
        // Load source and try to create backend
        let source_repo = SourceRepositoryImpl::new(db.clone());
        let source_entity = source_repo
            .find_by_id(source_id.as_str())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Source not found"))?;

        // Try to create and initialize backend
        match Self::create_backend_for_source(db, &source_entity).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Update playback progress on the backend server
    pub async fn update_playback_progress(
        db: &DatabaseConnection,
        source_id: &str,
        media_id: &MediaItemId,
        position: std::time::Duration,
        duration: std::time::Duration,
    ) -> Result<()> {
        // Load source configuration
        let source_repo = SourceRepositoryImpl::new(db.clone());
        let source_entity = source_repo
            .find_by_id(source_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Source not found"))?;

        // Create backend and update progress
        let backend = Self::create_backend_for_source(db, &source_entity).await?;
        backend.update_progress(media_id, position, duration).await
    }

    /// Get home sections from all active sources
    pub async fn get_all_home_sections(
        db: &DatabaseConnection,
    ) -> Result<Vec<crate::models::HomeSection>> {
        // Load all sources
        let source_repo = SourceRepositoryImpl::new(db.clone());
        let sources = source_repo.find_all().await?;
        let sources_count = sources.len();

        let mut all_sections = Vec::new();

        // Get home sections from each source concurrently
        let mut section_futures = Vec::new();

        for source_entity in sources.iter() {
            // Skip disabled or offline sources
            if !source_entity.is_online {
                continue;
            }

            let db_clone = db.clone();
            let source_clone = source_entity.clone();

            let future = async move {
                match Self::create_backend_for_source(&db_clone, &source_clone).await {
                    Ok(backend) => {
                        match backend.get_home_sections().await {
                            Ok(mut sections) => {
                                // Prefix section IDs with source ID to avoid conflicts
                                for section in &mut sections {
                                    section.id = format!("{}::{}", source_clone.id, section.id);
                                    // Also prefix the title with source name if multiple sources exist
                                    if sources_count > 1 {
                                        section.title =
                                            format!("{} - {}", source_clone.name, section.title);
                                    }
                                }
                                Ok::<Vec<HomeSection>, anyhow::Error>(sections)
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to get home sections from source {}: {}",
                                    source_clone.id,
                                    e
                                );
                                Ok(Vec::new())
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to create backend for source {}: {}",
                            source_clone.id,
                            e
                        );
                        Ok(Vec::new())
                    }
                }
            };

            section_futures.push(future);
        }

        // Wait for all futures to complete
        let results = futures::future::join_all(section_futures).await;

        // Collect all successful results
        for result in results {
            match result {
                Ok(sections) => all_sections.extend(sections),
                Err(e) => {
                    tracing::error!("Error getting home sections: {}", e);
                }
            }
        }

        tracing::info!(
            "Loaded {} total home sections from {} sources",
            all_sections.len(),
            sources.len()
        );

        Ok(all_sections)
    }

    /// Get home sections per source with individual error handling
    pub async fn get_home_sections_per_source(
        db: &DatabaseConnection,
    ) -> Vec<(crate::models::SourceId, Result<Vec<HomeSection>>)> {
        use std::time::Duration;
        use tokio::time::timeout;

        // Load all sources
        let source_repo = SourceRepositoryImpl::new(db.clone());
        let sources = match source_repo.find_all().await {
            Ok(sources) => sources,
            Err(e) => {
                tracing::error!("Failed to load sources: {}", e);
                return Vec::new();
            }
        };
        let sources_count = sources.len();

        let mut section_futures = Vec::new();

        for source_entity in sources.iter() {
            // Skip disabled or offline sources
            if !source_entity.is_online {
                continue;
            }

            let db_clone = db.clone();
            let source_clone = source_entity.clone();
            let source_id = crate::models::SourceId::new(source_clone.id.clone());

            let future = async move {
                // Apply timeout to prevent slow backends from blocking
                let timeout_result = timeout(Duration::from_secs(10), async {
                    match Self::create_backend_for_source(&db_clone, &source_clone).await {
                        Ok(backend) => {
                            match backend.get_home_sections().await {
                                Ok(mut sections) => {
                                    // Prefix section IDs with source ID to avoid conflicts
                                    for section in &mut sections {
                                        section.id = format!("{}::{}", source_clone.id, section.id);
                                        // Also prefix the title with source name if multiple sources exist
                                        if sources_count > 1 {
                                            section.title = format!(
                                                "{} - {}",
                                                source_clone.name, section.title
                                            );
                                        }
                                    }
                                    Ok(sections)
                                }
                                Err(e) => Err(anyhow::anyhow!("Failed to get sections: {}", e)),
                            }
                        }
                        Err(e) => Err(anyhow::anyhow!("Failed to create backend: {}", e)),
                    }
                })
                .await;

                match timeout_result {
                    Ok(result) => (source_id, result),
                    Err(_) => (source_id, Err(anyhow::anyhow!("Request timed out"))),
                }
            };

            section_futures.push(future);
        }

        // Wait for all futures to complete
        futures::future::join_all(section_futures).await
    }

    /// Load cached home sections from database
    /// Returns sections constructed from cached media items
    pub async fn get_cached_home_sections(
        db: &DatabaseConnection,
    ) -> Vec<(crate::models::SourceId, Vec<HomeSection>)> {
        use crate::db::repository::playback_repository::{
            PlaybackRepository, PlaybackRepositoryImpl,
        };
        use crate::models::HomeSectionType;

        tracing::info!("Loading cached home sections from database as models");

        let mut cached_sections = Vec::new();

        // Load all sources
        let source_repo = SourceRepositoryImpl::new(db.clone());
        let sources = match source_repo.find_all().await {
            Ok(sources) => sources,
            Err(e) => {
                tracing::error!("Failed to load sources: {}", e);
                return cached_sections;
            }
        };

        for source in sources {
            let source_id = crate::models::SourceId::new(source.id.clone());
            let mut sections = Vec::new();

            // Load continue watching items from playback progress
            let playback_repo = PlaybackRepositoryImpl::new(db.clone());
            if let Ok(in_progress) = playback_repo.find_in_progress(None).await {
                if !in_progress.is_empty() {
                    let media_repo = MediaRepositoryImpl::new(db.clone());
                    let mut continue_watching_items = Vec::new();

                    // Load media items for each in-progress item
                    for progress in in_progress.iter().take(20) {
                        // Only include items from this source
                        if let Ok(Some(media_item)) =
                            media_repo.find_by_id(&progress.media_id).await
                        {
                            if media_item.source_id == source.id {
                                // Convert to MediaItem
                                let item = Self::db_model_to_media_item(media_item);
                                continue_watching_items.push(item);
                            }
                        }
                    }

                    if !continue_watching_items.is_empty() {
                        sections.push(HomeSection {
                            id: format!("{}::continue-watching", source.id),
                            title: "Continue Watching".to_string(),
                            section_type: HomeSectionType::ContinueWatching,
                            items: continue_watching_items,
                        });
                    }
                }
            }

            // Load recently added items
            let media_repo = MediaRepositoryImpl::new(db.clone());
            if let Ok(recent_items) = media_repo.find_recently_added(20).await {
                let mut recently_added = Vec::new();

                for item in recent_items {
                    if item.source_id == source.id {
                        let media_item = Self::db_model_to_media_item(item);
                        recently_added.push(media_item);
                    }
                }

                if !recently_added.is_empty() {
                    sections.push(HomeSection {
                        id: format!("{}::recently-added", source.id),
                        title: "Recently Added".to_string(),
                        section_type: HomeSectionType::RecentlyAdded,
                        items: recently_added,
                    });
                }
            }

            // Load movie and show libraries by type
            if let Ok(movies) = media_repo
                .find_by_source_and_type(&source.id, "movie")
                .await
            {
                if !movies.is_empty() {
                    let movie_items: Vec<MediaItem> = movies
                        .into_iter()
                        .take(20)
                        .map(Self::db_model_to_media_item)
                        .collect();

                    sections.push(HomeSection {
                        id: format!("{}::movies", source.id),
                        title: "Movies".to_string(),
                        section_type: HomeSectionType::Custom("Movies".to_string()),
                        items: movie_items,
                    });
                }
            }

            if let Ok(shows) = media_repo.find_by_source_and_type(&source.id, "show").await {
                if !shows.is_empty() {
                    let show_items: Vec<MediaItem> = shows
                        .into_iter()
                        .take(20)
                        .map(Self::db_model_to_media_item)
                        .collect();

                    sections.push(HomeSection {
                        id: format!("{}::shows", source.id),
                        title: "TV Shows".to_string(),
                        section_type: HomeSectionType::Custom("Movies".to_string()),
                        items: show_items,
                    });
                }
            }

            if !sections.is_empty() {
                tracing::info!(
                    "Loaded {} cached sections for source {}",
                    sections.len(),
                    source.name
                );
                cached_sections.push((source_id, sections));
            }
        }

        tracing::info!(
            "Loaded {} sources with cached sections",
            cached_sections.len()
        );

        cached_sections
    }

    /// Convert database MediaItemModel to MediaItem enum
    fn db_model_to_media_item(model: crate::db::entities::MediaItemModel) -> MediaItem {
        use chrono::{DateTime, Utc};

        match model.media_type.as_str() {
            "movie" => MediaItem::Movie(Movie {
                id: model.id.clone(),
                backend_id: model.source_id.clone(),
                title: model.title,
                year: model.year.map(|y| y as u32),
                overview: model.overview,
                rating: model.rating,
                duration: std::time::Duration::from_millis(model.duration_ms.unwrap_or(0) as u64),
                poster_url: model.poster_url,
                backdrop_url: model.backdrop_url,
                genres: Vec::new(), // TODO: Extract from JSON if needed
                cast: Vec::new(),
                crew: Vec::new(),
                added_at: model.added_at.map(|dt| {
                    DateTime::<Utc>::from_timestamp(dt.and_utc().timestamp(), 0).unwrap()
                }),
                updated_at: Some(
                    DateTime::<Utc>::from_timestamp(model.updated_at.and_utc().timestamp(), 0)
                        .unwrap(),
                ),
                watched: false, // Would need to fetch from playback progress
                view_count: 0,
                last_watched_at: None,
                playback_position: None,
                intro_marker: None,
                credits_marker: None,
            }),
            "show" => MediaItem::Show(Show {
                id: model.id.clone(),
                backend_id: model.source_id.clone(),
                title: model.title,
                year: model.year.map(|y| y as u32),
                overview: model.overview,
                rating: model.rating,
                poster_url: model.poster_url,
                backdrop_url: model.backdrop_url,
                genres: Vec::new(), // TODO: Extract from JSON if needed
                seasons: Vec::new(),
                cast: Vec::new(),
                added_at: model.added_at.map(|dt| {
                    DateTime::<Utc>::from_timestamp(dt.and_utc().timestamp(), 0).unwrap()
                }),
                updated_at: Some(
                    DateTime::<Utc>::from_timestamp(model.updated_at.and_utc().timestamp(), 0)
                        .unwrap(),
                ),
                watched_episode_count: 0, // Would need to calculate from episodes
                total_episode_count: 0,
                last_watched_at: None,
            }),
            "episode" => MediaItem::Episode(Episode {
                id: model.id.clone(),
                backend_id: model.source_id.clone(),
                show_id: model.parent_id.clone(),
                season_number: model.season_number.unwrap_or(1) as u32,
                episode_number: model.episode_number.unwrap_or(1) as u32,
                title: model.title,
                overview: model.overview,
                air_date: None,
                duration: std::time::Duration::from_millis(model.duration_ms.unwrap_or(0) as u64),
                thumbnail_url: model.poster_url.clone(),
                show_poster_url: None, // Would need to fetch from parent show
                watched: false,        // Would need to fetch from playback progress
                view_count: 0,
                last_watched_at: None,
                playback_position: None,
                show_title: None,
                intro_marker: None,
                credits_marker: None,
            }),
            _ => {
                // Unknown type, default to movie
                MediaItem::Movie(Movie {
                    id: model.id.clone(),
                    backend_id: model.source_id.clone(),
                    title: model.title,
                    year: model.year.map(|y| y as u32),
                    overview: model.overview,
                    rating: model.rating,
                    duration: std::time::Duration::from_millis(
                        model.duration_ms.unwrap_or(0) as u64
                    ),
                    poster_url: model.poster_url,
                    backdrop_url: model.backdrop_url,
                    genres: Vec::new(),
                    cast: Vec::new(),
                    crew: Vec::new(),
                    added_at: model.added_at.map(|dt| {
                        DateTime::<Utc>::from_timestamp(dt.and_utc().timestamp(), 0).unwrap()
                    }),
                    updated_at: Some(
                        DateTime::<Utc>::from_timestamp(model.updated_at.and_utc().timestamp(), 0)
                            .unwrap(),
                    ),
                    watched: false,
                    view_count: 0,
                    last_watched_at: None,
                    playback_position: None,
                    intro_marker: None,
                    credits_marker: None,
                })
            }
        }
    }
}
