# Reel Development Tasks

## 🔥 Critical Issues

### Bugs to Fix
- [ ] **GStreamer subtitle colorspace** - Fix color artifacts or disable subtitles
- [ ] **Navigation history** - Only tracks player page, needs full implementation

### Incomplete Features  
- [ ] **Search**
  - [ ] Plex backend returns `todo!()`
  - [ ] Local files returns empty
  - [ ] No UI implementation yet
  - [x] ~~Jellyfin backend works~~
- [ ] **Cast/Crew** - Plex returns empty arrays (Jellyfin works)
- [ ] **Local Files Backend** - ~10% implemented, mostly stubs
- [ ] **Watch Status** - Plex missing mark as watched/unwatched API

## 📋 Core Features

### Authentication & Security
- [ ] Handle Plex token refresh and expiration
- [ ] Add network source authentication (SMB, SFTP, WebDAV, NFS)

### Media & Metadata
- [ ] Create media info cards (ratings, duration, genre)
- [ ] Display cast/crew UI (backend data exists for Jellyfin)
- [ ] Add media badges (4K, HDR, Dolby Vision, etc.)
- [ ] Implement synopsis/overview display

### Navigation & UI
- [ ] Add breadcrumb navigation
- [ ] Create consistent loading states/skeletons
- [ ] Add error state displays
- [ ] Implement connection retry UI
- [ ] Add offline mode banner
- [ ] Implement "View All" for homepage sections

### Settings Management
- [ ] Migrate to GSettings for GNOME compliance
- [ ] Create GSettings schema (.gschema.xml)
- [ ] Auto-reload settings on change
- [ ] Migrate from config.toml

### Playback
- [ ] Handle transcoding decisions
- [ ] Implement quality selection UI
- [ ] Move skip intro button to bottom-right
- [ ] Fix next episode loading (needs show ID lookup)
- [ ] Clean up verbose marker debug logging

### Watch Tracking
- [ ] Add context menu for watched status toggle
- [ ] Implement mark all as watched/unwatched
- [ ] Add bulk selection

### Advanced Filters
- [ ] Genre filter
- [ ] Year range filter  
- [ ] Rating filter
- [ ] Resolution filter
- [ ] Advanced filter popover

## 🚀 Future Enhancements

### Performance
- [ ] FlowBox model-based recycling (reuse widgets)
- [ ] Smart prefetching based on scroll velocity
- [ ] Request smaller thumbnails from APIs
- [ ] Progressive image loading
- [ ] WebP format support
- [ ] Pre-cache next library when idle

### Additional Features
- [ ] Collections support
- [ ] Playlists
- [ ] Watchlist/Up Next
- [ ] Download queue manager
- [ ] Offline playback
- [ ] Smart storage management
- [ ] Music/Photo libraries

## 📊 Progress Summary

### ✅ Completed (v0.4.0)
- Reactive architecture with SeaORM
- Event-driven ViewModels (75% migrated)
- Multi-backend support (Plex, Jellyfin)
- Dual player backends (MPV, GStreamer)
- Offline-first with SQLite caching
- Secure credential storage
- Basic filtering and sorting
- Homepage with Continue Watching
- Backend switcher UI
- Sources & Accounts page

### 🚧 In Progress
- Architecture migration (25% remaining)
- Repository event integration
- 4 pages need ViewModel integration
- Cast/crew display (Jellyfin only)
- Search implementation

### 📝 Testing Needed
- Local Plex server
- Remote Plex server
- Plex Cloud
- Offline scenarios
- Large libraries (1000+ items)
- Various media formats
- Different screen sizes

## 🐛 Known Limitations

### Player
- **MPV**: Default, fully working
- **GStreamer**: Has subtitle color issues

### Backends
- **Plex**: 90% complete (missing cast/crew, search)
- **Jellyfin**: 90% complete (needs server metadata for cast/crew)
- **Local Files**: 10% complete (stubs only)

### Not Implemented
- Music/Photo library views
- Metadata provider integration
- Network-aware sync
- Chapter markers (requires Jellyfin plugin)