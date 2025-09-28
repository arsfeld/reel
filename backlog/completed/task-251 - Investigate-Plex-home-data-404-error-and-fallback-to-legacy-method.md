---
id: task-251
title: Investigate Plex home data 404 error and fallback to legacy method
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 13:54'
updated_date: '2025-09-26 14:14'
labels:
  - backend
  - plex
  - bug
dependencies: []
priority: high
---

## Description

The application is encountering a 404 error when fetching Plex home data from /hubs/home/refresh endpoint, causing it to fall back to a legacy method. This needs investigation to understand why the primary endpoint fails and whether the fallback is sufficient.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify why /hubs/home/refresh returns 404 for Plex backend
- [x] #2 Determine if this is a Plex API version issue or configuration problem
- [x] #3 Verify the legacy fallback method provides equivalent functionality
- [x] #4 Document the differences between primary and legacy methods
- [x] #5 Implement proper fix or improve error handling if needed
<!-- AC:END -->


## Implementation Plan

1. Test the /hubs/home/refresh endpoint directly with curl to understand the 404 response
2. Check if there are any missing headers or API version requirements
3. Compare the legacy method with the new endpoint to understand feature parity
4. Verify Plex server version compatibility
5. Implement proper error handling and logging
6. Document the differences between methods


## Implementation Notes

## Investigation Results

The `/hubs/home/refresh` endpoint does not exist in the Plex API. This was confirmed through:
1. Receiving consistent 404 errors from Plex servers
2. Researching official Plex API documentation
3. Checking community resources and API specifications


## Root Cause

The code was attempting to use a non-existent endpoint `/hubs/home/refresh`. This endpoint does not exist in any version of the Plex API.

## Solution Implemented

1. Removed the invalid `fetch_home_hubs()` method that was trying to call `/hubs/home/refresh`
2. Renamed the "legacy" method to `get_home_sections_standard()` as it uses the correct Plex API endpoints:
   - `/library/onDeck` - For Continue Watching items
   - `/library/recentlyAdded` - For Recently Added items
   - `/hubs/sections/{id}` - For library-specific hubs
3. Updated documentation to clarify the correct API usage
4. Removed unnecessary fallback logic since there is only one correct method

## Files Modified

- `src/backends/plex/api/home.rs`: Removed invalid endpoint, renamed methods, updated documentation

The application now uses only the standard Plex API endpoints, eliminating the 404 errors and unnecessary fallback logic.
