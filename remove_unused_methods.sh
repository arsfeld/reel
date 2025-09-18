#!/bin/bash

# List of methods to find and report
methods=(
    "is_initialized"
    "is_playback_ready"
    "mark_watched"
    "mark_unwatched"
    "get_watch_status"
    "search"
    "find_next_episode"
    "get_last_sync_time"
    "supports_offline"
    "fetch_media_markers"
    "fetch_episode_markers"
    "get_backend_info"
    "get_library_items"
    "get_music_albums"
    "get_music_tracks"
    "get_photos"
)

for backend in src/backends/*/mod.rs; do
    echo "=== $backend ==="
    for method in "${methods[@]}"; do
        grep -n "async fn $method\|fn $method" "$backend" && echo "  Found: $method"
    done
    echo
done
