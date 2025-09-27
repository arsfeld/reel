---
id: task-260
title: Create database schema for storing Plex home sections
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 17:48'
updated_date: '2025-09-26 17:53'
labels: []
dependencies: []
---

## Description

Design and implement database tables to store the actual Plex home sections data including hub identifiers, titles, section types, and ordering. This preserves the exact structure from Plex for offline access.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Design home_sections table schema with columns: id, source_id, hub_identifier, title, section_type, position, context, style
- [x] #2 Design home_section_items junction table with columns: section_id, media_item_id, position
- [x] #3 Consider adding metadata columns for caching: last_updated, is_stale
- [x] #4 Document the schema design decisions
<!-- AC:END -->


## Implementation Plan

1. Study existing database schema and migration patterns
2. Design home_sections table with all required columns
3. Design home_section_items junction table for media relationships
4. Create new migration file following SeaORM patterns
5. Implement up() migration to create tables
6. Implement down() migration to drop tables
7. Add appropriate indexes for performance
8. Document schema design decisions


## Implementation Notes

Created database schema for storing Plex home sections with two tables:

1. **home_sections table**: Stores section metadata including hub_identifier, title, section_type, position, and caching fields
2. **home_section_items table**: Junction table linking sections to media items with ordering

Implementation includes:
- Migration file m20250107_000001_add_home_sections.rs with up/down migrations
- Foreign key relationships to sources and media_items tables
- Performance indexes for lookups and ordering
- Unique constraints to prevent duplicates
- Documentation in docs/database/home-sections-schema.md explaining design decisions

The schema supports offline-first architecture by storing complete section data locally with staleness tracking for intelligent refresh strategies.
