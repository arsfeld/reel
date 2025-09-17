---
id: task-157
title: >-
  Fix UI blocking when navigating to TV show details while loading episode
  thumbnails
status: To Do
assignee: []
created_date: '2025-09-17 18:54'
labels: []
dependencies: []
priority: high
---

## Description

When navigating to a TV show details page, the UI becomes blocked/frozen while episode thumbnails are being loaded. This creates a poor user experience as the entire interface becomes unresponsive during the loading process. The thumbnail loading should be asynchronous and non-blocking.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 UI remains responsive when navigating to TV show details page
- [ ] #2 Episode thumbnails load asynchronously without blocking the main UI thread
- [ ] #3 User can interact with the page while thumbnails are still loading
- [ ] #4 Loading indicators are shown for thumbnails that are still being fetched
- [ ] #5 Implement proper async/background loading for episode thumbnails
<!-- AC:END -->
