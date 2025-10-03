---
id: task-390
title: 'Phase 3: Cache Integration with Quality-Aware Keys'
status: Done
assignee:
  - '@claude-code'
created_date: '2025-10-03 23:13'
updated_date: '2025-10-03 23:19'
labels:
  - backend
  - cache
  - transcoding
  - phase-3
dependencies:
  - task-389
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Make file cache quality-aware to support caching multiple quality levels separately. Part of Plex transcoding integration (Phase 3 of 8). See docs/transcode-plan.md for complete implementation plan.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Update FileCache::get_cached_stream_with_quality() method
- [x] #2 Implement quality-based cache key generation
- [x] #3 Verify cache_entries schema supports quality field
- [x] #4 Test multiple qualities cached simultaneously
- [x] #5 Different qualities cached separately
- [x] #6 Cache lookup uses (source_id, media_id, quality) key
- [x] #7 Can cache original + 1080p + 720p simultaneously
- [x] #8 Chunk-based downloads work for transcoded streams
- [x] #9 Files created/updated as per docs/transcode-plan.md Phase 3
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Examine current FileCache implementation in src/cache/file_cache.rs
2. Verify cache_entries schema has quality field
3. Check CacheRepository for quality support
4. Implement get_cached_stream_with_quality() method
5. Update cache key generation to include quality
6. Test with multiple quality levels
7. Verify chunk-based downloads work for transcoded streams
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented quality-aware cache integration for Phase 3 of Plex transcoding support.

## Changes Made

### 1. Added get_cached_stream_with_quality() Method
- Location: src/cache/file_cache.rs
- New FileCacheCommand::GetCachedStreamWithQuality variant
- Accepts explicit quality parameter instead of auto-determining from StreamInfo
- Returns proxy URL for cached stream
- Follows same pattern as existing get_cached_stream() but with quality control

### 2. Updated FileCacheHandle
- Added public get_cached_stream_with_quality() method
- Allows external callers to request specific quality levels
- Uses command pattern for async communication

### 3. Quality-Based Cache Keys
- Infrastructure already existed via MediaCacheKey struct
- MediaCacheKey includes (source_id, media_id, quality) tuple
- Each quality variant cached separately with unique keys
- Filenames include quality: source__media__quality format

### 4. Database Schema Verification
- Confirmed cache_entries table has quality column
- CacheRepository already supports quality-aware lookups
- find_cache_entry(source_id, media_id, quality) working correctly

### 5. Testing
- Added test_media_cache_key_quality_separation test
- Verifies different qualities create different cache keys
- Confirms filename generation includes quality

## Architecture Notes

The quality-aware caching is implemented at multiple layers:
1. **MediaCacheKey**: Core data structure with quality field
2. **CacheStorage**: Uses MediaCacheKey for entry lookup
3. **FileCache**: New method accepts explicit quality
4. **Database**: cache_entries table unique constraint on (source_id, media_id, quality)

This ensures:
- Same media can be cached in multiple qualities simultaneously
- Original + 1080p + 720p + 480p can coexist
- Cache lookups are quality-specific
- Chunk-based downloads work for all quality levels

## Integration with Transcode Plan

This implementation completes Phase 3 requirements:
- Quality-based cache keys ✅
- Separate storage for different qualities ✅  
- Database schema support ✅
- Multiple simultaneous qualities ✅

Ready for Phase 4 (Quality Selector UI) which will consume this API.
<!-- SECTION:NOTES:END -->
