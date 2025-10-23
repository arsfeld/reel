---
id: task-442
title: >-
  Fix TV shows failing to load episodes from database despite having episode
  counts in metadata
status: In Progress
assignee: []
created_date: '2025-10-23 00:52'
updated_date: '2025-10-23 01:14'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Some TV shows like "What If…?" display correct season and episode counts in their metadata (e.g., 3 seasons with 26 total episodes) but fail to load any episodes when querying the database. The logs show "Loaded 0 episodes from database" for all seasons even though the show metadata indicates episodes exist. This suggests either episodes are not being synced to the database properly, there's a mismatch between season numbering during sync vs display, or the episode loading query has incorrect filtering logic.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 TV shows with episode counts in metadata successfully load episodes from database
- [ ] #2 Season selection displays correct episodes for each season
- [ ] #3 Episode counts in metadata match actual episodes loaded from database
- [ ] #4 Episode sync process correctly stores episodes with proper season numbering
- [ ] #5 Episode query logic correctly matches season numbers between sync and display
- [ ] #6 Shows like 'What If…?' display all their episodes correctly
- [ ] #7 Debug logging identifies root cause of episode loading failure
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Investigation Plan

1. ✅ Trace episode loading path from UI to database
   - `show_details.rs` loads episodes via `GetEpisodesCommand`
   - Command calls `MediaService::get_episodes_for_show`
   - Service calls repository `find_episodes_by_season`

2. ✅ Examine database query logic
   - Query filters by: `MediaType='episode'`, `ParentId=show_id`, `SeasonNumber=season_number`
   - Query looks correct

3. ✅ Check episode sync logic
   - Episodes synced via `sync_show_episodes_with_progress` in sync.rs
   - Calls `backend.get_episodes(show_id, season_number)`
   - Plex backend sets `episode.show_id = Some(show_id.to_string())`

4. ✅ Verify ID mapping
   - Episode model: `show_id: Option<String>` maps to database `parent_id` column
   - Show model: `id: String` is used to query episodes
   - Need to verify if show.id matches episode.show_id values

5. 🔄 Add debug logging (COMPLETED)
   - Added comprehensive logging to `find_episodes_by_season` method
   - Logs show_id being queried, all episodes found for show, and filtered results
   - Need to run app and observe logs to identify mismatch

6. ⏳ Identify root cause
   - Run application and navigate to a TV show with episodes
   - Check debug logs for ID format mismatches
   - Likely issue: show_id format doesn't match parent_id in episodes

7. ⏳ Implement fix based on findings
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Root Cause Analysis

Found the critical bug in `src/services/core/sync.rs:287`:

```rust
let shows = MediaService::get_media_items(db, &library.id.clone().into(), None, 0, 1000).await?;
```

The `None` parameter means it fetches ALL media types (shows, episodes, movies), not just shows. With ~1000 items in the library (including movies and episodes already synced), only the first ~154 shows were included in the result set. The remaining 12 shows (alphabetically: Twisted Metal, Undead Unluck, Undone, Utopia, Vinland Saga, Virgin River, War of the Worlds, We Were Liars, Westworld, What If…?, X-Men '97, You & Me) came after item #1000 and never got processed for episode syncing.

## The Fix

Changed line 287 to explicitly filter by MediaType::Show:

```rust
let shows = MediaService::get_media_items(db, &library.id.clone().into(), Some(crate::models::MediaType::Show), 0, 10000).await?;
```

This ensures:
1. Only shows are fetched for episode syncing
2. Increased limit to 10000 to handle large libraries
3. All shows will have their episodes synced regardless of database ordering

## Additional Fixes

Also fixed episode parent_id format issues in both Plex and Jellyfin backends:
- `src/backends/plex/mod.rs:1291`: Changed conditional assignment to always override `episode.show_id` with the composite database ID
- `src/backends/jellyfin/mod.rs:492`: Same fix for Jellyfin backend

These ensure episodes get the correct composite parent_id format (e.g., "source_id:library_id:show:rating_key") to match the show's database ID.

## Testing Required

1. Kill current Reel instance
2. Start new instance with fix: `nix develop -c cargo run`
3. Trigger full library sync
4. Verify all 166 shows get episode syncing (log should say "Found 166 shows" not "Found 1000")
5. Navigate to "You & Me (2023)" and verify episodes appear
6. Check other previously broken shows: What If…?, X-Men '97, Westworld, etc.
<!-- SECTION:NOTES:END -->
