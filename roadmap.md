# Reel - Development Roadmap

## Milestone Overview

| Milestone | Name | Goal | Key Deliverable |
|-----------|------|------|-----------------|
| M0 | Skeleton | Walking skeleton - app window with mpv playback | Play a local file in a GTK4 window |
| M1 | Player | Full-featured media player | Polished playback with controls, tracks, subtitles |
| M2 | Plex Core | Plex library browsing | Browse Plex libraries with metadata and artwork |
| M3 | Library UX | Rich library experience | Search, filters, collections, detail pages |
| M4 | Watch State | Progress tracking and sync | Resume, watched status, Plex sync, continue watching |
| M5 | Polish | Release quality | Settings, MPRIS, error handling, packaging |
| M6 | Standalone | Direct source support | Local dirs, SMB/NFS, TMDb metadata |
| M7 | Extensions | Additional integrations | Jellyfin, Emby, Trakt, OpenSubtitles download |

---

## M0: Walking Skeleton

**Goal:** Prove the stack works end-to-end. A Relm4 + libadwaita window that plays a video file using mpv.

### Tasks

- [ ] Initialize Rust project with Cargo.toml and all core dependencies
- [ ] Set up Nix flake for development:
  - `flake.nix` with `devShells.default` providing: `rustc`, `cargo`, `clippy`, `rustfmt`, `pkg-config`
  - Native build inputs: `gtk4`, `libadwaita`, `mpv`, `glib`, `pango`, `gdk-pixbuf`, `graphene`, `wrapGAppsHook4`
  - `LD_LIBRARY_PATH` / `PKG_CONFIG_PATH` set via `shellHook` or `nativeBuildInputs`
  - Nix package definition for building Reel (`packages.default`)
  - `nix develop` must provide a working environment where `cargo build` succeeds
  - Pin nixpkgs to a specific rev for reproducibility
- [ ] Create `main.rs`: create `adw::Application`, load CSS
- [ ] Create `app.rs`: root `App` component with `AdwApplicationWindow`
- [ ] Define `VideoBackend` trait in `player/backend.rs` with core playback methods
- [ ] Implement `MpvBackend` wrapping `libmpv2`:
  - Initialize mpv with `vo=libmpv`, `hwdec=auto`
  - Create render context on GLArea `realize` signal
  - Platform detection (Wayland/X11) for display handle + `get_proc_address`
  - Set update callback → `glib::idle_add` → `gl_area.queue_render()`
  - Render into GLArea FBO via `mpv_render_context_render()`
- [ ] Create `VideoArea` widget: `GtkGraphicsOffload` wrapping `GtkGLArea`
- [ ] Wire mpv wakeup callback → event processing on main thread
- [ ] Basic play/pause with spacebar
- [ ] Open file via command-line argument or file chooser dialog
- [ ] Verify hardware acceleration works (check `hwdec-current` property)

### Success Criteria
- `nix develop` drops into a shell where `cargo build` succeeds
- `nix build` produces a working binary
- Run `reel /path/to/video.mkv` and see it play in a libadwaita window
- Video renders via mpv OpenGL render API into GtkGLArea
- Hardware-accelerated decoding active on supported hardware (VA-API/NVDEC)
- Works on both Wayland and X11
- No crashes on common formats (MKV, MP4, AVI)

---

## M1: Full-Featured Player

**Goal:** A polished video player that rivals Celluloid in playback capabilities, with a clean overlay UI. All playback interaction goes through the `VideoBackend` trait.

### Tasks

- [ ] Player controls overlay component (`PlayerControls`)
  - Play/pause button
  - Progress bar with seek (click and drag via `VideoBackend::seek_absolute`)
  - Position / duration labels (from `BackendEvent::PositionChanged`)
  - Volume slider with mute toggle (via `VideoBackend::set_volume/set_mute`)
  - Fullscreen toggle
- [ ] Auto-hide controls (show on mouse move via `EventControllerMotion`, hide after 3s timeout)
- [ ] Keyboard shortcuts (space=pause, left/right=seek, up/down=volume, F11=fullscreen, Esc=exit fullscreen)
- [ ] Audio track selector popover (parse `track-list` via `VideoBackend::tracks()`, switch via `set_audio_track`)
- [ ] Subtitle track selector popover (embedded + external subs via `VideoBackend::tracks()`)
- [ ] External subtitle file loading (via `VideoBackend::add_subtitle_file`, auto-detect matching filenames)
- [ ] Subtitle rendering customization (via `VideoBackend::set_subtitle_style`) stored in settings
- [ ] Playback speed control (0.25x-4x via `VideoBackend::set_speed`)
- [ ] Skip forward/back buttons (configurable interval via `VideoBackend::seek_relative`)
- [ ] Chapter navigation (when chapters present)
- [ ] Screensaver inhibition during playback (D-Bus `org.freedesktop.ScreenSaver.Inhibit`)
- [ ] Drag-and-drop file onto window to play
- [ ] Remember window size/position across sessions
- [ ] Error handling: show toast on playback errors with codec/format info

### Success Criteria
- Can play any format mpv/FFmpeg supports with full control
- Controls auto-hide during playback, appear on mouse movement
- Audio/subtitle tracks switchable during playback
- All keyboard shortcuts functional

---

## M2: Plex Core

**Goal:** Connect to a Plex server and browse its libraries with metadata and artwork.

### Tasks

- [ ] Define `MediaSource` trait in `services/media_source.rs`
- [ ] Implement `PlexClient` HTTP API client
  - Authentication (server URL + token, or Plex account OAuth)
  - List libraries
  - Fetch library items (movies, shows) with pagination
  - Fetch metadata details (cast, crew, synopsis)
  - Construct direct play URLs
  - Image URL construction (poster, backdrop, via Plex image transcoder)
- [ ] Implement `PlexSource: MediaSource`
- [ ] Plex server connection UI (onboarding/settings)
  - Server URL + token input
  - Connection test with feedback
  - mDNS discovery of local Plex servers (nice-to-have, can defer)
- [ ] SQLite database setup
  - Schema creation with migrations
  - `MediaRepo` for caching Plex items locally
- [ ] Sidebar navigation component
  - Movies / TV Shows sections
  - Source indicator
- [ ] Library grid view (`TypedGridView<MediaCard>`)
  - Poster card factory component with title, year
  - Async artwork loading with placeholder
  - Artwork disk cache (`$XDG_CACHE_HOME/reel/artwork/`)
  - Click to navigate to detail page
- [ ] Basic movie detail page
  - Backdrop header image
  - Title, year, rating, runtime, genres
  - Synopsis
  - Play button → launches player with Plex direct play URL
- [ ] Basic TV show detail page
  - Show info + season list
  - Episode list per season
  - Play episode → launches player

### Success Criteria
- Connect to a Plex server, see movie library as a poster grid
- Click a movie → see detail page with metadata → click Play → video plays
- Browse TV show → season → episode → play
- Artwork loads and caches properly
- Navigation between library, detail, and player views works smoothly

---

## M3: Library UX

**Goal:** Rich library browsing experience with search, filtering, sorting, and collections.

### Tasks

- [ ] Search component
  - Instant search as-you-type across titles
  - Results displayed in dropdown or dedicated view
  - Keyboard shortcut (Ctrl+F or /) to activate
- [ ] Filter bar component
  - Genre filter (multi-select dropdown)
  - Year/decade filter
  - Unwatched only toggle
- [ ] Sort options (title, year, date added, rating)
- [ ] Collections view
  - Fetch Plex collections
  - Collection detail page (poster grid of collection items, ordered)
  - Collection cards in library browse
- [ ] Enhanced movie detail page
  - Cast list with photos (horizontal scrollable `FactoryVecDeque`)
  - Director / writer credits
  - Technical info (resolution, codec, audio channels, file size)
  - Collection membership link
- [ ] Enhanced TV show detail page
  - Episode thumbnails
  - Episode descriptions and air dates
  - Season artwork
- [ ] List view alternative to grid (toggle in toolbar)
- [ ] Grid density control (small/medium/large posters)
- [ ] Adaptive layout with `AdwBreakpoint`
  - Sidebar collapses on narrow windows
  - Grid columns adjust to window width
- [ ] Empty states (`AdwStatusPage`) for no results, no connection

### Success Criteria
- Search finds movies/shows instantly
- Filters narrow down library by genre, year, watched state
- Collections browsable as grouped sets
- Detail pages show cast, crew, and technical info
- Layout adapts gracefully from wide to narrow windows

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
- [ ] Nix packaging (flake.nix)
- [ ] First-run experience
  - Welcome page prompting to add a Plex server
  - Connection wizard with test

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
M0 ──► M1 ──► M2 ──► M3 ──► M4 ──► M5 (release)
                                      │
                                      ├──► M6 (standalone)
                                      └──► M7 (extensions)
```

M0-M5 are sequential - each builds on the previous. M6 and M7 can be worked on in parallel after M5.
