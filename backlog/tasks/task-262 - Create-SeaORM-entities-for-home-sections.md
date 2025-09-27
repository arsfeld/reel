---
id: task-262
title: Create SeaORM entities for home sections
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 17:48'
updated_date: '2025-09-26 17:58'
labels: []
dependencies: []
---

## Description

Implement SeaORM entity models for the home_sections and home_section_items tables. These entities will provide type-safe database access for storing and retrieving Plex home sections.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create home_sections.rs entity with proper SeaORM derives
- [x] #2 Create home_section_items.rs entity with proper SeaORM derives
- [x] #3 Define relations between home_sections and media_items through junction table
- [x] #4 Add the entities to the entities module exports
- [x] #5 Ensure entities match the migration schema exactly
<!-- AC:END -->


## Implementation Plan

1. Create home_sections.rs entity file matching the migration schema
2. Create home_section_items.rs entity file for the junction table
3. Define proper relations between entities (home_sections has_many home_section_items, belongs_to sources)
4. Define relations for junction table (belongs_to home_sections, belongs_to media_items)
5. Update mod.rs to export the new entities
6. Test compilation to ensure all entities are properly integrated


## Implementation Notes

Created SeaORM entities for home_sections and home_section_items tables:

1. **home_sections.rs**: Entity for the main sections table with all fields matching the migration schema (id, source_id, hub_identifier, title, section_type, position, context, style, hub_type, size, last_updated, is_stale, created_at, updated_at)

2. **home_section_items.rs**: Junction table entity linking sections to media items with fields (id, section_id, media_item_id, position, created_at)

3. **Relations defined**:
   - home_sections belongs_to sources
   - home_sections has_many home_section_items
   - home_sections has many-to-many with media_items via home_section_items junction
   - home_section_items belongs_to both home_sections and media_items

4. **Module exports updated**: Added both new entities to mod.rs with consistent naming pattern (HomeSectionActiveModel, HomeSection entity, HomeSectionModel, etc.)

All entities compile successfully and follow the established patterns in the codebase.
