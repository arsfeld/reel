---
title: "Reel: Native Media Center in Zig"
type: feat
status: active
date: 2026-03-14
origin: docs/brainstorms/2026-03-14-reel-media-center-brainstorm.md
---

# Reel: Native Media Center in Zig

## Overview

Reel is a full-featured media center — an Infuse clone — written in Zig with native platform frontends. It embeds libmpv for universal playback, serves as a full Plex client, manages local media libraries with TMDB metadata, and supports offline sync. The architecture follows Ghostty's proven pattern: a Zig core library (`libreel`) exposing a C ABI, consumed by GTK4 on Linux and Swift/AppKit on macOS.

(See brainstorm: `docs/brainstorms/2026-03-14-reel-media-center-brainstorm.md`)

## Problem Statement / Motivation

There is no native, high-performance media center on Linux that matches Infuse's quality. Existing options are either Electron-based (Plex web), toolkit-agnostic with non-native feel (VLC, Celluloid), or abandoned. macOS has IINA for local playback but no full Plex client with a native UI. Reel fills this gap with a Zig core for performance and native frontends for platform-correct UX.

## Proposed Solution

### Architecture

```
┌─────────────────────────────────────────────────────┐
│                  Platform Frontends                  │
│  ┌─────────────────────┐  ┌───────────────────────┐ │
│  │    GTK4 + libadwaita │  │  Swift / AppKit       │ │
│  │    (Linux)           │  │  (macOS)              │ │
│  │  ┌─────────────────┐ │  │  ┌─────────────────┐  │ │
│  │  │  GtkGLArea      │ │  │  │  MTKView /      │  │ │
│  │  │  (render surface)│ │  │  │  NSOpenGLView   │  │ │
│  │  └─────────────────┘ │  │  └─────────────────┘  │ │
│  │  - MPRIS D-Bus       │  │  - Menu bar / Dock    │ │
│  │  - Media keys        │  │  - Media keys         │ │
│  └──────────┬──────────┘  └──────────┬────────────┘ │
│             │          C ABI         │              │
│  ┌──────────┴────────────────────────┴────────────┐ │
│  │              libreel (Zig Core)                 │ │
│  │                                                 │ │
│  │  ┌──────────┐ ┌──────────────┐ ┌─────────────┐ │ │
│  │  │ Player   │ │ MediaServer  │ │ Metadata    │ │ │
│  │  │ (mpv)    │ │ (trait)      │ │ (TMDB)      │ │ │
│  │  └──────────┘ └──────────────┘ └─────────────┘ │ │
│  │  ┌──────────┐ ┌──────────────┐ ┌─────────────┐ │ │
│  │  │ Library  │ │ Scanner      │ │ Downloader  │ │ │
│  │  │ (SQLite) │ │              │ │ (sync)      │ │ │
│  │  └──────────┘ └──────────────┘ └─────────────┘ │ │
│  │  ┌──────────┐ ┌──────────────┐                 │ │
│  │  │ HTTP     │ │ Settings     │                 │ │
│  │  │ Client   │ │              │                 │ │
│  │  └──────────┘ └──────────────┘                 │ │
│  └─────────────────────────────────────────────────┘ │
│                         │                            │
│  ┌─────────────────────────────────────────────────┐ │
│  │              System Libraries                    │ │
│  │  libmpv · FFmpeg · SQLite · OpenSSL/TLS          │ │
│  └─────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────┘
```

### Key Technical Decisions

All decisions carried forward from brainstorm (see origin document):

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Language | Zig | Performance, C interop, memory control, Ghostty precedent |
| Architecture | Core lib + native frontends | Ghostty pattern, maximum native feel |
| Media backend | libmpv (embedded) | Universal codec support, hardware accel, proven |
| Linux frontend | GTK4 + libadwaita | Modern, C API, Wayland support, GNOME integration |
| macOS frontend | Swift/AppKit via C ABI | Native look & feel, platform conventions |
| Network source | Abstract MediaServer trait | Plex first, Jellyfin/Emby later |
| Metadata | TMDB API | Industry standard for movies/TV |
| Storage | SQLite (WAL mode) | Embedded, zero config, concurrent reads |
| Build system | Nix + build.zig | Reproducible deps, idiomatic Zig |
| Plex auth | Browser redirect + PIN polling | Simple, no webview dependency |
| Video render | Frontend-owned surface | GtkGLArea / NSOpenGLView → libmpv |
| HTTP client | `std.http.Client` | Zig stdlib, sufficient for REST APIs; mpv handles media streaming |
| Token storage | SQLite (plaintext initially) | Simple; platform keyring as future enhancement |
| GTK styling | libadwaita | Modern adaptive widgets, dark mode, GNOME-native |

### Threading Model

```
┌─────────────────┐     ┌──────────────────┐     ┌────────────────┐
│   Main Thread    │     │   Network Pool   │     │  Scanner Thread│
│   (GTK/AppKit)   │     │   (Zig threads)  │     │  (background)  │
│                  │     │                  │     │                │
│ - UI rendering   │     │ - Plex API calls │     │ - File enumera-│
│ - mpv render     │     │ - TMDB fetches   │     │   tion         │
│ - User input     │     │ - Image downloads│     │ - Metadata     │
│ - SQLite reads   │     │ - Timeline report│     │   matching     │
│                  │     │                  │     │ - SQLite writes│
└────────┬─────────┘     └────────┬─────────┘     └───────┬────────┘
         │                        │                        │
         └────────────────────────┼────────────────────────┘
                                  │
                    Message passing via thread-safe
                    queues (Zig std.Thread.ResetEvent
                    + atomic ring buffers)
```

- **Main thread**: UI event loop (GTK main loop / AppKit run loop), mpv OpenGL rendering, SQLite reads
- **Network thread pool**: All HTTP requests (Plex API, TMDB API, image downloads), timeline reporting
- **Scanner thread**: File system enumeration, filename parsing, TMDB matching, SQLite writes
- **mpv internal threads**: Managed by libmpv (demuxing, decoding, audio output)
- **Synchronization**: Thread-safe message queues. Network/scanner results posted to main thread via `g_idle_add()` (GTK) or `DispatchQueue.main.async` (AppKit)
- **SQLite**: WAL mode, single connection with mutex. Main thread reads, scanner/network threads write through the mutex

### Directory Structure

Adapted from Ghostty's proven layout:

```
reel-zig/
├── build.zig                    # Zig build system
├── build.zig.zon                # Zig package dependencies
├── flake.nix                    # Nix flake for reproducible dev environment
├── flake.lock
├── include/
│   └── reel.h                   # Manually maintained C header for libreel
├── src/
│   ├── main.zig                 # Linux GTK entry point
│   ├── lib.zig                  # libreel public API (C ABI exports)
│   ├── config.zig               # Build-time configuration options
│   ├── core/
│   │   ├── player.zig           # mpv wrapper (create, command, properties, events)
│   │   ├── player_render.zig    # mpv render context management
│   │   ├── library.zig          # Media library management (CRUD, queries)
│   │   ├── database.zig         # SQLite connection, migrations, thread safety
│   │   ├── scanner.zig          # File system scanning, filename parsing
│   │   ├── downloader.zig       # Offline sync / download manager
│   │   ├── settings.zig         # App settings (read/write from SQLite)
│   │   └── types.zig            # Shared types (MediaItem, Episode, Movie, etc.)
│   ├── net/
│   │   ├── http.zig             # HTTP client wrapper (std.http.Client)
│   │   ├── media_server.zig     # MediaServer interface (trait)
│   │   ├── plex/
│   │   │   ├── client.zig       # Plex API client
│   │   │   ├── auth.zig         # Plex PIN-based authentication
│   │   │   ├── types.zig        # Plex API response types
│   │   │   └── xml.zig          # Plex XML response parser
│   │   └── tmdb/
│   │       ├── client.zig       # TMDB API client
│   │       └── types.zig        # TMDB response types
│   └── apprt/
│       ├── gtk/
│       │   ├── app.zig          # GtkApplication lifecycle
│       │   ├── window.zig       # Main window (GtkApplicationWindow)
│       │   ├── video_area.zig   # GtkGLArea + mpv render integration
│       │   ├── library_view.zig # Library browsing grid
│       │   ├── detail_view.zig  # Media item detail page
│       │   ├── player_controls.zig  # Playback OSD controls
│       │   ├── auth_view.zig    # Plex authentication UI
│       │   ├── settings_view.zig    # Settings panel
│       │   └── mpris.zig        # MPRIS D-Bus integration
│       └── appkit/              # (Phase 5 — macOS frontend)
│           └── ...
├── macos/                       # (Phase 5 — Xcode project, Swift frontend)
│   ├── Reel.xcodeproj/
│   └── Reel/
│       ├── AppDelegate.swift
│       ├── MainWindow.swift
│       ├── VideoView.swift       # NSOpenGLView + mpv
│       └── ...
├── test/
│   ├── core/
│   │   ├── player_test.zig
│   │   ├── library_test.zig
│   │   ├── database_test.zig
│   │   └── scanner_test.zig
│   └── net/
│       ├── plex_client_test.zig
│       └── tmdb_client_test.zig
└── docs/
    ├── brainstorms/
    ├── plans/
    └── TECHNOLOGY_REFERENCE.md
```

## Technical Considerations

### libmpv Integration

- Use `@cImport` on `mpv/client.h`, `mpv/render.h`, `mpv/render_gl.h` for full API access
- The existing `zmpv` library covers client API but NOT render API — write render bindings directly
- Render flow: frontend creates GL context → passes FBO ID to mpv via `mpv_render_context_render()` → mpv renders into it
- **Critical threading rule**: mpv's update callback must NOT call any mpv API — only signal the UI thread via `g_idle_add()`
- GTK4's GtkGLArea renders into its own FBO (not framebuffer 0) — must call `glGetIntegerv(GL_FRAMEBUFFER_BINDING, ...)` to get the correct FBO
- Set `hwdec=auto-safe` for hardware acceleration with software fallback

### Plex API Integration

- PIN auth flow: `POST /api/v2/pins` → open browser to `plex.tv/link` → poll `GET /api/v2/pins/<id>` every 2s (30min timeout)
- All requests require `X-Plex-Client-Identifier` (UUID, generated once and persisted in SQLite) and `X-Plex-Product: Reel`
- Server discovery: `GET /api/v2/resources?includeHttps=1` → returns servers with connection URIs
- Connection priority: try local LAN URIs first, then relay, configurable in settings
- Library browsing: `/library/sections`, `/library/sections/{id}/all`, `/library/onDeck`, `/library/recentlyAdded`
- Direct play: `http://<ip>:32400{part_key}?X-Plex-Token=<token>` — mpv handles the HTTP streaming
- Timeline/scrobbling: `GET /:/timeline?ratingKey=X&state=playing&time=Y&duration=Z` every 10s during playback
- Token expiration: on 401 response, show "Session expired" dialog with re-authenticate button

### MediaServer Interface

```zig
// src/net/media_server.zig — comptime interface pattern (Ghostty-style)
pub fn MediaServer(comptime Self: type) type {
    return struct {
        pub fn authenticate(self: *Self) !AuthResult { return self.authenticateImpl(); }
        pub fn getLibraries(self: *Self) ![]Library { return self.getLibrariesImpl(); }
        pub fn getItems(self: *Self, library_id: []const u8, opts: BrowseOptions) ![]MediaItem { ... }
        pub fn getOnDeck(self: *Self) ![]MediaItem { ... }
        pub fn getRecentlyAdded(self: *Self) ![]MediaItem { ... }
        pub fn getMetadata(self: *Self, item_id: []const u8) !MediaDetail { ... }
        pub fn reportTimeline(self: *Self, item_id: []const u8, state: PlayState, time_ms: u64) !void { ... }
        pub fn getStreamUrl(self: *Self, item: MediaItem) ![]const u8 { ... }
    };
}
```

### SQLite Schema (Draft)

```sql
-- Schema version tracking
CREATE TABLE schema_version (version INTEGER PRIMARY KEY);

-- Plex server connections
CREATE TABLE servers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    client_identifier TEXT NOT NULL,  -- X-Plex-Client-Identifier UUID
    auth_token TEXT,
    connection_uri TEXT,
    last_connected_at INTEGER
);

-- Media items (both Plex and local)
CREATE TABLE media_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,              -- 'plex' or 'local'
    source_id TEXT,                    -- Plex ratingKey or local file hash
    server_id TEXT REFERENCES servers(id),
    media_type TEXT NOT NULL,          -- 'movie', 'episode', 'show', 'season'
    title TEXT NOT NULL,
    sort_title TEXT,
    year INTEGER,
    summary TEXT,
    rating REAL,
    duration_ms INTEGER,
    poster_path TEXT,                  -- local cached path
    backdrop_path TEXT,
    tmdb_id INTEGER,
    parent_id INTEGER REFERENCES media_items(id),  -- show→season→episode
    season_number INTEGER,
    episode_number INTEGER,
    file_path TEXT,                    -- local file or Plex part key
    added_at INTEGER,
    updated_at INTEGER
);

-- Watch history and progress
CREATE TABLE watch_progress (
    media_item_id INTEGER PRIMARY KEY REFERENCES media_items(id),
    position_ms INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER,
    watched BOOLEAN NOT NULL DEFAULT 0,
    last_watched_at INTEGER
);

-- Offline downloads
CREATE TABLE downloads (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    media_item_id INTEGER REFERENCES media_items(id),
    server_id TEXT REFERENCES servers(id),
    source_url TEXT NOT NULL,
    local_path TEXT,
    total_bytes INTEGER,
    downloaded_bytes INTEGER DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'queued',  -- queued, downloading, paused, complete, failed
    created_at INTEGER,
    completed_at INTEGER
);

-- Local library scan paths
CREATE TABLE scan_paths (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    last_scanned_at INTEGER
);

-- Image cache metadata
CREATE TABLE image_cache (
    url TEXT PRIMARY KEY,
    local_path TEXT NOT NULL,
    size_bytes INTEGER,
    cached_at INTEGER
);

-- App settings (key-value)
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT
);
```

### Error Handling Strategy

Four error categories with corresponding UI patterns:

| Category | Examples | UI Pattern |
|----------|----------|------------|
| **Auth** | Token expired, PIN timeout, server rejected | Dialog with "Re-authenticate" / "Try again" |
| **Network** | Server unreachable, TMDB timeout, DNS failure | Toast banner "Server unreachable — showing cached data", background retry |
| **Playback** | Unsupported codec, corrupt file, stream timeout | Error overlay on player with "Try again" / "Back to library" |
| **Storage** | Disk full, permission denied, DB corruption | Modal dialog with specific error and suggested action |

### Playback UX Decisions

- **Resume**: Auto-resume from last position with a 5s toast "Resuming from X:XX" and a "Start over" button
- **Next episode**: 10-second countdown to next episode after credits, with cancel button
- **Fullscreen**: Double-click or F11 toggles. Mouse movement reveals OSD for 3 seconds, then auto-hides
- **Volume**: Up/Down arrows adjust mpv volume (app-level, not system). Displayed in OSD
- **Keyboard shortcuts**: Space (play/pause), Left/Right (seek ±10s), Up/Down (volume ±5%), F (fullscreen), M (mute), S (cycle subtitles), A (cycle audio tracks), Escape (exit fullscreen)
- **Buffering**: Spinner overlay with "Buffering..." text when mpv reports `paused-for-cache=yes`

### TMDB Integration

- User provides their own TMDB API key in settings (with link to registration page)
- Bearer token auth: `Authorization: Bearer <key>`
- Use `append_to_response=videos,credits,images` to minimize API calls
- Cache metadata in SQLite, images on disk (w342 for grid thumbnails, w780 for detail view)
- Image cache: LRU eviction at 500 MB cap
- For Plex items: use Plex-provided metadata directly, no TMDB lookup (Plex already has rich metadata)
- TMDB only used for local-only media

### Filename Parsing (Local Library)

Follow Plex naming conventions:
- Movies: `Movie Name (2020)/Movie Name (2020).mkv`
- TV: `Show Name/Season 01/Show Name - S01E01 - Episode Title.mkv`
- Extract title, year, season/episode numbers via regex
- Unmatched files: show in "Unmatched" section for manual TMDB search

## System-Wide Impact

### Interaction Graph

This is a greenfield project — no existing system impact. Internal interaction chain:

- User selects media → Frontend calls `reel_play(item_id)` via C ABI → Core resolves stream URL (Plex API or local path) → Core sends `loadfile` command to mpv → mpv fires `FILE_LOADED` event → Core notifies frontend → Frontend begins render loop → Core starts timeline reporting thread (Plex items)

### Error Propagation

- mpv errors surface via `mpv_wait_event()` → Core translates to `ReelError` enum → Frontend displays error overlay
- HTTP errors (Plex/TMDB) → Core returns error union → Frontend shows toast or dialog based on error category
- SQLite errors → Core logs and returns error → Frontend shows storage error dialog
- All errors include context (what was attempted, what failed, suggested action)

### State Lifecycle Risks

- **Partial download**: If download interrupted, `downloads.status` remains `downloading` with `downloaded_bytes < total_bytes`. On restart, resume from `downloaded_bytes` using HTTP Range header
- **Token revocation during playback**: mpv continues playing current stream (already buffered), but next API call fails → show re-auth dialog after current playback
- **Scanner crash**: `scan_paths.last_scanned_at` not updated → next app launch rescans automatically
- **SQLite WAL checkpoint**: Run `PRAGMA wal_checkpoint(PASSIVE)` on clean app shutdown

### API Surface Parity

The C ABI (`include/reel.h`) is the single API surface. Both GTK and AppKit frontends consume identical functions. Parity enforced by the shared header.

### Integration Test Scenarios

1. **Plex auth → browse → play → scrobble**: Full flow from PIN auth to watching a movie, verifying timeline reports reach the server
2. **Local scan → TMDB match → playback**: Add a movie file, verify TMDB metadata appears, play the file
3. **Offline download → disconnect → play → reconnect → sync**: Download a Plex item, disconnect network, play offline, reconnect, verify watch status syncs
4. **Token expiration mid-browse**: Simulate 401 during library browsing, verify re-auth flow triggers
5. **Concurrent scan + playback**: Start a library scan while playing media, verify no UI stutter or SQLite contention

## Acceptance Criteria

### Functional Requirements

- [ ] **Playback**: Can play any video format supported by mpv (MKV, MP4, AVI, etc.) with hardware acceleration
- [ ] **Plex auth**: Complete PIN-based browser redirect authentication flow
- [ ] **Plex browsing**: Browse libraries, On Deck, Recently Added, search, TV show drill-down (Show → Season → Episode)
- [ ] **Plex playback**: Direct play from Plex server with timeline/scrobble reporting every 10 seconds
- [ ] **Resume playback**: Resume from last position for both Plex and local media
- [ ] **Local library**: Scan local directories, parse filenames, organize into Movies/TV Shows
- [ ] **TMDB metadata**: Fetch and display posters, descriptions, ratings, cast for local media
- [ ] **Subtitles**: Select embedded or external subtitle tracks during playback
- [ ] **Audio tracks**: Select audio tracks during playback
- [ ] **Offline sync**: Download Plex media for offline viewing, resume interrupted downloads
- [ ] **Keyboard controls**: Play/pause, seek, volume, fullscreen, subtitle/audio cycling
- [ ] **Native UI**: GTK4 + libadwaita on Linux with proper theming and dark mode

### Non-Functional Requirements

- [ ] **Performance**: Playback starts within 2 seconds of selection; UI stays responsive during background operations
- [ ] **Memory**: Idle memory under 100 MB; during 4K playback under 500 MB (excluding mpv's internal buffers)
- [ ] **Startup**: App window visible within 1 second on warm start
- [ ] **Build**: `nix develop -c zig build` produces working binary on NixOS

### Quality Gates

- [ ] All Zig tests pass (`zig build test`)
- [ ] No Zig compiler warnings
- [ ] Core library has tests for player control, database operations, Plex API parsing, filename parsing
- [ ] Manual testing covers all 5 integration scenarios listed above

## Implementation Phases

### Phase 1: Foundation — Project Scaffold + Basic Playback

**Goal**: A GTK4 window that plays a local video file via libmpv.

**Tasks**:
1. Initialize git repository
2. Create `flake.nix` with Zig, libmpv, GTK4, libadwaita, SQLite, pkg-config
3. Create `build.zig` and `build.zig.zon` — static library target for `libreel` + executable target for GTK app
4. Implement `src/core/player.zig` — mpv create, initialize, loadfile, basic property observation (time-pos, duration, pause, eof-reached)
5. Implement `src/core/player_render.zig` — mpv render context creation, OpenGL FBO rendering
6. Implement `src/apprt/gtk/app.zig` — GtkApplication lifecycle, window creation
7. Implement `src/apprt/gtk/video_area.zig` — GtkGLArea setup, GL context, mpv render integration
8. Implement `src/apprt/gtk/player_controls.zig` — Basic OSD: play/pause button, seek bar, time display
9. Implement keyboard shortcuts (space, arrows, F, M, Escape)
10. Write `include/reel.h` — initial C header with player functions
11. Write tests for `player.zig` (command sending, property observation)

**Success Criteria**:
- `nix develop -c zig build` compiles without errors
- Running the binary opens a GTK4 window
- Passing a file path as argument plays the video with working controls
- Seek, pause, fullscreen, volume all work via keyboard

**Files**: `flake.nix`, `build.zig`, `build.zig.zon`, `include/reel.h`, `src/main.zig`, `src/lib.zig`, `src/core/player.zig`, `src/core/player_render.zig`, `src/apprt/gtk/app.zig`, `src/apprt/gtk/window.zig`, `src/apprt/gtk/video_area.zig`, `src/apprt/gtk/player_controls.zig`, `test/core/player_test.zig`

---

### Phase 2: Data Layer — SQLite + Settings + HTTP Client

**Goal**: Persistent storage, settings management, and a working HTTP client for API calls.

**Tasks**:
1. Implement `src/core/database.zig` — SQLite connection via `@cImport("sqlite3.h")`, WAL mode, mutex wrapper, schema migrations
2. Create initial schema (all tables from Technical Considerations)
3. Implement `src/core/settings.zig` — read/write key-value settings from SQLite
4. Implement `src/net/http.zig` — async HTTP wrapper around `std.http.Client` with connection pooling
5. Implement `src/core/types.zig` — shared data types (MediaItem, Movie, Episode, Show, Season, etc.)
6. Implement `src/core/library.zig` — CRUD operations on media_items, watch_progress
7. Write tests for database (migrations, CRUD), settings, HTTP client

**Success Criteria**:
- Database creates and migrates schema on first run
- Settings persist across app restarts
- HTTP client can make GET/POST requests with headers
- Library CRUD operations work with proper SQLite concurrency

**Files**: `src/core/database.zig`, `src/core/settings.zig`, `src/core/types.zig`, `src/core/library.zig`, `src/net/http.zig`, `test/core/database_test.zig`, `test/core/library_test.zig`

---

### Phase 3: Plex Integration — Auth, Browse, Play

**Goal**: Full Plex client — authenticate, browse libraries, play media with scrobbling.

**Tasks**:
1. Implement `src/net/media_server.zig` — comptime MediaServer interface
2. Implement `src/net/plex/auth.zig` — PIN-based auth flow (generate PIN, open browser, poll for token)
3. Implement `src/net/plex/xml.zig` — Plex XML response parser (Plex API returns XML by default)
4. Implement `src/net/plex/types.zig` — Plex-specific types (PlexServer, PlexLibrary, PlexMediaItem, etc.)
5. Implement `src/net/plex/client.zig` — Full Plex API client implementing MediaServer interface:
   - Server discovery (`GET /api/v2/resources`)
   - Library listing (`GET /library/sections`)
   - Browse items (`GET /library/sections/{id}/all`)
   - On Deck / Recently Added
   - Item metadata (`GET /library/metadata/{ratingKey}`)
   - Stream URL construction
   - Timeline reporting (`GET /:/timeline`)
6. Generate and persist `X-Plex-Client-Identifier` UUID on first run
7. Implement `src/apprt/gtk/auth_view.zig` — PIN display, "Open browser" button, polling status
8. Implement `src/apprt/gtk/library_view.zig` — Poster grid, library section tabs, On Deck row, Recently Added row, search
9. Implement `src/apprt/gtk/detail_view.zig` — Movie/show detail page with metadata, play button, episode list for TV
10. Wire playback: select item → get stream URL → loadfile in mpv → start timeline reporting
11. Implement resume playback (read `viewOffset` from Plex metadata, seek on load)
12. Implement next-episode auto-play with countdown
13. Handle 401 → re-authentication flow
14. Write tests for Plex API parsing, auth flow state machine

**Success Criteria**:
- Can authenticate with Plex via browser PIN flow
- Can browse all Plex library sections, On Deck, Recently Added
- Can play any Plex media item via direct play
- Watch progress syncs to Plex (visible in Plex web)
- Resume playback works for partially watched items
- TV show drill-down (Show → Season → Episode) works
- Token expiration triggers re-auth gracefully

**Files**: `src/net/media_server.zig`, `src/net/plex/auth.zig`, `src/net/plex/client.zig`, `src/net/plex/types.zig`, `src/net/plex/xml.zig`, `src/apprt/gtk/auth_view.zig`, `src/apprt/gtk/library_view.zig`, `src/apprt/gtk/detail_view.zig`, `test/net/plex_client_test.zig`

---

### Phase 4: Local Library + TMDB Metadata

**Goal**: Scan local media files, match to TMDB, display in organized library alongside Plex content.

**Tasks**:
1. Implement `src/core/scanner.zig` — recursive directory enumeration, video file detection (by extension), filename parser (regex for title/year/season/episode following Plex naming conventions)
2. Implement `src/net/tmdb/types.zig` — TMDB response types (SearchResult, MovieDetail, TVDetail, etc.)
3. Implement `src/net/tmdb/client.zig` — TMDB API client:
   - Movie search (`GET /3/search/movie`)
   - TV search (`GET /3/search/tv`)
   - Movie details with credits+images (`GET /3/movie/{id}?append_to_response=credits,images`)
   - TV details (`GET /3/tv/{id}`)
   - Image URL construction (`https://image.tmdb.org/t/p/{size}{path}`)
4. Implement image caching — download poster/backdrop to disk, track in `image_cache` table, LRU eviction at 500MB
5. Integrate scanner with library: scan → parse filename → search TMDB → store metadata + images → update UI
6. Add "Add library folder" to settings view (file chooser dialog)
7. Update library_view to show both Plex and local items with source indicator
8. Handle unmatched files — show in "Unmatched" section with manual TMDB search
9. Implement periodic rescan (configurable interval, default 1 hour)
10. Implement TMDB API key entry in settings
11. Write tests for filename parser, TMDB response parsing, scanner

**Success Criteria**:
- Adding a folder triggers scan and TMDB metadata lookup
- Movies and TV shows appear in library with posters, descriptions, ratings
- Local media plays correctly from library view
- Unmatched files are surfaced for manual matching
- Image cache stays under 500 MB with LRU eviction
- Rescans detect new/removed files

**Files**: `src/core/scanner.zig`, `src/net/tmdb/client.zig`, `src/net/tmdb/types.zig`, `src/apprt/gtk/settings_view.zig`, `test/core/scanner_test.zig`, `test/net/tmdb_client_test.zig`

---

### Phase 5: Offline Sync + Polish

**Goal**: Download Plex media for offline viewing. Polish UX across all features.

**Tasks**:
1. Implement `src/core/downloader.zig` — download queue manager:
   - Queue management (add, remove, pause, resume, prioritize)
   - HTTP download with progress tracking
   - Resume interrupted downloads via HTTP Range header
   - Max 2 concurrent downloads
   - Disk space check before starting
2. Add downloads UI — download button on Plex items, downloads section showing progress/status
3. Implement offline playback — detect offline state, play from local download path
4. Implement watch status sync — when back online, report offline watch progress to Plex
5. Implement MPRIS D-Bus integration (`src/apprt/gtk/mpris.zig`) — now-playing metadata, transport controls
6. Polish loading states — skeleton placeholders while images/metadata load
7. Polish empty states — guided setup when no server configured, empty library messages
8. Polish error states — toast notifications, retry buttons, error dialogs per category
9. Implement subtitle selection UI in player controls
10. Implement audio track selection UI in player controls
11. Buffering indicator (spinner overlay when `paused-for-cache=yes`)
12. Write integration tests for download flow

**Success Criteria**:
- Can download Plex media items with progress indication
- Downloads resume after interruption
- Offline playback works when server is unreachable
- Watch status syncs back to Plex on reconnection
- MPRIS integration works (system media controls, now-playing in DE)
- All empty/loading/error states handled gracefully
- Subtitle and audio track selection works during playback

**Files**: `src/core/downloader.zig`, `src/apprt/gtk/mpris.zig`, `test/core/downloader_test.zig`

---

### Phase 6: macOS Frontend

**Goal**: Swift/AppKit frontend consuming the same libreel C ABI.

**Tasks**:
1. Set up Xcode project in `macos/` directory
2. Build libreel as static library, package as XCFramework with `module.modulemap`
3. Create `module.modulemap` pointing to `include/reel.h`
4. Implement `AppDelegate.swift` — app lifecycle, menu bar
5. Implement `MainWindow.swift` — main window with sidebar navigation
6. Implement `VideoView.swift` — NSOpenGLView (or MTKView for Metal) + mpv render integration
7. Implement library browsing views (NSCollectionView for poster grid)
8. Implement detail view (movie/show detail)
9. Implement player controls (NSView overlay with transport controls)
10. Implement Plex auth view (PIN display, open browser)
11. Implement settings (NSPreferencesWindow)
12. Media key integration (MPRemoteCommandCenter)
13. Evaluate OpenGL vs Metal — if Metal is feasible with mpv, prefer it (OpenGL deprecated on macOS)

**Success Criteria**:
- Same libreel binary works on macOS via XCFramework
- All features from Phases 1-5 work on macOS
- Native macOS look and feel (menu bar, keyboard shortcuts, system preferences)
- Media keys work

**Files**: `macos/Reel.xcodeproj/`, `macos/Reel/AppDelegate.swift`, `macos/Reel/MainWindow.swift`, `macos/Reel/VideoView.swift`, etc.

## Alternative Approaches Considered

(See brainstorm for full analysis)

1. **GTK4 on both platforms** — Rejected: non-native macOS feel defeats the "ala Ghostty" goal
2. **Protocol-based IPC** (separate process for core) — Rejected: IPC overhead for video frames is impractical, over-engineered for two platforms
3. **Pure Zig decoders** — Rejected: enormous effort, limited format support; libmpv gives us everything for free
4. **FFmpeg direct** (without mpv) — Rejected: mpv adds playback controls, subtitle rendering, hardware acceleration management that FFmpeg alone doesn't provide
5. **Electron/web UI** — Not considered: contradicts the entire premise of native performance and platform feel

## Dependencies & Prerequisites

### Build Dependencies (provided by Nix)

- Zig (latest stable, currently 0.14.x)
- libmpv (mpv-unwrapped)
- GTK4 + libadwaita
- SQLite3
- libepoxy (OpenGL function loading)
- pkg-config
- (macOS only) Xcode + Swift compiler

### Runtime Dependencies

- Plex Media Server (user's own server or Plex Pass for remote access)
- TMDB API key (free, user-provided)
- OpenGL 3.3+ capable GPU
- Wayland or X11 display server (Linux)

### External API Dependencies

- `plex.tv` — authentication, server discovery
- Plex Media Server — library browsing, media streaming, timeline reporting
- `api.themoviedb.org` — metadata search, details, images
- `image.tmdb.org` — poster and backdrop image CDN

## Risk Analysis & Mitigation

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| Zig language breaking changes | High | Medium | Pin Zig version in flake.nix, follow Zig release notes |
| libmpv render API changes | High | Low | mpv render API is stable; pin mpv version |
| Plex API changes/deprecation | High | Low | Plex API has been stable for years; abstract via MediaServer trait |
| GTK4 → GTK5 migration | Medium | Low | Not expected for years; libadwaita provides stability layer |
| macOS OpenGL removal | High | Medium | Plan Metal migration in Phase 6; mpv supports `--gpu-api=vulkan` via MoltenVK |
| TMDB API key abuse | Low | Low | Users provide own key; no bundled key to abuse |
| SQLite corruption | Medium | Low | WAL mode + clean shutdown checkpoint; document backup strategy |
| Large library performance | Medium | Medium | Pagination, lazy loading, indexed SQLite queries |

## Success Metrics

- **Phase 1 complete**: Can play any local video file with controls in a native GTK4 window
- **Phase 3 complete**: Fully functional Plex client with browsing, playback, and scrobbling
- **Phase 5 complete**: Feature-complete Linux media center with offline sync
- **Phase 6 complete**: Cross-platform (Linux + macOS) with native UI on both

## Sources & References

### Origin

- **Brainstorm document:** [docs/brainstorms/2026-03-14-reel-media-center-brainstorm.md](docs/brainstorms/2026-03-14-reel-media-center-brainstorm.md) — Key decisions carried forward: Ghostty architecture pattern, libmpv backend, abstract MediaServer interface, frontend-owned render surfaces

### Internal References

- Technology reference: `docs/TECHNOLOGY_REFERENCE.md` — API signatures, code examples for all integrations
- Best practices research: `docs/research/best-practices-research.md` — Ghostty patterns, zig-gobject, Plex API details

### External References

- [Ghostty source](https://github.com/ghostty-org/ghostty) — architecture reference
- [zig-gobject](https://github.com/ianprime0509/zig-gobject) — typed GTK4 bindings for Zig
- [mpv render API](https://github.com/mpv-player/mpv/blob/master/libmpv/render.h) — OpenGL rendering integration
- [mpv-examples GTK PR #44](https://github.com/mpv-player/mpv-examples/pull/44) — GTK + mpv render example
- [Plex API wiki](https://github.com/Arcanemagus/plex-api/wiki) — comprehensive endpoint documentation
- [TMDB API docs](https://developer.themoviedb.org/docs) — metadata API reference
- [Mitchell Hashimoto: Zig and Useful Patterns](https://mitchellh.com/writing/ghostty-and-useful-zig-patterns) — comptime interfaces, C ABI patterns
- [Mitchell Hashimoto: Zig and SwiftUI](https://mitchellh.com/writing/zig-and-swiftui) — Zig → Swift/XCFramework pipeline
- [Ian Johnson: Zero-cost Zig Bindings](https://ianjohnson.dev/posts/zero-cost-bindings-with-zig/) — GTK wrapper patterns
- [Celluloid source](https://github.com/celluloid-player/celluloid) — GTK + mpv reference implementation
