use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use std::time::Duration;
use tracing::info;

use super::client::PlexApi;
use super::types::*;
use crate::models::{Episode, Library, LibraryType, Movie, Person, Season, Show};

impl PlexApi {
    pub async fn get_libraries(&self) -> Result<Vec<Library>> {
        let url = self.build_url("/library/sections");

        let response = self
            .execute_get(&url, "get_libraries")
            .await
            .map_err(|e| anyhow!("Failed to get libraries: {}", e))?;

        let plex_response: PlexLibrariesResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse libraries response: {}", e))?;

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
        let url = self.build_url(&format!(
            "/library/sections/{}/all?includeExtras=1&includeRelated=1&includePopularLeaves=1&includeGuids=1",
            library_id
        ));

        let response = self
            .execute_get(&url, "get_movies")
            .await
            .map_err(|e| anyhow!("Failed to get movies from library {}: {}", library_id, e))?;

        let plex_response: PlexMoviesResponse = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse movies response: {}", e))?;

        tracing::info!(
            "Fetched {} movies from Plex library {}",
            plex_response.media_container.metadata.len(),
            library_id
        );

        // Log first movie in detail
        if let Some(first) = plex_response.media_container.metadata.first() {
            tracing::warn!(
                "SAMPLE MOVIE '{}': {} roles, {} directors, {} writers - Role IDs: {:?}",
                first.title,
                first.roles.len(),
                first.directors.len(),
                first.writers.len(),
                first
                    .roles
                    .iter()
                    .map(|r| (r.id, &r.tag))
                    .collect::<Vec<_>>()
            );
        }

        let movies: Vec<Movie> = plex_response
            .media_container
            .metadata
            .into_iter()
            .map(|meta| {
                tracing::debug!(
                    "Movie '{}': {} roles, {} directors, {} writers from bulk API",
                    meta.title,
                    meta.roles.len(),
                    meta.directors.len(),
                    meta.writers.len()
                );

                let duration_ms = meta.duration.unwrap_or(0);
                let duration = Duration::from_millis(duration_ms as u64);

                // Consider watched if view_count > 0 or view_offset is close to duration
                let watched = meta.view_count.unwrap_or(0) > 0
                    || (meta.view_offset.is_some()
                        && duration_ms > 0
                        && meta.view_offset.unwrap_or(0) as f64 / duration_ms as f64 > 0.9);

                // Skip cast/crew during sync - will be loaded lazily when viewing details
                // This avoids storing incomplete data from bulk API responses

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
                    cast: Vec::new(),
                    crew: Vec::new(),
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

    /// Get full metadata for a single movie (complete cast/crew)
    pub async fn get_movie_metadata(&self, rating_key: &str) -> Result<Movie> {
        let url = self.build_url(&format!("/library/metadata/{}", rating_key));

        let response = self
            .client
            .get(&url)
            .headers(self.standard_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to get movie metadata: {}",
                response.status()
            ));
        }

        let plex_response: PlexMoviesResponse = response.json().await?;

        let meta = plex_response
            .media_container
            .metadata
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No metadata found for movie {}", rating_key))?;

        tracing::debug!(
            "Fetched full metadata for movie '{}': {} roles, {} directors, {} writers",
            meta.title,
            meta.roles.len(),
            meta.directors.len(),
            meta.writers.len()
        );

        let duration_ms = meta.duration.unwrap_or(0);
        let duration = Duration::from_millis(duration_ms as u64);

        let watched = meta.view_count.unwrap_or(0) > 0
            || (meta.view_offset.is_some()
                && duration_ms > 0
                && meta.view_offset.unwrap_or(0) as f64 / duration_ms as f64 > 0.9);

        let cast: Vec<Person> = meta
            .roles
            .iter()
            .map(|role| Person {
                id: role
                    .id
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| format!("plex-role-{}", role.tag)),
                name: role.tag.clone(),
                role: role.role.clone(),
                image_url: role.thumb.as_ref().map(|t| self.build_image_url(t)),
            })
            .collect();

        let mut crew: Vec<Person> = Vec::new();
        for director in &meta.directors {
            crew.push(Person {
                id: director
                    .id
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| format!("plex-director-{}", director.tag)),
                name: director.tag.clone(),
                role: Some("Director".to_string()),
                image_url: director.thumb.as_ref().map(|t| self.build_image_url(t)),
            });
        }
        for writer in &meta.writers {
            crew.push(Person {
                id: writer
                    .id
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| format!("plex-writer-{}", writer.tag)),
                name: writer.tag.clone(),
                role: Some("Writer".to_string()),
                image_url: writer.thumb.as_ref().map(|t| self.build_image_url(t)),
            });
        }

        Ok(Movie {
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
            cast,
            crew,
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
            intro_marker: None,
            credits_marker: None,
        })
    }

    /// Get all TV shows from a library
    pub async fn get_shows(&self, library_id: &str) -> Result<Vec<Show>> {
        let url = self.build_url(&format!(
            "/library/sections/{}/all?includeExtras=1&includeRelated=1&includePopularLeaves=1&includeGuids=1",
            library_id
        ));

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

        tracing::info!(
            "Fetched {} shows from Plex library {}",
            plex_response.media_container.metadata.len(),
            library_id
        );

        let mut shows = Vec::new();
        for (idx, meta) in plex_response
            .media_container
            .metadata
            .into_iter()
            .enumerate()
        {
            // Log first show in detail to see what Plex returns
            if idx == 0 {
                tracing::warn!(
                    "SAMPLE SHOW '{}': {} roles, {} directors, {} writers - Role IDs: {:?}",
                    meta.title,
                    meta.roles.len(),
                    meta.directors.len(),
                    meta.writers.len(),
                    meta.roles
                        .iter()
                        .map(|r| (r.id, &r.tag))
                        .collect::<Vec<_>>()
                );
            }

            // Fetch seasons for each show
            let seasons = self.get_seasons(&meta.rating_key).await?;

            // Skip cast during sync - will be loaded lazily when viewing details
            // This avoids storing incomplete data from bulk API responses

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
                cast: Vec::new(),
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

    /// Get full metadata for a single show (complete cast/crew)
    pub async fn get_show_metadata(&self, rating_key: &str) -> Result<Show> {
        let url = self.build_url(&format!("/library/metadata/{}", rating_key));

        let response = self
            .client
            .get(&url)
            .headers(self.standard_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to get show metadata: {}",
                response.status()
            ));
        }

        let plex_response: PlexShowsResponse = response.json().await?;

        let meta = plex_response
            .media_container
            .metadata
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No metadata found for show {}", rating_key))?;

        tracing::debug!(
            "Fetched full metadata for show '{}': {} roles, {} directors, {} writers",
            meta.title,
            meta.roles.len(),
            meta.directors.len(),
            meta.writers.len()
        );

        // Fetch seasons
        let seasons = self.get_seasons(&meta.rating_key).await?;

        let cast: Vec<Person> = meta
            .roles
            .iter()
            .map(|role| Person {
                id: role
                    .id
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| format!("plex-role-{}", role.tag)),
                name: role.tag.clone(),
                role: role.role.clone(),
                image_url: role.thumb.as_ref().map(|t| self.build_image_url(t)),
            })
            .collect();

        Ok(Show {
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
            cast,
            added_at: meta.added_at.and_then(|ts| DateTime::from_timestamp(ts, 0)),
            updated_at: meta
                .updated_at
                .and_then(|ts| DateTime::from_timestamp(ts, 0)),
            watched_episode_count: meta.viewed_leaf_count.unwrap_or(0) as u32,
            total_episode_count: meta.leaf_count.unwrap_or(0) as u32,
            last_watched_at: None,
        })
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
