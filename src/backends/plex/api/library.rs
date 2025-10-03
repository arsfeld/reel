use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use std::time::Duration;
use tracing::info;

use super::client::PlexApi;
use super::types::*;
use crate::models::{Episode, Library, LibraryType, Movie, Season, Show};

impl PlexApi {
    pub async fn get_libraries(&self) -> Result<Vec<Library>> {
        let url = self.build_url("/library/sections");

        let response = self
            .client
            .get(&url)
            .headers(self.standard_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get libraries: {}", response.status()));
        }

        let plex_response: PlexLibrariesResponse = response.json().await?;

        let libraries: Vec<Library> = plex_response
            .media_container
            .directory
            .into_iter()
            .map(|dir| Library {
                id: dir.key,
                title: dir.title,
                library_type: match dir.library_type.as_str() {
                    "movie" => LibraryType::Movies,
                    "show" => LibraryType::Shows,
                    "artist" => LibraryType::Music,
                    "photo" => LibraryType::Photos,
                    _ => LibraryType::Mixed,
                },
                icon: None,
                item_count: 0, // Plex doesn't provide count in library listing
            })
            .collect();

        info!("Found {} libraries", libraries.len());
        Ok(libraries)
    }

    /// Get all movies from a library
    pub async fn get_movies(&self, library_id: &str) -> Result<Vec<Movie>> {
        let url = self.build_url(&format!("/library/sections/{}/all", library_id));

        let response = self
            .client
            .get(&url)
            .headers(self.standard_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get movies: {}", response.status()));
        }

        let plex_response: PlexMoviesResponse = response.json().await?;

        let movies: Vec<Movie> = plex_response
            .media_container
            .metadata
            .into_iter()
            .map(|meta| {
                let duration_ms = meta.duration.unwrap_or(0);
                let duration = Duration::from_millis(duration_ms as u64);

                // Consider watched if view_count > 0 or view_offset is close to duration
                let watched = meta.view_count.unwrap_or(0) > 0
                    || (meta.view_offset.is_some()
                        && duration_ms > 0
                        && meta.view_offset.unwrap_or(0) as f64 / duration_ms as f64 > 0.9);

                Movie {
                    id: meta.rating_key,
                    backend_id: self.backend_id.clone(),
                    title: meta.title,
                    year: meta.year.map(|y| y as u32),
                    duration,
                    rating: meta.rating.map(|r| r as f32),
                    poster_url: meta.thumb.map(|t| self.build_image_url(&t)),
                    backdrop_url: meta.art.map(|a| self.build_image_url(&a)),
                    overview: meta.summary,
                    genres: meta.genres.into_iter().map(|g| g.tag).collect(),
                    cast: vec![], // TODO: Fetch cast details
                    crew: vec![], // TODO: Fetch crew details
                    added_at: meta.added_at.and_then(|ts| DateTime::from_timestamp(ts, 0)),
                    updated_at: meta
                        .updated_at
                        .and_then(|ts| DateTime::from_timestamp(ts, 0)),
                    watched,
                    view_count: meta.view_count.unwrap_or(0),
                    last_watched_at: meta
                        .last_viewed_at
                        .and_then(|ts| DateTime::from_timestamp(ts, 0)),
                    playback_position: meta.view_offset.map(|v| Duration::from_millis(v as u64)),
                    intro_marker: None,   // Will be fetched when playing
                    credits_marker: None, // Will be fetched when playing
                }
            })
            .collect();

        info!("Found {} movies in library {}", movies.len(), library_id);
        Ok(movies)
    }

    /// Get all TV shows from a library
    pub async fn get_shows(&self, library_id: &str) -> Result<Vec<Show>> {
        let url = self.build_url(&format!("/library/sections/{}/all", library_id));

        let response = self
            .client
            .get(&url)
            .headers(self.standard_headers())
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
                backend_id: self.backend_id.clone(),
                title: meta.title,
                year: meta.year.map(|y| y as u32),
                seasons,
                rating: meta.rating.map(|r| r as f32),
                poster_url: meta.thumb.map(|t| self.build_image_url(&t)),
                backdrop_url: meta.art.map(|a| self.build_image_url(&a)),
                overview: meta.summary,
                genres: meta.genres.into_iter().map(|g| g.tag).collect(),
                cast: vec![], // TODO: Fetch cast details
                added_at: meta.added_at.and_then(|ts| DateTime::from_timestamp(ts, 0)),
                updated_at: meta
                    .updated_at
                    .and_then(|ts| DateTime::from_timestamp(ts, 0)),
                watched_episode_count: meta.viewed_leaf_count.unwrap_or(0) as u32,
                total_episode_count: meta.leaf_count.unwrap_or(0) as u32,
                last_watched_at: None, // TODO: Fetch from episodes
            });
        }

        info!("Found {} shows in library {}", shows.len(), library_id);
        Ok(shows)
    }

    /// Get seasons for a TV show
    pub async fn get_seasons(&self, show_id: &str) -> Result<Vec<Season>> {
        let url = self.build_url(&format!("/library/metadata/{}/children", show_id));

        let response = self
            .client
            .get(&url)
            .headers(self.standard_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get seasons: {}", response.status()));
        }

        let plex_response: PlexSeasonsResponse = response.json().await?;

        let seasons = plex_response
            .media_container
            .metadata
            .into_iter()
            .map(|meta| Season {
                id: meta.rating_key,
                season_number: meta.index as u32,
                episode_count: meta.leaf_count.unwrap_or(0) as u32,
                poster_url: meta.thumb.map(|t| self.build_image_url(&t)),
            })
            .collect();

        Ok(seasons)
    }

    /// Get episodes for a season
    pub async fn get_episodes(&self, season_id: &str) -> Result<Vec<Episode>> {
        let url = self.build_url(&format!("/library/metadata/{}/children", season_id));

        let response = self
            .client
            .get(&url)
            .headers(self.standard_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get episodes: {}", response.status()));
        }

        let plex_response: PlexEpisodesResponse = response.json().await?;

        let episodes = plex_response
            .media_container
            .metadata
            .into_iter()
            .map(|meta| {
                let duration_ms = meta.duration.unwrap_or(0);
                let duration = Duration::from_millis(duration_ms as u64);

                // Consider watched if view_count > 0 or view_offset is close to duration
                let watched = meta.view_count.unwrap_or(0) > 0
                    || (meta.view_offset.is_some()
                        && duration_ms > 0
                        && meta.view_offset.unwrap_or(0) as f64 / duration_ms as f64 > 0.9);

                Episode {
                    id: meta.rating_key,
                    backend_id: self.backend_id.clone(),
                    show_id: None, // Not provided in response
                    title: meta.title,
                    season_number: meta.parent_index.unwrap_or(0) as u32,
                    episode_number: meta.index as u32,
                    duration,
                    thumbnail_url: meta.thumb.map(|t| self.build_image_url(&t)),
                    overview: meta.summary,
                    air_date: meta
                        .aired_at
                        .and_then(|date| DateTime::parse_from_rfc3339(&date).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    watched,
                    view_count: meta.view_count.unwrap_or(0),
                    last_watched_at: meta
                        .last_viewed_at
                        .and_then(|ts| DateTime::from_timestamp(ts, 0)),
                    playback_position: meta.view_offset.map(|v| Duration::from_millis(v as u64)),
                    show_title: None,
                    show_poster_url: None,
                    intro_marker: None,   // Will be fetched when playing
                    credits_marker: None, // Will be fetched when playing
                }
            })
            .collect();

        Ok(episodes)
    }
}
