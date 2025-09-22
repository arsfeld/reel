use anyhow::{Result, anyhow};
use chrono::DateTime;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tracing::{debug, info, warn};

use super::client::PlexApi;
use super::types::*;
use crate::models::{Episode, HomeSection, HomeSectionType, LibraryType, MediaItem, Movie, Show};

impl PlexApi {
    pub async fn get_home_sections(&self) -> Result<Vec<HomeSection>> {
        info!(
            "PlexApi::get_home_sections() - Starting to fetch homepage data from /hubs/home/refresh"
        );

        // Try to use the new /hubs/home/refresh endpoint first
        match self.fetch_home_hubs().await {
            Ok(sections) if !sections.is_empty() => {
                info!(
                    "Successfully fetched {} sections from /hubs/home/refresh",
                    sections.len()
                );
                return Ok(sections);
            }
            Ok(_) => {
                info!(
                    "No sections returned from /hubs/home/refresh, falling back to legacy method"
                );
            }
            Err(e) => {
                warn!(
                    "Failed to fetch from /hubs/home/refresh: {}, falling back to legacy method",
                    e
                );
            }
        }

        // Fallback to the legacy method if the new endpoint fails or returns no data
        self.get_home_sections_legacy().await
    }

    /// Fetch home sections from the /hubs/home/refresh endpoint (Plex's dynamic home hubs)
    async fn fetch_home_hubs(&self) -> Result<Vec<HomeSection>> {
        let url = self.build_url("/hubs/home/refresh");

        debug!("Fetching home hubs from: {}", url);

        let response = self
            .client
            .get(&url)
            .headers(self.standard_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to fetch home hubs: HTTP {}",
                response.status()
            ));
        }

        let plex_response: PlexHubsResponse = response.json().await?;
        let mut sections = Vec::new();

        for hub in plex_response.media_container.hubs {
            if hub.metadata.is_empty() {
                debug!("Skipping empty hub: {}", hub.title);
                continue;
            }

            // Parse media items from hub metadata
            let mut items = Vec::new();
            for meta in &hub.metadata {
                if let Ok(item) = self.parse_media_item(meta.clone()) {
                    items.push(item);
                }
            }

            if items.is_empty() {
                continue;
            }

            // Respect hub size limits if specified
            if let Some(size) = hub.size {
                items.truncate(size as usize);
            }

            // Map hub context/type to HomeSectionType
            let section_type = self.map_hub_to_section_type(&hub);

            // Create the home section
            let section = HomeSection {
                id: hub
                    .hub_identifier
                    .clone()
                    .unwrap_or_else(|| hub.title.clone()),
                title: hub.title.clone(),
                section_type,
                items,
            };

            info!(
                "Added hub section '{}' with {} items (style: {:?}, type: {:?})",
                section.title,
                section.items.len(),
                hub.style,
                hub.hub_type
            );

            sections.push(section);
        }

        Ok(sections)
    }

    /// Map Plex hub metadata to HomeSectionType
    fn map_hub_to_section_type(&self, hub: &PlexHub) -> HomeSectionType {
        // First check the context field for specific hub types
        if let Some(context) = &hub.context {
            let context_lower = context.to_lowercase();
            if context_lower.contains("continue") || context_lower.contains("ondeck") {
                return HomeSectionType::ContinueWatching;
            } else if context_lower.contains("recentlyadded") {
                return HomeSectionType::RecentlyAdded;
            } else if context_lower.contains("recentlyplayed")
                || context_lower.contains("recentlyviewed")
            {
                return HomeSectionType::RecentlyPlayed;
            } else if context_lower.contains("toprated") {
                return HomeSectionType::TopRated;
            } else if context_lower.contains("popular") || context_lower.contains("trending") {
                return HomeSectionType::Trending;
            }
        }

        // Fallback to title-based detection
        let title_lower = hub.title.to_lowercase();
        if title_lower.contains("continue") || title_lower.contains("on deck") {
            HomeSectionType::ContinueWatching
        } else if title_lower.contains("recently added") {
            HomeSectionType::RecentlyAdded
        } else if title_lower.contains("recently played") || title_lower.contains("recently viewed")
        {
            HomeSectionType::RecentlyPlayed
        } else if title_lower.contains("top rated") {
            HomeSectionType::TopRated
        } else if title_lower.contains("popular") || title_lower.contains("trending") {
            HomeSectionType::Trending
        } else {
            // Use Custom type for any unrecognized hub
            HomeSectionType::Custom(hub.title.clone())
        }
    }

    /// Legacy method for getting homepage sections (fallback)
    async fn get_home_sections_legacy(&self) -> Result<Vec<HomeSection>> {
        let mut sections = Vec::new();
        let mut all_item_ids = HashSet::new();

        info!("PlexApi::get_home_sections_legacy() - Using legacy hub fetching method");

        // First, make a single batched API call to get all hub data
        let hubs_data = self.get_all_hubs_batched().await?;

        // Process On Deck section
        if let Some(on_deck) = hubs_data.get("on_deck")
            && !on_deck.is_empty()
        {
            // Collect all item IDs for batch caching
            for item in on_deck {
                match item {
                    MediaItem::Movie(m) => all_item_ids.insert(m.id.clone()),
                    MediaItem::Show(s) => all_item_ids.insert(s.id.clone()),
                    MediaItem::Episode(e) => all_item_ids.insert(e.id.clone()),
                    _ => false, // Ignore other media types for now
                };
            }

            sections.push(HomeSection {
                id: "on_deck".to_string(),
                title: "Continue Watching".to_string(),
                section_type: HomeSectionType::ContinueWatching,
                items: on_deck.clone(),
            });
        }

        // Process Recently Added section
        if let Some(recently_added) = hubs_data.get("recently_added")
            && !recently_added.is_empty()
        {
            // Collect all item IDs for batch caching
            for item in recently_added {
                match item {
                    MediaItem::Movie(m) => all_item_ids.insert(m.id.clone()),
                    MediaItem::Show(s) => all_item_ids.insert(s.id.clone()),
                    MediaItem::Episode(e) => all_item_ids.insert(e.id.clone()),
                    _ => false, // Ignore other media types for now
                };
            }

            sections.push(HomeSection {
                id: "recently_added".to_string(),
                title: "Recently Added".to_string(),
                section_type: HomeSectionType::RecentlyAdded,
                items: recently_added.clone(),
            });
        }

        // Process library-specific hubs
        for (hub_id, items) in hubs_data.iter() {
            if hub_id != "on_deck" && hub_id != "recently_added" && !items.is_empty() {
                // Parse hub_id to get library name and hub title
                if let Some((library_name, hub_title)) = hub_id.split_once("::") {
                    // Collect all item IDs for batch caching
                    for item in items {
                        match item {
                            MediaItem::Movie(m) => all_item_ids.insert(m.id.clone()),
                            MediaItem::Show(s) => all_item_ids.insert(s.id.clone()),
                            MediaItem::Episode(e) => all_item_ids.insert(e.id.clone()),
                            _ => false, // Ignore other media types for now
                        };
                    }

                    let section_type = match hub_title {
                        "Top Rated" => HomeSectionType::TopRated,
                        "Popular" | "Trending" => HomeSectionType::Trending,
                        "Recently Played" | "Recently Viewed" => HomeSectionType::RecentlyPlayed,
                        _ => HomeSectionType::Custom(hub_title.to_string()),
                    };

                    sections.push(HomeSection {
                        id: hub_id.clone(),
                        title: format!("{} - {}", library_name, hub_title),
                        section_type,
                        items: items.clone(),
                    });
                }
            }
        }

        // Note: Caching is handled by SyncManager, not by the backend

        info!(
            "PlexApi::get_home_sections() - Total sections: {}",
            sections.len()
        );
        for section in &sections {
            info!(
                "  Section '{}' has {} items",
                section.title,
                section.items.len()
            );
        }

        Ok(sections)
    }

    /// Get all hub data in a single batched call
    async fn get_all_hubs_batched(&self) -> Result<HashMap<String, Vec<MediaItem>>> {
        let mut hubs_data = HashMap::new();

        // Make a single API call to get all hub endpoints
        let hub_endpoints = vec![
            ("/library/onDeck", "on_deck"),
            ("/library/recentlyAdded", "recently_added"),
        ];

        // Fetch global hubs concurrently
        let mut tasks = Vec::new();
        for (endpoint, hub_id) in hub_endpoints {
            let url = self.build_url(endpoint);
            let client = self.client.clone();
            let headers = self.standard_headers();
            let hub_id = hub_id.to_string();

            tasks.push(tokio::spawn(async move {
                let response = client.get(&url).headers(headers).send().await;
                (hub_id, response)
            }));
        }

        // Also fetch library hubs concurrently
        let libraries = self.get_libraries().await?;
        for library in libraries
            .iter()
            .filter(|l| matches!(l.library_type, LibraryType::Movies | LibraryType::Shows))
        {
            let url = self.build_url(&format!("/hubs/sections/{}", library.id));
            let client = self.client.clone();
            let headers = self.standard_headers();
            let library_title = library.title.clone();

            tasks.push(tokio::spawn(async move {
                let response = client.get(&url).headers(headers).send().await;
                (library_title, response)
            }));
        }

        // Process all responses
        for task in tasks {
            let (hub_id, response_result) = task.await?;

            match response_result {
                Ok(response) if response.status().is_success() => {
                    // Handle global hubs (onDeck, recentlyAdded)
                    if hub_id == "on_deck" || hub_id == "recently_added" {
                        if let Ok(plex_response) = response.json::<PlexOnDeckResponse>().await {
                            let mut items = Vec::new();
                            for meta in plex_response.media_container.metadata {
                                if let Ok(item) = self.parse_media_item(meta) {
                                    items.push(item);
                                }
                            }
                            hubs_data.insert(hub_id, items);
                        }
                    } else {
                        // Handle library-specific hubs
                        if let Ok(plex_response) = response.json::<PlexHubsResponse>().await {
                            for hub in plex_response.media_container.hubs {
                                let hub_key = format!("{}::{}", hub_id, hub.title);
                                let mut items = Vec::new();
                                for meta in hub.metadata {
                                    if let Ok(item) = self.parse_media_item(meta) {
                                        items.push(item);
                                    }
                                }
                                if !items.is_empty() {
                                    hubs_data.insert(hub_key, items);
                                }
                            }
                        }
                    }
                }
                _ => {
                    debug!("Failed to fetch hub: {}", hub_id);
                }
            }
        }

        info!(
            "PlexApi::get_all_hubs_batched() - Retrieved {} hub sections",
            hubs_data.len()
        );

        Ok(hubs_data)
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
