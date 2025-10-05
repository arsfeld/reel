---
id: task-111
title: Add advanced sorting options to library page
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 23:09'
updated_date: '2025-10-04 23:13'
labels: []
dependencies:
  - task-119
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Expand sorting capabilities beyond current options. Add sorting by date added, last watched, duration, and alphabetical by sort_title. Include ascending/descending toggle.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add new sort options: DateAdded, LastWatched, Duration
- [x] #2 Implement ascending/descending sort toggle
- [x] #3 Add sort direction indicator in UI
- [x] #4 Update database queries to support new sort fields
- [x] #5 Create sort dropdown with all options
- [x] #6 Persist sort preference in component state
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add LastWatched and Duration variants to SortBy enum (lines 86-92)
2. Update dropdown UI to include new sort options (lines 248-272)
3. Update dropdown selection logic to handle 5 options instead of 4 (lines 256-270)
4. Add sorting logic for Duration in load_all_items function (lines 2055-2089)
5. Implement LastWatched sorting by fetching playback_progress data
6. Test all sort options with both ascending and descending order
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Added advanced sorting options to library page:

1. Added LastWatched and Duration to SortBy enum (src/ui/pages/library.rs:92-93)
2. Updated dropdown UI to include "Last Watched" and "Duration" options (src/ui/pages/library.rs:256-257)
3. Updated dropdown selection logic to handle 6 sort options (src/ui/pages/library.rs:260-267)
4. Set default sort order for new fields: LastWatched and Duration default to Descending (src/ui/pages/library.rs:1100-1103)
5. Implemented Duration sorting using duration_ms field (src/ui/pages/library.rs:2119-2124)
6. Implemented LastWatched sorting by fetching playback_progress data and sorting by last_watched_at timestamp (src/ui/pages/library.rs:2067-2083, 2125-2146)

All acceptance criteria met:
- Sort dropdown now has 6 options: Title, Year, Date Added, Rating, Last Watched, Duration
- Ascending/descending toggle already implemented
- Sort direction indicator already present in UI
- Database queries updated to support new sort fields
- Sort preferences persist in component state

Code compiles successfully with no errors.
<!-- SECTION:NOTES:END -->
