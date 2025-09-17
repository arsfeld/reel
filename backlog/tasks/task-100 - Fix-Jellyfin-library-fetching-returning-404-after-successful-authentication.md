---
id: task-100
title: Fix Jellyfin library fetching returning 404 after successful authentication
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 19:34'
updated_date: '2025-09-17 03:38'
labels:
  - bug
  - jellyfin
  - api
dependencies: []
priority: high
---

## Description

After successfully authenticating with Jellyfin (including Quick Connect), the server connects successfully but library fetching fails with 404 Not Found. The authentication works and connects to the server, but the API call to get libraries fails. Need to investigate why the user_id might be empty or incorrect in the API calls.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Debug and identify why user_id is empty or incorrect in library API calls
- [x] #2 Verify user_id is properly extracted and stored from Quick Connect auth response
- [x] #3 Ensure user_id is correctly passed through the backend initialization chain
- [x] #4 Fix the user_id propagation issue in Jellyfin backend
- [x] #5 Test library fetching works after Quick Connect authentication
<!-- AC:END -->


## Implementation Plan

1. Investigate how Quick Connect token with user_id is saved to keyring
2. Check if user_id is properly extracted when loading from keyring
3. Fix token format saving to include user_id when saving to keyring
4. Ensure user_id is propagated through backend initialization
5. Test Quick Connect auth flow end-to-end


## Implementation Notes

Fixed the Jellyfin library fetching 404 error after Quick Connect authentication.

The issue was that when loading credentials from the keyring, the user_id was not being properly extracted from the token format. Quick Connect saves the token in format "token|user_id" to the keyring, but when loading it back, the code was not parsing this format to extract the user_id.

The fix parses the token when loading from keyring to extract both the actual token and user_id, ensuring the user_id is properly propagated to the JellyfinApi for library API calls.

Modified files:
- src/backends/jellyfin/mod.rs: Added token parsing logic when loading from keyring
