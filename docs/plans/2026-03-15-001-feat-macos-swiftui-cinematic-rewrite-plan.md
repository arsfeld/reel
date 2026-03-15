---
title: "feat: Full SwiftUI Rewrite — Cinematic macOS Media Center"
type: feat
status: completed
date: 2026-03-15
origin: docs/brainstorms/2026-03-15-macos-swiftui-rewrite-brainstorm.md
---

# Full SwiftUI Rewrite — Cinematic macOS Media Center

## Overview

Complete rewrite of Reel's macOS frontend from AppKit to SwiftUI, targeting macOS 26 Tahoe with Liquid Glass. Replace all existing AppKit view controllers with SwiftUI views. Build every sidebar view with real data from the Zig C ABI. Bridge only the mpv video player via `NSViewRepresentable`. Deliver an Infuse-quality cinematic media center experience.

The Zig backend and C API are unchanged — this is a frontend rewrite with targeted C ABI gap-fills.

## Problem Statement

The macOS frontend is far behind the GTK4 frontend. Of 8 sidebar views, only Settings (Plex auth) has real functionality. All others are `PlaceholderViewController` showing an icon and text. The video player's OpenGL rendering is stubbed out (TODO comment, clears to black, never renders frames). The entire UI is AppKit, which Apple is deprecating in favor of SwiftUI. Building new features on AppKit accumulates tech debt in a new app.

## Proposed Solution

Full SwiftUI rewrite targeting macOS 26 Tahoe:

- **NavigationSplitView** for sidebar + content split
- **NavigationStack** inside content for push/pop (grid → detail → player)
- **@Observable** view models wrapping C ABI calls with async/await
- **NSViewRepresentable** bridge for mpv video player only
- **Liquid Glass** via `.glassEffect()` for player controls, toolbar, and detail view overlays
- **Infuse-style cinematic design**: dark, large artwork, blurred backdrops, poster-forward

## Technical Approach

### Architecture

```
ReelApp (@main, SwiftUI App)
├── NavigationSplitView
│   ├── SidebarView
│   │   ├── Library: Home, Movies, TV Shows, Other
│   │   ├── Sources: Favorites, Files
│   │   └── Management: Downloads, Settings
│   └── NavigationStack (content)
│       ├── HomeView (hero banner + carousels)
│       ├── MediaGridView (lazy poster grid, reused)
│       ├── DetailView (cinematic metadata + actions)
│       ├── PlayerView (full window takeover)
│       ├── FilesView (source browser)
│       ├── DownloadsView (queue list)
│       └── SettingsView (preferences + auth)
│
├── @Observable Models (C ABI wrappers)
│   ├── LibraryModel (items, search, genres, favorites)
│   ├── PlayerModel (playback state, position, volume)
│   ├── PlexAuthModel (PIN flow, server discovery)
│   ├── DownloadModel (queue, progress polling)
│   └── SettingsModel (key-value settings)
│
└── C ABI Bridge
    ├── ReelCore module (module.modulemap → reel.h)
    ├── Swift MediaItem struct (converts from ReelMediaItem)
    └── reel_free() for string ownership
```

### C ABI Bridge Layer

Create a `Bridge/` directory with Swift types that wrap C structs:

```swift
// Bridge/MediaItem.swift
struct MediaItem: Identifiable, Hashable {
    let id: Int64
    let mediaType: MediaType
    let source: MediaSource
    let title: String
    let sortTitle: String?
    let year: Int32
    let summary: String?
    let rating: Double
    let durationMs: Int64
    let posterPath: String?
    let backdropPath: String?
    let parentId: Int64
    let seasonNumber: Int32
    let episodeNumber: Int32
    let filePath: String?

    init(from c: ReelMediaItem) {
        self.id = c.id
        self.title = String(cString: c.title)
        self.posterPath = c.poster_path.map { String(cString: $0) }
        // ... etc, nil-safe conversions
    }
}
```

### State Management

- `@Observable` classes for all view models
- C ABI calls dispatched to background threads via `Task { }` + `nonisolated` methods
- Results published on `@MainActor`
- Image loading: custom `CachedImageView` that calls C image cache on background thread, shows placeholder during load

### Liquid Glass Integration

- `NavigationSplitView` sidebar gets Liquid Glass automatically on macOS 26
- Player controls overlay: `.glassEffect(.regular, in: .rect(cornerRadius: 16))`
- Detail view metadata card: `.glassEffect(.regular, in: .rect(cornerRadius: 12))` over backdrop
- Toolbar uses standard system glass behavior

### Video Player Bridge

The mpv player requires `NSViewRepresentable` because it renders via OpenGL/Metal into an `NSView`. The existing `VideoView` (NSOpenGLView) will be adapted:

- Wrap in `NSViewRepresentable` with `makeNSView`/`updateNSView`
- Use `Coordinator` for mpv update callbacks → SwiftUI state
- Marshal mpv thread callbacks to main actor via `DispatchQueue.main.async`
- **Metal migration**: Replace `NSOpenGLView` with `MTKView` + mpv's `--gpu-api=vulkan` via MoltenVK, or use mpv's `--gpu-context=moltenvk` directly. OpenGL may not be available on macOS 26.

### Implementation Phases

#### Phase 1: C ABI Gap-Fill

**Goal**: Export all missing Zig library functions needed by the SwiftUI views. Fix critical bugs.

**Tasks:**

1. **Export missing library query functions** in `src/lib.zig`:
   - `reel_library_get_items` — query by type with sort/pagination (declared in header, not exported)
   - `reel_library_get_recently_added` — convenience query (declared, not exported)
   - `reel_library_get_continue_watching` — items with watch progress (declared, not exported)
   - `reel_library_search` — full-text search (declared, not exported)
   - `reel_library_get_item_by_id` — single item fetch for detail view (new)
   - `reel_library_get_items_by_parent` — seasons of show, episodes of season (new)
   - `reel_library_get_genres` — distinct genre list for home screen rows (new)
   - `reel_library_get_items_by_genre` — items matching a genre (new)
   - `reel_library_list_favorites` — all favorited items (new)
   - `reel_library_list_scan_paths` — configured scan directories (new)

2. **Export missing player functions**:
   - `reel_player_get_volume` — current volume level (new, fix `adjustVolume` bug in VideoView.swift:145)
   - `reel_player_stop` — stop playback, return to idle state (new)
   - Fix `reel_player_get_position` / `reel_player_get_duration` to actually query mpv properties instead of returning cached zeros

3. **Export missing download functions**:
   - `reel_download_list` — all downloads with status/progress (new)
   - `reel_download_get_progress` — bytes downloaded/total for a specific item (new)

4. **Export watch progress functions**:
   - `reel_library_update_watch_progress` — save position on pause/stop (new)
   - `reel_library_get_watch_progress` — resume position for an item (new)

5. **Add memory management**:
   - `reel_free(void* ptr)` — free Zig-allocated strings/buffers from Swift
   - Document string ownership convention: all returned `const char*` must be freed via `reel_free()`

6. **Update `include/reel.h`** with all new function declarations and any new struct fields.

**Files:**
- `src/lib.zig` — add ~20 new `export fn` declarations
- `include/reel.h` — add corresponding C declarations
- `src/core/library.zig` — may need minor additions for new query functions

**Success criteria:**
- [ ] All declared functions in `reel.h` have matching `export fn` in `lib.zig`
- [ ] `reel_player_get_position` returns actual mpv position, not 0
- [ ] `reel_free` exists and frees Zig-allocated memory
- [ ] macOS `swift build` links without undefined symbol errors

**Estimated effort:** Medium

---

#### Phase 2: SwiftUI App Shell + Sidebar

**Goal**: Replace the entire AppKit app with a SwiftUI app. Working sidebar with NavigationSplitView, placeholder content views.

**Tasks:**

1. **Create SwiftUI App entry point** — `ReelApp.swift`:
   - `@main struct ReelApp: App`
   - `@State` for database and library handles (init in `.onAppear` or `init()`)
   - Window group with `NavigationSplitView`
   - Handle CLI file argument: if `CommandLine.arguments.count > 1`, go straight to player

2. **Create `SidebarView.swift`**:
   - `List` with `Section` groups: Library, Sources, Management
   - SF Symbols icons matching current sidebar (house, film, tv, tray.full, star.fill, externaldrive, arrow.down.circle, gearshape)
   - `@Binding` selection state for sidebar item
   - Liquid Glass comes free from NavigationSplitView

3. **Create `ContentRouter.swift`**:
   - Switch on sidebar selection → show appropriate view in NavigationStack
   - Placeholder views for all 8 items initially

4. **Create Swift bridge types** — `Bridge/` directory:
   - `Bridge/MediaItem.swift` — Swift struct from `ReelMediaItem`
   - `Bridge/Collection.swift` — Swift struct from `ReelCollectionC`
   - `Bridge/Server.swift` — Swift struct from `ReelServerC`
   - `Bridge/ReelBridge.swift` — static functions wrapping C ABI calls with Swift types
   - `Bridge/Enums.swift` — Swift enums mirroring C enums (`MediaType`, `SortField`, etc.)

5. **Update `Package.swift`**:
   - Change platform to `.macOS(.v26)` (or equivalent for Tahoe)
   - Keep `ReelCore` system library target unchanged
   - Update Swift tools version if needed

6. **Delete AppKit files** that are fully replaced:
   - `main.swift` (replaced by `@main ReelApp`)
   - `AppDelegate.swift` (lifecycle moves to SwiftUI App)
   - `MainWindow.swift` (replaced by NavigationSplitView)
   - `SidebarViewController.swift` (replaced by SidebarView)
   - `PlaceholderViewController.swift` (no longer needed)
   - Keep `VideoView.swift` and `PlayerControlsView.swift` for Phase 5 bridging

**Files:**
- New: `ReelApp.swift`, `SidebarView.swift`, `ContentRouter.swift`, `Bridge/*.swift`
- Modified: `Package.swift`
- Deleted: `main.swift`, `AppDelegate.swift`, `MainWindow.swift`, `SidebarViewController.swift`, `PlaceholderViewController.swift`

**Success criteria:**
- [ ] App launches with SwiftUI NavigationSplitView sidebar
- [ ] Sidebar shows all 8 items with correct icons and sections
- [ ] Clicking sidebar items switches content area
- [ ] Liquid Glass sidebar appearance on macOS 26
- [ ] CLI file argument still detected (player integration in Phase 5)

**Estimated effort:** Medium

---

#### Phase 3: Home Screen + Poster Grid

**Goal**: Cinematic home screen with hero banner and carousels. Reusable poster grid for Movies/TV Shows/Other.

**Tasks:**

1. **Create `LibraryModel.swift`** — `@Observable` class:
   - `func getItems(type:sortBy:sortOrder:limit:offset:) async -> [MediaItem]`
   - `func getRecentlyAdded(limit:) async -> [MediaItem]`
   - `func getContinueWatching(limit:) async -> [MediaItem]`
   - `func getGenres() async -> [String]`
   - `func getItemsByGenre(_ genre: String, limit:) async -> [MediaItem]`
   - `func getFavorites() async -> [MediaItem]`
   - `func search(query:) async -> [MediaItem]`
   - All methods dispatch to background thread, call C ABI, convert to Swift types

2. **Create `CachedImageView.swift`** — async image loader:
   - Takes a poster/backdrop path (local file path from image cache)
   - Loads image on background thread
   - Shows gradient placeholder during load
   - Fade-in animation on load complete

3. **Create `HomeView.swift`**:
   - **Hero banner**: Large backdrop image from most recent/featured item, title overlay, Play button
   - Backdrop uses `.blur()` + gradient overlay for cinematic feel
   - **Carousel rows**: `ScrollView(.horizontal)` with `LazyHStack` of poster cards
   - Rows: Continue Watching, Recently Added, Favorites, then dynamic genre rows (top genres by item count)
   - Each poster card: image + title + year, click pushes to detail view
   - **Empty state**: If no library items, show welcome message with button linking to Settings

4. **Create `MediaGridView.swift`** — reusable poster grid:
   - `LazyVGrid` with adaptive columns (~150pt poster width)
   - Sort dropdown in toolbar: Title, Year, Rating, Date Added (ASC/DESC)
   - Lazy loading: fetch 50 items, load more on scroll (`.onAppear` on last item)
   - Each cell: poster image + title + year overlay
   - Click pushes NavigationLink to detail view

5. **Create `MoviesView.swift`** — `MediaGridView` filtered to `.movie` type
6. **Create `TVShowsView.swift`** — `MediaGridView` filtered to `.show` type
7. **Create `OtherView.swift`** — `MediaGridView` filtered to `.other` type
8. **Create `FavoritesView.swift`** — `MediaGridView` populated from favorites query

**Files:**
- New: `Models/LibraryModel.swift`, `Views/HomeView.swift`, `Views/MediaGridView.swift`, `Views/MoviesView.swift`, `Views/TVShowsView.swift`, `Views/OtherView.swift`, `Views/FavoritesView.swift`, `Components/CachedImageView.swift`, `Components/PosterCard.swift`, `Components/CarouselRow.swift`

**Success criteria:**
- [ ] Home screen shows hero banner with backdrop image
- [ ] Carousel rows populated with real library data
- [ ] Dynamic genre rows appear based on library content
- [ ] Movies/TV/Other grids show poster images with sort dropdown
- [ ] Favorites grid shows favorited items
- [ ] Empty states show helpful messages with Settings link
- [ ] Smooth scrolling with lazy loading on 1000+ item libraries

**Estimated effort:** Large

---

#### Phase 4: Detail View + Actions

**Goal**: Cinematic detail view with full metadata, actions, and TV show drill-down.

**Tasks:**

1. **Create `DetailView.swift`**:
   - **Cinematic backdrop**: Full-width backdrop image, dimmed/blurred, gradient fade to dark at bottom
   - **Metadata card** with `.glassEffect()`:
     - Poster thumbnail (left)
     - Title, year, rating (stars), runtime, genre tags
     - Summary text (expandable)
   - **Action buttons**: Play, Mark Watched/Unwatched, Favorite toggle, Add to Collection, Fix Match, Download (Plex items only)
   - **TV show mode**: Season picker (horizontal tabs or dropdown), episode list below with episode number, title, runtime, watched indicator, and play button per episode

2. **Create `SeasonPickerView.swift`**:
   - Horizontal scrolling tabs for seasons
   - Loads episodes via `reel_library_get_items_by_parent(season_id)`

3. **Create `EpisodeRowView.swift`**:
   - Episode number, thumbnail (if available), title, runtime
   - Watched indicator (checkmark)
   - Play button

4. **Create `FixMatchSheet.swift`**:
   - Sheet/modal with TMDB search field
   - Pre-populated with current title
   - Results list: poster thumbnail, title, year, overview
   - Selecting a result triggers metadata update + `reel_match_set_locked`

5. **Create `CollectionSheet.swift`**:
   - Sheet to add item to existing collection or create new one
   - Lists existing collections from `reel_collection_list`
   - "New Collection" option with name field

**Files:**
- New: `Views/DetailView.swift`, `Views/SeasonPickerView.swift`, `Views/EpisodeRowView.swift`, `Sheets/FixMatchSheet.swift`, `Sheets/CollectionSheet.swift`

**Success criteria:**
- [ ] Detail view shows cinematic backdrop + metadata with Liquid Glass card
- [ ] All action buttons functional (Play deferred to Phase 5)
- [ ] TV show drill-down: Show → Season tabs → Episode list
- [ ] Fix Match sheet searches TMDB and updates metadata
- [ ] Add to Collection sheet works with existing and new collections
- [ ] Watched/Unwatched toggle updates database

**Estimated effort:** Large

---

#### Phase 5: Video Player Integration

**Goal**: mpv video playback working in SwiftUI with full window takeover and controls overlay.

**Tasks:**

1. **Migrate VideoView to Metal**:
   - Replace `NSOpenGLView` with `MTKView` or `CAMetalLayer`-backed `NSView`
   - Configure mpv with `--gpu-api=vulkan --gpu-context=moltenvk` or `--gpu-api=opengl` with `CAOpenGLLayer` (test what works on macOS 26)
   - Wire mpv render callback to trigger Metal/OpenGL redraw
   - This is the highest-risk technical item

2. **Create `PlayerView.swift`** — NSViewRepresentable wrapper:
   ```swift
   struct PlayerView: NSViewRepresentable {
       let player: OpaquePointer  // ReelPlayer*
       func makeNSView(context: Context) -> VideoView { ... }
       func updateNSView(_ view: VideoView, context: Context) { }
       func makeCoordinator() -> Coordinator { ... }
   }
   ```
   - Coordinator marshals mpv update callbacks to SwiftUI state
   - `@Observable PlayerModel` receives position/duration/state updates

3. **Create `PlayerModel.swift`** — `@Observable` class:
   - `position: Double`, `duration: Double`, `state: PlayerState`, `volume: Double`
   - Timer-based polling (250ms) calling `reel_player_get_position/duration/state/volume`
   - Methods: `play(filePath:)`, `togglePause()`, `seek(to:)`, `setVolume(_:)`, `stop()`

4. **Create `PlayerControlsOverlay.swift`** — SwiftUI replacement for AppKit controls:
   - Semi-transparent overlay at bottom with `.glassEffect()`
   - Play/Pause button, seek slider, time label (current / total), volume slider, fullscreen button
   - Auto-hide: fade out after 3 seconds of no mouse movement (use `onHover` + Timer)
   - Keyboard shortcuts: Space (pause), Left/Right (seek ±10s), Up/Down (volume), F (fullscreen), Escape (exit player)

5. **Implement full window takeover**:
   - When Play is triggered, hide sidebar (`columnVisibility = .detailOnly`)
   - Push `PlayerView` onto NavigationStack
   - Escape: stop player, pop navigation, restore sidebar (`columnVisibility = .all`)
   - F or double-click: enter macOS fullscreen Space

6. **Wire playback from DetailView**:
   - Play button calls `playerModel.play(filePath:)` → triggers window takeover
   - Save watch progress on pause/stop via `reel_library_update_watch_progress`
   - Resume from saved position via `reel_library_get_watch_progress`

7. **Handle CLI direct play**:
   - If launched with file argument, skip sidebar, go straight to full-window player

**Files:**
- Modified: `VideoView.swift` (Metal migration)
- New: `Views/PlayerView.swift`, `Models/PlayerModel.swift`, `Components/PlayerControlsOverlay.swift`
- Deleted: `PlayerControlsView.swift` (replaced by SwiftUI overlay)

**Success criteria:**
- [ ] Video renders correctly via Metal/MoltenVK in SwiftUI
- [ ] Player controls overlay with Liquid Glass effect
- [ ] Auto-hide controls after 3 seconds
- [ ] Full window takeover: sidebar hides, Escape returns
- [ ] Keyboard shortcuts work (Space, arrows, F, Escape)
- [ ] Watch progress saved and restored
- [ ] CLI `reel /path/to/file.mkv` plays directly

**Estimated effort:** Large (highest risk phase — Metal migration)

---

#### Phase 6: Settings, Downloads, Files

**Goal**: All remaining sidebar views fully functional.

**Tasks:**

1. **Create `SettingsView.swift`**:
   - Sections using SwiftUI `Form` + `Section`:
     - **Plex**: Connected servers list, "Add Server" button → auth flow, remove server
     - **Library**: Local scan paths list, "Add Folder" (NSOpenPanel via `.fileImporter`), remove path, "Scan Now" button
     - **Playback**: Preferred subtitle/audio language pickers, hardware acceleration toggle
     - **Storage**: Download directory, image cache size, "Clear Cache" button
   - Port Plex auth flow from `SettingsViewController.swift` to SwiftUI with `PlexAuthModel`

2. **Create `PlexAuthModel.swift`** — `@Observable`:
   - States: disconnected, waitingForAuth(pin, url), connected(servers)
   - `requestPin()` → opens browser, starts polling
   - Polling with `Task` + `try await Task.sleep` instead of Timer
   - Timeout after 5 minutes (300 seconds)
   - Server picker when multiple servers discovered (not just first)

3. **Create `DownloadsView.swift`**:
   - List of downloads with `DownloadModel`
   - Each row: title, progress bar, status (downloading/paused/completed/failed), speed
   - Context menu: Pause, Resume, Cancel, Play (completed), Remove
   - Empty state: "No downloads — download Plex items for offline viewing"
   - Poll `reel_download_list` every 1 second when view is visible

4. **Create `DownloadModel.swift`** — `@Observable`:
   - `downloads: [Download]` — periodically refreshed
   - Methods: `enqueue(mediaItemId:serverId:url:)`, `pause(id:)`, `resume(id:)`, `remove(id:)`

5. **Create `FilesView.swift`**:
   - Two sections: Plex Servers, Local Folders
   - Plex servers from `reel_server_list` with connection status
   - Local folders from `reel_library_list_scan_paths`
   - Clicking a Plex server → shows library sections (future: browse items)
   - Clicking a local folder → shows file listing
   - "Add Source" button links to Settings

**Files:**
- New: `Views/SettingsView.swift`, `Views/DownloadsView.swift`, `Views/FilesView.swift`, `Models/PlexAuthModel.swift`, `Models/DownloadModel.swift`, `Models/SettingsModel.swift`
- Deleted: `SettingsViewController.swift` (fully replaced)

**Success criteria:**
- [ ] Settings: Plex auth flow works end-to-end with server picker
- [ ] Settings: Add/remove scan paths, trigger scan
- [ ] Downloads: Queue shows progress, pause/resume/cancel work
- [ ] Files: Shows Plex servers and local paths
- [ ] Plex auth timeout after 5 minutes
- [ ] All empty states have helpful messages

**Estimated effort:** Medium-Large

---

#### Phase 7: Polish + Accessibility

**Goal**: Error handling, loading states, keyboard navigation, VoiceOver, performance.

**Tasks:**

1. **Loading states**: Skeleton/shimmer placeholders in grids and carousels while data loads
2. **Error states**: Alert/banner for network errors, database errors, file-not-found. Retry buttons where applicable
3. **Empty states**: All views have actionable empty states (buttons to Settings, explanatory text)
4. **Keyboard navigation**:
   - Arrow keys in poster grid (focus management)
   - Number keys 1-8 for sidebar items
   - Cmd+F for search
   - Tab navigation through all interactive elements
5. **VoiceOver**:
   - `.accessibilityLabel` on all poster cards (title, year, media type)
   - `.accessibilityValue` on seek slider ("2 minutes 30 seconds of 1 hour 45 minutes")
   - `.accessibilityAction` for custom interactions
6. **Reduced motion**: Respect `AccessibilitySettings.isReduceMotionEnabled` — skip fade animations, hero backdrop parallax
7. **Performance**: Verify smooth scrolling at 1000+ items with `LazyVGrid`. Profile image loading pipeline. Ensure view switching < 100ms
8. **Menu bar**: File → Open (play file), Playback menu (Play/Pause, Skip, Fullscreen), View → Toggle Sidebar
9. **Window management**: Remember window size/position, minimum size 800×500, default 1280×720
10. **Database error handling**: If `reel_db_open` fails, show error alert instead of silent nil

**Success criteria:**
- [ ] No crashes on empty library, disconnected server, or file not found
- [ ] VoiceOver can navigate all views and read all content
- [ ] Keyboard-only navigation works throughout
- [ ] Smooth scrolling with 1000+ items
- [ ] Window size persists across launches

**Estimated effort:** Medium

## System-Wide Impact

### Interaction Graph

- Sidebar selection → `@State` change → `NavigationSplitView` detail updates → `@Observable` model fetches data via C ABI → SQLite query / Plex API → results converted to Swift types → SwiftUI redraws
- Poster click → `NavigationLink` pushes `DetailView` → `LibraryModel.getItemById()` → C ABI → render metadata
- Play click → `PlayerModel.play()` → `reel_player_load_file` → mpv starts → `NSViewRepresentable` renders frames → `columnVisibility = .detailOnly` hides sidebar
- Download enqueue → `DownloadModel.enqueue()` → `reel_download_enqueue` → Zig worker thread downloads → `DownloadModel` polls progress every 1s → SwiftUI updates progress bar

### Error Propagation

- C ABI returns `ReelError` codes → Swift bridge maps to `enum ReelError: Error` → `@Observable` models surface errors as `@Published error: ReelError?` → Views show `.alert()` or inline error banner
- Network errors (Plex API) → C ABI returns error → same path
- File not found → `reel_player_load_file` returns error → `PlayerModel` surfaces error → alert shown, return to library
- Database failure → `reel_db_open` returns nil → `ReelApp` shows fatal error alert

### State Lifecycle Risks

- **View state during navigation**: `NavigationStack` preserves view state on push/pop. Sidebar selection change does NOT destroy in-flight async operations — `@Observable` models live in the environment, not in views
- **Playback state during navigation**: `PlayerModel` is environment-level, not view-level. Player continues if user somehow navigates (though sidebar is hidden during playback)
- **Concurrent database access**: Zig's SQLite wrapper uses WAL mode + mutex. Safe for concurrent reads from UI thread + writes from scanner/downloader

### API Surface Parity

- Every `export fn` in `lib.zig` must match a declaration in `reel.h`
- Every sidebar view in GTK4 frontend must have a SwiftUI equivalent
- Every C struct type must have a Swift bridge struct

## Acceptance Criteria

### Functional Requirements

- [ ] SwiftUI app launches with NavigationSplitView sidebar on macOS 26
- [ ] Home view: hero banner, Continue Watching, Recently Added, Favorites, genre carousels
- [ ] Movies/TV Shows/Other: poster grid with sort dropdown
- [ ] Detail view: cinematic backdrop, metadata, Play, Fix Match, Collection, Watched, Favorite, Download
- [ ] TV drill-down: Show → Seasons → Episodes
- [ ] Player: full window takeover, controls overlay, keyboard shortcuts, Escape to return
- [ ] Settings: Plex auth with server picker, scan paths, TMDB config
- [ ] Downloads: queue with progress, pause/resume/cancel
- [ ] Files: Plex servers + local paths
- [ ] Favorites: grid of favorited items
- [ ] First launch: empty state guides to Settings
- [ ] CLI `reel /path/to/file.mkv` plays directly

### Non-Functional Requirements

- [ ] Liquid Glass sidebar and player controls on macOS 26
- [ ] Poster grid scrolls smoothly with 1000+ items (lazy loading)
- [ ] View switching < 100ms
- [ ] Images load asynchronously, never block UI
- [ ] VoiceOver navigable throughout
- [ ] Keyboard-only navigation works
- [ ] Reduced motion respected

### Quality Gates

- [ ] All C ABI functions declared in `reel.h` have implementations in `lib.zig`
- [ ] `swift build` links successfully against expanded libreel
- [ ] No memory leaks from C string bridging (reel_free used consistently)
- [ ] Video renders via Metal (not deprecated OpenGL)

## Dependencies & Prerequisites

- **Phase 1 (C ABI) blocks Phases 3-6** — views need data queries
- **Phase 2 (Shell) blocks Phases 3-6** — need the app structure to add views into
- **Phase 3 (Home + Grid) and Phase 4 (Detail) can partially overlap** — grid is prerequisite for detail navigation
- **Phase 5 (Player) is independent of 3-4** — can develop in parallel once shell exists
- **Phase 6 (Settings/Downloads/Files) is independent of 3-5**
- **Phase 7 (Polish) depends on all views existing**

```
Phase 1 (C ABI) ─┬─→ Phase 2 (Shell) ─┬─→ Phase 3 (Home + Grid) → Phase 4 (Detail)
                  │                     ├─→ Phase 5 (Player)
                  │                     └─→ Phase 6 (Settings/Downloads/Files)
                  └──────────────────────→ Phase 7 (Polish) [after 3-6]
```

## Risk Analysis & Mitigation

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Metal/mpv rendering doesn't work on macOS 26 | Medium | Critical | Prototype early in Phase 5. Fallback: `CAOpenGLLayer` if OpenGL still available. Worst case: `AVPlayer` for local files |
| Liquid Glass APIs change before Tahoe release | Low | Medium | Use standard `.glassEffect()` modifiers. If API changes, update is localized |
| C ABI string ownership causes memory leaks | Medium | Medium | Implement `reel_free()` in Phase 1. Create Swift bridge that always frees at conversion boundary |
| Large library performance (10k+ items) | Medium | Medium | `LazyVGrid` + pagination from the start. Profile with large dataset early |
| macOS 26 SDK not yet available | High | High | Develop against macOS 15 SDK initially, use `#if available` for Liquid Glass. Port to 26 SDK when available |

## Sources & References

### Origin

- **Brainstorm document:** [docs/brainstorms/2026-03-15-macos-swiftui-rewrite-brainstorm.md](docs/brainstorms/2026-03-15-macos-swiftui-rewrite-brainstorm.md) — Key decisions: full SwiftUI rewrite, macOS 26 Tahoe, Infuse-style cinematic design, NavigationSplitView + NavigationStack, full window takeover for playback

### Internal References

- C ABI header: `include/reel.h`
- C ABI exports (gaps here): `src/lib.zig`
- Current macOS AppKit code: `macos/Reel/Sources/*.swift`
- Build config: `macos/Package.swift`
- Module map: `macos/libreel/module.modulemap`
- Zig library queries: `src/core/library.zig`
- Zig player: `src/core/player.zig`
- Existing sidebar plan: `docs/plans/2026-03-15-feat-infuse-sidebar-navigation-views-plan.md`
- Existing metadata plan: `docs/plans/2026-03-15-feat-infuse-parity-library-metadata-plan.md`

### External References

- [Apple SwiftUI NavigationSplitView](https://developer.apple.com/documentation/swiftui/navigationsplitview)
- [Apple NSViewRepresentable](https://developer.apple.com/documentation/swiftui/nsviewrepresentable)
- [Apple Liquid Glass glassEffect](https://developer.apple.com/documentation/swiftui/view/glasseffect(_:in:))
- [Apple Observable macro](https://developer.apple.com/documentation/observation/observable())
- [mpv render API](https://mpv.io/manual/master/#embedding-into-other-programs-(libmpv))
- [Infuse for Mac](https://firecore.com/infuse)
