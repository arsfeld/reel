---
id: task-131
title: >-
  Fix Continue Watching thumbnails to show TV show poster instead of episode
  thumbnail
status: Done
assignee:
  - '@claude'
created_date: '2025-09-17 03:10'
updated_date: '2025-09-17 03:15'
labels: []
dependencies: []
priority: high
---

## Description

The Continue Watching section on the home screen currently shows episode thumbnails for TV shows. This should be changed to display the parent TV show's poster/thumbnail for consistency with other sections and better visual identification.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Continue Watching items for TV show episodes display the show's poster
- [x] #2 Movie items in Continue Watching continue to show movie posters correctly
- [x] #3 Thumbnail selection logic properly identifies parent show for episodes
<!-- AC:END -->


## Implementation Plan

1. Search for Continue Watching section implementation in the codebase
2. Identify where thumbnails are selected for Continue Watching items
3. Find the logic that determines episode vs show thumbnails
4. Modify the logic to use parent show poster for TV episodes
5. Test both TV shows and movies display correctly
6. Verify all acceptance criteria are met


## Implementation Notes

Fixed Continue Watching section to display TV show posters instead of episode thumbnails.

Changes made:
1. Modified src/platforms/relm4/components/pages/home.rs:
   - Updated media_item_to_model() function to use show_poster_url for episodes
   - Used .or() fallback to episode thumbnail_url if show poster is not available

2. Updated src/backends/jellyfin/api.rs:
   - Modified 4 locations where Episode items are created
   - Now populates show_poster_url using series_id to build proper Jellyfin image URL
   - Format: "{base_url}/Items/{series_id}/Images/Primary"

3. Plex backend already had proper show_poster_url implementation using grandparent_thumb field

The fix ensures that:
- Episodes in Continue Watching now display their parent show's poster
- Movies continue to show their own posters correctly
- Fallback to episode thumbnail if show poster is unavailable
- Works for both Plex and Jellyfin backends
