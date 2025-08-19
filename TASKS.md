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
  - [ ] Create movie grid view component
  - [ ] Implement lazy loading for large libraries
  - [ ] Add movie detail view

- [x] **TV Shows Library Implementation**
  - [x] Fetch shows from library endpoint
  - [x] Parse show/season/episode structure
  - [ ] Create show grid view component
  - [ ] Implement season/episode navigation
  - [ ] Add episode list view

### 🖼️ Media & Metadata
- [ ] **Image Loading & Caching**
  - [ ] Implement poster/thumb URL construction
  - [ ] Create image download service
  - [ ] Implement disk-based image cache
  - [ ] Add placeholder images
  - [ ] Handle image loading errors

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
- [ ] **Stream URL Generation**
  - [ ] Construct direct play URLs
  - [ ] Handle transcoding decisions
  - [ ] Implement quality selection
  - [ ] Add subtitle/audio track selection
  - [ ] Create playback decision engine

- [ ] **Player Integration**
  - [ ] Initialize GStreamer player
  - [ ] Load and play video streams
  - [ ] Implement basic controls (play/pause/seek)
  - [ ] Add fullscreen support
  - [ ] Handle playback errors

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

### 🔧 In Progress
1. [ ] **Library Navigation**
   - [ ] Navigate to library views when clicked
   - [ ] Create media grid view component (generic for all types)
   - [ ] Implement type-specific views (movies, shows, music, photos)

### 📋 Next Steps
1. [ ] **Generic Media Library View**
   - [ ] Create unified grid view for all media types
   - [ ] Implement lazy loading for large libraries
   - [ ] Add media detail view with type-specific layouts
   - [ ] Show metadata based on media type

2. [ ] **Type-Specific Features**
   - [ ] Movie view: rating, year, duration, cast
   - [ ] TV show view: seasons, episodes, air dates
   - [ ] Music view: albums, artists, tracks
   - [ ] Photo view: gallery, date organization

3. [ ] **Image Loading**
   - [ ] Implement poster/thumb URL construction
   - [ ] Create image download service
   - [ ] Add disk-based image cache
   - [ ] Display placeholder images while loading

## Testing Checklist
- [ ] Test with local Plex server
- [ ] Test with remote Plex server
- [ ] Test with Plex Cloud
- [ ] Test offline scenarios
- [ ] Test large libraries (1000+ items)
- [ ] Test various media formats
- [ ] Test on different screen sizes

## Known Issues & Troubleshooting

### Resolved Issues
- ✅ **GTK Template Loading Error**: Fixed by correcting Blueprint syntax
- ✅ **Plex PIN Authentication**: Fixed by removing "strong" parameter
- ✅ **Server Discovery Parsing**: Fixed by handling array response format
- ✅ **Connection Selection**: Implemented parallel testing for best server
- ✅ **UI Server Status Display**: Fixed RwLock deadlock and added server info display with connection type icons
- ✅ **Backend-Specific Hard-coding**: Completely refactored to backend-agnostic architecture
- ✅ **Slow Startup**: Cache now loads instantly before authentication
- ✅ **Backend ID Management**: Dynamic IDs with persistent storage

## Documentation
- [ ] API documentation
- [ ] User guide
- [ ] Developer setup guide
- [ ] Contributing guidelines
- [ ] Blueprint UI development guide