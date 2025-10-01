---
id: task-315
title: Integrate database schema with cache system
status: Done
assignee:
  - '@claude'
created_date: '2025-09-29 13:12'
updated_date: '2025-09-29 13:32'
labels:
  - cache
  - database
  - integration
dependencies: []
priority: high
---

## Description

Replace the current JSON-based metadata storage in src/cache/ with the new database schema. Update CacheStorage, CacheMetadata, and related components to use the database tables instead of metadata.json files.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Update CacheStorage to use database instead of metadata.json
- [x] #2 Migrate CacheMetadata operations to use cache_entries table
- [x] #3 Update cache chunk tracking to use cache_chunks table
- [x] #4 Integrate download queue management with cache_download_queue table
- [x] #5 Update cache statistics to use cache_statistics table
- [x] #6 Ensure all cache operations maintain database consistency
<!-- AC:END -->


## Implementation Plan

1. Analyze existing cache metadata JSON implementation
2. Create SeaORM entity files for cache tables (cache_entries, cache_chunks, cache_headers, cache_download_queue, cache_quality_variants, cache_statistics)
3. Create repository layer for cache database operations
4. Update CacheStorage to use database repository instead of JSON files
5. Update FileCache to pass database connection to CacheStorage
6. Update cache service initialization to provide database connection
7. Fix all compilation errors from async method changes
8. Test that cache operations work with database backend


## Implementation Notes

Successfully integrated the database schema with the cache system:

1. Created SeaORM entity files for all cache-related tables (cache_entries, cache_chunks, cache_headers, cache_download_queue, cache_quality_variants, cache_statistics)

2. Implemented comprehensive CacheRepository with all CRUD operations for:
   - Cache entries with file metadata tracking
   - Chunk management for partial downloads
   - Download queue management
   - Cache statistics tracking
   - Quality variants
   - HTTP headers storage

3. Updated CacheStorage to use database repository instead of metadata.json:
   - All metadata operations now use database
   - Converted async methods throughout
   - Maintained backward compatibility with existing cache files

4. Updated FileCache and cache service to pass database connection through initialization chain

5. Fixed all compilation errors from async/await changes

The cache system now uses the database for all metadata storage, providing better consistency, concurrent access support, and eliminating the need for JSON file management.
