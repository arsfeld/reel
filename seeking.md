# GStreamer Seeking Investigation

## Problem
GStreamer seeking fails with HTTP proxy sources. When attempting to seek, we get:
- "Failed to seek" errors from both `seek()` and `seek_simple()` APIs
- Media reports as seekable: `true` with correct time range
- Seek events are created successfully (visible in GST debug logs)
- Segments update correctly in the pipeline
- But the actual seek operation returns failure

## Test Environment
- GStreamer with `curlhttpsrc` as HTTP source
- Media served through local cache proxy at `http://127.0.0.1:50000/stream/{id}`
- Cache proxy implements partial file serving with Range request support
- Media files are progressively downloaded and cached locally

## What We Tried

### 1. HTTP Proxy HEAD Request Support
**Hypothesis**: GStreamer needs HEAD requests to determine seekability.

**Implementation**:
- Added HEAD request handlers to cache proxy routes
- Returns `Accept-Ranges: bytes` header
- Returns proper `Content-Length` header

**Result**: ❌ No HEAD requests were ever made by GStreamer. Seeking still fails.

### 2. Force 206 Partial Content Response
**Hypothesis**: GStreamer needs to see 206 status to know the stream supports ranges.

**Implementation**:
- Changed proxy to always return `206 Partial Content` for non-range requests
- Include `Content-Range` header even for full file requests

**Result**: ✅ Fixed a panic with content-length mismatch, but ❌ seeking still fails.

### 3. Seek Flags Modification
**Hypothesis**: HTTP sources need different seek flags than local files.

**Implementations Tried**:
- Removed `SNAP_BEFORE` flag (can cause issues with HTTP sources)
- Used only `FLUSH | KEY_UNIT` flags
- Tried only `KEY_UNIT` flag without `FLUSH`
- Tried no flags at all

**Result**: ❌ All combinations failed with same error.

### 4. Pipeline State Management
**Hypothesis**: HTTP sources need pipeline to be fully ready (ASYNC_DONE) before seeking.

**Implementation**:
- Added `pipeline_ready` flag tracking
- Only allow seeks after receiving ASYNC_DONE message
- Verified pipeline is in PLAYING or PAUSED state before seeking

**Result**: ❌ Pipeline is ready, but seeking still fails.

### 5. Different Seek APIs
**Order Tried**:
1. `playbin.seek()` - Full API with explicit parameters
2. `playbin.seek_simple()` - Simplified API
3. `playbin.send_event()` - Direct event sending
4. Conservative seek without FLUSH flag

**Result**: ❌ All methods fail. Direct event returns `false`.

### 6. Pause Before Seeking
**Hypothesis**: Some HTTP sources can't seek while actively streaming.

**Implementation**:
- Pause pipeline before seeking
- Wait for pause to complete
- Perform seek
- Resume playback

**Result**: ❌ Seeking fails even when already paused.

### 7. Enable Download Buffering
**Hypothesis**: Progressive download buffering helps with HTTP seeking.

**Implementation**:
- Set playbin flags to enable download buffering (0x00000080)

**Result**: ❌ Caused type mismatch panic with GstPlayFlags. Removed.

### 8. Check Seeking Capability
**Implementation**:
- Query both TIME and BYTES format seeking support
- TIME seeking: `seekable=true`
- BYTES seeking: `seekable=false`

**Result**: Pipeline reports TIME seeking is supported, but seeks still fail.

## Debug Observations

### GStreamer Debug Logs Show:
```
INFO GST_EVENT gstevent.c:1374:gst_event_new_seek: creating seek rate 1.000000, format TIME, flags 5, start_type 1, start 0:24:06.624000000
INFO default gstsegment.c:386:gst_segment_do_seek: segment updated: time segment start=0:24:06.624000000
```
- Seek events ARE being created
- Segments ARE being updated
- But the seek operation returns failure

### Error Location:
All failures point to:
- `gstreamer-0.24.1/src/element.rs:606` (for `seek()`)
- `gstreamer-0.24.1/src/element.rs:629` (for `seek_simple()`)

These lines in the Rust bindings check the return value from the underlying GStreamer C API.

### Proxy Behavior:
- Initial GET request arrives without Range header
- No HEAD requests are made by GStreamer
- No Range requests are made when attempting to seek
- Proxy successfully serves the file content for playback

## Current Theory

The issue appears to be that **curlhttpsrc cannot perform seeks on our proxy URLs** despite the proxy claiming to support Range requests. Possible reasons:

1. **GStreamer expects specific HTTP behavior** that our proxy doesn't implement correctly
2. **The proxy breaks the seek chain** - original Plex/Jellyfin URLs might support seeking, but our proxy layer prevents it
3. **Missing HTTP features** - GStreamer might require specific HTTP/1.1 features like persistent connections for seeking
4. **Buffering interference** - The proxy's buffering strategy might interfere with GStreamer's seeking mechanism

## Next Steps to Try

1. **Test with original URLs**: Bypass the cache proxy entirely and use direct Plex/Jellyfin URLs to confirm if seeking works without the proxy

2. **Analyze curlhttpsrc source**: Look at the actual GStreamer curlhttpsrc element source code to understand what makes it reject seeks

3. **Implement proper Range forwarding**: When cache doesn't have the requested range, forward the Range request to the backend

4. **Check HTTP response headers**: Compare headers from working seekable HTTP sources vs our proxy

5. **Try souphttpsrc instead of curlhttpsrc**: Force GStreamer to use a different HTTP source element

6. **Implement a test HTTP server**: Create a minimal HTTP server that definitely supports seeking to test if the issue is proxy-specific

7. **Check if it's a playbin3 issue**: Try using playbin (v2) instead of playbin3

## Related Files
- `/src/player/gstreamer_player.rs` - GStreamer player implementation
- `/src/cache/proxy.rs` - HTTP cache proxy server
- `/src/cache/file_cache.rs` - Cache management logic

## Environment Details
- Platform: macOS (Darwin)
- GStreamer HTTP source: curlhttpsrc
- Playbin version: playbin3
- Cache proxy port: 50000