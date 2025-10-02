---
id: task-280
title: Set curlhttpsrc as default HTTP source for macOS GStreamer
status: Done
assignee:
  - '@assistant'
created_date: '2025-09-27 02:49'
updated_date: '2025-09-27 23:55'
labels:
  - gstreamer
  - macos
  - player
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Configure GStreamer to use curlhttpsrc instead of souphttpsrc on macOS to avoid TLS issues. The souphttpsrc plugin lacks TLS support on macOS, causing HTTPS streaming failures. curlhttpsrc uses libcurl which has proper TLS support.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Research the best method to set curlhttpsrc as default (environment variable vs runtime configuration)
- [x] #2 Implement solution to prioritize curlhttpsrc over souphttpsrc on macOS
- [x] #3 Test HTTPS streaming works with Plex/Jellyfin servers
- [x] #4 Document the configuration in CLAUDE.md for future reference
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Research GStreamer plugin ranking and element prioritization methods
2. Check current GStreamer setup in the codebase
3. Implement runtime configuration to set curlhttpsrc as preferred HTTP source
4. Add platform-specific check to apply this only on macOS
5. Test with HTTPS streaming from Plex/Jellyfin
6. Update documentation
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented minimal runtime configuration to prioritize curlhttpsrc over souphttpsrc on macOS.

Changes made:
1. Added configure_macos_http_source_priority() method that simply:
   - Sets curlhttpsrc rank to PRIMARY+100 to ensure it is preferred
   - Logs a warning if curlhttpsrc is not available

The solution uses GStreamer's plugin ranking system to ensure playbin automatically selects curlhttpsrc when creating HTTP source elements, providing reliable HTTPS streaming on macOS.

This minimal approach avoids over-configuration and lets GStreamer handle the rest.
<!-- SECTION:NOTES:END -->
