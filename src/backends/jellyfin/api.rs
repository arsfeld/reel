use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::models::{
    Episode, HomeSection, HomeSectionType, Library, LibraryType, MediaItem, Movie, Resolution,
    Season, Show, StreamInfo, User,
};

const JELLYFIN_CLIENT_NAME: &str = "Reel";
const JELLYFIN_VERSION: &str = "0.1.0";

#[derive(Clone)]
pub struct JellyfinApi {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    user_id: String,
    device_id: String,
    backend_id: String,
}

impl JellyfinApi {
    pub fn new(base_url: String, api_key: String, user_id: String) -> Self {
        Self::with_backend_id(base_url, api_key, user_id, "jellyfin".to_string())
    }

    pub fn with_backend_id(
        base_url: String,
        api_key: String,
        user_id: String,
        backend_id: String,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        let device_id = Self::get_or_create_device_id();

        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            user_id,
            device_id,
            backend_id,
        }
    }

    fn get_or_create_device_id() -> String {
        Uuid::new_v4().to_string()
    }

    fn get_auth_header(&self) -> String {
        format!(
            r#"MediaBrowser Client="{}", Device="Linux", DeviceId="{}", Version="{}", Token="{}""#,
            JELLYFIN_CLIENT_NAME, self.device_id, JELLYFIN_VERSION, self.api_key
        )
    }

    pub async fn get_server_info(&self) -> Result<ServerInfo> {
        let url = format!("{}/System/Info/Public", self.base_url);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get server info: {}", response.status()));
        }

        let info: ServerInfo = response.json().await?;
        Ok(info)
    }

    pub async fn authenticate(
        base_url: &str,
        username: &str,
        password: &str,
    ) -> Result<AuthResponse> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        let device_id = Self::get_or_create_device_id();
        let auth_header = format!(
            r#"MediaBrowser Client="{}", Device="Linux", DeviceId="{}", Version="{}""#,
            JELLYFIN_CLIENT_NAME, device_id, JELLYFIN_VERSION
        );

        let url = format!(
            "{}/Users/AuthenticateByName",
            base_url.trim_end_matches('/')
        );

        info!("Attempting to authenticate with Jellyfin at: {}", url);
        debug!("Auth header: {}", auth_header);

        let auth_request = AuthRequest {
            username: username.to_string(),
            pw: password.to_string(),
        };

        let response = client
            .post(&url)
            .header("X-Emby-Authorization", auth_header)
            .header("Content-Type", "application/json")
            .json(&auth_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!(
                "Authentication failed with status {}: {}",
                status, error_text
            );
            return Err(anyhow!(
                "Authentication failed: {} - {}",
                status,
                error_text
            ));
        }

        let response_text = response.text().await?;
        debug!("Authentication response: {}", response_text);

        let auth_response: AuthResponse = serde_json::from_str(&response_text).map_err(|e| {
            error!("Failed to parse authentication response: {}", e);
            error!("Response was: {}", response_text);
            anyhow!("Failed to parse authentication response: {}", e)
        })?;

        info!(
            "Successfully authenticated with Jellyfin as user: {}",
            auth_response.user.name
        );
        Ok(auth_response)
    }

    pub async fn get_user(&self) -> Result<User> {
        let url = format!("{}/Users/{}", self.base_url, self.user_id);

        let response = self
            .client
            .get(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get user info: {}", response.status()));
        }

        let jellyfin_user: JellyfinUser = response.json().await?;

        Ok(User {
            id: jellyfin_user.id.clone(),
            username: jellyfin_user.name,
            email: None,
            avatar_url: jellyfin_user.primary_image_tag.map(|tag| {
                format!(
                    "{}/Users/{}/Images/Primary?tag={}",
                    self.base_url, jellyfin_user.id, tag
                )
            }),
        })
    }

    pub async fn get_libraries(&self) -> Result<Vec<Library>> {
        let url = format!("{}/Users/{}/Views", self.base_url, self.user_id);

        let response = self
            .client
            .get(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get libraries: {}", response.status()));
        }

        let views_response: ViewsResponse = response.json().await?;

        let libraries: Vec<Library> = views_response
            .items
            .into_iter()
            .map(|view| Library {
                id: view.id.clone(),
                title: view.name,
                library_type: match view.collection_type.as_deref() {
                    Some("movies") => LibraryType::Movies,
                    Some("tvshows") => LibraryType::Shows,
                    Some("music") => LibraryType::Music,
                    Some("homevideos") | Some("photos") => LibraryType::Photos,
                    Some("mixed") | _ => LibraryType::Mixed,
                },
                icon: view.primary_image_tag.map(|tag| {
                    format!(
                        "{}/Items/{}/Images/Primary?tag={}",
                        self.base_url, view.id, tag
                    )
                }),
            })
            .collect();

        info!("Found {} libraries", libraries.len());
        Ok(libraries)
    }

    pub async fn get_movies(&self, library_id: &str) -> Result<Vec<Movie>> {
        let url = format!(
            "{}/Users/{}/Items?ParentId={}&IncludeItemTypes=Movie&Fields=Overview,Genres,DateCreated,MediaStreams,People,ProviderIds,RunTimeTicks&SortBy=SortName",
            self.base_url, self.user_id, library_id
        );

        debug!("Fetching movies from URL: {}", url);
        let response = self
            .client
            .get(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send request to Jellyfin: {}", e);
                anyhow!("Failed to send request to Jellyfin: {}", e)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!(
                "Failed to get movies from Jellyfin: {} - {}",
                status, error_text
            );
            return Err(anyhow!("Failed to get movies: {} - {}", status, error_text));
        }

        // First, get the response text to debug
        let response_text = response.text().await.map_err(|e| {
            error!("Failed to read movies response body: {}", e);
            anyhow!("Failed to read movies response body: {}", e)
        })?;

        debug!(
            "Raw movies response (first 1000 chars): {}",
            &response_text[..response_text.len().min(1000)]
        );

        let items_response: ItemsResponse = serde_json::from_str(&response_text).map_err(|e| {
            error!("Failed to parse movies response from Jellyfin: {}", e);
            error!(
                "Response was: {}",
                &response_text[..response_text.len().min(500)]
            );
            anyhow!("Failed to parse movies response: {}", e)
        })?;

        let movies: Vec<Movie> = items_response
            .items
            .into_iter()
            .map(|item| {
                let duration = Duration::from_secs(item.run_time_ticks.unwrap_or(0) / 10_000_000);
                let (cast, crew) = self.convert_people_to_cast_crew(item.people.clone());

                Movie {
                    id: item.id.clone(),
                    backend_id: self.backend_id.clone(),
                    title: item.name,
                    year: item.production_year,
                    duration,
                    rating: item.community_rating,
                    poster_url: self.build_image_url(
                        &item.id,
                        "Primary",
                        item.image_tags.primary.as_deref(),
                    ),
                    backdrop_url: self.build_image_url(
                        &item.id,
                        "Backdrop",
                        item.backdrop_image_tags.first().map(|s| s.as_str()),
                    ),
                    overview: item.overview,
                    genres: item.genres.unwrap_or_default(),
                    cast,
                    crew,
                    added_at: item
                        .date_created
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    updated_at: None,
                    watched: item.user_data.as_ref().is_some_and(|ud| ud.played),
                    view_count: item.user_data.as_ref().map_or(0, |ud| ud.play_count),
                    last_watched_at: item
                        .user_data
                        .as_ref()
                        .and_then(|ud| ud.last_played_date.as_ref())
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    playback_position: item
                        .user_data
                        .as_ref()
                        .and_then(|ud| ud.playback_position_ticks)
                        .map(|ticks| Duration::from_secs(ticks / 10_000_000)),
                    intro_marker: None,
                    credits_marker: None,
                }
            })
            .collect();

        info!("Found {} movies in library {}", movies.len(), library_id);
        Ok(movies)
    }

    pub async fn get_shows(&self, library_id: &str) -> Result<Vec<Show>> {
        let url = format!(
            "{}/Users/{}/Items?ParentId={}&IncludeItemTypes=Series&Fields=Overview,Genres,DateCreated,ChildCount,People&SortBy=SortName",
            self.base_url, self.user_id, library_id
        );

        debug!("Fetching shows from URL: {}", url);
        let response = self
            .client
            .get(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send request to Jellyfin: {}", e);
                anyhow!("Failed to send request to Jellyfin: {}", e)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!(
                "Failed to get shows from Jellyfin: {} - {}",
                status, error_text
            );
            return Err(anyhow!("Failed to get shows: {} - {}", status, error_text));
        }

        // First, get the response text to debug
        let response_text = response.text().await.map_err(|e| {
            error!("Failed to read shows response body: {}", e);
            anyhow!("Failed to read shows response body: {}", e)
        })?;

        debug!(
            "Raw shows response (first 1000 chars): {}",
            &response_text[..response_text.len().min(1000)]
        );

        let items_response: ItemsResponse = serde_json::from_str(&response_text).map_err(|e| {
            error!("Failed to parse shows response from Jellyfin: {}", e);
            error!(
                "Response was: {}",
                &response_text[..response_text.len().min(500)]
            );
            anyhow!("Failed to parse shows response: {}", e)
        })?;

        let mut shows = Vec::new();
        for item in items_response.items {
            let seasons = self.get_seasons(&item.id).await?;
            let (cast, _crew) = self.convert_people_to_cast_crew(item.people.clone());

            shows.push(Show {
                id: item.id.clone(),
                backend_id: self.backend_id.clone(),
                title: item.name,
                year: item.production_year,
                seasons,
                rating: item.community_rating,
                poster_url: self.build_image_url(
                    &item.id,
                    "Primary",
                    item.image_tags.primary.as_deref(),
                ),
                backdrop_url: self.build_image_url(
                    &item.id,
                    "Backdrop",
                    item.backdrop_image_tags.first().map(|s| s.as_str()),
                ),
                overview: item.overview,
                genres: item.genres.unwrap_or_default(),
                cast,
                added_at: item
                    .date_created
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                updated_at: None,
                watched_episode_count: item.user_data.as_ref().map_or(0, |ud| ud.played_count),
                total_episode_count: item.child_count.unwrap_or(0) as u32,
                last_watched_at: item
                    .user_data
                    .as_ref()
                    .and_then(|ud| ud.last_played_date.as_ref())
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
            });
        }

        info!("Found {} shows in library {}", shows.len(), library_id);
        Ok(shows)
    }

    pub async fn get_seasons(&self, show_id: &str) -> Result<Vec<Season>> {
        let url = format!(
            "{}/Shows/{}/Seasons?userId={}&Fields=ItemCounts",
            self.base_url, show_id, self.user_id
        );

        let response = self
            .client
            .get(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get seasons: {}", response.status()));
        }

        let items_response: ItemsResponse = response.json().await?;

        let seasons = items_response
            .items
            .into_iter()
            .map(|item| Season {
                id: item.id.clone(),
                season_number: item.index_number.unwrap_or(0) as u32,
                episode_count: item.child_count.unwrap_or(0) as u32,
                poster_url: self.build_image_url(
                    &item.id,
                    "Primary",
                    item.image_tags.primary.as_deref(),
                ),
            })
            .collect();

        Ok(seasons)
    }

    pub async fn get_episodes(&self, season_id: &str) -> Result<Vec<Episode>> {
        self.get_episodes_with_segments(season_id, false).await
    }

    pub async fn get_episodes_with_segments(
        &self,
        season_id: &str,
        include_segments: bool,
    ) -> Result<Vec<Episode>> {
        let url = format!(
            "{}/Users/{}/Items?ParentId={}&IncludeItemTypes=Episode&Fields=Overview,MediaStreams,DateCreated&SortBy=IndexNumber",
            self.base_url, self.user_id, season_id
        );

        let response = self
            .client
            .get(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get episodes: {}", response.status()));
        }

        let items_response: ItemsResponse = response.json().await?;

        let mut episodes = Vec::new();
        for item in items_response.items {
            let duration = Duration::from_secs(item.run_time_ticks.unwrap_or(0) / 10_000_000);

            // Only try to get media segments if explicitly requested (e.g., for individual episode details)
            let (intro_marker, credits_marker) = if include_segments {
                if let Ok(segments) = self.get_media_segments(&item.id).await {
                    let mut intro = None;
                    let mut credits = None;

                    for segment in segments {
                        match segment.segment_type {
                            MediaSegmentType::Intro => {
                                intro = Some(crate::models::ChapterMarker {
                                    start_time: Duration::from_secs(
                                        segment.start_ticks / 10_000_000,
                                    ),
                                    end_time: Duration::from_secs(segment.end_ticks / 10_000_000),
                                    marker_type: crate::models::ChapterType::Intro,
                                });
                            }
                            MediaSegmentType::Credits | MediaSegmentType::Outro => {
                                credits = Some(crate::models::ChapterMarker {
                                    start_time: Duration::from_secs(
                                        segment.start_ticks / 10_000_000,
                                    ),
                                    end_time: Duration::from_secs(segment.end_ticks / 10_000_000),
                                    marker_type: crate::models::ChapterType::Credits,
                                });
                            }
                            _ => {}
                        }
                    }

                    (intro, credits)
                } else {
                    (None, None)
                }
            } else {
                // Skip media segments during bulk sync to avoid excessive API calls
                (None, None)
            };

            episodes.push(Episode {
                id: item.id.clone(),
                backend_id: self.backend_id.clone(),
                show_id: item.series_id.clone(),
                title: item.name,
                season_number: item.parent_index_number.unwrap_or(0) as u32,
                episode_number: item.index_number.unwrap_or(0) as u32,
                duration,
                thumbnail_url: self.build_image_url(
                    &item.id,
                    "Primary",
                    item.image_tags.primary.as_deref(),
                ),
                overview: item.overview,
                air_date: item
                    .premiere_date
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                watched: item.user_data.as_ref().is_some_and(|ud| ud.played),
                view_count: item.user_data.as_ref().map_or(0, |ud| ud.play_count),
                last_watched_at: item
                    .user_data
                    .as_ref()
                    .and_then(|ud| ud.last_played_date.as_ref())
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                playback_position: item
                    .user_data
                    .as_ref()
                    .and_then(|ud| ud.playback_position_ticks)
                    .map(|ticks| Duration::from_secs(ticks / 10_000_000)),
                show_title: item.series_name,
                show_poster_url: None,
                intro_marker,
                credits_marker,
            });
        }

        Ok(episodes)
    }

    pub async fn get_stream_url(&self, media_id: &str) -> Result<StreamInfo> {
        let playback_info_url = format!(
            "{}/Items/{}/PlaybackInfo?UserId={}&StartTimeTicks=0&IsPlayback=true&AutoOpenLiveStream=true&MediaSourceId={}",
            self.base_url, media_id, self.user_id, media_id
        );

        let response = self
            .client
            .post(&playback_info_url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "DeviceProfile": {
                    "MaxStreamingBitrate": 120000000,
                    "DirectPlayProfiles": [
                        {
                            "Container": "mp4,m4v,mkv,webm",
                            "Type": "Video",
                            "VideoCodec": "h264,hevc,vp8,vp9,av1",
                            "AudioCodec": "aac,mp3,opus,flac,vorbis"
                        }
                    ],
                    "TranscodingProfiles": [
                        {
                            "Container": "mp4",
                            "Type": "Video",
                            "AudioCodec": "aac",
                            "VideoCodec": "h264",
                            "Context": "Streaming",
                            "Protocol": "hls",
                            "MaxAudioChannels": "6"
                        }
                    ]
                }
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to get playback info: {}",
                response.status()
            ));
        }

        let playback_info: PlaybackInfoResponse = response.json().await?;

        if playback_info.media_sources.is_empty() {
            return Err(anyhow!("No media sources available"));
        }

        let media_source = &playback_info.media_sources[0];

        let stream_url = if media_source.supports_direct_play {
            format!(
                "{}/Videos/{}/stream?Static=true&mediaSourceId={}&api_key={}",
                self.base_url, media_id, media_source.id, self.api_key
            )
        } else if media_source.supports_direct_stream {
            format!(
                "{}/Videos/{}/stream?mediaSourceId={}&api_key={}",
                self.base_url, media_id, media_source.id, self.api_key
            )
        } else {
            format!(
                "{}/Videos/{}/main.m3u8?mediaSourceId={}&api_key={}",
                self.base_url, media_id, media_source.id, self.api_key
            )
        };

        let video_stream = media_source
            .media_streams
            .iter()
            .find(|s| s.stream_type == "Video")
            .ok_or_else(|| anyhow!("No video stream found"))?;

        let audio_stream = media_source
            .media_streams
            .iter()
            .find(|s| s.stream_type == "Audio");

        Ok(StreamInfo {
            url: stream_url,
            direct_play: media_source.supports_direct_play,
            video_codec: video_stream.codec.clone().unwrap_or_default(),
            audio_codec: audio_stream
                .and_then(|s| s.codec.clone())
                .unwrap_or_default(),
            container: media_source.container.clone().unwrap_or_default(),
            bitrate: media_source.bitrate.unwrap_or(0) as u64,
            resolution: Resolution {
                width: video_stream.width.unwrap_or(0) as u32,
                height: video_stream.height.unwrap_or(0) as u32,
            },
            quality_options: vec![],
        })
    }

    pub async fn report_playback_start(&self, media_id: &str) -> Result<()> {
        let url = format!("{}/Sessions/Playing", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .json(&serde_json::json!({
                "ItemId": media_id,
                "MediaSourceId": media_id,
                "PositionTicks": 0,
                "IsPaused": false,
                "IsMuted": false,
                "PlayMethod": "DirectPlay",
                "PlaySessionId": Uuid::new_v4().to_string(),
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            warn!("Failed to report playback start: {}", response.status());
        }

        Ok(())
    }

    pub async fn update_playback_progress(&self, media_id: &str, position: Duration) -> Result<()> {
        let url = format!("{}/Sessions/Playing/Progress", self.base_url);

        let position_ticks = position.as_secs() * 10_000_000;

        let response = self
            .client
            .post(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .json(&serde_json::json!({
                "ItemId": media_id,
                "MediaSourceId": media_id,
                "PositionTicks": position_ticks,
                "IsPaused": false,
                "IsMuted": false,
                "PlayMethod": "DirectPlay",
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            warn!("Failed to update playback progress: {}", response.status());
        }

        Ok(())
    }

    pub async fn report_playback_stopped(&self, media_id: &str, position: Duration) -> Result<()> {
        let url = format!("{}/Sessions/Playing/Stopped", self.base_url);

        let position_ticks = position.as_secs() * 10_000_000;

        let response = self
            .client
            .post(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .json(&serde_json::json!({
                "ItemId": media_id,
                "MediaSourceId": media_id,
                "PositionTicks": position_ticks,
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            warn!("Failed to report playback stopped: {}", response.status());
        }

        Ok(())
    }

    pub async fn mark_as_watched(&self, media_id: &str) -> Result<()> {
        let url = format!(
            "{}/Users/{}/PlayedItems/{}",
            self.base_url, self.user_id, media_id
        );

        let response = self
            .client
            .post(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to mark as watched: {}", response.status()));
        }

        Ok(())
    }

    pub async fn mark_as_unwatched(&self, media_id: &str) -> Result<()> {
        let url = format!(
            "{}/Users/{}/PlayedItems/{}",
            self.base_url, self.user_id, media_id
        );

        let response = self
            .client
            .delete(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
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

    pub async fn get_item(&self, media_id: &str) -> Result<JellyfinItem> {
        let url = format!(
            "{}/Users/{}/Items/{}?Fields=UserData,Overview,Genres,RunTimeTicks",
            self.base_url, self.user_id, media_id
        );

        let response = self
            .client
            .get(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get item: {}", response.status()));
        }

        let item: JellyfinItem = response.json().await?;
        Ok(item)
    }

    pub async fn get_watch_status(
        &self,
        media_id: &str,
    ) -> Result<crate::backends::traits::WatchStatus> {
        let item = self.get_item(media_id).await?;

        let watch_status = crate::backends::traits::WatchStatus {
            watched: item.user_data.as_ref().is_some_and(|ud| ud.played),
            view_count: item.user_data.as_ref().map_or(0, |ud| ud.play_count),
            last_watched_at: item
                .user_data
                .as_ref()
                .and_then(|ud| ud.last_played_date.as_ref())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            playback_position: item
                .user_data
                .as_ref()
                .and_then(|ud| ud.playback_position_ticks)
                .map(|ticks| Duration::from_secs(ticks / 10_000_000)),
        };

        Ok(watch_status)
    }

    pub async fn get_media_segments(&self, item_id: &str) -> Result<Vec<MediaSegment>> {
        let url = format!("{}/Items/{}/MediaSegments", self.base_url, item_id);

        let response = self
            .client
            .get(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            debug!(
                "No media segments found for item {}: {}",
                item_id,
                response.status()
            );
            return Ok(vec![]);
        }

        let segments: MediaSegmentsResponse = response.json().await?;
        Ok(segments.items)
    }

    pub async fn find_next_episode(&self, current_episode: &Episode) -> Result<Option<Episode>> {
        // First, get the current episode's full info to get series ID
        let current_item = self.get_item(&current_episode.id).await?;

        let series_id = match current_item.series_id {
            Some(id) => id,
            None => {
                debug!("No series ID found for episode {}", current_episode.id);
                return Ok(None);
            }
        };

        // Get all episodes for the series
        let url = format!(
            "{}/Shows/{}/Episodes?UserId={}&Fields=Overview,MediaStreams,DateCreated&SortBy=SortName",
            self.base_url, series_id, self.user_id
        );

        let response = self
            .client
            .get(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            warn!("Failed to get episodes for series: {}", response.status());
            return Ok(None);
        }

        let items_response: ItemsResponse = response.json().await?;

        // Find the current episode and get the next one
        let episodes: Vec<_> = items_response
            .items
            .into_iter()
            .filter(|item| item.item_type.as_deref() == Some("Episode"))
            .collect();

        // Find current episode index
        let current_index = episodes.iter().position(|ep| ep.id == current_episode.id);

        match current_index {
            Some(idx) if idx + 1 < episodes.len() => {
                let next_item = &episodes[idx + 1];
                let duration =
                    Duration::from_secs(next_item.run_time_ticks.unwrap_or(0) / 10_000_000);

                Ok(Some(Episode {
                    id: next_item.id.clone(),
                    backend_id: self.backend_id.clone(),
                    show_id: next_item.series_id.clone(),
                    title: next_item.name.clone(),
                    season_number: next_item.parent_index_number.unwrap_or(0) as u32,
                    episode_number: next_item.index_number.unwrap_or(0) as u32,
                    duration,
                    thumbnail_url: self.build_image_url(
                        &next_item.id,
                        "Primary",
                        next_item.image_tags.primary.as_deref(),
                    ),
                    overview: next_item.overview.clone(),
                    air_date: next_item
                        .premiere_date
                        .as_ref()
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    watched: next_item.user_data.as_ref().is_some_and(|ud| ud.played),
                    view_count: next_item.user_data.as_ref().map_or(0, |ud| ud.play_count),
                    last_watched_at: next_item
                        .user_data
                        .as_ref()
                        .and_then(|ud| ud.last_played_date.as_ref())
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    playback_position: next_item
                        .user_data
                        .as_ref()
                        .and_then(|ud| ud.playback_position_ticks)
                        .map(|ticks| Duration::from_secs(ticks / 10_000_000)),
                    show_title: next_item.series_name.clone(),
                    show_poster_url: None,
                    intro_marker: None,
                    credits_marker: None,
                }))
            }
            _ => {
                debug!("No next episode found for episode {}", current_episode.id);
                Ok(None)
            }
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<MediaItem>> {
        let url = format!(
            "{}/Users/{}/Items?searchTerm={}&IncludeItemTypes=Movie,Series,Episode&Fields=Overview,Genres,People&Limit=50",
            self.base_url,
            self.user_id,
            utf8_percent_encode(query, NON_ALPHANUMERIC)
        );

        let response = self
            .client
            .get(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to search: {}", response.status()));
        }

        let items_response: ItemsResponse = response.json().await?;

        let mut results = Vec::new();

        for item in items_response.items {
            match item.item_type.as_deref() {
                Some("Movie") => {
                    let duration =
                        Duration::from_secs(item.run_time_ticks.unwrap_or(0) / 10_000_000);
                    let (cast, crew) = self.convert_people_to_cast_crew(item.people.clone());
                    results.push(MediaItem::Movie(Movie {
                        id: item.id.clone(),
                        backend_id: self.backend_id.clone(),
                        title: item.name,
                        year: item.production_year,
                        duration,
                        rating: item.community_rating,
                        poster_url: self.build_image_url(
                            &item.id,
                            "Primary",
                            item.image_tags.primary.as_deref(),
                        ),
                        backdrop_url: self.build_image_url(
                            &item.id,
                            "Backdrop",
                            item.backdrop_image_tags.first().map(|s| s.as_str()),
                        ),
                        overview: item.overview,
                        genres: item.genres.unwrap_or_default(),
                        cast,
                        crew,
                        added_at: None,
                        updated_at: None,
                        watched: item.user_data.as_ref().is_some_and(|ud| ud.played),
                        view_count: item.user_data.as_ref().map_or(0, |ud| ud.play_count),
                        last_watched_at: None,
                        playback_position: None,
                        intro_marker: None,
                        credits_marker: None,
                    }));
                }
                Some("Series") => {
                    let (cast, _crew) = self.convert_people_to_cast_crew(item.people.clone());
                    results.push(MediaItem::Show(Show {
                        id: item.id.clone(),
                        backend_id: self.backend_id.clone(),
                        title: item.name,
                        year: item.production_year,
                        seasons: vec![],
                        rating: item.community_rating,
                        poster_url: self.build_image_url(
                            &item.id,
                            "Primary",
                            item.image_tags.primary.as_deref(),
                        ),
                        backdrop_url: self.build_image_url(
                            &item.id,
                            "Backdrop",
                            item.backdrop_image_tags.first().map(|s| s.as_str()),
                        ),
                        overview: item.overview,
                        genres: item.genres.unwrap_or_default(),
                        cast,
                        added_at: None,
                        updated_at: None,
                        watched_episode_count: 0,
                        total_episode_count: 0,
                        last_watched_at: None,
                    }));
                }
                Some("Episode") => {
                    let duration =
                        Duration::from_secs(item.run_time_ticks.unwrap_or(0) / 10_000_000);
                    results.push(MediaItem::Episode(Episode {
                        id: item.id.clone(),
                        backend_id: self.backend_id.clone(),
                        show_id: item.series_id.clone(),
                        title: item.name,
                        season_number: item.parent_index_number.unwrap_or(0) as u32,
                        episode_number: item.index_number.unwrap_or(0) as u32,
                        duration,
                        thumbnail_url: self.build_image_url(
                            &item.id,
                            "Primary",
                            item.image_tags.primary.as_deref(),
                        ),
                        overview: item.overview,
                        air_date: None,
                        watched: item.user_data.as_ref().is_some_and(|ud| ud.played),
                        view_count: item.user_data.as_ref().map_or(0, |ud| ud.play_count),
                        last_watched_at: None,
                        playback_position: None,
                        show_title: item.series_name,
                        show_poster_url: None,
                        intro_marker: None,
                        credits_marker: None,
                    }));
                }
                _ => {}
            }
        }

        Ok(results)
    }

    pub async fn get_home_sections(&self) -> Result<Vec<HomeSection>> {
        let mut sections = Vec::new();

        let continue_watching = self.get_continue_watching().await?;
        if !continue_watching.is_empty() {
            sections.push(HomeSection {
                id: "continue_watching".to_string(),
                title: "Continue Watching".to_string(),
                section_type: HomeSectionType::ContinueWatching,
                items: continue_watching,
            });
        }

        let latest_movies = self.get_latest_movies().await?;
        if !latest_movies.is_empty() {
            sections.push(HomeSection {
                id: "latest_movies".to_string(),
                title: "Recently Added Movies".to_string(),
                section_type: HomeSectionType::RecentlyAdded,
                items: latest_movies,
            });
        }

        let next_up = self.get_next_up().await?;
        if !next_up.is_empty() {
            sections.push(HomeSection {
                id: "next_up".to_string(),
                title: "Next Up".to_string(),
                section_type: HomeSectionType::ContinueWatching,
                items: next_up,
            });
        }

        Ok(sections)
    }

    async fn get_continue_watching(&self) -> Result<Vec<MediaItem>> {
        let url = format!(
            "{}/Users/{}/Items/Resume?Fields=Overview,Genres,People&Limit=20",
            self.base_url, self.user_id
        );

        let response = self
            .client
            .get(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            warn!("Failed to get continue watching: {}", response.status());
            return Ok(Vec::new());
        }

        let items_response: ItemsResponse = response.json().await?;
        Ok(self.convert_items_to_media(items_response.items))
    }

    async fn get_latest_movies(&self) -> Result<Vec<MediaItem>> {
        let url = format!(
            "{}/Users/{}/Items/Latest?IncludeItemTypes=Movie&Fields=Overview,Genres,People&Limit=20",
            self.base_url, self.user_id
        );

        let response = self
            .client
            .get(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            warn!("Failed to get latest movies: {}", response.status());
            return Ok(Vec::new());
        }

        let items: Vec<JellyfinItem> = response.json().await?;
        Ok(self.convert_items_to_media(items))
    }

    async fn get_next_up(&self) -> Result<Vec<MediaItem>> {
        let url = format!(
            "{}/Shows/NextUp?UserId={}&Fields=Overview,Genres,People&Limit=20",
            self.base_url, self.user_id
        );

        let response = self
            .client
            .get(&url)
            .header("X-Emby-Authorization", self.get_auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            warn!("Failed to get next up: {}", response.status());
            return Ok(Vec::new());
        }

        let items_response: ItemsResponse = response.json().await?;
        Ok(self.convert_items_to_media(items_response.items))
    }

    fn convert_items_to_media(&self, items: Vec<JellyfinItem>) -> Vec<MediaItem> {
        items
            .into_iter()
            .filter_map(|item| match item.item_type.as_deref() {
                Some("Movie") => {
                    let duration =
                        Duration::from_secs(item.run_time_ticks.unwrap_or(0) / 10_000_000);
                    let (cast, crew) = self.convert_people_to_cast_crew(item.people.clone());
                    Some(MediaItem::Movie(Movie {
                        id: item.id.clone(),
                        backend_id: self.backend_id.clone(),
                        title: item.name,
                        year: item.production_year,
                        duration,
                        rating: item.community_rating,
                        poster_url: self.build_image_url(
                            &item.id,
                            "Primary",
                            item.image_tags.primary.as_deref(),
                        ),
                        backdrop_url: self.build_image_url(
                            &item.id,
                            "Backdrop",
                            item.backdrop_image_tags.first().map(|s| s.as_str()),
                        ),
                        overview: item.overview,
                        genres: item.genres.unwrap_or_default(),
                        cast,
                        crew,
                        added_at: None,
                        updated_at: None,
                        watched: item.user_data.as_ref().is_some_and(|ud| ud.played),
                        view_count: item.user_data.as_ref().map_or(0, |ud| ud.play_count),
                        last_watched_at: None,
                        playback_position: item
                            .user_data
                            .as_ref()
                            .and_then(|ud| ud.playback_position_ticks)
                            .map(|ticks| Duration::from_secs(ticks / 10_000_000)),
                        intro_marker: None,
                        credits_marker: None,
                    }))
                }
                Some("Episode") => {
                    let duration =
                        Duration::from_secs(item.run_time_ticks.unwrap_or(0) / 10_000_000);
                    Some(MediaItem::Episode(Episode {
                        id: item.id.clone(),
                        backend_id: self.backend_id.clone(),
                        show_id: item.series_id.clone(),
                        title: item.name,
                        season_number: item.parent_index_number.unwrap_or(0) as u32,
                        episode_number: item.index_number.unwrap_or(0) as u32,
                        duration,
                        thumbnail_url: self.build_image_url(
                            &item.id,
                            "Primary",
                            item.image_tags.primary.as_deref(),
                        ),
                        overview: item.overview,
                        air_date: None,
                        watched: item.user_data.as_ref().is_some_and(|ud| ud.played),
                        view_count: item.user_data.as_ref().map_or(0, |ud| ud.play_count),
                        last_watched_at: None,
                        playback_position: item
                            .user_data
                            .as_ref()
                            .and_then(|ud| ud.playback_position_ticks)
                            .map(|ticks| Duration::from_secs(ticks / 10_000_000)),
                        show_title: item.series_name,
                        show_poster_url: None,
                        intro_marker: None,
                        credits_marker: None,
                    }))
                }
                _ => None,
            })
            .collect()
    }

    fn build_image_url(
        &self,
        item_id: &str,
        image_type: &str,
        tag: Option<&str>,
    ) -> Option<String> {
        tag.map(|t| {
            format!(
                "{}/Items/{}/Images/{}?tag={}",
                self.base_url, item_id, image_type, t
            )
        })
    }

    fn convert_people_to_cast_crew(
        &self,
        people: Option<Vec<BaseItemPerson>>,
    ) -> (Vec<crate::models::Person>, Vec<crate::models::Person>) {
        let mut cast = Vec::new();
        let mut crew = Vec::new();

        if let Some(people_list) = people {
            for person in people_list {
                let person_model = crate::models::Person {
                    id: person.id.as_ref().unwrap_or(&person.name).clone(),
                    name: person.name,
                    role: person.role,
                    image_url: person.primary_image_tag.as_ref().and_then(|tag| {
                        person.id.as_ref().map(|id| {
                            format!("{}/Items/{}/Images/Primary?tag={}", self.base_url, id, tag)
                        })
                    }),
                };

                match person.person_type.as_deref() {
                    Some("Actor") | Some("GuestStar") => cast.push(person_model),
                    Some("Director") | Some("Writer") | Some("Producer") | Some("Composer") => {
                        crew.push(person_model)
                    }
                    _ => {
                        // Default to cast if type is unknown
                        cast.push(person_model);
                    }
                }
            }
        }

        (cast, crew)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServerInfo {
    pub server_name: String,
    pub version: String,
    pub operating_system: String,
    pub id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct AuthRequest {
    username: String,
    pw: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthResponse {
    pub user: JellyfinUser,
    pub access_token: String,
    pub server_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct JellyfinUser {
    pub id: String,
    pub name: String,
    pub primary_image_tag: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ViewsResponse {
    items: Vec<JellyfinView>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct JellyfinView {
    id: String,
    name: String,
    collection_type: Option<String>,
    primary_image_tag: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ItemsResponse {
    items: Vec<JellyfinItem>,
    total_record_count: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct JellyfinItem {
    id: String,
    name: String,
    #[serde(rename = "Type")]
    item_type: Option<String>,
    production_year: Option<u32>,
    index_number: Option<i32>,
    parent_index_number: Option<i32>,
    premiere_date: Option<String>,
    date_created: Option<String>,
    run_time_ticks: Option<u64>,
    community_rating: Option<f32>,
    overview: Option<String>,
    genres: Option<Vec<String>>,
    #[serde(default)]
    image_tags: ImageTags,
    #[serde(default)]
    backdrop_image_tags: Vec<String>,
    user_data: Option<UserData>,
    series_name: Option<String>,
    series_id: Option<String>,
    season_id: Option<String>,
    child_count: Option<i32>,
    people: Option<Vec<BaseItemPerson>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct ImageTags {
    primary: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct UserData {
    #[serde(default)]
    played: bool,
    #[serde(default)]
    play_count: u32,
    #[serde(default)]
    played_count: u32,
    last_played_date: Option<String>,
    playback_position_ticks: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
struct BaseItemPerson {
    id: Option<String>,
    name: String,
    role: Option<String>,
    #[serde(rename = "Type")]
    person_type: Option<String>,
    primary_image_tag: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MediaSegment {
    pub id: String,
    pub item_id: String,
    #[serde(rename = "Type")]
    pub segment_type: MediaSegmentType,
    pub start_ticks: u64,
    pub end_ticks: u64,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum MediaSegmentType {
    Intro,
    Outro,
    Credits,
    Recap,
    Preview,
    Commercial,
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MediaSegmentsResponse {
    items: Vec<MediaSegment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PlaybackInfoResponse {
    media_sources: Vec<MediaSource>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MediaSource {
    id: String,
    container: Option<String>,
    bitrate: Option<u32>,
    supports_direct_play: bool,
    supports_direct_stream: bool,
    supports_transcoding: bool,
    media_streams: Vec<MediaStream>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MediaStream {
    #[serde(rename = "Type")]
    stream_type: String,
    codec: Option<String>,
    width: Option<i32>,
    height: Option<i32>,
}
