---
id: task-401
title: Fix full integration flow test failures - mock endpoint configuration
status: Done
assignee:
  - '@assistant'
created_date: '2025-10-05 20:38'
updated_date: '2025-10-05 20:58'
labels:
  - testing
  - integration-tests
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Plex and Jellyfin full integration flow tests are failing with 501 Not Implemented errors. The issue is that the mock server endpoints for progress updates need proper query parameter matching. The backend update_progress calls are working correctly but the mock endpoints in the tests need to match the actual API call parameters.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Plex full integration test passes successfully
- [x] #2 Jellyfin full integration test passes successfully
- [x] #3 Mock endpoints match actual backend API call parameters
- [x] #4 All 6 integration tests pass without errors
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Review test fixture setup and foreign key chain
2. Add debug output to see what values are being inserted
3. Fix foreign key constraint issue
4. Verify all tests pass
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed two critical issues causing the integration tests to fail:

## Issue 1: MessageBroker incompatibility in tests
- The MediaRepository::insert() method broadcasts to MessageBroker after insert
- Tests don't have MessageBroker initialized, causing failures
- **Solution**: Use insert_silent() in tests to bypass MessageBroker

## Issue 2: Foreign key constraint on playback_progress
- playback_progress.media_id has FK to media_items.id
- Tests were using prefixed IDs like "test_plex:movie-1" for playback
- Media items stored with unprefixed IDs like "movie-1" from backend
- **Solution**: Use actual inserted_movie.id instead of hardcoded prefixed IDs

## Changes made:
1. tests/integration/plex/auth_and_sync.rs:
   - Changed from insert() to insert_silent()
   - Captured inserted_movie and used its .id for playback operations
   
2. tests/integration/jellyfin/auth_and_sync.rs:
   - Same fixes as Plex test
   - Use insert_silent() and actual movie.id

All 6 integration tests now pass successfully.
<!-- SECTION:NOTES:END -->
