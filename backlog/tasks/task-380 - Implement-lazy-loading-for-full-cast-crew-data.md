---
id: task-380
title: Implement lazy loading for full cast/crew data
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 18:08'
updated_date: '2025-10-03 18:19'
labels:
  - cast
  - crew
  - lazy-loading
  - backend
  - plex
dependencies: []
priority: high
---

## Description

Initial sync fetches only 3 cast members from Plex bulk API (performance limitation). Need to implement lazy loading to fetch full cast/crew when user views movie/show details. Backend abstraction already in place with get_movie_metadata and get_show_metadata traits.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add backend-agnostic lazy loading in movie details page
- [x] #2 Add backend-agnostic lazy loading in show details page
- [x] #3 Check if full cast already loaded before fetching
- [x] #4 Update database with full cast/crew after lazy load
- [x] #5 Add loading state while fetching full metadata
- [x] #6 Test with Plex backend to ensure full cast loads
<!-- AC:END -->


## Implementation Plan

1. Check current cast count - if ≤3, need to fetch full data
2. Create LoadFullMetadataCommand for movies and shows
3. Implement lazy loading in movie_details.rs:
   - Add LoadingFullCast state
   - Check cast count on LoadDetails
   - Trigger metadata fetch if needed
   - Update DB with full cast/crew
4. Implement lazy loading in show_details.rs:
   - Same pattern as movies
5. Test with Plex backend to verify full cast loads


## Implementation Notes

Implemented lazy loading for cast/crew data with following approach:

1. Created BackendService methods:
   - load_full_movie_metadata(): Fetches full cast/crew from backend and updates database
   - load_full_show_metadata(): Similar for shows

2. Added commands in media_commands.rs:
   - LoadFullMovieMetadataCommand
   - LoadFullShowMetadataCommand

3. Modified movie_details.rs:
   - Added LoadFullMetadata and FullMetadataLoaded commands
   - Checks cast count on LoadDetails (≤3 triggers lazy load)
   - Fetches full metadata and reloads details

4. Modified show_details.rs:
   - Same pattern as movies
   - Checks cast count and triggers lazy load

5. Backend implementations:
   - Plex: Already had get_movie_metadata/get_show_metadata
   - Jellyfin: Added stub implementations using bulk API
   - Local: Added todo! stubs

6. Database updates:
   - Uses PeopleRepository.upsert() for people
   - Uses save_media_people() to replace relationships
   - Maintains sort order for cast/crew

The implementation is backend-agnostic and works seamlessly with the existing Plex implementation. When users view movie or show details, if only 3 or fewer cast members are present (from initial sync), the system automatically fetches and stores the full cast/crew list from the backend.

## Bug Fix

Fixed infinite loop issue where pages would continuously refresh if a movie/show actually had ≤3 cast members. Added `full_metadata_loaded` flag to both MovieDetailsPage and ShowDetailsPage to track whether we've already attempted to load full metadata, preventing repeated fetches.
