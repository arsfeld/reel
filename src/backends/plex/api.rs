use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

use crate::models::{
    Library, LibraryType, Movie, Show, Season, Episode, Person, StreamInfo, Resolution
};

const PLEX_HEADERS: &[(&str, &str)] = &[
    ("X-Plex-Product", "Reel"),
    ("X-Plex-Version", "0.1.0"),
    ("X-Plex-Client-Identifier", "reel-media-player"),
    ("X-Plex-Platform", "Linux"),
    ("Accept", "application/json"),
];

pub struct PlexApi {
    client: reqwest::Client,
    base_url: String,
    auth_token: String,
}

impl PlexApi {
    pub fn new(base_url: String, auth_token: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        
        Self {
            client,
            base_url,
            auth_token,
        }
    }
    
    /// Get all libraries from the Plex server
    pub async fn get_libraries(&self) -> Result<Vec<Library>> {
        let url = format!("{}/library/sections", self.base_url);
        
        let response = self.client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("Accept", "application/json")
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to get libraries: {}", response.status()));
        }
        
        let plex_response: PlexLibrariesResponse = response.json().await?;
        
        let libraries: Vec<Library> = plex_response.media_container.directory
            .into_iter()
            .map(|dir| Library {
                id: dir.key,
                title: dir.title,
                library_type: match dir.type_.as_str() {
                    "movie" => LibraryType::Movies,
                    "show" => LibraryType::Shows,
                    "artist" => LibraryType::Music,
                    "photo" => LibraryType::Photos,
                    _ => LibraryType::Mixed,
                },
                icon: dir.thumb,
            })
            .collect();
        
        info!("Found {} libraries", libraries.len());
        Ok(libraries)
    }
    
    /// Get all movies from a library
    pub async fn get_movies(&self, library_id: &str) -> Result<Vec<Movie>> {
        let url = format!("{}/library/sections/{}/all", self.base_url, library_id);
        
        let response = self.client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("Accept", "application/json")
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to get movies: {}", response.status()));
        }
        
        let plex_response: PlexMoviesResponse = response.json().await?;
        
        let movies: Vec<Movie> = plex_response.media_container.metadata
            .into_iter()
            .map(|meta| {
                let duration_ms = meta.duration.unwrap_or(0);
                Movie {
                    id: meta.rating_key,
                    title: meta.title,
                    year: meta.year,
                    duration: Duration::from_millis(duration_ms),
                    rating: meta.rating,
                    poster_url: meta.thumb.map(|t| self.build_image_url(&t)),
                    backdrop_url: meta.art.map(|a| self.build_image_url(&a)),
                    overview: meta.summary,
                    genres: meta.genre.unwrap_or_default()
                        .into_iter()
                        .map(|g| g.tag)
                        .collect(),
                    cast: vec![], // TODO: Fetch cast details
                    crew: vec![], // TODO: Fetch crew details
                    added_at: meta.added_at.and_then(|ts| DateTime::from_timestamp(ts, 0)),
                    updated_at: meta.updated_at.and_then(|ts| DateTime::from_timestamp(ts, 0)),
                }
            })
            .collect();
        
        info!("Found {} movies in library {}", movies.len(), library_id);
        Ok(movies)
    }
    
    /// Get all TV shows from a library
    pub async fn get_shows(&self, library_id: &str) -> Result<Vec<Show>> {
        let url = format!("{}/library/sections/{}/all", self.base_url, library_id);
        
        let response = self.client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("Accept", "application/json")
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to get shows: {}", response.status()));
        }
        
        let plex_response: PlexShowsResponse = response.json().await?;
        
        let mut shows = Vec::new();
        for meta in plex_response.media_container.metadata {
            // Fetch seasons for each show
            let seasons = self.get_seasons(&meta.rating_key).await?;
            
            shows.push(Show {
                id: meta.rating_key,
                title: meta.title,
                year: meta.year,
                seasons,
                rating: meta.rating,
                poster_url: meta.thumb.map(|t| self.build_image_url(&t)),
                backdrop_url: meta.art.map(|a| self.build_image_url(&a)),
                overview: meta.summary,
                genres: meta.genre.unwrap_or_default()
                    .into_iter()
                    .map(|g| g.tag)
                    .collect(),
                cast: vec![], // TODO: Fetch cast details
                added_at: meta.added_at.and_then(|ts| DateTime::from_timestamp(ts, 0)),
                updated_at: meta.updated_at.and_then(|ts| DateTime::from_timestamp(ts, 0)),
            });
        }
        
        info!("Found {} shows in library {}", shows.len(), library_id);
        Ok(shows)
    }
    
    /// Get seasons for a TV show
    async fn get_seasons(&self, show_id: &str) -> Result<Vec<Season>> {
        let url = format!("{}/library/metadata/{}/children", self.base_url, show_id);
        
        let response = self.client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("Accept", "application/json")
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to get seasons: {}", response.status()));
        }
        
        let plex_response: PlexSeasonsResponse = response.json().await?;
        
        let seasons = plex_response.media_container.metadata
            .into_iter()
            .map(|meta| Season {
                id: meta.rating_key,
                season_number: meta.index.unwrap_or(0),
                episode_count: meta.leaf_count.unwrap_or(0),
                poster_url: meta.thumb.map(|t| self.build_image_url(&t)),
            })
            .collect();
        
        Ok(seasons)
    }
    
    /// Get episodes for a season
    pub async fn get_episodes(&self, season_id: &str) -> Result<Vec<Episode>> {
        let url = format!("{}/library/metadata/{}/children", self.base_url, season_id);
        
        let response = self.client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("Accept", "application/json")
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to get episodes: {}", response.status()));
        }
        
        let plex_response: PlexEpisodesResponse = response.json().await?;
        
        let episodes = plex_response.media_container.metadata
            .into_iter()
            .map(|meta| {
                let duration_ms = meta.duration.unwrap_or(0);
                Episode {
                    id: meta.rating_key,
                    title: meta.title,
                    season_number: meta.parent_index.unwrap_or(0),
                    episode_number: meta.index.unwrap_or(0),
                    duration: Duration::from_millis(duration_ms),
                    thumbnail_url: meta.thumb.map(|t| self.build_image_url(&t)),
                    overview: meta.summary,
                    air_date: meta.originally_available_at
                        .and_then(|date| DateTime::parse_from_rfc3339(&date).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                }
            })
            .collect();
        
        Ok(episodes)
    }
    
    /// Get stream URL for a media item
    pub async fn get_stream_url(&self, media_id: &str) -> Result<StreamInfo> {
        // For Plex, we can usually direct play
        // This is a simplified version - real implementation would check transcoding needs
        let url = format!("{}/library/metadata/{}", self.base_url, media_id);
        
        let response = self.client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("Accept", "application/json")
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to get media info: {}", response.status()));
        }
        
        let plex_response: PlexMediaResponse = response.json().await?;
        
        if let Some(metadata) = plex_response.media_container.metadata.first() {
            if let Some(media) = metadata.media.as_ref().and_then(|m| m.first()) {
                if let Some(part) = media.part.as_ref().and_then(|p| p.first()) {
                    let stream_url = format!("{}{}?X-Plex-Token={}", 
                        self.base_url, 
                        part.key, 
                        self.auth_token
                    );
                    
                    return Ok(StreamInfo {
                        url: stream_url,
                        direct_play: true,
                        video_codec: media.video_codec.clone().unwrap_or_default(),
                        audio_codec: media.audio_codec.clone().unwrap_or_default(),
                        container: part.container.clone().unwrap_or_default(),
                        bitrate: media.bitrate.unwrap_or(0),
                        resolution: Resolution {
                            width: media.width.unwrap_or(0),
                            height: media.height.unwrap_or(0),
                        },
                    });
                }
            }
        }
        
        Err(anyhow!("Failed to get stream info for media"))
    }
    
    /// Update playback progress
    pub async fn update_progress(&self, media_id: &str, position: Duration) -> Result<()> {
        let url = format!("{}/:/timeline", self.base_url);
        let position_ms = position.as_millis() as u64;
        
        let response = self.client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .query(&[
                ("ratingKey", media_id),
                ("key", &format!("/library/metadata/{}", media_id)),
                ("state", "playing"),
                ("time", &position_ms.to_string()),
            ])
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Failed to update progress: {}", response.status()));
        }
        
        Ok(())
    }
    
    /// Build full image URL from Plex path
    fn build_image_url(&self, path: &str) -> String {
        if path.starts_with("http") {
            path.to_string()
        } else {
            format!("{}{}?X-Plex-Token={}", self.base_url, path, self.auth_token)
        }
    }
}

// Plex API Response Types

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlexLibrariesResponse {
    media_container: PlexLibrariesContainer,
}

#[derive(Debug, Deserialize)]
struct PlexLibrariesContainer {
    #[serde(rename = "Directory", default)]
    directory: Vec<PlexLibraryDirectory>,
}

#[derive(Debug, Deserialize)]
struct PlexLibraryDirectory {
    key: String,
    title: String,
    #[serde(rename = "type")]
    type_: String,
    thumb: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlexMoviesResponse {
    media_container: PlexMoviesContainer,
}

#[derive(Debug, Deserialize)]
struct PlexMoviesContainer {
    #[serde(rename = "Metadata", default)]
    metadata: Vec<PlexMovieMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlexMovieMetadata {
    rating_key: String,
    title: String,
    year: Option<u32>,
    duration: Option<u64>,
    rating: Option<f32>,
    thumb: Option<String>,
    art: Option<String>,
    summary: Option<String>,
    #[serde(rename = "Genre", default)]
    genre: Option<Vec<PlexTag>>,
    added_at: Option<i64>,
    updated_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct PlexTag {
    tag: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlexShowsResponse {
    media_container: PlexShowsContainer,
}

#[derive(Debug, Deserialize)]
struct PlexShowsContainer {
    #[serde(rename = "Metadata", default)]
    metadata: Vec<PlexShowMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlexShowMetadata {
    rating_key: String,
    title: String,
    year: Option<u32>,
    rating: Option<f32>,
    thumb: Option<String>,
    art: Option<String>,
    summary: Option<String>,
    #[serde(rename = "Genre", default)]
    genre: Option<Vec<PlexTag>>,
    added_at: Option<i64>,
    updated_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlexSeasonsResponse {
    media_container: PlexSeasonsContainer,
}

#[derive(Debug, Deserialize)]
struct PlexSeasonsContainer {
    #[serde(rename = "Metadata", default)]
    metadata: Vec<PlexSeasonMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlexSeasonMetadata {
    rating_key: String,
    index: Option<u32>,
    leaf_count: Option<u32>,
    thumb: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlexEpisodesResponse {
    media_container: PlexEpisodesContainer,
}

#[derive(Debug, Deserialize)]
struct PlexEpisodesContainer {
    #[serde(rename = "Metadata", default)]
    metadata: Vec<PlexEpisodeMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlexEpisodeMetadata {
    rating_key: String,
    title: String,
    parent_index: Option<u32>,
    index: Option<u32>,
    duration: Option<u64>,
    thumb: Option<String>,
    summary: Option<String>,
    originally_available_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlexMediaResponse {
    media_container: PlexMediaContainer,
}

#[derive(Debug, Deserialize)]
struct PlexMediaContainer {
    #[serde(rename = "Metadata", default)]
    metadata: Vec<PlexMediaMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlexMediaMetadata {
    rating_key: String,
    #[serde(rename = "Media", default)]
    media: Option<Vec<PlexMedia>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlexMedia {
    bitrate: Option<u64>,
    width: Option<u32>,
    height: Option<u32>,
    video_codec: Option<String>,
    audio_codec: Option<String>,
    #[serde(rename = "Part", default)]
    part: Option<Vec<PlexPart>>,
}

#[derive(Debug, Deserialize)]
struct PlexPart {
    key: String,
    container: Option<String>,
}