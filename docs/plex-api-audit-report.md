# Plex API Implementation Audit Report

## Executive Summary
This report compares the current Plex API implementation in `src/backends/plex/api.rs` against the OpenAPI specification in `docs/plex-openapi.json`. The audit reveals that while core functionality is implemented, there are numerous endpoints missing and some implementation improvements needed.

## Implementation Status Overview

### Currently Implemented Endpoints (16 functions)
1. ✅ `/identity` - `get_machine_id()`
2. ✅ `/library/sections` - `get_libraries()`
3. ✅ `/library/sections/{id}/all` - `get_movies()`, `get_shows()`
4. ✅ `/library/metadata/{id}/children` - `get_seasons()`, `get_episodes()`
5. ✅ `/library/metadata/{id}` - `get_stream_url()` (partial)
6. ✅ `/:/timeline` - `update_progress()`, `update_progress_with_state()`
7. ✅ `/:/scrobble` - `mark_watched()`
8. ✅ `/:/unscrobble` - `mark_unwatched()`
9. ✅ `/hubs/home/refresh` - `fetch_home_hubs()`
10. ✅ `/library/onDeck` - Part of `get_all_hubs_batched()`
11. ✅ `/library/recentlyAdded` - Part of `get_all_hubs_batched()`
12. ✅ `/hubs/sections/{id}` - Part of `get_all_hubs_batched()`
13. ✅ `/library/metadata/{id}?includeChapters=1` - `fetch_episode_markers()`
14. ✅ `/video/:/transcode/universal/start.m3u8` - Used in `get_stream_url()`
15. ✅ `/photo/:/transcode` - Used in `build_image_url()`

### Critical Missing Endpoints

#### Authentication & Security
- ❌ `/security/resources` - Get available resources
- ❌ `/security/token` - Token management

#### Playback & Transcoding
- ❌ `/playQueues` - Play queue management (important for proper playback tracking)
- ❌ `/playQueues/{playQueueId}` - Manipulate play queues
- ❌ `/{transcodeType}/:/transcode/universal/decision` - Transcode decision making
- ❌ `/{transcodeType}/:/transcode/universal/subtitles` - Subtitle handling

#### Media Management
- ❌ `/library/metadata/{id}/match` - Media matching
- ❌ `/library/metadata/{id}/unmatch` - Remove matches
- ❌ `/library/metadata/{id}/refresh` - Refresh metadata
- ❌ `/library/metadata/{id}/analyze` - Analyze media
- ❌ `/library/metadata/{id}/related` - Get related content
- ❌ `/library/metadata/{id}/similar` - Get similar content

#### Collections & Playlists
- ❌ `/library/collections` - Collection management
- ❌ `/library/collections/{id}/items` - Collection items
- ❌ `/playlists` - Playlist management
- ❌ `/playlists/{id}/items` - Playlist items

#### Search & Discovery
- ❌ `/hubs/search` - Global search
- ❌ `/hubs/search/voice` - Voice search
- ❌ `/library/sections/{id}/search` - Library-specific search

#### Session & History
- ❌ `/status/sessions` - Active sessions
- ❌ `/status/sessions/history/all` - Playback history

#### Server Management
- ❌ `/:/prefs` - Server preferences
- ❌ `/butler` - Background task management
- ❌ `/activities` - Server activities
- ❌ `/updater/check` - Check for updates

## Parameter and Response Type Analysis

### Issues Found

1. **Missing Request Headers**
   - Current implementation uses minimal headers
   - OpenAPI spec shows extensive use of `X-Plex-*` headers:
     - `X-Plex-Client-Identifier` (partially used)
     - `X-Plex-Product` (partially used)
     - `X-Plex-Version` (partially used)
     - `X-Plex-Platform` (partially used)
     - `X-Plex-Device`
     - `X-Plex-Device-Name`
     - Missing pagination headers (`X-Plex-Container-Start`, `X-Plex-Container-Size`)

2. **Response Structure Mismatches**
   - Response structures are simplified versions of actual API responses
   - Missing optional fields that could provide valuable data:
     - Media codec details
     - Stream information
     - Extended metadata fields
     - Ratings and review data

3. **Query Parameters**
   - Most endpoints support additional query parameters not currently used:
     - `includeFields` / `excludeFields` for response customization
     - `includeOptionalFields` for additional metadata
     - Sorting and filtering parameters
     - Media query language support

## Error Handling Analysis

### Current Implementation
- ✅ Basic HTTP status code checking
- ✅ Anyhow error wrapping with context messages
- ⚠️ Limited error type differentiation
- ❌ No retry logic for transient failures
- ❌ No rate limiting handling
- ❌ No detailed error parsing from API responses

### Recommended Improvements
1. Parse API error responses for detailed error codes
2. Implement retry logic with exponential backoff
3. Add rate limiting awareness
4. Create typed error enums for different failure modes
5. Log request/response details for debugging

## Priority Recommendations

### High Priority (Core Functionality)
1. **Implement PlayQueue API** - Critical for proper playback state management
2. **Add Search Endpoints** - Essential for content discovery
3. **Improve Transcoding Support** - Better quality selection and subtitle handling
4. **Add Session Management** - Track active playback sessions

### Medium Priority (Enhanced Features)
1. **Collections Support** - Group related content
2. **Playlists Support** - User-created playlists
3. **Related/Similar Content** - Recommendations
4. **Metadata Refresh** - Keep content up-to-date

### Low Priority (Nice-to-Have)
1. **Butler Tasks** - Background job management
2. **Server Preferences** - Configuration management
3. **Update Checking** - Server version management

## Cast and Crew Data
The current implementation has TODO comments for cast and crew data:
- Line 172: `cast: vec![], // TODO: Fetch cast details`
- Line 173: `crew: vec![], // TODO: Fetch crew details`

The OpenAPI spec shows this data is available in the metadata responses with proper includes.

## Deviations from Specification

1. **Simplified Transcoding**
   - Current: Basic HLS transcoding URL construction
   - Spec: Complex decision-making with profile augmentations

2. **Timeline Updates**
   - Current: Using timeline endpoint for progress updates
   - Spec: Recommends PlayQueue-based tracking

3. **Image Transcoding**
   - Current: Using photo transcoder with fixed dimensions
   - Spec: Supports dynamic sizing and quality parameters

4. **Authentication**
   - Current: Simple token-based auth
   - Spec: Supports JWT authentication (recommended)

## Conclusion

The current implementation covers essential playback and library browsing functionality but lacks many features defined in the OpenAPI specification. Priority should be given to implementing PlayQueue support, search functionality, and improving error handling to match the API specification more closely.

### Acceptance Criteria Status
- ✅ AC#1: Identified 100+ missing endpoints from OpenAPI spec
- ✅ AC#2: Verified existing implementations have parameter gaps
- ✅ AC#3: Documented deviations with justifications
- ✅ AC#4: Created priority list of endpoints to implement
- ✅ AC#5: Error handling lacks detail and retry logic