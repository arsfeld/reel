---
id: task-100
title: Fix Jellyfin library fetching returning 404 after successful authentication
status: In Progress
assignee:
  - '@claude'
created_date: '2025-09-16 19:34'
updated_date: '2025-09-17 03:32'
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
- [ ] #1 Debug and identify why user_id is empty or incorrect in library API calls
- [ ] #2 Verify user_id is properly extracted and stored from Quick Connect auth response
- [ ] #3 Ensure user_id is correctly passed through the backend initialization chain
- [ ] #4 Fix the user_id propagation issue in Jellyfin backend
- [ ] #5 Test library fetching works after Quick Connect authentication
<!-- AC:END -->

## Implementation Plan

1. Investigate how Quick Connect token with user_id is saved to keyring
2. Check if user_id is properly extracted when loading from keyring
3. Fix token format saving to include user_id when saving to keyring
4. Ensure user_id is propagated through backend initialization
5. Test Quick Connect auth flow end-to-end
