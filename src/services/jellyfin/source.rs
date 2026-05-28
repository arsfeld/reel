use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use async_trait::async_trait;

use crate::models::{
    detail::MediaDetail,
    hub::MediaHub,
    library::LibrarySection,
    media::{MediaItem, SourceType},
};
use crate::player::SkipMarkers;
use crate::services::media_source::{MediaSource, SourceError};

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
}

impl JellyfinSource {
    pub fn new(client: JellyfinClient, name: String) -> Self {
        Self {
            client,
            name,
            libraries_cache: OnceLock::new(),
            play_sessions: Mutex::new(HashMap::new()),
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
        // Minimal viable per-server hubs: Latest + Next Up. Empty hubs are
        // dropped so the home view never renders an empty shelf (mirror Plex).
        let mut hubs = Vec::new();

        let latest = self.client.latest(None).await?;
        let latest_items = self.convert_items(&latest, None);
        if !latest_items.is_empty() {
            hubs.push(MediaHub {
                title: "Latest".to_string(),
                identifier: Some("jellyfin.latest".to_string()),
                items: latest_items,
            });
        }

        let next_up = self.client.next_up().await?;
        let next_up_items = self.convert_items(&next_up, None);
        if !next_up_items.is_empty() {
            hubs.push(MediaHub {
                title: "Next Up".to_string(),
                identifier: Some("jellyfin.nextup".to_string()),
                items: next_up_items,
            });
        }

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

        if state == "stopped" {
            // Take the session id out of the map, dropping the guard before any
            // await. Only report stopped if we actually started a session.
            let session_id = {
                let mut sessions = self.play_sessions.lock().unwrap();
                sessions.remove(rating_key)
            };
            if let Some(session_id) = session_id {
                self.client
                    .report_stopped(rating_key, &session_id, position_ticks)
                    .await?;
            }
            return Ok(());
        }

        // playing / paused: ensure a session exists, emitting a START on the
        // first report for this item. Compute the id under the lock, then drop
        // the guard before awaiting (never hold a std Mutex across .await).
        let (session_id, is_new) = {
            let mut sessions = self.play_sessions.lock().unwrap();
            match sessions.get(rating_key) {
                Some(existing) => (existing.clone(), false),
                None => {
                    let counter = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
                    let new_id = format!("{rating_key}-{counter}");
                    sessions.insert(rating_key.to_string(), new_id.clone());
                    (new_id, true)
                }
            }
        };

        let is_paused = state == "paused";
        if is_new {
            self.client
                .report_playing(rating_key, &session_id, position_ticks)
                .await?;
        }
        self.client
            .report_progress(rating_key, &session_id, position_ticks, is_paused)
            .await?;
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
}
