//! Source construction and the live source registry.
//!
//! The factory turns a persisted [`Source`] into a concrete `Arc<dyn
//! MediaSource>`; the registry holds every live source keyed by
//! `{source_type}:{source_id}` in **insertion order** (load-bearing: it drives
//! sidebar source order and Home's merge-by-recency). Sources are built once
//! and reused, so each source's internal `OnceLock` library cache survives
//! revalidation (this is what makes "switching the browsed server does not
//! re-sync" hold).

use std::sync::Arc;

use crate::models::media::{MediaItem, SourceType};
use crate::models::source::Source;
use crate::services::jellyfin::api::JellyfinClient;
use crate::services::jellyfin::source::JellyfinSource;
use crate::services::media_source::MediaSource;
use crate::services::plex::api::PlexClient;
use crate::services::plex::source::PlexSource;

/// Build a live `MediaSource` from a persisted source row. Returns `None` for
/// source types that have no networked backend (e.g. `Local`).
pub fn build_source(source: &Source) -> Option<Arc<dyn MediaSource>> {
    match source.source_type {
        SourceType::Plex => {
            let client = PlexClient::new(&source.config.url, &source.config.token);
            Some(Arc::new(PlexSource::new(client, source.name.clone())))
        }
        SourceType::Jellyfin => {
            let user_id = source.config.user_id.clone().unwrap_or_default();
            let device_id = crate::services::jellyfin::auth::device_id(&crate::config::data_dir());
            let client = JellyfinClient::new(
                &source.config.url,
                &source.config.token,
                &user_id,
                &device_id,
            );
            Some(Arc::new(JellyfinSource::new(client, source.name.clone())))
        }
        SourceType::Local => None,
    }
}

/// One live source plus its identity.
pub struct RegistrySource {
    pub source_type: SourceType,
    /// The server URL — the `source_id` half of the registry key and of every
    /// `MediaItem` this source owns.
    pub source_id: String,
    pub source: Arc<dyn MediaSource>,
}

/// Insertion-ordered registry of live sources.
///
/// Ordering is load-bearing (sidebar source order, Home merge), so this is a
/// `Vec`, never a `HashMap`. Sources are looked up by `(source_type,
/// source_id)` — the same pair a `MediaItem` carries — so detail/play/scrobble
/// resolve the *owning* source per item, never the currently-browsed one.
#[derive(Default)]
pub struct SourceRegistry {
    entries: Vec<RegistrySource>,
}

impl SourceRegistry {
    /// The registry key for a source: `{source_type}:{source_id}`.
    #[allow(dead_code)] // consumed by U8/U9 (sidebar grouping, Home merge)
    pub fn key(source_type: SourceType, source_id: &str) -> String {
        format!("{}:{}", source_type.as_str(), source_id)
    }

    /// Insert or replace a source, preserving insertion order. Re-registering an
    /// existing `(type, id)` swaps the `Arc` in place without moving it, so
    /// order is stable across revalidation.
    pub fn register(
        &mut self,
        source_type: SourceType,
        source_id: String,
        source: Arc<dyn MediaSource>,
    ) {
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|e| e.source_type == source_type && e.source_id == source_id)
        {
            existing.source = source;
        } else {
            self.entries.push(RegistrySource {
                source_type,
                source_id,
                source,
            });
        }
    }

    /// Look up a source by its identity.
    pub fn get(&self, source_type: SourceType, source_id: &str) -> Option<Arc<dyn MediaSource>> {
        self.entries
            .iter()
            .find(|e| e.source_type == source_type && e.source_id == source_id)
            .map(|e| e.source.clone())
    }

    /// Resolve the source that owns a media item, from the item's own
    /// `source_type` + `source_id`. This is mandatory for merged Home, where a
    /// user can open an item whose source isn't the browsed one.
    pub fn for_item(&self, item: &MediaItem) -> Option<Arc<dyn MediaSource>> {
        self.get(item.source_type, &item.source_id)
    }

    /// Remove a source by identity. Used by the explicit remove-source path.
    #[allow(dead_code)] // consumed by U8 (remove-source action)
    pub fn remove(&mut self, source_type: SourceType, source_id: &str) {
        self.entries
            .retain(|e| !(e.source_type == source_type && e.source_id == source_id));
    }

    /// Iterate live sources in insertion order.
    #[allow(dead_code)] // consumed by U8/U9 (sidebar grouping, Home fan-out)
    pub fn iter(&self) -> impl Iterator<Item = &RegistrySource> {
        self.entries.iter()
    }

    #[allow(dead_code)] // consumed by U6c (startup multi-source load)
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::media::{MediaType, SourceType};
    use crate::services::media_source::{MediaSource, SourceError};
    use async_trait::async_trait;

    // A trivial fake source for registry tests — identity only.
    struct FakeSource {
        name: String,
        kind: SourceType,
    }

    #[async_trait]
    impl MediaSource for FakeSource {
        fn name(&self) -> &str {
            &self.name
        }
        fn source_type(&self) -> SourceType {
            self.kind
        }
        async fn test_connection(&self) -> Result<String, SourceError> {
            Ok(self.name.clone())
        }
        async fn libraries(
            &self,
        ) -> Result<Vec<crate::models::library::LibrarySection>, SourceError> {
            Ok(vec![])
        }
        async fn library_items(&self, _: &str) -> Result<Vec<MediaItem>, SourceError> {
            Ok(vec![])
        }
        async fn metadata(
            &self,
            _: &str,
        ) -> Result<crate::models::detail::MediaDetail, SourceError> {
            Err(SourceError::NotFound("x".into()))
        }
        async fn children(&self, _: &str) -> Result<Vec<MediaItem>, SourceError> {
            Ok(vec![])
        }
        fn playback_url(&self, p: &str) -> String {
            p.to_string()
        }
        fn artwork_url(&self, p: &str, _: u32, _: u32) -> String {
            p.to_string()
        }
    }

    fn fake(name: &str, kind: SourceType) -> Arc<dyn MediaSource> {
        Arc::new(FakeSource {
            name: name.to_string(),
            kind,
        })
    }

    fn item(source_type: SourceType, source_id: &str, external_id: &str) -> MediaItem {
        MediaItem {
            id: MediaItem::make_id(source_type, source_id, external_id),
            source_type,
            source_id: source_id.to_string(),
            external_id: external_id.to_string(),
            media_type: MediaType::Movie,
            title: "T".into(),
            year: None,
            overview: None,
            content_rating: None,
            rating: None,
            runtime_minutes: None,
            poster_path: None,
            series_poster_path: None,
            backdrop_path: None,
            genres: vec![],
            parent_id: None,
            season_number: None,
            episode_number: None,
            air_date: None,
            file_path: None,
            video_resolution: None,
            hdr: None,
            added_at: String::new(),
            updated_at: String::new(),
            playback_position_ms: None,
            watched: false,
            library_section_id: None,
        }
    }

    #[test]
    fn registry_holds_multiple_sources() {
        let mut reg = SourceRegistry::default();
        reg.register(SourceType::Plex, "p".into(), fake("Plex", SourceType::Plex));
        reg.register(
            SourceType::Jellyfin,
            "j".into(),
            fake("Jelly", SourceType::Jellyfin),
        );
        assert_eq!(reg.len(), 2);
        assert_eq!(reg.get(SourceType::Plex, "p").unwrap().name(), "Plex");
        assert_eq!(reg.get(SourceType::Jellyfin, "j").unwrap().name(), "Jelly");
    }

    #[test]
    fn source_lookup_by_item_id() {
        let mut reg = SourceRegistry::default();
        reg.register(
            SourceType::Jellyfin,
            "https://jf".into(),
            fake("Jelly", SourceType::Jellyfin),
        );
        let it = item(SourceType::Jellyfin, "https://jf", "abc");
        assert_eq!(reg.for_item(&it).unwrap().name(), "Jelly");
    }

    #[test]
    fn show_detail_for_non_browsed_source_resolves_owning_source() {
        // Browsing Plex, but opening a Jellyfin item resolves the Jellyfin
        // source — per-item resolution, not the browsed pointer.
        let mut reg = SourceRegistry::default();
        reg.register(SourceType::Plex, "p".into(), fake("Plex", SourceType::Plex));
        reg.register(
            SourceType::Jellyfin,
            "j".into(),
            fake("Jelly", SourceType::Jellyfin),
        );
        let jellyfin_item = item(SourceType::Jellyfin, "j", "ep1");
        assert_eq!(reg.for_item(&jellyfin_item).unwrap().name(), "Jelly");
    }

    #[test]
    fn scrobble_dispatches_to_now_playing_items_source() {
        // An item whose source is not the first/browsed one still resolves to
        // its own owning source.
        let mut reg = SourceRegistry::default();
        reg.register(SourceType::Plex, "p".into(), fake("Plex", SourceType::Plex));
        reg.register(
            SourceType::Jellyfin,
            "j".into(),
            fake("Jelly", SourceType::Jellyfin),
        );
        let it = item(SourceType::Jellyfin, "j", "m1");
        let resolved = reg.for_item(&it).unwrap();
        assert_eq!(resolved.source_type(), SourceType::Jellyfin);
    }

    #[test]
    fn registry_iteration_order_is_insertion_order() {
        let mut reg = SourceRegistry::default();
        reg.register(SourceType::Plex, "a".into(), fake("A", SourceType::Plex));
        reg.register(
            SourceType::Jellyfin,
            "b".into(),
            fake("B", SourceType::Jellyfin),
        );
        reg.register(SourceType::Plex, "c".into(), fake("C", SourceType::Plex));
        let names: Vec<&str> = reg.iter().map(|e| e.source.name()).collect();
        assert_eq!(names, vec!["A", "B", "C"]);
        // Re-registering keeps position.
        reg.register(SourceType::Plex, "a".into(), fake("A2", SourceType::Plex));
        let names: Vec<&str> = reg.iter().map(|e| e.source.name()).collect();
        assert_eq!(names, vec!["A2", "B", "C"]);
    }

    #[test]
    fn remove_drops_only_the_named_source() {
        let mut reg = SourceRegistry::default();
        reg.register(SourceType::Plex, "p".into(), fake("Plex", SourceType::Plex));
        reg.register(
            SourceType::Jellyfin,
            "j".into(),
            fake("Jelly", SourceType::Jellyfin),
        );
        reg.remove(SourceType::Plex, "p");
        assert_eq!(reg.len(), 1);
        assert!(reg.get(SourceType::Plex, "p").is_none());
        assert!(reg.get(SourceType::Jellyfin, "j").is_some());
    }

    #[test]
    fn key_format() {
        assert_eq!(
            SourceRegistry::key(SourceType::Jellyfin, "https://x"),
            "jellyfin:https://x"
        );
    }
}
