use crate::db::entities::{libraries, sources};
use crate::models::MediaItem;
use crate::services::DataService;
use anyhow::Result;
use std::sync::Arc;

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

pub async fn execute_command(command: AppCommand, data_service: Arc<DataService>) -> CommandResult {
    match command {
        AppCommand::LoadInitialData => match load_initial_data(data_service).await {
            Ok((sources, libraries)) => CommandResult::InitialDataLoaded { sources, libraries },
            Err(e) => CommandResult::Error(e.to_string()),
        },
        AppCommand::LoadSources => match data_service.get_all_sources().await {
            Ok(sources) => CommandResult::SourcesLoaded(sources),
            Err(e) => CommandResult::Error(e.to_string()),
        },
        AppCommand::LoadLibraries { source_id } => {
            match data_service.get_libraries(&source_id).await {
                Ok(libraries) => CommandResult::LibrariesLoaded {
                    source_id,
                    libraries,
                },
                Err(e) => CommandResult::Error(e.to_string()),
            }
        }
        AppCommand::LoadMediaItems { library_id } => {
            match data_service.get_media_items(&library_id).await {
                Ok(items) => CommandResult::MediaItemsLoaded { library_id, items },
                Err(e) => CommandResult::Error(e.to_string()),
            }
        }
        AppCommand::LoadHomeData => match load_home_data(data_service).await {
            Ok((continue_watching, recently_added, trending)) => CommandResult::HomeDataLoaded {
                continue_watching,
                recently_added,
                trending,
            },
            Err(e) => CommandResult::Error(e.to_string()),
        },
        AppCommand::LoadMovieDetails { media_id } => {
            match data_service.get_media_item(&media_id).await {
                Ok(Some(item)) => CommandResult::MovieDetailsLoaded(item),
                Ok(None) => CommandResult::Error(format!("Movie {} not found", media_id)),
                Err(e) => CommandResult::Error(e.to_string()),
            }
        }
        AppCommand::LoadShowDetails { media_id } => {
            match data_service.get_media_item(&media_id).await {
                Ok(Some(item)) => CommandResult::ShowDetailsLoaded(item),
                Ok(None) => CommandResult::Error(format!("Show {} not found", media_id)),
                Err(e) => CommandResult::Error(e.to_string()),
            }
        }
        AppCommand::StartPlayback { media_id } => {
            match start_playback(data_service, &media_id).await {
                Ok(url) => CommandResult::PlaybackStarted { media_id, url },
                Err(e) => CommandResult::Error(e.to_string()),
            }
        }
        AppCommand::UpdateProgress { media_id, position } => {
            // Convert position (0.0-1.0) to milliseconds assuming a 2 hour movie
            let position_ms = (position * 7200000.0) as i64;
            let duration_ms = 7200000i64; // 2 hours in ms
            match data_service
                .update_playback_progress(&media_id, position_ms, duration_ms, position > 0.9)
                .await
            {
                Ok(_) => CommandResult::ProgressUpdated { media_id, position },
                Err(e) => CommandResult::Error(e.to_string()),
            }
        }
    }
}

async fn load_initial_data(
    data_service: Arc<DataService>,
) -> Result<(Vec<sources::Model>, Vec<libraries::Model>)> {
    let sources = data_service.get_all_sources().await?;
    let mut all_libraries = Vec::new();

    for source in &sources {
        if let Ok(libraries) = data_service.get_libraries(&source.id).await {
            all_libraries.extend(libraries);
        }
    }

    Ok((sources, all_libraries))
}

async fn load_home_data(
    data_service: Arc<DataService>,
) -> Result<(Vec<MediaItem>, Vec<MediaItem>, Vec<MediaItem>)> {
    let continue_watching = data_service.get_continue_watching().await?;
    let recently_added = data_service.get_recently_added(None).await?;
    let trending = Vec::new(); // TODO: Implement trending

    Ok((continue_watching, recently_added, trending))
}

async fn start_playback(_data_service: Arc<DataService>, media_id: &str) -> Result<String> {
    // TODO: Get actual stream URL from backend
    Ok(format!("stream://{}", media_id))
}
