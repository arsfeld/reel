# Jellyfin Integration Implementation Plan

## Overview
This document outlines the comprehensive plan for implementing full Jellyfin support in Reel, providing feature parity with the existing Plex backend while leveraging Jellyfin-specific capabilities.

## Phase 1: Core API Infrastructure

### 1.1 Authentication System
- [ ] Implement Jellyfin authentication flow
  - Username/password authentication
  - API key generation and management
  - Quick Connect support (PIN-based authentication)
  - LDAP/SSO authentication support (if configured)
- [ ] Create credential storage mechanism
  - Secure token storage in system keyring
  - Support for multiple Jellyfin servers
  - Auto-discovery of local Jellyfin servers via broadcast
- [ ] Session management
  - Token refresh logic
  - Connection state monitoring
  - Automatic reconnection on network changes

### 1.2 API Client Foundation
- [ ] Create `JellyfinApi` struct similar to `PlexApi`
  - HTTP client configuration with proper timeouts
  - Request/response error handling
  - Rate limiting and retry logic
- [ ] Implement core API endpoints
  - `/System/Info` - Server information
  - `/Users/AuthenticateByName` - Authentication
  - `/Users/{userId}/Items` - Library browsing
  - `/Items/{itemId}/PlaybackInfo` - Stream URLs
  - `/Sessions/Playing/Progress` - Playback tracking
- [ ] Response parsing structures
  - Define Jellyfin-specific DTOs
  - Map Jellyfin responses to internal models
  - Handle API versioning differences

## Phase 2: Media Library Integration

### 2.1 Library Discovery
- [ ] Implement `get_libraries()` 
  - Fetch user's accessible libraries
  - Support for all Jellyfin library types:
    - Movies
    - TV Shows
    - Music
    - Books
    - Mixed Content
    - Live TV
- [ ] Library metadata extraction
  - Library icons and artwork
  - Collection grouping support
  - Virtual folders handling

### 2.2 Content Retrieval
- [ ] Movies implementation
  - Fetch movie listings with filters
  - Support for collections and box sets
  - Extra content (trailers, specials)
- [ ] TV Shows implementation
  - Show and season hierarchy
  - Episode listings with metadata
  - Next up/continue watching
  - Missing episode handling
- [ ] Music support (optional enhancement)
  - Album and artist browsing
  - Playlist support
  - Genre and mood categorization

### 2.3 Metadata Mapping
- [ ] Convert Jellyfin metadata to internal models
  - Primary metadata (title, year, overview)
  - Artwork URLs (poster, backdrop, logo)
  - Cast and crew information
  - Ratings from various sources
  - Content ratings and parental controls
- [ ] Handle Jellyfin-specific fields
  - Community rating vs critic rating
  - Multiple image types
  - External IDs (IMDB, TMDB, etc.)

## Phase 3: Playback and Streaming

### 3.1 Stream URL Generation
- [ ] Implement `get_stream_url()`
  - Direct play URL construction
  - Transcoding URL generation
  - Subtitle URL handling
  - Audio track selection
- [ ] Quality management
  - Bitrate detection and selection
  - Resolution options
  - HDR/SDR handling
  - Container format support

### 3.2 Transcoding Support
- [ ] Detect transcoding requirements
  - Client capability reporting
  - Codec compatibility checks
  - Bandwidth detection
- [ ] Transcoding parameters
  - Video codec selection (H.264, H.265, AV1)
  - Audio codec selection
  - Container format selection
  - Hardware acceleration detection

### 3.3 Playback Progress
- [ ] Implement progress tracking
  - `update_progress()` - Report playback position
  - `mark_watched()` - Mark items as watched
  - `mark_unwatched()` - Mark items as unwatched
  - Scrobbling support
- [ ] Resume functionality
  - Store and retrieve playback positions
  - Cross-device resume support
  - Handle partially watched items

## Phase 4: Advanced Features

### 4.1 Search Functionality
- [ ] Implement `search()`
  - Multi-type search (movies, shows, episodes)
  - Filter support (year, genre, rating)
  - Person search (actors, directors)
  - Advanced search syntax support
- [ ] Search result ranking
  - Relevance scoring
  - Recent/popular weighting
  - User preference learning

### 4.2 Home Screen Integration
- [ ] Implement `get_home_sections()`
  - Latest media additions
  - Continue watching
  - Next up episodes
  - Recommendations
  - Live TV guide (if available)
- [ ] Custom collections
  - User-created collections
  - Smart playlists
  - Favorites and likes

### 4.3 Live TV and DVR (Optional)
- [ ] Live TV support
  - Channel listings
  - EPG (Electronic Program Guide)
  - Recording management
- [ ] DVR functionality
  - Schedule recordings
  - Series recording rules
  - Recording playback

## Phase 5: Jellyfin-Specific Features

### 5.1 SyncPlay Support
- [ ] Implement synchronized playback
  - Room creation and joining
  - Playback synchronization
  - Chat functionality
  - Permission management

### 5.2 User Management
- [ ] Multi-user support
  - User switching
  - Parental controls
  - Age ratings and restrictions
  - Viewing history per user

### 5.3 Plugin Integration
- [ ] Support for Jellyfin plugins
  - Intro skipper integration
  - Anime plugin support
  - Trakt scrobbling
  - Custom metadata providers

### 5.4 Offline Sync
- [ ] Download functionality
  - Download queue management
  - Offline playback support
  - Sync status tracking
  - Storage management

## Phase 6: UI Integration

### 6.1 Settings Page
- [ ] Jellyfin server configuration UI
  - Server URL input
  - Authentication UI (username/password, Quick Connect)
  - Server discovery UI
  - Connection test functionality
- [ ] User preferences
  - Default quality settings
  - Subtitle preferences
  - Audio track preferences
  - Home screen customization

### 6.2 Server Selection
- [ ] Multi-server support UI
  - Server switcher in main UI
  - Server status indicators
  - Quick server actions
- [ ] Mixed backend support
  - Allow Jellyfin + Plex simultaneously
  - Unified library view option
  - Server priority settings

### 6.3 Jellyfin-Specific UI Elements
- [ ] Live TV interface (if applicable)
  - Channel guide
  - Recording scheduler
  - DVR management
- [ ] SyncPlay controls
  - Room management UI
  - Sync status indicators
  - Group playback controls

## Technical Implementation Details

### API Communication
```rust
// Example API structure
pub struct JellyfinApi {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    user_id: String,
    device_id: String,
    session_id: String,
}

// Authentication header format
Authorization: MediaBrowser Client="Reel", Device="Linux", DeviceId="{device_id}", Version="0.1.0", Token="{api_key}"
```

### Key Jellyfin API Endpoints
- `/Users/AuthenticateByName` - POST - Initial authentication
- `/Users/{userId}/Items` - GET - Fetch library items
- `/Items/{itemId}` - GET - Get specific item details
- `/Items/{itemId}/PlaybackInfo` - POST - Get playback URLs
- `/Videos/{itemId}/stream` - GET - Direct stream access
- `/Sessions/Playing` - POST - Report playback start
- `/Sessions/Playing/Progress` - POST - Update playback progress
- `/Sessions/Playing/Stopped` - POST - Report playback stop

### Data Model Mapping
```rust
// Jellyfin to internal model mapping examples
JellyfinMovie -> Movie {
    id: jellyfin.Id,
    title: jellyfin.Name,
    year: jellyfin.ProductionYear,
    duration: Duration::from_secs(jellyfin.RunTimeTicks / 10_000_000),
    rating: jellyfin.CommunityRating,
    // ... additional mappings
}
```

## Testing Strategy

### Unit Tests
- [ ] API client methods
- [ ] Response parsing and mapping
- [ ] Error handling scenarios
- [ ] Authentication flows

### Integration Tests
- [ ] Full authentication cycle
- [ ] Library browsing workflow
- [ ] Playback initiation and tracking
- [ ] Search functionality
- [ ] Offline sync operations

### Manual Testing Checklist
- [ ] Server discovery and connection
- [ ] Various authentication methods
- [ ] Different library types
- [ ] Direct play vs transcoding
- [ ] Multi-user scenarios
- [ ] Network interruption handling
- [ ] Large library performance

## Migration Considerations

### From Plex to Jellyfin
- [ ] Watch status migration tool
- [ ] Playlist conversion
- [ ] User preference migration
- [ ] Collection mapping

### Coexistence Strategy
- [ ] Unified search across backends
- [ ] Combined home screen sections
- [ ] Cross-backend continue watching
- [ ] Deduplicated content detection

## Performance Optimizations

### Caching Strategy
- [ ] Response caching with TTL
- [ ] Image caching and preloading
- [ ] Metadata prefetching
- [ ] Search result caching

### Network Optimization
- [ ] Connection pooling
- [ ] Request batching where possible
- [ ] Lazy loading for large lists
- [ ] Progressive image loading

## Security Considerations

### Authentication Security
- [ ] Secure token storage (system keyring)
- [ ] Token rotation support
- [ ] SSL/TLS enforcement option
- [ ] Certificate validation

### Privacy Features
- [ ] Incognito mode support
- [ ] Watch history management
- [ ] Clear cache functionality
- [ ] Anonymous usage statistics

## Documentation Requirements

### User Documentation
- [ ] Jellyfin setup guide
- [ ] Feature comparison (Plex vs Jellyfin)
- [ ] Troubleshooting guide
- [ ] FAQ section

### Developer Documentation
- [ ] API integration guide
- [ ] Testing procedures
- [ ] Contribution guidelines
- [ ] Architecture documentation

## Success Metrics

### Functionality Metrics
- Complete feature parity with Plex backend
- All core Jellyfin features supported
- Seamless playback experience
- Reliable sync and offline support

### Performance Metrics
- Library load time < 2 seconds
- Playback start time < 3 seconds
- Search response time < 1 second
- Memory usage comparable to Plex backend

### User Experience Metrics
- Intuitive server setup process
- Smooth playback without buffering
- Accurate progress tracking
- Reliable offline functionality

## Implementation Order

**Phase 1**: Core API infrastructure and authentication
**Phase 2**: Library integration and playback functionality  
**Phase 3**: Advanced features and Jellyfin-specific capabilities
**Phase 4**: UI integration and testing

## Dependencies and Prerequisites

### External Dependencies
- `reqwest` for HTTP communication
- `serde` for JSON serialization
- `tokio` for async runtime
- `chrono` for date/time handling

### Jellyfin Server Requirements
- Jellyfin Server 10.8.0 or higher
- API key or user authentication enabled
- Proper network configuration (ports, firewall)
- SSL certificate (recommended)

## Risk Mitigation

### Technical Risks
- API version incompatibilities -> Support multiple API versions
- Network reliability issues -> Implement robust retry logic
- Large library performance -> Implement pagination and lazy loading
- Transcoding server load -> Offer quality presets and warnings

### User Experience Risks
- Complex setup process -> Provide server discovery and setup wizard
- Feature differences from Plex -> Clear documentation and migration guides
- Multi-server confusion -> Clear server indicators and switching UI

## Future Enhancements

### Post-Launch Features
- [ ] Jellyfin Connect integration (cloud authentication)
- [ ] Advanced subtitle management (OpenSubtitles integration)
- [ ] Social features (reviews, recommendations)
- [ ] Mobile sync support
- [ ] Cast device support (Chromecast, AirPlay)
- [ ] VR/360 video support
- [ ] HDR tone mapping options
- [ ] Advanced audio (Atmos, DTS:X) support

### Community Features
- [ ] Plugin marketplace integration
- [ ] Theme support
- [ ] Custom metadata providers
- [ ] Third-party service integrations

## Conclusion

This implementation plan provides a structured approach to adding comprehensive Jellyfin support to Reel. The phased approach ensures that core functionality is delivered early while allowing for iterative improvements and feature additions. The plan prioritizes user experience, performance, and reliability while maintaining compatibility with existing Plex functionality.