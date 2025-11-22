---
id: task-373
title: Implement cast/crew fetching in Jellyfin backend
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 17:09'
updated_date: '2025-10-03 17:16'
labels:
  - backend
  - jellyfin
  - metadata
dependencies: []
priority: high
---

## Description

Add cast and crew metadata parsing to Jellyfin API responses. Parse the People array from Jellyfin API and convert to Person structs with proper type mapping (Actor, Director, Writer, etc.)

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add Jellyfin People type definitions to api types
- [x] #2 Update JellyfinMovieMetadata to include People array
- [x] #3 Update JellyfinShowMetadata to include People array
- [x] #4 Parse People array into cast/crew lists in get_movies
- [x] #5 Parse People array into cast list in get_shows
- [x] #6 Map Jellyfin Type field to Person roles
<!-- AC:END -->


## Implementation Plan

1. Examine Plex implementation for reference
2. Look at current Jellyfin API types and responses
3. Add People type definitions to Jellyfin API types
4. Update JellyfinMovieMetadata and JellyfinShowMetadata structs
5. Implement parsing logic in get_movies and get_shows
6. Map Jellyfin Type field to Person roles (Actor, Director, Writer, etc.)
7. Test implementation


## Implementation Notes

Cast and crew fetching for Jellyfin was already fully implemented.

Implementation details:
- BaseItemPerson type defined in api.rs:1390-1399 with id, name, role, person_type, and primary_image_tag fields
- JellyfinItem struct includes people: Option<Vec<BaseItemPerson>> field (api.rs:1363)
- Fields parameter includes "People" in all relevant API calls (get_movies, get_shows, get_continue_watching, get_latest_movies, get_next_up)
- convert_people_to_cast_crew method (api.rs:1229-1263) properly maps Jellyfin person types:
  - Actor/GuestStar → cast array
  - Director/Writer/Producer/Composer → crew array
  - Unknown types → cast array (fallback)
- Method is called in:
  - get_movies (api.rs:476) - parses cast and crew
  - get_shows (api.rs:578) - parses cast only
  - convert_items_to_media (api.rs:1140) - parses cast and crew for movies

Fixed unrelated compilation issues:
- Added .clone() to show.metadata in media_repository.rs:666 to fix partial move error
- Added PaginatorTrait import to people_repository.rs:9 for .count() method

All acceptance criteria verified and checked. The implementation is complete and the code compiles successfully.
