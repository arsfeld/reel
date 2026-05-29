# Reel

A modern, native media player for the Linux desktop. Reel targets the "Infuse for Linux" experience: a beautiful library UI, automatic metadata, first-class Plex and Jellyfin integration, server-side transcoding, offline downloads, and universal format playback through GStreamer.

Built with **Rust**, **GTK4**, **Relm4** (Elm/MVU), **libadwaita**, and **GStreamer**.

> App ID: `dev.arsfeld.Reel`

## Features

### Media sources

- **Plex Media Server** — full integration: authentication, libraries, collections, hubs, recently-added, continue-watching, metadata, child navigation, progress reporting, scrobble/unscrobble, skip markers, server-side transcode decisions, and offline downloads.
- **Jellyfin** — full integration: authentication, libraries, box sets, hubs (Latest / Next Up), metadata, child navigation, and play-session progress reporting.
- **Multiple sources at once** — connect several servers; the sidebar groups libraries per source and sources can be added or removed individually. Each source can be enabled/disabled.
- **Cross-source Home** — the Home page merges *Continue Watching* across every connected source, badged by the server it came from.

### Playback

Video plays through GStreamer `playbin3` with a `gtk4paintablesink`, wrapped by `PlaybackPipeline` (`src/player/gst_pipeline.rs`) and driven by the `VideoPlayer` Relm4 component.

- **Universal format support** via GStreamer.
- **Audio & subtitle track switching** — tracks are parsed from the GStreamer `StreamCollection` and labelled with language/codec; forced-subtitle detection; external subtitle file discovery. Preferred audio/subtitle language (ISO 639-1) is applied automatically from settings.
- **Resume playback** from the last saved position.
- **mpv-style on-screen controls** — seek bar, transport, current/total time, volume slider with mute, fullscreen toggle, and auto-hiding chrome.
- **Keyboard shortcuts** (mpv-compatible): Space/`k` play-pause, ←/→ ±5s, `j`/`l` ±10s, ↑/↓ ±60s, Home/`0` start, End, `m` mute, `9`/`0` volume, `f` fullscreen, Esc exit fullscreen.
- **Skip markers** — intro / credits ranges fetched from Plex (chapters) and Jellyfin (media segments).

### Transcoding & streaming

- **Plex server-side transcoding** — uses Plex's universal transcoder with a quality ladder (Original → 1080p·8Mbps → 720p·4Mbps → 480p·2Mbps). Auto mode caps bitrate when remote and runs uncapped when local; manual selection overrides Auto.
- **In-player quality menu** with a decision indicator showing whether the stream is Direct Play or transcoding (and at what resolution/bitrate), including mid-playback quality switching.
- **Transcode session lifecycle** — unique per-session IDs, keepalive pings during playback, and explicit teardown on stop/cleanup.
- **Stream read-ahead cache** — HTTP/HTTPS streams are buffered through GStreamer's `downloadbuffer`; the cached read-ahead region is drawn on the seek bar. Orphaned temp files are reclaimed at startup.

> Jellyfin playback currently uses direct play; Plex is the source with server-side transcode support.

### Library & browsing

- **Home page** with a hero carousel and horizontal shelves (hubs, Latest, Recently Added, Continue Watching, Next Up), with loading/empty/error/connecting states.
- **Library grid** with configurable poster density, grid/list view modes, per-library title and item count, and an in-grid search filter.
- **Filters & sorting** — sort by title (A–Z / Z–A), year, date added, rating, or runtime; filter by watch status, genre, content rating, year range, runtime range, video resolution, and HDR format. Active filters show as a removable pill bar. Filter + sort state is persisted per library.
- **Detail pages** for movies and shows — poster/backdrop, title, year, rating, runtime, overview, genres, technical metadata (resolution, codec, HDR, audio channels, container), season/episode navigation, and a watched-status toggle.
- **Library visibility** — libraries can be hidden from the sidebar.

### Offline downloads (Plex)

- **Download queue** with configurable concurrency (1–4, default 2), pause/resume, and byte-range resume with ETag validation.
- **Live progress** — byte counts and progress shown on a poster-card grid; a sidebar badge shows the active download count.
- **Storage budget** — optional size cap with automatic pruning of watched items when exceeded; configurable download folder.
- **Offline metadata sidecars** — poster, title, year, and season/episode are stored so downloads remain browsable offline.
- **Failure handling** — distinct reasons (network, disk full, auth expired, source file changed, file missing) with retry/remove actions.

> Downloads are currently supported for Plex sources only.

### Watch state & sync

- **Progress tracking** persisted locally (debounced), with a watched threshold at 90% of duration.
- **Plex is authoritative** for watch state (offset / view count); the local database is the fallback for other sources and offline playback.
- **Timeline & scrobble reporting** to Plex during playback; play-session reporting to Jellyfin.
- **Offline sync queue** — progress and watched events recorded while offline are queued and dispatched on reconnect.

### Desktop integration

- **MPRIS2** D-Bus server (`mpris-server`): playback status, track metadata and art, and remote control (play/pause/stop/seek/set-position/volume/open-uri/raise/quit).

## Tech stack

| Area | Library |
| --- | --- |
| UI framework | Relm4 0.11, GTK4 0.11, libadwaita 0.9 |
| Video | GStreamer 0.25 (`playbin3` + `gtk4paintablesink`) |
| HTTP / data | reqwest 0.12 (rustls), serde, serde_json, toml |
| Database | diesel 2.3 (bundled SQLite) |
| Async | tokio (multi-threaded), async-trait |
| Desktop | mpris-server 0.9 |

Data is stored in a SQLite database (media items, sources, watch progress, downloads, download groups, and a pending-sync queue). Settings persist as TOML in the platform config directory.

## Build & run

All build/run/test commands require the Nix dev shell, which provides the native dependencies (GTK4, GStreamer, etc.).

```bash
# Enter the dev shell
nix develop

# Build
nix develop -c cargo build

# Check without building
nix develop -c cargo check

# Run, optionally with a video file
nix develop -c cargo run -- /path/to/video.mkv

# Lint & format
nix develop -c cargo clippy
nix develop -c cargo fmt
```

## Testing

```bash
# Unit tests (no display needed)
nix develop -c cargo test

# A specific module
nix develop -c cargo test services::watch_state

# GTK / GStreamer integration tests (need a virtual display)
nix develop -c xvfb-run --auto-servernum cargo test --features integration
```

The project follows a TDD approach: business logic is extracted into pure, testable functions (parsing, state derivation, filtering, watch-state tracking), with traits at the source/repository boundaries for mocking. GStreamer and GTK interaction is confined to the `player/` and component layers.

## Project layout

```
src/
  main.rs              # Entry point
  app/                 # Root App component (Relm4) + handlers, dialogs, watch events
  components/
    home/              # Hero + shelves
    library/           # Poster grid, filters, sorting
    detail/            # Movie & show detail pages
    player/            # VideoPlayer component, OSD chrome, quality menu
    sidebar.rs         # Per-source library navigation
  player/              # PlaybackPipeline (GStreamer), tracks, subtitles, skip markers
  services/
    plex/              # Plex client, transcode decisions, session lifecycle
    jellyfin/          # Jellyfin client
    download/          # Offline download queue, transfer, pruning, sidecars
    media_source.rs    # MediaSource trait (source boundary)
    watch_state.rs     # Pure watch-state tracker
    stream_cache.rs    # Read-ahead buffering
    mpris.rs           # MPRIS2 desktop integration
  db/                  # diesel migrations & repositories
  models/              # Media models, SourceType
  settings.rs          # TOML-backed settings
```

## Architectural rules

1. **No GStreamer outside `player/`** — all pipeline interaction goes through `PlaybackPipeline`.
2. **No GTK in `services/`** — the service layer is pure Rust.
3. **No business logic in `update()`** — Relm4 `update()` methods are thin dispatchers over pure functions and service calls.
4. **Traits at boundaries** — `MediaSource` and repository traits are mock-friendly by design.
5. **Errors as types** — `thiserror` enums, not strings.
