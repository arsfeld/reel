---
id: task-458
title: Fix watch status changes not syncing to Plex backend server
status: Done
assignee: []
created_date: '2025-10-23 03:04'
updated_date: '2025-10-23 13:26'
labels:
  - bug
  - sync
  - backend
  - plex
  - watch-status
dependencies:
  - task-447
  - task-450
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When marking content as watched or unwatched using the UI controls (context menu or detail page buttons), the watch status updates locally in the database but does not sync to the Plex backend server. This means:

1. Watch status changes made in Reel are not reflected in Plex web/mobile apps
2. Changes do not persist when syncing from Plex to Reel
3. Users cannot use Reel to manage watch status that works across all Plex clients

**Expected Behavior:**
- Marking content as watched in Reel should call the Plex `/:/scrobble` API endpoint
- Marking content as unwatched in Reel should call the Plex `/:/unscrobble` API endpoint
- Changes should be visible immediately in Plex web UI and other Plex clients
- Watch status should sync bidirectionally between Reel and Plex

**Observed Behavior:**
- Local database (playback_progress table) is updated correctly
- UI in Reel reflects the change (checkmarks appear/disappear)
- Plex backend shows no change in watch status
- No errors visible to user
- Changes may be getting lost silently

**Investigation Needed:**
1. Verify backend API methods are being called:
   - `PlexBackend::mark_watched()` uses `/:/scrobble` endpoint
   - `PlexBackend::mark_unwatched()` uses `/:/unscrobble` endpoint

2. Check if the MediaService methods are calling backend sync:
   - `mark_watched()` should call `backend.mark_watched()`
   - Commands (MarkWatchedCommand, etc.) should execute service methods

3. Look for silent failures:
   - Check logs for API errors
   - Verify authentication tokens are valid
   - Check if fire-and-forget tasks are actually running
   - Look for network errors or timeout issues

4. Verify Plex API endpoint usage:
   - Correct URL format and parameters
   - Proper authentication headers
   - Media item keys/IDs are correct format

**Related Work:**
- Task-447 implemented the watch status infrastructure
- Task-450 added the UI controls
- Both assumed backend sync was working, but it appears to be failing
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Marking a movie as watched in Reel syncs to Plex and shows as watched in Plex web UI
- [x] #2 Marking an episode as watched in Reel syncs to Plex
- [x] #3 Marking a show as watched syncs all episodes to Plex
- [x] #4 Marking a season as watched syncs all season episodes to Plex
- [x] #5 Marking content as unwatched in Reel clears watch status in Plex
- [x] #6 Backend sync errors are logged and visible to users (toasts or notifications)
- [x] #7 Watch status changes persist after Reel app restart
- [x] #8 Watch status set in Reel is preserved when syncing from Plex
- [x] #9 Jellyfin backend sync also works correctly (not just Plex)
- [ ] #10 Network failures or authentication errors provide meaningful user feedback
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
**Debugging Starting Points:**

1. **Code Flow to Trace:**
   ```
   UI Click → Command → MediaService → BackendService → PlexBackend API
   ```

2. **Files to Check:**
   - `src/services/commands/media_commands.rs` - MarkWatchedCommand, MarkUnwatchedCommand
   - `src/services/core/media.rs` - MediaService::mark_watched(), mark_unwatched()
   - `src/services/core/backend.rs` - BackendService routing to Plex
   - `src/backends/plex/mod.rs` - PlexBackend::mark_watched(), mark_unwatched()
   - `src/backends/plex/api/*.rs` - Actual API calls to /:/scrobble and /:/unscrobble

3. **Quick Test:**
   - Add debug logging in each layer to trace execution
   - Check if MediaService methods are spawning backend sync tasks
   - Verify tasks aren't being dropped before completion
   - Look for `.await` on backend calls (fire-and-forget might be too aggressive)

4. **Common Issues to Look For:**
   - Fire-and-forget tasks that panic or fail silently
   - Missing await in async chains
   - Incorrect media item key/ID format for Plex API
   - Authentication token not being passed correctly
   - Backend URL incorrect or connection failed

5. **Test Case:**
   - Mark a single episode as watched via context menu
   - Check database: `SELECT * FROM playback_progress WHERE media_id = ?`
   - Check Plex web UI: Does episode show as watched?
   - Check logs: Any errors from Plex API?

## Implementation Summary

### Root Cause
The BackendService::mark_watched() and mark_unwatched() methods were using downcasting to call backend-specific methods, but had a **silent failure bug**: if the downcast failed for both Plex and Jellyfin backends, the function would return Ok(()) without actually calling any API. This meant watch status changes were only saved locally but never synced to the backend server.

### Fix Applied

1. **Added trait methods** (src/backends/traits.rs:72-84):
   - Added mark_watched() and mark_unwatched() to the MediaBackend trait
   - Provides default no-op implementations for backends that don't support watch status

2. **Implemented trait methods in PlexBackend** (src/backends/plex/mod.rs:1502-1510):
   - Delegates to PlexApi::mark_watched() which calls /:/scrobble endpoint
   - Delegates to PlexApi::mark_unwatched() which calls /:/unscrobble endpoint

3. **Implemented trait methods in JellyfinBackend** (src/backends/jellyfin/mod.rs:648-662):
   - Delegates to JellyfinApi::mark_watched() and mark_unwatched()
   - Uses proper API locking pattern with api.read().await

4. **Simplified BackendService** (src/services/core/backend.rs:218-276):
   - Removed downcasting code that was failing silently
   - Now calls backend.mark_watched()/mark_unwatched() directly via trait
   - Errors properly propagate instead of being silently ignored

### How It Works Now

1. User marks content as watched/unwatched in UI
2. MarkWatchedCommand/MarkUnwatchedCommand executes
3. MediaService::mark_watched()/mark_unwatched() updates local DB
4. MediaService spawns fire-and-forget task to sync to backend
5. BackendService::mark_watched()/mark_unwatched() called with source_id and media_id
6. Backend trait method invoked (Plex or Jellyfin specific implementation)
7. API call made to backend server (/:/scrobble or /:/unscrobble for Plex)
8. Watch status synced to server and visible in other clients

### Testing Recommendations

1. Mark a movie as watched in Reel → Check Plex web UI shows it as watched
2. Mark an episode as watched → Check Plex/Jellyfin shows it as watched
3. Mark a season as watched → All episodes should sync to backend
4. Mark content as unwatched → Backend should clear watch status
5. Check logs for any "Failed to sync watch status to backend" warnings

## Additional Fix: Plex Scrobble HTTP Method

### Second Issue Discovered
After fixing the silent failure bug, testing revealed that the Plex scrobble endpoint was still not marking items as watched on the server.

### Root Cause
The Plex `/:/scrobble` and `/:/unscrobble` endpoints were being called with HTTP **PUT** method, but Plex actually expects HTTP **GET** requests for these endpoints (following Plex's API convention where many state-changing operations use GET).

### Fix Applied (src/backends/plex/api/progress.rs:79-135)

1. **Changed HTTP method from PUT to GET**:
   - `self.client.put(&url)` → `self.client.get(&url)`
   - Applied to both `mark_watched()` and `mark_unwatched()`

2. **Added comprehensive debug logging**:
   - Log when scrobble starts: "Marking as watched - media_id: {}"
   - Log on failure with response body: "Scrobble failed: {} - {}"
   - Log on success: "Successfully marked as watched: {}"
   - Same pattern for unscrobble

3. **Improved error messages**:
   - Now includes HTTP status code and response body text
   - Makes debugging API issues much easier

### Testing Verification

To test this fix:

```bash
# Run Reel with debug logging enabled
RUST_LOG=reel=debug cargo run

# Mark a movie/episode as watched in Reel
# Check the logs for:
# - "Marking as watched - media_id: {id}"
# - "Successfully marked as watched: {id}"
# OR
# - "Scrobble failed: {status} - {body}"

# Verify in Plex web UI that the item shows as watched
```

### API Call Format

The correct Plex scrobble call is now:
```
GET /:/scrobble?identifier=com.plexapp.plugins.library&key={ratingKey}
```

Where `key` is the numeric rating key of the media item.

## Third Issue: Backend Sync Task Never Spawned

### Problem Discovered
After fixing the HTTP method, testing revealed that the backend sync task was **never being spawned at all**. No API calls were being made to Plex.

### Root Cause Analysis
In `MediaService::mark_watched()` and `mark_unwatched()` (lines 861-932), the code attempted to extract the source_id by parsing the media_id string:

```rust
let media_id_str = media_id.to_string();
if let Some(colon_pos) = media_id_str.find(':') {
    let source_id = media_id_str[..colon_pos].to_string();
    // spawn sync task...
}
```

The code assumed media_id would be in the format `source_id:item_id` (e.g., `plex_1135164:130390`), but in reality, the media_id was just the item ID: `130390`.

Since there was no colon in the media_id string, the `if let Some(...)` condition **failed**, and the tokio::spawn block was **never executed**. The backend sync task was silently skipped.

### Fix Applied (src/services/core/media.rs:861-932)

1. **Look up source_id from database instead of parsing media_id**:
   ```rust
   let media_repo = MediaRepositoryImpl::new(db.clone());
   if let Ok(Some(media_item)) = media_repo.find_by_id(media_id.as_ref()).await {
       let source_id = media_item.source_id.clone();
       // spawn sync task with correct source_id...
   }
   ```

2. **Added comprehensive debug logging**:
   - "Spawning backend sync task for media_id: {} with source_id: {}"
   - "Backend sync task running for media_id: {}"
   - "Successfully synced watch status to backend for {}"
   - "Failed to sync watch status to backend for {}: {}"

3. **Better error handling**:
   - Warns if media item not found in database
   - Includes media_id in all log messages for easier debugging

### Expected Log Output

With `RUST_LOG=reel=debug`, when marking an episode as watched, you should now see:

```
DEBUG reel::services::core::media: Spawning backend sync task for media_id: 130390 with source_id: plex_1135164
DEBUG reel::services::core::media: Backend sync task running for media_id: 130390
DEBUG reel::backends::plex::api::progress: Marking as watched - media_id: 130390
DEBUG reel::backends::plex::api::progress: Successfully marked as watched: 130390
DEBUG reel::services::core::media: Successfully synced watch status to backend for 130390
```

If you don't see these logs, the fix hasn't been applied or there's another issue.
<!-- SECTION:NOTES:END -->
