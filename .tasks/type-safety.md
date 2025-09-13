# Type-Safety Refactoring Checklist

This checklist tracks the implementation of type-safety improvements identified in `docs/service-type-safety.md`.

## Phase 1: Core Type Definitions ✅ COMPLETED
*Create fundamental ID types as newtypes around strings*

### Create New Types Module
- [x] Create `src/models/identifiers.rs` file
- [x] Add module export in `src/models/mod.rs`

### Implement Core ID Types
- [x] `SourceId` newtype wrapper
  - [x] Implement `new()` constructor
  - [x] Implement `as_str()` method
  - [x] Implement `Display` trait
  - [x] Implement `Debug` trait
  - [x] Implement `Clone` trait
  - [x] Implement `PartialEq` and `Eq` traits
  - [x] Implement `Hash` trait
  - [x] Implement `Serialize` and `Deserialize` traits
  - [x] Implement `From<String>` and `From<&str>` for compatibility

- [x] `BackendId` newtype wrapper
  - [x] Implement `new()` constructor
  - [x] Implement `as_str()` method
  - [x] Implement `Display` trait
  - [x] Implement `Debug` trait
  - [x] Implement `Clone` trait
  - [x] Implement `PartialEq` and `Eq` traits
  - [x] Implement `Hash` trait
  - [x] Implement `Serialize` and `Deserialize` traits
  - [x] Implement `From<String>` and `From<&str>` for compatibility

- [x] `ProviderId` newtype wrapper
  - [x] Implement `new()` constructor
  - [x] Implement `as_str()` method
  - [x] Implement `Display` trait
  - [x] Implement `Debug` trait
  - [x] Implement `Clone` trait
  - [x] Implement `PartialEq` and `Eq` traits
  - [x] Implement `Hash` trait
  - [x] Implement `Serialize` and `Deserialize` traits
  - [x] Implement `From<String>` and `From<&str>` for compatibility

- [x] `LibraryId` newtype wrapper
  - [x] Implement `new()` constructor
  - [x] Implement `as_str()` method
  - [x] Implement `Display` trait
  - [x] Implement `Debug` trait
  - [x] Implement `Clone` trait
  - [x] Implement `PartialEq` and `Eq` traits
  - [x] Implement `Hash` trait
  - [x] Implement `Serialize` and `Deserialize` traits
  - [x] Implement `From<String>` and `From<&str>` for compatibility

- [x] `MediaItemId` newtype wrapper
  - [x] Implement `new()` constructor
  - [x] Implement `as_str()` method
  - [x] Implement `Display` trait
  - [x] Implement `Debug` trait
  - [x] Implement `Clone` trait
  - [x] Implement `PartialEq` and `Eq` traits
  - [x] Implement `Hash` trait
  - [x] Implement `Serialize` and `Deserialize` traits
  - [x] Implement `From<String>` and `From<&str>` for compatibility

- [x] `ShowId` newtype wrapper
  - [x] Implement `new()` constructor
  - [x] Implement `as_str()` method
  - [x] Implement `Display` trait
  - [x] Implement `Debug` trait
  - [x] Implement `Clone` trait
  - [x] Implement `PartialEq` and `Eq` traits
  - [x] Implement `Hash` trait
  - [x] Implement `Serialize` and `Deserialize` traits
  - [x] Implement `From<String>` and `From<&str>` for compatibility

- [x] `UserId` newtype wrapper
  - [x] Implement `new()` constructor
  - [x] Implement `as_str()` method
  - [x] Implement `Display` trait
  - [x] Implement `Debug` trait
  - [x] Implement `Clone` trait
  - [x] Implement `PartialEq` and `Eq` traits
  - [x] Implement `Hash` trait
  - [x] Implement `Serialize` and `Deserialize` traits
  - [x] Implement `From<String>` and `From<&str>` for compatibility

### Add Unit Tests for ID Types
- [x] Test `SourceId` creation and conversion
- [x] Test `BackendId` creation and conversion
- [x] Test `ProviderId` creation and conversion
- [x] Test `LibraryId` creation and conversion
- [x] Test `MediaItemId` creation and conversion
- [x] Test `ShowId` creation and conversion
- [x] Test `UserId` creation and conversion
- [x] Test equality and hashing for all types
- [x] Test serialization/deserialization for all types

## Phase 2: Cache Key System ⚠️ PARTIAL (30% Complete)
*Replace string-based cache key construction with type-safe enum*

### Create Cache Key Module ✅
- [x] Create `src/services/cache_keys.rs` file
- [x] Add module export in `src/services/mod.rs`

### Implement CacheKey Enum ✅
- [x] Define `CacheKey` enum with variants:
  - [x] `Media(String)` - Simple media cache key for backward compatibility
  - [x] `Libraries(SourceId)`
  - [x] `LibraryItems(SourceId, LibraryId)`
  - [x] `MediaItem { source: SourceId, library: LibraryId, media_type: MediaType, item_id: MediaItemId }`
  - [x] `HomeSections(SourceId)`
  - [x] `ShowEpisodes(SourceId, LibraryId, ShowId)`
  - [x] `Episode(SourceId, LibraryId, MediaItemId)`
  - [x] `Show(SourceId, LibraryId, ShowId)`
  - [x] `Movie(SourceId, LibraryId, MediaItemId)`

- [x] Implement `CacheKey` methods:
  - [x] `to_string()` - Convert to string representation
  - [x] `parse()` - Parse from string (for migration)
  - [x] `source_id()` - Extract source ID if present
  - [x] `library_id()` - Extract library ID if present

### Replace Cache Key Construction in DataService ⚠️ PARTIAL
- [x] Replace `format!("media:{}", cache_key)` patterns (3 instances replaced in get_media() and store_library_list())
- [ ] Replace `format!("{}:libraries", backend_id)` 
- [ ] Replace `format!("{}:library:{}:items", backend_id, library_id)`
- [ ] Replace `format!("{}:{}:{}:{}", backend_id, library_id, item_type, item.id())`
- [ ] Replace `format!("{}:home_sections", source_id)`
- [ ] Update `store_media_item()` to use CacheKey
- [x] Update `get_media()` to use CacheKey (partial - Media variant only)
- [x] Update `store_library_list()` to use CacheKey (partial - Media variant only)
- [ ] Update `get_home_sections()` to use CacheKey

### Replace Cache Key Construction in SyncManager
- [ ] Replace all `format!("{}:libraries", backend_id)` (line 574)
- [ ] Replace all `format!("{}:library:{}:items", backend_id, library_id)` (line 400)
- [ ] Replace all `format!("{}:{}:{}:{}", backend_id, library_id, item_type, item.id())` (line 436)
- [ ] Replace all `format!("{}:{}:show:{}", backend_id, library_id, show_id)` (line 671)
- [ ] Replace all `format!("{}:{}:episode:{}", backend_id, library_id, episode.id)` (line 714)
- [ ] Replace all `format!("{}:home_sections", backend_id)` (line 159)
- [ ] Replace all `format!("{}:library:{}:movies", backend_id, library_id)` (line 609)
- [ ] Replace all `format!("{}:library:{}:shows", backend_id, library_id)` (line 619)

### Add Cache Key Tests ✅
- [x] Test `to_string()` for all variants
- [x] Test `parse()` for valid strings
- [x] Test `parse()` error handling for invalid strings
- [x] Test round-trip conversion (parse(to_string()) == original)

## Phase 3: DataService Type Safety
*Update DataService to use typed IDs - highest impact service*

### Update Method Signatures
- [ ] `store_library(library: &Library, source_id: SourceId)`
- [ ] `store_media_item(cache_key: CacheKey, media_item: &MediaItem)`
- [ ] `store_media_item_internal(cache_key: CacheKey, media_item: &MediaItem, emit_events: bool)`
- [ ] `store_media_item_silent(cache_key: CacheKey, media_item: &MediaItem)`
- [ ] `get_media<T>(cache_key: CacheKey) -> Result<Option<T>>`
- [ ] `get_media_item(id: MediaItemId) -> Result<Option<MediaItem>>`
- [ ] `get_media_item_by_backend_id(source_id: SourceId, backend_item_id: &str)`
- [ ] `get_libraries(source_id: SourceId) -> Result<Vec<LibraryModel>>`
- [ ] `get_library(id: LibraryId) -> Result<Option<LibraryModel>>`
- [ ] `get_source(id: SourceId) -> Result<Option<SourceModel>>`
- [ ] `get_playback_progress(media_id: MediaItemId) -> Result<Option<(u64, u64)>>`
- [ ] `set_playback_progress(media_id: MediaItemId, position: u64, duration: u64)`
- [ ] `update_playback_progress(media_id: MediaItemId, position_ms: i64, duration_ms: i64, watched: bool)`
- [ ] `clear_backend_cache(backend_id: BackendId) -> Result<()>`
- [ ] `sync_libraries_transactional(backend_id: BackendId, libraries: &[Library], items_by_library: &[(LibraryId, Vec<MediaItem>)])`
- [ ] `get_media_items(library_id: LibraryId) -> Result<Vec<MediaItem>>`
- [ ] `get_media_item_models(library_id: LibraryId) -> Result<Vec<MediaItemModel>>`
- [ ] `get_media_items_by_ids(ids: &[MediaItemId]) -> Result<Vec<MediaItem>>`
- [ ] `get_media_items_since(library_id: LibraryId, since: NaiveDateTime)`
- [ ] `add_source(source: SourceModel) -> Result<()>`
- [ ] `remove_source(id: SourceId) -> Result<()>`
- [ ] `sync_sources_to_database(provider_id: ProviderId, discovered_sources: &[Source])`
- [ ] `cleanup_sources_from_config(valid_source_ids: &[SourceId])`
- [ ] `get_latest_sync_status(source_id: SourceId)`
- [ ] `get_continue_watching_for_source(source_id: SourceId)`
- [ ] `get_recently_added_for_source(source_id: SourceId, limit: Option<usize>)`
- [ ] `store_home_sections(cache_key: CacheKey, sections: &[HomeSection])`
- [ ] `get_home_sections(cache_key: CacheKey) -> Result<Vec<HomeSection>>`
- [ ] `get_home_sections_for_source(source_id: SourceId)`
- [ ] `get_episodes_by_show(show_id: ShowId) -> Result<Vec<MediaItem>>`
- [ ] `get_episodes_by_season(show_id: ShowId, season_number: i32)`
- [ ] `store_episode(episode: &Episode, show_id: ShowId, season_number: i32)`
- [ ] `count_media_in_library(library_id: LibraryId) -> Result<i64>`
- [ ] `get_media_in_library_paginated(library_id: LibraryId, offset: i64, limit: usize)`
- [ ] `update_library_item_count(library_id: LibraryId, count: i32)`

### Update Internal Logic
- [ ] Remove string parsing in `store_media_item_internal()` (lines 130-138, 300-301)
- [ ] Update cache key extraction logic to use CacheKey methods
- [ ] Replace all `format!()` calls with CacheKey construction
- [ ] Update repository calls to use typed IDs

## Phase 4: SyncManager Type Safety
*Update SyncManager to use typed IDs*

### Update Method Signatures
- [ ] `sync_backend(backend_id: BackendId, backend: Arc<dyn MediaBackend>)`
- [ ] `sync_library(backend_id: BackendId, library_id: LibraryId)`
- [ ] `sync_library_items(backend_id: BackendId, library_id: LibraryId, library_type: &LibraryType, backend: Arc<dyn MediaBackend>)`
- [ ] `get_sync_status(backend_id: BackendId) -> SyncStatus`
- [ ] `get_cached_libraries(backend_id: BackendId) -> Result<Vec<Library>>`
- [ ] `get_cached_movies(backend_id: BackendId, library_id: LibraryId)`
- [ ] `get_cached_shows(backend_id: BackendId, library_id: LibraryId)`
- [ ] `get_cached_items(backend_id: BackendId, library_id: LibraryId)`
- [ ] `get_library_item_count(backend_id: BackendId, library_id: LibraryId)`
- [ ] `queue_media_poster(media_item: &MediaItem, media_id: MediaItemId)`
- [ ] `sync_show_episodes(backend_id: BackendId, library_id: LibraryId, show_id: ShowId, backend: Arc<dyn MediaBackend>)`

### Update Internal Logic
- [ ] Replace `HashMap<String, SyncStatus>` with `HashMap<BackendId, SyncStatus>`
- [ ] Update all cache key construction to use CacheKey enum
- [ ] Update PosterDownloadRequest to use MediaItemId
- [ ] Update SyncResult to use BackendId

## Phase 5: AuthManager Type Safety
*Update AuthManager to use typed IDs*

### Update Method Signatures
- [ ] `authenticate_provider(provider_id: ProviderId) -> Result<AuthStatus>`
- [ ] `get_provider_status(provider_id: ProviderId) -> AuthStatus`
- [ ] `remove_provider(provider_id: ProviderId) -> Result<()>`
- [ ] `store_credentials(provider_id: ProviderId, field: &str, value: &str)`
- [ ] `get_credentials(provider_id: ProviderId, field: &str) -> Result<Option<String>>`
- [ ] `remove_credentials(provider_id: ProviderId, field: &str)`
- [ ] `store_token(provider_id: ProviderId, token: &str)`
- [ ] `get_token(provider_id: ProviderId) -> Result<Option<String>>`
- [ ] `remove_token(provider_id: ProviderId)`
- [ ] `get_user_for_provider(provider_id: ProviderId) -> Option<User>`

### Update Internal Logic
- [ ] Replace `HashMap<String, AuthProvider>` with `HashMap<ProviderId, AuthProvider>`
- [ ] Replace `HashMap<String, AuthStatus>` with `HashMap<ProviderId, AuthStatus>`
- [ ] Replace `HashMap<String, User>` with `HashMap<ProviderId, User>`
- [ ] Update keyring key construction to use ProviderId

## Phase 6: SourceCoordinator Type Safety
*Update SourceCoordinator to use typed IDs*

### Update Method Signatures
- [ ] `register_backend(source_id: SourceId, backend: Arc<dyn MediaBackend>)`
- [ ] `unregister_backend(source_id: SourceId)`
- [ ] `get_backend(source_id: SourceId) -> Option<Arc<dyn MediaBackend>>`
- [ ] `sync_source(source_id: SourceId) -> Result<()>`
- [ ] `sync_all_sources() -> Result<Vec<(SourceId, Result<()>)>>`
- [ ] `get_source_status(source_id: SourceId) -> Option<SourceStatus>`
- [ ] `update_source_status(source_id: SourceId, status: SourceStatus)`
- [ ] `get_libraries_for_source(source_id: SourceId) -> Result<Vec<Library>>`
- [ ] `get_media_items_for_library(source_id: SourceId, library_id: LibraryId)`
- [ ] `discover_sources_for_provider(provider_id: ProviderId) -> Result<Vec<Source>>`

### Update SourceStatus Structure
- [ ] Change `source_id: String` to `source_id: SourceId`
- [ ] Update all usages of SourceStatus

### Update Internal Logic
- [ ] Replace `Arc<RwLock<HashMap<String, Arc<dyn MediaBackend>>>>` with `HashMap<SourceId, Arc<dyn MediaBackend>>`
- [ ] Replace `Arc<RwLock<HashMap<String, SourceStatus>>>` with `HashMap<SourceId, SourceStatus>`
- [ ] Update discovery logic to use typed IDs

## Phase 7: Backend Manager Type Safety
*Update BackendManager to use typed IDs*

### Update in `src/backends/mod.rs`
- [ ] Change `backends: HashMap<String, Arc<dyn MediaBackend>>` to use `BackendId`
- [ ] Change `backend_order: Vec<String>` to `Vec<BackendId>`
- [ ] Update `register_backend(name: BackendId, backend: Arc<dyn MediaBackend>)`
- [ ] Update `remove_backend(name: BackendId) -> Option<Arc<dyn MediaBackend>>`
- [ ] Update `get_backend(name: BackendId) -> Option<Arc<dyn MediaBackend>>`
- [ ] Update `get_all_backends() -> Vec<(BackendId, Arc<dyn MediaBackend>)>`
- [ ] Update `reorder_backends(new_order: Vec<BackendId>)`
- [ ] Update `move_backend_up(backend_id: BackendId)`
- [ ] Update `move_backend_down(backend_id: BackendId)`
- [ ] Update `unregister_backend(name: BackendId)`
- [ ] Update `list_backends() -> Vec<(BackendId, BackendInfo)>`

## Phase 8: MediaBackend Trait Updates
*Update the MediaBackend trait and implementations*

### Update Trait Definition
- [ ] Change `get_backend_id() -> BackendId` return type
- [ ] Update any methods that accept or return string IDs

### Update Plex Backend
- [ ] Return `BackendId` from `get_backend_id()`
- [ ] Use `LibraryId` for library methods
- [ ] Use `MediaItemId` for media methods
- [ ] Use `ShowId` for show-specific methods

### Update Jellyfin Backend
- [ ] Return `BackendId` from `get_backend_id()`
- [ ] Use `LibraryId` for library methods
- [ ] Use `MediaItemId` for media methods
- [ ] Use `ShowId` for show-specific methods

### Update Local Backend
- [ ] Return `BackendId` from `get_backend_id()`
- [ ] Use `LibraryId` for library methods
- [ ] Use `MediaItemId` for media methods

## Phase 9: Repository Layer Updates
*Update repository implementations to use typed IDs*

### MediaRepository
- [ ] Update `find_by_id(id: MediaItemId)`
- [ ] Update `find_by_library(library_id: LibraryId)`
- [ ] Update `find_by_source(source_id: SourceId)`
- [ ] Update `find_by_source_and_backend_id(source_id: SourceId, backend_item_id: &str)`
- [ ] Update `find_episodes_by_show(show_id: ShowId)`
- [ ] Update `find_episodes_by_season(show_id: ShowId, season_number: i32)`
- [ ] Update `count_by_library(library_id: LibraryId)`
- [ ] Update `find_by_library_paginated(library_id: LibraryId, offset: u64, limit: u64)`

### LibraryRepository
- [ ] Update `find_by_id(id: LibraryId)`
- [ ] Update `find_by_source(source_id: SourceId)`
- [ ] Update `update_item_count(library_id: LibraryId, count: i32)`

### SourceRepository
- [ ] Update `find_by_id(id: SourceId)`
- [ ] Update `cleanup_sources_for_provider(provider_id: ProviderId, keep_ids: &[SourceId])`
- [ ] Update `archive_invalid_sources(valid_ids: &[SourceId])`

### PlaybackRepository
- [ ] Update `find_by_media_id(media_id: MediaItemId)`
- [ ] Update `upsert_progress(media_id: MediaItemId, user_id: Option<UserId>, position_ms: i64, duration_ms: i64)`

## Phase 10: Database Entity Updates
*Update SeaORM entities to use typed IDs*

### Update Entity Models
- [ ] Update `MediaItemModel` to use typed IDs in fields
- [ ] Update `LibraryModel` to use typed IDs in fields
- [ ] Update `SourceModel` to use typed IDs in fields
- [ ] Update `PlaybackProgressModel` to use typed IDs in fields
- [ ] Update `SyncStatusModel` to use typed IDs in fields

### Add Custom SeaORM Type Implementations
- [ ] Implement `From<SourceId>` for sea_orm Value
- [ ] Implement `TryFrom<Value>` for SourceId
- [ ] Implement similar for all ID types
- [ ] Add database column type mappings

## Phase 11: Event System Updates
*Update event payloads to use typed IDs*

### Update EventPayload Variants
- [ ] Update `Media` variant to use typed IDs
  - [ ] `id: MediaItemId`
  - [ ] `library_id: LibraryId`
  - [ ] `source_id: SourceId`
- [ ] Update `Library` variant to use typed IDs
  - [ ] `id: LibraryId`
  - [ ] `source_id: SourceId`
- [ ] Update `Source` variant to use typed IDs
  - [ ] `id: SourceId`
- [ ] Update `Sync` variant to use typed IDs
  - [ ] `source_id: SourceId`
- [ ] Update `MediaBatch` variant to use typed IDs
  - [ ] `ids: Vec<MediaItemId>`
  - [ ] `library_id: LibraryId`
  - [ ] `source_id: SourceId`

## Phase 12: UI Layer Updates
*Update UI components to use typed IDs*

### Update ViewModels
- [ ] LibraryViewModel to use typed IDs
- [ ] PlayerViewModel to use typed IDs
- [ ] SourcesViewModel to use typed IDs
- [ ] SidebarViewModel to use typed IDs
- [ ] DetailsViewModel to use typed IDs
- [ ] HomeViewModel to use typed IDs

### Update Navigation
- [ ] Update `LibraryIdentifier` to use typed IDs
- [ ] Update navigation requests to use typed IDs
- [ ] Update route handling to use typed IDs

## Phase 13: Testing & Validation
*Comprehensive testing of type-safe implementation*

### Integration Tests
- [ ] Test DataService with typed IDs
- [ ] Test SyncManager with typed IDs
- [ ] Test AuthManager with typed IDs
- [ ] Test SourceCoordinator with typed IDs
- [ ] Test BackendManager with typed IDs

### Migration Tests
- [ ] Test backward compatibility with string-based IDs
- [ ] Test CacheKey parsing of legacy keys
- [ ] Test database migration with existing data
- [ ] Test gradual migration path

### Performance Tests
- [ ] Benchmark typed ID creation vs strings
- [ ] Benchmark HashMap lookups with typed keys
- [ ] Benchmark serialization/deserialization

## Phase 14: Documentation Updates
*Update documentation to reflect type-safe APIs*

- [ ] Update API documentation in code
- [ ] Update CLAUDE.md with new type system
- [ ] Update example code snippets
- [ ] Document migration guide for external users
- [ ] Add type-safety best practices guide

## Phase 15: Cleanup & Optimization
*Remove legacy code and optimize*

### Remove Legacy Code
- [ ] Remove old string-based cache key construction
- [ ] Remove string parsing logic
- [ ] Remove "unknown" fallback patterns
- [ ] Remove unnecessary string cloning

### Optimize ID Storage
- [ ] Consider using `Arc<str>` internally for IDs
- [ ] Implement ID interning for frequently used IDs
- [ ] Add ID validation on construction
- [ ] Consider SmallString optimization for short IDs

## Completion Metrics

- [ ] All 87+ `backend_id: &str` parameters converted
- [ ] All 52+ `source_id: &str` parameters converted
- [ ] All 43+ `library_id: &str` parameters converted
- [ ] All 35+ `media_id: &str` parameters converted
- [ ] All 28+ `provider_id: &str` parameters converted
- [ ] Zero string-based cache key constructions remaining
- [ ] Zero string parsing operations for ID extraction
- [ ] 100% of service methods use typed IDs
- [ ] All tests passing with typed IDs
- [ ] Migration guide complete and tested

## Notes

- Start with Phase 1-2 as they lay the foundation
- DataService (Phase 3) has highest impact - prioritize after foundation
- Phases can be done in parallel by different team members after Phase 1-2
- Maintain backward compatibility throughout migration
- Each phase should include its own tests before moving on
- Consider feature flags for gradual rollout