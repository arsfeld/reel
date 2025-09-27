# Home Sections Database Schema

## Overview

The home sections schema stores the actual Plex (and eventually Jellyfin) home page sections data for offline-first access. This allows the application to display the home page immediately from the cache while background synchronization updates the data.

## Tables

### `home_sections`

Stores the section metadata and configuration.

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER (PK) | Auto-incrementing primary key |
| `source_id` | STRING (FK) | Reference to the source (Plex/Jellyfin server) |
| `hub_identifier` | STRING | Unique identifier from the backend (e.g., "home.continue", "library.recentlyAdded.1") |
| `title` | STRING | Display title of the section |
| `section_type` | STRING | Type classification (ContinueWatching, RecentlyAdded, etc.) |
| `position` | INTEGER | Display order on the home page |
| `context` | STRING (nullable) | Additional context from the backend |
| `style` | STRING (nullable) | Display style hint from the backend |
| `hub_type` | STRING (nullable) | Backend-specific hub type |
| `size` | INTEGER (nullable) | Suggested number of items to display |
| `last_updated` | TIMESTAMP | When this section was last updated |
| `is_stale` | BOOLEAN | Whether this section needs refreshing |
| `created_at` | TIMESTAMP | When the record was created |
| `updated_at` | TIMESTAMP | When the record was last modified |

### `home_section_items`

Junction table linking sections to media items with ordering.

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER (PK) | Auto-incrementing primary key |
| `section_id` | INTEGER (FK) | Reference to home_sections.id |
| `media_item_id` | STRING (FK) | Reference to media_items.id |
| `position` | INTEGER | Display order within the section |
| `created_at` | TIMESTAMP | When the record was created |

## Indexes

### Performance Indexes

1. **`idx_home_sections_source_hub`** (UNIQUE)
   - On: `source_id`, `hub_identifier`
   - Purpose: Ensure uniqueness and fast lookups when updating sections

2. **`idx_home_sections_source_position`**
   - On: `source_id`, `position`
   - Purpose: Fast ordered retrieval of sections for a source

3. **`idx_home_section_items_section_position`**
   - On: `section_id`, `position`
   - Purpose: Fast ordered retrieval of items within a section

4. **`idx_home_section_items_unique`** (UNIQUE)
   - On: `section_id`, `media_item_id`
   - Purpose: Prevent duplicate media items within a section

## Design Decisions

### 1. Separate Tables vs. JSON

**Decision**: Use normalized tables instead of storing items as JSON in the home_sections table.

**Rationale**:
- Maintains referential integrity with media_items table
- Allows efficient queries for specific items across sections
- Enables cascade deletes when media items are removed
- Better query performance for large datasets

### 2. Position-based Ordering

**Decision**: Use integer `position` columns for ordering both sections and items.

**Rationale**:
- Explicit control over display order
- Easy reordering without affecting other data
- Consistent with backend API patterns

### 3. Staleness Tracking

**Decision**: Include `last_updated` and `is_stale` columns for cache management.

**Rationale**:
- Enables intelligent background refresh strategies
- Allows marking sections as stale without immediate deletion
- Supports offline-first architecture with gradual updates

### 4. Backend-specific Fields

**Decision**: Store backend-specific fields (context, style, hub_type, size) as nullable columns.

**Rationale**:
- Preserves backend-specific hints for UI rendering
- Allows future backends to use same schema
- Nullable fields don't impact storage for backends that don't use them

### 5. Auto-increment IDs

**Decision**: Use auto-incrementing integer IDs instead of composite keys.

**Rationale**:
- Simpler foreign key relationships
- Better performance for joins
- Easier to work with in the application layer

## Migration Strategy

The schema is added via migration `m20250107_000001_add_home_sections.rs` which:
1. Creates both tables with all columns and constraints
2. Establishes foreign key relationships to existing tables
3. Creates all necessary indexes
4. Provides rollback capability via down() method

## Future Considerations

1. **Partitioning**: If the items table grows very large, consider partitioning by source_id
2. **Caching Strategy**: The `is_stale` flag can be used for implementing various cache invalidation strategies
3. **Additional Metadata**: The schema can be extended with more backend-specific columns as needed
4. **Archival**: Old sections could be archived rather than deleted for historical analysis