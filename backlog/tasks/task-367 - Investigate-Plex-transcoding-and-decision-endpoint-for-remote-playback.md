---
id: task-367
title: Investigate Plex transcoding and decision endpoint for remote playback
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 16:09'
updated_date: '2025-10-03 21:28'
labels:
  - research
  - plex
  - streaming
  - documentation
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Remote Plex connections cannot directly stream media files - they require going through the Plex transcoding/decision endpoint even for direct play scenarios. Research how Plex's /video/:/transcode/universal/decision endpoint works, what parameters it requires, and how to properly request direct play streams through it.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Document Plex decision endpoint API structure and required parameters
- [x] #2 Identify difference between local direct URLs and transcode endpoint URLs
- [x] #3 Research how Plex clients request direct play vs transcoded streams
- [x] #4 Document how to construct proper decision endpoint requests for direct play
- [x] #5 Identify what response format the decision endpoint returns
- [x] #6 Create plan for implementing decision endpoint support in stream URL generation
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Research Plex decision endpoint API from web sources
2. Analyze current PlexBackend stream URL implementation
3. Document decision endpoint structure, parameters, and response
4. Identify local vs remote connection differences
5. Create implementation recommendations
6. Document findings in task notes
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
# Plex Transcode Decision Endpoint Research

## Summary
Remote Plex connections require using the `/video/:/transcode/universal/decision` or `/video/:/transcode/universal/start` endpoints even for direct play scenarios. Direct file URLs (like `/library/metadata/{id}/file`) only work on local network connections.

## 1. Decision Endpoint API Structure

### Endpoint Format
```
GET /video/:/transcode/universal/decision
```

### Required Query Parameters
- **path**: Path to the media item (e.g., `/library/metadata/{rating_key}`)
- **protocol**: Streaming protocol (e.g., `http`, `hls`, `dash`)
- **hasMDE**: Whether media item has MDE (typically `1`)
- **mediaIndex**: Index of the media item to transcode (typically `0`)
- **partIndex**: Index of the part to transcode (typically `0`)

### Optional Parameters (for control)
- **directPlay**: Whether to allow direct play (`0` or `1`)
- **directStream**: Whether to allow direct stream (`0` or `1`)
- **fastSeek**: Whether to use fast seek (`0` or `1`)
- **location**: Connection context (`lan` for local, `wan` for remote)
- **maxVideoBitrate**: Maximum video bitrate in kbps
- **videoResolution**: Target resolution (e.g., `1920x1080`)
- **subtitleSize**: Subtitle size
- **audioBoost**: Audio boost level
- **session**: Session identifier for tracking

### Required Headers
Standard Plex headers are required:
- **X-Plex-Token**: Authentication token
- **X-Plex-Platform**: Platform name (e.g., Linux, MacOSX, Windows)
- **X-Plex-Platform-Version**: OS version
- **X-Plex-Provides**: Capabilities (e.g., player, controller)
- **X-Plex-Client-Identifier**: Unique device UUID
- **X-Plex-Product**: Application name
- **X-Plex-Version**: Application version
- **X-Plex-Device**: Device name and model

## 2. Local vs Remote Connection Differences

### Local (LAN) Connections
- Can use direct file URLs: `{base_url}/library/metadata/{id}/file?X-Plex-Token={token}`
- Lower latency
- No bandwidth restrictions
- Can access media parts directly
- Identified by `PlexConnection.local = true`

### Remote (WAN/Relay) Connections
- **MUST** use transcode/universal endpoints even for direct play
- May go through Plex Relay (limited to 2 Mbps)
- Require proper Remote Access configuration
- Cannot access direct file paths
- Identified by `PlexConnection.local = false`

### Current Implementation Issue
Our current `get_stream_url()` implementation in `src/backends/plex/api/streaming.rs:8` returns direct file URLs which only work for local connections. For remote connections, this fails because the direct path is not accessible.

## 3. Direct Play vs Transcoded Streams

### Direct Play (No Transcoding)
Plex clients request direct play by:
1. Calling decision endpoint with `directPlay=1&directStream=1`
2. Setting `maxVideoBitrate` to original or very high value
3. Not specifying `videoResolution` or setting it to original
4. Protocol typically `http` or `hls` for streaming

### Transcoded Streams
For transcoding:
1. Set `directPlay=0&directStream=0`
2. Specify target `maxVideoBitrate` (e.g., `4000` for 4 Mbps)
3. Specify `videoResolution` (e.g., `1280x720`)
4. Protocol typically `hls` or `dash`
5. Use `/video/:/transcode/universal/start.m3u8` or `start.mpd`

## 4. Constructing Decision Endpoint Requests

### For Direct Play (Remote Connection)
```
GET {base_url}/video/:/transcode/universal/decision?path=/library/metadata/{rating_key}&mediaIndex=0&partIndex=0&protocol=http&directPlay=1&directStream=1&hasMDE=1&location={connection_type}&X-Plex-Token={token}
```

### For Transcoding
```
GET {base_url}/video/:/transcode/universal/start.m3u8?path=/library/metadata/{rating_key}&mediaIndex=0&partIndex=0&protocol=hls&directPlay=0&directStream=0&fastSeek=1&maxVideoBitrate={bitrate_kbps}&videoResolution={width}x{height}&X-Plex-Token={token}
```

### URL Encoding
The `path` parameter should be URL-encoded when passed as query parameter.

## 5. Response Format

The decision endpoint returns information about how the stream should be handled:

### Response Fields (based on PlexAPI implementation)
- **decision**: Overall decision (e.g., "direct play", "transcode", "copy")
- **videoDecision**: Decision for video stream
- **audioDecision**: Decision for audio stream
- **MediaContainer**: Container with stream information

The actual stream URL is then constructed based on the decision response.

## 6. Implementation Plan

### Current State
- `PlexBackend` tracks connection type via `PlexConnection.local` and `.relay` flags
- `get_stream_url()` currently returns direct file URLs
- Works for local connections, fails for remote

### Recommended Changes

1. **Detect Connection Type**
   - Use `PlexConnection.local` flag from current connection
   - Store current connection type in `PlexBackend`
   - Available via `cached_connections` and `find_best_connection()`

2. **Modify `get_stream_url()` Logic**
   ```rust
   async fn get_stream_url(&self, media_id: &str) -> Result<StreamInfo> {
       let connection = self.get_current_connection().await?;
       
       if connection.local {
           // Use current direct file URL approach
           // Works fine for local connections
       } else {
           // Use decision/transcode endpoint
           // Required for remote/relay connections
       }
   }
   ```

3. **Add Decision Endpoint Method**
   ```rust
   pub async fn get_stream_url_via_decision(
       &self,
       media_id: &str,
       direct_play: bool,
   ) -> Result<StreamInfo> {
       let params = [
           ("path", format!("/library/metadata/{}", media_id)),
           ("mediaIndex", "0".to_string()),
           ("partIndex", "0".to_string()),
           ("protocol", "http".to_string()),
           ("directPlay", if direct_play { "1" } else { "0" }.to_string()),
           ("directStream", if direct_play { "1" } else { "0" }.to_string()),
           ("hasMDE", "1".to_string()),
           ("location", if is_local { "lan" } else { "wan" }.to_string()),
       ];
       
       // Call decision endpoint
       // Parse response
       // Return StreamInfo with proper URL
   }
   ```

4. **Fallback Strategy**
   - Try direct URL first for local connections (faster)
   - If fails, fall back to decision endpoint
   - Always use decision endpoint for remote connections

## References
- Python PlexAPI implementation: https://github.com/pkkid/python-plexapi/blob/master/plexapi/base.py
- Plex API Documentation: https://plexapi.dev/api-reference/video/start-universal-transcode
- Current implementation: `src/backends/plex/api/streaming.rs:8`
- Connection detection: `src/backends/plex/mod.rs:162` (find_best_connection)
<!-- SECTION:NOTES:END -->
