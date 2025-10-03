---
id: task-209
title: 'Phase 1: Plex decision endpoint and connection type detection'
status: Done
assignee:
  - '@claude'
created_date: '2025-09-22 14:19'
updated_date: '2025-10-03 23:08'
labels:
  - backend
  - plex
  - api
  - transcoding
  - phase-1
dependencies:
  - task-206
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Phase 1 of Plex transcoding integration: Implement decision endpoint and integrate with existing ConnectionService for connection type detection. This enables remote Plex playback and lays the foundation for quality selection and adaptive streaming. Leverages the existing ConnectionService/ConnectionCache infrastructure to avoid duplicate connection tracking. See docs/transcode-plan.md for complete implementation plan (8 phases total).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create src/backends/plex/api/decision.rs with DecisionResponse types
- [x] #2 Implement get_stream_url_via_decision() method
- [x] #3 Decision endpoint handles both direct play and transcode modes
- [x] #4 Connection location (lan/wan) correctly passed to decision API
- [x] #5 Add unit tests for decision endpoint request/response parsing
- [x] #6 Files created/updated as per docs/transcode-plan.md Phase 1
- [x] #7 Create task-209.2 for Phase 2 implementation (see docs/transcode-plan.md)

- [x] #8 PlexBackend implements is_local_connection() that queries ConnectionService::cache()
- [x] #9 PlexBackend implements get_connection_location() helper method returning lan/wan
- [x] #10 No duplicate connection tracking logic added (uses existing ConnectionService)
- [x] #11 Connection type correctly determined from ConnectionCache for all operations
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Review existing ConnectionService and ConnectionCache infrastructure
2. Add is_local_connection() to PlexBackend that queries ConnectionService::cache()
3. Add get_connection_location() helper returning "lan" or "wan"
4. Create src/backends/plex/api/decision.rs with decision endpoint types
5. Implement get_stream_url_via_decision() in PlexApi
6. Add decision.rs to api/mod.rs exports
7. Write unit tests for decision endpoint request/response parsing
8. Verify no duplicate connection tracking is introduced
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Phase 1 implementation complete. Added Plex decision endpoint support and connection type detection.

## Files Created
- src/backends/plex/api/decision.rs: Decision endpoint types and implementation

## Files Modified
- src/backends/plex/mod.rs: Added is_local_connection() and get_connection_location() methods
- src/backends/plex/api/mod.rs: Added decision module

## Implementation Details
- PlexBackend.is_local_connection(): Queries ConnectionService::cache() to determine connection type
- PlexBackend.get_connection_location(): Returns "lan" or "wan" for decision endpoint
- PlexApi.get_stream_url_via_decision(): Calls Plex decision endpoint with quality parameters
- Decision endpoint supports both direct play (directPlay=1) and transcode modes (directPlay=0)
- Connection location (lan/wan) correctly passed based on ConnectionCache state
- No duplicate connection tracking - uses existing ConnectionService infrastructure
- Added unit tests for decision response parsing (direct play and transcode modes)

## Next Steps
Phase 2 (task-389): Enhance get_stream_url() to return quality options and implement get_stream_url_for_quality().
<!-- SECTION:NOTES:END -->
