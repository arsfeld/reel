use std::sync::OnceLock;

use async_trait::async_trait;

use crate::models::{
    detail::MediaDetail,
    hub::MediaHub,
    library::LibrarySection,
    media::{MediaItem, SourceType},
};
use crate::services::media_source::{DownloadDescriptor, MediaSource, SourceError};

use super::api::PlexClient;
use super::convert::{
    plex_library_to_section, plex_metadata_to_media_detail, plex_metadata_to_media_item,
};
use super::error::PlexError;

/// MediaSource implementation backed by a Plex Media Server.
pub struct PlexSource {
    client: PlexClient,
    name: String,
    /// Cached library sections. Fetched once on first call; reused thereafter.
    libraries_cache: OnceLock<Vec<LibrarySection>>,
}

impl PlexSource {
    pub fn new(client: PlexClient, name: String) -> Self {
        Self {
            client,
            name,
            libraries_cache: OnceLock::new(),
        }
    }
}

impl From<PlexError> for SourceError {
    fn from(e: PlexError) -> Self {
        match e {
            PlexError::Unauthorized => SourceError::Auth("Invalid or expired Plex token".into()),
            PlexError::NotFound(msg) => SourceError::NotFound(msg),
            PlexError::Http(e) => SourceError::Connection(e.to_string()),
            PlexError::Deserialize(e) => SourceError::Other(format!("Parse error: {e}")),
            PlexError::Server { status, message } => {
                SourceError::Other(format!("Server error {status}: {message}"))
            }
        }
    }
}

impl From<super::transcode::TranscodeError> for SourceError {
    fn from(e: super::transcode::TranscodeError) -> Self {
        use super::transcode::TranscodeError::*;
        let msg = e.to_string();
        match e {
            // Connection-ish failures the play path can retry / fall back from.
            Timeout | Request(_) => SourceError::Connection(msg),
            // Server errors and a missing/garbled decision are loud: the play
            // path surfaces an actionable notice rather than silently
            // direct-playing an incompatible file (U7).
            Server { .. } | Parse(_) | NoDecision | Url => SourceError::Other(msg),
        }
    }
}

#[async_trait]
impl MediaSource for PlexSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn source_type(&self) -> SourceType {
        SourceType::Plex
    }

    async fn test_connection(&self) -> Result<String, SourceError> {
        Ok(self.client.test_connection().await?)
    }

    async fn libraries(&self) -> Result<Vec<LibrarySection>, SourceError> {
        if let Some(cached) = self.libraries_cache.get() {
            return Ok(cached.clone());
        }
        let plex_libs = self.client.libraries().await?;
        let sections: Vec<LibrarySection> = plex_libs
            .iter()
            .filter_map(plex_library_to_section)
            .collect();
        // OnceLock::set returns Err if already set — we only get here
        // when it's empty, so this always succeeds.
        let _ = self.libraries_cache.set(sections.clone());
        Ok(sections)
    }

    async fn library_items(&self, library_key: &str) -> Result<Vec<MediaItem>, SourceError> {
        let metadata = self.client.library_items(library_key).await?;
        let base_url = self.client.base_url();
        // The URL is authoritative for the section, and per-item payloads here
        // omit librarySectionID, so tag each item with the requested key.
        Ok(metadata
            .iter()
            .filter_map(|m| plex_metadata_to_media_item(m, base_url))
            .map(|mut item| {
                item.library_section_id = Some(library_key.to_string());
                item
            })
            .collect())
    }

    async fn metadata(&self, rating_key: &str) -> Result<MediaDetail, SourceError> {
        let plex_meta = self.client.metadata(rating_key).await?;
        let base_url = self.client.base_url();
        plex_metadata_to_media_detail(&plex_meta, base_url)
            .ok_or_else(|| SourceError::Other("Failed to convert metadata".into()))
    }

    async fn children(&self, rating_key: &str) -> Result<Vec<MediaItem>, SourceError> {
        let plex_children = self.client.children(rating_key).await?;
        let base_url = self.client.base_url();
        Ok(plex_children
            .iter()
            .filter_map(|m| plex_metadata_to_media_item(m, base_url))
            .collect())
    }

    fn playback_url(&self, part_key: &str) -> String {
        self.client.playback_url(part_key)
    }

    async fn resolve_playback(
        &self,
        req: &crate::models::playback::PlaybackRequest,
    ) -> Result<crate::models::playback::PlaybackDecision, SourceError> {
        // Delegates to the transcode client (U5), which reads `is_remote` from
        // the PlexClient (U2) to apply the default cap on Auto/remote.
        Ok(self.client.resolve_decision(req).await?)
    }

    /// Stop a transcode session (R14/U10).
    async fn stop_transcode(&self, session: &str) -> Result<(), SourceError> {
        Ok(self.client.stop_transcode(session).await?)
    }

    /// Keep a transcode session alive (R15/U10). Fired on a timer.
    async fn ping_transcode(&self, session: &str) -> Result<(), SourceError> {
        Ok(self.client.ping_transcode(session).await?)
    }

    fn artwork_url(&self, path: &str, width: u32, height: u32) -> String {
        self.client.transcode_image_url(path, width, height)
    }

    fn download_descriptor(&self, item: &MediaItem) -> Result<DownloadDescriptor, SourceError> {
        let part_key = item
            .file_path
            .as_deref()
            .filter(|p| !p.is_empty())
            .ok_or_else(|| {
                SourceError::NotFound(format!("No downloadable file for item {}", item.id))
            })?;
        Ok(DownloadDescriptor {
            url: self.client.playback_url(part_key),
            part_key: part_key.to_string(),
            // The list MediaItem carries no size; the transfer client takes the
            // total from the first response's Content-Length.
            expected_size: None,
        })
    }

    async fn collections(&self, library_key: &str) -> Result<Vec<MediaItem>, SourceError> {
        let plex_collections = self.client.collections(library_key).await?;
        let base_url = self.client.base_url();
        Ok(plex_collections
            .iter()
            .filter_map(|m| plex_metadata_to_media_item(m, base_url))
            .map(|mut item| {
                item.library_section_id = Some(library_key.to_string());
                item
            })
            .collect())
    }

    async fn collection_items(&self, collection_key: &str) -> Result<Vec<MediaItem>, SourceError> {
        let plex_items = self.client.collection_items(collection_key).await?;
        let base_url = self.client.base_url();
        Ok(plex_items
            .iter()
            .filter_map(|m| plex_metadata_to_media_item(m, base_url))
            .collect())
    }

    async fn recently_added(&self) -> Result<Vec<MediaItem>, SourceError> {
        let plex_items = self.client.recently_added().await?;
        let base_url = self.client.base_url();
        Ok(plex_items
            .iter()
            .filter_map(|m| plex_metadata_to_media_item(m, base_url))
            .collect())
    }

    async fn recently_added_in_library(
        &self,
        library_key: &str,
    ) -> Result<Vec<MediaItem>, SourceError> {
        let plex_items = self.client.recently_added_in_library(library_key).await?;
        let base_url = self.client.base_url();
        Ok(plex_items
            .iter()
            .filter_map(|m| plex_metadata_to_media_item(m, base_url))
            .map(|mut item| {
                item.library_section_id = Some(library_key.to_string());
                item
            })
            .collect())
    }

    async fn continue_watching(&self) -> Result<Vec<MediaItem>, SourceError> {
        let plex_items = self.client.on_deck().await?;
        let base_url = self.client.base_url();
        Ok(plex_items
            .iter()
            .filter_map(|m| plex_metadata_to_media_item(m, base_url))
            .collect())
    }

    async fn hubs(&self) -> Result<Vec<MediaHub>, SourceError> {
        let plex_hubs = self.client.hubs().await?;
        let base_url = self.client.base_url();
        let hubs = plex_hubs
            .into_iter()
            .map(|hub| {
                let items: Vec<MediaItem> = hub
                    .metadata
                    .iter()
                    .filter_map(|m| plex_metadata_to_media_item(m, base_url))
                    .collect();
                MediaHub {
                    title: hub.title,
                    identifier: hub.hub_identifier,
                    items,
                }
            })
            // Drop hubs that ended up empty (e.g. all items were unsupported
            // types) so the home view never renders an empty hub shelf.
            .filter(|hub| !hub.items.is_empty())
            .collect();
        Ok(hubs)
    }

    async fn report_progress(
        &self,
        rating_key: &str,
        state: &str,
        time_ms: i64,
        duration_ms: i64,
    ) -> Result<(), SourceError> {
        Ok(self
            .client
            .report_timeline(rating_key, state, time_ms, duration_ms)
            .await?)
    }

    async fn scrobble(&self, rating_key: &str) -> Result<(), SourceError> {
        Ok(self.client.scrobble(rating_key).await?)
    }

    async fn unscrobble(&self, rating_key: &str) -> Result<(), SourceError> {
        Ok(self.client.unscrobble(rating_key).await?)
    }

    async fn skip_markers(
        &self,
        rating_key: &str,
        duration_secs: f64,
    ) -> Result<crate::player::SkipMarkers, SourceError> {
        let chapters = self.client.chapters(rating_key).await?;
        super::convert::plex_chapters_to_skip_markers(&chapters, duration_secs).ok_or(
            SourceError::NotSupported("No intro/credits markers found".into()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn plex_source_test_connection() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_string(r#"{"MediaContainer":{"friendlyName":"My Server"}}"#),
            )
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let source = PlexSource::new(client, "Test".into());

        let name = source.test_connection().await.unwrap();
        assert_eq!(name, "My Server");
    }

    #[tokio::test]
    async fn plex_source_libraries_filters_unsupported_types() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/library/sections"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"MediaContainer":{"size":3,"Directory":[
                    {"key":"1","title":"Movies","type":"movie","count":500},
                    {"key":"2","title":"TV Shows","type":"show","count":100},
                    {"key":"3","title":"Music","type":"artist","count":200}
                ]}}"#,
            ))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let source = PlexSource::new(client, "Test".into());

        let libs = source.libraries().await.unwrap();
        assert_eq!(libs.len(), 2); // Music filtered out
    }

    #[tokio::test]
    async fn plex_source_library_items_converts_to_media_items() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/library/sections/1/all"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"MediaContainer":{"size":1,"Metadata":[
                    {"ratingKey":"123","title":"Dune","type":"movie","year":2021}
                ]}}"#,
            ))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let source = PlexSource::new(client, "Test".into());

        let items = source.library_items("1").await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Dune");
        assert_eq!(items[0].media_type, crate::models::media::MediaType::Movie);
    }

    #[tokio::test]
    async fn plex_source_unauthorized_maps_to_auth_error() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/"))
            .respond_with(wiremock::ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "bad-token");
        let source = PlexSource::new(client, "Test".into());

        let result = source.test_connection().await;
        assert!(matches!(result, Err(SourceError::Auth(_))));
    }

    #[test]
    fn plex_source_name_and_type() {
        let client = PlexClient::new("http://localhost:32400", "token");
        let source = PlexSource::new(client, "My Plex".into());

        assert_eq!(source.name(), "My Plex");
        assert_eq!(source.source_type(), SourceType::Plex);
    }

    #[test]
    fn plex_source_playback_url() {
        let client = PlexClient::new("http://localhost:32400", "token");
        let source = PlexSource::new(client, "Test".into());

        let url = source.playback_url("/library/parts/456/file.mkv");
        assert!(url.contains("/library/parts/456/file.mkv"));
        assert!(url.contains("X-Plex-Token=token"));
    }

    fn item_with_file(id: &str, file_path: Option<&str>) -> MediaItem {
        MediaItem {
            id: id.to_string(),
            source_type: SourceType::Plex,
            source_id: "http://localhost:32400".to_string(),
            external_id: "456".to_string(),
            media_type: crate::models::media::MediaType::Movie,
            title: "Dune".to_string(),
            year: Some(2021),
            overview: None,
            content_rating: None,
            rating: None,
            runtime_minutes: Some(155),
            poster_path: None,
            series_poster_path: None,
            backdrop_path: None,
            genres: vec![],
            parent_id: None,
            season_number: None,
            episode_number: None,
            air_date: None,
            file_path: file_path.map(String::from),
            video_resolution: None,
            hdr: None,
            added_at: "2026-01-01".to_string(),
            updated_at: "2026-01-01".to_string(),
            playback_position_ms: None,
            watched: false,
            library_section_id: None,
        }
    }

    #[test]
    fn plex_download_descriptor_has_download_url_and_part_key() {
        let client = PlexClient::new("http://localhost:32400", "token");
        let source = PlexSource::new(client, "Test".into());
        let item = item_with_file("plex:srv:456", Some("/library/parts/456/file.mkv"));

        let desc = source.download_descriptor(&item).unwrap();
        assert!(desc.url.contains("download=1"));
        assert!(desc.url.contains("X-Plex-Token=token"));
        assert_eq!(desc.part_key, "/library/parts/456/file.mkv");
        assert_eq!(desc.expected_size, None);
    }

    #[test]
    fn plex_download_descriptor_errors_without_file() {
        let client = PlexClient::new("http://localhost:32400", "token");
        let source = PlexSource::new(client, "Test".into());
        let item = item_with_file("plex:srv:456", None);

        let result = source.download_descriptor(&item);
        assert!(matches!(result, Err(SourceError::NotFound(_))));
    }

    #[test]
    fn plex_source_artwork_url() {
        let client = PlexClient::new("http://localhost:32400", "token");
        let source = PlexSource::new(client, "Test".into());

        let url = source.artwork_url("/library/metadata/123/thumb/1", 300, 450);
        assert!(url.contains("width=300"));
        assert!(url.contains("height=450"));
    }

    #[test]
    fn plex_source_is_trait_object_compatible() {
        let client = PlexClient::new("http://localhost:32400", "token");
        let source = PlexSource::new(client, "Test".into());
        let _boxed: Box<dyn MediaSource> = Box::new(source);
    }

    #[tokio::test]
    async fn plex_source_hubs_maps_and_drops_empty() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/hubs"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"MediaContainer":{"size":2,"Hub":[
                    {"title":"Recommended","hubIdentifier":"home.movies.recommended","type":"movie","Metadata":[
                        {"ratingKey":"10","title":"Dune","type":"movie","year":2021},
                        {"ratingKey":"11","title":"Better Call Saul","type":"show"}
                    ]},
                    {"title":"Drop Me","hubIdentifier":"home.music","type":"artist","Metadata":[
                        {"ratingKey":"99","title":"Some Band","type":"artist"}
                    ]}
                ]}}"#,
            ))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let source = PlexSource::new(client, "Test".into());

        let hubs = source.hubs().await.unwrap();
        // The second hub's only item is an unsupported "artist" type, so the
        // hub converts to zero items and is dropped.
        assert_eq!(hubs.len(), 1);
        assert_eq!(hubs[0].title, "Recommended");
        assert_eq!(
            hubs[0].identifier,
            Some("home.movies.recommended".to_string())
        );
        // Mixed movie + show both convert.
        assert_eq!(hubs[0].items.len(), 2);
        assert_eq!(hubs[0].items[0].title, "Dune");
    }

    #[tokio::test]
    async fn plex_source_recently_added_in_library_converts() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(
                "/library/sections/1/recentlyAdded",
            ))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"MediaContainer":{"size":2,"Metadata":[
                    {"ratingKey":"1","title":"Newest","type":"movie","year":2025},
                    {"ratingKey":"2","title":"Older","type":"movie","year":2024}
                ]}}"#,
            ))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let source = PlexSource::new(client, "Test".into());

        let items = source.recently_added_in_library("1").await.unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Newest");
    }

    #[tokio::test]
    async fn plex_source_hubs_empty_container() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/hubs"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_string(r#"{"MediaContainer":{"size":0}}"#),
            )
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let source = PlexSource::new(client, "Test".into());

        let hubs = source.hubs().await.unwrap();
        assert!(hubs.is_empty());
    }

    #[tokio::test]
    async fn plex_source_resolve_playback_returns_transcode_decision() {
        use crate::models::playback::{PlaybackDecisionKind, PlaybackRequest, QualitySelection};
        let server = wiremock::MockServer::start().await;
        let body = include_str!("../../../tests/fixtures/plex/decision_transcode.json");
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(
                "/video/:/transcode/universal/decision",
            ))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token").with_remote(true);
        let source = PlexSource::new(client, "Test".into());
        let req = PlaybackRequest {
            rating_key: "136798".into(),
            part_key: "/library/parts/950874/file.mkv".into(),
            media_index: 0,
            part_index: 0,
            quality: QualitySelection::Auto,
            force_transcode: false,
            can_direct_play_10bit: false,
            audio_stream_id: None,
            subtitle_stream_id: None,
            offset_secs: 0.0,
        };
        let decision = source.resolve_playback(&req).await.unwrap();
        assert_eq!(decision.kind, PlaybackDecisionKind::Transcode);
        assert!(decision.session.is_some());
        assert!(decision.url.contains("start.m3u8"));
        // Remote Auto applied the default cap in the (transcode) URL.
        assert!(decision.url.contains("maxVideoBitrate=8000"));
        // Output metadata comes from the server's selected Media.
        assert_eq!(decision.video_resolution.as_deref(), Some("720p"));
    }

    #[tokio::test]
    async fn plex_source_resolve_playback_decision_error_surfaces() {
        // A 500 from the decision endpoint surfaces as a loud SourceError, not a
        // silent direct-play (U7 relies on this).
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path(
                "/video/:/transcode/universal/decision",
            ))
            .respond_with(wiremock::ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let client = PlexClient::new(&server.uri(), "token");
        let source = PlexSource::new(client, "Test".into());
        let req = crate::models::playback::PlaybackRequest {
            rating_key: "1".into(),
            part_key: "/p/1".into(),
            media_index: 0,
            part_index: 0,
            quality: crate::models::playback::QualitySelection::Auto,
            force_transcode: false,
            can_direct_play_10bit: false,
            audio_stream_id: None,
            subtitle_stream_id: None,
            offset_secs: 0.0,
        };
        assert!(source.resolve_playback(&req).await.is_err());
    }

    #[tokio::test]
    async fn plex_source_libraries_caches_result() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/library/sections"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"MediaContainer":{"size":1,"Directory":[
                    {"key":"1","title":"Movies","type":"movie","count":500}
                ]}}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let source = PlexSource::new(client, "Test".into());

        let libs1 = source.libraries().await.unwrap();
        assert_eq!(libs1.len(), 1);
        assert_eq!(libs1[0].title, "Movies");

        // Second call: returns cached result, no network request
        let libs2 = source.libraries().await.unwrap();
        assert_eq!(libs2.len(), 1);
        assert_eq!(libs2[0].title, "Movies");
    }
}
