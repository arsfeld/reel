# Reel Development Tasks

## Phase 1: Plex Authentication & Basic Browsing

### 🔐 Authentication Foundation
- [x] **Implement Plex OAuth authentication flow**
  - [x] Create Plex auth module with PIN-based authentication
  - [x] Implement auth token exchange with Plex.tv API
  - [x] Store auth token to disk (temporary solution)
  - [ ] Store auth token securely in system keyring
  - [ ] Handle token refresh and expiration
  - [x] Create auth status UI indicators

- [x] **Server Discovery & Connection**
  - [x] Implement Plex server discovery via API
  - [x] Parallel connection testing for best server
  - [x] Test server connectivity with latency measurement
  - [x] Store server URL and connection details
  - [x] Handle connection errors gracefully
  - [ ] Create server selection dialog (for multiple servers)

### 📚 Library Browsing
- [x] **Fetch and Display Libraries**
  - [x] Implement `/library/sections` API call
  - [x] Parse library metadata (movies, shows, music)
  - [x] Update home page with actual library counts
  - [x] Create library type icons and badges
  - [x] Cache library data locally

- [x] **Movies Library Implementation**
  - [x] Fetch movies from library endpoint
  - [x] Parse movie metadata (title, year, rating, poster)
  - [x] Create movie grid view component
  - [x] Implement lazy loading for large libraries
  - [ ] Add movie detail view

- [x] **TV Shows Library Implementation**
  - [x] Fetch shows from library endpoint
  - [x] Parse show/season/episode structure
  - [x] Create show grid view component
  - [ ] Implement season/episode navigation
  - [ ] Add episode list view

### 🖼️ Media & Metadata
- [x] **Image Loading & Caching** (Partially Working - Performance Issues)
  - [x] Implement poster/thumb URL construction
  - [x] Create async image download service with throttling
  - [x] Implement disk-based image cache (~/.cache/reel/images/)
  - [x] Add placeholder images for unloaded content
  - [x] Handle image loading errors with fallback
  - [x] Viewport-based lazy loading for performance
  - [x] Concurrent download limiting (increased to 50 simultaneous)
  - [x] Memory cache for instant access
  - [x] Pre-fetch 2 screens ahead for smoother scrolling
  - [x] Reduced debounce delay to 50ms
  - [ ] **Performance still needs improvement** - images load too slowly

- [ ] **Metadata Display**
  - [ ] Create media info cards
  - [ ] Display ratings, duration, genre
  - [ ] Show cast and crew information
  - [ ] Implement synopsis/overview display
  - [ ] Add media badges (4K, HDR, etc.)

### 🔄 Sync & Cache System
- [x] **SQLite Database Setup**
  - [x] Create database schema migrations
  - [x] Implement cache manager
  - [x] Create CRUD operations for media
  - [ ] Add indexes for performance
  - [ ] Implement cache expiration logic

- [x] **Background Sync Service**
  - [x] Create sync manager structure
  - [x] Implement incremental sync
  - [x] Add sync status indicators
  - [ ] Handle sync conflicts
  - [ ] Create sync scheduling system

### 🎨 UI Improvements
- [x] **Blueprint UI Setup**
  - [x] Migrate to GNOME Blueprint for UI definitions
  - [x] Create reusable Blueprint components
  - [x] Set up resource compilation in build.rs
  
- [ ] **Navigation & Routing**
  - [ ] Fix navigation between pages
  - [ ] Implement back button handling
  - [ ] Add breadcrumb navigation
  - [ ] Create loading states
  - [ ] Add error state displays

- [x] **Server Connection UI**
  - [x] Create connection dialog with Blueprint
  - [x] Add server status indicators
  - [x] Show connected user and server status
  - [x] Display server name with connection type (Local/Remote/Relay)
  - [x] Add connection type icons (wired/wireless/cellular)
  - [x] Hide welcome screen when connected
  - [ ] Implement connection retry UI
  - [x] Show sync progress
  - [ ] Add offline mode banner

### 🎬 Basic Playback
- [x] **Stream URL Generation**
  - [x] Construct direct play URLs
  - [ ] Handle transcoding decisions
  - [ ] Implement quality selection
  - [ ] Add subtitle/audio track selection
  - [ ] Create playback decision engine

- [x] **Player Integration** (Completed!)
  - [x] Initialize GStreamer player
  - [x] Load and play video streams
  - [x] Implement basic controls (play/pause/seek)
  - [x] Add immersive playback mode with auto-hiding controls
  - [x] Handle playback errors with user-friendly dialogs
  - [x] Fix seek loop issue in progress bar
  - [x] Implement hover-based UI controls (header and player controls)
  - [x] Add window resizing to match video aspect ratio
  - [x] Create overlay header bar that doesn't affect video layout
  - [ ] Add fullscreen support (partial - button exists but needs implementation)

## Phase 2: Enhanced Features (Future)

### 📊 Advanced Features
- [ ] Continue Watching functionality
- [ ] Recently Added section
- [ ] Search implementation
- [ ] Filters and sorting
- [ ] Collections support
- [ ] Playlists
- [ ] Watchlist/Up Next

### 🌐 Additional Backends
- [ ] Jellyfin integration
- [ ] Local file support
- [ ] Metadata provider integration

### 💾 Offline Support
- [ ] Download queue manager
- [ ] Offline playback
- [ ] Smart storage management
- [ ] Network-aware sync

## ✅ COMPLETED - Architecture Refactoring

### **Backend-Agnostic Architecture** (COMPLETED)
Successfully refactored the entire codebase to remove all backend-specific hard-coding. The UI layer is now completely agnostic and works with any backend type.

**Completed Fixes:**
- [x] Removed all "plex" string literals from UI code
- [x] Removed hard-coded movie/TV show assumptions from UI
- [x] Made cache manager backend-agnostic (uses dynamic backend IDs)
- [x] Store libraries in AppState with backend ID association
- [x] Made sync manager work with any backend generically
- [x] Updated all UI components to work with generic library data
- [x] Removed hard-coded library type filtering in sync
- [x] Store and load last active backend ID persistently
- [x] Support multiple backends of same type with unique IDs

**Completed Refactoring Tasks:**
1. [x] **AppState Refactoring**
   - [x] Added `libraries: HashMap<String, Vec<Library>>` to AppState
   - [x] Added `library_items: HashMap<String, Vec<MediaItem>>` for cached items
   - [x] Added methods to get libraries for active backend
   - [x] Added methods to get items for a specific library
   - [x] Added method to get active backend ID

2. [x] **Cache Manager Refactoring**
   - [x] Uses backend IDs dynamically instead of hard-coded "plex"
   - [x] Created generic cache keys: `{backend_id}:{type}:{id}`
   - [x] Supports multiple backends in same cache

3. [x] **Sync Manager Refactoring**
   - [x] Removed all hard-coded "plex" references
   - [x] Uses active backend from AppState
   - [x] Supports syncing any library type (Movies, Shows, Music, Photos)
   - [x] Generic `sync_library_items` method for all media types

4. [x] **UI Components Refactoring**
   - [x] Library list is completely generic
   - [x] Displays ALL library types from backend
   - [x] Uses library type from backend data, not hard-coded
   - [x] Removed PlexBackend downcasting - uses generic backend info

5. [x] **Backend Info System**
   - [x] Added `BackendInfo` struct with server details
   - [x] Added `get_backend_info()` to MediaBackend trait
   - [x] UI uses generic backend info instead of type-specific methods

6. [x] **Persistent Backend Management**
   - [x] Added RuntimeConfig to store last active backend
   - [x] Automatically loads last used backend on startup
   - [x] Generates unique backend IDs (plex, plex_1, plex_2, etc.)

7. [x] **Instant Cache Loading**
   - [x] Cache loads immediately on app startup
   - [x] Welcome UI hidden as soon as cached data is available
   - [x] Authentication happens in background without blocking UI

### **Architecture Principles to Enforce:**
1. **Backend Agnostic UI**: The UI layer should NEVER import or reference specific backend implementations
2. **Generic Data Flow**: UI → AppState → BackendManager → Active Backend
3. **Dynamic Backend Selection**: Support multiple backends simultaneously with runtime switching
4. **Universal Caching**: Cache should work identically for all backends
5. **Type Safety**: Use the MediaBackend trait exclusively in UI/services

### **Example of Correct Architecture:**
```
// BAD - UI knows about Plex
window.sync_and_update_libraries("plex", backend)

// GOOD - UI uses active backend
let backend_id = state.backend_manager.get_active_id();
window.sync_and_update_libraries(backend_id, backend)
```

## Current Priority Tasks

### ✅ Completed
1. [x] **Blueprint UI Implementation**
   - [x] Set up GNOME Blueprint for UI development
   - [x] Create Blueprint templates for main window
   - [x] Create auth dialog with Blueprint
   - [x] Fix Blueprint syntax errors (semicolons, signal handlers)
   - [x] Fix UI layout issues (vertical expansion, selectable PIN)
   - [x] Successfully compile and run with Blueprint UI

2. [x] **Plex Authentication**
   - [x] Implement PIN-based authentication flow
   - [x] Generate 4-character PIN codes
   - [x] Poll for auth token
   - [x] Save token to disk
   - [x] Update UI to show auth status
   - [x] Auto-load saved credentials on startup

3. [x] **Server Discovery**
   - [x] Implement Plex server discovery API
   - [x] Parse server responses correctly
   - [x] Test all connections in parallel
   - [x] Select fastest responding server
   - [x] Handle connection failures gracefully
   - [x] Store server connection info (name, local/relay status)
   - [x] Display server details in UI status bar

4. [x] **Library Sync & Display**
   - [x] Implement Plex API for fetching libraries
   - [x] Create sync manager for background updates
   - [x] Cache libraries and media in SQLite
   - [x] Update UI with real library counts
   - [x] Show sync progress spinner
   - [x] Auto-sync on authentication

### ✅ Recently Completed
1. [x] **Library Navigation**
   - [x] Navigate to library views when clicked
   - [x] Create media grid view component (generic for all types)
   - [x] Implement movie and TV show views
   - [x] Fix AdwApplicationWindow navigation issues
   - [x] Create MediaCard widget for grid display
   - [x] Add back navigation from library view

2. [x] **Backend Management System**
   - [x] Create preferences window with AdwPreferencesWindow
   - [x] Implement backend list view with add/remove functionality
   - [x] Add backend removal with confirmation dialog
   - [x] Fix backend ID generation to reuse existing IDs
   - [x] Add clear cache functionality for removed backends
   - [x] Create add backend flow with type selection
   - [x] Integrate with existing auth dialog for Plex

### 📋 Next Steps

1. [x] **Image Loading & Display** (COMPLETED)
   - [x] Implement poster/thumb URL construction for Plex
   - [x] Create async image download service
   - [x] Add disk-based image cache
   - [x] Load and display poster images in MediaCard
   - [x] Add loading spinner while images load
   - [x] Handle image loading errors gracefully
   - [x] Viewport-based lazy loading to prevent UI freezing
   - [x] Concurrent download throttling

2. [ ] **Media Detail Views**
   - [ ] Create media detail page layout
   - [ ] Implement movie detail view with full metadata
   - [ ] Add TV show detail view with seasons/episodes
   - [ ] Display cast, crew, and synopsis
   - [ ] Add play button to start playback

3. [ ] **Library Enhancements**
   - [x] Implement lazy loading for large libraries
   - [ ] Add search within library
   - [ ] Implement sorting options (title, year, rating)
   - [ ] Add filter by genre, year, etc.
   - [ ] Create list/grid view toggle

4. [ ] **Performance Optimizations** (High Priority)
   - [ ] Request smaller thumbnail sizes from Plex API (150x225 instead of full posters)
   - [ ] Implement progressive image loading (low-res placeholder → full image)
   - [ ] Use WebP format if Plex supports it for smaller file sizes
   - [ ] Pre-cache next library's images when idle
   - [ ] Consider using native GTK async image loading APIs
   - [ ] Investigate GdkPixbuf loader performance
   - [ ] Profile actual bottlenecks (network vs decoding vs rendering)

5. [x] **Playback Foundation** (COMPLETED!)
   - [x] Initialize GStreamer player component
   - [x] Generate stream URLs from Plex
   - [x] Implement basic video playback
   - [x] Add playback controls overlay
   - [ ] Track playback progress (partially done - position tracking works, needs to save to server)

## Testing Checklist
- [ ] Test with local Plex server
- [ ] Test with remote Plex server
- [ ] Test with Plex Cloud
- [ ] Test offline scenarios
- [ ] Test large libraries (1000+ items)
- [ ] Test various media formats
- [ ] Test on different screen sizes

## Known Issues & Troubleshooting

### Current Issues
- [ ] **Music/Photo Libraries**: Views not yet implemented
- [ ] **Jellyfin Backend**: Integration pending implementation
- [ ] **Local Files Backend**: File browser not yet implemented
- [ ] **Image Loading Performance**: Still slow despite optimizations - needs further work
  - Loading takes 100-500ms per image even with parallel downloads
  - UI still feels sluggish when scrolling through large libraries
  - May need to implement thumbnail generation or smaller image variants
  - Consider pre-caching images in background after library load
- [ ] **Minor Player UI Issues**: 
  - Occasional duplicate back button in player overlay (mostly fixed)
  - Fullscreen button exists but not fully implemented

### Resolved Issues
- ✅ **GTK Template Loading Error**: Fixed by correcting Blueprint syntax
- ✅ **Plex PIN Authentication**: Fixed by removing "strong" parameter
- ✅ **Server Discovery Parsing**: Fixed by handling array response format
- ✅ **Connection Selection**: Implemented parallel testing for best server
- ✅ **UI Server Status Display**: Fixed RwLock deadlock and added server info display with connection type icons
- ✅ **Backend-Specific Hard-coding**: Completely refactored to backend-agnostic architecture
- ✅ **Slow Startup**: Cache now loads instantly before authentication
- ✅ **Backend ID Management**: Fixed to reuse existing IDs instead of creating new ones
- ✅ **AdwApplicationWindow Navigation**: Fixed set_child error by using set_content
- ✅ **RefCell Borrow Panic**: Fixed multiple borrow issue in library navigation
- ✅ **Widget Parent Issues**: Resolved GTK widget parent conflicts when switching views
- ✅ **Poster Images Not Loading**: Implemented async image loader with disk/memory caching
- ✅ **UI Freezing with Large Libraries**: Added viewport-based lazy loading with throttling
- ✅ **Source ID Removal Panic**: Fixed with counter-based debouncing approach
- ✅ **GStreamer Playback Issues**: Fixed missing typefind element, playbin creation, and video sink setup
- ✅ **Player Navigation**: Fixed page not changing when clicking movies
- ✅ **Seek Loop Bug**: Fixed infinite seeking caused by progress bar updates
- ✅ **Immersive Player Mode**: Implemented auto-hiding controls with overlay header bar
- ✅ **Window Aspect Ratio**: Window now resizes to match video aspect ratio
- ✅ **Player Controls Layout**: Header bar now overlays video instead of pushing it down

## Documentation
- [ ] API documentation
- [ ] User guide
- [ ] Developer setup guide
- [ ] Contributing guidelines
- [ ] Blueprint UI development guide