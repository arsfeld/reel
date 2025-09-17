---
id: task-097
title: Implement proper error handling and loading states for home sections
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 19:30'
updated_date: '2025-09-17 03:23'
labels:
  - home
  - error-handling
  - ux
  - high
dependencies: []
priority: high
---

## Description

The HomePage component needs robust error handling for backend failures and proper loading states to prevent broken UI when sections fail to load or are slow to respond.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Individual section failures don't break the entire home page
- [x] #2 Loading spinners appear for each section while data is being fetched
- [x] #3 Network errors show appropriate retry mechanisms
- [x] #4 Backend authentication failures are handled gracefully
- [x] #5 Empty or failed sections show informative messages to users
- [x] #6 Loading states don't interfere with successful sections displaying
<!-- AC:END -->


## Implementation Plan

1. Add per-section loading states to track individual section loading
2. Add per-section error states and retry mechanism
3. Modify BackendService to return per-source results with errors
4. Update HomePage UI to show section-specific loading and error states
5. Add timeout handling for slow backends
6. Implement retry button for failed sections
7. Test with network disconnection and auth failures

## Implementation Notes

Implemented robust error handling and loading states for home sections:

## What was changed:

1. **Added per-source loading states** - Each media source now has its own loading state tracked independently
2. **Created new BackendService method** - `get_home_sections_per_source()` returns individual results per source with error handling
3. **Updated HomePage component** - Now handles SourceSectionsLoaded events with per-source error/success states
4. **Added retry mechanism** - Failed sources show error message with retry button
5. **Implemented timeout handling** - 10-second timeout prevents slow backends from blocking
6. **Added visual feedback** - Loading spinners and error icons for better UX

## Technical details:

- Modified `/src/services/core/backend.rs` to add `get_home_sections_per_source()` method
- Updated `/src/platforms/relm4/components/pages/home.rs` with new state management
- Added `SectionLoadState` enum to track Loading/Loaded/Failed states per source
- Implemented display methods: `display_source_loading()`, `display_source_error()`, `display_source_sections()`
- Each source loads independently and failures do not affect other sources

This implementation ensures the home page remains functional even when individual backends fail, providing a better user experience with clear feedback and recovery options.
