# Reel Development Tasks

## Phase 1: Core Features ✅

### Remaining Tasks
- [x] **Security & Auth**
  - [x] Store auth token securely in system keyring (Completed for Plex & Jellyfin)
  - [ ] Handle token refresh and expiration
  - [x] Create server selection dialog (for multiple servers) - Backend switcher implemented

### 🔐 Authentication & Source Management
- [x] **Sources Page Implementation** (January 2025)
  - [x] Create dedicated Sources & Accounts page
  - [x] Separate AuthProvider from Source model
  - [x] Support for multiple auth types (Plex accounts, Jellyfin, future network sources)
  - [x] AuthManager service for credential management
  - [x] Move backend configuration from preferences to Sources page
  - [x] Backend constructors accept AuthProvider and Source
  - [x] Centralized credential storage in keyring via AuthManager
  - [x] Fix auth dialog compilation errors (adw::Dialog API usage)
  - [x] Migrate existing backends to AuthProvider model
  - [x] Simplify auth dialog (remove backend selector)
  - [x] Enhanced sources page with exciting UI
  - [x] **Offline-first sources page** - Loads cached data instantly, refreshes in background
  - [x] **Centralized Config management** - Single shared instance via AppState
  - [x] **Source caching** - Providers and sources cached with TTL for instant display
  - [ ] Implement Plex account token refresh
  - [ ] Add network source authentication (SMB, SFTP, WebDAV, NFS)
  - [x] Implement source discovery for Plex accounts (working in SourceCoordinator)
  - [x] Complete auth dialog migration to use SourceCoordinator

### 🖼️ Media & Metadata
- [ ] **Metadata Display**
  - [ ] Create media info cards
  - [ ] Display ratings, duration, genre
  - [ ] Show cast and crew information
  - [ ] Implement synopsis/overview display
  - [ ] Add media badges (4K, HDR, etc.)

### 🔄 Sync & Cache System
- [ ] **Remaining Database Tasks**
  - [ ] Add indexes for performance
  - [ ] Implement cache expiration logic
  - [ ] Handle sync conflicts
  - [ ] Create sync scheduling system

### 🎨 UI Improvements
- [ ] **Navigation & Routing** (PARTIALLY IMPLEMENTED)
  - [x] Basic navigation between pages works
  - [x] Basic back button in player and detail pages
  - [ ] Navigation history only tracks player page (limited implementation)
  - [ ] Add breadcrumb navigation
  - [ ] Create consistent loading states
  - [ ] Add consistent error state displays

- [x] **Source & Backend Management** (January 2025)
  - [x] Create dedicated Sources page for backend management
  - [x] Remove backend configuration from preferences window
  - [x] Implement AuthProvider/Source separation
  - [x] Support multiple sources per auth provider

- [ ] **Proper Settings Management**
  - [ ] Implement GSettings for GNOME-compliant settings storage
  - [ ] Create GSettings schema file (.gschema.xml)
  - [ ] Replace current Config system with GSettings
  - [ ] Auto-reload settings on change (reactive configuration)
  - [ ] Support settings sync across instances
  - [ ] Add settings migration from old config.toml

- [ ] **Server Connection UI**
  - [ ] Implement connection retry UI
  - [ ] Add offline mode banner

### 🎬 Playback Enhancements
- [ ] **Stream Optimization**
  - [ ] Handle transcoding decisions
  - [ ] Implement quality selection
  - [ ] Create playback decision engine

- [ ] **Skip Intro/Credits UI Improvements**
  - [ ] Move skip intro button to bottom-right corner (currently top-right)
  - [ ] Clean up verbose debug logging for markers
  - [ ] Actual next episode loading (infrastructure complete, needs show ID lookup)

### Player Issues
- [ ] **GStreamer Subtitle Colorspace Bug**
  - [ ] Enable playbin3 (remove `false &&` on line 479)
  - [ ] Remove video-filter setup (lines 497-501, 531-535)
  - [ ] Enable QoS with `enable-qos` property
  - [ ] Use `n-threads: 0` for automatic CPU detection
  - [ ] Add configurable subtitle properties
  - [ ] Implement overlay composition for subtitles

### 📺 Watch Tracking
- [ ] **Manual Controls** (backend support limited)
  - [ ] Add context menu to toggle watched status (Jellyfin has API, Plex missing)
  - [ ] Implement mark all as watched/unwatched
  - [ ] Add bulk selection for multiple items
  - [x] Watch status filter implemented (All/Watched/Unwatched)

## Phase 2: Enhanced Features

### 🏠 Homepage
- [ ] Implement "View All" navigation for sections

### 📊 Advanced Features
- [ ] **Search implementation** (backend support varies)
  - [x] Jellyfin search implemented
  - [ ] Plex search returns todo!()
  - [ ] Local files search returns empty
  - [ ] No UI for search yet
- [ ] **Additional Filters**
  - [ ] Genre filter implementation
  - [ ] Year range filter
  - [ ] Rating filter
  - [ ] Resolution filter
  - [ ] Advanced filter popover/dialog
- [ ] Collections support
- [ ] Playlists
- [ ] Watchlist/Up Next

### 🌐 Additional Backends
- [x] **Jellyfin Integration** (January 2025) ✅ COMPLETE
  - [x] Full authentication flow with auth dialog
  - [x] Backend management in preferences
  - [x] Seamless backend switching
  - [x] Cast/crew info (implemented, depends on server metadata)
  - [x] Watch status retrieval (fully functional)
  - [x] Chapter markers (MediaSegments API, requires server plugin)
  - [x] Playback position sync (resume from last position)
- [ ] Local file support (UI exists, backend not implemented)
- [ ] Metadata provider integration

### 💾 Offline Support
- [ ] Download queue manager
- [ ] Offline playback
- [ ] Smart storage management
- [ ] Network-aware sync

## ✅ Completed Major Milestones

### Phase 1 Core (December 2024 - January 2025)
- ✅ Plex authentication & server discovery
- ✅ Library browsing (movies, TV shows)
- ✅ Media detail pages with premium design
- ✅ Dual player backends (MPV default, GStreamer secondary)
- ✅ Watch status tracking
- ✅ Skip intro/credits with auto-play
- ✅ Image loading optimization
- ✅ Backend-agnostic architecture refactoring
- ✅ Jellyfin integration with full auth dialog
- ✅ Homepage with Continue Watching & Recently Added
- ✅ Filters and sorting infrastructure (title, year, rating, date added, watch status)
- ✅ Library visibility management
- ✅ Multi-backend support with backend switcher
- ✅ Secure credential storage in system keyring

## Player Status
- **MPV**: Default player, fully working
- **GStreamer**: Available but has subtitle color issues

## Recently Completed (January 2025)

### ✅ Jellyfin Sync & UI Fixes (January 2025)
- [x] **Fixed Jellyfin sync parsing errors**
  - [x] Added #[serde(default)] to UserData fields to handle missing PlayedCount
  - [x] Made ImageTags and backdrop_image_tags use default values
  - [x] Jellyfin now successfully syncs 337 movies and 184 shows
- [x] **Library count display in Sources page**
  - [x] Added library_count field to Source model
  - [x] Sources page now shows "Jellyfin • X libraries" instead of just "Online"
  - [x] Library count updates after sync completes
  - [x] Added update_source_library_count() method to AuthManager
- [x] **Fixed Jellyfin API initialization in detail pages**
  - [x] Added ensure_api_initialized() helper method to JellyfinBackend
  - [x] All MediaBackend trait methods now auto-initialize API if needed
  - [x] Fixed keyring service name consistency (standardized on "dev.arsfeld.Reel")
  - [x] Removed legacy credential loading from JellyfinBackend - now relies solely on AuthProvider
  - [x] Movie and show detail pages now work correctly with Jellyfin

### ✅ Offline-First Architecture Improvements (January 2025)
- [x] **Centralized Configuration Management**
  - [x] Config is now loaded once at app startup and shared via Arc<RwLock<Config>>
  - [x] Eliminated multiple disk reads throughout the application
  - [x] All components use shared Config instance from AppState
  - [x] Updated PreferencesWindow, AuthManager, PlayerPage to use shared config
  - [x] MPV player now receives config at initialization instead of loading from disk

- [x] **Offline-First Sources Page**
  - [x] Sources page loads cached provider and source data instantly
  - [x] Background refresh happens asynchronously without blocking UI
  - [x] AuthManager caches Plex sources with 5-minute TTL
  - [x] Falls back to cached data gracefully on network failures
  - [x] Added cached_sources and sources_last_fetched to RuntimeConfig
  - [x] Implemented get_cached_sources() and refresh_sources_background() in AuthManager

- [x] **SourceCoordinator Improvements**
  - [x] initialize_all_sources() now loads cached sources first with offline status
  - [x] Creates UI entries immediately from cache
  - [x] Triggers background refresh to update connection status
  - [x] Provides instant app launch with visible library data

### ✅ SourceCoordinator Implementation & Integration (January 2025)
- [x] **Core Service Created**
  - [x] Centralized backend lifecycle management
  - [x] Integration with AuthManager for credentials
  - [x] Support for Plex, Jellyfin, and LocalFiles backends
  - [x] Source status tracking and connection management
  - [x] Sync coordination across sources
  - [x] Source discovery from Plex accounts
  - [x] Backend removal support (added to BackendManager)
- [x] **Full Integration Completed**
  - [x] Added SourceCoordinator to AppState with late initialization
  - [x] Updated auth dialog to use SourceCoordinator for Plex/Jellyfin
  - [x] Migrated main window backend initialization to use SourceCoordinator
  - [x] Added get_credentials() method to JellyfinBackend
  - [x] Replaced direct backend creation with SourceCoordinator calls throughout

## Previously Completed (January 2025)

### ✅ Sources Page UI Improvements (January 2025)
- [x] **Enhanced Sources Page**
  - [x] Made Sources button sticky in sidebar (not a list item)
  - [x] Added exciting empty state with big "Connect to Plex" and "Connect to Jellyfin" buttons
  - [x] Removed duplicate window controls from sources page
  - [x] Added "Add Source" button to header bar when viewing sources
  - [x] Properly clear header buttons when navigating between pages

### ✅ Auth Dialog Simplification (January 2025)
- [x] **Simplified Auth Dialog**
  - [x] Removed backend selector dropdown from auth dialog
  - [x] Dialog title changes based on backend type ("Connect to Plex" or "Connect to Jellyfin")
  - [x] Fixed template callback signatures (added button parameter)
  - [x] Backend type is now set programmatically when opening dialog

### ✅ Legacy Backend Migration (January 2025)
- [x] **AuthProvider Migration**
  - [x] Added migrate_legacy_backends() method to AuthManager
  - [x] Automatically migrates existing Plex backends from keyring to AuthProvider model
  - [x] Supports migration of Jellyfin backends with stored credentials
  - [x] Preserves existing backend IDs during migration
  - [x] Migration runs automatically when sources page loads

### ✅ Fixed Compilation Errors (January 2025)
- [x] **Auth Dialog Compilation Issues**
  - [x] Fixed adw::Dialog present() method signature (requires Option<&Window>)
  - [x] Added libadwaita::prelude import to sources.rs for PreferencesGroupExt trait
  - [x] Implemented Default trait for NetworkCredentialData enum
  - [x] Fixed borrow checker issue with configured_backends in main_window.rs
  - [x] Resolved all E0599, E0277, E0308, and E0382 compilation errors

### ✅ Backend Management System
- [x] **Multi-Backend Support**
  - [x] Simultaneous Plex and Jellyfin connections
  - [x] Backend switcher in sidebar (subtle dropdown)
  - [x] Only visible when multiple backends configured
  - [x] Seamless switching with library refresh

- [x] **Enhanced Auth Dialog**
  - [x] Backend type selector (Plex/Jellyfin)
  - [x] Dynamic UI based on backend type
  - [x] Jellyfin username/password authentication
  - [x] Programmatic backend type setting

- [x] **Sources & Authentication Architecture**
  - [x] Separate AuthProvider model for account management
  - [x] Support for multiple sources per auth provider (e.g., Plex account → multiple servers)
  - [x] Dedicated Sources page in main window
  - [x] AuthManager service for credential management
  - [x] Preparation for future network sources (SMB, SFTP, WebDAV, NFS)
  - [x] Moved backend management from preferences to Sources page

### ✅ Authentication Refactoring (January 2025)
- [x] **Backend AuthProvider Integration**
  - [x] PlexBackend::from_auth() constructor for AuthProvider+Source
  - [x] JellyfinBackend::from_auth() constructor for AuthProvider+Source
  - [x] Backends can use AuthProvider credentials or fall back to legacy storage
  - [x] AuthManager integrated into AppState
  - [x] Centralized credential management in system keyring
  - [x] Support for re-authentication with stored credentials
  - [x] Fixed NetworkCredentials naming conflict (→ NetworkCredentialData)

## 🎯 Immediate Priority Tasks (Now that Auth is Done)

### 1️⃣ Critical Bug Fixes
- [ ] **Homepage multi-backend conflict** - Sections from different backends randomly replace each other
- [ ] **Homepage horizontal scrolling** - Images don't load when scrolling
- [ ] **GStreamer subtitle colorspace** - Fix color artifacts or disable subtitles
- [ ] **Navigation history** - Extend beyond just player page

### 2️⃣ Complete Partial Implementations
- [ ] **Plex search** - Replace todo!() with actual implementation
- [ ] **Cast/crew data** - Plex still returns empty arrays (Jellyfin now implemented)
- [ ] **Local files backend** - Implement actual file scanning
- [ ] **Watch status for Plex** - Add mark as watched/unwatched API calls

### 3️⃣ UI Polish
- [ ] **Search UI** - Add search bar and results display
- [ ] **Loading states** - Consistent spinners/skeletons
- [ ] **Error handling** - User-friendly error messages
- [ ] **Settings migration** - Move from config.toml to GSettings

## Future Enhancements

### 🚀 Performance Optimizations

- [ ] **FlowBox Model-Based Recycling**
  - [ ] Replace child removal/recreation with ListStore model binding
  - [ ] Reuse MediaCard widgets when filtering/sorting
  - [ ] Implement dirty tracking for differential updates

- [ ] **Smart Prefetching**
  - [ ] Calculate prefetch range based on scroll velocity
  - [ ] Implement predictive loading based on user patterns
  - [ ] Combine immediate and prefetch passes into single operation

### 📋 Next Steps

1. [ ] **Library Enhancements**
   - [ ] Add search UI within library (backend support varies)
   - [x] Implement sorting options (title, year, rating, date added - DONE)
   - [x] Basic watch status filter (All/Watched/Unwatched - DONE)
   - [ ] Add filter by genre, year range, resolution
   - [ ] Create list/grid view toggle
   - [ ] Display cast and crew information UI (Jellyfin provides data, Plex empty)

2. [ ] **Performance Optimizations**
   - [ ] Request smaller thumbnail sizes from Plex API
   - [ ] Implement progressive image loading
   - [ ] Use WebP format if supported
   - [ ] Pre-cache next library's images when idle
   - [ ] Profile actual bottlenecks

## Testing Checklist
- [ ] Test with local Plex server
- [ ] Test with remote Plex server
- [ ] Test with Plex Cloud
- [ ] Test offline scenarios
- [ ] Test large libraries (1000+ items)
- [ ] Test various media formats
- [ ] Test on different screen sizes

## Known Issues

### Critical Issues
- [ ] **Homepage with multiple backends**: When multiple backends are enabled, homepage sections randomly replace each other instead of showing all backends' content
- [ ] **Homepage horizontal scrolling**: Images don't load when scrolling horizontally
- [ ] **GStreamer subtitles**: Color artifacts when subtitles displayed (use MPV instead)
- [x] **Jellyfin sync failure**: ~~Connects successfully but fails to fetch movie/show items during sync~~ FIXED - Added #[serde(default)] to handle missing UserData fields
- [x] **Jellyfin detail pages**: ~~"Jellyfin API not initialized" error when viewing movie/show details~~ FIXED - Added ensure_api_initialized() helper method

### Not Yet Implemented
- [ ] Music/Photo library views
- [ ] Local files backend (stub exists, needs implementation)
- [ ] Cast/crew information display UI (Jellyfin backend provides data, Plex returns empty)
- [ ] Search UI (backend implementations vary: Jellyfin works, Plex todo!(), Local empty)


## Placeholder & Unimplemented Code

### Backend Implementations

#### Jellyfin Backend ✅ COMPLETE (January 2025)
- [x] **Core API Implementation** - Complete
  - [x] Authentication (username/password)
  - [x] Library fetching
  - [x] Movie and show retrieval
  - [x] Episode fetching
  - [x] Stream URL generation
  - [x] Playback progress tracking ✅
  - [x] Search functionality
  - [x] Home sections
- [x] **UI Integration**:
  - [x] Auth dialog with backend type selector
  - [x] Preferences window backend management
  - [x] Backend switcher in main window
  - [x] Automatic backend ID generation
  - [x] Secure credential storage in keyring
- [x] **Recently Implemented** (January 2025):
  - [x] Watch status retrieval (gets watched state, view count, last watched date, playback position)
  - [x] Cast and crew information (fetches People field, separates actors from crew)
  - [x] Find next episode functionality (retrieves next episode in series)
  - [x] Chapter markers support (MediaSegments API for intro/credits detection)
  - [x] Backend loading at startup (fixed - providers now load from config)
- [x] **Remaining Issues FIXED**:
  - [x] **Sync fails to fetch library items** - ~~Connection works, libraries found, but get_movies/get_shows failing~~ FIXED - Serde deserialization issue with missing fields
  - [x] **Detail pages can't access API** - ~~Movie/show detail pages report "Jellyfin API not initialized"~~ FIXED - Added ensure_api_initialized() helper that auto-initializes if needed
  - [ ] Cast/crew may not show if Jellyfin server lacks metadata
  - [ ] MediaSegments requires server plugin for intro detection
  - [ ] No fallback if series_id is missing for next episode

#### Local Files Backend (STUB IMPLEMENTED - January 2025)
- [x] **Basic structure implemented**:
  - [x] Backend creation with `from_auth()` constructor
  - [x] Basic library support (returns single library)
  - [x] Initialize method (checks path exists)
- [ ] **Actual functionality not implemented**:
  - [ ] Library scanning (returns empty)
  - [ ] Movie scanning (returns empty)
  - [ ] TV show scanning (returns empty)
  - [ ] Episode scanning (returns empty)
  - [ ] Progress tracking (no-op)
  - [ ] Watch status management (no-op)
  - [ ] Search functionality (returns empty)

#### Plex Backend
- [ ] **Incomplete Features**:
  - [ ] Cast and crew details (returns empty arrays)
  - [ ] Search functionality (returns `todo!()`)
  - [ ] Proper next episode finding (placeholder implementation)
  - [ ] Last watched timestamp for shows (returns None)

### Core Services

#### Sync Manager
- [x] **Library-specific sync** (`sync_library` implemented - uses backend sync for now)
- [ ] **Actual library-level sync logic** (currently syncs entire backend)

#### Cache Manager
- [ ] **Offline functionality**:
  - [ ] `mark_for_offline` (empty implementation)
  - [ ] `is_available_offline` (always returns false)
- [ ] **Image cache**:
  - [ ] `get` method (always returns None)
  - [ ] `set` method (no-op implementation)
- [ ] **Database filtering**:
  - [ ] `get_libraries` by backend_id (returns empty vec)
  - [ ] `get_movies` by backend_id and library_id (returns empty vec)

### Player Features

#### GStreamer Player
- [ ] **Stream selection for playbin3**:
  - [ ] Proper audio track selection
  - [ ] Proper subtitle track selection
  - [ ] GstStreamCollection API usage

#### Next Episode Auto-play
- [ ] **Actual episode loading** - Infrastructure is complete but needs:
  - [ ] Show ID lookup from current episode
  - [ ] Proper next episode retrieval from backend

### UI Components

#### Filter Manager
- [ ] **Sort implementations**:
  - [ ] Date Added sorting (uses title as fallback)
  - [ ] Date Watched sorting (uses title as fallback)

#### Preferences Window
- [ ] **Local backend registration** - Folder selection UI exists but doesn't create backend

#### Main Window
- [x] **Backend initialization** - Plex and Jellyfin backends are properly initialized
- [x] **Backend switcher** - Subtle dropdown at bottom of sidebar (only visible with 2+ backends)

### Media Metadata

#### Backend-Specific Issues
- [ ] **Plex: Cast and crew information** - Returns empty arrays for:
  - [ ] Movie cast/crew
  - [ ] TV show cast
  - [ ] Person images and roles
- [x] **Jellyfin: Cast and crew** - Fully implemented with person images
- [ ] **Local files: Cast and crew** - Not implemented

## Documentation
- [ ] API documentation
- [ ] User guide
- [ ] Developer setup guide
- [ ] Contributing guidelines
- [ ] Blueprint UI development guide