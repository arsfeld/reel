# Reel - Development Roadmap

## Milestone Overview

| Milestone | Name | Status | Goal | Key Deliverable |
|-----------|------|--------|------|-----------------|
| M0 | Skeleton | **Done** | Walking skeleton - app window with mpv playback | Play a local file in a GTK4 window |
| M1 | Player | **Done** | Full-featured media player | Polished playback with controls, tracks, subtitles |
| M2 | Plex Core | **Done** | Plex library browsing | Browse Plex libraries with metadata and artwork |
| M3 | Library UX | **Done** | Rich library experience | Search, filters, collections, detail pages |
| M4 | Watch State | Planned | Progress tracking and sync | Resume, watched status, Plex sync, continue watching |
| M5 | Polish | Planned | Release quality | Settings, MPRIS, error handling, packaging |
| M6 | Standalone | Planned | Direct source support | Local dirs, SMB/NFS, TMDb metadata |
| M7 | Extensions | Planned | Additional integrations | Jellyfin, Emby, Trakt, OpenSubtitles download |

---

## M0: Walking Skeleton ✓

**Goal:** Prove the stack works end-to-end. A Relm4 + libadwaita window that plays a video file using mpv.

**Status:** Complete

### Tasks

- [x] Initialize Rust project with Cargo.toml and all core dependencies
- [x] Set up Nix flake for development:
  - `flake.nix` with `devShells.default` providing: `rustc`, `cargo`, `clippy`, `rustfmt`, `pkg-config`
  - Native build inputs: `gtk4`, `libadwaita`, `mpv`, `glib`, `pango`, `gdk-pixbuf`, `graphene`, `wrapGAppsHook4`
  - `LD_LIBRARY_PATH` / `PKG_CONFIG_PATH` set via `shellHook` or `nativeBuildInputs`
  - Nix package definition for building Reel (`packages.default`)
  - `nix develop` must provide a working environment where `cargo build` succeeds
  - Pin nixpkgs to a specific rev for reproducibility
- [x] Create `main.rs`: create `adw::Application`, load CSS
- [x] Create `app.rs`: root `App` component with `AdwApplicationWindow`
- [x] Define `PlayState`, `EndReason`, and utility functions in `player/backend.rs`
- [x] Implement `MpvBackend` wrapping `libmpv2`:
  - Initialize mpv with `vo=libmpv`, `hwdec=auto`
  - Create render context on GLArea `realize` signal
  - Platform detection (Wayland/X11) for display handle + `get_proc_address`
  - Set update callback → `glib::idle_add` → `gl_area.queue_render()`
  - Render into GLArea FBO via `mpv_render_context_render()`
- [x] Create `VideoArea` component with GLArea + mpv render context
- [x] Wire mpv wakeup callback → event processing on main thread
- [x] Basic play/pause with spacebar
- [x] Open file via command-line argument or file chooser dialog
- [x] Hardware acceleration works (VA-API/NVDEC detected via `hwdec-current`)
- [x] `PlaybackTracker` pure state machine with 57+ tests

---

## M1: Full-Featured Player ✓

**Goal:** A polished video player that rivals Celluloid in playback capabilities, with a clean overlay UI.

**Status:** Complete

### Tasks

- [x] Player controls overlay component (`PlayerControls`)
  - Play/pause button
  - Progress bar with seek (click and drag)
  - Position / duration labels
  - Volume slider with mute toggle
  - Fullscreen toggle
- [x] Auto-hide controls (show on mouse move, hide after 3s timeout via `OverlayController`)
- [x] Keyboard shortcuts (space=pause, left/right=seek, up/down=volume, F11=fullscreen, Esc=exit fullscreen)
- [x] Audio track selector popover (parse `track-list`, switch tracks)
- [x] Subtitle track selector popover (embedded + external subs)
- [x] External subtitle file loading (auto-detect matching filenames + file chooser)
- [x] Playback speed control (0.25x–4x with presets)
- [x] Chapter navigation (when chapters present)
- [x] Screensaver inhibition during playback (D-Bus `org.freedesktop.ScreenSaver.Inhibit`)
- [x] Drag-and-drop file onto window to play
- [x] Remember window size/position across sessions
- [x] Error handling: show toast on playback errors

### Deferred from M1

- Subtitle rendering customization (font, size, color) → M5 settings
- Skip forward/back buttons in controls bar → M3 or M5

---

## M2: Plex Core ✓

**Goal:** Connect to a Plex server and browse its libraries with metadata and artwork.

**Status:** Complete (needs manual testing against real Plex server)

### Tasks

- [x] Define `MediaSource` trait in `services/media_source.rs`
- [x] Implement `PlexClient` HTTP API client
  - List libraries, fetch library items, fetch metadata
  - Construct direct play URLs and image transcoder URLs
  - Proper Plex headers (`X-Plex-Token`, `X-Plex-Product`, `Accept: application/json`)
  - Error handling (401 → Unauthorized, 404 → NotFound, 5xx → Server)
- [x] Implement `PlexSource: MediaSource`
- [x] Plex OAuth browser sign-in flow
  - PIN-based auth via `plex.tv/api/v2/pins` with `strong=true`
  - Opens browser with `xdg-open` for user authentication
  - Polls for token (1s interval, 5min timeout)
  - Auto-discovers servers via `plex.tv/api/v2/resources`
  - Auto-connects if single server, shows picker if multiple
  - Persistent `X-Plex-Client-Identifier` (UUID stored in `$XDG_DATA_HOME/reel/client_id`)
- [x] SQLite database setup
  - Schema with `media_items`, `sources`, `schema_version` tables
  - `MediaRepo` for caching Plex items locally (upsert, find, list, delete)
  - `SourceRepo` for persisting server connections
  - In-memory SQLite tests
- [x] Data models: `MediaItem`, `MediaType`, `SourceType`, `Source`, `SourceConfig`, `LibrarySection`
- [x] Plex serde models with separate container types for Directory vs Metadata responses
- [x] Conversion layer: `PlexMetadata` → `MediaItem` (duration ms→min, genres, file paths, parent IDs)
- [x] Config module: XDG path helpers (`data_dir`, `cache_dir`, `config_dir`, `db_path`, `artwork_dir`)
- [x] Sidebar navigation component (Movies / TV Shows)
- [x] Library grid view (`TypedGridView<MediaCardData>`)
  - Poster card with title, year
  - Async artwork loading via `ArtworkCache`
  - Loading / empty / error states with `AdwStatusPage`
  - Click to navigate to detail page
- [x] Artwork disk cache (`$XDG_CACHE_HOME/reel/artwork/`, SHA256 cache keys)
- [x] Movie detail page (backdrop, title, year, rating, runtime, genres, synopsis, Play button)
- [x] TV show detail page (season dropdown, episode list with `AdwActionRow`, per-episode play)
- [x] Navigation shell: `gtk::Stack` (shell/player) + `AdwNavigationSplitView` + `AdwNavigationView`
- [x] CLI `reel file.mkv` still works (bypasses library, straight to player)
- [x] Drag-and-drop still works in library views (switches to player)
- [x] Keyboard shortcuts context-aware (player shortcuts only in player view)

### Test Coverage

302 tests total (181 new in M2):
- Config: 6 tests
- Models (media, source, library): 16 tests
- Database (schema, media_repo, source_repo): 26 tests
- Plex serde models: 10 tests
- Plex conversion: 15 tests
- Plex HTTP client (wiremock): 12 tests
- PlexSource (wiremock): 7 tests
- Artwork cache (wiremock + tempfile): 8 tests
- Plex auth: 8 tests
- Connection dialog (URL normalization): 4 tests

### Known Issues / Not Yet Verified

- End-to-end flow not yet tested against a real Plex server
- Artwork reload after async download may not visually refresh grid items
- Pushing same component widget onto NavigationView repeatedly may have lifecycle issues
- Token could appear in debug-level reqwest logs

---

## M3: Library UX

**Goal:** Rich library browsing experience with search, filtering, sorting, and collections.

**Status:** Complete

### Tasks

- [x] Search component (instant as-you-type, Ctrl+F / /, SearchBar + SearchEntry)
- [x] Filter bar (genre multi-select chips, decade DropDown, clear filters button)
- [x] Sort options (7 options: title, year, date added, rating, runtime)
- [x] Pure filter/sort/search logic in `services/library_filter.rs` (52 unit tests)
- [x] Client-side filtering on in-memory `Vec<MediaItem>` with texture cache
- [x] Fix `is_text_input_focused` keyboard shortcut conflict
- [x] State preserved on navigate-back, reset on library type switch
- [x] No-results empty state (`AdwStatusPage`) with "Clear Filters" action
- [x] Collections view (sidebar entry, fetch from Plex, poster grid)
- [x] Enhanced movie detail (cast with photos, director/writer credits, technical info, collection links)
- [x] Enhanced TV show detail (episode thumbnails, descriptions, season artwork, show backdrop)
- [x] Grid density control (Small 120x180, Medium 180x270, Large 240x360)
- [x] View preference persistence (view_mode + grid_density in WindowState TOML)
- [x] Adaptive sidebar (AdwNavigationSplitView collapses on narrow)
- [x] Adaptive grid (min_columns/max_columns auto-adjust)
- [x] `MediaDetail` model (CastMember, TechnicalInfo, CollectionRef) with display helpers
- [x] Plex serde: Role, Director, Writer, Collection arrays + PlexMedia technical fields
- [x] PlexClient: collections() + collection_items() endpoints
- [x] MediaSource trait: metadata() → MediaDetail, collections(), collection_items()
- [x] MediaType::Collection variant
- [ ] List view alternative to grid → deferred to M5 (grid density covers the UX need)

### Test Coverage

390 tests total (25 new in M3b):
- Plex serde: 4 tests (roles, directors, writers, collections, technical fields, parentThumb, defaults)
- Plex wiremock: 3 tests (collections, collection_items, empty collections)
- Plex conversion: 7 tests (cast, credits, collections, technical info, empty fields, base item)
- Detail model: 11 tests (display_resolution, display_audio_channels, display_file_size, from_item)

### Success Criteria
- ~~Search finds movies/shows instantly~~ ✓
- ~~Filters narrow down library by genre, year~~ ✓
- ~~Collections browsable as grouped sets~~ ✓
- ~~Detail pages show cast, crew, and technical info~~ ✓
- Layout adapts gracefully from wide to narrow windows → deferred

---

## M4: Watch State

**Goal:** Track what you've watched, resume where you left off, sync with Plex.

### Tasks

- [ ] Watch state database table and repository
- [ ] Save playback position on pause/stop/exit (debounced, every 10s during playback)
- [ ] Resume playback from saved position (prompt: "Resume from X:XX?" or auto-resume)
- [ ] Watched/unwatched indicators on poster cards
  - Unwatched: no indicator
  - Partially watched: orange progress bar at bottom of poster
  - Fully watched: checkmark or dimmed poster
- [ ] Mark as watched/unwatched from detail page and context menu (right-click on poster)
- [ ] "Continue Watching" section on home/library view
  - Shows in-progress items sorted by last watched
- [ ] "Recently Added" section (from Plex)
- [ ] Plex watch state sync
  - Report progress to Plex during playback (timeline API, every 10s)
  - Scrobble (mark watched) when >90% played
  - Pull watched state from Plex on sync
- [ ] Auto-play next episode
  - Detect near end of episode (< 60s remaining or credits)
  - Show "Next Episode" overlay with countdown
  - Auto-advance or click to skip
- [ ] "Up Next" for TV shows (next unwatched episode per show)

### Success Criteria
- Close app mid-movie, reopen → resume from exact position
- Poster grid shows watch progress visually
- Plex server reflects watch state from Reel and vice versa
- Auto-play next episode works for TV shows
- Continue Watching shows in-progress items

---

## M5: Polish & Release

**Goal:** Release-quality application. Settings UI, desktop integration, error handling, packaging.

### Tasks

- [ ] Settings window (`AdwPreferencesWindow`)
  - Playback settings (skip interval, default speed, HW accel toggle)
  - Subtitle settings (default language, font customization, auto-download toggle)
  - Library settings (grid density, sort default, watched indicators)
  - Connection management (add/edit/remove Plex servers)
  - Cache management (clear artwork cache, show cache size)
  - About page (version, credits, license)
- [ ] MPRIS2 implementation
  - Playback control (play/pause/stop/next/prev/seek)
  - Metadata broadcasting (title, show name, artwork)
  - Position reporting
  - Media key integration verified on GNOME, KDE, Sway
- [ ] Global error handling audit
  - Network failures → toast with retry
  - Playback failures → descriptive error dialog
  - Database failures → graceful degradation
  - No silent failures, no panics in production
- [ ] Logging with `tracing`
  - Structured logging to stderr and optional file
  - Log levels configurable via env var (REEL_LOG=debug)
  - mpv log message integration (`mpv_request_log_messages`)
- [ ] Application desktop entry and icons
  - SVG app icon (scalable)
  - `.desktop` file with proper categories and MIME types
  - AppStream metainfo XML
- [ ] Flatpak packaging
  - Flatpak manifest
  - Build and test in Flatpak sandbox
  - Verify HW acceleration works in sandbox
  - Verify network access works
  - Verify screensaver inhibition works
- [ ] Nix packaging (flake.nix — already partially done)
- [ ] First-run experience
  - Welcome page prompting to sign in to Plex
  - Connection wizard with browser OAuth flow

### Success Criteria
- Install from Flatpak, connect Plex server, browse and play - all works
- Media keys control playback (GNOME, KDE)
- Settings are intuitive and persistent
- No crashes or unhandled errors in normal usage
- App feels polished and responsive

---

## M6: Standalone Sources (Post-Release)

**Goal:** Browse and play media from local directories and network shares without a media server.

### Tasks

- [ ] `LocalSource: MediaSource` implementation
  - Scan local directories recursively for media files
  - Filename parsing (title, year, season, episode extraction)
  - TMDb metadata lookup and caching
  - Artwork downloading
- [ ] TMDb API client (`services/tmdb/`)
  - Search movies by title + year
  - Search TV shows by title
  - Get details (cast, crew, synopsis, artwork URLs)
  - Get collection info
  - Rate limiting (50 req/s, but be conservative)
- [ ] Filesystem watcher (notify crate)
  - Watch configured directories for changes
  - Auto-add new files, remove deleted ones
  - Trigger metadata lookup for new items
- [ ] Add source UI: "Add Local Folder" option in settings
- [ ] SMB/CIFS share browsing (pavao crate)
  - Add SMB source with server/share/credentials
  - Browse and stream from SMB shares
- [ ] NFS support (via GIO/GVfs or direct mount)
- [ ] NFO file support (read Kodi-style sidecar metadata)
- [ ] Network service discovery (mdns-sd)
  - Discover SMB shares on local network
  - Present in source connection UI

### Success Criteria
- Point Reel at `/mnt/nas/Movies/` → movies appear with TMDb metadata
- New files added to directory appear automatically
- SMB share browsable and playable
- Metadata matches correctly for standard naming conventions

---

## M7: Extensions (Post-Release)

**Goal:** Additional integrations that expand Reel's ecosystem.

### Tasks

- [ ] Jellyfin integration (`JellyfinSource: MediaSource`)
  - Jellyfin API client
  - Library browsing, direct play, watch sync
- [ ] Emby integration (`EmbySource: MediaSource`)
- [ ] Trakt.tv integration
  - OAuth authentication flow
  - Scrobbling (auto-log watches)
  - 2-way watch history sync
  - Rating sync
- [ ] OpenSubtitles download integration
  - Search by file hash (compute hash from video file)
  - Search by title + season + episode
  - Language preference
  - One-click download and apply
  - Respect rate limits (40 req/10s) and download limits
- [ ] Fanart.tv integration (clearlogos, additional backdrops)
- [ ] Person detail page (photo, bio, filmography within library)
- [ ] Plex server discovery via mDNS (GDNSd)
- [ ] Multiple Plex server support
- [ ] Playback queue / playlist support
- [ ] Volume boost beyond 100%
- [ ] Audio-video sync offset adjustment UI
- [ ] Deinterlacing toggle

---

## Development Principles

### Iteration Strategy

Each milestone follows the same cycle:

1. **Scaffold** - Create component files, define types and message enums
2. **Wire** - Connect components with message passing, verify navigation
3. **Implement** - Build features with tests, one at a time
4. **Test** - Manual testing + automated tests for services
5. **Refine** - UI polish, error handling, edge cases

### Quality Gates Per Milestone

Before moving to the next milestone:
- [ ] All features in milestone are functional
- [ ] No panics or crashes in normal usage
- [ ] Automated tests pass for service layer
- [ ] Code compiles with zero warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt`)

### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| mpv + GTK4 GL integration complexity | M0 proves the stack works (GtkGLArea + mpv render API), following Celluloid's proven pattern |
| libmpv2 crate stability | Active crate (Dec 2025), can fall back to raw libmpv2-sys FFI if needed |
| Plex API undocumented/changes | Use established community-documented endpoints; abstract behind MediaSource trait |
| Performance with large libraries (10K+ items) | TypedGridView virtual scrolling from M2; database indexes; paginated loading |
| Flatpak sandbox restrictions | Test in sandbox from M5, but verify HW accel earlier in M1 |
| libadwaita API instability | Pin to specific GNOME SDK version (46); use only stable widgets (1.4+) |

### Dependency Order

```
M0 ✓ ──► M1 ✓ ──► M2 ✓ ──► M3 ──► M4 ──► M5 (release)
                                              │
                                              ├──► M6 (standalone)
                                              └──► M7 (extensions)
```

M0-M5 are sequential - each builds on the previous. M6 and M7 can be worked on in parallel after M5.
