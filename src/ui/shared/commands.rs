use crate::db::connection::DatabaseConnection;
use crate::db::entities::{libraries, sources};
use crate::db::repository::{Repository, SourceRepositoryImpl};
use crate::models::{LibraryId, MediaItem, MediaItemId, SourceId};
use crate::services::core::backend::BackendService;
use crate::services::core::media::MediaService;
use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub enum AppCommand {
    LoadInitialData,
    LoadSources,
    LoadLibraries { source_id: String },
    LoadMediaItems { library_id: String },
    LoadHomeData,
    LoadMovieDetails { media_id: String },
    LoadShowDetails { media_id: String },
    StartPlayback { media_id: String },
    UpdateProgress { media_id: String, position: f64 },
}

#[derive(Debug, Clone)]
pub enum CommandResult {
    InitialDataLoaded {
        sources: Vec<sources::Model>,
        libraries: Vec<libraries::Model>,
    },
    SourcesLoaded(Vec<sources::Model>),
    LibrariesLoaded {
        source_id: String,
        libraries: Vec<libraries::Model>,
    },
    MediaItemsLoaded {
        library_id: String,
        items: Vec<MediaItem>,
    },
    HomeDataLoaded {
        continue_watching: Vec<MediaItem>,
        recently_added: Vec<MediaItem>,
        trending: Vec<MediaItem>,
    },
    MovieDetailsLoaded(MediaItem),
    ShowDetailsLoaded(MediaItem),
    PlaybackStarted {
        media_id: String,
        url: String,
    },
    ProgressUpdated {
        media_id: String,
        position: f64,
    },
    Error(String),
}

pub async fn execute_command(command: AppCommand, db: &DatabaseConnection) -> CommandResult {
    match command {
        AppCommand::LoadInitialData => match load_initial_data(db).await {
            Ok((sources, libraries)) => CommandResult::InitialDataLoaded { sources, libraries },
            Err(e) => CommandResult::Error(e.to_string()),
        },
        AppCommand::LoadSources => match load_all_sources(db).await {
            Ok(sources) => CommandResult::SourcesLoaded(sources),
            Err(e) => CommandResult::Error(e.to_string()),
        },
        AppCommand::LoadLibraries { source_id } => {
            let source_id = SourceId::new(source_id);
            match MediaService::get_libraries_for_source(db, &source_id).await {
                Ok(libraries) => {
                    // Get media repository to calculate item counts
                    use crate::db::repository::MediaRepositoryImpl;
                    let media_repo = MediaRepositoryImpl::new(db.clone());

                    // Convert to entity models with actual item counts
                    let mut library_models = Vec::new();
                    for lib in libraries {
                        // Get actual item count from database
                        let item_count = media_repo.count_by_library(&lib.id).await.unwrap_or(0);

                        library_models.push(libraries::Model {
                            id: lib.id.clone(),
                            source_id: source_id.to_string(),
                            title: lib.title,
                            library_type: match lib.library_type {
                                crate::models::LibraryType::Movies => "movies".to_string(),
                                crate::models::LibraryType::Shows => "shows".to_string(),
                                crate::models::LibraryType::Music => "music".to_string(),
                                crate::models::LibraryType::Photos => "photos".to_string(),
                                crate::models::LibraryType::Mixed => "mixed".to_string(),
                            },
                            icon: lib.icon,
                            item_count: item_count as i32,
                            created_at: chrono::Utc::now().naive_utc(),
                            updated_at: chrono::Utc::now().naive_utc(),
                        });
                    }
                    CommandResult::LibrariesLoaded {
                        source_id: source_id.to_string(),
                        libraries: library_models,
                    }
                }
                Err(e) => CommandResult::Error(e.to_string()),
            }
        }
        AppCommand::LoadMediaItems { library_id } => {
            let library_id = LibraryId::new(library_id);
            match MediaService::get_media_items(db, &library_id, None, 0, 100).await {
                Ok(items) => CommandResult::MediaItemsLoaded {
                    library_id: library_id.to_string(),
                    items,
                },
                Err(e) => CommandResult::Error(e.to_string()),
            }
        }
        AppCommand::LoadHomeData => match load_home_data(db).await {
            Ok((continue_watching, recently_added, trending)) => CommandResult::HomeDataLoaded {
                continue_watching,
                recently_added,
                trending,
            },
            Err(e) => CommandResult::Error(e.to_string()),
        },
        AppCommand::LoadMovieDetails { media_id } => {
            let media_id = MediaItemId::new(media_id);
            match MediaService::get_media_item(db, &media_id).await {
                Ok(Some(item)) => CommandResult::MovieDetailsLoaded(item),
                Ok(None) => CommandResult::Error(format!("Movie {} not found", media_id)),
                Err(e) => CommandResult::Error(e.to_string()),
            }
        }
        AppCommand::LoadShowDetails { media_id } => {
            let media_id = MediaItemId::new(media_id);
            match MediaService::get_media_item(db, &media_id).await {
                Ok(Some(item)) => CommandResult::ShowDetailsLoaded(item),
                Ok(None) => CommandResult::Error(format!("Show {} not found", media_id)),
                Err(e) => CommandResult::Error(e.to_string()),
            }
        }
        AppCommand::StartPlayback { media_id } => match start_playback(db, &media_id).await {
            Ok(url) => CommandResult::PlaybackStarted { media_id, url },
            Err(e) => CommandResult::Error(e.to_string()),
        },
        AppCommand::UpdateProgress { media_id, position } => {
            // Convert position (0.0-1.0) to milliseconds assuming a 2 hour movie
            let _position_ms = (position * 7200000.0) as i64;
            let _duration_ms = 7200000i64; // 2 hours in ms
            match update_progress(db, &media_id, position).await {
                Ok(_) => CommandResult::ProgressUpdated { media_id, position },
                Err(e) => CommandResult::Error(e.to_string()),
            }
        }
    }
}

async fn load_initial_data(
    db: &DatabaseConnection,
) -> Result<(Vec<sources::Model>, Vec<libraries::Model>)> {
    let sources = load_all_sources(db).await?;
    let mut all_libraries = Vec::new();

    // Get media repository to calculate item counts
    use crate::db::repository::MediaRepositoryImpl;
    let media_repo = MediaRepositoryImpl::new(db.clone());

    for source in &sources {
        let source_id = SourceId::new(source.id.clone());
        if let Ok(libraries) = MediaService::get_libraries_for_source(db, &source_id).await {
            // Convert to entity models with actual item counts
            for lib in libraries {
                // Get actual item count from database
                let item_count = media_repo.count_by_library(&lib.id).await.unwrap_or(0);

                all_libraries.push(libraries::Model {
                    id: lib.id.clone(),
                    source_id: source.id.clone(),
                    title: lib.title,
                    library_type: match lib.library_type {
                        crate::models::LibraryType::Movies => "movies".to_string(),
                        crate::models::LibraryType::Shows => "shows".to_string(),
                        crate::models::LibraryType::Music => "music".to_string(),
                        crate::models::LibraryType::Photos => "photos".to_string(),
                        crate::models::LibraryType::Mixed => "mixed".to_string(),
                    },
                    icon: lib.icon,
                    item_count: item_count as i32,
                    created_at: chrono::Utc::now().naive_utc(),
                    updated_at: chrono::Utc::now().naive_utc(),
                });
            }
        }
    }

    Ok((sources, all_libraries))
}

async fn load_all_sources(db: &DatabaseConnection) -> Result<Vec<sources::Model>> {
    let repo = SourceRepositoryImpl::new(db.clone());
    repo.find_all().await
}

async fn load_home_data(
    db: &DatabaseConnection,
) -> Result<(Vec<MediaItem>, Vec<MediaItem>, Vec<MediaItem>)> {
    let continue_watching = MediaService::get_continue_watching(db, 10).await?;
    let recently_added = MediaService::get_recently_added(db, 10).await?;
    let trending = MediaService::get_trending(db, 10).await?;

    Ok((continue_watching, recently_added, trending))
}

async fn start_playback(db: &DatabaseConnection, media_id: &str) -> Result<String> {
    use crate::db::repository::{MediaRepositoryImpl, Repository};
    use crate::services::cache_service::cache_service;

    // Get actual stream URL from backend using stateless BackendService
    let media_item_id = MediaItemId::new(media_id.to_string());

    // Get source_id from the media item
    let media_repo = MediaRepositoryImpl::new(db.clone());
    let media_entity = media_repo
        .find_by_id(&media_item_id.to_string())
        .await?
        .ok_or_else(|| anyhow::anyhow!("Media item not found: {}", media_item_id))?;
    let source_id = SourceId::new(media_entity.source_id);

    // BackendService::get_stream_url handles all the backend creation and URL fetching
    let stream_info = BackendService::get_stream_url(db, &media_item_id).await?;

    // Get cached stream - no fallback
    let cache_handle = cache_service()
        .get_handle()
        .await
        .context("Cache service is not available")?;

    let cached_stream = cache_handle
        .get_cached_stream(source_id, media_item_id, stream_info)
        .await
        .context("Failed to get cached stream")?;

    tracing::info!(
        "Using cached stream for media: {} (cached: {}, complete: {})",
        media_id,
        cached_stream.cached_url.is_some(),
        cached_stream.is_complete
    );

    Ok(cached_stream.playback_url().to_string())
}

async fn update_progress(db: &DatabaseConnection, media_id: &str, position: f64) -> Result<()> {
    // Convert position (0.0-1.0) to milliseconds assuming a 2 hour movie
    let position_ms = (position * 7200000.0) as i64;
    let duration_ms = 7200000i64; // 2 hours in ms
    let media_id = MediaItemId::new(media_id.to_string());

    MediaService::update_playback_progress(db, &media_id, position_ms, duration_ms, position > 0.9)
        .await
}
