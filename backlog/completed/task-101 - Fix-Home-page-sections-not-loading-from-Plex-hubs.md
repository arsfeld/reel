---
id: task-101
title: Fix Home page sections not loading from Plex hubs
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 19:36'
updated_date: '2025-09-16 23:47'
labels:
  - bug
  - ui
  - plex
dependencies: []
priority: high
---

## Description

The Home page only shows Continue Watching and Recently Added sections, but should display all sections/hubs available from the Plex server including On Deck, Recently Added per library, Popular, etc. Need to implement proper Plex hubs API integration.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Home page displays all hub sections from Plex server
- [x] #2 Each section shows the correct items from its hub
- [x] #3 Section titles match those from Plex
- [x] #4 Sections are displayed in the correct order
<!-- AC:END -->


## Implementation Plan

1. Review current Plex hub implementation in api.rs
2. Identify that get_all_home_sections already fetches all hubs
3. Update HomePage component to call BackendService::get_all_home_sections
4. Refactor HomePage to support dynamic section rendering
5. Create factories for each section dynamically
6. Fix compilation errors with proper type conversions


## Implementation Notes

Fixed the HomePage to properly load and display all Plex hub sections:

- Replaced hardcoded Continue Watching and Recently Added sections with dynamic section loading
- Added call to BackendService::get_all_home_sections() which fetches hubs from all backends
- Refactored HomePage component to use HashMap of section factories for dynamic sections
- Each section is created dynamically with its own FactoryVecDeque for media cards
- Added proper MediaItem to MediaItemModel conversion with correct type mappings
- Fixed compilation errors related to DateTime types and Option handling

The HomePage now displays all hub sections returned by Plex including On Deck, Recently Added per library, Popular, and other library-specific hubs.
