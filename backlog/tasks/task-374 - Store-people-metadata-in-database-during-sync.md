---
id: task-374
title: Store people metadata in database during sync
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 17:09'
updated_date: '2025-10-03 17:14'
labels:
  - database
  - repository
  - sync
  - backend
dependencies: []
priority: high
---

## Description

Implement repository layer to persist people and media_people relationships to database. Create upsert logic for people table and manage media_people junction entries during media sync operations.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create PeopleRepository with upsert method
- [x] #2 Add methods to MediaRepository for saving people relationships
- [x] #3 Implement upsert_people helper in sync logic
- [x] #4 Update media item save to persist cast/crew
- [x] #5 Handle cleanup of old people relationships on resync
- [x] #6 Test people data persists across syncs
<!-- AC:END -->


## Implementation Plan

1. Create PeopleRepository with upsert method in src/db/repository/people_repository.rs
2. Update mod.rs to export PeopleRepository
3. Add helper methods to MediaService for extracting and saving people from metadata
4. Integrate people saving into MediaService::save_media_item
5. Add cleanup logic for stale people relationships
6. Build and test to verify people data persists


## Implementation Notes

Implemented complete people metadata persistence system:

1. Created PeopleRepository with upsert methods for both individual and batch operations
2. Added save_media_people and delete_media_people methods for managing relationships
3. Integrated people saving into MediaService::save_media_item
4. People are extracted from Movie and Show items (cast and crew)
5. Person IDs are prefixed with source_id to avoid conflicts across backends
6. Media_people junction table maintains relationships with type, role, and sort_order
7. Cleanup is automatic - save_media_people deletes old relationships before inserting new ones

The system now persists cast (actors) and crew (directors, writers, producers) to the database during sync operations and maintains proper relationships through the media_people junction table.
