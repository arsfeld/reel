---
id: task-273
title: Fix episode sync storing all episodes with season_number=0
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 23:25'
updated_date: '2025-09-26 23:40'
labels:
  - bug
  - sync
  - episodes
dependencies: []
---

## Description

Critical bug: All episodes are being synced with season_number=0 regardless of their actual season. This causes newer season episodes (S2, S4, S5, etc.) to not be found in the database, which breaks home sections like Continue Watching and On Deck. Episodes from shows like Stillwater S4, Resident Alien S4, Peacemaker S2, Slow Horses S5 are missing because the sync process is not properly fetching or storing season information.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Investigate why episodes are stored with season_number=0
- [x] #2 Fix episode sync to properly store season_number from Plex metadata
- [x] #3 Ensure all seasons are fetched during sync, not just season 0
- [x] #4 Re-sync shows to populate correct season numbers
- [x] #5 Verify Continue Watching and On Deck sections show episodes from all seasons
<!-- AC:END -->


## Implementation Plan

1. Investigate episode sync implementation in services/core/sync.rs
2. Check where season_number is being set in Plex and Jellyfin backends
3. Add parent_index field to PlexEpisodeMetadata struct to capture season info
4. Update Plex API to use parent_index for season_number
5. Add logging to warn about season_number=0 issues during sync
6. Test the fix with a fresh sync operation


## Implementation Notes

Fixed the episode sync issue where all episodes were stored with season_number=0.

The issue was in the Plex API types where PlexEpisodeMetadata struct was missing the parent_index field that contains the season number from the Plex API response. 

Changes made:
1. Added parent_index field to PlexEpisodeMetadata struct in src/backends/plex/api/types.rs
2. Updated episode creation in src/backends/plex/api/library.rs to use parent_index for season_number
3. Added logging in src/services/core/sync.rs to warn when episodes have season_number=0
4. Added logging in src/backends/plex/mod.rs to track when season numbers are being overridden

The Plex backend now correctly captures the season number from the API response and properly sets it on episodes during sync.
