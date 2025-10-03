---
id: task-377
title: Load cast/crew from people tables when fetching media items
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 17:29'
updated_date: '2025-10-03 17:32'
labels:
  - bug
dependencies: []
priority: high
---

## Description

Modify get_item_details to load people from people/media_people tables and populate cast/crew fields, instead of relying on metadata JSON which may be outdated

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Load people data from people_repository.find_by_media_item()
- [x] #2 Separate cast and crew based on person_type field
- [x] #3 Convert to Person objects and populate MediaItem cast/crew fields
- [x] #4 Update both movie and show loading
<!-- AC:END -->


## Implementation Notes

Fixed cast/crew loading to read from people/media_people tables instead of metadata JSON.

## Bug Fix: Ambiguous Column Name

After initial implementation, encountered SQL error: "ambiguous column name: media_people.id"

**Root Cause**: Both `people` and `media_people` tables have an `id` column:
- `people.id` (String, primary key)  
- `media_people.id` (i32, auto-increment primary key)

When using JOIN with `find_also_related()`, SeaORM does SELECT * which includes both IDs, causing SQLite ambiguity.

**Solution**: Changed query approach in `find_by_media_item()`:
1. First query `media_people` table filtered by `media_item_id`
2. Then fetch each corresponding `person` record by `person_id`
3. This avoids the JOIN and ambiguous column issue
4. Results are ordered by `sort_order` to maintain cast/crew ordering

Builds successfully and ready to test.


## Changes Made

1. Modified `MediaService::get_media_item()` in `src/services/core/media.rs`:
   - After loading media item model from database, now also loads people data
   - Uses `PeopleRepository::find_by_media_item()` to get all associated people
   - Separates cast and crew based on `person_type` field ("cast" vs "crew")
   - Converts database models to `Person` objects with id, name, role, and image_url
   - Injects cast/crew into Movie and Show items after conversion

2. Why this was needed:
   - Previous implementation relied on metadata JSON which was empty for items synced before task-374
   - People data is now properly stored in dedicated people/media_people tables
   - This ensures cast/crew displays correctly for all media items

## Testing
Builds successfully. Cast/crew will now load from the database for both movies and TV shows.
