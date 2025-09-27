---
id: task-280
title: Set curlhttpsrc as default HTTP source for macOS GStreamer
status: To Do
assignee: []
created_date: '2025-09-27 02:49'
labels:
  - gstreamer
  - macos
  - player
dependencies: []
priority: high
---

## Description

Configure GStreamer to use curlhttpsrc instead of souphttpsrc on macOS to avoid TLS issues. The souphttpsrc plugin lacks TLS support on macOS, causing HTTPS streaming failures. curlhttpsrc uses libcurl which has proper TLS support.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Research the best method to set curlhttpsrc as default (environment variable vs runtime configuration)
- [ ] #2 Implement solution to prioritize curlhttpsrc over souphttpsrc on macOS
- [ ] #3 Test HTTPS streaming works with Plex/Jellyfin servers
- [ ] #4 Document the configuration in CLAUDE.md for future reference
<!-- AC:END -->
