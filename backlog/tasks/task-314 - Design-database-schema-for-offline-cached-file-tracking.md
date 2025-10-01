---
id: task-314
title: Design database schema for offline/cached file tracking
status: Done
assignee:
  - '@claude'
created_date: '2025-09-29 13:00'
updated_date: '2025-09-29 13:09'
labels:
  - cache
  - database
  - offline
dependencies: []
priority: high
---

## Description

Create a database schema to track offline and cached files, replacing the current metadata.json approach. This schema must support the existing cache system and enable future offline playback/download features.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Schema supports all current cache metadata (file size, download status, timestamps, etc.)
- [x] #2 Schema includes tables for tracking partial downloads and resume capability
- [x] #3 Schema supports offline content management (download priorities, expiration, user preferences)
- [x] #4 Schema includes indexes for efficient cache lookups by media_id and source_id
- [x] #5 Schema supports tracking multiple quality variants per media item
- [x] #6 Migration plan defined for existing cached content with metadata.json files
<!-- AC:END -->


## Implementation Plan

1. Analyze existing cache metadata structure and requirements
2. Design new database tables for cache tracking (cache_entries, cache_chunks, cache_stats)
3. Add support for partial downloads and byte range tracking
4. Include tables for quality variants and download priorities
5. Create appropriate indexes for performance
6. Design migration strategy from metadata.json to database


## Implementation Notes

Designed and implemented comprehensive database schema for cache tracking to replace the current metadata.json approach.


## Schema Design

Created 6 new tables with proper relationships and indexes:

1. **cache_entries** - Main cache entry tracking
   - Tracks all metadata from current CacheMetadata struct
   - Supports file size, download status, timestamps, access counts
   - Includes media codec information and quality details
   - Foreign keys to media_items and sources tables

2. **cache_chunks** - Byte range tracking for partial downloads
   - Enables resume capability by tracking downloaded byte ranges
   - Supports HTTP range requests for streaming

3. **cache_download_queue** - Download priority management
   - User-requested vs automatic downloads
   - Retry tracking and scheduling
   - Priority-based queue processing

4. **cache_quality_variants** - Multiple quality options per media
   - Tracks available resolutions and bitrates
   - Codec and container format information
   - Default quality selection

5. **cache_statistics** - Global cache metrics
   - Total size and file count tracking
   - Hit/miss ratios for performance monitoring
   - Automatic updates via database triggers

6. **cache_headers** - HTTP headers for validation
   - ETag support for cache validation
   - Stores relevant HTTP headers

## Performance Optimizations

- Composite unique index on (source_id, media_id, quality)
- Indexes on frequently queried columns (media_id, source_id)
- Priority-based indexes for LRU eviction
- Range query optimization for byte chunks

## Migration Strategy

No migration needed as the application has not been released yet. The new schema will be the initial implementation.

## Files Modified

- Created: src/db/migrations/m20250929_000001_add_cache_tracking.rs
- Modified: src/db/migrations/mod.rs (registered new migration)
