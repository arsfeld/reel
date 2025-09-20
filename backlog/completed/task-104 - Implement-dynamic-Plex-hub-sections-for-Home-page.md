---
id: task-104
title: Implement dynamic Plex hub sections for Home page
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 19:37'
updated_date: '2025-09-17 02:57'
labels:
  - feature
  - ui
  - plex
dependencies: []
priority: high
---

## Description

Replace hardcoded Home page sections with dynamic hub data from Plex API. Plex provides various hubs like 'Recently Added Movies', 'Popular This Week', 'Top Rated', etc. that should be displayed dynamically based on what the server provides.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Fetch hub data from Plex /hubs/home/refresh endpoint
- [x] #2 Dynamically create sections based on hub response
- [x] #3 Support all Plex hub types (movie, show, episode, mixed)
- [x] #4 Handle hub-specific layouts (hero, shelf, grid)
- [x] #5 Respect hub size limits and sorting
<!-- AC:END -->


## Implementation Plan

1. Research Plex /hubs/home/refresh API endpoint
2. Update PlexApi struct to support home hub endpoint
3. Implement fetch_home_hubs method for /hubs/home/refresh
4. Parse hub response to handle different hub types and layouts
5. Modify get_home_sections to use dynamic hub data
6. Map hub types to HomeSectionType enum
7. Test with real Plex server


## Implementation Notes

## Implementation Summary

Successfully implemented dynamic Plex hub sections using the /hubs/home/refresh endpoint.

### Changes Made:

1. **Enhanced PlexHub struct** - Added fields to capture hub metadata:
   - hub_type: Media type (movie/show/mixed)
   - hub_identifier: Unique hub ID
   - context: Hub context for categorization
   - size: Display limit for items
   - style: Display style (shelf/hero/grid)
   - promoted: Promotion status

2. **New fetch_home_hubs method** - Fetches and processes /hubs/home/refresh endpoint:
   - Makes HTTP request to Plex home hubs endpoint
   - Parses hub response into HomeSection structures
   - Respects size limits specified by Plex
   - Logs detailed hub information for debugging

3. **Hub to section type mapping** - Smart mapping logic:
   - First checks context field for specific hub types
   - Falls back to title-based detection
   - Supports all standard types: ContinueWatching, RecentlyAdded, TopRated, Trending, etc.
   - Uses Custom type for unrecognized hubs

4. **Fallback mechanism** - Graceful degradation:
   - Tries new /hubs/home/refresh endpoint first
   - Falls back to legacy method if endpoint fails
   - Ensures backward compatibility

5. **Clone trait additions** - Fixed ownership issues:
   - Added Clone to PlexGenericMetadata
   - Added Clone to PlexTag

### Modified Files:
- src/backends/plex/api.rs

The implementation now dynamically loads all Plex hub sections, respecting server-provided metadata for layout, size limits, and categorization. The fallback ensures compatibility with older Plex servers.
