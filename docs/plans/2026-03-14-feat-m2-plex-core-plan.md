---
title: "feat: M2 Plex Core - Library browsing with metadata and artwork"
type: feat
status: active
date: 2026-03-14
---

# M2: Plex Core

## Context

M0 (Walking Skeleton) and M1 (Full-Featured Player) are complete. The app currently launches as a single-view video player with 121+ tests, overlay controls, keyboard shortcuts, drag-and-drop, screensaver inhibition, and window state persistence. There is no library UI, no database, no HTTP client, and no Plex integration.

M2 transforms Reel from a standalone file player into a Plex library browser. Users will connect to a Plex server, browse movie and TV show libraries as poster grids, view detail pages with metadata, and play media directly from Plex. This is the foundational milestone that enables all future library features (search, watch state, collections).

## Overview

Connect to a Plex server and browse its libraries with metadata and artwork. The user flow is: configure Plex server → browse poster grid → view detail page → play media.

## Acceptance Criteria

### Functional

- [ ] Connect to a Plex server via URL + token, with "Test Connection" feedback
- [ ] Browse movie library as a poster grid with artwork
- [ ] Browse TV show library as a poster grid with artwork
- [ ] Click movie → detail page with backdrop, title, year, rating, runtime, genres, synopsis
- [ ] Click Play on movie detail → video plays via Plex direct play URL
- [ ] Click TV show → detail page with seasons → episode list → play episode
- [ ] Sidebar navigation between Movies and TV Shows
- [ ] Artwork downloads and caches to disk
- [ ] Navigation back from detail to library, from player to detail
- [ ] Empty state shown when no Plex server configured
- [ ] Error states for connection failures, network errors

### Non-Functional

- [ ] Library grid uses virtual scrolling (TypedGridView) for performance with large libraries
- [ ] All new backend code has unit tests (models, db, plex client, artwork)
- [ ] Plex HTTP client tested with wiremock mock server
- [ ] Database tested with in-memory SQLite
- [ ] Serde models tested with real Plex API response fixtures
- [ ] Zero clippy warnings, formatted with cargo fmt
- [ ] Auth token not logged or exposed in UI

## Technical Approach

### Architecture

M2 introduces three new layers beneath the existing UI:

```
┌─────────────────────────────────────────────┐
│ UI Layer (Relm4 Components)                  │
│  Sidebar │ LibraryGrid │ Detail │ Player     │
└──────────────────┬──────────────────────────┘
                   │ Messages + Commands
┌──────────────────┴──────────────────────────┐
│ Service Layer                                │
│  MediaSource trait │ PlexSource │ ArtworkCache│
└──────────────────┬──────────────────────────┘
                   │
┌──────────────────┴──────────────────────────┐
│ Backend Layer                                │
│  PlexClient (HTTP) │ SQLite DB │ Models      │
└─────────────────────────────────────────────┘
```

### Top-Level UI Architecture

The app restructures from single-view player to a navigation shell:

```
AdwApplicationWindow
  ToastOverlay
    gtk::Stack ("shell" / "player")
      ├── AdwNavigationSplitView (shell page)
      │     sidebar: Sidebar component
      │     content: AdwNavigationView
      │       page 0: LibraryView (root, always present)
      │       page 1: MovieDetailView (pushed on click)
      │       page 2: ShowDetailView (pushed on click)
      └── PlayerView (player page)
            VideoArea + PlayerControls (existing M1 code)
```

**Key decision**: `gtk::Stack` at the top level separates the navigation shell from the player. The player needs to control fullscreen, cursor hiding, and overlay behavior independently. Keeping it in a separate stack page means existing M1 player code stays largely unchanged.

### New Dependencies

```toml
# Cargo.toml additions
[dependencies]
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.32", features = ["bundled"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
async-trait = "0.1"
sha2 = "0.10"

[dev-dependencies]
wiremock = "0.6"
```

Notes:
- `rustls-tls` avoids OpenSSL linkage (simpler nix/flatpak builds)
- `rusqlite` with `bundled` compiles SQLite from C source (no system dependency)
- `sha2` for artwork cache key hashing

### Error Strategy

Per-layer error enums with `thiserror` (already a dependency):

- `PlexError` — HTTP, deserialization, auth, not found
- `DbError` — wraps rusqlite::Error + migration errors
- `SourceError` — generic source errors (MediaSource trait boundary)
- `ArtworkError` — download, I/O

### Async Integration

- Plex API calls: `sender.oneshot_command(async { ... })` in Relm4 components
- Artwork downloads: same pattern, returns `gdk::Texture` or file path
- Database: synchronous rusqlite (fast enough for local cache reads)
- All async tests use `#[tokio::test]`

## Implementation Phases

### Phase 1: Foundation (Dependencies, Config, Models)

Add M2 dependencies to Cargo.toml. Create config module and data model types.

#### 1a. Dependencies + Config

**Files**:
- `Cargo.toml` — add reqwest, serde, serde_json, rusqlite, tokio, async-trait, sha2, wiremock
- `src/config.rs` — XDG path helpers

```rust
// src/config.rs
pub fn data_dir() -> PathBuf       // $XDG_DATA_HOME/reel
pub fn cache_dir() -> PathBuf      // $XDG_CACHE_HOME/reel
pub fn config_dir() -> PathBuf     // $XDG_CONFIG_HOME/reel
pub fn db_path() -> PathBuf        // data_dir()/reel.db
pub fn artwork_dir() -> PathBuf    // cache_dir()/artwork
```

**Tests**: paths end with expected segments, `nix develop -c cargo check` passes

#### 1b. Data Models

**Files**:
- `src/models/mod.rs`
- `src/models/media.rs` — MediaItem, MediaType, SourceType
- `src/models/source.rs` — Source, SourceConfig
- `src/models/library.rs` — LibrarySection

```rust
// src/models/media.rs
#[derive(Debug, Clone, PartialEq)]
pub enum MediaType { Movie, Show, Season, Episode }

#[derive(Debug, Clone, PartialEq)]
pub enum SourceType { Plex, Local }

#[derive(Debug, Clone)]
pub struct MediaItem {
    pub id: String,                  // "plex:{base_url}:{rating_key}"
    pub source_type: SourceType,
    pub source_id: String,           // server URL
    pub external_id: String,         // rating key
    pub media_type: MediaType,
    pub title: String,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub content_rating: Option<String>,
    pub rating: Option<f64>,
    pub runtime_minutes: Option<i32>,
    pub poster_path: Option<String>, // relative Plex path (e.g. /library/metadata/123/thumb/...)
    pub backdrop_path: Option<String>,
    pub genres: Vec<String>,
    pub parent_id: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub air_date: Option<String>,
    pub file_path: Option<String>,   // Plex part key for playback URL
    pub added_at: String,
    pub updated_at: String,
}
```

**Tests**: display_title() formatting, SourceConfig serde round-trip, MediaType equality

**Deferred fields** (M3/M4): sort_title, original_title, collection_id, collection_name, file_size, video_codec, audio_codec, resolution

---

### Phase 2: Database Layer

SQLite schema and repository CRUD for media items and sources.

**Files**:
- `src/db/mod.rs` — module declarations + `init_db(conn)`
- `src/db/error.rs` — `DbError` enum
- `src/db/schema.rs` — SQL CREATE TABLE statements
- `src/db/media_repo.rs` — `MediaRepo` CRUD
- `src/db/source_repo.rs` — `SourceRepo` CRUD

```rust
// src/db/schema.rs — init_db creates:
// - schema_version (version INTEGER)
// - media_items (all MediaItem fields, TEXT PRIMARY KEY on id)
// - sources (id, source_type, name, config JSON, enabled, last_synced_at)
// - Indexes: media_type, parent_id, source_type+source_id, added_at DESC
```

```rust
// src/db/media_repo.rs
pub struct MediaRepo { conn: Connection }
impl MediaRepo {
    pub fn new(conn: Connection) -> Self;
    pub fn upsert(&self, item: &MediaItem) -> Result<(), DbError>;
    pub fn find_by_id(&self, id: &str) -> Result<Option<MediaItem>, DbError>;
    pub fn list_by_type(&self, media_type: MediaType, limit: usize, offset: usize) -> Result<Vec<MediaItem>, DbError>;
    pub fn list_by_parent(&self, parent_id: &str) -> Result<Vec<MediaItem>, DbError>;
    pub fn delete_by_source(&self, source_type: &SourceType, source_id: &str) -> Result<usize, DbError>;
    pub fn count_by_type(&self, media_type: MediaType) -> Result<usize, DbError>;
}
```

```rust
// src/db/source_repo.rs
pub struct SourceRepo { conn: Connection }
impl SourceRepo {
    pub fn new(conn: Connection) -> Self;
    pub fn insert(&self, source: &Source) -> Result<(), DbError>;
    pub fn find_by_id(&self, id: &str) -> Result<Option<Source>, DbError>;
    pub fn list(&self) -> Result<Vec<Source>, DbError>;
    pub fn update(&self, source: &Source) -> Result<(), DbError>;
    pub fn delete(&self, id: &str) -> Result<(), DbError>;
}
```

**Tests** (~20, all use `Connection::open_in_memory()`):
- init_db succeeds, is idempotent, sets schema_version=1
- upsert → find_by_id round-trip
- upsert existing id updates (not duplicates)
- list_by_type returns only matching type, respects limit/offset
- list_by_parent returns children
- find_by_id returns None for missing
- delete_by_source removes correct items
- genres stored as JSON array, deserialized correctly
- Source insert/find/list/update/delete round-trips

---

### Phase 3: Plex API Response Models + Fixtures

Serde types for Plex JSON responses, tested with real fixture files.

**Files**:
- `src/services/plex/mod.rs`
- `src/services/plex/models.rs` — Plex API response serde types
- `src/services/plex/convert.rs` — PlexMetadata → MediaItem conversion
- `tests/common/mod.rs` — `load_fixture()` helper
- `tests/fixtures/plex/libraries.json`
- `tests/fixtures/plex/movies.json`
- `tests/fixtures/plex/movie_metadata.json`
- `tests/fixtures/plex/shows.json`
- `tests/fixtures/plex/seasons.json`
- `tests/fixtures/plex/episodes.json`

**Critical serde detail**: Plex uses different child array names per endpoint:

```json
// /library/sections → "Directory" array
{ "MediaContainer": { "Directory": [...] } }

// /library/sections/1/all → "Metadata" array
{ "MediaContainer": { "Metadata": [...] } }
```

Use separate container types (not generics) to handle this cleanly:

```rust
#[derive(Debug, Deserialize)]
pub struct PlexLibraryResponse {
    #[serde(rename = "MediaContainer")]
    pub media_container: PlexLibraryContainer,
}
#[derive(Debug, Deserialize)]
pub struct PlexLibraryContainer {
    pub size: Option<i32>,
    #[serde(default, rename = "Directory")]
    pub directories: Vec<PlexLibrary>,
}

#[derive(Debug, Deserialize)]
pub struct PlexMetadataResponse {
    #[serde(rename = "MediaContainer")]
    pub media_container: PlexMetadataContainer,
}
#[derive(Debug, Deserialize)]
pub struct PlexMetadataContainer {
    pub size: Option<i32>,
    #[serde(default, rename = "Metadata")]
    pub metadata: Vec<PlexMetadata>,
}
```

```rust
// src/services/plex/convert.rs
pub fn plex_metadata_to_media_item(
    metadata: &PlexMetadata,
    source_id: &str,
) -> MediaItem
// Maps: ratingKey→external_id, type→media_type, summary→overview,
//        duration(ms)→runtime_minutes, Genre tags→genres,
//        thumb→poster_path, art→backdrop_path,
//        Part.key→file_path, addedAt(unix)→added_at
```

**Tests** (~15):
- Deserialize each fixture file, verify key fields
- Missing optional fields don't panic
- Unknown extra fields ignored (forward-compatible)
- Conversion maps all fields correctly
- Duration ms → runtime minutes
- Genres extracted from PlexTag list
- Episode/season numbers populated for episodes

---

### Phase 4: Plex HTTP Client

HTTP client with proper headers, error handling, tested with wiremock.

**Files**:
- `src/services/plex/error.rs` — `PlexError` enum
- `src/services/plex/api.rs` — `PlexClient`

```rust
#[derive(Debug, thiserror::Error)]
pub enum PlexError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Deserialization error: {0}")]
    Deserialize(#[from] serde_json::Error),
    #[error("Unauthorized: invalid or expired token")]
    Unauthorized,
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Server error: {status} - {message}")]
    Server { status: u16, message: String },
}
```

```rust
pub struct PlexClient {
    http: reqwest::Client,
    base_url: String,
    auth_token: String,
    client_identifier: String,
}

impl PlexClient {
    pub fn new(base_url: &str, auth_token: &str) -> Self;
    pub async fn test_connection(&self) -> Result<(), PlexError>;
    pub async fn libraries(&self) -> Result<Vec<PlexLibrary>, PlexError>;
    pub async fn library_items(&self, library_key: &str) -> Result<Vec<PlexMetadata>, PlexError>;
    pub async fn metadata(&self, rating_key: &str) -> Result<PlexMetadata, PlexError>;
    pub async fn children(&self, rating_key: &str) -> Result<Vec<PlexMetadata>, PlexError>;
    pub fn playback_url(&self, part_key: &str) -> String;       // pure, not async
    pub fn transcode_image_url(&self, path: &str, width: u32, height: u32) -> String; // pure
}
```

All requests include headers: `X-Plex-Token`, `X-Plex-Client-Identifier`, `X-Plex-Product: Reel`, `Accept: application/json`

Internal `get()` helper maps: 401 → Unauthorized, 404 → NotFound, 5xx → Server

**Tests** (~15, all use wiremock `MockServer`):
- test_connection succeeds/fails (200/401)
- libraries() returns parsed data from fixture
- library_items() returns parsed metadata list
- metadata() returns single item
- children() returns children list
- playback_url() constructs correct URL with token
- transcode_image_url() constructs correct transcoder URL
- Handles 404, 500, timeout, malformed JSON
- Plex headers included in requests (verified via wiremock matchers)

---

### Phase 5: MediaSource Trait + PlexSource

Abstract interface and Plex implementation.

**Files**:
- `src/services/media_source.rs` — MediaSource trait + SourceError
- `src/services/plex/source.rs` — PlexSource: impl MediaSource

```rust
// Scoped for M2 — smaller than full tech.md version
#[async_trait]
pub trait MediaSource: Send + Sync {
    fn name(&self) -> &str;
    fn source_type(&self) -> SourceType;
    async fn test_connection(&self) -> Result<(), SourceError>;
    async fn libraries(&self) -> Result<Vec<LibrarySection>, SourceError>;
    async fn movies(&self) -> Result<Vec<MediaItem>, SourceError>;
    async fn shows(&self) -> Result<Vec<MediaItem>, SourceError>;
    async fn seasons(&self, show_id: &str) -> Result<Vec<MediaItem>, SourceError>;
    async fn episodes(&self, show_id: &str, season_number: u32) -> Result<Vec<MediaItem>, SourceError>;
    async fn playback_uri(&self, media_id: &str) -> Result<String, SourceError>;
    fn artwork_url(&self, path: &str, width: u32, height: u32) -> String;
}
```

**Deferred methods** (M3/M4): search, report_progress, set_watched, collections, recently_added, continue_watching

PlexSource delegates to PlexClient and converts Plex types → app models via `convert.rs`.

**Tests** (~10): delegates correctly, error mapping, trait object works

---

### Phase 6: Artwork Cache

Download and cache poster/backdrop images to disk.

**Files**:
- `src/services/artwork.rs` — ArtworkCache + ArtworkError

```rust
pub struct ArtworkCache {
    cache_dir: PathBuf,
    http: reqwest::Client,
}

impl ArtworkCache {
    pub fn new(cache_dir: PathBuf) -> Self;
    pub async fn get_or_download(&self, url: &str) -> Result<PathBuf, ArtworkError>;
    pub fn cached_path(&self, url: &str) -> Option<PathBuf>;
    pub fn path_for_url(&self, url: &str) -> PathBuf;  // sha256(url) truncated
    pub fn clear(&self) -> Result<(), ArtworkError>;
}
```

Cache key: `sha256(url)` truncated to 16 hex chars, preserving original file extension.

**Tests** (~8, use wiremock + tempfile):
- path_for_url is deterministic
- Different URLs → different paths
- get_or_download downloads and caches (verify file on disk)
- Second call uses cache (wiremock verify request count = 1)
- Download failure returns ArtworkError

---

### Phase 7: Navigation Shell (app.rs Restructure)

Transform from single-view player to multi-view navigation shell.

**Files**:
- `src/app.rs` — restructure to gtk::Stack + AdwNavigationSplitView
- `src/components/sidebar.rs` — new Sidebar component
- `src/components/mod.rs` — add new module declarations
- `src/navigation.rs` — NavigationTarget, CurrentView enums

**App struct change**:

```rust
// Before (M1): single VideoArea
pub struct App {
    video_area: Controller<VideoArea>,
    screensaver: ScreensaverInhibitor,
}

// After (M2): navigation shell + player
pub struct App {
    stack: gtk::Stack,                    // "shell" vs "player"
    nav_view: adw::NavigationView,        // content drill-down
    sidebar: Controller<Sidebar>,
    library_view: Controller<LibraryView>,
    player_view: Controller<PlayerView>,
    screensaver: ScreensaverInhibitor,
    current_view: CurrentView,
}
```

**Sidebar**: `SimpleComponent` with `gtk::ListBox` using `"navigation-sidebar"` CSS class. Rows for Movies and TV Shows. Emits `SidebarOutput::Navigate(target)`.

**Message flow**:

```
AppMsg::Navigate(target)        ← Sidebar
AppMsg::ShowMovieDetail(id)     ← LibraryView
AppMsg::ShowShowDetail(id)      ← LibraryView
AppMsg::PlayMedia(uri)          ← DetailViews
AppMsg::PlayerExited            ← PlayerView
AppMsg::GoBack                  ← AdwNavigationView pop
```

**Deliverable**: Sidebar appears, clicking Movies/TV Shows changes header title, player still works when activated via `gtk::Stack` switch.

---

### Phase 8: Library Grid View

Poster grid with async data loading from Plex.

**Files**:
- `src/components/library/mod.rs` — LibraryView component
- `src/components/library/media_card.rs` — MediaCardData + RelmGridItem

```rust
// media_card.rs
pub struct MediaCardData {
    pub media_id: String,
    pub title: String,
    pub year: Option<i32>,
    pub poster_texture: Option<gdk::Texture>,
    pub media_type: MediaType,
}

impl RelmGridItem for MediaCardData {
    // Card layout: poster Picture (180×270) + title Label + year Label
    // On bind: set title, year, poster texture (or placeholder)
}
```

```rust
// LibraryView component
pub enum LibraryViewMsg {
    LoadLibrary(LibraryType),
    ItemActivated(u32),
    LibraryLoaded(Vec<MediaCardData>),
    LoadError(String),
}
pub enum LibraryViewOutput {
    ShowMovieDetail(String),
    ShowShowDetail(String),
    Error(String),
}
```

Uses `sender.oneshot_command()` to fetch from PlexSource asynchronously. Shows `AdwStatusPage` for loading/empty states. `TypedGridView` for virtual scrolling.

**Artwork loading (M2 approach)**: Pre-load poster textures during library fetch via ArtworkCache. Store `gdk::Texture` in `MediaCardData`. For M3, can optimize with lazy loading for visible items only.

---

### Phase 9: Movie Detail View

Detail page with metadata and Play button.

**Files**:
- `src/components/detail/mod.rs`
- `src/components/detail/movie_detail.rs`

Layout: `AdwClamp` wrapping vertical box with:
- Backdrop `gtk::Picture` (360px height)
- Title + Year row
- Metadata row (rating badge, runtime, content rating, genre tags)
- Play button (`"suggested-action"` + `"pill"` classes)
- Synopsis label (wrapped text)

Pushed onto `AdwNavigationView` when user clicks a poster in the grid. Back navigation is automatic via AdwNavigationView.

```rust
pub enum MovieDetailMsg {
    LoadMovie(String),          // media_id
    Play,
    MetadataLoaded(MovieData),
    BackdropLoaded(gdk::Texture),
    LoadError(String),
}
pub enum MovieDetailOutput {
    PlayMedia(String),          // playback URI
    Error(String),
}
```

---

### Phase 10: TV Show Detail View

Show info + season selector + episode list.

**Files**:
- `src/components/detail/show_detail.rs`

Layout: Show metadata header + `gtk::DropDown` for season selection + `gtk::ListBox` of episode rows.

Each episode row: `adw::ActionRow` with episode number, title, air date, and play button.

Season selection triggers async fetch of episodes for that season via `PlexSource::episodes()`.

```rust
pub enum ShowDetailMsg {
    LoadShow(String),
    SelectSeason(usize),
    PlayEpisode(String),
    ShowLoaded(ShowData, Vec<SeasonData>),
    EpisodesLoaded(Vec<EpisodeData>),
    LoadError(String),
}
pub enum ShowDetailOutput {
    PlayMedia(String),
    Error(String),
}
```

---

### Phase 11: Plex Connection UI

Settings window for server configuration.

**Files**:
- `src/components/connection.rs` — ConnectionDialog

Uses `adw::PreferencesWindow` (available in libadwaita 1.4, which we target) with:
- `adw::EntryRow` for server URL
- `adw::PasswordEntryRow` for auth token
- "Test Connection" button with success/error feedback
- Save/Cancel actions

```rust
pub enum ConnectionDialogMsg {
    TestConnection,
    TestResult(Result<String, String>),  // Ok(server_name) / Err(message)
    Save,
    Cancel,
}
pub enum ConnectionDialogOutput {
    ConnectionSaved { url: String, token: String, name: String },
    Cancelled,
}
```

Connection config persisted via SourceRepo (stored in SQLite sources table).

---

### Phase 12: Integration + Polish

Wire all components together and verify end-to-end flows.

- First-run: empty state → connection dialog → library loads
- Movies: sidebar → grid → detail → play → back
- TV Shows: sidebar → grid → show detail → season → episode → play → back
- Error handling: connection failures show toast, network errors show inline message
- CSS polish: media-card styling, backdrop rounded corners, grid spacing

---

## New File Summary

### Backend (18 files)

```
src/config.rs
src/models/mod.rs
src/models/media.rs
src/models/source.rs
src/models/library.rs
src/db/mod.rs
src/db/error.rs
src/db/schema.rs
src/db/media_repo.rs
src/db/source_repo.rs
src/services/plex/mod.rs
src/services/plex/models.rs
src/services/plex/convert.rs
src/services/plex/error.rs
src/services/plex/api.rs
src/services/plex/source.rs
src/services/media_source.rs
src/services/artwork.rs
```

### UI (8 files)

```
src/navigation.rs
src/components/sidebar.rs
src/components/library/mod.rs
src/components/library/media_card.rs
src/components/detail/mod.rs
src/components/detail/movie_detail.rs
src/components/detail/show_detail.rs
src/components/connection.rs
```

### Test Infrastructure (8 files)

```
tests/common/mod.rs
tests/fixtures/plex/libraries.json
tests/fixtures/plex/movies.json
tests/fixtures/plex/movie_metadata.json
tests/fixtures/plex/shows.json
tests/fixtures/plex/seasons.json
tests/fixtures/plex/episodes.json
```

### Modified (3 files)

```
Cargo.toml
src/main.rs                  (add mod declarations)
src/services/mod.rs          (add pub mod plex, media_source, artwork)
src/components/mod.rs        (add pub mod sidebar, library, detail, connection)
src/style.css                (add media-card, backdrop, grid CSS)
```

## Existing Code to Reuse

- `src/player/backend.rs` — PlayState, EndReason enums (used by PlayerView)
- `src/player/playback_tracker.rs` — state machine pattern (reference for new state machines)
- `src/components/player/` — entire player subsystem (wrapped in PlayerView)
- `src/services/screensaver.rs` — ScreensaverInhibitor (moved to PlayerView)
- `src/services/window_state.rs` — persistence pattern (reference for connection config)
- `src/components/player/shortcuts.rs` — pure function testing pattern
- `src/components/player/overlay_controller.rs` — state machine for UI logic

## Edge Cases & Error Handling

### Connection & Auth
- **No Plex configured**: Show `AdwStatusPage` with "Add Plex Server" prompt
- **Connection refused / timeout**: Toast notification with error message
- **401 Unauthorized**: Clear feedback that token is invalid, prompt to re-enter
- **Token storage**: Plaintext in SQLite `sources` table for M2 (same as Plex Desktop). Keyring integration deferred to M5
- **URL format variations**: Normalize user input — strip trailing slash, default to `http://` if no scheme, validate port

### Library & Data
- **Large libraries (10K+ items)**: TypedGridView virtual scrolling handles rendering. Fetch first page (~50 items) immediately, continue syncing in background
- **Multiple libraries of same type**: Fetch from all movie libraries of same type, combine results in grid
- **Missing artwork**: Two-state placeholder: spinner while loading, then styled "no artwork" card with title text on permanent failure
- **Plex server goes offline mid-browse**: Show error toast, cached data still visible in grid
- **Multiple media versions** (1080p + 4K): Play the first listed Part. Version selector deferred to M3

### TV Shows
- **Season 0 / Specials**: Display as "Specials" in season selector, shown last after numbered seasons
- **TV show with no seasons**: Show empty state on detail page
- **Episodes without files**: Show row but disable play button, dim the row

### Playback from Plex
- **Direct play URL**: `http://{server}:{port}{part_key}?X-Plex-Token={token}` where `part_key` comes from `PlexPart.key` in metadata response
- **Network buffering**: mpv handles HTTP buffering internally. No `PlayState::Buffering` needed for M2 — mpv shows last frame while buffering. Revisit if user reports warrant it
- **Transcoding**: Not supported in M2. If direct play fails (codec not supported), show error toast explaining the limitation

### Navigation & Integration
- **CLI file argument**: Still works — `reel /path/to/file.mkv` bypasses library and goes straight to player view. Back exits the app (no library to return to)
- **Keyboard shortcut context**: Shortcuts are view-context-aware. In player: M1 shortcuts (space=pause, arrows=seek). In library: arrows navigate grid, Enter opens detail, Backspace goes back. The `gtk::Stack` visible child determines context
- **Drag-and-drop in library views**: Still works — dropping a video file switches to player view and plays it
- **Concurrent navigation**: Cancel previous async command when new navigation occurs
- **Network latency**: Show loading spinner during API calls, grid items appear as loaded

## Dependency Order

```
Phase 1 (deps + config + models)
  │
  ├──→ Phase 2 (database)
  │         │
  ├──→ Phase 3 (plex serde models)  ──→  Phase 4 (plex HTTP client)
  │                                              │
  ├──→ Phase 6 (artwork cache)                   │
  │                                              │
  │         └──────────────┬─────────────────────┘
  │                        │
  │                  Phase 5 (MediaSource + PlexSource)
  │                        │
  ├──→ Phase 7 (nav shell) │
  │         │              │
  │         ├──→ Phase 8 (library grid) ←── uses PlexSource + ArtworkCache
  │         │         │
  │         │    Phase 9 (movie detail) + Phase 10 (show detail)
  │         │
  │         └──→ Phase 11 (connection UI) ←── uses PlexClient
  │
  └──→ Phase 12 (integration + polish)
```

Phases 2, 3, 6, and 7 can proceed in parallel after Phase 1.

## Estimated Test Count

| Phase | Tests |
|-------|-------|
| 1 (config + models) | ~15 |
| 2 (database) | ~20 |
| 3 (plex serde) | ~15 |
| 4 (plex client) | ~15 |
| 5 (MediaSource) | ~10 |
| 6 (artwork) | ~8 |
| **Total new** | **~83** |
| **Existing** | **121** |
| **Grand total** | **~204** |

UI components (Phases 7-12) are tested manually (GTK components cannot be unit tested without a display).

## Verification Plan

### Automated

```bash
nix develop -c cargo test              # All unit tests pass
nix develop -c cargo clippy            # Zero warnings
nix develop -c cargo fmt -- --check    # Formatted
```

### Manual Testing

1. **First run**: Launch with no config → see empty state → add Plex server → library loads
2. **Movie browsing**: Click Movies → grid of posters → scroll → click poster → detail page → Play → video plays → exit player → back to detail
3. **TV browsing**: Click TV Shows → grid → click show → season selector → episode list → play episode → exit
4. **Error handling**: Disconnect network → see error toast → reconnect → retry works
5. **Large library**: Test with 500+ movie library → smooth scrolling
6. **Invalid credentials**: Enter wrong token → connection test fails with clear message

## Sources & References

### Internal

- `roadmap.md:92-137` — M2 specification
- `tech.md:257-306` — MediaSource trait design
- `tech.md:310-333` — PlexClient API design
- `tech.md:800-892` — Database schema
- `tech.md:179-233` — Component hierarchy and message flow
- `product.md:99-195` — Library management features

### External

- Plex Media Server API: community-documented endpoints (plex.tv)
- Relm4 0.10 TypedGridView: relm4 docs
- libadwaita 1.4 widgets: GNOME developer docs
