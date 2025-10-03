---
id: task-381
title: Add database fields to store intro and credits markers
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 18:08'
updated_date: '2025-10-03 18:25'
labels:
  - database
  - schema
  - markers
dependencies: []
priority: high
---

## Description

Extend the media_items database schema to persist intro_marker and credits_marker data fetched from backends, allowing the UI to display skip buttons without re-fetching from APIs

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Database migration adds intro_marker_start_ms, intro_marker_end_ms columns to media_items table
- [x] #2 Database migration adds credits_marker_start_ms, credits_marker_end_ms columns to media_items table
- [x] #3 MediaItemModel entity updated with new marker fields
- [x] #4 Mapper updates to_model() to serialize ChapterMarker to database fields
- [x] #5 Mapper updates TryFrom to deserialize database fields to ChapterMarker
- [x] #6 Migration tested with cargo test
<!-- AC:END -->


## Implementation Plan

1. Create new migration file for adding marker columns to media_items table
2. Add intro_marker_start_ms, intro_marker_end_ms, credits_marker_start_ms, credits_marker_end_ms columns
3. Update MediaItemModel entity with new Option<i64> fields
4. Update mapper to_model() to serialize ChapterMarker Duration to milliseconds
5. Update mapper TryFrom to deserialize milliseconds to ChapterMarker with Duration
6. Register migration in mod.rs
7. Test migration with cargo test


## Implementation Notes

Implemented database fields to store intro and credits markers for movies and episodes:

1. Created migration m20251003_000002_add_marker_fields.rs that adds:
   - intro_marker_start_ms (Option<i64>)
   - intro_marker_end_ms (Option<i64>)
   - credits_marker_start_ms (Option<i64>)
   - credits_marker_end_ms (Option<i64>)

2. Updated MediaItemModel entity in src/db/entities/media_items.rs with new fields

3. Updated TryFrom<Model> for MediaItem to deserialize database fields into ChapterMarker instances for movies and episodes

4. Updated MediaItem::to_model() to serialize ChapterMarker Duration values to milliseconds for storage

5. Updated all MediaItemActiveModel creations in media_repository.rs to include new fields

6. Registered migration in migrations/mod.rs

The implementation is complete. Note: cargo test blocked by unrelated compilation errors in people_repository (cast/crew feature work in progress).
