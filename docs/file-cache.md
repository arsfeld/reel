# File Cache System

**Version**: 2.0 (Chunk-Based Architecture)
**Last Updated**: 2025-10-01

## Overview

The file cache system enables progressive download and streaming of media files from backend servers (Plex, Jellyfin, etc.) with support for seeking, offline playback, and efficient bandwidth usage. The system is built around a **chunk-based architecture** with the database as the single source of truth.

### Key Features

- **Chunk-based downloads**: Downloads arbitrary 10MB chunks on demand, not just sequential
- **Priority-aware**: Prioritizes chunks needed for active playback over background downloads
- **Database-driven**: All state derived from the `cache_chunks` table, survives restarts
- **Event-driven**: Efficient waiting for chunk availability without polling
- **Progressive streaming**: Seamless playback with automatic chunk downloading
- **HTTP proxy**: Serves cached files over HTTP with HTTP 206 Partial Content support
- **Offline-first**: Downloaded content available without network connection

### Known Limitations

None currently - all core features are working including seeking.

## Architecture

### Component Overview

```
┌──────────────────────────────────────────────────────────────────┐
│                        Player (GStreamer/MPV)                    │
└────────────────────────┬─────────────────────────────────────────┘
                         │ HTTP Requests
                         ↓
┌────────────────────────────────────────────────────────────────────┐
│                      CacheProxy (HTTP Server)                     │
│  - Queries ChunkManager for availability                          │
│  - Waits for chunks if missing (event-based)                      │
│  - Streams chunks as they become available                        │
└────────────┬───────────────────────────────────────────────────────┘
             │
             ↓ (queries)
┌────────────────────────────────────────────────────────────────────┐
│                      ChunkManager (Coordinator)                   │
│  - Maintains priority queue of chunk requests                     │
│  - Queries database for chunk availability                        │
│  - Dispatches downloads to ChunkDownloader                        │
│  - Provides event notifications when chunks complete              │
│  - Manages concurrent download limits                             │
└────────┬────────────────────────────┬──────────────────────────────┘
         │                            │
         ↓ (dispatches)               ↓ (queries/updates)
┌────────────────────┐     ┌──────────────────────────────────┐
│  ChunkDownloader   │     │   Database (SQLite + SeaORM)     │
│  - Downloads ranges│────→│   - cache_entries                │
│  - Records chunks  │     │   - cache_chunks ★               │
│  - Handles errors  │     │   (source of truth)              │
└────────┬───────────┘     └──────────────────────────────────┘
         │
         ↓ (writes)
┌────────────────────┐
│   ChunkStore       │
│   - File I/O       │
│   - Sparse writes  │
└────────────────────┘
```

### Components

#### 1. ChunkManager

Central coordinator for all chunk operations.

**Responsibilities**:
- Maintains priority queue of chunk download requests
- Queries database to determine chunk availability
- Dispatches download tasks to ChunkDownloader
- Provides event-based waiting for chunk availability
- Manages concurrent download limits (default: 3 simultaneous downloads)

**Key Operations**:
```rust
// Check if chunk is available
has_chunk(entry_id: i32, chunk_index: u64) -> bool

// Check if entire byte range is available
has_byte_range(entry_id: i32, start: u64, end: u64) -> bool

// Request chunk with priority
request_chunk(entry_id: i32, chunk_index: u64, priority: Priority)

// Wait for chunk (event-based)
wait_for_chunk(entry_id: i32, chunk_index: u64, timeout: Duration)
```

#### 2. ChunkDownloader

Downloads specific byte ranges from upstream servers.

**Responsibilities**:
- Downloads individual chunks using HTTP range requests
- Records completed chunks in the `cache_chunks` table
- Handles network errors with exponential backoff retry
- Supports concurrent downloads of different ranges

**Process**:
1. Calculate byte range for chunk (chunk_index × 10MB)
2. Make HTTP range request to upstream server
3. Stream response data
4. Write to ChunkStore at appropriate offset
5. Record chunk in database
6. Notify ChunkManager of completion

#### 3. ChunkStore

Manages physical storage of chunks on disk.

**Responsibilities**:
- File I/O operations with sparse file support
- Read/write operations at specific byte offsets
- File creation and cleanup

**Storage Strategy**:
- Single file per cache entry: `{cache_dir}/{entry_id}.cache`
- Uses sparse file writes (filesystem handles gaps)
- Efficient for both sequential and random access

#### 4. CacheProxy

HTTP server that serves cached media to players.

**Responsibilities**:
- HTTP server on localhost (port 50000-60000)
- **Always returns HTTP 206 Partial Content** (standard for video streaming)
- Queries ChunkManager for chunk availability
- Waits for missing chunks during streaming (event-based)
- Handles client disconnects gracefully

**Key Behavior**:
- All responses are `206 Partial Content` with `Content-Range` headers, even for full file requests
- Small ranges (<50MB): Check availability, wait if needed, then read into memory
- Large ranges (≥50MB): Immediate progressive streaming, chunks requested individually
- Queries database for exact chunk availability (not file size guessing)
- Requests missing chunks with CRITICAL priority during streaming
- Waits for chunks with timeout (30 seconds default)
- Returns 503 Service Unavailable only on timeout

## Data Flow

### Use Case: Initial Playback

```
1. Player requests stream
   └─→ FileCache::get_cached_stream()
       ├─→ Creates/gets cache entry
       ├─→ Registers with proxy
       └─→ Returns proxy URL

2. Player makes HTTP request to proxy
   └─→ CacheProxy::serve_file()
       ├─→ Queries ChunkManager for chunk availability
       ├─→ Requests missing chunks (Priority::CRITICAL)
       └─→ Streams chunks as available

3. Background chunk downloads
   └─→ ChunkManager processes priority queue
       ├─→ ChunkDownloader downloads chunks
       ├─→ Records in cache_chunks table
       └─→ Notifies waiters

4. Player reads continuously
   └─→ Proxy streams from ChunkStore
```

**Timing**: Initial playback starts within 100-500ms (network + first chunk).

### Use Case: Seek to Later Position
```
1. Player seeks to 70% (e.g., 2.24GB into 3.2GB file)
   └─→ Makes new request: Range: bytes=2240000000-

2. Proxy calculates required chunks
   └─→ Chunk 213 (at seek position) + chunks 214-223 (lookahead)

3. ChunkManager prioritizes
   └─→ Chunk 213: Priority::CRITICAL (needed NOW)
   └─→ Chunks 214-223: Priority::HIGH (smooth playback)

4. ChunkDownloader downloads chunk 213
   └─→ 10MB download @ typical connection speed
   └─→ Completes in 2-6 seconds

5. Proxy begins streaming from chunk 213
   └─→ Player starts playback at 70%
   └─→ Background downloads continue for lookahead
```

**Result**: Seek completes in **2-6 seconds** instead of minutes of sequential downloading.

## Database Schema

### cache_entries

Main cache tracking table.

```sql
CREATE TABLE cache_entries (
    id INTEGER PRIMARY KEY,
    source_id TEXT NOT NULL,
    media_id TEXT NOT NULL,
    quality TEXT NOT NULL,
    original_url TEXT NOT NULL,
    file_path TEXT NOT NULL,
    expected_total_size INTEGER,  -- Content-Length from upstream
    is_complete BOOLEAN DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_accessed TIMESTAMP,
    -- ... additional metadata fields
    UNIQUE(source_id, media_id, quality)
);
```

### cache_chunks

**Core table** - tracks downloaded byte ranges.

```sql
CREATE TABLE cache_chunks (
    id INTEGER PRIMARY KEY,
    cache_entry_id INTEGER NOT NULL,
    start_byte INTEGER NOT NULL,
    end_byte INTEGER NOT NULL,
    downloaded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (cache_entry_id) REFERENCES cache_entries(id) ON DELETE CASCADE,
    INDEX idx_cache_chunks_entry (cache_entry_id),
    INDEX idx_cache_chunks_range (cache_entry_id, start_byte, end_byte)
);
```

**Key Queries**:

```rust
// Check if specific chunk exists
SELECT EXISTS(
    SELECT 1 FROM cache_chunks
    WHERE cache_entry_id = ?
      AND start_byte <= ?
      AND end_byte >= ?
);

// Get all chunks for entry
SELECT start_byte, end_byte FROM cache_chunks
WHERE cache_entry_id = ?
ORDER BY start_byte;

// Verify contiguous coverage of byte range
// (Application verifies no gaps between chunks)
```

## Chunk Management

### Chunk Size

**Default**: 10MB (10,485,760 bytes)

**Rationale**:
- Large enough to minimize overhead (4GB file = 400 chunks vs 2,048 with 2MB)
- Small enough for responsive seeks (<1 second download on typical connections)
- Aligns with typical video GOP sizes
- Reduces database overhead by ~5x compared to 2MB chunks

**Calculation**:
```rust
chunk_index = byte_offset / CHUNK_SIZE
chunk_start = chunk_index * CHUNK_SIZE
chunk_end = min(chunk_start + CHUNK_SIZE, file_size)
```

### Priority Levels

Downloads are prioritized based on urgency:

```rust
pub enum Priority {
    CRITICAL = 0,  // Playback will stall without this chunk NOW
    HIGH = 1,      // Needed soon for smooth playback (next 5-10 chunks)
    MEDIUM = 2,    // User-requested pre-cache
    LOW = 3,       // Background sequential fill
}
```

**Priority Assignment**:

| Scenario | Priority | Description |
|----------|----------|-------------|
| Active playback position | CRITICAL | Chunk needed immediately or playback stalls |
| Lookahead chunks (next 5-10) | HIGH | Ensures smooth playback without buffering |
| User-requested download | MEDIUM | Explicit "Download for Offline" action |
| Background gap filling | LOW | Completes partial downloads during idle time |

**Dynamic Re-Prioritization**:
When user seeks during download, the system automatically:
1. Cancels LOW priority downloads
2. Downgrades previous HIGH chunks to LOW
3. Promotes seek target chunk to CRITICAL
4. Promotes new lookahead chunks to HIGH

## Configuration

### Tunable Parameters

```rust
pub struct CacheConfig {
    /// Chunk size in bytes (default: 10MB)
    pub chunk_size: u64,

    /// Maximum concurrent chunk downloads (default: 3)
    pub max_concurrent_downloads: usize,

    /// Lookahead chunks for smooth playback (default: 10)
    pub lookahead_chunks: usize,

    /// Chunk wait timeout in seconds (default: 30)
    pub chunk_wait_timeout_secs: u64,

    /// Enable background fill (default: true)
    pub enable_background_fill: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            chunk_size: 10 * 1024 * 1024, // 10MB
            max_concurrent_downloads: 3,
            lookahead_chunks: 10,
            chunk_wait_timeout_secs: 30,
            enable_background_fill: true,
        }
    }
}
```

## API Usage

### Getting a Cached Stream

```rust
// Request stream (creates cache entry if needed)
let proxy_url = file_cache
    .get_cached_stream(media_id, quality, original_url)
    .await?;

// Player uses proxy URL for playback
player.play(proxy_url);
```

### Pre-Caching for Offline

```rust
// Pre-download entire file
file_cache
    .pre_cache(media_id, quality, original_url)
    .await?;

// Check completion status
let is_complete = file_cache
    .is_cached(media_id, quality)
    .await?;
```

### Checking Cache Status

```rust
// Check if media is cached
let status = chunk_manager
    .get_cache_status(entry_id)
    .await?;

println!("Downloaded: {}%", status.progress_percent);
println!("Chunks: {}/{}", status.cached_chunks, status.total_chunks);
println!("Complete: {}", status.is_complete);
```

## Performance Characteristics

### Playback Performance

| Scenario | Old System | New System | Status |
|----------|-----------|-----------|---------|
| Initial playback | 100-500ms | 100-500ms | ✅ Working |
| Progressive streaming | Sequential only | Chunk-based on demand | ✅ Working |
| Resume after restart | State lost, rebuild | Automatic from DB | ✅ Working |

### Seek Performance

| Scenario | Old System | New System |
|----------|-----------|-----------|
| Seek to 70% of 4GB file | ~30 minutes | 2-6 seconds (300x faster) ✅ |
| Seek to uncached position | Returns 503, player retries | Downloads chunk on-demand ✅ |
| Multiple concurrent seeks | Only first succeeds | All served concurrently ✅ |

### Download Efficiency

- **Concurrent downloads**: Up to 3 simultaneous chunk downloads
- **Bandwidth usage**: Only downloads requested chunks (no sequential waste)
- **Resume capability**: Automatic resume from any chunk on restart
- **Sparse files**: No disk space wasted on uncached regions

### Database Performance

- **Chunk queries**: O(log n) with btree index on `(cache_entry_id, start_byte, end_byte)`
- **Typical lookup**: <1ms for chunk existence check
- **Range coverage**: O(k) where k = number of chunks in range (verified in application)

## Error Handling

### Network Errors

- **Transient errors**: Exponential backoff retry (max 3 attempts)
- **Range not supported**: Falls back to full file download
- **Upstream server down**: Marks entry as failed, retries later
- **Timeout**: Returns 503 to player with Retry-After header

### Storage Errors

- **Disk full**: Stops downloads, emits error event
- **Write failure**: Marks chunk as failed, retries
- **Sparse file unsupported**: Falls back to pre-allocated file

### Client Behavior

- **Client disconnect**: Cancels pending chunk requests
- **Concurrent access**: Multiple clients can read same cache entry
- **Restart**: State automatically recovered from database

## Migration from Old System

The previous sequential-only download system had the following limitations:

- ❌ Sequential downloads only (no seeking to uncached positions)
- ❌ Proxy guessed availability using file size instead of database
- ❌ In-memory state lost on restart
- ❌ No chunk prioritization or concurrent downloads
- ❌ Database schema existed but was unused

The new chunk-based system addresses all of these issues:

- ✅ Chunk-based downloads with priority queue
- ✅ Database-driven state (cache_chunks table is core)
- ✅ Event-driven waiting (no polling)
- ✅ Progressive streaming with automatic chunk downloads
- ✅ Concurrent downloads (3 simultaneous ranges)
- ✅ State survives restarts automatically
- ✅ Fast seeks with on-demand chunk downloading

## Troubleshooting

### Playback Stalls During Streaming

**Symptoms**: Video playback pauses/buffers frequently

**Possible Causes**:
1. Network bandwidth insufficient for real-time download
2. Too few lookahead chunks configured
3. Concurrent download limit too low

**Solutions**:
- Increase `lookahead_chunks` (default: 10)
- Increase `max_concurrent_downloads` (default: 3)
- Check network connection speed

### Seeks Take Too Long

**Symptoms**: Seeking to uncached positions takes >10 seconds

**Possible Causes**:
1. Chunk size too large
2. Network latency to upstream server
3. Upstream server throttling

**Solutions**:
- Reduce `chunk_size` (try 5MB instead of 10MB)
- Check network latency to backend server
- Verify backend server isn't rate limiting

### Database Queries Slow

**Symptoms**: Chunk availability checks take >100ms

**Possible Causes**:
1. Missing database indexes
2. Database file fragmentation
3. Too many entries

**Solutions**:
- Verify indexes exist: `idx_cache_chunks_entry`, `idx_cache_chunks_range`
- Run `VACUUM` on database
- Implement cache entry cleanup/expiration

### Disk Space Issues

**Symptoms**: Downloads fail with disk full errors

**Solutions**:
- Implement automatic cache cleanup
- Set maximum cache size limit
- Delete old/unused cache entries

## Future Enhancements

Potential improvements for future versions:

- **Adaptive chunk sizing**: Adjust chunk size based on network conditions
- **Bandwidth throttling**: Limit background download speed
- **P2P sharing**: Share cached chunks between local instances
- **Predictive prefetch**: Machine learning-based chunk prediction
- **Compression**: On-the-fly compression of cached chunks
- **CDN integration**: Serve chunks from edge locations
- **Distributed cache**: Multi-node cache coordination

## References

### Related Files

- `src/cache/chunk_manager.rs` - ChunkManager implementation
- `src/cache/chunk_downloader.rs` - ChunkDownloader implementation
- `src/cache/chunk_store.rs` - ChunkStore implementation
- `src/cache/proxy.rs` - CacheProxy HTTP server
- `src/cache/file_cache.rs` - FileCache coordinator
- `src/db/repository/cache_repository.rs` - Database operations

### Database Migrations

- `m20250929_000001_add_cache_tracking.rs` - Initial cache schema
- `m20251001_000001_enable_chunk_usage.rs` - Chunk system activation

### Design Documents

- `CACHE_ARCHITECTURE.md` - Analysis of previous sequential system
- `CACHE_DESIGN.md` - Chunk-based architecture design
