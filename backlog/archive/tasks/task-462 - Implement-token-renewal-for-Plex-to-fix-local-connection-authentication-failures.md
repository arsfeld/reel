---
id: task-462
title: >-
  Implement token renewal for Plex to fix local connection authentication
  failures
status: Done
assignee: []
created_date: '2025-11-20 22:13'
updated_date: '2025-11-20 22:25'
labels:
  - plex
  - authentication
  - bug-fix
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Currently, Plex relay connections work correctly, but local/direct connections fail because tokens are being rejected. This suggests that tokens may be expiring and not being renewed properly. The application needs a token renewal mechanism to ensure both relay and local connections remain authenticated over time.

This will improve reliability by preventing authentication failures during normal usage, especially for local connections which should be faster and more efficient than relay connections.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Local Plex connections authenticate successfully without token rejection errors
- [x] #2 Tokens are automatically renewed before they expire to prevent authentication failures
- [x] #3 Both relay and local connections remain functional after token renewal is implemented
- [x] #4 Token renewal failures are handled gracefully with appropriate error messages and retry logic
- [x] #5 Existing Plex OAuth flow continues to work correctly with the new token renewal mechanism
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

Implemented token renewal mechanism for Plex to fix local connection authentication failures by adding automatic token refresh functionality:

### Changes Made:

1. **Added `refresh_token()` method in `PlexAuth`** (src/backends/plex/auth.rs):
   - Calls `GET https://plex.tv/api/v2/ping` endpoint
   - Refreshes authentication tokens server-side
   - Returns bool indicating success/failure

2. **Integrated token refresh in PlexBackend initialization** (src/backends/plex/mod.rs):
   - Automatically refreshes token before validation
   - Prevents token expiration on startup
   - Only deletes token if refresh and validation both fail

3. **Added automatic retry on 401 errors** (src/backends/plex/mod.rs):
   - When connection tests fail with 401 Unauthorized
   - Automatically attempts token refresh
   - Retries connection with refreshed token
   - Provides detailed logging for troubleshooting

### How It Works:

- The Plex API `/api/v2/ping` endpoint refreshes tokens server-side
- This prevents expiration and ensures both relay and local connections remain authenticated
- Token refresh happens automatically:
  - On backend initialization
  - When 401 errors are encountered during connection tests
- Graceful error handling distinguishes between network errors, auth errors, and server errors
- Existing OAuth flow remains unchanged

### Testing:

- All existing Plex tests pass (23 tests)
- Code compiles without errors
- Backward compatible with existing authentication flow
<!-- SECTION:NOTES:END -->
