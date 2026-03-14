---
title: "feat: M4 Watch State - Progress tracking, scrobble, and resume"
type: feat
status: completed
date: 2026-03-14
---

# M4: Watch State

## Context

M0-M3 are complete. The app is a full Plex library browser with poster grids, detail pages, search/filter/sort, collections, and a feature-rich video player with overlay controls, keyboard shortcuts, and subtitle support. There are ~390 tests, a fake Plex server for e2e testing, and in-memory SQLite for DB tests.

What's missing: the app has no memory of what the user has watched. `VideoAreaOutput::PositionChanged` is a no-op in `app.rs:429`. There is no watch progress table, no resume prompt, no scrobble logic, and no watched indicators on library cards. The `progress_bar` widget exists on `MediaCardWidgets` but is hidden.

M4 closes this gap. Users will see their watch progress reflected everywhere: resume prompts, library card indicators, a Continue Watching row, and bidirectional Plex sync.

## Overview

Track watch progress locally and sync with Plex. The core flow is: play media -> persist position periodically -> resume on re-open -> scrobble at 90% -> show watched/in-progress indicators in library UI -> Continue Watching row.

## Acceptance Criteria

### Functional

- [x] Watch progress persisted to SQLite every ~15 seconds during playback, plus on pause/stop/exit
- [x]Resume overlay appears when opening a file with saved progress (position > 30s and < 90%)
- [x]Resume overlay is non-modal, auto-dismisses after 5 seconds defaulting to resume
- [x]User can press Escape/click "Start Over" to play from beginning
- [x]Playback resumes at `max(0, saved_position - 10s)` for viewing context
- [x]Scrobble triggers when position crosses 90% of duration (once per session)
- [x]Scrobble marks item as watched locally and calls Plex `/:/scrobble` API
- [x]Plex timeline reporting every ~10 seconds during playback + on pause/stop
- [x]Library poster cards show progress bar for in-progress items
- [x]Library poster cards show watched indicator (checkmark) for completed items
- [x]"Continue Watching" horizontal row at top of library view for in-progress items
- [x]Continue Watching click navigates to detail page (where Play resumes)
- [x]Right-click context menu on poster cards: "Mark as Watched" / "Mark as Unwatched"
- [x]Manual mark watched/unwatched syncs to Plex via scrobble/unscrobble API
- [x]Plex `viewOffset`/`viewCount`/`lastViewedAt` parsed from metadata responses
- [x]On library sync, Plex watch state merges with local DB (most recent timestamp wins)
- [x]Graceful degradation when DB is unavailable (play without resume/tracking)

### Non-Functional

- [x]All watch state logic is pure Rust, no GTK dependencies (testable without display)
- [x]WatchStateService tested with 20+ unit tests for thresholds, debouncing, scrobble
- [x]WatchProgressRepo tested with in-memory SQLite
- [x]Plex timeline/scrobble endpoints tested with wiremock
- [x]FakePlexServer extended with timeline/scrobble route handlers for e2e tests
- [x]Zero clippy warnings, formatted with cargo fmt
- [x]Network failures during Plex reporting do not disrupt playback or crash

## Technical Approach

### Architecture

M4 adds a watch state layer that sits between the existing event pipeline and the persistence/API layers:

```
┌─────────────────────────────────────────────────────┐
│ UI Layer (Relm4 Components)                          │
│  ResumeOverlay │ MediaCard(progress) │ ContinueRow   │
└──────────────────────┬──────────────────────────────┘
                       │ Messages
┌──────────────────────┴──────────────────────────────┐
│ Watch State Service (pure Rust, no GTK)              │
│  WatchStateTracker │ debounce │ scrobble threshold   │
└──────────┬───────────────────────────┬──────────────┘
           │                           │
┌──────────┴──────────┐  ┌─────────────┴──────────────┐
│ Persistence Layer    │  │ Plex API Layer              │
│  WatchProgressRepo   │  │  report_timeline()          │
│  SQLite              │  │  scrobble() / unscrobble()  │
│                      │  │  on_deck()                  │
└─────────────────────┘  └────────────────────────────┘
```

### Data Model

New `watch_progress` table (schema version 2):

```sql
CREATE TABLE IF NOT EXISTS watch_progress (
    media_item_id TEXT PRIMARY KEY,
    position_seconds REAL NOT NULL DEFAULT 0.0,
    duration_seconds REAL NOT NULL DEFAULT 0.0,
    watched INTEGER NOT NULL DEFAULT 0,
    last_watched_at TEXT NOT NULL,
    FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE CASCADE
);

CREATE INDEX idx_watch_progress_last_watched
    ON watch_progress(last_watched_at DESC);
CREATE INDEX idx_watch_progress_in_progress
    ON watch_progress(watched, position_seconds)
    WHERE watched = 0 AND position_seconds > 0;
```

New model in `src/models/watch.rs`:

```rust
pub struct WatchProgress {
    pub media_item_id: String,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub watched: bool,
    pub last_watched_at: String, // ISO 8601
}

impl WatchProgress {
    pub fn progress_fraction(&self) -> f64 {
        if self.duration_seconds > 0.0 {
            (self.position_seconds / self.duration_seconds).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn should_show_resume(&self) -> bool {
        !self.watched
            && self.position_seconds > 30.0
            && self.progress_fraction() < 0.90
    }
}
```

### Media Identity in Playback

Currently `AppMsg::PlayMedia(String)` only carries a URL. Expand to carry the media item:

```rust
// In app.rs
pub enum AppMsg {
    PlayMedia { url: String, media_item: Option<MediaItem> },
    // ...
}
```

Add `now_playing: Option<MediaItem>` to `App` state. This enables:
- Looking up prior watch progress for the resume prompt
- Saving progress against the correct `media_item_id`
- Reporting Plex timeline with the correct `ratingKey`

For CLI/drag-and-drop files (no `MediaItem`), `now_playing` is `None` and progress is tracked by file path as a fallback key.

### WatchStateTracker (Pure State Machine)

Following the PlaybackTracker pattern -- a pure struct with no I/O that processes position updates and emits events:

```rust
// src/services/watch_state.rs

pub struct WatchStateTracker {
    media_id: Option<String>,
    duration: f64,
    last_persisted_position: f64,
    last_persist_time: std::time::Instant,
    last_timeline_time: std::time::Instant,
    scrobbled: bool,
}

pub enum WatchStateEvent {
    /// Save position to local DB (debounced every ~15s)
    PersistProgress { media_id: String, position: f64, duration: f64 },
    /// Report to Plex timeline API (every ~10s)
    ReportTimeline { rating_key: String, state: String, time_ms: i64, duration_ms: i64 },
    /// Mark as watched locally + Plex scrobble
    Scrobble { media_id: String },
    /// Send "stopped" timeline to Plex
    ReportStopped { rating_key: String, time_ms: i64, duration_ms: i64 },
}
```

The tracker is initialized when a file loads (`FileLoaded` event) and produces events on each `PositionChanged`. The `App::update()` handler dispatches these events to the DB repo and Plex client asynchronously.

### Plex API Extensions

Add to `PlexClient` (`src/services/plex/api.rs`):

```rust
pub async fn report_timeline(&self, rating_key: &str, state: &str, time_ms: i64, duration_ms: i64) -> Result<(), PlexError>
pub async fn scrobble(&self, rating_key: &str) -> Result<(), PlexError>
pub async fn unscrobble(&self, rating_key: &str) -> Result<(), PlexError>
pub async fn on_deck(&self) -> Result<Vec<PlexMetadata>, PlexError>
```

Plex API details:
- **Timeline**: `GET /:/timeline?ratingKey={key}&key=/library/metadata/{key}&state={playing|paused|stopped}&time={ms}&duration={ms}&identifier=com.plexapp.plugins.library`
- **Scrobble**: `GET /:/scrobble?key={ratingKey}&identifier=com.plexapp.plugins.library`
- **Unscrobble**: `GET /:/unscrobble?key={ratingKey}&identifier=com.plexapp.plugins.library`
- **On Deck**: `GET /library/onDeck`
- All times in **milliseconds**. The 90% scrobble threshold is server-side (automatic from timeline), but we also call scrobble explicitly for reliability.

Add to `PlexMetadata` serde model:

```rust
#[serde(rename = "viewOffset")]
pub view_offset: Option<i64>,   // ms, present if in-progress
#[serde(rename = "viewCount")]
pub view_count: Option<i32>,    // >= 1 means watched
#[serde(rename = "lastViewedAt")]
pub last_viewed_at: Option<i64>, // Unix timestamp
```

Extend `MediaSource` trait with default no-op implementations:

```rust
async fn report_progress(&self, _media_id: &str, _position_ms: i64, _duration_ms: i64, _state: &str) -> Result<(), SourceError> { Ok(()) }
async fn scrobble(&self, _media_id: &str) -> Result<(), SourceError> { Ok(()) }
async fn unscrobble(&self, _media_id: &str) -> Result<(), SourceError> { Ok(()) }
```

### Resume Overlay Component

New Relm4 component `src/components/player/resume_overlay.rs`:

- Overlaid on the video area (GTK Overlay)
- Shows: "Resume from {formatted_time}" with Resume / Start Over buttons
- Auto-dismisses after 5 seconds, defaulting to resume
- Countdown indicator (progress ring or text "5...4...3...")
- Video is paused while the overlay is visible
- On resume: seek to `max(0, saved_position - 10)` then unpause
- On start over: seek to 0 then unpause
- Escape key = start over

### Library UI Integration

**Poster cards** (`MediaCardData`):
- Add `watch_progress: Option<f64>` (0.0-1.0) and `watched: bool` fields
- In `bind()`: set `progress_bar` fraction and visibility when `watch_progress.is_some() && !watched`
- Add a small checkmark overlay in bottom-right for watched items

**Continue Watching row** (`src/components/library/continue_watching.rs`):
- Horizontal scrolling row of poster cards, placed above the main library grid
- Shows all in-progress items (watched=false, position > 0) sorted by `last_watched_at DESC`
- For TV: show the episode card with show title as subtitle
- Hidden when no in-progress items exist
- Capped at ~20 items

**Context menu**:
- Right-click/long-press gesture on poster cards
- Actions: "Mark as Watched", "Mark as Unwatched"
- Instant visual update + async Plex API call (fire-and-forget on failure)

### Watch State Sync on Library Load

During library sync (existing `sync_library()` flow):
1. Plex metadata already includes `viewOffset` and `viewCount` -- extract during conversion
2. Merge strategy: **most recent timestamp wins**
   - Compare Plex `lastViewedAt` vs local `last_watched_at`
   - Take the record with the later timestamp
3. Write merged state to local `watch_progress` table
4. Orphan cleanup: delete `watch_progress` rows where `media_item_id` no longer exists in `media_items`

### Event Flow: Position Update Pipeline

```
mpv poll (100ms) -> PlaybackTracker -> PositionChanged event
    -> App::update() -> WatchStateTracker.process()
        -> PersistProgress (every 15s) -> spawn_local: WatchProgressRepo.upsert()
        -> ReportTimeline (every 10s) -> spawn_local: PlexClient.report_timeline()
        -> Scrobble (at 90%, once) -> spawn_local: WatchProgressRepo.mark_watched() + PlexClient.scrobble()
```

### Event Flow: Playback End

```
EOF -> App::update()
    -> WatchStateTracker.stop()
        -> PersistProgress (final position) -> WatchProgressRepo.upsert()
        -> ReportStopped -> PlexClient.report_timeline(state="stopped")
        -> Scrobble (if >= 90% and not already scrobbled)
```

### Event Flow: App Close During Playback

```
close_request handler (app.rs:284)
    -> save window state (existing)
    -> save watch progress (new): WatchProgressRepo.upsert() with current position
    -> send stopped timeline to Plex (best-effort)
```

## System-Wide Impact

- **Interaction graph**: PositionChanged (currently no-op) -> WatchStateTracker -> DB write + Plex HTTP call. EndOfFile -> WatchStateTracker.stop() -> final persist + scrobble. LibraryView load -> query watch_progress -> populate card indicators.
- **Error propagation**: Plex API failures are fire-and-forget with `tracing::warn!()`. DB write failures log a warning but do not interrupt playback. The player is never blocked by watch state operations.
- **State lifecycle risks**: App crash between position updates could lose up to 15 seconds of progress -- acceptable. `ON DELETE CASCADE` on the foreign key prevents orphan watch_progress rows when media_items are deleted during sync.
- **API surface parity**: `MediaSource` trait gains three new methods with default no-op implementations, so existing `PlexSource` can add them incrementally without breaking the trait.
- **Integration test scenarios**: (1) Play a Plex movie, verify timeline calls hit the fake server at correct intervals. (2) Play past 90%, verify scrobble fires and library card updates. (3) Close app mid-playback, reopen, verify resume overlay appears with correct position. (4) Manual mark watched via context menu, verify Plex scrobble API called.

## Acceptance Criteria (Detailed)

- [x]`watch_progress` table created in schema v2 migration
- [x]`WatchProgress` model with `progress_fraction()` and `should_show_resume()` pure methods
- [x]`WatchProgressRepo` with `upsert()`, `find_by_media_id()`, `list_in_progress()`, `mark_watched()`, `mark_unwatched()`, `delete_by_media_id()`
- [x]`WatchStateTracker` pure state machine with debounced persist, interval timeline, one-shot scrobble
- [x]`PlexClient.report_timeline()` sends correct query params and headers
- [x]`PlexClient.scrobble()` / `unscrobble()` call correct Plex endpoints
- [x]`PlexClient.on_deck()` fetches in-progress items
- [x]`PlexMetadata` deserializes `viewOffset`, `viewCount`, `lastViewedAt`
- [x]`MediaSource` trait extended with `report_progress()`, `scrobble()`, `unscrobble()`
- [x]`AppMsg::PlayMedia` carries `Option<MediaItem>` for media identity
- [x]`App` has `now_playing: Option<MediaItem>` state
- [x]`PositionChanged` handler delegates to `WatchStateTracker`
- [x]`EndOfFile` handler triggers final persist + scrobble check
- [x]`close_request` handler saves watch progress before exit
- [x]Resume overlay component with auto-dismiss countdown (5s)
- [x]Resume backs up 10 seconds for context
- [x]Resume only shown when position > 30s and < 90%
- [x]`MediaCardData` gains `watch_progress` and `watched` fields
- [x]Poster progress bar visible for in-progress items
- [x]Watched checkmark overlay on completed items
- [x]Continue Watching row in library view, sorted by `last_watched_at DESC`
- [x]Right-click context menu with mark watched/unwatched
- [x]Watch state sync during Plex library sync (most recent timestamp wins)
- [x]Orphan watch_progress cleanup on sync

## Success Metrics

- Users can close and reopen the app, resuming exactly where they left off
- Plex dashboard reflects watch progress from Reel within ~10 seconds of playback
- Library browsing shows at-a-glance which items are watched, in-progress, or unwatched
- Continue Watching row makes it trivial to pick up where the user left off
- All watch state operations are resilient to network failures

## Dependencies & Risks

**Dependencies:**
- Schema migration system must support v1 -> v2 upgrade (current `init_db` creates tables idempotently; needs migration logic for adding new table alongside existing data)
- `progress_bar` widget already exists in `MediaCardWidgets` (hidden) -- needs wiring
- Plex timeline API requires `X-Plex-Client-Identifier` header (already set in PlexClient)

**Risks:**
- **Timeline reporting interval**: Too frequent = unnecessary HTTP traffic. Too infrequent = stale Plex dashboard. 10 seconds is the standard Plex client interval.
- **DB write contention**: Writing every 15 seconds from the UI thread could cause micro-stutters. Mitigate by using `spawn_local` with async channel.
- **Plex API rate limiting**: Plex servers don't typically rate-limit local clients, but timeline calls should be debounced regardless.
- **TV show "next episode" logic**: Determining the next unwatched episode requires querying all episodes of a show. For Continue Watching, keep it simple: show the specific episode that's in progress, not the show-level "next up".

**Out of scope for M4:**
- Auto-advance to next episode (future milestone)
- Trakt/Jellyfin/Emby scrobble (M7)
- Playback history log / viewing statistics
- Server-side watch state conflict resolution UI

## Sources & References

### Internal References
- `src/app.rs:429` -- PositionChanged no-op (primary integration point)
- `src/app.rs:441` -- EndOfFile handler (scrobble integration point)
- `src/app.rs:393` -- PlayMedia handler (needs media identity)
- `src/app.rs:284` -- close_request handler (needs final persist)
- `src/player/playback_tracker.rs` -- PlaybackTracker pattern to follow for WatchStateTracker
- `src/db/schema.rs` -- Current schema v1, needs v2 migration
- `src/db/media_repo.rs` -- Repository pattern to follow for WatchProgressRepo
- `src/services/plex/api.rs` -- PlexClient, needs timeline/scrobble methods
- `src/services/plex/models.rs:44-92` -- PlexMetadata, needs viewOffset/viewCount/lastViewedAt
- `src/services/media_source.rs` -- MediaSource trait, needs watch state methods
- `src/services/plex/fake_server.rs` -- FakePlexServer, needs timeline/scrobble routes
- `src/components/library/media_card.rs:53` -- progress_bar widget (hidden, ready for M4)

### Plex API References
- Timeline reporting: `GET /:/timeline` with ratingKey, state, time (ms), duration (ms)
- Scrobble: `GET /:/scrobble?key={ratingKey}&identifier=com.plexapp.plugins.library`
- Unscrobble: `GET /:/unscrobble?key={ratingKey}&identifier=com.plexapp.plugins.library`
- On Deck: `GET /library/onDeck`
- Progress minimum: 60001ms (server default `minimumProgressTime`)
- Watched threshold: 90% (server-side, hardcoded)
- All time values in milliseconds
