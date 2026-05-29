# Reel - Development Guide

## Project Overview

Reel is a native **Plex and Jellyfin client for the GNOME desktop**, built with Rust, GTK4, Relm4, libadwaita, and GStreamer. It is **not** a generic local media player — it is a media-server client whose job is to connect to Plex/Jellyfin servers, browse their libraries, and play their content with proper watch-state sync, server-side transcoding, and offline downloads.

It follows **GNOME's libadwaita design conventions** throughout: the UI should look and behave like a first-class GNOME app (adaptive layouts, libadwaita widgets/styling, GNOME HIG patterns). When adding or changing UI, prefer adwaita components and conventions over custom chrome. Local file playback exists as a convenience, but the product is defined by the server-client experience, not standalone playback.

**Architecture**: Relm4 (Elm/MVU) components -> Service layer -> Backend layer (GStreamer, SQLite, HTTP APIs).

**Playback backend (read this before touching player code)**: Video plays through GStreamer `playbin3` with a `gtk4paintablesink`, wrapped by `PlaybackPipeline` in `src/player/gst_pipeline.rs` and driven by the `VideoPlayer` Relm4 component in `src/components/player/video_player.rs`. There is **no libmpv** in this codebase, and no `MpvBackend`, `VideoBackend` trait, or `playback_tracker.rs`. The TDD examples further down that reference mpv, `MpvBackend`, `VideoBackend`, `VideoArea`, or `playback_tracker.rs` are illustrative of testing *patterns* (extract pure functions, mock at a boundary), not the real modules — the live state-tracking equivalent is the pure tracker in `src/services/watch_state.rs`.

## Build & Run

```bash
# Enter dev shell (required - provides all native deps)
nix develop

# Build (check for error count on last line)
nix develop -c cargo build

# Check without building
nix develop -c cargo check

# Run with a video file
nix develop -c cargo run -- /path/to/video.mkv

# Run clippy
nix develop -c cargo clippy

# Format
nix develop -c cargo fmt
```

**Important**: Always use `nix develop -c` prefix for build/check commands. Always check the LAST LINE of build output for error count.

## Running Tests

```bash
# Unit tests (no display needed)
nix develop -c cargo test

# Unit tests for a specific module
nix develop -c cargo test services::watch_state

# Tests with output visible
nix develop -c cargo test -- --nocapture

# Run tests sequentially (required for any GTK tests)
nix develop -c cargo test -- --test-threads=1

# GTK integration tests (need virtual display)
nix develop -c xvfb-run --auto-servernum cargo test -- --test-threads=1

# Integration tests requiring GStreamer/display
nix develop -c xvfb-run --auto-servernum cargo test --features integration

# Continuous testing during development
nix develop -c cargo watch -x test
```

## Project Structure

```
src/
  main.rs                          # Entry point
  app.rs                           # Root App component (Relm4)
  player/
    mod.rs                         # PlayState enum, SkipMarkers, window_title_for_state
    gst_pipeline.rs                # PlaybackPipeline (GStreamer playbin3 + gtk4paintablesink)
    pipeline_msgs.rs               # Pipeline bus message types
    subtitles.rs                   # Subtitle handling
    tracks.rs                      # Audio/subtitle track models
  components/
    mod.rs
    player/
      mod.rs
      video_player.rs              # VideoPlayer Relm4 component (drives PlaybackPipeline)
    # Future: sidebar.rs, library/, detail/, search.rs, settings/
  # Future: models/, services/, db/
tests/
  common/
    mod.rs                         # Shared test helpers (NOT compiled as test crate)
  fixtures/
    plex/                          # Plex API response JSON files
    tmdb/                          # TMDb API response JSON files
```

---

## Test-Driven Development Guide

### TDD Philosophy for This Project

Every feature follows **Red-Green-Refactor**:

1. **RED**: Write a failing test (or a test that doesn't compile -- in Rust, compilation failure IS the failing test)
2. **GREEN**: Write the minimum code to make it pass
3. **REFACTOR**: Clean up while tests stay green

The Rust compiler is your first test runner. A type error or missing match arm is a failing test.

### What to Test (and What Not To)

#### Always Test (Unit Tests, No GTK)
- **Pure functions**: formatting, parsing, state derivation, data transformations
- **State machines**: PlaybackTracker transitions, watch state logic
- **Service layer logic**: library sync, metadata resolution, watch progress
- **Data models**: serialization/deserialization, validation, display formatting
- **Repository operations**: CRUD against in-memory SQLite
- **HTTP API response parsing**: serde deserialization of Plex/TMDb JSON
- **Error handling paths**: every error variant should have a test

#### Test with Mocks
- **Service layer with dependencies**: mock repositories, mock HTTP clients
- **Components with backend deps**: mock VideoBackend trait
- **Message routing logic**: effect-as-data pattern for Relm4 update()

#### Do NOT Unit Test (Manual/Visual Only)
- GTK widget layout, spacing, CSS rendering
- OpenGL rendering (gl_render.rs FFI code)
- Video playback quality, frame timing
- Hardware acceleration behavior
- Drag-and-drop, animations, transitions

---

### Architecture Patterns for Testability

#### Pattern 1: Extract Pure Functions from Components

The `update()` method on Relm4 components cannot be tested without GTK. Extract all logic into pure functions that take data in, return data out.

```rust
// BAD: Logic buried in update()
fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
    match msg {
        AppMsg::VideoOutput(VideoAreaOutput::StateChanged(state)) => {
            let title = match state {
                PlayState::Playing => "Reel - Playing",
                PlayState::Paused => "Reel - Paused",
                PlayState::Stopped => "Reel",
            };
            root.set_title(Some(title));
        }
    }
}

// GOOD: Pure function extracted, update() is a thin dispatcher
pub fn window_title_for_state(state: PlayState) -> &'static str {
    match state {
        PlayState::Playing => "Reel - Playing",
        PlayState::Paused => "Reel - Paused",
        PlayState::Stopped => "Reel",
    }
}

fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>, root: &Self::Root) {
    match msg {
        AppMsg::VideoOutput(VideoAreaOutput::StateChanged(state)) => {
            root.set_title(Some(window_title_for_state(state)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_reflects_playing() {
        assert_eq!(window_title_for_state(PlayState::Playing), "Reel - Playing");
    }
}
```

#### Pattern 2: PlaybackTracker (Testable State Machine)

The PollState logic in VideoArea is the most complex code in the app. Extract it into a standalone struct with no GTK/Relm4 dependencies.

```rust
// src/player/playback_tracker.rs

use crate::player::backend::{PlayState, EndReason};

/// Polled property values from the video backend.
#[derive(Debug, Clone, Default)]
pub struct PollData {
    pub path: Option<String>,
    pub duration: Option<f64>,
    pub position: Option<f64>,
    pub paused: Option<bool>,
    pub eof_reached: Option<bool>,
    pub hwdec_current: Option<String>,
}

/// Events produced by processing poll data. These are data, not side effects.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackEvent {
    FileLoaded { path: String, duration: f64, hwdec: Option<String> },
    PositionChanged { position: f64, duration: f64 },
    StateChanged(PlayState),
    EndOfFile(EndReason),
}

/// Pure state tracker. No I/O, no GTK, no mpv. Fully testable.
pub struct PlaybackTracker {
    pub file_loaded: bool,
    pub last_position: f64,
    pub last_paused: Option<bool>,
}

impl PlaybackTracker {
    pub fn new() -> Self {
        Self {
            file_loaded: false,
            last_position: -1.0,
            last_paused: None,
        }
    }

    /// Process polled backend state, return events to emit.
    /// This is a pure function with no side effects.
    pub fn process(&mut self, data: &PollData) -> Vec<PlaybackEvent> {
        let mut events = Vec::new();

        // File loaded detection
        if !self.file_loaded {
            if let (Some(ref path), Some(dur)) = (&data.path, data.duration) {
                if !path.is_empty() && dur > 0.0 {
                    self.file_loaded = true;
                    events.push(PlaybackEvent::FileLoaded {
                        path: path.clone(),
                        duration: dur,
                        hwdec: data.hwdec_current.clone(),
                    });
                }
            }
        }

        // Position change (threshold: 50ms)
        if let Some(pos) = data.position {
            if (pos - self.last_position).abs() > 0.05 {
                self.last_position = pos;
                events.push(PlaybackEvent::PositionChanged {
                    position: pos,
                    duration: data.duration.unwrap_or(0.0),
                });
            }
        }

        // Pause state change
        if let Some(paused) = data.paused {
            if self.last_paused != Some(paused) {
                self.last_paused = Some(paused);
                let state = if paused { PlayState::Paused } else { PlayState::Playing };
                events.push(PlaybackEvent::StateChanged(state));
            }
        }

        // EOF
        if data.eof_reached == Some(true) && self.file_loaded {
            self.file_loaded = false;
            events.push(PlaybackEvent::EndOfFile(EndReason::Finished));
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playing_state(position: f64) -> PollData {
        PollData {
            path: Some("/test.mkv".into()),
            duration: Some(100.0),
            position: Some(position),
            paused: Some(false),
            eof_reached: Some(false),
            hwdec_current: None,
        }
    }

    #[test]
    fn detects_file_loaded() {
        let mut t = PlaybackTracker::new();
        let events = t.process(&playing_state(0.0));
        assert!(events.iter().any(|e| matches!(e, PlaybackEvent::FileLoaded { .. })));
        assert!(t.file_loaded);
    }

    #[test]
    fn no_duplicate_file_loaded() {
        let mut t = PlaybackTracker::new();
        t.process(&playing_state(0.0));
        let events = t.process(&playing_state(1.0));
        assert!(!events.iter().any(|e| matches!(e, PlaybackEvent::FileLoaded { .. })));
    }

    #[test]
    fn pause_transition() {
        let mut t = PlaybackTracker::new();
        t.process(&playing_state(0.0)); // starts playing
        let mut data = playing_state(1.0);
        data.paused = Some(true);
        let events = t.process(&data);
        assert!(events.contains(&PlaybackEvent::StateChanged(PlayState::Paused)));
    }

    #[test]
    fn no_state_change_when_unchanged() {
        let mut t = PlaybackTracker::new();
        t.process(&playing_state(0.0));
        let events = t.process(&playing_state(1.0));
        assert!(!events.iter().any(|e| matches!(e, PlaybackEvent::StateChanged(_))));
    }

    #[test]
    fn position_below_threshold_ignored() {
        let mut t = PlaybackTracker::new();
        t.process(&playing_state(10.0));
        let events = t.process(&playing_state(10.03)); // < 0.05
        assert!(!events.iter().any(|e| matches!(e, PlaybackEvent::PositionChanged { .. })));
    }

    #[test]
    fn eof_resets_file_loaded() {
        let mut t = PlaybackTracker::new();
        t.process(&playing_state(0.0));
        let mut data = playing_state(100.0);
        data.eof_reached = Some(true);
        let events = t.process(&data);
        assert!(events.contains(&PlaybackEvent::EndOfFile(EndReason::Finished)));
        assert!(!t.file_loaded);
    }
}
```

Then VideoArea.update() delegates to it:
```rust
VideoAreaMsg::PollState => {
    let poll_data = /* gather from mpv */;
    for event in self.tracker.process(&poll_data) {
        match event {
            PlaybackEvent::FileLoaded { .. } => { sender.output(VideoAreaOutput::FileLoaded).ok(); }
            PlaybackEvent::PositionChanged { position, duration } => { sender.output(VideoAreaOutput::PositionChanged { position, duration }).ok(); }
            PlaybackEvent::StateChanged(state) => { sender.output(VideoAreaOutput::StateChanged(state)).ok(); }
            PlaybackEvent::EndOfFile(reason) => { sender.output(VideoAreaOutput::EndOfFile(reason)).ok(); }
        }
    }
}
```

#### Pattern 3: VideoBackend Trait (Mock the FFI Boundary)

Define a trait for video backend operations. The UI never touches libmpv directly.

```rust
// src/player/backend.rs

#[cfg_attr(test, mockall::automock)]
pub trait VideoBackend: Send {
    fn load_file(&self, uri: &str) -> Result<(), BackendError>;
    fn toggle_pause(&self) -> Result<(), BackendError>;
    fn seek_absolute(&self, position: f64) -> Result<(), BackendError>;
    fn seek_relative(&self, offset: f64) -> Result<(), BackendError>;
    fn set_volume(&self, volume: f64) -> Result<(), BackendError>;
    fn set_speed(&self, speed: f64) -> Result<(), BackendError>;
    fn stop(&self) -> Result<(), BackendError>;

    fn poll_state(&self) -> PollData;
    fn position(&self) -> Option<f64>;
    fn duration(&self) -> Option<f64>;
    fn is_paused(&self) -> bool;
}
```

MpvBackend implements it. Tests use MockVideoBackend (auto-generated by mockall).

#### Pattern 4: MediaSource Trait (Mock HTTP APIs)

```rust
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait MediaSource: Send + Sync {
    async fn movies(&self) -> Result<Vec<Movie>, SourceError>;
    async fn shows(&self) -> Result<Vec<TvShow>, SourceError>;
    async fn playback_uri(&self, media_id: &str) -> Result<String, SourceError>;
    async fn report_progress(&self, media_id: &str, position: Duration, duration: Duration) -> Result<(), SourceError>;
}
```

#### Pattern 5: Repository Trait (Mock the Database)

```rust
pub trait MovieRepository {
    fn insert(&self, movie: &NewMovie) -> Result<Movie, RepoError>;
    fn find_by_id(&self, id: i64) -> Result<Option<Movie>, RepoError>;
    fn find_by_plex_key(&self, key: &str) -> Result<Option<Movie>, RepoError>;
    fn list(&self, limit: usize, offset: usize) -> Result<Vec<Movie>, RepoError>;
}
```

Real impl uses rusqlite. Tests use in-memory SQLite or hand-written fakes.

---

### Testing Each Layer

#### Layer 1: Data Models (src/models/)

Test with plain `#[test]`. No dependencies.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movie_display_title_with_year() {
        let movie = Movie { title: "Dune".into(), year: Some(2021), ..Default::default() };
        assert_eq!(movie.display_title(), "Dune (2021)");
    }

    #[test]
    fn movie_display_title_without_year() {
        let movie = Movie { title: "Unknown".into(), year: None, ..Default::default() };
        assert_eq!(movie.display_title(), "Unknown");
    }
}
```

Use builder pattern for test fixtures:
```rust
#[cfg(test)]
pub struct MovieBuilder { /* fields with defaults */ }

#[cfg(test)]
impl MovieBuilder {
    pub fn new(title: &str) -> Self { /* sensible defaults */ }
    pub fn year(mut self, y: i32) -> Self { self.year = Some(y); self }
    pub fn build(self) -> Movie { /* construct */ }
}
```

#### Layer 2: Service Layer (src/services/)

Test business logic with mocked dependencies.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;

    #[tokio::test]
    async fn sync_skips_existing_movie() {
        let mut repo = MockMovieRepository::new();
        repo.expect_find_by_plex_key()
            .with(eq("/library/metadata/123"))
            .returning(|_| Ok(Some(MovieBuilder::new("Existing").build())));
        repo.expect_insert().never(); // Must NOT insert

        let service = LibrarySyncService::new(repo);
        service.sync_movie("/library/metadata/123", &plex_data).unwrap();
    }
}
```

#### Layer 3: HTTP API Clients (src/services/plex/, tmdb/)

Use wiremock for HTTP mocking. Make base URL injectable.

```rust
pub struct PlexClient {
    http: reqwest::Client,
    base_url: String,  // Injectable for tests
    token: String,
}

impl PlexClient {
    pub fn new(base_url: &str, token: &str) -> Self { /* ... */ }
}

// Test:
#[tokio::test]
async fn fetches_libraries() {
    let server = MockServer::start().await;
    let body = load_fixture("plex/libraries.json");
    Mock::given(method("GET"))
        .and(path("/library/sections"))
        .and(header("X-Plex-Token", "test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server).await;

    let client = PlexClient::new(&server.uri(), "test-token");
    let libs = client.get_libraries().await.unwrap();
    assert_eq!(libs[0].title, "Movies");
}
```

Always test error paths: 401, 404, 429, 500, timeout, malformed JSON.

#### Layer 4: Database (src/db/)

Every test gets its own `Connection::open_in_memory()`.

```rust
fn test_db() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    crate::db::init_db(&mut conn).unwrap();
    conn
}

#[test]
fn insert_and_retrieve_movie() {
    let conn = test_db();
    let repo = SqliteMovieRepository::new(conn);
    let movie = repo.insert(&NewMovie { title: "Dune".into(), year: Some(2021), .. }).unwrap();
    let found = repo.find_by_id(movie.id).unwrap();
    assert_eq!(found.unwrap().title, "Dune");
}
```

Test migrations separately:
```rust
#[test]
fn all_migrations_valid() {
    crate::db::migrations().validate().unwrap();
}
```

#### Layer 5: Filename Parser (src/services/filename_parser.rs)

Pure functions, extensive tests, good candidate for property-based testing.

```rust
#[test]
fn parse_plex_style() {
    let info = parse_filename("Movie Name (2024).mkv");
    assert_eq!(info.title, "Movie Name");
    assert_eq!(info.year, Some(2024));
}

#[test]
fn parse_scene_style() {
    let info = parse_filename("Movie.Name.2024.1080p.BluRay.x264-GROUP.mkv");
    assert_eq!(info.title, "Movie Name");
    assert_eq!(info.year, Some(2024));
}

#[test]
fn parse_tv_episode() {
    let info = parse_filename("Show Name - S01E01 - Episode Title.mkv");
    assert_eq!(info.title, "Show Name");
    assert_eq!(info.season, Some(1));
    assert_eq!(info.episode, Some(1));
}

// Property test: any string with a 4-digit year in parens should extract it
proptest! {
    #[test]
    fn extracts_year_in_parens(year in 1900u32..2100) {
        let filename = format!("Some Movie ({year}).mkv");
        let info = parse_filename(&filename);
        assert_eq!(info.year, Some(year));
    }
}
```

#### Layer 6: Serde Deserialization

Test with real recorded API response fixtures.

```rust
#[test]
fn parse_real_plex_response() {
    let json = include_str!("../../tests/fixtures/plex/libraries.json");
    let container: PlexMediaContainer = serde_json::from_str(json).unwrap();
    assert!(!container.directories.is_empty());
}

#[test]
fn parse_with_missing_optional_fields() {
    let json = r#"{"id": 1, "title": "Minimal"}"#;
    let movie: TmdbMovie = serde_json::from_str(json).unwrap();
    assert_eq!(movie.overview, None);
}

#[test]
fn parse_fails_on_missing_required_field() {
    let json = r#"{"id": 1}"#; // missing title
    assert!(serde_json::from_str::<TmdbMovie>(json).is_err());
}
```

---

### Test File Organization

```
src/
  player/
    mod.rs                  # Unit tests for pure helpers (window_title_for_state)
    gst_pipeline.rs         # NO unit tests (needs real GStreamer/display)
    tracks.rs               # Unit tests for track models in #[cfg(test)]
  services/
    filename_parser.rs      # Extensive unit tests + proptest
    plex/
      api.rs                # Deserialization tests in #[cfg(test)]
      models.rs             # Serde tests in #[cfg(test)]
    metadata.rs             # Unit tests with mocked deps
  models/
    media.rs                # Unit tests for display/formatting
  db/
    schema.rs               # Migration validation tests
    media_repo.rs           # Unit tests with in-memory SQLite

tests/
  common/
    mod.rs                  # load_fixture(), builders, helpers
  fixtures/
    plex/
      libraries.json        # Real recorded Plex API response
      movie_metadata.json
    tmdb/
      movie_detail.json     # Real recorded TMDb API response
      search_results.json
  plex_client_test.rs       # Integration tests with wiremock
  tmdb_client_test.rs       # Integration tests with wiremock
  db_integration_test.rs    # Full database integration tests
```

### Test Naming Convention

Use `{action}_{scenario}_{expected}` pattern:

```rust
#[test] fn parse_plex_style_filename_extracts_title_and_year() { }
#[test] fn sync_existing_movie_skips_insert() { }
#[test] fn tracker_detects_pause_transition() { }
#[test] fn plex_client_handles_401_unauthorized() { }
#[test] fn migration_rollback_removes_columns() { }
```

---

### Dev Dependencies

```toml
[dev-dependencies]
mockall = "0.13"
wiremock = "0.6"
proptest = "1"
tokio = { version = "1", features = ["test-util", "macros", "rt-multi-thread"] }
tempfile = "3"

[features]
default = ["wayland", "x11"]
wayland = ["dep:gdk4-wayland"]
x11 = ["dep:gdk4-x11"]
integration = []  # Gate tests needing real GStreamer/display
```

### File Size Limits

- **Hard cap**: No `src/**/*.rs` file may exceed 2000 lines. Enforced by `tests/file_size_limits.rs` (`cargo test`).
- **Soft target**: Files over 800 lines should be split into focused submodules.
- **Grandfathered files**: Known-large files listed in the test's `GRANDFATHERED` array. When you split one, remove it from the list.
- **Per-function cap**: `clippy.toml` enforces `too-many-lines-threshold = 100` per function.
- **App component split**: `src/app/mod.rs` keeps the component skeleton (struct, enums, `init()`, `update()`, `update_cmd()`). Free functions live in `src/app/utils.rs`, `db_helpers.rs`, `dialogs.rs`, `player_ui.rs`, `source_validation.rs`, `watch_events.rs`. Widget construction in `widget_builder.rs`. Heavy message handlers in `handlers.rs`.

### TDD Workflow

1. Write test for the next behavior
2. `nix develop -c cargo test` -- confirm it fails (or doesn't compile)
3. Write minimum code to pass
4. `nix develop -c cargo test` -- confirm it passes
5. Refactor while green
6. `nix develop -c cargo clippy` -- no warnings
7. `nix develop -c cargo fmt` -- formatted
8. Commit

For continuous feedback: `nix develop -c cargo watch -x test`

### When Writing a New Module

1. Create the module file
2. Write the trait or type definition
3. Write tests for the first behavior
4. Implement until tests pass
5. Add the next behavior's test
6. Repeat

### Fixture Recording (For HTTP Tests)

Record real API responses for use as test fixtures:

```rust
#[tokio::test]
#[ignore] // Only run manually: cargo test record -- --ignored
async fn record_plex_libraries() {
    let token = std::env::var("PLEX_TOKEN").expect("PLEX_TOKEN required");
    let url = std::env::var("PLEX_URL").expect("PLEX_URL required");
    let resp = reqwest::get(format!("{url}/library/sections?X-Plex-Token={token}"))
        .await.unwrap().text().await.unwrap();
    let pretty: serde_json::Value = serde_json::from_str(&resp).unwrap();
    std::fs::write(
        "tests/fixtures/plex/libraries.json",
        serde_json::to_string_pretty(&pretty).unwrap(),
    ).unwrap();
}
```

---

## Key Architectural Rules

1. **No GStreamer calls outside player/**: All `playbin3`/GStreamer interaction goes through `PlaybackPipeline` (`src/player/gst_pipeline.rs`). The `VideoPlayer` component and App never import `gstreamer` directly.
2. **No GTK in services/**: The service layer is pure Rust. No gtk4::, no glib::, no relm4::.
3. **No business logic in update()**: Relm4 `update()` methods are thin dispatchers. Extract logic to pure functions or service calls.
4. **Traits at boundaries**: MediaSource, MovieRepository, and similar seams. Mock-friendly by design. (Playback is concrete today — `PlaybackPipeline` wraps GStreamer; there is no `VideoBackend` trait.)
5. **Errors as types, not strings**: Use thiserror enums. Each error variant testable via `matches!()`.

## Milestone-Specific Test Requirements

| Milestone | Test Focus |
|-----------|-----------|
| M0 (Skeleton) | PlaybackTracker state machine, PlayState/EndReason coverage |
| M1 (Player) | Keyboard shortcut mapping, time formatting, subtitle style defaults |
| M2 (Plex Core) | PlexClient with wiremock, serde for Plex responses, SQLite CRUD |
| M3 (Library UX) | Search scoring, filter logic, sort comparators, filename parser |
| M4 (Watch State) | Progress tracking thresholds, scrobble logic, resume logic |
| M5 (Polish) | MPRIS property derivation, error type coverage, settings validation |
| M6 (Standalone) | Filename parser (proptest), TMDb client, filesystem watcher events |
| M7 (Extensions) | Jellyfin/Emby clients, Trakt OAuth, OpenSubtitles hash computation |

## GTK4 CSS Rules

**Widget margins are non-negative**: GTK4 widget margin properties (`margin-bottom`, `margin-top`, `margin-start`, `margin-end`, `margin-left`, `margin-right`) take `gint` with minimum 0. Setting a negative value (e.g. `.margin_bottom(-30)`) causes a panic at widget build time. Use Overlay positioning instead of negative margins for overlapping layouts.

**CSS subset is limited**: GTK4's CSS parser supports only a subset of web CSS. Properties that cause warnings:

| Unsupported | Replacement |
|---|---|
| `margin-start`, `margin-end` | `margin-left`, `margin-right` |
| `overflow: hidden` | Not available; use `border-radius` without overflow |
| `transform`, `translateY()`, `scale()` | Not available in GTK CSS |
| `transition` | Not available in GTK CSS |
| `animation`, `@keyframes` | Not available in GTK CSS |
| `cursor` | Not available in GTK CSS |
| `pointer-events` | Not available in GTK CSS |
| `letter-spacing` | Not available in GTK CSS |
| `font-feature-settings` | Not available in GTK CSS |

**Supported properties**: `background*`, `border*`, `box-shadow`, `color`, `font-*`, `margin*` (non-negative), `min-*`, `opacity`, `outline*`, `padding*`, `text-shadow`, `-gtk-icon-shadow`, `-gtk-icon-transform`.
