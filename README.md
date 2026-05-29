# Reel

A native Plex and Jellyfin client for the GNOME desktop.

Reel brings your home media server to GNOME with a clean, modern interface that feels at home alongside your other apps. Connect your Plex or Jellyfin server, browse your movies and shows, and watch — with proper resume, watch-state sync, server transcoding, and offline downloads. It follows GNOME's libadwaita design conventions, so it looks and behaves like a first-class part of your desktop.

> App ID: `dev.arsfeld.Reel`

## What it does

- **Connect your servers.** Sign in to Plex or Jellyfin (or both at once). Reel groups each server's libraries in the sidebar, and you can add, remove, enable, or disable sources individually.
- **Browse beautifully.** A Home page with a hero carousel and shelves for Recently Added, Continue Watching, Next Up, and your server's hubs. Library views with poster grids, list mode, and adjustable poster size.
- **Find what you want.** Sort by title, year, date added, rating, or runtime. Filter by watch status, genre, content rating, year, runtime, resolution, or HDR format. Your filters and sort order are remembered per library.
- **Watch anything.** Universal format playback. Switch audio and subtitle tracks on the fly, with your preferred languages applied automatically. External subtitle files are picked up too.
- **Pick up where you left off.** Resume from your last position, with watch state kept in sync with your server. Plex's own watch state stays authoritative, so progress matches across all your devices.
- **Skip the boring parts.** Intro and credits skip markers from Plex and Jellyfin.
- **Stream at the right quality.** Plex server-side transcoding with a quality ladder, automatic bitrate capping when you're away from home, and an in-player quality menu that shows whether you're getting Direct Play or a transcode.
- **Take it offline.** Download movies and episodes from Plex for offline viewing, with a download queue, pause/resume, a storage budget that prunes watched items, and metadata kept so downloads stay browsable without a connection.
- **Control it from anywhere.** MPRIS2 integration means media keys, the GNOME volume-panel media controls, and other desktop tools can play, pause, seek, and see what's playing.

## Player controls

Reel uses familiar, mpv-style on-screen controls — a seek bar, transport buttons, current/total time, a volume slider with mute, and a fullscreen toggle that auto-hides while you watch.

Keyboard shortcuts:

| Key | Action |
| --- | --- |
| `Space` / `k` | Play / pause |
| `←` / `→` | Seek ∓5 seconds |
| `j` / `l` | Seek ∓10 seconds |
| `↑` / `↓` | Seek ∓60 seconds |
| `Home` / `0` | Jump to start |
| `End` | Jump to end |
| `m` | Mute |
| `9` / `0` | Volume down / up |
| `f` | Toggle fullscreen |
| `Esc` | Exit fullscreen |

## Source support at a glance

| Feature | Plex | Jellyfin |
| --- | --- | --- |
| Sign-in & libraries | ✅ | ✅ |
| Collections / box sets | ✅ | ✅ |
| Hubs, Latest, Continue Watching, Next Up | ✅ | ✅ |
| Watch-state sync | ✅ (authoritative) | ✅ |
| Skip markers (intro / credits) | ✅ | ✅ |
| Server-side transcoding | ✅ | Direct play |
| Offline downloads | ✅ | — |

## Installing & running

Reel is built with Rust and GTK4. Development and builds use a [Nix](https://nixos.org/) dev shell that provides the native dependencies (GTK4, GStreamer, libadwaita).

```bash
# Enter the dev shell (provides all native deps)
nix develop

# Build and run
nix develop -c cargo run

# Open a local video file directly
nix develop -c cargo run -- /path/to/video.mkv
```

Your library, sources, watch progress, and downloads are stored in a local SQLite database. Settings are kept as TOML in your platform config directory.

## Built with

Rust · GTK4 · Relm4 (Elm/MVU) · libadwaita · GStreamer (`playbin3`) · diesel/SQLite · tokio

