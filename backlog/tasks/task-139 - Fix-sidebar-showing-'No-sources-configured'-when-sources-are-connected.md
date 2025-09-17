---
id: task-139
title: Fix sidebar showing 'No sources configured' when sources are connected
status: To Do
assignee: []
created_date: '2025-09-17 04:01'
labels: []
dependencies: []
---

## Description

The sidebar status area incorrectly displays 'No sources configured' even when sources are successfully connected and have libraries. This happens because the update_status_text method is not being called after sources are loaded. The status should properly reflect the connection state of sources.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Status shows correct count of connected sources when sources are loaded
- [ ] #2 Status updates immediately when sources are connected or disconnected
- [ ] #3 Status shows 'No sources configured' only when there are actually no sources
- [ ] #4 update_status_text is called after SourcesLoaded input is handled
<!-- AC:END -->
