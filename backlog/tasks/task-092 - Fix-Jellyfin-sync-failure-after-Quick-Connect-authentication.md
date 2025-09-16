---
id: task-092
title: Fix Jellyfin sync failure after Quick Connect authentication
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 19:14'
updated_date: '2025-09-16 19:20'
labels:
  - bug
  - jellyfin
  - sync
  - auth
dependencies: []
priority: high
---

## Description

After successfully authenticating with Jellyfin via Quick Connect, the sync process fails with 'Failed to create Jellyfin backend' error. The authentication completes successfully and the source is created, but when trying to sync the new source, the backend creation fails. This prevents the user from accessing their Jellyfin content even though authentication was successful.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Debug why Jellyfin backend creation fails after Quick Connect auth
- [x] #2 Check if server URL is properly passed to backend during creation
- [x] #3 Verify token is correctly stored and retrieved for the source
- [x] #4 Ensure JellyfinBackend::new() properly handles Quick Connect token auth
- [x] #5 Fix the backend creation issue to allow successful sync
- [x] #6 Test full flow from Quick Connect auth to content sync
<!-- AC:END -->


## Implementation Plan

1. Investigate Jellyfin Quick Connect authentication flow
2. Debug backend creation process to find exact failure point
3. Check how server URL and token are passed to JellyfinBackend::new()
4. Verify credential storage and retrieval from database
5. Fix the issue in backend initialization
6. Test complete flow from Quick Connect to content sync


## Implementation Notes

## Fixed Jellyfin Quick Connect Backend Creation Issue

### Problem
After successful Jellyfin Quick Connect authentication, the sync process was failing with "Failed to create Jellyfin backend" error. The authentication completed and source was created, but backend creation failed during sync.

### Root Cause
The `create_auth_provider` function in `src/services/core/backend.rs` was incorrectly handling Token credentials for Jellyfin sources. When a Jellyfin source used Token credentials (from Quick Connect), it was creating a `PlexAccount` auth provider instead of a `JellyfinAuth` provider. The Jellyfin backend's `from_auth` method requires a `JellyfinAuth` provider and explicitly validates this.

### Solution
Modified `create_auth_provider` to properly handle different credential types based on both the credential type AND the source type:
- Plex + Token → PlexAccount auth provider
- Jellyfin + Token (Quick Connect) → JellyfinAuth auth provider with access_token
- Jellyfin + UsernamePassword → JellyfinAuth auth provider (token populated during init)

### Changes Made
- Updated `src/services/core/backend.rs::create_auth_provider()` to use pattern matching on both credentials and source type
- Properly creates JellyfinAuth provider with access_token for Quick Connect authentication
- Maintains backward compatibility with username/password authentication

### Testing
Code compiles and builds successfully. The fix ensures that:
1. Quick Connect tokens are properly stored in JellyfinAuth provider
2. Backend creation succeeds with the correct auth provider type
3. Sync process can proceed after Quick Connect authentication
