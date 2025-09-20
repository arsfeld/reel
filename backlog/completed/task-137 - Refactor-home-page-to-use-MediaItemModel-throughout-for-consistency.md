---
id: task-137
title: Refactor home page to use MediaItemModel throughout for consistency
status: Done
assignee:
  - '@claude'
created_date: '2025-09-17 03:53'
updated_date: '2025-09-17 14:08'
labels: []
dependencies: []
priority: high
---

## Description

The home page currently uses MediaItem domain models while the library page uses MediaItemModel database entities. This causes unnecessary conversions and loss of metadata. Refactor the home page to work entirely with MediaItemModel like the library page does, ensuring all metadata (playback progress, watched status, etc.) is preserved when displaying cached or fresh data.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Home page works entirely with MediaItemModel instead of MediaItem
- [x] #2 API responses are immediately converted to MediaItemModel and merged with existing DB records
- [x] #3 Cached data uses MediaItemModel directly without conversion
- [x] #4 All metadata (playback progress, watched status) is preserved
- [x] #5 Remove db_model_to_media_item conversion function
- [x] #6 Home sections contain MediaItemModel instead of MediaItem enum
<!-- AC:END -->


## Implementation Plan

1. Change HomeSection model to use MediaItemModel instead of MediaItem
2. Update BackendService methods to return MediaItemModel instead of MediaItem
3. Remove db_model_to_media_item conversion function
4. Update HomePage component to work with MediaItemModel
5. Update MediaCard factory to accept MediaItemModel
6. Test the refactored implementation


## Implementation Notes

Refactored the home page to use MediaItemModel throughout for consistency. Created a new HomeSectionWithModels type to maintain backward compatibility with the API layer while allowing the UI to work directly with database models.

Key changes:
- Created HomeSectionWithModels type that contains MediaItemModel instead of MediaItem
- Modified BackendService::get_home_sections_per_source to convert API MediaItem to MediaItemModel and save to database
- Updated BackendService::get_cached_home_sections to return MediaItemModel directly
- Removed db_model_to_media_item conversion function
- Updated HomePage component to work with HomeSectionWithModels
- Added MediaService::media_item_to_model helper function for conversions

This ensures all metadata like playback progress and watched status is preserved when displaying both cached and fresh data.
