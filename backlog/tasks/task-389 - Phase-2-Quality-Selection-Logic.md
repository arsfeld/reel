---
id: task-389
title: 'Phase 2: Quality Selection Logic'
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 23:07'
updated_date: '2025-10-03 23:11'
labels:
  - backend
  - plex
  - api
  - transcoding
  - phase-2
dependencies:
  - task-209
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Enhance stream URL generation to support quality options and decision endpoint routing. Part of Plex transcoding integration (Phase 2 of 8). See docs/transcode-plan.md for complete implementation plan.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Enhance get_stream_url() to return quality options in StreamInfo
- [x] #2 Implement get_stream_url_for_quality() method
- [x] #3 Add logic to choose direct URL vs decision endpoint based on quality
- [x] #4 Update StreamInfo model if needed to store quality options
- [x] #5 Quality options include original + transcoded variants
- [x] #6 get_stream_url_for_quality() generates correct URLs
- [x] #7 Direct play uses direct URLs, transcoded uses decision endpoint
- [x] #8 Files created/updated as per docs/transcode-plan.md Phase 2
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Review current StreamInfo model and identify needed changes
2. Check if QualityOption and Resolution types exist in models
3. Update StreamInfo to include quality_options field
4. Enhance get_stream_url() in streaming.rs to build quality options array
5. Implement get_stream_url_for_quality() that routes to direct URL or decision endpoint
6. Test quality option generation with sample metadata
7. Verify integration with decision endpoint from Phase 1
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Phase 2 implementation complete. Enhanced stream URL generation with quality selection and decision endpoint routing.

## Files Modified
- src/backends/plex/api/streaming.rs: Updated quality option generation and added get_stream_url_for_quality()

## Implementation Details

### Quality Option Generation (streaming.rs)
- Modified get_stream_url() to generate quality options without hardcoded transcode URLs
- Transcode quality options now use empty URL string (generated on-demand)
- Quality options still include: Original (direct play), 1080p, 720p, 480p, 360p
- Only qualities lower than original resolution are included

### get_stream_url_for_quality() Method
- New method that takes media_id, QualityOption, and is_local flag
- Routes to direct URL for original quality (direct play)
- Routes to decision endpoint for transcoded qualities
- Calls get_stream_url_via_decision() from Phase 1 with quality parameters
- Converts bitrate to kbps and passes resolution to decision endpoint
- Returns stream URL ready for playback

### Integration with Phase 1
- Leverages get_stream_url_via_decision() from task-209
- Uses connection type detection (is_local) for proper lan/wan routing
- Decision endpoint handles transcoding parameter negotiation

## Design Decisions
- Transcode URLs generated on-demand to avoid stale URLs
- Quality options remain part of StreamInfo for UI display
- Direct play uses original URL for optimal performance
- Transcoded streams always use decision endpoint for server-side quality negotiation

## Next Steps
Phase 3 (task-TBD): Cache integration with quality-aware cache keys
<!-- SECTION:NOTES:END -->
