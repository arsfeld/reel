---
id: task-106
title: Add genre filtering to library page
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 23:08'
updated_date: '2025-09-16 23:40'
labels: []
dependencies:
  - task-119
priority: high
---

## Description

Implement genre-based filtering for movies and shows in the library view. Users should be able to filter content by one or multiple genres, with the filter persisting during the session.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add genres field extraction from database metadata JSON
- [x] #2 Create genre filter UI component with multi-select dropdown
- [x] #3 Implement genre filtering logic in LibraryPage
- [x] #4 Persist selected genres in component state
- [x] #5 Apply genre filter when loading and refreshing library items
<!-- AC:END -->


## Implementation Plan

1. Examine database schema and entities to understand metadata JSON structure for genres
2. Study LibraryPage component to understand current filtering architecture
3. Check if genre data exists in current database records
4. Create genre extraction logic from metadata JSON
5. Add genre filter state to LibraryPage component
6. Create multi-select dropdown UI component for genres
7. Implement filtering logic to apply genre filters to media items
8. Test filtering with multiple genre selections

## Implementation Notes

Implemented genre filtering for library page with the following changes:

1. Added genre filter state fields to LibraryPage struct (selected_genres, available_genres, genre_popover, genre_menu_button)
2. Created a MenuButton with popover containing checkboxes for each available genre
3. Extracted unique genres from all loaded media items using existing get_genres() method
4. Implemented filtering logic that combines text and genre filters when loading items
5. Added ToggleGenreFilter and ClearGenreFilters input messages to handle user interactions
6. Created update_genre_popover() method to dynamically update popover content with available genres
7. Added get_genre_label() helper method to format button label based on selection
8. Integrated genre filtering with existing sort and text filter functionality

The implementation uses Relm4's reactive patterns with proper state management and updates the UI when filters change. Genre filters persist during the session and are applied when refreshing or reloading library items.
