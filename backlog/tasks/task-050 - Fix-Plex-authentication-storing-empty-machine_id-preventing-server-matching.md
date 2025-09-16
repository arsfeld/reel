---
id: task-050
title: Fix Plex authentication storing empty machine_id preventing server matching
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 02:39'
updated_date: '2025-09-16 02:57'
labels:
  - backend
  - plex
  - bug
  - auth
dependencies: []
priority: high
---

## Description

When authenticating with Plex, the system stores an empty machine_id value, which causes server matching to fail even when the correct server is discovered. The logs show 'Looking for server with machine_id:' (empty) and then fails to match the discovered server. The machine_id should be properly extracted and stored during authentication to enable successful server connection.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Extract correct machine_id from Plex server during authentication
- [x] #2 Store machine_id properly in the database when creating Plex source
- [x] #3 Successfully match discovered server using stored machine_id
- [x] #4 Connection succeeds after authentication without manual intervention
- [x] #5 Sync completes successfully after fixing machine_id storage
<!-- AC:END -->


## Implementation Plan

1. Identify where machine_id is missing during manual authentication
2. For manual auth, fetch server info to get machine_id
3. Update auth dialog to fetch and store machine_id for manual connections
4. Test that server matching works after fix


## Implementation Notes

Implemented fix for missing machine_id during Plex authentication:

1. Added PlexIdentityResponse structure to parse /identity endpoint response
2. Created get_machine_id() method in PlexApi to fetch machine identifier from server
3. Updated auth dialog to handle case when best_server is None (manual auth or edge cases)
4. When best_server is None, now fetches machine_id directly from server using /identity endpoint
5. Added logging to track when machine_id is set or missing

The fix ensures machine_id is always fetched and stored, either from discovered servers (OAuth flow) or directly from the server (manual auth flow).

Final implementation:
- Modified CreateSourceCommand to accept machine_id and is_owned parameters
- Updated AuthService::create_source to set machine_id during source creation
- For OAuth: machine_id extracted from discovered PlexServer
- For manual auth: machine_id fetched via /identity endpoint
- Source now created with machine_id from the start, avoiding race condition with sync
