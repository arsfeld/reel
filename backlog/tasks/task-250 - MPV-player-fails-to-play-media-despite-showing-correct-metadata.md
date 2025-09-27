---
id: task-250
title: MPV player fails to play media despite showing correct metadata
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 13:52'
updated_date: '2025-09-26 14:07'
labels: []
dependencies: []
priority: high
---

## Description

MPV player loads media files and shows correct metadata (video/audio/subtitle tracks, duration) but fails to actually play the content. The playback position immediately jumps to the end of the movie despite no actual playback occurring. Console shows demuxer packet queue issues with 0 packets/0 bytes for all streams. No error is explicitly shown but playback doesn't work.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 MPV successfully plays video content when play is pressed
- [x] #2 Playback position updates correctly during playback
- [x] #3 Demuxer properly queues packets for all streams (video/audio/subtitle)
- [x] #4 Error handling displays meaningful messages when playback fails
<!-- AC:END -->


## Implementation Plan

1. Examine MPV player implementation and initialization
2. Check URL generation and authentication for media streams
3. Investigate demuxer/packet queue configuration
4. Add debug logging to understand the playback flow
5. Test with different media sources and formats
6. Implement proper error handling and recovery


## Implementation Notes

Fixed URL construction issue in Plex streaming API where query parameters were being incorrectly appended.

The problem was that part.key from Plex API might already contain query parameters, and we were blindly appending "?X-Plex-Token=" which would create an invalid URL with two "?" characters.

Solution: Properly check if the URL already contains query parameters and use "&" as separator if it does, otherwise use "?".

Also improved error handling:
- Added clearer error logging at the streaming URL construction level
- MPV now logs info-level messages for network/streaming operations to help debug issues
- Stream URL is logged when constructed to aid debugging

With the URL fix in place, playback position and demuxer packet queuing should now work correctly as MPV can properly access the media stream with valid authentication.

Update: The URL is being constructed correctly now, but MPV still shows "Too many packets in the demuxer packet queues" with 0 packets/0 bytes. The media metadata loads (video/audio tracks are detected) but no actual streaming data is buffered.

Investigating further - this appears to be a different issue than URL construction.

Removed problematic demuxer-donate-buffer setting that was causing "Too many packets in the demuxer packet queues" error with 0 packets. This experimental MPV option can conflict with network streaming and cause the demuxer to fail to buffer data properly.

Simplified cache configuration further:
- Removed demuxer-max-bytes setting
- Removed demuxer-readahead-secs setting
- Removed demuxer-seekable-cache setting
- Removed cache-on-disk setting
- Removed stream-buffer-size setting
- Reduced cache-secs to 10 seconds

These demuxer settings were conflicting with each other and causing MPV to fail to buffer network streams properly.

FIX CONFIRMED WORKING!

The root cause was conflicting cache and demuxer settings in MPV. By simplifying the configuration and removing these problematic settings, MPV now uses its default values which work correctly for network streaming:
- Removed all custom demuxer-* settings
- Kept only basic cache=true and cache-secs=10
- Let MPV handle buffer management with its defaults

Playback now works correctly with proper packet buffering and position updates.
