use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::models::{
    ChapterMarker, ChapterType, Episode, HomeSection, HomeSectionType, Library, LibraryType,
    MediaItem, Movie, QualityOption, Resolution, Season, Show, StreamInfo,
};

const PLEX_HEADERS: &[(&str, &str)] = &[
    ("X-Plex-Product", "Reel"),
    ("X-Plex-Version", "0.1.0"),
    ("X-Plex-Client-Identifier", "reel-media-player"),
    ("X-Plex-Platform", "Linux"),
    ("Accept", "application/json"),
];

#[derive(Clone)]
pub struct PlexApi {
    client: reqwest::Client,
    base_url: String,
    auth_token: String,
    backend_id: String,
}

impl PlexApi {
    pub fn new(base_url: String, auth_token: String) -> Self {
        Self::with_backend_id(base_url, auth_token, "plex".to_string())
    }

    pub fn with_backend_id(base_url: String, auth_token: String, backend_id: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url,
            auth_token,
            backend_id,
        }
    }

    /// Get all libraries from the Plex server
    pub async fn get_libraries(&self) -> Result<Vec<Library>> {
        let url = format!("{}/library/sections", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("Accept", "application/json")
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

        let response = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("Accept", "application/json")
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
                let duration = Duration::from_millis(duration_ms);

                // Consider watched if view_count > 0 or view_offset is close to duration
                let watched = meta.view_count.unwrap_or(0) > 0
                    || (meta.view_offset.is_some()
                        && duration_ms > 0
                        && meta.view_offset.unwrap_or(0) as f64 / duration_ms as f64 > 0.9);

                Movie {
                    id: meta.rating_key,
                    backend_id: self.backend_id.clone(),
                    title: meta.title,
                    year: meta.year,
                    duration,
                    rating: meta.rating,
                    poster_url: meta.thumb.map(|t| self.build_image_url(&t)),
                    backdrop_url: meta.art.map(|a| self.build_image_url(&a)),
                    overview: meta.summary,
                    genres: meta
                        .genre
                        .unwrap_or_default()
                        .into_iter()
                        .map(|g| g.tag)
                        .collect(),
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
                    playback_position: meta.view_offset.map(Duration::from_millis),
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
        let url = format!("{}/library/sections/{}/all", self.base_url, library_id);

        let response = self
            .client
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
                backend_id: self.backend_id.clone(),
                title: meta.title,
                year: meta.year,
                seasons,
                rating: meta.rating,
                poster_url: meta.thumb.map(|t| self.build_image_url(&t)),
                backdrop_url: meta.art.map(|a| self.build_image_url(&a)),
                overview: meta.summary,
                genres: meta
                    .genre
                    .unwrap_or_default()
                    .into_iter()
                    .map(|g| g.tag)
                    .collect(),
                cast: vec![], // TODO: Fetch cast details
                added_at: meta.added_at.and_then(|ts| DateTime::from_timestamp(ts, 0)),
                updated_at: meta
                    .updated_at
                    .and_then(|ts| DateTime::from_timestamp(ts, 0)),
                watched_episode_count: meta.viewed_leaf_count.unwrap_or(0),
                total_episode_count: meta.leaf_count.unwrap_or(0),
                last_watched_at: None, // TODO: Fetch from episodes
            });
        }

        info!("Found {} shows in library {}", shows.len(), library_id);
        Ok(shows)
    }

    /// Get seasons for a TV show
    pub async fn get_seasons(&self, show_id: &str) -> Result<Vec<Season>> {
        let url = format!("{}/library/metadata/{}/children", self.base_url, show_id);

        let response = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("Accept", "application/json")
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

        let response = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("Accept", "application/json")
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
                let duration = Duration::from_millis(duration_ms);

                // Consider watched if view_count > 0 or view_offset is close to duration
                let watched = meta.view_count.unwrap_or(0) > 0
                    || (meta.view_offset.is_some()
                        && duration_ms > 0
                        && meta.view_offset.unwrap_or(0) as f64 / duration_ms as f64 > 0.9);

                // Note: We can't fetch markers here in a sync context
                // They would need to be fetched separately after episodes are loaded
                let intro_marker = None;
                let credits_marker = None;

                // TODO: markers will need separate fetch (can't await in map)
                Episode {
                    id: meta.rating_key,
                    backend_id: self.backend_id.clone(),
                    show_id: meta.grandparent_rating_key,
                    title: meta.title,
                    season_number: meta.parent_index.unwrap_or(0),
                    episode_number: meta.index.unwrap_or(0),
                    duration,
                    thumbnail_url: meta.thumb.map(|t| self.build_image_url(&t)),
                    overview: meta.summary,
                    air_date: meta
                        .originally_available_at
                        .and_then(|date| DateTime::parse_from_rfc3339(&date).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    watched,
                    view_count: meta.view_count.unwrap_or(0),
                    last_watched_at: meta
                        .last_viewed_at
                        .and_then(|ts| DateTime::from_timestamp(ts, 0)),
                    playback_position: meta.view_offset.map(Duration::from_millis),
                    show_title: meta.grandparent_title,
                    show_poster_url: meta
                        .grandparent_thumb
                        .as_ref()
                        .map(|t| self.build_image_url(t)),
                    intro_marker,
                    credits_marker,
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

        let response = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get media info: {}", response.status()));
        }

        let plex_response: PlexMediaResponse = response.json().await?;

        if let Some(metadata) = plex_response.media_container.metadata.first()
            && let Some(media) = metadata.media.as_ref().and_then(|m| m.first())
            && let Some(part) = media.part.as_ref().and_then(|p| p.first())
        {
            let stream_url = format!(
                "{}{}?X-Plex-Token={}",
                self.base_url, part.key, self.auth_token
            );

            // Generate quality options for transcoding
            let mut quality_options = Vec::new();

            // Add original quality (direct play)
            let original_bitrate = media.bitrate.unwrap_or(0);
            let original_width = media.width.unwrap_or(1920);
            let original_height = media.height.unwrap_or(1080);

            quality_options.push(QualityOption {
                name: format!("Original ({}p)", original_height),
                resolution: Resolution {
                    width: original_width,
                    height: original_height,
                },
                bitrate: original_bitrate,
                url: stream_url.clone(),
                requires_transcode: false,
            });

            // Add transcoding options
            let transcode_qualities = vec![
                ("1080p", 1920, 1080, 8000000),
                ("720p", 1280, 720, 4000000),
                ("480p", 854, 480, 2000000),
                ("360p", 640, 360, 1000000),
            ];

            for (name, width, height, bitrate) in transcode_qualities {
                // Only add qualities lower than original
                if height < original_height {
                    let path = format!("/library/metadata/{}", media_id);
                    let transcode_url = format!(
                        "{}/video/:/transcode/universal/start.m3u8?path={}&mediaIndex=0&partIndex=0&protocol=hls&directPlay=0&directStream=0&fastSeek=1&maxVideoBitrate={}&videoResolution={}x{}&X-Plex-Token={}",
                        self.base_url,
                        path.replace("/", "%2F"),
                        bitrate / 1000, // Convert to kbps
                        width,
                        height,
                        self.auth_token
                    );

                    quality_options.push(QualityOption {
                        name: name.to_string(),
                        resolution: Resolution { width, height },
                        bitrate: bitrate as u64,
                        url: transcode_url,
                        requires_transcode: true,
                    });
                }
            }

            return Ok(StreamInfo {
                url: stream_url,
                direct_play: true,
                video_codec: media.video_codec.clone().unwrap_or_default(),
                audio_codec: media.audio_codec.clone().unwrap_or_default(),
                container: part.container.clone().unwrap_or_default(),
                bitrate: original_bitrate,
                resolution: Resolution {
                    width: original_width,
                    height: original_height,
                },
                quality_options,
            });
        }

        Err(anyhow!("Failed to get stream info for media"))
    }

    /// Update playback progress
    /// Note: state should be "playing" for active playback or "paused" when paused
    pub async fn update_progress(
        &self,
        media_id: &str,
        position: Duration,
        duration: Duration,
    ) -> Result<()> {
        self.update_progress_with_state(media_id, position, duration, "playing")
            .await
    }

    /// Update playback progress with explicit state
    pub async fn update_progress_with_state(
        &self,
        media_id: &str,
        position: Duration,
        duration: Duration,
        state: &str,
    ) -> Result<()> {
        // For simple progress tracking without a playQueue, we can update the viewOffset directly
        // by "scrobbling" with the current position
        let position_ms = position.as_millis() as u64;

        // If we're more than 90% through, mark as watched
        let duration_ms = duration.as_millis() as u64;
        if duration_ms > 0 && position_ms > (duration_ms * 9 / 10) {
            debug!("Position is >90% of duration, marking as watched");
            return self.mark_watched(media_id).await;
        }

        // Otherwise update the viewOffset using timeline endpoint with proper headers
        // The timeline endpoint is more reliable for position updates
        let timeline_url = format!("{}/:/timeline", self.base_url);

        debug!(
            "Updating progress via timeline - media_id: {}, position: {}ms",
            media_id, position_ms
        );

        let response = self
            .client
            .get(&timeline_url)
            .header("X-Plex-Token", &self.auth_token)
            .header("X-Plex-Client-Identifier", "reel")
            .header("X-Plex-Product", "Reel")
            .header("X-Plex-Version", "0.1.0")
            .header("X-Plex-Platform", "Linux")
            .query(&[
                ("ratingKey", media_id),
                ("key", &format!("/library/metadata/{}", media_id)),
                ("identifier", "com.plexapp.plugins.library"),
                ("state", state),
                ("time", &position_ms.to_string()),
                ("duration", &duration_ms.to_string()),
                ("playbackTime", &position_ms.to_string()),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            debug!("Timeline update response: {} - {}", status, text);
            // Timeline endpoint often returns 200 with empty response, which is OK
            if status != 200 {
                return Err(anyhow!("Failed to update progress: {}", status));
            }
        } else {
            debug!("Timeline update successful for media_id: {}", media_id);
        }

        Ok(())
    }

    /// Mark media as watched
    pub async fn mark_watched(&self, media_id: &str) -> Result<()> {
        let url = format!("{}/:/scrobble", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .query(&[
                ("key", media_id),
                ("identifier", "com.plexapp.plugins.library"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to mark as watched: {}", response.status()));
        }

        Ok(())
    }

    /// Mark media as unwatched
    pub async fn mark_unwatched(&self, media_id: &str) -> Result<()> {
        let url = format!("{}/:/unscrobble", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .query(&[
                ("key", media_id),
                ("identifier", "com.plexapp.plugins.library"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to mark as unwatched: {}",
                response.status()
            ));
        }

        Ok(())
    }

    /// Get homepage sections (On Deck, Recently Added, etc.)
    pub async fn get_home_sections(&self) -> Result<Vec<HomeSection>> {
        let mut sections = Vec::new();
        let mut all_item_ids = HashSet::new();

        info!("PlexApi::get_home_sections() - Starting to fetch homepage data");

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
            let url = format!("{}{}", self.base_url, endpoint);
            let client = self.client.clone();
            let token = self.auth_token.clone();
            let hub_id = hub_id.to_string();

            tasks.push(tokio::spawn(async move {
                let response = client
                    .get(&url)
                    .header("X-Plex-Token", &token)
                    .header("Accept", "application/json")
                    .send()
                    .await;
                (hub_id, response)
            }));
        }

        // Also fetch library hubs concurrently
        let libraries = self.get_libraries().await?;
        for library in libraries
            .iter()
            .filter(|l| matches!(l.library_type, LibraryType::Movies | LibraryType::Shows))
        {
            let url = format!("{}/hubs/sections/{}", self.base_url, library.id);
            let client = self.client.clone();
            let token = self.auth_token.clone();
            let library_title = library.title.clone();

            tasks.push(tokio::spawn(async move {
                let response = client
                    .get(&url)
                    .header("X-Plex-Token", &token)
                    .header("Accept", "application/json")
                    .send()
                    .await;
                (library_title, response)
            }));
        }

        // Process all responses
        for task in tasks {
            match task.await {
                Ok((hub_id, Ok(response))) if response.status().is_success() => {
                    if hub_id == "on_deck" || hub_id == "recently_added" {
                        // Process global hubs
                        if let Ok(text) = response.text().await
                            && let Ok(plex_response) =
                                serde_json::from_str::<PlexOnDeckResponse>(&text)
                        {
                            let mut items = Vec::new();
                            for meta in plex_response.media_container.metadata {
                                if let Ok(item) = self.parse_media_item(meta) {
                                    items.push(item);
                                }
                            }
                            if !items.is_empty() {
                                hubs_data.insert(hub_id, items);
                            }
                        }
                    } else {
                        // Process library-specific hubs
                        if let Ok(text) = response.text().await
                            && let Ok(plex_response) =
                                serde_json::from_str::<PlexHubsResponse>(&text)
                        {
                            for hub in plex_response.media_container.hub {
                                if hub.metadata.is_empty() {
                                    continue;
                                }

                                let mut items = Vec::new();
                                for meta in hub.metadata {
                                    if let Ok(item) = self.parse_media_item(meta) {
                                        items.push(item);
                                    }
                                }

                                if !items.is_empty() {
                                    let title_lower = hub.title.to_lowercase();
                                    if !title_lower.contains("recently")
                                        && !title_lower.contains("on deck")
                                        && !title_lower.contains("continue")
                                    {
                                        let full_hub_id = format!("{}::{}", hub_id, hub.title);
                                        hubs_data.insert(full_hub_id, items);
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {
                    // Ignore failures for individual hubs
                }
            }
        }

        info!(
            "PlexApi::get_all_hubs_batched() - Retrieved {} hub sections",
            hubs_data.len()
        );
        Ok(hubs_data)
    }

    /// Get On Deck items (partially watched content)
    async fn get_on_deck(&self) -> Result<Vec<MediaItem>> {
        let url = format!("{}/library/onDeck", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get on deck: {}", response.status()));
        }

        let plex_response: PlexOnDeckResponse = response.json().await?;
        let mut items = Vec::new();

        for meta in plex_response.media_container.metadata {
            // Only add items we can successfully parse
            match self.parse_media_item(meta) {
                Ok(item) => items.push(item),
                Err(e) => {
                    debug!("Skipping item in on deck: {}", e);
                }
            }
        }

        info!(
            "PlexApi::get_on_deck() - Successfully parsed {} items",
            items.len()
        );
        Ok(items)
    }

    /// Get recently added items
    async fn get_recently_added(&self) -> Result<Vec<MediaItem>> {
        let url = format!("{}/library/recentlyAdded", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("Accept", "application/json")
            .query(&[("limit", "30")]) // Get more items since we'll filter some out
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to get recently added: {}",
                response.status()
            ));
        }

        let plex_response: PlexRecentlyAddedResponse = response.json().await?;
        let mut items = Vec::new();

        for meta in plex_response.media_container.metadata {
            // Only add items we can successfully parse
            match self.parse_media_item(meta) {
                Ok(item) => items.push(item),
                Err(e) => {
                    debug!("Skipping item in recently added: {}", e);
                }
            }
        }

        info!(
            "PlexApi::get_recently_added() - Successfully parsed {} items",
            items.len()
        );
        Ok(items)
    }

    /// Get hub sections for a specific library
    async fn get_library_hubs(&self, library_id: &str) -> Result<Vec<HomeSection>> {
        let url = format!("{}/hubs/sections/{}", self.base_url, library_id);

        let response = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            // Hubs might not be available for all libraries
            return Ok(Vec::new());
        }

        let plex_response: PlexHubsResponse = response.json().await?;
        let mut sections = Vec::new();

        for hub in plex_response.media_container.hub {
            if hub.metadata.is_empty() {
                continue;
            }

            let section_type = match hub.context.as_deref() {
                Some("hub.movie.recentlyadded") | Some("hub.show.recentlyadded") => {
                    HomeSectionType::RecentlyAdded
                }
                Some("hub.movie.toprated") | Some("hub.show.toprated") => HomeSectionType::TopRated,
                Some("hub.movie.popular") | Some("hub.show.popular") => HomeSectionType::Trending,
                Some("hub.movie.recentlyviewed") | Some("hub.show.recentlyviewed") => {
                    HomeSectionType::RecentlyPlayed
                }
                _ => HomeSectionType::Custom(hub.title.clone()),
            };

            let mut items = Vec::new();
            for meta in hub.metadata {
                match self.parse_media_item(meta) {
                    Ok(item) => items.push(item),
                    Err(e) => {
                        debug!("Skipping item in hub '{}': {}", hub.title, e);
                    }
                }
            }

            // Only add the section if it has items
            if !items.is_empty() {
                debug!(
                    "PlexApi::get_library_hubs() - Hub '{}' has {} items",
                    hub.title,
                    items.len()
                );
                sections.push(HomeSection {
                    id: format!(
                        "{}_{}",
                        library_id,
                        hub.key.unwrap_or_else(|| hub.title.clone())
                    ),
                    title: hub.title,
                    section_type,
                    items,
                });
            }
        }

        Ok(sections)
    }

    /// Parse a generic Plex metadata item into a MediaItem
    fn parse_media_item(&self, meta: PlexGenericMetadata) -> Result<MediaItem> {
        match meta.type_.as_deref() {
            Some("movie") => {
                let duration_ms = meta.duration.unwrap_or(0);
                let duration = Duration::from_millis(duration_ms);

                let watched = meta.view_count.unwrap_or(0) > 0
                    || (meta.view_offset.unwrap_or(0) as f64 / duration_ms.max(1) as f64) > 0.9;

                let poster_url = meta.thumb.map(|t| self.build_image_url(&t));
                let backdrop_url = meta.art.map(|a| self.build_image_url(&a));

                let movie = Movie {
                    id: meta.rating_key,
                    title: meta.title,
                    year: meta.year,
                    duration,
                    rating: meta.rating.map(|r| r / 10.0),
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
                    playback_position: meta.view_offset.map(Duration::from_millis),
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
                    year: meta.year,
                    seasons: Vec::new(),
                    rating: meta.rating.map(|r| r / 10.0),
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
                    watched_episode_count: meta.viewed_leaf_count.unwrap_or(0),
                    total_episode_count: meta.leaf_count.unwrap_or(0),
                    last_watched_at: meta
                        .last_viewed_at
                        .map(|ts| DateTime::from_timestamp(ts, 0).unwrap()),
                };
                Ok(MediaItem::Show(show))
            }
            Some("episode") => {
                let duration_ms = meta.duration.unwrap_or(0);
                let duration = Duration::from_millis(duration_ms);

                let watched = meta.view_count.unwrap_or(0) > 0
                    || (meta.view_offset.unwrap_or(0) as f64 / duration_ms.max(1) as f64) > 0.9;

                let episode = Episode {
                    intro_marker: None,
                    credits_marker: None,
                    id: meta.rating_key,
                    backend_id: self.backend_id.clone(),
                    show_id: meta.grandparent_rating_key,
                    title: meta.title,
                    season_number: meta.parent_index.unwrap_or(0),
                    episode_number: meta.index.unwrap_or(0),
                    duration,
                    thumbnail_url: meta.thumb.map(|t| self.build_image_url(&t)),
                    overview: meta.summary,
                    air_date: None,
                    watched,
                    view_count: meta.view_count.unwrap_or(0),
                    last_watched_at: meta
                        .last_viewed_at
                        .map(|ts| DateTime::from_timestamp(ts, 0).unwrap()),
                    playback_position: meta.view_offset.map(Duration::from_millis),
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

    /// Build full image URL from Plex path
    fn build_image_url(&self, path: &str) -> String {
        if path.starts_with("http") {
            path.to_string()
        } else {
            // Use Plex transcoding endpoint for server-side image resizing
            // This dramatically reduces bandwidth and client-side processing
            let encoded_url = utf8_percent_encode(path, NON_ALPHANUMERIC).to_string();
            format!(
                "{}/photo/:/transcode?width=320&height=480&minSize=1&upscale=1&url={}&X-Plex-Token={}",
                self.base_url, encoded_url, self.auth_token
            )
        }
    }

    /// Fetch intro and credit markers for any media (episode or movie)
    pub async fn fetch_episode_markers(
        &self,
        rating_key: &str,
    ) -> Result<(Option<ChapterMarker>, Option<ChapterMarker>)> {
        // Include additional parameters to ensure markers are returned
        // includeChapters=1 ensures chapter/marker data is included
        info!("Fetching markers for media ID: {}", rating_key);
        let url = format!(
            "{}/library/metadata/{}?includeChapters=1&includeMarkers=1&includeOnDeck=1&includeRelated=1&includeExtras=1&includeGeolocation=1&X-Plex-Token={}",
            self.base_url, rating_key, self.auth_token
        );

        let response = self
            .client
            .get(&url)
            .header("X-Plex-Token", &self.auth_token)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            warn!(
                "Failed to fetch markers for episode {}: {}",
                rating_key,
                response.status()
            );
            return Ok((None, None));
        }

        let response_text = response.text().await?;

        // Try to parse the response
        let data: PlexMetadataResponse = match serde_json::from_str(&response_text) {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to parse Plex metadata response: {}", e);
                return Ok((None, None));
            }
        };

        let mut intro_marker = None;
        let mut credits_marker = None;

        if let Some(metadata) = data.media_container.metadata.first() {
            if let Some(markers) = &metadata.marker {
                info!(
                    "Found {} markers for media ID: {}",
                    markers.len(),
                    rating_key
                );
                for marker in markers.iter() {
                    info!(
                        "Marker type: '{}', start: {}ms, end: {}ms",
                        marker.type_, marker.start_time_offset, marker.end_time_offset
                    );
                    match marker.type_.as_str() {
                        "intro" => {
                            intro_marker = Some(ChapterMarker {
                                start_time: Duration::from_millis(marker.start_time_offset),
                                end_time: Duration::from_millis(marker.end_time_offset),
                                marker_type: ChapterType::Intro,
                            });
                        }
                        "credits" => {
                            credits_marker = Some(ChapterMarker {
                                start_time: Duration::from_millis(marker.start_time_offset),
                                end_time: Duration::from_millis(marker.end_time_offset),
                                marker_type: ChapterType::Credits,
                            });
                        }
                        _ => {}
                    }
                }
            } else {
                info!("No markers found for media ID: {}", rating_key);
            }
        } else {
            warn!("No metadata found in response for media ID: {}", rating_key);
        }

        Ok((intro_marker, credits_marker))
    }
}

// Plex API Response Types

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlexMetadataResponse {
    media_container: PlexMetadataContainer,
}

#[derive(Debug, Deserialize)]
struct PlexMetadataContainer {
    #[serde(rename = "Metadata", default)]
    metadata: Vec<PlexMetadataWithMarkers>,
}

#[derive(Debug, Deserialize)]
struct PlexMetadataWithMarkers {
    #[serde(rename = "Marker", default)]
    marker: Option<Vec<PlexMarker>>,
}

#[derive(Debug, Deserialize)]
struct PlexMarker {
    #[serde(rename = "type")]
    type_: String,
    #[serde(rename = "startTimeOffset")]
    start_time_offset: u64,
    #[serde(rename = "endTimeOffset")]
    end_time_offset: u64,
}

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
    view_count: Option<u32>,
    view_offset: Option<u64>,
    last_viewed_at: Option<i64>,
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
    view_count: Option<u32>,
    viewed_leaf_count: Option<u32>,
    leaf_count: Option<u32>,
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
    view_count: Option<u32>,
    view_offset: Option<u64>,
    last_viewed_at: Option<i64>,
    grandparent_rating_key: Option<String>,
    grandparent_title: Option<String>,
    grandparent_thumb: Option<String>,
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

// Generic metadata structure that can handle movies, shows, and episodes
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlexGenericMetadata {
    rating_key: String,
    title: String,
    #[serde(rename = "type")]
    type_: Option<String>,
    year: Option<u32>,
    rating: Option<f32>,
    thumb: Option<String>,
    art: Option<String>,
    summary: Option<String>,
    duration: Option<u64>,
    view_count: Option<u32>,
    view_offset: Option<u64>,
    last_viewed_at: Option<i64>,
    added_at: Option<i64>,
    updated_at: Option<i64>,
    parent_index: Option<u32>,              // Season number for episodes
    index: Option<u32>,                     // Episode number
    viewed_leaf_count: Option<u32>,         // For shows
    leaf_count: Option<u32>,                // Total episodes for shows
    grandparent_title: Option<String>,      // Show name for episodes
    grandparent_thumb: Option<String>,      // Show poster for episodes
    grandparent_rating_key: Option<String>, // Show ID for episodes
    #[serde(rename = "Genre", default)]
    genre: Option<Vec<PlexTag>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlexOnDeckResponse {
    media_container: PlexOnDeckContainer,
}

#[derive(Debug, Deserialize)]
struct PlexOnDeckContainer {
    #[serde(rename = "Metadata", default)]
    metadata: Vec<PlexGenericMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlexRecentlyAddedResponse {
    media_container: PlexRecentlyAddedContainer,
}

#[derive(Debug, Deserialize)]
struct PlexRecentlyAddedContainer {
    #[serde(rename = "Metadata", default)]
    metadata: Vec<PlexGenericMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlexHubsResponse {
    media_container: PlexHubsContainer,
}

#[derive(Debug, Deserialize)]
struct PlexHubsContainer {
    #[serde(rename = "Hub", default)]
    hub: Vec<PlexHub>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlexHub {
    title: String,
    key: Option<String>,
    context: Option<String>,
    #[serde(rename = "Metadata", default)]
    metadata: Vec<PlexGenericMetadata>,
}
