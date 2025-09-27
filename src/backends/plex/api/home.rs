use anyhow::{Result, anyhow};
use chrono::DateTime;
use std::time::Duration;
use tracing::{debug, info};

use super::client::PlexApi;
use super::types::*;
use crate::models::{Episode, HomeSection, HomeSectionType, MediaItem, Movie, Show};

impl PlexApi {
    /// Get home sections (hubs) from Plex server
    ///
    /// This uses the Plex `/hubs` endpoint to fetch the actual
    /// home page sections as configured by the Plex server, including:
    /// - Continue Watching (home.continue)
    /// - On Deck (home.ondeck)
    /// - Recently Added (library.recentlyAdded.X)
    /// - Library-specific sections
    pub async fn get_home_sections(&self) -> Result<Vec<HomeSection>> {
        info!("PlexApi::get_home_sections() - Fetching homepage data from /hubs");

        // Use the standard hubs endpoint which works on all Plex servers
        self.get_home_sections_hubs().await
    }

    /// Get homepage sections using the /hubs endpoint
    async fn get_home_sections_hubs(&self) -> Result<Vec<HomeSection>> {
        let mut sections = Vec::new();

        // Fetch the home hubs from the standard endpoint
        let url = self.build_url("/hubs");
        let response = self
            .client
            .get(&url)
            .headers(self.standard_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to fetch home hubs: {}", response.status()));
        }

        let hub_response: PlexHubsResponse = response.json().await?;

        // Process each hub
        for hub in hub_response.media_container.hubs {
            if hub.metadata.is_empty() {
                continue;
            }

            // Map hub identifier to section type based on actual /hubs endpoint identifiers
            let section_type = match hub.hub_identifier.as_deref() {
                Some("home.continue") => HomeSectionType::ContinueWatching,
                Some("home.ondeck") => HomeSectionType::OnDeck,
                Some("home.playlists") => HomeSectionType::RecentPlaylists,
                Some(id) => {
                    // Extract media type from patterns like "home.movies.recent" or "movie.recentlyadded.1"
                    if id.starts_with("home.") && id.ends_with(".recent") {
                        // Pattern: home.{media_type}.recent
                        let parts: Vec<&str> = id.split('.').collect();
                        if parts.len() >= 3 {
                            let media_type = parts[1].to_string();
                            HomeSectionType::RecentlyAdded(media_type)
                        } else {
                            HomeSectionType::RecentlyAdded("unknown".to_string())
                        }
                    } else if id.contains(".recentlyadded.") {
                        // Pattern: {media_type}.recentlyadded.{id}
                        let parts: Vec<&str> = id.split('.').collect();
                        if !parts.is_empty() {
                            let media_type = parts[0].to_string();
                            HomeSectionType::RecentlyAdded(media_type)
                        } else {
                            HomeSectionType::RecentlyAdded("unknown".to_string())
                        }
                    } else if id.starts_with("library.recentlyAdded.") {
                        // Pattern: library.recentlyAdded.{library_id}
                        HomeSectionType::RecentlyAdded("library".to_string())
                    } else if id.contains("recentlyViewed") || id.contains("recentlyPlayed") {
                        HomeSectionType::RecentlyPlayed
                    } else if id.contains("topRated") {
                        HomeSectionType::TopRated
                    } else if id.contains("popular") || id.contains("trending") {
                        HomeSectionType::Trending
                    } else if id.contains("recent") {
                        // Generic recent without clear media type
                        HomeSectionType::RecentlyAdded("mixed".to_string())
                    } else {
                        HomeSectionType::Custom(hub.title.clone())
                    }
                }
                _ => HomeSectionType::Custom(hub.title.clone()),
            };

            // Parse media items
            let mut items = Vec::new();
            for meta in hub.metadata {
                if let Ok(item) = self.parse_media_item(meta) {
                    items.push(item);
                }
            }

            sections.push(HomeSection {
                id: hub.hub_identifier.unwrap_or_else(|| hub.key.clone()),
                title: hub.title,
                section_type,
                items,
            });
        }

        info!(
            "PlexApi::get_home_sections_hubs() - Retrieved {} sections from /hubs",
            sections.len()
        );
        for section in &sections {
            info!(
                "  Section '{}' ({}): {} items",
                section.title,
                section.id,
                section.items.len()
            );
        }

        Ok(sections)
    }

    fn parse_media_item(&self, meta: PlexGenericMetadata) -> Result<MediaItem> {
        match meta.type_.as_deref() {
            Some("movie") => {
                let duration_ms = meta.duration.unwrap_or(0);
                let duration = Duration::from_millis(duration_ms as u64);

                let watched = meta.view_count.unwrap_or(0) > 0
                    || (meta.view_offset.unwrap_or(0) as f64 / duration_ms.max(1) as f64) > 0.9;

                let poster_url = meta.thumb.map(|t| self.build_image_url(&t));
                let backdrop_url = meta.art.map(|a| self.build_image_url(&a));

                let movie = Movie {
                    id: meta.rating_key,
                    title: meta.title,
                    year: meta.year.map(|y| y as u32),
                    duration,
                    rating: meta.rating.map(|r| (r / 10.0) as f32),
                    poster_url,
                    backdrop_url,
                    overview: meta.summary,
                    genres: meta
                        .genre
                        .unwrap_or_default()
                        .into_iter()
                        .map(|g| g.tag)
                        .collect(),
                    cast: Vec::new(),
                    crew: Vec::new(),
                    added_at: meta
                        .added_at
                        .map(|ts| DateTime::from_timestamp(ts, 0).unwrap()),
                    updated_at: meta
                        .updated_at
                        .map(|ts| DateTime::from_timestamp(ts, 0).unwrap()),
                    watched,
                    view_count: meta.view_count.unwrap_or(0),
                    last_watched_at: meta
                        .last_viewed_at
                        .map(|ts| DateTime::from_timestamp(ts, 0).unwrap()),
                    playback_position: meta.view_offset.map(|v| Duration::from_millis(v as u64)),
                    intro_marker: None,   // Will be fetched when playing
                    credits_marker: None, // Will be fetched when playing
                    backend_id: self.backend_id.clone(),
                };
                Ok(MediaItem::Movie(movie))
            }
            Some("show") => {
                let poster_url = meta.thumb.map(|t| self.build_image_url(&t));
                let backdrop_url = meta.art.map(|a| self.build_image_url(&a));

                let show = Show {
                    id: meta.rating_key,
                    backend_id: self.backend_id.clone(),
                    title: meta.title,
                    year: meta.year.map(|y| y as u32),
                    seasons: Vec::new(),
                    rating: meta.rating.map(|r| (r / 10.0) as f32),
                    poster_url,
                    backdrop_url,
                    overview: meta.summary,
                    genres: meta
                        .genre
                        .unwrap_or_default()
                        .into_iter()
                        .map(|g| g.tag)
                        .collect(),
                    cast: Vec::new(),
                    added_at: meta
                        .added_at
                        .map(|ts| DateTime::from_timestamp(ts, 0).unwrap()),
                    updated_at: meta
                        .updated_at
                        .map(|ts| DateTime::from_timestamp(ts, 0).unwrap()),
                    watched_episode_count: meta.viewed_leaf_count.unwrap_or(0) as u32,
                    total_episode_count: meta.leaf_count.unwrap_or(0) as u32,
                    last_watched_at: meta
                        .last_viewed_at
                        .map(|ts| DateTime::from_timestamp(ts, 0).unwrap()),
                };
                Ok(MediaItem::Show(show))
            }
            Some("episode") => {
                let duration_ms = meta.duration.unwrap_or(0);
                let duration = Duration::from_millis(duration_ms as u64);

                let watched = meta.view_count.unwrap_or(0) > 0
                    || (meta.view_offset.unwrap_or(0) as f64 / duration_ms.max(1) as f64) > 0.9;

                let episode = Episode {
                    intro_marker: None,
                    credits_marker: None,
                    id: meta.rating_key,
                    backend_id: self.backend_id.clone(),
                    show_id: Some(meta.grandparent_rating_key),
                    title: meta.title,
                    season_number: meta.parent_index.unwrap_or(0) as u32,
                    episode_number: meta.index.unwrap_or(0) as u32,
                    duration,
                    thumbnail_url: meta.thumb.map(|t| self.build_image_url(&t)),
                    overview: meta.summary,
                    air_date: None,
                    watched,
                    view_count: meta.view_count.unwrap_or(0),
                    last_watched_at: meta
                        .last_viewed_at
                        .map(|ts| DateTime::from_timestamp(ts, 0).unwrap()),
                    playback_position: meta.view_offset.map(|v| Duration::from_millis(v as u64)),
                    show_title: meta.grandparent_title,
                    show_poster_url: meta
                        .grandparent_thumb
                        .as_ref()
                        .map(|t| self.build_image_url(t)),
                };
                Ok(MediaItem::Episode(episode))
            }
            Some("season") => {
                // Seasons in recently added should be treated as shows
                // We'll create a placeholder show entry for the season
                debug!("  Season detected, skipping for now");
                Err(anyhow!("Skipping season type"))
            }
            Some("album") | Some("track") | Some("artist") => {
                // Skip music items for now
                debug!("  Music item detected, skipping");
                Err(anyhow!("Skipping music type"))
            }
            _ => {
                debug!("  Unknown type: {:?}", meta.type_);
                Err(anyhow!("Unknown media type: {:?}", meta.type_))
            }
        }
    }
}
