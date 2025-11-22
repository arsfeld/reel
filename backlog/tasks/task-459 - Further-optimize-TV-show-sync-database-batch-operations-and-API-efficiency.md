---
id: task-459
title: Further optimize TV show sync - database batch operations and API efficiency
status: To Do
assignee: []
created_date: '2025-10-23 14:09'
updated_date: '2025-10-23 14:12'
labels:
  - performance
  - sync
  - database
  - optimization
dependencies:
  - task-456
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Task-456 successfully implemented parallelization optimizations that improved TV show episode sync performance by 6-12x. However, the biggest remaining issue is that **every sync re-processes all content**, even though most TV shows and episodes haven't changed since the last sync.

**Current Status After Task-456:**
- Parallelized show processing (5 concurrent shows)
- Parallelized season episode fetching (all seasons per show)
- Eliminated redundant get_seasons() API calls
- Estimated improvement: 191s → 15-30s for 100-show library

**Core Problem: No Incremental Sync Strategy**

After the initial sync, subsequent syncs should be nearly instantaneous because:
- Most shows haven't added new episodes
- Existing episode metadata rarely changes
- Watch status is tracked separately via playback progress API
- Only new episodes or metadata updates need to be synced

**Current Behavior:**
- Every sync fetches all seasons for all shows
- Every sync fetches all episodes for all seasons
- Every sync re-inserts or updates all episodes in database
- A library with 100 shows and 5,000 episodes re-syncs all 5,000 episodes every time

**Impact:**
- Initial sync: 15-30 seconds (acceptable)
- Subsequent syncs: Still 15-30 seconds (unacceptable - should be near-instant)
- User waits the same amount of time every sync
- Unnecessary load on Plex/Jellyfin servers
- Wasted bandwidth and battery on mobile devices

**Proposed Solution: Implement Incremental Sync**

1. **Track Last Modified Timestamps**
   - Both Plex and Jellyfin APIs provide `updatedAt` timestamps
   - Store last sync timestamp in `sync_status` table
   - Only fetch shows/episodes modified since last sync

2. **Skip Unchanged Shows**
   - Compare show `updatedAt` with last sync timestamp
   - If show unchanged, skip fetching seasons and episodes
   - Reduces API calls by 90%+ after initial sync

3. **Smart Episode Sync Strategy**
   - Only fetch episodes if:
     - Show has new `updatedAt` timestamp (new episodes added)
     - Show's `leaf_count` changed (episode count changed)
     - First sync for this show
   - Skip episode fetch entirely for unchanged shows

4. **Fallback to Full Sync**
   - Allow manual "force full sync" for troubleshooting
   - Auto-trigger full sync if incremental sync fails
   - Full sync if last sync was >7 days ago (safety net)

**Backend API Support:**

Both Plex and Jellyfin support incremental queries:

**Plex:**
```
/library/sections/{id}/all?updatedAt>={timestamp}
/library/metadata/{show_id}  (includes updatedAt)
```

**Jellyfin:**
```
/Users/{userId}/Items?ParentId={id}&MinDateLastSaved={timestamp}
/Shows/{id}  (includes DateLastSaved)
```

**Secondary Optimizations (Lower Priority):**

These can be addressed after incremental sync is working:

1. **True Bulk Database Operations**
   - Use SeaORM's `insert_many()` instead of individual inserts
   - Reduces database time from seconds to milliseconds

2. **Batch Episode Lookups**
   - Fetch all existing episodes upfront, build HashMap
   - Eliminates N+1 query pattern

3. **Configurable Concurrency**
   - Make CONCURRENT_SHOWS tunable
   - Add CONCURRENT_SEASONS limit

**Success Metrics:**
- **Primary**: Subsequent syncs complete in under 2 seconds for unchanged libraries
- **Primary**: Subsequent syncs make <10 API calls for unchanged libraries
- Initial sync maintains current 15-30s performance
- Adding 1 new episode to a 100-show library syncs in <3 seconds
- Database operations reduced by 95%+ for incremental syncs
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Implement timestamp tracking in sync_status table (last_full_sync, last_incremental_sync)
- [ ] #2 Query Plex/Jellyfin APIs with updatedAt/MinDateLastSaved filters to fetch only changed content
- [ ] #3 Skip episode sync for shows where updatedAt and leaf_count are unchanged
- [ ] #4 Measure and document incremental sync performance (target: <2s for unchanged libraries)
- [ ] #5 Subsequent syncs make fewer than 10 API calls when no content has changed
- [ ] #6 Adding a single new episode syncs in under 3 seconds
- [ ] #7 Implement force full sync option for troubleshooting
- [ ] #8 Auto-trigger full sync if last full sync was >7 days ago or if incremental fails
<!-- AC:END -->
