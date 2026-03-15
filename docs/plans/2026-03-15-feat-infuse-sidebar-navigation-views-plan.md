---
title: "feat: Replicate Infuse Sidebar Navigation with All Views"
type: feat
status: active
date: 2026-03-15
origin: docs/brainstorms/2026-03-14-reel-media-center-brainstorm.md
---

# Replicate Infuse Sidebar Navigation with All Views

## Overview

Add a full sidebar navigation system to Reel on both GTK4 (Linux) and macOS (AppKit), replicating Infuse's sidebar structure. Transform the current single-view video player into a multi-view media center with 8 sidebar views: Home, Movies, TV Shows, Other, Favorites, Files, Downloads, and Settings. This requires expanding the C ABI, adding missing database queries, creating new UI views on both platforms, and implementing an image loading pipeline.

## Problem Statement

Reel is currently a single-view video player — it opens a window, plays a file passed via CLI argument, and shows OSD playback controls. There is no way to browse media, manage a library, connect to Plex, configure settings, or do anything beyond playing a single video file. The entire media center experience described in the brainstorm (see brainstorm: `docs/brainstorms/2026-03-14-reel-media-center-brainstorm.md`) is missing.

Infuse's sidebar provides the primary navigation model that users expect from a media center. Without it, Reel is a video player, not a media center.

## Proposed Solution

Implement Infuse's sidebar navigation with the following views:

| Sidebar Item | Purpose | Data Sources |
|---|---|---|
| **Home** | Landing page: Up Next, Recently Added, On Deck rows | `library.zig` + `plex/client.zig` |
| **Movies** | Poster grid of all movies with sort/filter | `library.zig` + `plex/client.zig` |
| **TV Shows** | Poster grid of all TV shows with drill-down to seasons/episodes | `library.zig` + `plex/client.zig` |
| **Other** | Media not matching movie/show patterns | `library.zig` |
| **Favorites** | User-pinned items, folders, categories | New `favorites` table |
| **Files** | Browse connected sources (Plex servers, local folders) | `library.zig` + `plex/client.zig` |
| **Downloads** | Offline download queue and completed downloads | `downloader.zig` |
| **Settings** | TMDB key, library folders, Plex connections, preferences | `settings.zig` |

Plus a **Detail View** (not in sidebar — navigated to by clicking a poster) and a **Player View** (replaces content area during playback).

### Navigation Model

- **Split layout**: Sidebar on the left, content area on the right
- **Sidebar is always visible** except during fullscreen playback
- **Detail view**: Clicking a poster pushes a detail view onto the content area (back button to return)
- **Player**: Clicking "Play" replaces the content area with the player. Pressing Escape returns to the previous view. Fullscreen hides the sidebar entirely
- **View state is preserved**: Switching sidebar items and back preserves scroll position and filters
- **First launch**: If no sources are configured, show a welcome prompt in Home with links to Settings

### Platform Widgets

| Concern | GTK4 (Linux) | macOS (AppKit) |
|---|---|---|
| Sidebar container | `AdwOverlaySplitView` | `NSSplitViewController` |
| Sidebar list | `GtkListBox` with rows | `NSOutlineView` with `NSSidebarListStyle` |
| Content switching | `GtkStack` | Swap child `NSViewController` |
| Detail navigation | `AdwNavigationView` (push/pop) | `NSViewController` push onto stack |
| Poster grid | `GtkFlowBox` with `GtkPicture` children | `NSCollectionView` |
| Responsive | `AdwOverlaySplitView` collapses sidebar on narrow windows | `NSSplitViewController` min width |

## Technical Approach

### Architecture

```
Sidebar Click
    │
    ▼
┌─────────────────────────────────────────┐
│           Frontend (GTK4 / AppKit)       │
│  ┌──────────┐  ┌──────────────────────┐ │
│  │ Sidebar   │  │ Content Area         │ │
│  │ (ListBox/ │  │ (Stack/NavView)      │ │
│  │ Outline)  │  │                      │ │
│  │           │  │  ┌────────────────┐  │ │
│  │ > Home    │  │  │  Active View   │  │ │
│  │   Movies  │  │  │  (poster grid, │  │ │
│  │   TV Shows│  │  │   detail, etc) │  │ │
│  │   Other   │  │  └────────────────┘  │ │
│  │   ─────── │  │                      │ │
│  │   Favorites│ │                      │ │
│  │   Files   │  │                      │ │
│  │   ─────── │  │                      │ │
│  │   Downloads│ │                      │ │
│  │   Settings│  │                      │ │
│  └──────────┘  └──────────────────────┘ │
│           │          C ABI              │
│  ┌────────┴──────────────────────────────┐
│  │         libreel (Zig Core)            │
│  │  library queries, settings, auth,     │
│  │  downloads, scanner, image cache      │
│  └───────────────────────────────────────┘
└─────────────────────────────────────────┘
```

### Implementation Phases

#### Phase 1: Data Layer & C ABI Foundation

**Goal**: All data the sidebar views need is queryable from the Zig core AND accessible via the C ABI for the macOS frontend.

**Tasks and deliverables:**

1. **Add missing query functions to `library.zig`**:
   - `getItemsByType(media_type, sort_by, sort_order, limit, offset) -> []MediaItem`
   - `getRecentlyAdded(limit) -> []MediaItem`
   - `getContinueWatching(limit) -> []MediaItem` (join `media_items` + `watch_progress` WHERE `watched=false AND position_ms > 0`)
   - `searchItems(query, media_type_filter) -> []MediaItem`
   - `getItemsByParent(parent_id) -> []MediaItem` (seasons of a show, episodes of a season)
   - `listServers() -> []Server`
   - `listScanPaths() -> []ScanPath`
   - `getItemCount(media_type) -> u32`

2. **Add `.other` variant to `MediaType`** in `types.zig`. Update scanner to assign `.other` when filename doesn't match movie/show patterns.

3. **Add `favorites` table** via database migration v2 in `database.zig`:
   ```sql
   CREATE TABLE favorites (
       id INTEGER PRIMARY KEY,
       item_type TEXT NOT NULL,  -- 'media_item', 'plex_library', 'scan_path', 'filter'
       item_id TEXT NOT NULL,
       display_name TEXT NOT NULL,
       sort_order INTEGER NOT NULL DEFAULT 0,
       created_at TEXT NOT NULL DEFAULT (datetime('now'))
   );
   ```
   Add CRUD functions: `addFavorite`, `removeFavorite`, `listFavorites`, `reorderFavorite`.

4. **Implement image loading pipeline** in new `src/core/image_cache.zig`:
   - Download poster/backdrop from URL (TMDB or Plex)
   - Cache to disk at `~/.local/share/reel/images/` (Linux) or `~/Library/Caches/Reel/images/` (macOS)
   - LRU eviction based on `image_cache_max_mb` setting
   - Return local file path for a given image URL
   - Async download with callback notification

5. **Expand `include/reel.h` C ABI** with exported functions for:
   - Library queries (get by type, search, recently added, continue watching)
   - Settings read/write
   - Plex auth (request PIN, poll PIN)
   - Server management (list, add, remove)
   - Download management (enqueue, list, pause, resume, cancel)
   - Favorites CRUD
   - Image cache (get local path for URL, trigger download)
   - Scanner control (add scan path, trigger scan)

6. **Implement all `export fn` declarations** in `src/lib.zig` wrapping the internal Zig functions with C-compatible types.

7. **Fix hardcoded `platform = "Linux"`** in `src/net/plex/types.zig:6` — make it a comptime constant based on `@import("builtin").os.tag`.

**Success criteria:**
- [x] All new library queries have unit tests
- [x] Favorites table created via migration
- [ ] Image cache downloads and serves a test image
- [ ] `reel.h` header compiles when included from C
- [ ] macOS `Package.swift` links against the expanded library without errors

**Estimated effort:** Large — this is the biggest phase and blocks all UI work.

#### Phase 2: GTK4 Sidebar Shell & View Switching

**Goal**: GTK4 app has a working sidebar that switches between placeholder views.

**Tasks and deliverables:**

1. **Restructure `app.zig`**:
   - Replace the current vertical box (header + overlay) with `AdwOverlaySplitView`
   - Sidebar pane: `GtkListBox` with 8 rows (Home, Movies, TV Shows, Other, separator, Favorites, Files, separator, Downloads, Settings)
   - Content pane: `GtkStack` holding one widget per view
   - Wire sidebar row selection to stack page switching

2. **Create `src/apprt/gtk/window.zig`** — extract window management from `app.zig`:
   - Window creation, sidebar setup, content stack
   - View lifecycle management (create views lazily on first visit)
   - Track active view for keyboard context

3. **Create placeholder views** (each as its own file under `src/apprt/gtk/`):
   - `home_view.zig` — placeholder label "Home"
   - `movies_view.zig` — placeholder label "Movies"
   - `tv_shows_view.zig` — placeholder label "TV Shows"
   - `other_view.zig` — placeholder label "Other"
   - `favorites_view.zig` — placeholder label "Favorites"
   - `files_view.zig` — placeholder label "Files"
   - `downloads_view.zig` — placeholder label "Downloads"
   - `settings_view.zig` — placeholder label "Settings"

4. **Make `keys.zig` context-aware**: Only handle playback keys when player is active. Add navigation keys (1-8 for sidebar items, Ctrl+F for search).

5. **Reconcile CLI file argument**: If `reel /path/to/file.mkv` is invoked, skip sidebar and go straight to player (preserve current behavior). If no argument, show sidebar with Home view.

**Success criteria:**
- [x] App launches with sidebar visible
- [x] Clicking sidebar items switches the content area
- [x] Sidebar collapses on narrow windows (AdwOverlaySplitView adaptive)
- [x] `reel /path/to/file.mkv` still works (direct playback)
- [ ] Keyboard shortcuts 1-8 switch sidebar views

**Estimated effort:** Medium

#### Phase 3: GTK4 Core Views (Movies, TV Shows, Detail, Player)

**Goal**: Users can browse movies/TV shows as poster grids, view details, and play content — the core media center loop.

**Tasks and deliverables:**

1. **Poster grid widget** — shared component used by Movies, TV Shows, Home:
   - `GtkFlowBox` with fixed-size children (poster + title label)
   - `GtkPicture` for poster images loaded from the image cache
   - Placeholder image (gray with title text) while loading
   - Click handler navigates to detail view

2. **`movies_view.zig`** — full implementation:
   - Calls `getItemsByType(.movie, ...)` from library
   - Renders poster grid
   - Toolbar with sort dropdown (Title, Year, Rating, Date Added) and search entry
   - Lazy loading: fetch 50 items, load more on scroll

3. **`tv_shows_view.zig`** — full implementation:
   - Same poster grid as movies, filtered to `.show` type
   - Clicking a show navigates to detail view with season/episode drill-down

4. **`detail_view.zig`** — new file:
   - Pushed onto `AdwNavigationView` when a poster is clicked
   - Shows: backdrop image, poster, title, year, rating, duration, summary, genres
   - Play button, Mark Watched/Unwatched toggle
   - For TV shows: season tabs, episode list with watched indicators
   - Download button (for Plex items)
   - Back button returns to the poster grid (preserving scroll position)

5. **Player integration** — restructure `video_area.zig` + `player_controls.zig`:
   - Player view replaces content area (pushed onto nav stack)
   - Sidebar remains visible but can be hidden
   - Fullscreen mode hides sidebar + header bar (existing behavior)
   - Escape exits fullscreen; second Escape returns to previous view
   - Player state (position, duration) syncs to `watch_progress`

6. **`other_view.zig`** — same poster grid, filtered to `.other` type.

**Success criteria:**
- [ ] Movies view shows poster grid with images
- [ ] Sort and search work
- [ ] Clicking a poster shows detail view with metadata
- [ ] Clicking Play starts playback
- [ ] Escape returns to library view
- [ ] TV show drill-down: Show -> Seasons -> Episodes works
- [ ] Watch progress is saved and "Continue Watching" appears on Home

**Estimated effort:** Large

#### Phase 4: GTK4 Remaining Views (Home, Favorites, Files, Downloads, Settings)

**Goal**: All 8 sidebar views are fully functional on GTK4.

**Tasks and deliverables:**

1. **`home_view.zig`** — full implementation:
   - Horizontal scrolling rows: "Continue Watching", "Recently Added", "On Deck" (Plex)
   - Each row is a horizontal `GtkFlowBox` or `GtkListBox` with poster thumbnails
   - Clicking a poster navigates to detail view
   - Empty state: "Welcome to Reel — connect to Plex or add local folders in Settings"

2. **`favorites_view.zig`** — full implementation:
   - Grid/list of favorited items
   - Context menu to remove from favorites
   - Empty state: "No favorites yet — right-click any item to add it"

3. **`files_view.zig`** — full implementation:
   - Tree/list of connected sources (Plex servers, local scan paths)
   - Clicking a Plex server shows its libraries
   - Clicking a library shows items in poster grid
   - Clicking a local folder shows file listing
   - "Add Source" button (links to Settings)
   - Connection status indicators (online/offline)

4. **`downloads_view.zig`** — full implementation:
   - List of downloads with progress bars
   - Status: downloading, paused, completed, failed
   - Context menu: pause, resume, cancel, play (completed), remove
   - Empty state: "No downloads — download Plex items for offline viewing"

5. **`settings_view.zig`** — full implementation:
   - Sections with `AdwPreferencesPage` / `AdwPreferencesGroup`:
     - **Plex**: Connected servers list, "Add Server" button (triggers auth flow), remove server
     - **Library**: Local scan paths list, "Add Folder" button, remove path, "Scan Now" button
     - **Metadata**: TMDB API key entry
     - **Playback**: Preferred subtitle/audio language, hardware acceleration toggle
     - **Storage**: Download path, image cache size limit, "Clear Cache" button
   - Use `AdwActionRow`, `AdwEntryRow`, `AdwSwitchRow` for native look

6. **`auth_view.zig`** — Plex PIN authentication (shown modally from Settings):
   - Display PIN code
   - "Open Browser" button to open `plex.tv/link`
   - Polling indicator
   - Success → server added, return to Settings

7. **Add context menu to poster grid items**: Right-click → "Add to Favorites", "Mark as Watched/Unwatched", "Download" (Plex items)

**Success criteria:**
- [ ] Home view shows Continue Watching, Recently Added, On Deck
- [ ] First-launch empty state guides user to Settings
- [ ] Favorites can be added/removed via context menu
- [ ] Files view shows Plex servers and local folders
- [ ] Downloads view shows queue with progress
- [ ] Settings allows full app configuration
- [ ] Plex auth flow works end-to-end

**Estimated effort:** Large

#### Phase 5: macOS Sidebar & Views

**Goal**: macOS frontend has feature parity with GTK4 for all sidebar views.

**Tasks and deliverables:**

1. **Restructure `MainWindow.swift`**:
   - Use `NSSplitViewController` with sidebar + content
   - Sidebar: `NSOutlineView` with `NSSidebarListStyle`
   - Sections: "Library" (Home, Movies, TV Shows, Other), "Sources" (Favorites, Files), "Other" (Downloads, Settings)
   - Content: swap `NSViewController` children

2. **Create Swift view controllers** (under `macos/Reel/Sources/Views/`):
   - `SidebarViewController.swift`
   - `HomeViewController.swift`
   - `MoviesViewController.swift`
   - `TVShowsViewController.swift`
   - `OtherViewController.swift`
   - `FavoritesViewController.swift`
   - `FilesViewController.swift`
   - `DownloadsViewController.swift`
   - `SettingsViewController.swift` (or use `NSPreferencesWindow`)
   - `DetailViewController.swift`

3. **Poster grid on macOS**: `NSCollectionView` with `NSCollectionViewFlowLayout`
   - Same image cache C ABI for poster loading
   - Click handler → detail view

4. **Wire all views to C ABI functions** from Phase 1.

5. **macOS-native patterns**:
   - Settings as a proper Preferences window (`NSPreferencesWindow`)
   - Toolbar with search field
   - Touch Bar support for playback controls (if applicable)
   - Menu bar integration (File > Open, Playback menu items)

**Success criteria:**
- [x] macOS app launches with sidebar
- [ ] All 8 views functional with data from Zig core
- [ ] Poster grid shows images
- [ ] Detail view and playback work
- [ ] Settings uses native Preferences window pattern
- [ ] Feature parity with GTK4 frontend

**Estimated effort:** Large

#### Phase 6: Polish & Edge Cases

**Goal**: Handle all error states, empty states, loading states, and accessibility.

**Tasks and deliverables:**

1. **Loading states**: Spinner/skeleton in poster grids while data loads
2. **Error states**: Toast notifications for network errors, retry buttons
3. **Empty states**: Helpful messages with action buttons for each view
4. **Pagination**: Lazy loading for large libraries (50 items per page, load on scroll)
5. **Plex pagination**: Add `X-Plex-Container-Start/Size` headers to API calls
6. **Accessibility** (both platforms):
   - Keyboard navigation through sidebar and poster grid
   - Screen reader labels for all interactive elements
   - Focus indicators
   - Respect system accessibility settings (reduced motion, high contrast)
7. **Search**: Global search field in sidebar header, results grouped by type
8. **Drag-and-drop**: Drop video files on window to play immediately

**Success criteria:**
- [ ] No crashes on empty library, disconnected server, or network failure
- [ ] Keyboard-only navigation works through all views
- [ ] Screen reader announces all interactive elements
- [ ] Large libraries (10,000+ items) scroll smoothly with lazy loading

**Estimated effort:** Medium

## System-Wide Impact

### Interaction Graph

- Sidebar click → `GtkStack`/`NSViewController` swap → view's `activate()` method → C ABI query → SQLite/Plex API → callback with results → UI update on main thread
- Poster click → push detail view onto `AdwNavigationView`/`NSViewController` stack → C ABI `getMediaItem()` → render metadata
- Play click → push player view → C ABI `reel_player_load_file()` → mpv starts playback → existing render loop in `video_area.zig`/`VideoView.swift`
- Download enqueue → C ABI `reel_download_enqueue()` → background thread downloads → progress callback → UI update → completion notification

### Error Propagation

- Plex API errors (network, auth expired) → C ABI returns error code → frontend shows toast/banner with retry
- SQLite errors → C ABI returns null/error → frontend shows "Database error" (should not happen in practice)
- Image download failures → cache returns null path → frontend shows placeholder image
- mpv errors → existing error handling in player, propagates via event callbacks

### State Lifecycle Risks

- **View state during navigation**: Switching sidebar items must not destroy in-progress async operations (e.g., image downloads). Use `GtkStack` which keeps all children alive.
- **Playback state during navigation**: Player must continue playing when pushing/popping views. The mpv context lives in `AppState`, not in a view.
- **Database state during concurrent access**: SQLite WAL mode + mutex in `database.zig` handles concurrent reads from UI thread + writes from scanner/downloader threads.

### API Surface Parity

- Every C ABI function in `reel.h` must have a corresponding implementation in `src/lib.zig`
- Every GTK4 view must have a corresponding macOS view controller
- Every sidebar item must be present on both platforms

## Acceptance Criteria

### Functional Requirements

- [ ] Sidebar with 8 navigation items on both GTK4 and macOS
- [ ] Home view with Continue Watching, Recently Added, On Deck rows
- [ ] Movies view with poster grid, sort, and search
- [ ] TV Shows view with poster grid and season/episode drill-down
- [ ] Other view for non-movie/show content
- [ ] Favorites view with add/remove functionality
- [ ] Files view showing connected Plex servers and local folders
- [ ] Downloads view with queue management
- [ ] Settings view with full app configuration
- [ ] Detail view with metadata, play button, and download button
- [ ] Player integration: play from detail view, Escape to return
- [ ] Fullscreen playback hides sidebar
- [ ] First-launch experience guides user to configure sources
- [ ] CLI `reel /path/to/file.mkv` skips sidebar, plays directly

### Non-Functional Requirements

- [ ] Poster grid scrolls smoothly with 1000+ items (lazy loading)
- [ ] View switching is instant (< 100ms)
- [ ] Images load asynchronously without blocking the UI
- [ ] Sidebar adapts to narrow windows on GTK4
- [ ] Keyboard navigation works for all views

### Quality Gates

- [ ] Unit tests for all new library query functions
- [ ] Unit tests for favorites CRUD
- [ ] Unit tests for image cache
- [ ] C ABI header compiles clean from C
- [ ] macOS build links successfully against expanded libreel
- [ ] No memory leaks (Zig's allocator tracking)

## Dependencies & Prerequisites

- **Phase 1 blocks all other phases** — views cannot be built without data queries and C ABI
- **Image cache (Phase 1) blocks Phase 3** — poster grids need images
- **GTK4 phases (2-4) are independent of macOS (Phase 5)** — can be parallelized
- **Phase 6 (Polish) depends on all views existing**
- **Existing infrastructure**: libmpv, SQLite, Plex client, TMDB client, scanner, downloader — all implemented and tested

## Risk Analysis & Mitigation

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| C ABI design churn | Medium | High | Design full header before implementing; review with both frontends in mind |
| Image loading performance | Medium | Medium | Async loading with thread pool; LRU disk cache; placeholder images |
| GTK4 `AdwOverlaySplitView` API complexity | Low | Medium | Well-documented in libadwaita; Celluloid/GNOME apps as reference |
| macOS `NSOutlineView` sidebar styling | Low | Low | Standard macOS pattern, well-documented |
| Large library performance | Medium | Medium | Pagination from the start; virtual scrolling if needed |
| Threading bugs (async image load + UI update) | Medium | High | All UI updates via `g_idle_add` (GTK) / `DispatchQueue.main` (macOS); no shared mutable state |

## Sources & References

### Origin

- **Brainstorm document:** [docs/brainstorms/2026-03-14-reel-media-center-brainstorm.md](docs/brainstorms/2026-03-14-reel-media-center-brainstorm.md) — Key decisions: Ghostty architecture pattern, libmpv as media backend, Plex as primary network source, GTK4 + AppKit dual frontends

### Internal References

- Existing plan: `docs/plans/2026-03-14-feat-reel-native-media-center-plan.md`
- Technology reference: `docs/TECHNOLOGY_REFERENCE.md`
- GTK app entry: `src/apprt/gtk/app.zig`
- macOS window: `macos/Reel/Sources/MainWindow.swift`
- C ABI header: `include/reel.h`
- Library queries: `src/core/library.zig`
- Data types: `src/core/types.zig`
- Database/migrations: `src/core/database.zig`
- Settings: `src/core/settings.zig`
- Downloader: `src/core/downloader.zig`
- Plex client: `src/net/plex/client.zig`
- TMDB client: `src/net/tmdb/client.zig`

### External References

- [libadwaita AdwOverlaySplitView docs](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/class.OverlaySplitView.html)
- [Apple NSSplitViewController docs](https://developer.apple.com/documentation/appkit/nssplitviewcontroller)
- [Infuse features](https://firecore.com/infuse)
- [Infuse Favorites & Lists](https://support.firecore.com/hc/en-us/articles/360003184653-Managing-Favorites-Lists)
