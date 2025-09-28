---
id: task-200
title: Fix failing Jellyfin backend tests
status: Done
assignee:
  - '@arosenfeld'
created_date: '2025-09-21 14:01'
updated_date: '2025-09-21 14:12'
labels:
  - testing
  - jellyfin
  - bug
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Two Jellyfin backend tests are failing with timeout issues. These tests need to be fixed to ensure complete test coverage for the Jellyfin backend implementation.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Fix test_media_item_retrieval_shows test timeout issue
- [x] #2 Fix test_streaming_url_generation test timeout issue
- [x] #3 Ensure all 13 Jellyfin backend tests pass consistently
- [x] #4 Investigate root cause of mockito server timeout after 30 seconds
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Analyze test failures - both tests timeout after 30 seconds
2. Check mockito server lifecycle management
3. Investigate mock expectations that may not be met
4. Fix async/await patterns in tests
5. Verify all mocks are being properly matched
6. Run tests with debugging to pinpoint exact failure
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed two failing Jellyfin backend tests that were causing timeouts:

1. test_media_item_retrieval_shows:
   - Issue: total_episode_count was using child_count from series (which is season count) instead of summing episodes from seasons
   - Fix: Calculate total_episode_count by summing episode counts from all seasons
   - Issue: watched_episode_count was using played_count instead of calculating from unplayed_item_count
   - Fix: Added unplayed_item_count field to UserData and calculate watched as (total - unplayed)

2. test_streaming_url_generation:
   - Issue: Missing SupportsDirectPlay field in MediaSource causing deserialization failure
   - Fix: Added #[serde(default)] to make supports_direct_play and supports_direct_stream optional
   - Issue: DirectStreamUrl from mock not being used
   - Fix: Added direct_stream_url field to MediaSource and use it when available
   - Issue: quality_options was always empty
   - Fix: Populated quality_options with standard transcoding options (Original, 1080p, 720p, 480p)

All 13 Jellyfin backend tests now pass successfully. Root cause was incorrect field mapping and missing optional field handling in the API response structures.
<!-- SECTION:NOTES:END -->
