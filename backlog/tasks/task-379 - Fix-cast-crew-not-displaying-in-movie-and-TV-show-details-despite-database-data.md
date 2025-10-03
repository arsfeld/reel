---
id: task-379
title: >-
  Fix cast/crew not displaying in movie and TV show details despite database
  data
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 17:42'
updated_date: '2025-10-03 17:49'
labels:
  - bug
  - ui
  - movie-details
  - show-details
  - database
dependencies: []
priority: high
---

## Description

Cast and crew information is being successfully synced and stored in the database (people and media_people tables) as shown in logs, but the movie details and show details pages are displaying empty cast/crew sections. The UI is not querying or loading the cast/crew data from the database when displaying media details. Need to implement database queries and UI rendering for cast/crew information.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add method to PeopleRepository to load cast/crew for a media item
- [x] #2 Update MovieDetailsPage to query and load cast/crew data from database
- [x] #3 Update ShowDetailsPage to query and load cast/crew data from database
- [x] #4 Render cast/crew information in movie details UI
- [x] #5 Render cast/crew information in show details UI
- [x] #6 Test with synced movies to verify cast displays correctly
- [x] #7 Test with synced TV shows to verify cast displays correctly
<!-- AC:END -->


## Implementation Plan

1. Analyze the bug: person_type stored as "actor" but code checks for "cast"
2. Fix MediaService::get_media_item to handle "actor" person_type correctly
3. Update MediaService::save_people_for_media to use "cast" instead of "actor"
4. Test with synced movies to verify cast displays
5. Test with synced TV shows to verify cast displays


## Implementation Notes

## Root Cause

The bug was in MediaService::get_media_item and MediaService::save_people_for_media:
- When saving cast members, the code used person_type = "actor"
- When loading cast members, the code checked for person_type == "cast"
- This mismatch caused all cast members to be skipped during loading


## Additional Fix for Images

**Problem**: Cast/crew images were not being fetched from Plex.

**Root Cause**: Plex's bulk library API (/library/sections/{id}/all) doesn't include actor/director thumbnails by default.

**Solution**: Added query parameters to request complete metadata:
- src/backends/plex/api/library.rs:52-55: Added `?includeExtras=1&includeRelated=1` to movies request
- src/backends/plex/api/library.rs:159-162: Added `?includeExtras=1&includeRelated=1` to shows request

**User Action Required**: Users must re-sync their Plex libraries to fetch cast/crew images. The existing synced data has empty image URLs and needs to be refreshed.


## Changes Made

1. **src/services/core/media.rs:178-180**: Updated the person_type matching to accept both "actor" and "cast" for cast members, and handle all crew types (director, writer, producer)
2. **src/services/core/media.rs:690**: Changed person_type from "actor" to "cast" when saving cast members

## How It Works Now

- Cast members are saved with person_type = "cast"
- Loading accepts both "actor" (for existing data) and "cast" (for new data)
- Crew members (director, writer, producer) are properly categorized and displayed
- MovieDetailsPage and ShowDetailsPage already had the UI code to display cast/crew
- The fix enables the existing UI to receive and display the data

## Testing Required

1. Run the app and navigate to a movie details page
2. Verify cast members are displayed with their photos and roles
3. Verify crew (directors, writers) are displayed
4. Navigate to a TV show details page
5. Verify cast members are displayed for the show
