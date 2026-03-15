---
title: "feat: Offline Downloads from Plex (Infuse-style)"
type: feat
status: completed
date: 2026-03-15
origin: docs/brainstorms/2026-03-14-reel-media-center-brainstorm.md
---

# Offline Downloads from Plex (Infuse-style)

## Overview

Implement a complete offline download system so users can download media from their Plex server and play it without a network connection — matching the Infuse experience. Users browse their Plex library, tap "Download" on any movie or episode, monitor progress, and later play downloaded items from local disk even when fully offline.

## Problem Statement / Motivation

The brainstorm lists "Offline sync — Download media from Plex for offline viewing" as a core feature (see brainstorm: `docs/brainstorms/2026-03-14-reel-media-center-brainstorm.md`). The existing codebase has skeleton infrastructure (SQLite `downloads` table, `Downloader` queue manager, placeholder UI) but zero actual download execution — the HTTP client can't stream large files, there's no background worker, and the downloads view is an empty placeholder.

## Proposed Solution

Build a streaming download engine on a background thread, wire it into the existing queue manager, add download initiation from the detail view, populate the downloads list view with live progress, and integrate local-file-preferred playback.

**Scope boundaries for v1:**
- **Original quality only** — download the file as-is from Plex via direct play URL. No transcoded downloads (avoids undocumented Plex sync API complexity).
- **Individual downloads only** — no "Download Season" batch operations yet.
- **GTK4/Linux only** — macOS AppKit downloads deferred to the macOS frontend phase.
- **No storage quota UI** — show total usage, but no configurable limit yet.

## Technical Considerations

### Architecture

Downloads run on a dedicated background thread managed by `Downloader`. The thread monitors the SQLite queue, starts downloads up to `max_concurrent` (2), streams bytes to disk via chunked HTTP reads with `Range` header support for resume, and writes progress to SQLite. The GTK main thread polls the database on a 500ms GLib timer to update the downloads view.

```
GTK Main Thread                    Download Worker Thread
┌──────────────┐                  ┌──────────────────────┐
│ Downloads UI │◄── poll 500ms ──│ Downloader.run()     │
│ Detail View  │                  │  ├─ dequeue next     │
│ Play Button  │                  │  ├─ HTTP GET + Range │
│              │                  │  ├─ write chunks     │
│              │                  │  ├─ update SQLite    │
│              │                  │  └─ loop             │
└──────┬───────┘                  └──────────────────────┘
       │                                    │
       └────────── SQLite (WAL) ────────────┘
```

**Why polling over signals:** SQLite WAL mode handles concurrent readers/writers safely. A 500ms poll is simpler than cross-thread signaling, avoids GTK thread-safety issues, and is fast enough for smooth progress bar updates. This matches the existing pattern where the scanner thread writes to SQLite and the UI reads independently.

### Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Download quality | Original only (v1) | Direct play URL is simple; transcoded downloads require undocumented Plex sync API |
| URL construction | Reconstruct at download-start time | Avoids stale auth tokens in stored URLs |
| Resume strategy | HTTP Range from `downloaded_bytes` | Plex supports `Accept-Ranges: bytes` for direct play files |
| Progress reporting | GTK timer polls SQLite | Thread-safe, simple, matches existing patterns |
| File naming | `{media_item_id}_{sanitized_title}.{ext}` | Unique by ID, human-readable |
| Retry policy | 3 retries, exponential backoff (5s/30s/120s) | Handles transient network issues without infinite loops |
| App restart | Reset `downloading` → `queued`, auto-resume | Matches Infuse behavior |

### Security Consideration

The current `source_url` column embeds `X-Plex-Token` in the URL. This plan stores `part_key` and `server_id` instead, reconstructing the full URL with the current auth token at download time. This avoids persisting auth credentials in the downloads table.

## Acceptance Criteria

### Core Download Flow
- [x] "Download" button appears on detail view for Plex media items
- [x] Tapping "Download" enqueues the item (with duplicate prevention)
- [x] Downloads execute in the background, streaming chunks to disk
- [x] Download progress (bytes downloaded / total bytes) updates in the database
- [x] Max 2 concurrent downloads; additional queue as `queued`
- [x] When one completes/fails, the next queued item starts automatically

### Resume & Resilience
- [x] Pausing a download stops the HTTP connection, preserves the partial file
- [x] Resuming sends `Range: bytes={downloaded_bytes}-` header
- [x] On app restart, `downloading` status resets to `queued` and auto-resumes
- [x] Transient failures retry 3 times with exponential backoff before marking `failed`
- [x] Failed downloads store an error message (new `error_message` column)

### Offline Playback
- [x] Pressing Play on a Plex item with a completed download plays the local file
- [x] Local file is preferred over streaming when both are available
- [x] Downloaded items are playable with no network connection
- [x] Artwork for downloaded items is pinned in the image cache (not LRU-evicted)

### Downloads View (GTK)
- [x] Shows all downloads grouped by status (downloading, queued, paused, complete, failed)
- [x] Active downloads show: title, progress bar, percentage, download speed, file size
- [x] Completed downloads show: title, file size, play button
- [x] Failed downloads show: title, error message, retry button
- [x] Pause/resume/delete actions per download
- [x] Total storage used by downloads displayed in view header
- [x] "No Downloads" empty state when list is empty (existing)

### Disk Space
- [x] Real disk space check via `statvfs` before starting a download
- [x] Refuse to start if insufficient space (require file size + 10% margin)
- [x] Graceful handling when disk fills mid-download (pause + set failed with message)

### Database Migration (v3)
- [x] Add `error_message TEXT` column to `downloads`
- [x] Add `part_key TEXT` column to `downloads`
- [x] Add index on `downloads(status)`
- [x] Add index on `downloads(media_item_id)`
- [x] Stop storing auth token in `source_url`

### C ABI Exports
- [x] `reel_download_enqueue` — enqueue a download by media item ID
- [x] `reel_download_pause` / `reel_download_resume` — control download state
- [x] `reel_download_remove` — remove a download (optionally delete file)
- [ ] `reel_download_list` — list downloads by status
- [x] `reel_download_get_local_path` — check if a media item has a completed download

## Implementation Phases

### Phase 1: Streaming HTTP & Schema Migration

**Goal:** Build the foundation — a streaming HTTP download function and the schema updates.

**Files:**
- `src/net/http.zig` — Add `downloadToFile()` method
- `src/core/database.zig` — Migration v3
- `src/core/types.zig` — Update `Download` struct with `error_message`, `part_key`

**Details:**

Add to `HttpClient`:
```zig
// src/net/http.zig
pub fn downloadToFile(
    self: *HttpClient,
    url: []const u8,
    file_path: []const u8,
    resume_from: u64,  // byte offset for Range header
    headers: []const Header,
    progress_cb: ?*const fn (downloaded: u64, total: u64) void,
) HttpError!void
```

- Use `std.http.Client` with chunked reading (8KB buffer)
- Send `Range: bytes={resume_from}-` when `resume_from > 0`
- Open file with `std.fs.File` in append mode (or create)
- Write chunks in a loop, call `progress_cb` every ~100KB
- Handle 200 (full file) and 206 (partial content) responses
- Return errors for 401 (auth expired), 404, disk write failures

Schema migration v3:
```sql
ALTER TABLE downloads ADD COLUMN error_message TEXT;
ALTER TABLE downloads ADD COLUMN part_key TEXT;
CREATE INDEX idx_downloads_status ON downloads(status);
CREATE INDEX idx_downloads_media_item_id ON downloads(media_item_id);
```

**Success criteria:**
- [ ] Can download a 1GB+ file to disk with progress reporting
- [ ] Resume from byte offset works (206 response)
- [ ] Migration v3 applies cleanly on existing databases

### Phase 2: Download Worker Thread

**Goal:** Background thread that drives the download queue.

**Files:**
- `src/core/downloader.zig` — Add worker thread, URL reconstruction, retry logic

**Details:**

Add to `Downloader`:
```zig
// src/core/downloader.zig
pub fn start(self: *Downloader, http_client: *HttpClient, plex_client: *PlexClient) !void
// Spawns background thread

pub fn stop(self: *Downloader) void
// Signals thread to stop, joins

fn workerLoop(self: *Downloader) void
// Main loop: dequeue, download, update status, repeat
```

Worker loop logic:
1. Query `downloads WHERE status = 'queued' ORDER BY created_at ASC LIMIT 1`
2. If `activeCount() >= max_concurrent`, sleep 1s and retry
3. Set status to `downloading`
4. Reconstruct URL: call `plex_client.getStreamUrl(part_key)` using current auth token from `servers` table
5. Call `http_client.downloadToFile()` with resume offset from `downloaded_bytes`
6. On progress callback: `updateProgress(id, downloaded, total)` in SQLite
7. On success: set status `complete`, set `completed_at`
8. On failure: increment retry count, backoff, or set `failed` with `error_message`
9. Loop

On init: reset any `downloading` entries to `queued` (crashed downloads from last session).

Add duplicate prevention to `enqueue()`:
```zig
// Check existing download for this media_item_id
const existing = self.getByMediaItemId(media_item_id);
if (existing) |dl| {
    if (dl.status == .complete or dl.status == .downloading or dl.status == .queued) {
        return error.AlreadyExists;
    }
    // If failed/paused, re-enqueue by resetting status
}
```

**Success criteria:**
- [ ] Background thread starts on app launch, stops on shutdown
- [ ] Downloads execute automatically from the queue
- [ ] Max 2 concurrent downloads enforced
- [ ] Failed downloads retry 3 times then stay `failed`
- [ ] `downloading` entries reset to `queued` on restart

### Phase 3: Download Initiation & Playback Integration

**Goal:** "Download" button in detail view + prefer-local-file playback.

**Files:**
- `src/apprt/gtk/detail_view.zig` — Add "Download" button
- `src/apprt/gtk/app.zig` — Local file preference in playback path
- `src/core/downloader.zig` — Add `getCompletedLocalPath(media_item_id)`
- `src/core/library.zig` — Helper to resolve part_key for a media item

**Details:**

Detail view changes:
- Add a download button (icon: `folder-download-symbolic`) next to the Play button
- Only show for Plex-sourced items (`media_item.source == .plex`)
- On click: resolve `part_key` from the media item, call `downloader.enqueue(media_item_id, server_id, part_key, filename)`
- If already downloaded: show "Downloaded" badge instead of button, or offer "Delete Download"
- Disable button while enqueuing (prevent double-tap)

Playback integration in `app.zig`:
```zig
// In startPlexPlayback or equivalent
fn resolvePlaybackPath(self: *App, media_item_id: i64, plex_stream_url: []const u8) []const u8 {
    // Check for completed local download
    if (self.downloader.getCompletedLocalPath(media_item_id)) |local_path| {
        // Verify file still exists on disk
        if (std.fs.accessAbsolute(local_path, .{})) |_| {
            return local_path;
        } else |_| {
            // File was deleted externally, clean up DB
            self.downloader.setStatus(download_id, .failed);
        }
    }
    return plex_stream_url;
}
```

**Success criteria:**
- [ ] Download button appears on Plex media detail views
- [ ] Tapping download enqueues the item (feedback shown)
- [ ] Duplicate downloads are prevented
- [ ] Playing a downloaded item uses the local file
- [ ] Playing works when offline (no Plex API calls needed)

### Phase 4: Downloads View UI

**Goal:** Replace the placeholder downloads view with a live, interactive list.

**Files:**
- `src/apprt/gtk/downloads_view.zig` — Full rewrite

**Details:**

View layout:
```
┌─────────────────────────────────────────┐
│ Downloads                    12.4 GB used│
├─────────────────────────────────────────┤
│ ▶ The Matrix (1999)         [====  ] 67%│
│   2.1 GB / 3.2 GB • 15.2 MB/s    ⏸ 🗑 │
├─────────────────────────────────────────┤
│ ⏸ Inception (2010)          [==    ] 34%│
│   1.0 GB / 2.9 GB • Paused       ▶ 🗑 │
├─────────────────────────────────────────┤
│ ⏳ Interstellar (2014)      Queued       │
│   4.1 GB                          🗑    │
├─────────────────────────────────────────┤
│ ✓ Blade Runner 2049 (2017)  Complete     │
│   3.8 GB                     ▶ 🗑       │
├─────────────────────────────────────────┤
│ ✗ Dune (2021)               Failed       │
│   Connection lost            ↻ 🗑       │
└─────────────────────────────────────────┘
```

Implementation:
- GLib timeout source (500ms) polls `downloads` table, updates list
- Each row: `GtkBox` with title label, progress bar (active/paused), size label, status icon, action buttons
- Join `media_items` for title/year display
- Action buttons per status:
  - `downloading` → pause, delete
  - `paused` → resume, delete
  - `queued` → delete
  - `complete` → play, delete
  - `failed` → retry, delete
- Header shows total storage: `SELECT SUM(total_bytes) FROM downloads WHERE status = 'complete'`
- Calculate speed: `(current_bytes - last_poll_bytes) / poll_interval`
- Empty state: keep existing `AdwStatusPage` with "No Downloads"

**Success criteria:**
- [ ] All downloads shown with correct status and progress
- [ ] Progress bars update smoothly (~2 updates/second)
- [ ] Download speed calculated and displayed
- [ ] All action buttons work (pause/resume/retry/delete/play)
- [ ] Total storage displayed in header
- [ ] Empty state shown when no downloads exist

### Phase 5: Disk Space, Artwork Pinning & C ABI

**Goal:** Production hardening — real disk checks, artwork persistence, cross-platform API.

**Files:**
- `src/core/downloader.zig` — Real `checkDiskSpace` via `statvfs`
- `src/core/image_cache.zig` — Pin/unpin mechanism for download artwork
- `src/lib.zig` — Download C ABI exports
- `include/reel.h` — Download function declarations

**Details:**

Disk space check:
```zig
// src/core/downloader.zig
pub fn checkDiskSpace(path: []const u8, required_bytes: u64) bool {
    const stat = std.os.linux.statvfs(path);
    const available = stat.f_bavail * stat.f_frsize;
    return available > required_bytes + (required_bytes / 10); // 10% margin
}
```

Artwork pinning in image cache:
- Add `pinned BOOLEAN DEFAULT 0` to `image_cache` table (migration v3)
- On download complete: `UPDATE image_cache SET pinned = 1 WHERE url = ?` for the item's poster and backdrop URLs
- On download delete: `UPDATE image_cache SET pinned = 0 WHERE url = ?`
- Modify LRU eviction query: `WHERE pinned = 0 ORDER BY cached_at ASC`

C ABI exports for macOS frontend:
```c
// include/reel.h
ReelError reel_download_enqueue(ReelLibrary* lib, int64_t media_item_id);
ReelError reel_download_pause(ReelLibrary* lib, int64_t download_id);
ReelError reel_download_resume(ReelLibrary* lib, int64_t download_id);
ReelError reel_download_remove(ReelLibrary* lib, int64_t download_id, bool delete_file);
const char* reel_download_get_local_path(ReelLibrary* lib, int64_t media_item_id);
```

**Success criteria:**
- [ ] Downloads refuse to start when disk space is insufficient
- [ ] Disk-full mid-download sets status to `failed` with "Disk full" message
- [ ] Artwork for completed downloads survives LRU eviction
- [ ] Artwork unpinned when download is deleted
- [ ] All C ABI exports compile and are callable from Swift

### Phase 6: Edge Cases & Polish

**Goal:** Handle the long tail of failure modes.

**Files:** Various (across all download-related files)

**Details:**
- [ ] Handle 401 during download: log error, set `failed` with "Authentication expired" message
- [ ] Handle file deleted externally: on play attempt, detect missing file, update status to `failed`, show "File not found"
- [ ] Handle download directory inaccessible: check on startup, warn user in downloads view
- [ ] Verify downloaded file integrity: compare `downloaded_bytes` == `total_bytes` == actual file size on disk
- [ ] Clean up partial files when user deletes a failed/paused download
- [ ] Handle `Content-Length` unknown (chunked transfer): track bytes downloaded without percentage
- [ ] Default download path: create `$XDG_DATA_HOME/reel/downloads/` on first download if it doesn't exist
- [ ] Settings: add download path configuration to settings view

**Success criteria:**
- [ ] No crash or hang for any failure mode listed above
- [ ] User always sees a clear error message explaining what went wrong
- [ ] Orphaned partial files don't accumulate

## Dependencies & Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Plex doesn't support Range headers for some files | Low | High (no resume) | Detect missing `Accept-Ranges` header, fall back to full re-download |
| Auth token expires during multi-hour download | Medium | Medium | Detect 401, mark failed with clear message, user re-authenticates and retries |
| `std.http.Client` chunked reading has edge cases | Medium | High | Test with large files (10GB+), multiple servers, slow connections |
| SQLite contention between worker thread and UI polls | Low | Low | WAL mode handles this; mutex already exists for writes |
| Download directory on external/network drive | Low | Medium | Check accessibility on startup, fail gracefully |

## Success Metrics

- Downloads complete reliably for files up to 50GB
- Resume after pause/crash loses < 1MB of progress
- Downloaded items play instantly offline with no UI indicating they're different from streamed items
- Downloads view stays responsive during active downloads

## Sources & References

### Origin
- **Brainstorm document:** [docs/brainstorms/2026-03-14-reel-media-center-brainstorm.md](docs/brainstorms/2026-03-14-reel-media-center-brainstorm.md) — Key decisions: offline sync in scope, Plex as primary network source, SQLite for local storage

### Internal References
- Existing download schema: `src/core/database.zig:135-146`
- Downloader queue manager: `src/core/downloader.zig`
- Plex stream URL construction: `src/net/plex/client.zig:174-178`
- HTTP client (needs streaming extension): `src/net/http.zig`
- Image cache LRU pattern: `src/core/image_cache.zig`
- Downloads view placeholder: `src/apprt/gtk/downloads_view.zig`
- Detail view (needs download button): `src/apprt/gtk/detail_view.zig`
- Playback initiation: `src/apprt/gtk/app.zig`
- Master plan Phase 5: `docs/plans/2026-03-14-feat-reel-native-media-center-plan.md`

### External References
- Plex direct play URL format: `http://<server>:32400/library/parts/{id}/file.ext?X-Plex-Token={token}`
- HTTP Range requests: RFC 7233
- Zig `std.http.Client` chunked reading: `std.http.Client.Request.reader()`
- Linux `statvfs`: POSIX disk space check
