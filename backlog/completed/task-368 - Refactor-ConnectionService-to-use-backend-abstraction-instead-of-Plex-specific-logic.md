---
id: task-368
title: >-
  Refactor ConnectionService to use backend abstraction instead of Plex-specific
  logic
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 16:21'
updated_date: '2025-10-03 16:27'
labels:
  - refactoring
  - architecture
  - backends
  - connection
dependencies: []
priority: high
---

## Description

The ConnectionService.test_connections() method currently contains Plex-specific logic (checking for plex.direct, plex.tv URLs, hardcoded /identity endpoint). This violates backend abstraction and makes it impossible to properly support multiple backend types. Need to refactor so each backend implements its own connection testing through the MediaBackend trait.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add test_connection() method to MediaBackend trait that accepts a connection URL and returns availability/latency
- [x] #2 Implement test_connection() in PlexBackend with Plex-specific logic (identity endpoint, token handling)
- [x] #3 Implement test_connection() in JellyfinBackend with Jellyfin-specific logic
- [x] #4 Refactor ConnectionService.test_connections() to call backend.test_connection() instead of hardcoded Plex logic
- [x] #5 Remove all URL pattern matching (plex.direct, plex.tv) from ConnectionService
- [x] #6 Remove all endpoint hardcoding (/identity) from ConnectionService
- [x] #7 Verify connection testing works for both Plex and Jellyfin backends
<!-- AC:END -->


## Implementation Plan

1. Add test_connection() to MediaBackend trait (url, token) -> Result<(bool, Option<u64>)>
2. Implement in PlexBackend using /identity endpoint and standard headers
3. Implement in JellyfinBackend using appropriate Jellyfin health check endpoint
4. Refactor ConnectionService to get backend instance and delegate to backend.test_connection()
5. Remove Plex-specific URL matching (plex.direct, plex.tv) from ConnectionService
6. Remove hardcoded create_standard_headers import from ConnectionService
7. Test with both Plex and Jellyfin sources


## Implementation Notes

Successfully refactored ConnectionService to use backend abstraction instead of Plex-specific logic.


## Changes Made

### 1. MediaBackend Trait (src/backends/traits.rs)
- Added test_connection() method to trait that accepts URL and optional auth token
- Returns Result<(bool, Option<u64>)> for availability and response time

### 2. PlexBackend (src/backends/plex/mod.rs)
- Implemented test_connection() using /identity endpoint
- Uses create_standard_headers() for Plex-specific authentication
- Accepts self-signed certificates (danger_accept_invalid_certs)
- 5-second timeout for connection tests

### 3. JellyfinBackend (src/backends/jellyfin/mod.rs)
- Implemented test_connection() using /System/Info/Public endpoint
- Public endpoint requires no authentication
- 5-second timeout for connection tests

### 4. LocalBackend (src/backends/local/mod.rs)
- Implemented test_connection() to check file:// path existence
- Returns instant (0ms) response time for local files

### 5. ConnectionService (src/services/core/connection.rs)
- Refactored test_connections() to use backend abstraction
- Now accepts db and source entity parameters
- Creates backend instances and delegates to backend.test_connection()
- Removed all Plex-specific URL pattern matching (plex.tv, plex.direct)
- Removed hardcoded /identity endpoint
- Removed direct import of create_standard_headers from Plex backend
- Removed unused imports (Client, Instant)

## Architecture Improvements
- Each backend now handles its own connection testing logic
- Proper separation of concerns - ConnectionService no longer knows about Plex specifics
- Easy to extend for future backend types
- Maintains parallel connection testing for performance
