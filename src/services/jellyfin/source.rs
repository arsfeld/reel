use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use async_trait::async_trait;

use crate::models::{
    detail::MediaDetail,
    hub::MediaHub,
    library::LibrarySection,
    media::{MediaItem, SourceType},
    playback::{PlaybackDecision, PlaybackRequest},
};
use crate::player::SkipMarkers;
use crate::services::media_source::{DownloadDescriptor, MediaSource, SourceError};

use super::api::JellyfinClient;
use super::convert;
use super::error::JellyfinError;
use super::models::Ticks;

/// Process-global counter used to mint unique `PlaySessionId`s. Jellyfin only
/// requires the id to be unique per playback, so a monotonic counter combined
/// with the item id is sufficient (no uuid/rand dependency needed here).
static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// `MediaSource` implementation backed by a Jellyfin server.
pub struct JellyfinSource {
    client: JellyfinClient,
    name: String,
    /// Cached library sections. Fetched once on first call; reused thereafter.
    libraries_cache: OnceLock<Vec<LibrarySection>>,
    /// Tracks the active `PlaySessionId` per item, so `report_progress` can
    /// emit `/Sessions/Playing` (start) before the first `/Progress` and
    /// `/Sessions/Playing/Stopped` to close it. Keyed by item id.
    play_sessions: Mutex<HashMap<String, String>>,
    /// `PlaySessionId` from a transcode/direct-stream decision, keyed by item id.
    /// When present, the progress path reuses this id and reports
    /// `PlayMethod=Transcode`, so keepalive, progress, and stop all reference the
    /// one session the server associated with the encoder (KTD4b).
    transcode_sessions: Mutex<HashMap<String, String>>,
}

impl JellyfinSource {
    pub fn new(client: JellyfinClient, name: String) -> Self {
        Self {
            client,
            name,
            libraries_cache: OnceLock::new(),
            play_sessions: Mutex::new(HashMap::new()),
            transcode_sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Append a hub from a (possibly failed) fetch, converting items and
    /// dropping the row when the fetch errored or yielded nothing. Keeps `hubs()`
    /// resilient to one endpoint being unavailable on older servers.
    fn push_hub(
        &self,
        hubs: &mut Vec<MediaHub>,
        title: &str,
        identifier: &str,
        fetched: Result<Vec<super::models::BaseItemDto>, JellyfinError>,
    ) {
        let items = self.convert_items(&fetched.unwrap_or_default(), None);
        if !items.is_empty() {
            hubs.push(MediaHub {
                title: title.to_string(),
                identifier: Some(identifier.to_string()),
                items,
            });
        }
    }

    /// The `PlayMethod` to report for an item: `Transcode` while a transcode
    /// decision is active for it, else `DirectPlay`.
    fn play_method_for(&self, rating_key: &str) -> &'static str {
        if self
            .transcode_sessions
            .lock()
            .unwrap()
            .contains_key(rating_key)
        {
            "Transcode"
        } else {
            "DirectPlay"
        }
    }

    /// Convert a slice of Jellyfin items to `MediaItem`s, dropping unsupported
    /// types. `tag_library` optionally stamps each item's `library_section_id`.
    fn convert_items(
        &self,
        dtos: &[super::models::BaseItemDto],
        tag_library: Option<&str>,
    ) -> Vec<MediaItem> {
        let base = self.client.base_url();
        dtos.iter()
            .filter_map(|d| convert::base_item_to_media_item(d, base))
            .map(|mut item| {
                if let Some(lib) = tag_library {
                    item.library_section_id = Some(lib.to_string());
                }
                item
            })
            .collect()
    }
}

impl From<JellyfinError> for SourceError {
    fn from(e: JellyfinError) -> Self {
        match e {
            JellyfinError::Unauthorized => {
                SourceError::Auth("Invalid or expired Jellyfin token".into())
            }
            JellyfinError::NotFound(msg) => SourceError::NotFound(msg),
            JellyfinError::Http(e) => SourceError::Connection(e.to_string()),
            JellyfinError::Deserialize(e) => SourceError::Other(format!("Parse error: {e}")),
            JellyfinError::Server { status, message } => {
                SourceError::Other(format!("Server error {status}: {message}"))
            }
        }
    }
}

#[async_trait]
impl MediaSource for JellyfinSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn source_type(&self) -> SourceType {
        SourceType::Jellyfin
    }

    async fn test_connection(&self) -> Result<String, SourceError> {
        // Jellyfin has no simple friendly-name ping we use; listing the user's
        // views proves both reachability and a valid token.
        self.client.user_views().await?;
        Ok(self.name.clone())
    }

    async fn libraries(&self) -> Result<Vec<LibrarySection>, SourceError> {
        if let Some(cached) = self.libraries_cache.get() {
            return Ok(cached.clone());
        }
        let views = self.client.user_views().await?;
        let sections: Vec<LibrarySection> = views
            .iter()
            .filter_map(convert::user_view_to_section)
            .collect();
        let _ = self.libraries_cache.set(sections.clone());
        Ok(sections)
    }

    async fn library_items(&self, library_key: &str) -> Result<Vec<MediaItem>, SourceError> {
        let dtos = self
            .client
            .items(Some(library_key), Some("Movie,Series"), true)
            .await?;
        Ok(self.convert_items(&dtos, Some(library_key)))
    }

    async fn metadata(&self, rating_key: &str) -> Result<MediaDetail, SourceError> {
        let dto = self.client.item(rating_key).await?;
        let base = self.client.base_url();
        convert::base_item_to_media_detail(&dto, base)
            .ok_or_else(|| SourceError::Other("Failed to convert metadata".into()))
    }

    async fn children(&self, rating_key: &str) -> Result<Vec<MediaItem>, SourceError> {
        // Jellyfin needs to know whether the item is a Series or a Season to
        // choose the right child endpoint, so fetch it first.
        let dto = self.client.item(rating_key).await?;
        let dtos = match dto.type_.as_deref() {
            Some("Series") => self.client.seasons(rating_key).await?,
            Some("Season") => {
                let series_id = dto.series_id.as_deref().unwrap_or("");
                self.client.episodes(series_id, rating_key).await?
            }
            _ => return Ok(vec![]),
        };
        Ok(self.convert_items(&dtos, None))
    }

    fn playback_url(&self, part_key: &str) -> String {
        // `part_key` is the composite "{item_id}|{media_source_id}" built in
        // convert.rs. If no '|' is present, use the whole string as both.
        let (item_id, media_source_id) = match part_key.split_once('|') {
            Some((item, src)) => (item, src),
            None => (part_key, part_key),
        };
        self.client.stream_url(item_id, media_source_id)
    }

    fn download_descriptor(&self, item: &MediaItem) -> Result<DownloadDescriptor, SourceError> {
        // `file_path` is the composite "{item_id}|{media_source_id}" built in
        // convert.rs. The URL is rebuilt from it via playback_url on each resume
        // (stream?static=true is byte-rangeable), so only the part key persists.
        let part_key = item
            .file_path
            .as_deref()
            .filter(|p| !p.is_empty())
            .ok_or_else(|| {
                SourceError::NotFound(format!("No downloadable file for item {}", item.id))
            })?;
        Ok(DownloadDescriptor {
            url: self.playback_url(part_key),
            part_key: part_key.to_string(),
            expected_size: None,
        })
    }

    async fn resolve_playback(
        &self,
        req: &PlaybackRequest,
    ) -> Result<PlaybackDecision, SourceError> {
        let decision = self.client.resolve_decision(req).await?;
        // Record (or clear) the transcode PlaySessionId so the progress path
        // reuses it and reports PlayMethod=Transcode (KTD4b). A re-resolve that
        // lands on direct-play clears any prior transcode session for the item.
        match (&decision.session, decision.kind.is_transcode_like()) {
            (Some(psid), true) => {
                self.transcode_sessions
                    .lock()
                    .unwrap()
                    .insert(req.rating_key.clone(), psid.clone());
            }
            _ => {
                self.transcode_sessions
                    .lock()
                    .unwrap()
                    .remove(&req.rating_key);
            }
        }
        Ok(decision)
    }

    async fn ping_transcode(&self, session: &str) -> Result<(), SourceError> {
        Ok(self.client.ping_session(session).await?)
    }

    async fn stop_transcode(&self, session: &str) -> Result<(), SourceError> {
        // Kill the encoder. The /Sessions/Playing/Stopped accounting (which needs
        // the item id) flows through report_progress("stopped"), now unified onto
        // the same PlaySessionId.
        self.client.stop_active_encoding(session).await?;
        self.transcode_sessions
            .lock()
            .unwrap()
            .retain(|_, v| v != session);
        Ok(())
    }

    fn artwork_url(&self, path: &str, width: u32, height: u32) -> String {
        // `path` is the descriptor "{item_id}/{image_type}/{tag}" built in
        // convert.rs. Parse defensively; fall back to a Primary image.
        let mut parts = path.splitn(3, '/');
        let item_id = parts.next().unwrap_or("");
        let image_type = parts.next().unwrap_or("Primary");
        let tag = parts.next();
        self.client
            .image_url(item_id, image_type, tag, Some(width), Some(height))
    }

    async fn collections(&self, library_key: &str) -> Result<Vec<MediaItem>, SourceError> {
        // Box sets exposed in the library. A library with none yields an empty
        // Vec (NOT an error), satisfying graceful degradation (R13).
        let dtos = self
            .client
            .items(Some(library_key), Some("BoxSet"), true)
            .await?;
        Ok(self.convert_items(&dtos, None))
    }

    async fn collection_items(&self, collection_key: &str) -> Result<Vec<MediaItem>, SourceError> {
        let dtos = self.client.items(Some(collection_key), None, false).await?;
        Ok(self.convert_items(&dtos, None))
    }

    async fn recently_added(&self) -> Result<Vec<MediaItem>, SourceError> {
        let dtos = self.client.latest(None).await?;
        Ok(self.convert_items(&dtos, None))
    }

    async fn recently_added_in_library(
        &self,
        library_key: &str,
    ) -> Result<Vec<MediaItem>, SourceError> {
        let dtos = self.client.latest(Some(library_key)).await?;
        Ok(self.convert_items(&dtos, Some(library_key)))
    }

    async fn continue_watching(&self) -> Result<Vec<MediaItem>, SourceError> {
        let dtos = self.client.resume_items().await?;
        Ok(self.convert_items(&dtos, None))
    }

    async fn hubs(&self) -> Result<Vec<MediaHub>, SourceError> {
        // Per-server discovery shelves: Latest, Next Up, and Suggested. The
        // fetches are independent, so run them concurrently to save round-trips
        // on the Home load path, and treat any single failure as an empty row
        // (an older server lacking /Items/Suggestions must not blank the others).
        // Empty hubs are dropped so the home view never renders an empty shelf,
        // and the home view further drops hubs that duplicate core shelves
        // (Continue Watching / Recently Added) by identifier.
        let (latest, next_up, suggestions) = tokio::join!(
            self.client.latest(None),
            self.client.next_up(),
            self.client.suggestions(),
        );
        let mut hubs = Vec::new();
        self.push_hub(&mut hubs, "Latest", "jellyfin.latest", latest);
        self.push_hub(&mut hubs, "Next Up", "jellyfin.nextup", next_up);
        self.push_hub(&mut hubs, "Suggested", "jellyfin.suggestions", suggestions);
        Ok(hubs)
    }

    async fn report_progress(
        &self,
        rating_key: &str,
        state: &str,
        time_ms: i64,
        _duration_ms: i64,
    ) -> Result<(), SourceError> {
        let position_ticks = Ticks::from_ms(time_ms).0;

        let play_method = self.play_method_for(rating_key);

        if state == "stopped" {
            // Take the session id out of the map, dropping the guard before any
            // await. Only report stopped if we actually started a session. Also
            // clear the transcode session so subsequent playback reverts to
            // direct-play accounting.
            let session_id = {
                let mut sessions = self.play_sessions.lock().unwrap();
                sessions.remove(rating_key)
            };
            self.transcode_sessions.lock().unwrap().remove(rating_key);
            if let Some(session_id) = session_id {
                self.client
                    .report_stopped(rating_key, &session_id, position_ticks)
                    .await?;
            }
            return Ok(());
        }

        // playing / paused: ensure a session exists, emitting a START on the
        // first report for this item. Read the existing id under the lock, then
        // drop the guard before awaiting (never hold a std Mutex across .await).
        let existing = {
            let sessions = self.play_sessions.lock().unwrap();
            sessions.get(rating_key).cloned()
        };
        let is_paused = state == "paused";

        match existing {
            Some(session_id) => {
                self.client
                    .report_progress(rating_key, &session_id, position_ticks, is_paused, play_method)
                    .await?;
            }
            None => {
                // New playback: open the session with a START. For a transcode,
                // reuse the decision's PlaySessionId (KTD4b) so the server ties
                // progress/keepalive/stop to the running encoder; otherwise mint
                // a counter id. Only record the id AFTER the server accepts the
                // start — a failed start would leave a phantom session whose
                // later /Progress reports the server silently drops.
                let session_id = self
                    .transcode_sessions
                    .lock()
                    .unwrap()
                    .get(rating_key)
                    .cloned()
                    .unwrap_or_else(|| {
                        let counter = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
                        format!("{rating_key}-{counter}")
                    });
                self.client
                    .report_playing(rating_key, &session_id, position_ticks, play_method)
                    .await?;
                self.play_sessions
                    .lock()
                    .unwrap()
                    .insert(rating_key.to_string(), session_id.clone());
                self.client
                    .report_progress(rating_key, &session_id, position_ticks, is_paused, play_method)
                    .await?;
            }
        }
        Ok(())
    }

    async fn scrobble(&self, rating_key: &str) -> Result<(), SourceError> {
        Ok(self.client.mark_played(rating_key).await?)
    }

    async fn unscrobble(&self, rating_key: &str) -> Result<(), SourceError> {
        Ok(self.client.mark_unplayed(rating_key).await?)
    }

    async fn skip_markers(
        &self,
        rating_key: &str,
        _duration_secs: f64,
    ) -> Result<SkipMarkers, SourceError> {
        let segs = self.client.media_segments(rating_key).await?;
        convert::media_segments_to_skip_markers(&segs).ok_or(SourceError::NotSupported(
            "No intro/credits markers found".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_source() -> JellyfinSource {
        let client = JellyfinClient::new("http://localhost:8096", "token", "user1", "device1");
        JellyfinSource::new(client, "My Jellyfin".into())
    }

    #[test]
    fn jellyfin_source_is_trait_object_compatible() {
        let _boxed: Box<dyn MediaSource> = Box::new(test_source());
    }

    #[test]
    fn source_type_is_jellyfin() {
        let source = test_source();
        assert_eq!(source.source_type(), SourceType::Jellyfin);
        assert_eq!(source.name(), "My Jellyfin");
    }

    #[test]
    fn playback_url_splits_composite() {
        let source = test_source();
        let url = source.playback_url("item9|src5");
        assert!(url.contains("/Videos/item9/stream"));
        assert!(url.contains("mediaSourceId=src5"));
    }

    #[test]
    fn playback_url_without_pipe_uses_whole_as_both() {
        let source = test_source();
        let url = source.playback_url("item9");
        assert!(url.contains("/Videos/item9/stream"));
        assert!(url.contains("mediaSourceId=item9"));
    }

    #[test]
    fn artwork_url_parses_descriptor() {
        let source = test_source();
        let url = source.artwork_url("item9/Primary/tagX", 300, 450);
        assert!(url.contains("/Items/item9/Images/Primary"));
        assert!(url.contains("tag=tagX"));
        assert!(url.contains("fillWidth=300"));
        assert!(url.contains("fillHeight=450"));
    }

    #[test]
    fn artwork_url_best_effort_for_bare_id() {
        let source = test_source();
        let url = source.artwork_url("item9", 100, 100);
        assert!(url.contains("/Items/item9/Images/Primary"));
    }

    #[test]
    fn jellyfin_error_unauthorized_maps_to_auth() {
        let err: SourceError = JellyfinError::Unauthorized.into();
        assert!(matches!(err, SourceError::Auth(_)));
    }

    #[test]
    fn jellyfin_error_not_found_maps_to_not_found() {
        let err: SourceError = JellyfinError::NotFound("gone".into()).into();
        assert!(matches!(err, SourceError::NotFound(_)));
    }

    #[test]
    fn jellyfin_error_server_maps_to_other() {
        let err: SourceError = JellyfinError::Server {
            status: 500,
            message: "boom".into(),
        }
        .into();
        assert!(matches!(err, SourceError::Other(_)));
    }

    // --- U4: trait method wiring + PlaySessionId unification ---

    use crate::models::playback::{PlaybackRequest, QualitySelection};
    use wiremock::matchers::{body_partial_json, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn wm_source(uri: &str) -> JellyfinSource {
        let client = JellyfinClient::new(uri, "secret-token", "user1", "device1");
        JellyfinSource::new(client, "JF".into())
    }

    fn play_req() -> PlaybackRequest {
        PlaybackRequest {
            rating_key: "item9".into(),
            part_key: "item9|src1".into(),
            media_index: 0,
            part_index: 0,
            quality: QualitySelection::Auto,
            force_transcode: false,
            audio_stream_id: None,
            subtitle_stream_id: None,
            offset_secs: 0.0,
        }
    }

    fn transcode_playback_info() -> serde_json::Value {
        serde_json::json!({
            "PlaySessionId": "ps-trans",
            "MediaSources": [{
                "Id": "src1",
                "SupportsDirectPlay": false,
                "SupportsDirectStream": false,
                "SupportsTranscoding": true,
                "TranscodingUrl": "/videos/item9/master.m3u8?api_key=secret-token",
                "MediaStreams": [{"Index": 1, "Type": "Audio", "Codec": "aac"}]
            }]
        })
    }

    #[tokio::test]
    async fn ping_transcode_pings_session() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/Sessions/Playing/Ping"))
            .and(query_param("playSessionId", "ps-trans"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let src = wm_source(&server.uri());
        assert!(src.ping_transcode("ps-trans").await.is_ok());
    }

    #[tokio::test]
    async fn stop_transcode_deletes_active_encoding() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/Videos/ActiveEncodings"))
            .and(query_param("deviceId", "device1"))
            .and(query_param("playSessionId", "ps-trans"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let src = wm_source(&server.uri());
        assert!(src.stop_transcode("ps-trans").await.is_ok());
    }

    #[tokio::test]
    async fn stop_transcode_treats_404_as_success() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/Videos/ActiveEncodings"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let src = wm_source(&server.uri());
        assert!(src.stop_transcode("ps-gone").await.is_ok());
    }

    fn movie_array(id: &str) -> serde_json::Value {
        serde_json::json!([{"Id": id, "Type": "Movie", "Name": id}])
    }

    fn movie_query(id: &str) -> serde_json::Value {
        serde_json::json!({"Items": [{"Id": id, "Type": "Movie", "Name": id}]})
    }

    #[tokio::test]
    async fn hubs_includes_latest_next_up_and_suggested() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/Items/Latest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(movie_array("lat1")))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/Shows/NextUp"))
            .respond_with(ResponseTemplate::new(200).set_body_json(movie_query("nu1")))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/Items/Suggestions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(movie_query("sug1")))
            .mount(&server)
            .await;
        let src = wm_source(&server.uri());
        let hubs = src.hubs().await.unwrap();
        let ids: Vec<&str> = hubs.iter().filter_map(|h| h.identifier.as_deref()).collect();
        assert!(ids.contains(&"jellyfin.latest"));
        assert!(ids.contains(&"jellyfin.nextup"));
        assert!(ids.contains(&"jellyfin.suggestions"));
    }

    #[tokio::test]
    async fn hubs_resilient_when_suggestions_unavailable() {
        // An older server returns 404 for /Items/Suggestions; the other rows
        // must still appear (the failing row is dropped, not propagated).
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/Items/Latest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(movie_array("lat1")))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/Shows/NextUp"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"Items": []})))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/Items/Suggestions"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let src = wm_source(&server.uri());
        let hubs = src.hubs().await.unwrap();
        let ids: Vec<&str> = hubs.iter().filter_map(|h| h.identifier.as_deref()).collect();
        assert_eq!(ids, vec!["jellyfin.latest"]); // next_up empty, suggestions 404 -> both dropped
    }

    #[test]
    fn download_descriptor_builds_static_url_from_composite_key() {
        let src = wm_source("http://h:8096");
        let dto: super::super::models::BaseItemDto = serde_json::from_str(
            r#"{"Id":"item9","Type":"Movie","Name":"M","MediaSources":[{"Id":"src1"}]}"#,
        )
        .unwrap();
        let item = convert::base_item_to_media_item(&dto, "http://h:8096").unwrap();
        let desc = src.download_descriptor(&item).unwrap();
        assert_eq!(desc.part_key, "item9|src1");
        assert!(desc.url.contains("/Videos/item9/stream?static=true"));
        assert!(desc.url.contains("mediaSourceId=src1"));
        assert!(desc.expected_size.is_none());
    }

    #[test]
    fn download_descriptor_errors_when_no_file_path() {
        // A Series with no media source has no downloadable file (file_path None).
        let src = wm_source("http://h:8096");
        let dto: super::super::models::BaseItemDto =
            serde_json::from_str(r#"{"Id":"series1","Type":"Series","Name":"S"}"#).unwrap();
        let item = convert::base_item_to_media_item(&dto, "http://h:8096").unwrap();
        assert!(matches!(
            src.download_descriptor(&item),
            Err(SourceError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn transcode_progress_reuses_decision_session_and_play_method() {
        // After a transcode resolve, the progress START/Progress must carry the
        // decision's PlaySessionId and PlayMethod=Transcode (KTD4b), not a
        // counter-minted id with PlayMethod=DirectPlay.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/Items/item9/PlaybackInfo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(transcode_playback_info()))
            .mount(&server)
            .await;
        // START and Progress must reference ps-trans + Transcode.
        Mock::given(method("POST"))
            .and(path("/Sessions/Playing"))
            .and(body_partial_json(
                serde_json::json!({"PlaySessionId": "ps-trans", "PlayMethod": "Transcode"}),
            ))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/Sessions/Playing/Progress"))
            .and(body_partial_json(
                serde_json::json!({"PlaySessionId": "ps-trans", "PlayMethod": "Transcode"}),
            ))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let src = wm_source(&server.uri());
        let decision = src.resolve_playback(&play_req()).await.unwrap();
        assert_eq!(decision.session.as_deref(), Some("ps-trans"));
        // If the START/Progress bodies didn't match (wrong session or method),
        // these calls 404 and report_progress returns Err.
        src.report_progress("item9", "playing", 5_000, 600_000)
            .await
            .unwrap();
    }
}
