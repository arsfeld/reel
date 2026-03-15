# Brainstorm: Reel — Native Media Center in Zig

**Date:** 2026-03-14
**Status:** Draft

## What We're Building

Reel is a full-featured media center application — an Infuse clone — written in Zig with native platform frontends. It plays virtually any video format via embedded libmpv, manages media libraries with TMDB metadata, and serves as a full Plex client capable of replacing the official app.

The architecture follows Ghostty's proven pattern: a Zig core library exposing a C ABI, consumed by platform-native frontends (GTK4 on Linux, Swift/AppKit on macOS).

### Core Features

- **Universal playback** — libmpv-powered, direct-plays all major video/audio formats with hardware acceleration
- **Full Plex client** — Browse libraries, On Deck, Recently Added, sync watch status, direct play without transcoding
- **Media library management** — Scan local files and network shares, organize into Movies/TV Shows/Collections
- **Rich metadata** — TMDB integration for posters, descriptions, ratings, cast info, trailers
- **Subtitle support** — Embedded, external (.srt/.ass), and potentially OpenSubtitles search
- **Offline sync** — Download media from Plex for offline viewing
- **Native UI on every platform** — GTK4 on Linux, AppKit on macOS, each feeling at home on its platform

## Why This Approach

### Zig Core + Native Frontends (Ghostty Pattern)

We chose this over GTK-everywhere or protocol-based IPC because:

1. **Proven pattern** — Ghostty demonstrates this architecture works at production quality in Zig
2. **Maximum native feel** — Each platform gets its own idiomatic UI, not a lowest-common-denominator toolkit
3. **Maximum code sharing** — All business logic (Plex API, TMDB client, library DB, mpv control) lives in the Zig core
4. **Performance** — Zig's control over memory and lack of GC pairs well with media playback requirements

### libmpv as Media Backend

Rather than wrapping FFmpeg directly or writing decoders, embedding libmpv provides:

- Battle-tested codec support for virtually every format
- Hardware acceleration (VideoToolbox, VA-API, VDPAU) out of the box
- Subtitle rendering, audio normalization, and playback controls for free
- A well-documented C API that maps cleanly to Zig's C interop
- The "direct play everything" capability that makes Infuse valuable as a Plex client

### Plex as Primary Network Source

Instead of implementing SMB/NFS/DLNA protocol stacks, focusing on Plex integration means:

- Well-documented HTTP/XML API for library browsing
- Authentication via plex.tv tokens
- Direct play URLs for media streaming
- Watch status sync across devices
- Covers the most common "stream from server" use case

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Language | Zig | Performance, C interop, memory control, Ghostty precedent |
| Architecture | Core lib + native frontends | Ghostty pattern, maximum native feel |
| Media backend | libmpv (embedded) | Universal codec support, hardware accel, proven |
| Linux frontend | GTK4 | Modern, C API (Zig-friendly), Wayland support |
| macOS frontend | Swift/AppKit via C ABI | Native look & feel, platform conventions |
| Metadata | TMDB API | Industry standard, comprehensive movie/TV database |
| Network streaming | Plex client (abstract MediaServer interface) | Well-documented API, abstraction enables Jellyfin/Emby later |
| Library storage | SQLite (via Zig core) | Embedded, zero config, fast for local queries |
| Build system | Nix + build.zig | Reproducible deps via Nix, idiomatic Zig compilation |
| Plex auth | Browser redirect + PIN polling | Simple, no webview dependency |
| Video render | Frontend-owned surface | GtkGLArea / NSOpenGLView passed to libmpv. Matches IINA/Celluloid pattern |
| Offline sync | In scope | Download media for offline viewing |
| TV interface | Not in scope | Desktop-only for now |

## Architecture Overview

```
┌─────────────────────────────────────────────────┐
│                Platform Frontends                │
│  ┌──────────────────┐  ┌──────────────────────┐ │
│  │   GTK4 (Linux)   │  │  AppKit/Swift (macOS) │ │
│  │  - UI rendering  │  │  - UI rendering       │ │
│  │  - GTK video     │  │  - NSView video       │ │
│  │    widget        │  │    surface             │ │
│  │  - System tray   │  │  - Menu bar            │ │
│  │  - Media keys    │  │  - Media keys          │ │
│  └────────┬─────────┘  └──────────┬────────────┘ │
│           │        C ABI          │              │
│  ┌────────┴───────────────────────┴────────────┐ │
│  │              libreel (Zig Core)              │ │
│  │  ┌───────────┐ ┌──────────┐ ┌────────────┐  │ │
│  │  │ mpv ctrl  │ │ Plex API │ │ TMDB client│  │ │
│  │  └───────────┘ └──────────┘ └────────────┘  │ │
│  │  ┌───────────┐ ┌──────────┐ ┌────────────┐  │ │
│  │  │ Library   │ │ Scanner  │ │ Settings   │  │ │
│  │  │ (SQLite)  │ │          │ │            │  │ │
│  │  └───────────┘ └──────────┘ └────────────┘  │ │
│  └──────────────────────────────────────────────┘ │
│                        │                          │
│  ┌──────────────────────────────────────────────┐ │
│  │              System Libraries                 │ │
│  │  libmpv  ·  FFmpeg  ·  SQLite  ·  TLS/HTTP   │ │
│  └──────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────┘
```

## Resolved Questions

1. **Plex authentication flow** — Browser redirect: open default browser to plex.tv/link, Zig core polls for PIN completion. Simple, no webview dependency.

2. **Video rendering surface** — Frontend owns the render surface. Each frontend creates a render context (GtkGLArea on Linux, NSOpenGLView on macOS) and passes the handle down to libmpv via the C ABI. This matches how IINA and Celluloid work.

3. **Jellyfin/Emby support** — Abstract MediaServer interface from the start. Implement Plex first, but the trait/interface boundary makes adding Jellyfin/Emby straightforward later without refactoring.

4. **Offline/sync capability** — In scope. Users should be able to download media for offline viewing.

5. **TV interface** — Not in scope. Desktop-only for now.

## Open Questions

_(None remaining)_
