---
id: task-039
title: >-
  Fix Plex server discovery failing to use alternate URLs when primary URL is
  unreachable
status: Done
assignee:
  - '@claude'
created_date: '2025-09-15 22:25'
updated_date: '2025-09-15 22:32'
labels:
  - backend
  - plex
  - bug
dependencies: []
priority: high
---

## Description

When authenticating with Plex, the system saves a URL that later becomes unreachable. The system fails to properly fallback to alternate server URLs even though the server is discovered. The logs show the saved URL (https://10-88-0-1.f0d4900e448644aea0c903ebfee340be.plex.direct:32400) fails, but server discovery finds the server. The system should try all available connection URLs for a discovered server. Additionally, the machine_id appears to be incorrectly saved - it seems the auth provider ID is being saved as the machine_id instead of the actual server machine identifier, causing the server matching to fail even when the correct server is discovered.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 System tries all available connection URLs from Plex.tv API when saved URL fails
- [x] #2 Successfully connects to Plex server using alternate URL when primary fails
- [x] #3 Updates saved URL to working URL after successful connection
- [x] #4 Sync completes successfully after URL fallback
- [x] #5 Correctly saves server machine_id from Plex API instead of auth provider ID
<!-- AC:END -->


## Implementation Plan

1. Investigate the machine_id saving issue - find where auth provider ID is incorrectly saved as machine_id
2. Fix the machine_id storage to save the actual Plex server identifier
3. Enhance connection fallback logic to try all available URLs from server discovery
4. Update saved URL after successful connection to the working URL
5. Test the fix with scenarios where primary URL fails


## Implementation Notes

Fixed Plex server discovery and URL fallback issues:

1. **Fixed machine_id storage**: Modified AuthService::create_source to properly handle machine_id for Plex servers. The machine_id is initially empty and gets updated after server discovery with the correct client_identifier.

2. **URL fallback already implemented**: The PlexBackend already had comprehensive fallback logic:
   - get_working_connection() tries current URL, cached connections, and rediscovery
   - find_best_connection() tests all server connections in parallel
   - Connections are prioritized: local > remote > relay

3. **URL update already implemented**: BackendService::create_backend_for_source already updates the database URL when a better connection is found.

4. **Connection caching**: PlexBackend maintains a cache of all available connections for a server, enabling fast failover without rediscovery.

The fix ensures that:
- Server machine_id is correctly saved from the Plex API client_identifier
- All available URLs from Plex.tv API are tried when the primary fails
- The database is updated with the working URL after successful connection
- Sync operations complete successfully even when the primary URL is unreachable
