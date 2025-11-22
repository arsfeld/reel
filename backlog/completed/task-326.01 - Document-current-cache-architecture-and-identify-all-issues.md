---
id: task-326.01
title: Document current cache architecture and identify all issues
status: Done
assignee:
  - '@claude'
created_date: '2025-10-01 15:40'
updated_date: '2025-10-01 15:55'
labels:
  - cache
  - documentation
dependencies: []
parent_task_id: task-326
---

## Description

Create comprehensive documentation of:

1. Current component interactions (downloader, proxy, state machine, storage)
2. Data flow for initial playback vs seeks
3. What database tables/columns exist vs what's actually used\n4. All identified issues with current approach\n5. Edge cases that cause problems (seeks during download, sparse files, etc.)\n\n**Deliverable**: CACHE_ARCHITECTURE.md documenting current state and all identified issues

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Document component interactions and data flows
- [x] #2 List all database schema elements and their actual usage
- [x] #3 Identify all edge cases and failure modes
- [x] #4 Create CACHE_ARCHITECTURE.md with findings
<!-- AC:END -->


## Implementation Plan

1. Read and analyze all cache-related source files (downloader, proxy, state_machine, file_cache, metadata)
2. Examine database schema for cache tables (entities and migrations)
3. Trace data flow for key scenarios (initial playback, seeks, resume)
4. Document component interactions and data flows
5. Document database schema vs actual usage
6. Identify all edge cases and issues
7. Create CACHE_ARCHITECTURE.md with comprehensive findings


## Implementation Notes

Created comprehensive CACHE_ARCHITECTURE.md document with:

1. Complete component architecture documentation (5 main components)
2. Detailed data flow analysis for 4 key scenarios including happy path and failure cases
3. Full database schema documentation with usage analysis
4. 6 critical issues identified with code evidence
5. 8 edge cases and failure modes documented
6. File reference index for easy navigation

Key findings:
- cache_chunks table exists but is NEVER used (repository methods exist but no callers)
- Downloader only supports sequential downloads (cannot prioritize ranges)
- Proxy guesses availability by checking file size instead of querying database
- State machine keeps everything in memory instead of database-driven
- No range prioritization mechanism exists

Document is ready for task-326.02 (design) to reference.
