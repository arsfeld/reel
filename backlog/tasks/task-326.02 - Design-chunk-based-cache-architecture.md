---
id: task-326.02
title: Design chunk-based cache architecture
status: Done
assignee:
  - '@claude'
created_date: '2025-10-01 15:40'
updated_date: '2025-10-01 16:01'
labels:
  - cache
  - design
  - architecture
dependencies: []
parent_task_id: task-326
---

## Description

Design the new architecture where:

**Core Principles**:
1. Database is single source of truth for what chunks are available
2. Downloader works on chunk queues with priorities
3. Proxy queries database to check chunk availability
4. State derived from database, not tracked separately

**Key Components**:
- **ChunkManager**: Coordinates chunk requests, priorities, and availability
- **ChunkDownloader**: Downloads specific byte ranges, records in DB
- **ChunkStore**: Manages physical chunk storage (files/dirs)
- **Proxy**: Queries chunks, serves or returns 503

**Deliverable**: CACHE_DESIGN.md with detailed architecture, API contracts, and migration plan

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Define ChunkManager responsibilities and API
- [x] #2 Define ChunkDownloader responsibilities and API
- [x] #3 Define ChunkStore responsibilities and API
- [x] #4 Define Proxy query and serving logic
- [x] #5 Design chunk prioritization algorithm
- [x] #6 Design database schema changes (if needed)
- [x] #7 Create migration plan from current to new architecture
- [x] #8 Create CACHE_DESIGN.md with complete design
- [x] #9 Design explicitly addresses full file streaming use case
- [x] #10 Design specifies progressive streaming with chunk waiting
<!-- AC:END -->


## Implementation Plan

1. Review CACHE_ARCHITECTURE.md to understand current issues
2. Design ChunkManager API (coordination, priorities, availability)
3. Design ChunkDownloader API (range downloads, DB recording)
4. Design ChunkStore API (physical storage management)
5. Design Proxy query/serving logic (DB queries, chunk waiting)
6. Design chunk prioritization algorithm
7. Finalize database schema changes (leverage existing cache_chunks table)
8. Create migration plan from current to new architecture
9. Write CACHE_DESIGN.md with all components
10. Verify design addresses full file streaming use case


## Implementation Notes

Created comprehensive CACHE_DESIGN.md document that addresses all critical issues from CACHE_ARCHITECTURE.md.


## What Was Designed

### Core Components
1. **ChunkManager**: Central coordinator with priority queue, database queries, event-based waiting
2. **ChunkDownloader**: Downloads specific 2MB byte ranges, records in cache_chunks table
3. **ChunkStore**: Physical storage using single sparse file per entry
4. **Proxy (refactored)**: Queries database for chunk availability, waits transparently for missing chunks

### Key Features
- **Database-driven**: cache_chunks table is single source of truth (table already existed but was unused)
- **Priority system**: CRITICAL (current playback) → HIGH (lookahead) → MEDIUM (pre-cache) → LOW (background fill)
- **Event-based waiting**: No polling, uses tokio::sync::Notify
- **Full file streaming**: Handles 200 OK responses with transparent chunk waiting (critical use case)
- **Concurrent downloads**: 3 simultaneous ranges with intelligent prioritization

### Schema Changes
None! Existing cache_chunks table from m20250929_000001 is perfect. Just needs to be used.

## What It Solves

✅ Sequential downloads only → Chunk-based with priorities
✅ Proxy guessing availability → Database queries with has_byte_range()
✅ In-memory state lost on restart → State derived from database
✅ No range prioritization → 4-level priority queue with dynamic re-prioritization
✅ Unused database tables → cache_chunks becomes core of system

## Migration Strategy

6-phase incremental migration:
1. Create new components (non-breaking)
2. Start recording chunks (populate DB)
3. Refactor proxy (improved behavior)
4. Enable chunk downloads (full feature)
5. Cleanup old code
6. Monitor and optimize

Low-to-medium risk with clear rollback plan.

## Documentation

CACHE_DESIGN.md includes:
- Complete API specifications for all components
- Detailed data flow for 4 key use cases
- Database schema analysis (no changes needed)
- Chunk prioritization algorithm
- Phase-by-phase migration plan
- Configuration parameters
- Risk mitigation strategies
