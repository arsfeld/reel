# Reel - Product Vision Document

## Executive Summary

Reel is a modern, native media player application for the GNOME desktop environment, designed to provide a premium media consumption experience similar to Infuse on Apple platforms. Built with Rust and GTK4, it offers a beautiful, performant interface for browsing and playing media from various sources including Plex, Jellyfin, and local files. With its innovative offline-first architecture, users can instantly access their entire media library whether online or offline, with seamless background synchronization keeping content fresh.

## Product Vision

### Mission Statement
To create the definitive media player for the Linux desktop that seamlessly integrates with the GNOME ecosystem while providing a best-in-class user experience for media consumption, with unparalleled offline capabilities ensuring your media is always accessible.

### Target Audience
- Linux desktop users running GNOME
- Media enthusiasts with personal media servers (Plex/Jellyfin)
- Users seeking a premium media experience on Linux
- Power users who want local file playback with server-like organization
- Mobile professionals who need offline access to their media
- Users with unreliable internet connections
- Privacy-conscious users who prefer local-first applications

## Core Features

### 1. Media Playback
- **Integrated Video Player**: Native, high-performance video playback embedded in the UI
- **Format Support**: Wide codec support through GStreamer
- **Subtitles**: Multiple subtitle tracks, external subtitle files, subtitle styling
- **Audio Tracks**: Multiple audio track selection
- **Playback Controls**: Standard controls, playback speed, skip intro/credits
- **Hardware Acceleration**: VAAPI, VDPAU support for efficient playback
- **4K/HDR Support**: Full support for high-resolution and HDR content

### 2. Media Library
- **Movies**: Grid/list views, poster artwork, metadata display
- **TV Shows**: Season/episode organization, episode tracking
- **Music**: Album/artist views, playlist support
- **Photos**: Basic photo viewing and organization
- **Quick Browse**: Fast navigation with keyboard shortcuts
- **Search**: Global search across all media types
- **Filters**: Genre, year, rating, resolution filters
- **Collections**: Custom and automatic collections

### 3. User Interface
- **Modern Design**: Follows GNOME HIG (Human Interface Guidelines)
- **Dark Mode**: Full dark mode support
- **Responsive**: Adapts to different window sizes
- **Animations**: Smooth, purposeful animations
- **Touch Support**: Basic touch gestures for compatible devices
- **Keyboard Navigation**: Full keyboard control
- **Gamepad Support**: Navigate with game controllers

### 4. Unified Multi-Source Architecture
- **Sources**: Each source represents a media provider (Plex server, Jellyfin server, local folders)
- **Libraries**: Each source contains multiple libraries (Movies, TV Shows, Music, etc.)
- **Unified Sidebar**: All libraries from all active sources displayed together
  - **Source Grouping**: Libraries grouped by source with collapsible sections
  - **Show/Hide Controls**: Toggle visibility of entire sources or individual libraries
  - **Visual Indicators**: Icons and colors to distinguish source types
  - **Library Badges**: Show item counts and sync status per library
- **Backend Types**: 
  - **Plex**: Connect via Plex accounts, each account can access multiple servers (sources)
  - **Jellyfin**: Direct server connection with user credentials
  - **Local**: Single local source with multiple folder libraries
  - **Network** (future): Remote file shares and network sources
- **Account Management**:
  - **Plex Accounts**: Login once, discover all accessible servers automatically
  - **Multiple Accounts**: Support for multiple Plex accounts simultaneously
  - **Server Discovery**: Automatic detection of available servers per account
- **Server Optimization**: Test multiple server addresses to find fastest connection
- **Metadata Enrichment**: Automatic metadata fetching from TMDB, TVDB, OMDB
- **Smart Sync**: Each source syncs independently on configurable schedules
- **Unified Content View**: Seamless browsing across all visible libraries
- **Conflict Resolution**: Intelligent handling of duplicate media across sources

### 5. Offline-First Architecture
- **Instant Launch**: App opens immediately with cached library data
- **Background Sync**: Seamless updates without interrupting usage
- **Smart Caching**: Intelligent preloading of likely-to-watch content
- **Offline Playback**: Download media for viewing without connection
- **Queue Management**: Prioritized download queue with pause/resume
- **Storage Management**: Automatic cleanup of watched content
- **Network Awareness**: Respects WiFi-only and metered connection preferences

### 6. Advanced Features
- **Continue Watching**: Resume playback across devices
- **Up Next**: Automatic episode progression
- **Watchlist**: Personal media queue with offline sync
- **Recommendations**: Personalized content suggestions
- **Live TV**: Support for Plex/Jellyfin live TV features
- **Auto-Download**: Next episodes download automatically
- **Cast Support**: Cast to Chromecast/DLNA devices

### 7. Metadata Enrichment
- **Automatic Identification**: Parse file names to extract title, year, season, and episode information
- **Multi-Source Fetching**: Query multiple metadata providers for comprehensive information
- **Manual Matching**: Override automatic matches with manual selection
- **Metadata Editing**: Edit and customize metadata locally
- **Artwork Selection**: Choose from multiple poster and backdrop options
- **Subtitle Fetching**: Automatic subtitle download from OpenSubtitles and other providers

## User Experience Goals

### Performance
- Instant app launch (< 1 second) with cached data
- Smooth 60fps UI animations
- Fast media browsing with lazy loading
- Minimal memory footprint
- Efficient caching strategy
- Zero-wait library access (offline-first)
- Background sync never blocks UI

### Usability
- Zero-configuration for basic use
- Intuitive navigation
- Clear visual hierarchy
- Consistent interaction patterns
- Helpful empty states and error messages

### Accessibility
- Screen reader support
- Keyboard-only navigation
- High contrast mode
- Configurable font sizes
- RTL language support

## Platform Integration

### GNOME Integration
- Native GTK4/libadwaita widgets
- System theme compliance
- GNOME Online Accounts (future)
- MPRIS media controls
- Desktop notifications
- Portal support (file chooser, etc.)

### System Features
- Hardware video acceleration
- Power management awareness
- Network change handling with automatic sync pause/resume
- System proxy support
- XDG directory compliance
- Offline detection with seamless mode switching
- Background service for sync operations

## Success Metrics

### Quantitative
- App launch time < 1 second with full library visible
- Memory usage < 150MB idle
- 60fps UI performance
- < 2% CPU usage during idle
- > 95% direct play success rate
- < 5 seconds for incremental sync
- > 99% offline availability for cached content
- < 100ms UI response time when offline

### Qualitative
- User satisfaction ratings
- Feature completeness vs competitors
- GNOME ecosystem integration quality
- Community engagement and contributions

## Release Strategy

### MVP (v0.1)
- Basic Plex authentication
- Movie browsing with offline caching
- Essential playback controls
- Basic GTK4 UI
- Initial sync implementation

### Phase 1 (v0.5)
- TV show support with episode tracking
- Jellyfin backend integration
- Metadata provider integration (TMDB, TVDB)
- Local file metadata matching
- Full offline browsing capability
- Background sync service
- Improved UI/UX
- Subtitle support
- Settings panel with sync configuration

### Phase 2 (v1.0)
- Multiple backend management
- Smart download queue
- Advanced filtering with offline support
- Automatic content cleanup
- Network-aware sync
- Hardware acceleration
- Conflict resolution for multi-backend
- Polish and stability

### Future Releases
- Music support with playlist sync
- Live TV with recording
- Device-to-device sync
- Cast support
- Plugin system
- Companion mobile app

## Competition Analysis

### Strengths vs Competitors

**vs Official Plex App**
- Native performance
- Better GNOME integration
- Multiple Plex account support
- Multiple source types beyond Plex
- True offline mode with full library access
- Open source
- No internet required for local cached content

**vs Jellyfin Media Player**
- Modern Rust codebase
- Better UI/UX
- Integrated player
- Lighter resource usage
- Superior offline capabilities
- Multi-source unified interface (Plex + Jellyfin + Local)

**vs VLC**
- Server integration
- Media library organization
- Modern UI
- Focused media center experience
- Automatic metadata sync
- Smart content management

**vs Infuse (iOS)**
- Open source alternative
- Linux native
- Multiple backend types
- No subscription required
- Full offline library browsing

## Design Principles

1. **Offline First**: Always work, regardless of connectivity
2. **Native First**: Embrace platform conventions and capabilities
3. **Performance Focused**: Every millisecond counts
4. **Beautiful by Default**: Stunning without configuration
5. **Progressive Disclosure**: Simple for beginners, powerful for experts
6. **Reliability**: Never lose user progress or crash
7. **Privacy Respecting**: Local-first, minimal telemetry
8. **Smart Sync**: Intelligent background updates that never interrupt
9. **Unified Experience**: Seamless integration of multiple sources

## Constraints and Considerations

### Technical Constraints
- Must run on GNOME 42+
- Rust stable toolchain compatibility
- GStreamer for media playback
- GTK4/libadwaita for UI

### Business Constraints
- Open source (GPL-3.0 license)
- No proprietary codecs bundled
- Respect server API limits
- Community-driven development

### Legal Considerations
- Plex API terms compliance
- Codec licensing awareness
- Content rights respect
- GDPR compliance for any telemetry

## User Scenarios

### The Commuter
Sarah takes the train to work daily with spotty internet. She adds her favorite shows to Reel, which automatically downloads the next 3 episodes of each series. During her commute, she enjoys uninterrupted playback and can browse her entire library offline.

### The Multi-Source User  
John has two Plex accounts (personal and family shared), a Jellyfin server with friends, and keeps personal videos locally. When he adds his Plex accounts, Reel automatically discovers all accessible servers - his home server, his brother's server, and a friend's shared server. All libraries from all sources appear in the unified sidebar, grouped by their source. He can collapse his brother's server section when not needed, hide specific libraries like "Family Videos" that he rarely watches, and the main content area seamlessly displays items from all visible libraries. Each source syncs independently in the background, with status indicators in the sidebar showing sync progress.

### The Bandwidth-Conscious User
Maria has a metered connection. She configures Reel to only sync on WiFi, automatically downloading new episodes at night. The app respects her data limits while ensuring fresh content is always available.

### The Power User
Alex manages a 50TB media library across multiple sources - three Plex servers through two accounts, a Jellyfin server, and local archives. Reel automatically discovers all his Plex servers when he logs into his accounts, tests multiple addresses for each server to find the fastest connection. The unified sidebar shows all 15+ libraries across his sources, which he's customized by hiding rarely-used libraries and pinning his favorites to the top. The app launches instantly with the sidebar populated from cache, and the main content area aggregates media from all visible libraries with smart deduplication.

### The Local Media Collector
Emma has thousands of movie and TV files organized in folders. Reel automatically scans her directories, matches files with TMDB and TVDB data, downloads artwork, and presents her collection with the same polish as streaming services. She can manually correct any mismatches and the app remembers her preferences.

## Conclusion

Reel revolutionizes media consumption on the Linux desktop by introducing an offline-first architecture that ensures your media is always accessible. By seamlessly managing multiple backends with intelligent synchronization, it provides the reliability of local media with the convenience of streaming services. This combination of performance, native GNOME integration, and unparalleled offline capabilities will make it the definitive media player for Linux users who demand both flexibility and reliability in their media experience.