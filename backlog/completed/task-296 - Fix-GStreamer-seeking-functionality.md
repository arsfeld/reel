---
id: task-296
title: Fix GStreamer seeking functionality
status: Done
assignee:
  - '@claude'
created_date: '2025-09-28 02:07'
updated_date: '2025-10-02 14:51'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
GStreamer player backend currently doesn't allow seeking during playback. This is a critical functionality for a media player and needs to be fixed, especially since GStreamer will be the only backend on macOS.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Investigate why seeking is not working in the GStreamer implementation
- [ ] #2 Check if the pipeline state needs to be PLAYING or PAUSED for seeking to work
- [ ] #3 Verify that the seek flags and seek type are correctly configured
- [ ] #4 Ensure the playbin element supports seeking for the current media format
- [ ] #5 Add proper error handling and logging for seek operations
- [ ] #6 Test seeking with different media formats (MP4, MKV, etc.)
- [ ] #7 Verify seeking works with both keyboard shortcuts and UI controls
- [ ] #8 Add unit tests for GStreamer seeking functionality
<!-- AC:END -->


## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Investigate GStreamer seek implementation and identify state requirements
2. Check if pipeline needs to be in PAUSED or PLAYING state before seeking
3. Verify seek flags and seek type configuration
4. Check playbin3-specific seeking requirements
5. Add proper error handling and logging
6. Test with different media formats
7. Verify UI integration with keyboard shortcuts and controls
<!-- SECTION:PLAN:END -->
