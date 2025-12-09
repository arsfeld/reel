---
id: task-472
title: Implement Hybrid Cache Architecture for Media Metadata
status: To Do
assignee: []
created_date: '2025-12-09 18:50'
updated_date: '2025-12-09 18:51'
labels:
  - architecture
  - performance
  - sync
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Replace the current full-sync approach with a hybrid cache architecture that:
- **Keeps playback progress bidirectional sync** (already implemented via `playback_sync_queue`)
- **Treats media metadata as a TTL-based cache** (not synced, just cached with expiry)
- **Lazy-loads metadata on navigation** with cache fallback for offline
- **Background refreshes only visible/active libraries**

## Current Problems
1. Full sync fetches ALL metadata even if unchanged
2. No TTL on cached metadata - always re-fetches everything
3. Sync blocks on completion - UI waits for full sync
4. `SyncStrategy` config is defined but never used
5. Failed syncs block retries for 1 hour due to throttle logic

## Target Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    DATA FLOW                                 │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  PLAYBACK PROGRESS (Bidirectional - Keep As-Is)             │
│  ─────────────────────────────────────────────              │
│  Local Change → playback_sync_queue → Worker → Backend      │
│  (Conflict resolution via ConflictResolverContext)          │
│                                                              │
│  MEDIA METADATA (TTL Cache - New Approach)                  │
│  ─────────────────────────────────────────                  │
│  1. Page loads → Check DB cache                             │
│  2. If fresh (TTL ok) → Serve immediately                   │
│  3. If stale → Serve stale + queue background refresh       │
│  4. If missing → Show loading + fetch from backend          │
│  5. Background worker refreshes active libraries only       │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Key Design Decisions

1. **TTL Duration**: Configurable per content type
   - Libraries list: 1 hour
   - Media items (movies/shows): 4 hours  
   - Episodes: 12 hours
   - Full metadata (cast/crew): 24 hours

2. **Stale-While-Revalidate**: Always serve cached data immediately, refresh in background

3. **Active Library Tracking**: Only refresh libraries user is currently viewing

4. **Offline Fallback**: Serve any cached data when backend unreachable

## Success Criteria
- [ ] Sync only fetches metadata for active/visible libraries
- [ ] UI loads instantly from cache, never blocks on network
- [ ] Stale metadata is served while fresh data loads in background
- [ ] Playback progress continues to sync bidirectionally
- [ ] TTL is configurable per content type
- [ ] Failed refreshes don't block subsequent refresh attempts
<!-- SECTION:DESCRIPTION:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Order

Execute subtasks in this order for incremental, testable progress:

### Phase 1: Foundation (Required First)
1. **task-472.01** - Add `fetched_at` timestamp to media_items table
2. **task-472.02** - Create CacheConfig with TTL settings

### Phase 2: Core Services
3. **task-472.03** - Implement MetadataRefreshService for TTL-based refresh

### Phase 3: Worker Refactoring
4. **task-472.04** - Add active library tracking via MessageBroker
5. **task-472.05** - Refactor SyncWorker to use TTL-based refresh

### Phase 4: UI Integration
6. **task-472.06** - Implement stale-while-revalidate pattern in pages
7. **task-472.07** - Add refresh indicators to UI (optional polish)
8. **task-472.08** - Add manual refresh action (optional feature)

### Phase 5: Cleanup
9. **task-472.09** - Clean up unused sync code

## Key Architectural Notes

- **Playback progress sync is PRESERVED** - The existing `playback_sync_queue` + `PlaybackSyncWorker` + conflict resolution system is already excellent and should not be changed
- Each phase can be merged independently and tested
- Phase 4 UI changes can be done incrementally per page
- Phase 5 cleanup should only happen after everything else is verified working
<!-- SECTION:PLAN:END -->
