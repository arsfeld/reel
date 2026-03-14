# Reel - Product Specification

> A beautiful, metadata-rich media player and library manager for Linux.
> The Infuse experience, native on the Linux desktop.

## Vision

Reel fills a clear gap on Linux: there is no application that combines a beautiful browsing interface, automatic metadata fetching, powerful direct playback, and serverless or server-connected library management in a single, simple app. VLC and mpv are excellent players but have no library. Kodi has a library but requires significant setup and feels like an HTPC app, not a desktop app. Plex and Jellyfin require running a separate server. Reel brings these together with zero-configuration elegance.

## Target Users

- Users with media collections on NAS devices or local drives
- Plex users who want a native, performant Linux client (replacing Electron-based Plex Desktop)
- Users migrating from macOS/iOS who miss Infuse
- Linux desktop users who want more than a file player

## Core Principles

1. **It just works** - Connect a source, media appears with metadata and artwork within seconds
2. **Play everything** - Every format, every codec, hardware accelerated, no transcoding needed
3. **Beautiful by default** - Rich metadata, poster art, backdrops, cast info - no configuration required
4. **Desktop native** - Libadwaita, MPRIS, media keys, Wayland-first, proper dark mode
5. **Open architecture** - Plex first, but the door is open for Jellyfin, Emby, and standalone operation

---

## Feature Specification

### F1: Media Playback

#### F1.1: Format Support
Reel plays everything mpv/FFmpeg can decode, which covers virtually all media formats:

**Containers:** MKV, MP4, AVI, MOV, M4V, WEBM, FLV, TS, M2TS, MPEG, OGM, WMV, 3GP, VOB, ISO (DVD/Blu-ray structure)

**Video Codecs:** H.264 (AVC), H.265 (HEVC), AV1, VP9, VP8, MPEG-2, MPEG-4, Theora, WMV, VC-1

**Audio Codecs:** AAC, AC3 (Dolby Digital), E-AC3 (Dolby Digital Plus), DTS, DTS-HD, TrueHD, FLAC, Opus, Vorbis, MP3, WMA, PCM/LPCM

**HDR:** HDR10, HDR10+, HLG. Dolby Vision passthrough where hardware supports it.

#### F1.2: Hardware Acceleration
- VA-API (Intel, AMD) - primary Linux hardware decode path
- NVDEC via nvidia-vaapi-driver (NVIDIA)
- Vulkan Video where available (emerging standard, preferred by mpv 0.41+)
- Automatic fallback to software decoding when hardware unavailable
- Zero user configuration required - Reel detects and uses the best available backend

#### F1.3: Playback Controls
- Play / Pause
- Seek with timeline scrubbing (click or drag on progress bar)
- Skip forward / backward (configurable interval: 5s, 10s, 30s)
- Playback speed adjustment (0.25x to 4.0x)
- Volume control with boost capability (up to 200%)
- Chapter navigation (when chapters present in file)
- Keyboard shortcuts for all controls
- MPRIS integration (media keys, desktop widget control)

#### F1.4: Audio Track Management
- Display all audio tracks with language, codec, and channel info
- Switch audio tracks during playback without interruption
- Remember preferred audio language across sessions
- Audio-video sync offset adjustment

#### F1.5: Subtitle Support

**Formats:** SRT, SSA/ASS, VTT, SUB/IDX (VobSub), PGS (Blu-ray), DVB

**Features:**
- Embedded subtitle track detection and selection
- External subtitle file auto-detection (matching filename in same directory)
- Subtitle language preference (remembered across sessions)
- On-demand subtitle download from OpenSubtitles (hash-based matching first, title fallback)
- Forced subtitle track detection

**Customization:**
- Font family, size, weight
- Text color and outline/shadow
- Position adjustment (vertical offset)
- Background opacity

#### F1.6: Player UI
- Overlay controls that auto-hide during playback
- Show on mouse movement, hide after timeout
- Full metadata display accessible during playback (title, year, codec info)
- Smooth progress bar with position and remaining time
- Volume indicator
- Audio/subtitle track selector popover
- Fullscreen toggle (F11 or double-click)
- Keyboard-driven (spacebar pause, arrows seek, etc.)

#### F1.7: Screensaver/Idle Inhibition
- Automatically inhibit screensaver during video playback
- Release inhibition on pause or stop
- Uses org.freedesktop.ScreenSaver D-Bus interface (Wayland compatible)

---

### F2: Library Management

#### F2.1: Media Sources
Users add media sources through a simple connection dialog:

**Server Sources (Priority 1):**
- **Plex** - Full library integration via Plex API
  - Browse server libraries (Movies, TV Shows)
  - Use server metadata, artwork, and collections
  - Sync watched status bidirectionally
  - Direct play (no server transcoding)
  - Support for Plex's "Continue Watching" and "Recently Added"
  - Auto Skip Intros (if server provides intro markers)

**Server Sources (Future - Architecture Ready):**
- Jellyfin - via Jellyfin API
- Emby - via Emby API

**Direct Sources (Future - Architecture Ready):**
- SMB/CIFS network shares
- NFS mounts
- Local directories
- SFTP/FTP

#### F2.2: Library Browser
The main library view presents media as a grid of poster cards:

**Views:**
- **Poster Grid** - Default view, poster artwork with title below
- **List View** - Compact list with poster thumbnail, title, year, rating, runtime
- Configurable grid density (small, medium, large posters)
- Smooth scrolling with virtualized rendering (only visible items rendered)

**Organization:**
- Movies and TV Shows as top-level categories
- Collections grouping (sequels, franchises - from Plex or TMDb data)
- Genre browsing
- Recently Added
- Continue Watching (resume in-progress items)
- Unwatched filter

**Sorting:**
- Title (A-Z, Z-A)
- Year (newest first, oldest first)
- Date Added
- Rating
- Runtime

**Filtering:**
- By genre
- By year/decade
- By rating threshold
- Watched / Unwatched
- Combinable filters

#### F2.3: Search
- Instant search as you type
- Search across titles, cast, crew, collections
- Results grouped by type (Movies, TV Shows, People)
- Keyboard shortcut to activate search (Ctrl+F or /)

#### F2.4: Metadata
When connected to Plex, Reel uses the server's metadata. For standalone sources (future), Reel fetches from:

**TMDb (The Movie Database) - Primary Provider:**
- Movie/TV show title, original title, year
- Plot synopsis / overview
- Genres, runtime, content rating
- Poster artwork and backdrop images
- Cast and crew with photos
- Ratings (TMDb score)
- Collection/franchise grouping
- Matching by: parsed filename (title + year), or embedded metadata

**Filename Parsing:**
- Parse title, year, season, episode from common naming conventions
- Support Plex-style naming: `Movie Name (2024).mkv`
- Support scene-style naming: `Movie.Name.2024.1080p.BluRay.x264-GROUP.mkv`
- TV shows: `Show Name - S01E01 - Episode Title.mkv` or `Show.Name.S01E01.mkv`
- Date-based shows: `Show Name - 2024-01-15 - Episode Title.mkv`

#### F2.5: TV Show Organization
- Hierarchical browsing: Show > Season > Episode
- Season poster art
- Episode list with thumbnails, titles, air dates, descriptions
- Per-episode watched/progress tracking
- Auto-play next episode
- "Up Next" concept showing next unwatched episode per show

#### F2.6: Watch State
- Playback position saved per item (resume from where you left off)
- Watched / unwatched status with visual indicators
- Progress bar on poster cards showing partial watch progress
- Sync watch state with Plex server
- Clear watch state / mark as watched/unwatched

---

### F3: Detail Pages

#### F3.1: Movie Detail Page
- Hero backdrop image
- Poster artwork
- Title, year, runtime, content rating
- TMDb rating badge
- Genre tags
- Plot synopsis
- Cast list with photos (scrollable horizontal row)
- Crew (Director, Writer)
- Technical info (resolution, codec, audio channels, file size)
- Collection membership (link to other films in collection)
- Play button (prominent)
- Mark watched/unwatched toggle

#### F3.2: TV Show Detail Page
- Show backdrop and poster
- Show title, year range, status (ongoing/ended)
- Plot synopsis
- Cast list
- Season selector (tabs or dropdown)
- Episode list for selected season
- Per-episode: thumbnail, title, air date, description, watched indicator
- "Play Next" button starting from next unwatched episode

#### F3.3: Collection Page
- Collection backdrop/poster
- Collection description
- Grid of movies in the collection, ordered chronologically
- Total runtime, movie count

#### F3.4: Person Page (Future)
- Photo, name, biography
- Filmography (movies/shows they appear in, within the user's library)

---

### F4: User Interface

#### F4.1: Application Shell
- **Libadwaita** throughout - follows GNOME Human Interface Guidelines
- `AdwNavigationSplitView` for sidebar + content layout
- Sidebar: navigation (Movies, TV Shows, collections, sources)
- Content area: library grid, detail pages, or player
- Adaptive layout: sidebar collapses on narrow windows via `AdwBreakpoint`

#### F4.2: Theming
- Follows system dark/light preference via `adw::StyleManager`
- Custom CSS for media-specific elements (poster cards, player overlay, backdrop headers)
- Dark mode optimized (media apps are better in dark)

#### F4.3: Navigation
- Sidebar-driven primary navigation
- Back navigation via `AdwNavigationView` for drill-down (Library > Show > Season > Episode)
- Breadcrumb-style header showing current location
- Keyboard navigation support throughout

#### F4.4: Responsive Layout
- Full-width: sidebar visible + wide content grid
- Medium: sidebar collapsible, fewer grid columns
- Narrow: sidebar hidden, single-column layout
- Breakpoints managed via `AdwBreakpoint`

#### F4.5: Loading & Empty States
- Skeleton loading placeholders while metadata loads
- `AdwStatusPage` for empty states ("No movies found", "Add a media source to get started")
- Toast notifications via `AdwToastOverlay` for transient feedback

---

### F5: Integrations

#### F5.1: Plex Integration (Priority)
- Connect via server URL + auth token (or Plex account login)
- Discover local Plex servers via GDNSd/mDNS
- Browse all Plex libraries (Movies, TV Shows)
- Use Plex metadata, artwork, collections
- Direct play - Reel handles decoding, no server transcoding
- Bidirectional watch state sync
- Support Plex's "On Deck" / "Continue Watching"
- Respect Plex library sections and custom ordering

#### F5.2: Trakt.tv Integration
- Scrobbling: automatically log what you watch
- Watch history sync
- Rating sync
- 2-way sync (pull Trakt history, push Reel watches)
- Account linking through OAuth flow

#### F5.3: Desktop Integration
- **MPRIS2**: Full media player remote interface
  - Play/Pause/Stop/Next/Previous control
  - Track metadata (title, artist/show, artwork)
  - Position reporting and seeking
  - Integration with desktop widgets, media keys, Bluetooth controls
- **Notifications**: Now playing, download complete (future)
- **Freedesktop thumbnailer**: Provide video thumbnails to file managers

#### F5.4: OpenSubtitles Integration
- Search subtitles by file hash (most accurate) or title
- Language preference configuration
- One-click download and apply
- Rate limiting aware (respect API limits)

---

### F6: Settings

#### F6.1: General
- Default media source selection
- Language preference (metadata language, subtitle language, audio language)
- Cache management (clear metadata/artwork cache)

#### F6.2: Playback
- Default skip interval (5s, 10s, 15s, 30s)
- Default playback speed
- Hardware acceleration toggle (auto, force software)
- Audio output device selection
- Volume boost limit
- Remember playback position (on/off)
- Auto-play next episode (on/off)

#### F6.3: Subtitles
- Default subtitle language
- Font customization (family, size, color, outline)
- OpenSubtitles account connection
- Auto-download subtitles (on/off)

#### F6.4: Library
- Watched indicators (on/off)
- Show collections (on/off)
- Grid density preference
- Sort default

#### F6.5: Connections
- Manage Plex server connections
- Manage Trakt account
- Network source management (future)

---

### F7: Data & Storage

#### F7.1: Local Database
- SQLite database for local state
- Stores: media index, watch history, playback positions, user preferences, cached metadata
- Located in `$XDG_DATA_HOME/reel/` (typically `~/.local/share/reel/`)

#### F7.2: Artwork Cache
- Poster and backdrop images cached locally
- Located in `$XDG_CACHE_HOME/reel/` (typically `~/.cache/reel/`)
- Configurable cache size limit
- Lazy loading with placeholder images

#### F7.3: Configuration
- Settings stored in `$XDG_CONFIG_HOME/reel/` (typically `~/.config/reel/`)
- TOML configuration file

---

## Out of Scope (v1)

These features are intentionally deferred:

- Music playback and music library management
- Photo library
- Cloud storage sources (Google Drive, Dropbox, etc.)
- DVD/Blu-ray disc playback
- Video transcoding or conversion
- Media server functionality (Reel is a client, not a server)
- Mobile/tablet versions
- Remote control / companion app
- AI upscaling
- 3D video
- Multi-user profiles (single user for v1)
- Live TV / DVR

---

## Competitive Positioning

| Feature | Reel | VLC | Celluloid | Kodi | Plex Desktop |
|---------|------|-----|-----------|------|-------------|
| Beautiful library UI | Yes | No | No | Yes (heavy) | Yes |
| Metadata auto-fetch | Yes | No | No | Yes | Yes (server) |
| Serverless operation | Future | Yes | Yes | Yes | No |
| Plex integration | Yes | No | No | Plugin | Native |
| Play all formats | Yes | Yes | Yes | Yes | Limited |
| Hardware acceleration | Auto | Manual | Auto | Config | N/A |
| Native Linux desktop | Libadwaita | GTK | GTK | Custom | Electron |
| MPRIS support | Yes | Yes | Yes | No | No |
| Wayland native | Yes | Yes | Yes | Partial | Partial |
| Resource usage | Low | Medium | Low | High | Very High |
| Setup complexity | None | None | None | High | Medium |
