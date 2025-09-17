---
id: task-137
title: Refactor home page to use MediaItemModel throughout for consistency
status: To Do
assignee: []
created_date: '2025-09-17 03:53'
labels: []
dependencies: []
priority: high
---

## Description

The home page currently uses MediaItem domain models while the library page uses MediaItemModel database entities. This causes unnecessary conversions and loss of metadata. Refactor the home page to work entirely with MediaItemModel like the library page does, ensuring all metadata (playback progress, watched status, etc.) is preserved when displaying cached or fresh data.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Home page works entirely with MediaItemModel instead of MediaItem
- [ ] #2 API responses are immediately converted to MediaItemModel and merged with existing DB records
- [ ] #3 Cached data uses MediaItemModel directly without conversion
- [ ] #4 All metadata (playback progress, watched status) is preserved
- [ ] #5 Remove db_model_to_media_item conversion function
- [ ] #6 Home sections contain MediaItemModel instead of MediaItem enum
<!-- AC:END -->
