use crate::backends::{jellyfin::JellyfinBackend, plex::PlexBackend, traits::MediaBackend};
use crate::db::connection::DatabaseConnection;
use crate::db::repository::{
    Repository,
    media_repository::MediaRepositoryImpl,
    people_repository::PeopleRepository,
    source_repository::{SourceRepository, SourceRepositoryImpl},
};
use crate::models::{
    AuthProvider, ConnectionInfo, Credentials, HomeSection, MediaItemId, Source, SourceId,
    SourceType, StreamInfo,
};
use crate::services::core::auth::AuthService;
use anyhow::{Context, Result};

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

    /// Get stream URL for a specific quality option - pure function
    pub async fn get_stream_with_quality(
        db: &DatabaseConnection,
        media_item_id: &MediaItemId,
        quality: &crate::models::QualityOption,
    ) -> Result<String> {
        use crate::backends::plex::PlexBackend;

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

        // Create backend and get stream URL for specific quality
        let backend = Self::create_backend_for_source(db, &source_entity).await?;

        // Check connection type from ConnectionService cache
        let is_local = {
            use crate::services::core::connection::ConnectionService;
            let cache = ConnectionService::cache();
            let source_id = SourceId::new(source_entity.id.clone());
            if let Some(state) = cache.get(&source_id).await {
                state.is_local()
            } else {
                false // Default to remote if no cached connection
            }
        };

        // Get stream URL for quality (currently only Plex supports this)
        if let Some(plex_backend) = backend.as_any().downcast_ref::<PlexBackend>() {
            let api = plex_backend
                .get_api_for_playqueue()
                .await
                .ok_or_else(|| anyhow::anyhow!("Plex API not available"))?;

            api.get_stream_url_for_quality(media_item_id.as_ref(), quality, is_local)
                .await
        } else {
            // For non-Plex backends, fall back to the URL in the quality option
            Ok(quality.url.clone())
        }
    }

    /// Create a backend instance for a source - stateless factory
    pub async fn create_backend_for_source(
        db: &DatabaseConnection,
        source_entity: &crate::db::entities::sources::Model,
    ) -> Result<Box<dyn MediaBackend>> {
        // Load credentials from secure storage
        let source_id = SourceId::new(source_entity.id.clone());
        let credentials = AuthService::load_credentials(db, &source_id)
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
                if backend.has_url_changed().await
                    && let Some(new_url) = backend.get_current_url().await
                {
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
                connection_quality: None, // Will be set by ConnectionMonitor
            },
            enabled: true,
            last_sync: entity.last_sync.map(|dt| dt.and_utc()),
            library_count: 0,
        }
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

    /// Load full metadata (including full cast/crew) for a movie and update database
    pub async fn load_full_movie_metadata(
        db: &DatabaseConnection,
        movie_id: &MediaItemId,
    ) -> Result<()> {
        use crate::db::entities::media_people::Model as MediaPeopleModel;
        use crate::db::entities::people::Model as PeopleModel;
        use crate::db::repository::PeopleRepositoryImpl;

        // Load media item to find its source
        let media_repo = MediaRepositoryImpl::new(db.clone());
        let media_item = media_repo
            .find_by_id(movie_id.as_str())
            .await?
            .ok_or_else(|| anyhow::anyhow!("Movie not found"))?;

        // Load source configuration
        let source_repo = SourceRepositoryImpl::new(db.clone());
        let source_entity = source_repo
            .find_by_id(&media_item.source_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Source not found"))?;

        // Create backend and get full metadata
        let backend = Self::create_backend_for_source(db, &source_entity).await?;
        let full_movie = backend.get_movie_metadata(movie_id).await?;

        // Update database with full cast/crew
        let people_repo = PeopleRepositoryImpl::new(db.clone());

        let mut media_people = Vec::new();
        let now = chrono::Utc::now().naive_utc();

        // Upsert cast people and create relationships
        for (idx, person) in full_movie.cast.iter().enumerate() {
            let people_model = PeopleModel {
                id: person.id.clone(),
                name: person.name.clone(),
                image_url: person.image_url.clone(),
                created_at: now,
                updated_at: now,
            };
            people_repo.upsert(people_model).await?;

            media_people.push(MediaPeopleModel {
                id: 0, // Will be auto-generated
                media_item_id: movie_id.to_string(),
                person_id: person.id.clone(),
                person_type: "cast".to_string(),
                role: person.role.clone(),
                sort_order: Some(idx as i32),
            });
        }

        // Upsert crew people and create relationships
        for (idx, person) in full_movie.crew.iter().enumerate() {
            let people_model = PeopleModel {
                id: person.id.clone(),
                name: person.name.clone(),
                image_url: person.image_url.clone(),
                created_at: now,
                updated_at: now,
            };
            people_repo.upsert(people_model).await?;

            media_people.push(MediaPeopleModel {
                id: 0, // Will be auto-generated
                media_item_id: movie_id.to_string(),
                person_id: person.id.clone(),
                person_type: "crew".to_string(),
                role: person.role.clone(),
                sort_order: Some(idx as i32),
            });
        }

        // Save all media-people relationships (replaces existing)
        people_repo
            .save_media_people(movie_id.as_str(), media_people)
            .await?;

        Ok(())
    }

    /// Load full metadata (including full cast/crew) for a show and update database
    pub async fn load_full_show_metadata(
        db: &DatabaseConnection,
        show_id: &crate::models::ShowId,
    ) -> Result<()> {
        use crate::db::entities::media_people::Model as MediaPeopleModel;
        use crate::db::entities::people::Model as PeopleModel;
        use crate::db::repository::PeopleRepositoryImpl;

        // Convert ShowId to string for database lookup
        let show_id_str = show_id.to_string();

        // Load media item to find its source
        let media_repo = MediaRepositoryImpl::new(db.clone());
        let media_item = media_repo
            .find_by_id(&show_id_str)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Show not found"))?;

        // Load source configuration
        let source_repo = SourceRepositoryImpl::new(db.clone());
        let source_entity = source_repo
            .find_by_id(&media_item.source_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Source not found"))?;

        // Create backend and get full metadata
        let backend = Self::create_backend_for_source(db, &source_entity).await?;
        let full_show = backend.get_show_metadata(show_id).await?;

        // Update database with full cast
        let people_repo = PeopleRepositoryImpl::new(db.clone());

        let mut media_people = Vec::new();
        let now = chrono::Utc::now().naive_utc();

        // Upsert cast people and create relationships
        for (idx, person) in full_show.cast.iter().enumerate() {
            let people_model = PeopleModel {
                id: person.id.clone(),
                name: person.name.clone(),
                image_url: person.image_url.clone(),
                created_at: now,
                updated_at: now,
            };
            people_repo.upsert(people_model).await?;

            media_people.push(MediaPeopleModel {
                id: 0, // Will be auto-generated
                media_item_id: show_id_str.clone(),
                person_id: person.id.clone(),
                person_type: "cast".to_string(),
                role: person.role.clone(),
                sort_order: Some(idx as i32),
            });
        }

        // Save all media-people relationships (replaces existing)
        people_repo
            .save_media_people(&show_id_str, media_people)
            .await?;

        Ok(())
    }

    /// Get home sections per source with individual error handling
    pub async fn get_home_sections_per_source(
        db: &DatabaseConnection,
    ) -> Vec<(
        crate::models::SourceId,
        Result<Vec<crate::models::HomeSectionWithModels>>,
    )> {
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
                                Ok(sections) => {
                                    // Convert MediaItem to MediaItemModel and save to database
                                    let media_repo = MediaRepositoryImpl::new(db_clone.clone());
                                    let mut converted_sections = Vec::new();

                                    for mut section in sections {
                                        // Prefix section IDs with source ID to avoid conflicts
                                        section.id = format!("{}::{}", source_clone.id, section.id);
                                        // Also prefix the title with source name if multiple sources exist
                                        if sources_count > 1 {
                                            section.title = format!(
                                                "{} - {}",
                                                source_clone.name, section.title
                                            );
                                        }

                                        // Convert items to MediaItemModel
                                        let mut db_items = Vec::new();
                                        for item in section.items {
                                            // Convert MediaItem to database model using the mapper
                                            let db_model = item.to_model(
                                                &source_clone.id,
                                                None // library_id - we'll fetch it if needed
                                            );

                                            // Save or update in database
                                            // Check if item exists first
                                            let saved_model = match media_repo.find_by_id(&db_model.id).await {
                                                Ok(Some(existing)) => {
                                                    // Update existing, preserving library_id
                                                    let mut update_model = db_model.clone();
                                                    update_model.library_id = existing.library_id.clone();
                                                    match media_repo.update(update_model.clone()).await {
                                                        Ok(model) => model,
                                                        Err(e) => {
                                                            tracing::warn!("Failed to update media item {}: {}", db_model.id, e);
                                                            // Return the existing model on update failure
                                                            existing
                                                        }
                                                    }
                                                }
                                                Ok(None) => {
                                                    // Insert new - skip if we don't have a library_id
                                                    if db_model.library_id.is_empty() {
                                                        tracing::debug!("Skipping insert for media item {} without library_id", db_model.id);
                                                        continue;
                                                    }
                                                    match media_repo.insert(db_model.clone()).await {
                                                        Ok(model) => model,
                                                        Err(e) => {
                                                            tracing::warn!("Failed to insert media item {}: {}", db_model.id, e);
                                                            continue;
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    tracing::warn!("Failed to check media item {}: {}", db_model.id, e);
                                                    continue;
                                                }
                                            };
                                            db_items.push(saved_model);
                                        }

                                        // Create new section with MediaItemModel
                                        // Only add sections that have items (match cached behavior)
                                        if !db_items.is_empty() {
                                            let converted_section = crate::models::HomeSectionWithModels {
                                                id: section.id,
                                                title: section.title,
                                                section_type: section.section_type,
                                                items: db_items,
                                            };
                                            converted_sections.push(converted_section);
                                        }
                                    }

                                    Ok(converted_sections)
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
}
