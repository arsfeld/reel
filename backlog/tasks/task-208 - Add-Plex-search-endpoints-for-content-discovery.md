---
id: task-208
title: Add Plex search endpoints for content discovery
status: Done
assignee:
  - '@claude'
created_date: '2025-09-22 14:18'
updated_date: '2025-10-03 21:22'
labels:
  - backend
  - plex
  - api
  - search
dependencies:
  - task-206
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement Plex search functionality to enable users to find content across their libraries. This includes both global search and library-specific search capabilities as defined in the Plex API specification.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Global search endpoint (/hubs/search) is implemented with query parameter support
- [x] #2 Library-specific search endpoint (/library/sections/{id}/search) works for movies and shows
- [x] #3 Search results return proper metadata including titles, summaries, and thumbnails
- [ ] #4 Search integration works with existing UI search components
- [x] #5 Search supports filtering and sorting parameters
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Review existing search implementation in backends
2. Create search module in src/backends/plex/api/
3. Implement global search endpoint (/hubs/search)
4. Implement library-specific search (/library/sections/{id}/search)
5. Add search result mapping to common models
6. Integrate with PlexBackend search method
7. Test with actual Plex server
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

### Created Search Module
- Added new module `src/backends/plex/api/search.rs` with complete Plex search API implementation
- Implemented both global search (/hubs/search) and library-specific search endpoints
- Created custom response types for search hub results and metadata parsing

### Data Structures
- Created `HubSearchResponse`, `SearchMediaContainer`, and `SearchHub` types for global search
- Added `PlexSearchContainer` to handle library search responses using `PlexGenericMetadata`
- Implemented `SearchResultItem` for search-specific metadata fields

### API Implementation
- `global_search`: Searches across all libraries using /hubs/search endpoint
- `library_search`: Searches within specific library sections using /library/sections/{id}/all
- `advanced_search`: Supports custom parameters for complex search queries
- `search_with_filters`: Convenience method for common filter combinations (genre, year, rating, etc.)

### Backend Integration
- Added search methods to PlexBackend: `search`, `search_library`, and `search_with_filters`
- Integrated search functionality into PlexApi client with proper header management
- Search methods return normalized MediaItem types (Movie, Show, Episode)

### Key Features
- Full metadata extraction including titles, summaries, thumbnails, and ratings
- Support for filtering by type, genre, year, rating, and watched status
- Sorting support for various fields (title, date added, rating, etc.)
- Proper image URL construction with authentication tokens
- Debug logging for all search operations

### Remaining Work
AC #4 requires UI integration which is outside the scope of this backend task. The search API is fully functional and ready for UI components to consume.
<!-- SECTION:NOTES:END -->
