---
id: task-018
title: Fix AuthProvider creation in Relm4 auth dialog
status: Done
assignee:
  - '@claude'
created_date: '2025-09-15 02:34'
updated_date: '2025-09-15 15:56'
labels:
  - relm4
  - auth
  - backend
dependencies: []
priority: high
---

## Description

The auth dialog sets auth_provider_id to None because AuthProvider creation is not implemented before setting the field

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 AuthProvider is created and saved before setting auth_provider_id,Source creation includes valid auth_provider_id reference,Authentication flow properly links source to auth provider
<!-- AC:END -->


## Implementation Plan

1. Review how Jellyfin handles auth_provider_id creation in auth_dialog.rs\n2. Understand the AuthProvider model and its ID generation requirements\n3. Generate a unique auth_provider_id for Plex sources before creating the SourceModel\n4. Update the Plex authentication flow to properly set auth_provider_id\n5. Test the fix to ensure authentication works correctly


## Implementation Notes

Fixed AuthProvider creation in Plex authentication flow by:
1. Refactored Plex authentication to use CreateSourceCommand instead of manually creating SourceModel
2. Added new_for_auth() method to PlexBackend for temporary authentication purposes
3. Updated auth flow to properly set auth_provider_id through CreateSourceCommand which calls AuthService::create_source()
4. After source creation, added logic to update Plex-specific metadata (machine_id, connections)

The fix ensures that auth_provider_id is properly set when creating Plex sources, matching the pattern used by Jellyfin authentication.
