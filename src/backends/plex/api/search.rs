use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, warn};
use url::Url;

use super::client::create_standard_headers;
use super::types::PlexGenericMetadata;
use crate::models::{Episode, MediaItem, MediaItemId, Movie, Show, ShowId};

// Custom search response container that uses generic metadata
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlexSearchContainer {
    #[serde(rename = "Metadata", default)]
    pub metadata: Vec<PlexGenericMetadata>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct HubSearchResponse {
    pub media_container: SearchMediaContainer,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SearchMediaContainer {
    pub size: Option<i32>,
    #[serde(rename = "Hub", default)]
    pub hubs: Vec<SearchHub>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHub {
    pub hub_identifier: String,
    pub title: String,
    pub size: i32,
    #[serde(rename = "type")]
    pub hub_type: String,
    #[serde(rename = "Metadata", default)]
    pub metadata: Vec<SearchResultItem>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultItem {
    pub rating_key: String,
    pub key: String,
    pub guid: Option<String>,
    pub title: String,
    pub original_title: Option<String>,
    #[serde(rename = "type")]
    pub item_type: String,
    pub summary: Option<String>,
    pub thumb: Option<String>,
    pub art: Option<String>,
    pub year: Option<i32>,
    pub rating: Option<f32>,
    pub content_rating: Option<String>,
    pub duration: Option<i64>,
    pub added_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub reason: Option<String>,
    pub reason_title: Option<String>,
    pub reason_id: Option<String>,

    // Show-specific fields
    pub show_title: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,

    // Additional metadata
    pub genres: Option<Vec<String>>,
    pub roles: Option<Vec<String>>,
    pub directors: Option<Vec<String>>,
}

pub struct PlexSearch {
    base_url: String,
    auth_token: String,
    client: Client,
}

impl PlexSearch {
    pub fn new(base_url: String, auth_token: String, client: Client) -> Self {
        Self {
            base_url,
            auth_token,
            client,
        }
    }

    /// Perform a global search across all libraries using the hubs search endpoint
    pub async fn global_search(&self, query: &str, limit: Option<usize>) -> Result<Vec<MediaItem>> {
        let mut url = Url::parse(&format!("{}/hubs/search", self.base_url))?;
        url.query_pairs_mut().append_pair("query", query);

        if let Some(limit) = limit {
            url.query_pairs_mut()
                .append_pair("limit", &limit.to_string());
        }

        debug!("Performing global search with query: {}", query);

        let response = self
            .client
            .get(url.as_str())
            .headers(create_standard_headers(Some(&self.auth_token)))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Search request failed with status {}: {}",
                status,
                text
            ));
        }

        let search_response: HubSearchResponse = response.json().await?;
        let mut results = Vec::new();

        // Process each hub and convert items to our MediaItem types
        for hub in search_response.media_container.hubs {
            debug!(
                "Processing hub: {} with {} items",
                hub.title,
                hub.metadata.len()
            );

            for item in hub.metadata {
                match item.item_type.as_str() {
                    "movie" => {
                        if let Ok(movie) = self.convert_to_movie(item) {
                            results.push(MediaItem::Movie(movie));
                        }
                    }
                    "show" => {
                        if let Ok(show) = self.convert_to_show(item) {
                            results.push(MediaItem::Show(show));
                        }
                    }
                    "episode" => {
                        if let Ok(episode) = self.convert_to_episode(item) {
                            results.push(MediaItem::Episode(episode));
                        }
                    }
                    _ => {
                        debug!("Skipping unsupported item type: {}", item.item_type);
                    }
                }
            }
        }

        Ok(results)
    }

    /// Search within a specific library section
    pub async fn library_search(
        &self,
        section_id: &str,
        query: &str,
        media_type: Option<&str>,
        sort: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<MediaItem>> {
        let mut url = Url::parse(&format!(
            "{}/library/sections/{}/all",
            self.base_url, section_id
        ))?;

        // Add search parameters
        url.query_pairs_mut().append_pair("title", query); // Plex uses title parameter for text search

        if let Some(media_type) = media_type {
            let type_id = match media_type {
                "movie" => "1",
                "show" => "2",
                "episode" => "4",
                _ => media_type,
            };
            url.query_pairs_mut().append_pair("type", type_id);
        }

        if let Some(sort) = sort {
            url.query_pairs_mut().append_pair("sort", sort);
        }

        if let Some(limit) = limit {
            url.query_pairs_mut()
                .append_pair("limit", &limit.to_string());
        }

        debug!("Searching library {} with query: {}", section_id, query);

        let response = self
            .client
            .get(url.as_str())
            .headers(create_standard_headers(Some(&self.auth_token)))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Library search failed with status {}: {}",
                status,
                text
            ));
        }

        let container: PlexSearchContainer = response.json().await?;
        let mut results = Vec::new();

        {
            let metadata = container.metadata;
            for item in metadata {
                match item.type_.as_deref() {
                    Some("movie") => {
                        if let Ok(movie) = self.convert_metadata_to_movie(item) {
                            results.push(MediaItem::Movie(movie));
                        }
                    }
                    Some("show") => {
                        if let Ok(show) = self.convert_metadata_to_show(item) {
                            results.push(MediaItem::Show(show));
                        }
                    }
                    Some("episode") => {
                        if let Ok(episode) = self.convert_metadata_to_episode(item) {
                            results.push(MediaItem::Episode(episode));
                        }
                    }
                    _ => {
                        debug!("Skipping unsupported type: {:?}", item.type_);
                    }
                }
            }
        }

        Ok(results)
    }

    /// Advanced search with filters
    pub async fn advanced_search(
        &self,
        section_id: Option<&str>,
        params: HashMap<String, String>,
    ) -> Result<Vec<MediaItem>> {
        let base_path = if let Some(id) = section_id {
            format!("{}/library/sections/{}/all", self.base_url, id)
        } else {
            format!("{}/library/all", self.base_url)
        };

        let mut url = Url::parse(&base_path)?;

        // Add all custom parameters
        for (key, value) in params {
            url.query_pairs_mut().append_pair(&key, &value);
        }

        debug!("Advanced search with URL: {}", url.as_str());

        let response = self
            .client
            .get(url.as_str())
            .headers(create_standard_headers(Some(&self.auth_token)))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Advanced search failed with status {}: {}",
                status,
                text
            ));
        }

        let container: PlexSearchContainer = response.json().await?;
        let mut results = Vec::new();

        {
            let metadata = container.metadata;
            for item in metadata {
                match item.type_.as_deref() {
                    Some("movie") => {
                        if let Ok(movie) = self.convert_metadata_to_movie(item) {
                            results.push(MediaItem::Movie(movie));
                        }
                    }
                    Some("show") => {
                        if let Ok(show) = self.convert_metadata_to_show(item) {
                            results.push(MediaItem::Show(show));
                        }
                    }
                    Some("episode") => {
                        if let Ok(episode) = self.convert_metadata_to_episode(item) {
                            results.push(MediaItem::Episode(episode));
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(results)
    }

    // Helper methods to convert search results to our domain models
    fn convert_to_movie(&self, item: SearchResultItem) -> Result<Movie> {
        use std::time::Duration;
        Ok(Movie {
            id: item.rating_key.clone(),
            backend_id: String::new(), // Will be set by backend
            title: item.title,
            year: item.year.map(|y| y as u32),
            duration: item
                .duration
                .map(|d| Duration::from_millis(d as u64))
                .unwrap_or_default(),
            rating: item.rating,
            poster_url: item.thumb,
            backdrop_url: item.art,
            overview: item.summary,
            genres: item.genres.unwrap_or_default(),
            cast: Vec::new(), // Would need separate API call
            crew: Vec::new(), // Would need separate API call
            added_at: item
                .added_at
                .and_then(|t| chrono::DateTime::from_timestamp(t, 0)),
            updated_at: item
                .updated_at
                .and_then(|t| chrono::DateTime::from_timestamp(t, 0)),
            watched: false,
            view_count: 0,
            last_watched_at: None,
            playback_position: None,
            intro_marker: None,
            credits_marker: None,
        })
    }

    fn convert_to_show(&self, item: SearchResultItem) -> Result<Show> {
        Ok(Show {
            id: item.rating_key.clone(),
            backend_id: String::new(), // Will be set by backend
            title: item.title,
            year: item.year.map(|y| y as u32),
            seasons: Vec::new(), // Would need separate API call
            rating: item.rating,
            poster_url: item.thumb,
            backdrop_url: item.art,
            overview: item.summary,
            genres: item.genres.unwrap_or_default(),
            cast: Vec::new(),
            added_at: item
                .added_at
                .and_then(|t| chrono::DateTime::from_timestamp(t, 0)),
            updated_at: item
                .updated_at
                .and_then(|t| chrono::DateTime::from_timestamp(t, 0)),
            watched_episode_count: 0,
            total_episode_count: 0,
            last_watched_at: None,
        })
    }

    fn convert_to_episode(&self, item: SearchResultItem) -> Result<Episode> {
        use std::time::Duration;
        Ok(Episode {
            id: item.rating_key.clone(),
            backend_id: String::new(), // Will be set by backend
            show_id: None,             // Would need to parse from key
            title: item.title,
            season_number: item.season_number.unwrap_or(0) as u32,
            episode_number: item.episode_number.unwrap_or(0) as u32,
            duration: item
                .duration
                .map(|d| Duration::from_millis(d as u64))
                .unwrap_or_default(),
            thumbnail_url: item.thumb,
            overview: item.summary,
            air_date: None,
            watched: false,
            view_count: 0,
            last_watched_at: None,
            playback_position: None,
            show_title: item.show_title,
            show_poster_url: None,
            intro_marker: None,
            credits_marker: None,
        })
    }

    fn convert_metadata_to_movie(&self, item: PlexGenericMetadata) -> Result<Movie> {
        use chrono::Utc;
        use std::time::Duration;
        Ok(Movie {
            id: item.rating_key,
            backend_id: String::new(), // Will be set by backend
            title: item.title,
            year: item.year.map(|y| y as u32),
            duration: item
                .duration
                .map(|d| Duration::from_millis(d as u64))
                .unwrap_or_default(),
            rating: item.rating.map(|r| r as f32),
            poster_url: item
                .thumb
                .map(|t| self.base_url.clone() + &t + "?X-Plex-Token=" + &self.auth_token),
            backdrop_url: item
                .art
                .map(|a| self.base_url.clone() + &a + "?X-Plex-Token=" + &self.auth_token),
            overview: item.summary,
            genres: Vec::new(),
            cast: Vec::new(),
            crew: Vec::new(),
            added_at: item
                .added_at
                .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
            updated_at: item
                .updated_at
                .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
            watched: item.view_count.unwrap_or(0) > 0,
            view_count: item.view_count.unwrap_or(0),
            last_watched_at: item
                .last_viewed_at
                .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
            playback_position: item.view_offset.map(|o| Duration::from_millis(o as u64)),
            intro_marker: None,
            credits_marker: None,
        })
    }

    fn convert_metadata_to_show(&self, item: PlexGenericMetadata) -> Result<Show> {
        Ok(Show {
            id: item.rating_key,
            backend_id: String::new(), // Will be set by backend
            title: item.title,
            year: item.year.map(|y| y as u32),
            seasons: Vec::new(),
            rating: item.rating.map(|r| r as f32),
            poster_url: item
                .thumb
                .map(|t| self.base_url.clone() + &t + "?X-Plex-Token=" + &self.auth_token),
            backdrop_url: item
                .art
                .map(|a| self.base_url.clone() + &a + "?X-Plex-Token=" + &self.auth_token),
            overview: item.summary,
            genres: Vec::new(),
            cast: Vec::new(),
            added_at: item
                .added_at
                .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
            updated_at: item
                .updated_at
                .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
            watched_episode_count: item.viewed_leaf_count.unwrap_or(0) as u32,
            total_episode_count: item.leaf_count.unwrap_or(0) as u32,
            last_watched_at: item
                .last_viewed_at
                .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
        })
    }

    fn convert_metadata_to_episode(&self, item: PlexGenericMetadata) -> Result<Episode> {
        use std::time::Duration;
        Ok(Episode {
            id: item.rating_key,
            backend_id: String::new(), // Will be set by backend
            show_id: Some(item.grandparent_rating_key),
            title: item.title,
            season_number: item.parent_index.unwrap_or(0) as u32,
            episode_number: item.index.unwrap_or(0) as u32,
            duration: item
                .duration
                .map(|d| Duration::from_millis(d as u64))
                .unwrap_or_default(),
            thumbnail_url: item
                .thumb
                .map(|t| self.base_url.clone() + &t + "?X-Plex-Token=" + &self.auth_token),
            overview: item.summary,
            air_date: None,
            watched: item.view_count.unwrap_or(0) > 0,
            view_count: item.view_count.unwrap_or(0),
            last_watched_at: item
                .last_viewed_at
                .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0)),
            playback_position: item.view_offset.map(|o| Duration::from_millis(o as u64)),
            show_title: item.parent_title,
            show_poster_url: None,
            intro_marker: None,
            credits_marker: None,
        })
    }
}
