//! In-memory, session-scoped content cache for already-fetched library and Home
//! content.
//!
//! Pure Rust — no GTK, no Relm4 — so it lives in `services/` and is fully
//! unit-tested. The cache lets navigation render a previously-visited view
//! instantly instead of re-fetching behind a loading page. Library entries are
//! LRU-bounded by entry count; the Home slot is held separately and is exempt
//! from eviction (its instant render is the headline benefit, and browsing many
//! libraries must never evict it).
//!
//! Entries store raw `Vec<MediaItem>`, not derived watch maps — watch-state
//! convergence falls out of re-running `build_effective_watch_map` on a refetched
//! list, so the cache never reasons about watch state itself.
//!
//! Keys:
//! - library entry: `{source_type}:{source_id}:{section_key}` (per section, not
//!   per media type, so "Movies" and "4K Movies" never collide).
//! - Home slot: a `source_set_key` — the lexically sorted, joined set of
//!   `{source_type}:{source_id}` registry keys, so adding/removing a source
//!   naturally misses the cached Home.

use crate::models::hub::MediaHub;
use crate::models::media::{MediaItem, SourceType};
use std::collections::HashMap;

/// Default number of library entries retained before LRU eviction. Tiny on
/// purpose — a handful of recently-browsed libraries covers normal navigation,
/// and entry-count bounding keeps the implementation simple. Not user-facing.
pub const DEFAULT_LIBRARY_CAPACITY: usize = 12;

/// A cached library section: the raw fetched items plus the epoch the entry was
/// last written under. The epoch is used by background revalidation to drop a
/// refetch result whose entry was superseded or evicted while in flight.
#[derive(Debug, Clone)]
pub struct LibraryEntry {
    pub items: Vec<MediaItem>,
    pub epoch: u64,
}

/// A cached, assembled Home payload. Mirrors the shape the Home view builds from
/// (`HomeViewCmd::HomeData`) so it can be re-fed for an instant render. Carries
/// the `source_set_key` it was built for so a stale set is a natural miss.
#[derive(Debug, Clone)]
pub struct CachedHome {
    pub source_set_key: String,
    pub continue_watching: Vec<(MediaItem, String)>,
    pub recently_added: Vec<(String, Vec<MediaItem>)>,
    pub collections: Vec<MediaItem>,
    pub hubs: Vec<MediaHub>,
    pub epoch: u64,
}

/// Session-scoped content cache. Owned by `App`; all access happens on the GTK
/// main loop, so no interior mutability or locking is needed.
#[derive(Debug)]
pub struct SessionContentCache {
    library: HashMap<String, LibraryEntry>,
    /// LRU recency, least-recently-used first, most-recently-used last.
    recency: Vec<String>,
    capacity: usize,
    home: Option<CachedHome>,
    /// Monotonic source for entry epochs; every write takes the next value, so a
    /// re-write of the same key always gets a strictly greater epoch.
    next_epoch: u64,
    /// Bumped whenever the source set changes (add/remove). Background refetches
    /// stamp the value seen at dispatch and drop on mismatch, so a result built
    /// against a since-removed source is never applied.
    source_set_epoch: u64,
}

impl Default for SessionContentCache {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_LIBRARY_CAPACITY)
    }
}

impl SessionContentCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            library: HashMap::new(),
            recency: Vec::new(),
            capacity: capacity.max(1),
            home: None,
            next_epoch: 1,
            source_set_epoch: 1,
        }
    }

    /// Composite library cache key. Keyed on section, not media type.
    pub fn library_key(source_type: SourceType, source_id: &str, section_key: &str) -> String {
        format!("{}:{}:{}", source_type.as_str(), source_id, section_key)
    }

    /// Stable key for a set of sources: sort the `{source_type}:{source_id}`
    /// registry keys lexically and join them. Order-independent, collision-free
    /// (no hashing), and changes whenever a source id is added or removed.
    pub fn source_set_key(sources: &[(SourceType, String)]) -> String {
        let mut keys: Vec<String> = sources
            .iter()
            .map(|(t, id)| format!("{}:{}", t.as_str(), id))
            .collect();
        keys.sort();
        keys.join("|")
    }

    fn bump_epoch(&mut self) -> u64 {
        let e = self.next_epoch;
        self.next_epoch += 1;
        e
    }

    /// Move `key` to the most-recently-used position.
    fn touch(&mut self, key: &str) {
        if let Some(pos) = self.recency.iter().position(|k| k == key) {
            let k = self.recency.remove(pos);
            self.recency.push(k);
        }
    }

    /// Evict least-recently-used library entries until within capacity.
    fn evict_to_capacity(&mut self) {
        while self.library.len() > self.capacity {
            // Find the first still-present recency key (front = LRU).
            let victim = self
                .recency
                .iter()
                .find(|k| self.library.contains_key(*k))
                .cloned();
            match victim {
                Some(k) => {
                    self.library.remove(&k);
                    if let Some(pos) = self.recency.iter().position(|r| r == &k) {
                        self.recency.remove(pos);
                    }
                }
                None => break,
            }
        }
    }

    /// Read a library entry, marking it most-recently-used. Returns `None` on a
    /// miss (including a never-fetched library). An empty-but-successful fetch is
    /// a valid hit and returns `Some(&[])`.
    pub fn get_library(&mut self, key: &str) -> Option<&[MediaItem]> {
        if self.library.contains_key(key) {
            self.touch(key);
            return self.library.get(key).map(|e| e.items.as_slice());
        }
        None
    }

    /// Inspect a library entry without affecting recency.
    pub fn peek_library(&self, key: &str) -> Option<&[MediaItem]> {
        self.library.get(key).map(|e| e.items.as_slice())
    }

    pub fn contains_library(&self, key: &str) -> bool {
        self.library.contains_key(key)
    }

    /// The epoch of a cached library entry, if present.
    pub fn library_epoch(&self, key: &str) -> Option<u64> {
        self.library.get(key).map(|e| e.epoch)
    }

    /// Insert or replace a library entry. Bumps the entry epoch (even when the
    /// items are unchanged, so an in-flight refetch for the prior epoch is
    /// dropped), marks it most-recently-used, and evicts the LRU tail if over
    /// capacity. Returns the new epoch.
    pub fn put_library(&mut self, key: &str, items: Vec<MediaItem>) -> u64 {
        let epoch = self.bump_epoch();
        self.library
            .insert(key.to_string(), LibraryEntry { items, epoch });
        // Refresh recency position.
        if let Some(pos) = self.recency.iter().position(|k| k == key) {
            self.recency.remove(pos);
        }
        self.recency.push(key.to_string());
        self.evict_to_capacity();
        epoch
    }

    /// Remove every library entry whose key begins with `prefix` (e.g.
    /// `{source_type}:{source_id}:`). Used when a source is removed.
    pub fn invalidate_source(&mut self, prefix: &str) {
        let removed: Vec<String> = self
            .library
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        for k in &removed {
            self.library.remove(k);
        }
        self.recency.retain(|k| self.library.contains_key(k));
    }

    /// Number of cached library entries (excludes the Home slot).
    pub fn library_len(&self) -> usize {
        self.library.len()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    // --- Home slot (exempt from LRU) ---

    /// Store the assembled Home payload for `source_set_key`. Bumps the epoch.
    pub fn set_home(&mut self, home: CachedHome) {
        let epoch = self.bump_epoch();
        self.home = Some(CachedHome { epoch, ..home });
    }

    /// The cached Home payload, but only when it was built for `source_set_key`
    /// — a changed source set is a miss.
    pub fn get_home(&self, source_set_key: &str) -> Option<&CachedHome> {
        self.home
            .as_ref()
            .filter(|h| h.source_set_key == source_set_key)
    }

    /// The cached Home payload regardless of source set (for in-place patching of
    /// the existing entry, e.g. an R8 Continue-Watching patch).
    pub fn home_mut(&mut self) -> Option<&mut CachedHome> {
        self.home.as_mut()
    }

    pub fn home_epoch(&self) -> Option<u64> {
        self.home.as_ref().map(|h| h.epoch)
    }

    pub fn invalidate_home(&mut self) {
        self.home = None;
    }

    // --- Source-set epoch (revalidation guard) ---

    pub fn source_set_epoch(&self) -> u64 {
        self.source_set_epoch
    }

    /// Advance the source-set epoch. Call when the source set changes so any
    /// in-flight refetch stamped with the old value is dropped on return.
    pub fn bump_source_set_epoch(&mut self) {
        self.source_set_epoch += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::media::MediaType;

    /// Minimal MediaItem for cache tests; only identity fields matter here.
    fn item(id: &str) -> MediaItem {
        MediaItem {
            id: id.to_string(),
            source_type: SourceType::Plex,
            source_id: "srv".into(),
            external_id: id.to_string(),
            media_type: MediaType::Movie,
            title: id.to_string(),
            year: None,
            overview: None,
            content_rating: None,
            rating: None,
            runtime_minutes: None,
            poster_path: None,
            series_poster_path: None,
            backdrop_path: None,
            genres: Vec::new(),
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

    fn items(ids: &[&str]) -> Vec<MediaItem> {
        ids.iter().map(|i| item(i)).collect()
    }

    #[test]
    fn put_then_get_returns_stored_items() {
        let mut c = SessionContentCache::new();
        c.put_library("plex:srv:1", items(&["a", "b"]));
        let got = c.get_library("plex:srv:1").expect("hit");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].id, "a");
    }

    #[test]
    fn get_missing_library_returns_none() {
        let mut c = SessionContentCache::new();
        assert!(c.get_library("plex:srv:absent").is_none());
    }

    #[test]
    fn empty_successful_fetch_is_a_valid_hit() {
        let mut c = SessionContentCache::new();
        c.put_library("plex:srv:empty", Vec::new());
        // A hit returns Some(&[]), distinct from a miss (None).
        assert!(c.get_library("plex:srv:empty").is_some());
        assert_eq!(c.get_library("plex:srv:empty").unwrap().len(), 0);
    }

    #[test]
    fn over_capacity_put_evicts_least_recently_used() {
        let mut c = SessionContentCache::with_capacity(3);
        c.put_library("k1", items(&["a"]));
        c.put_library("k2", items(&["b"]));
        c.put_library("k3", items(&["c"]));
        // k1 is LRU; inserting k4 evicts it.
        c.put_library("k4", items(&["d"]));
        assert_eq!(c.library_len(), 3);
        assert!(!c.contains_library("k1"));
        assert!(c.contains_library("k4"));
    }

    #[test]
    fn get_refreshes_recency_so_touched_entry_survives() {
        let mut c = SessionContentCache::with_capacity(3);
        c.put_library("k1", items(&["a"]));
        c.put_library("k2", items(&["b"]));
        c.put_library("k3", items(&["c"]));
        // Touch k1 so it is no longer the LRU victim.
        let _ = c.get_library("k1");
        c.put_library("k4", items(&["d"]));
        assert!(c.contains_library("k1"), "touched entry should survive");
        assert!(!c.contains_library("k2"), "k2 is now LRU and evicted");
    }

    #[test]
    fn interleaved_recency_evicts_correct_victim() {
        // Known sequence over 5 keys at capacity 3, asserting the exact victim.
        let mut c = SessionContentCache::with_capacity(3);
        c.put_library("a", items(&["a"])); // [a]
        c.put_library("b", items(&["b"])); // [a,b]
        c.put_library("c", items(&["c"])); // [a,b,c]
        let _ = c.get_library("a"); // recency: b,c,a
        let _ = c.get_library("b"); // recency: c,a,b ; c is LRU
        c.put_library("d", items(&["d"])); // evicts c -> [a,b,d]
        assert!(!c.contains_library("c"));
        assert!(c.contains_library("a") && c.contains_library("b") && c.contains_library("d"));
        let _ = c.get_library("d"); // recency: a,b,d... wait a was touched earlier
        c.put_library("e", items(&["e"])); // a is LRU now -> evicts a
        assert!(!c.contains_library("a"));
        assert!(c.contains_library("b") && c.contains_library("d") && c.contains_library("e"));
    }

    #[test]
    fn home_slot_never_evicted_by_library_pressure() {
        let mut c = SessionContentCache::with_capacity(2);
        c.set_home(CachedHome {
            source_set_key: "plex:srv".into(),
            continue_watching: Vec::new(),
            recently_added: Vec::new(),
            collections: Vec::new(),
            hubs: Vec::new(),
            epoch: 0,
        });
        for n in 0..10 {
            c.put_library(&format!("k{n}"), items(&["x"]));
        }
        assert_eq!(c.library_len(), 2);
        assert!(c.get_home("plex:srv").is_some(), "Home survives churn");
    }

    #[test]
    fn invalidate_source_drops_only_matching_prefix() {
        let mut c = SessionContentCache::with_capacity(10);
        c.put_library("plex:srvA:1", items(&["a"]));
        c.put_library("plex:srvA:2", items(&["b"]));
        c.put_library("plex:srvB:1", items(&["c"]));
        c.invalidate_source("plex:srvA:");
        assert!(!c.contains_library("plex:srvA:1"));
        assert!(!c.contains_library("plex:srvA:2"));
        assert!(c.contains_library("plex:srvB:1"), "other source untouched");
        // Recency no longer references the dropped keys.
        c.put_library("plex:srvB:2", items(&["d"]));
        assert_eq!(c.library_len(), 2);
    }

    #[test]
    fn source_set_key_stable_under_reorder_changes_on_membership() {
        let a = SessionContentCache::source_set_key(&[
            (SourceType::Plex, "1".into()),
            (SourceType::Jellyfin, "2".into()),
        ]);
        let b = SessionContentCache::source_set_key(&[
            (SourceType::Jellyfin, "2".into()),
            (SourceType::Plex, "1".into()),
        ]);
        assert_eq!(a, b, "order-independent");
        let c = SessionContentCache::source_set_key(&[(SourceType::Plex, "1".into())]);
        assert_ne!(a, c, "membership change yields a different key");
    }

    #[test]
    fn get_home_misses_on_changed_source_set() {
        let mut c = SessionContentCache::new();
        c.set_home(CachedHome {
            source_set_key: "plex:1".into(),
            continue_watching: Vec::new(),
            recently_added: Vec::new(),
            collections: Vec::new(),
            hubs: Vec::new(),
            epoch: 0,
        });
        assert!(c.get_home("plex:1").is_some());
        assert!(
            c.get_home("plex:1|plex:2").is_none(),
            "different set misses"
        );
    }

    #[test]
    fn re_put_increments_epoch() {
        let mut c = SessionContentCache::new();
        let e1 = c.put_library("k", items(&["a"]));
        let e2 = c.put_library("k", items(&["a"])); // identical items
        assert!(e2 > e1, "epoch advances even for identical re-put");
        assert_eq!(c.library_epoch("k"), Some(e2));
    }

    #[test]
    fn source_set_epoch_advances_on_bump() {
        let mut c = SessionContentCache::new();
        let before = c.source_set_epoch();
        c.bump_source_set_epoch();
        assert!(c.source_set_epoch() > before);
    }
}
