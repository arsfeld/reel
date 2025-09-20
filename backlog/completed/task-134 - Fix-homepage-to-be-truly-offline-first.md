---
id: task-134
title: Fix homepage to be truly offline-first
status: Done
assignee:
  - '@claude'
created_date: '2025-09-17 03:20'
updated_date: '2025-09-17 03:47'
labels: []
dependencies: []
priority: high
---

## Description

The homepage currently doesn't follow the offline-first architecture principle. It should load instantly from the SQLite cache without waiting for backend API calls. Currently, the homepage may show loading states or empty sections when backends are unavailable, instead of displaying cached content immediately.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Homepage loads instantly from SQLite cache on app startup
- [x] #2 No loading spinners or empty states when cached data exists
- [x] #3 Background sync updates content without blocking UI
- [x] #4 Homepage displays cached content even when all backends are offline
- [x] #5 Proper fallback to cache when API calls fail
<!-- AC:END -->


## Implementation Plan

1. Create method to load cached home sections from database
2. Modify HomePage init to load cached data immediately
3. Trigger API calls in background after UI loads
4. Update display methods to merge cached and fresh data
5. Add error handling to fallback to cache when API fails

## Implementation Notes

Implemented offline-first homepage loading:

1. Created BackendService::get_cached_home_sections() method that loads sections from SQLite cache
2. Added MediaRepository::find_by_source_and_type() to query cached media by source and type
3. Modified HomePage component to load cached data immediately on startup
4. Changed initial loading state to false for instant display
5. Background API refresh runs after cached data is displayed
6. API failures no longer block UI - cached data remains visible
7. Added clear_source_sections() to properly update sections when fresh data arrives

The homepage now loads instantly from cache and updates seamlessly when API data arrives.
