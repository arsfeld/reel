---
id: task-370
title: Sync cast and crew metadata from backend servers
status: In Progress
assignee:
  - '@claude'
created_date: '2025-10-03 16:57'
updated_date: '2025-10-03 17:09'
labels:
  - feature
  - sync
  - backend
  - metadata
  - database
dependencies: []
priority: high
---

## Description

Cast and crew information (actors, directors, writers, producers) is not currently synced from Plex and Jellyfin backends. Need to implement proper fetching and storage of people metadata during sync, including roles, character names, and profile images. This information should be stored in a normalized database structure and displayed in movie/show detail pages.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Research Plex API for cast/crew metadata structure
- [x] #2 Research Jellyfin API for cast/crew metadata structure
- [x] #3 Design database schema for people/cast/crew (people table, media_people junction table)
- [x] #4 Create database migration for people tables
- [x] #5 Implement cast/crew fetching in Plex backend sync
- [ ] #6 Implement cast/crew fetching in Jellyfin backend sync
- [ ] #7 Store people metadata in database with roles and character names
- [ ] #8 Display cast information in movie details page
- [ ] #9 Display cast information in show details page
<!-- AC:END -->


## Implementation Plan

1. Research Plex and Jellyfin API responses to understand cast/crew data structure
2. Add Role/Director types to Plex and Jellyfin API type definitions
3. Design database schema (people table + media_people junction table)
4. Create SeaORM migration for the new tables
5. Update API parsing to extract cast/crew from responses
6. Store people and relationships in database during sync
7. Update repository to load cast/crew when fetching media
8. Update UI to display cast in movie/show details pages


## Implementation Notes

## Progress Summary

### Completed:
1. **Research** - Analyzed Plex and Jellyfin API structures
   - Plex: Role/Director/Writer arrays with name, role, thumb
   - Jellyfin: People array with Name, Type

2. **Database Schema** - Created normalized tables:
   - `people` table: id, name, image_url, timestamps
   - `media_people` junction: links media to people with type and role
   - Added indexes for efficient queries

3. **SeaORM Entities** - Created entity models for new tables

4. **Migration** - Created m20251003_000001_add_people_tables migration

5. **Plex Backend** - Implemented cast/crew fetching:
   - Added PlexRole, PlexDirector, PlexWriter types
   - Updated PlexMovieMetadata and PlexShowMetadata
   - Parse cast from roles with character names
   - Parse crew from directors/writers

### Remaining:
- Jellyfin backend cast/crew fetching
- Database storage logic (repository layer)
- UI display in movie/show details pages

### Follow-up Tasks Created:
- task-373: Implement cast/crew fetching in Jellyfin backend
- task-374: Store people metadata in database during sync
- task-375: Display cast information in movie details page
- task-376: Display cast information in show details page
