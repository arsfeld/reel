# Backend Methods to Remove

## Methods that are NOT in trait anymore:
- is_initialized
- is_playback_ready
- mark_watched
- mark_unwatched
- get_watch_status
- search
- find_next_episode
- get_last_sync_time
- supports_offline
- fetch_media_markers
- fetch_episode_markers
- get_backend_info
- get_library_items
- get_music_albums
- get_music_tracks
- get_photos

## Files to fix:
1. src/backends/jellyfin/mod.rs
2. src/backends/plex/mod.rs
3. src/backends/local/mod.rs
4. tests/common/mocks.rs (if it has a mock backend)