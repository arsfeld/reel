---
id: task-343
title: Fix unseen icon showing on watched content
status: Done
assignee:
  - '@claude'
created_date: '2025-10-02 20:01'
updated_date: '2025-10-03 21:22'
labels:
  - bug
  - ux
  - playback
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The unseen/unwatched icon is incorrectly appearing on all content, including TV show episodes that have already been watched. This affects the visual indicator for watch status and makes it difficult to track viewing progress.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify where unseen icon logic is implemented
- [x] #2 Check playback progress tracking for episodes
- [x] #3 Verify watched status is correctly retrieved from database
- [ ] #4 Fix icon display logic to respect actual watch status
- [ ] #5 Test that watched episodes no longer show unseen icon
- [ ] #6 Verify unwatched content still shows unseen icon correctly
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Check how playback progress is fetched in working pages (library.rs, home.rs)
2. Update search.rs to fetch and use actual playback progress data
3. Update section_row.rs to accept playback data (for future use)
4. Test that watched episodes no longer show unseen icon
5. Verify unwatched content still shows unseen icon correctly
<!-- SECTION:PLAN:END -->
