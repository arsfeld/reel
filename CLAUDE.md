# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Reel is a native media center application with a shared Zig core library and two platform-specific frontends:
- **Linux**: GTK4 + libadwaita (Zig, `src/apprt/gtk/`)
- **macOS**: SwiftUI (Swift, `macos/Reel/Sources/`)

The core library (`src/lib.zig` + `src/core/` + `src/net/`) exports a C ABI consumed by both frontends. Swift uses FFI via `ReelBridge.swift`; GTK calls Zig directly.

## Build Commands

All commands require the Nix dev shell. Prefix with `nix develop --command` or use `just`:

```bash
just build          # nix develop --command zig build
just run            # Build + run (GTK on Linux, Swift on macOS)
just test           # nix develop --command zig build test
just clean          # rm -rf zig-out .zig-cache macos/.build
```

Direct Zig commands (inside nix shell):
```bash
nix develop --command zig build              # Build library + GTK executable
nix develop --command zig build test          # Run core tests
nix develop --command zig build -Doptimize=Debug  # Debug build
```

macOS Swift frontend (after `zig build` produces `zig-out/lib/libreel.a`):
```bash
cd macos && swift build    # Needs REEL_MPV_LIBDIR, REEL_EPOXY_LIBDIR, REEL_SQLITE_LIBDIR env vars
```

## Architecture

### Core Library (`src/lib.zig`, `src/core/`)

`lib.zig` re-exports all core and net modules, plus defines C ABI `export fn reel_*()` functions for Swift interop. Key modules:

- **`core/player.zig`** — libmpv wrapper (load, seek, pause, render via OpenGL)
- **`core/database.zig`** — SQLite with WAL mode, thread-safe (`SQLITE_OPEN_FULLMUTEX`), auto-migration with backup
- **`core/library.zig`** — Stateless query API (getItems, getRecently, getCollections) with filtering/sorting/pagination
- **`core/settings.zig`** — Config file I/O, Plex credentials, window state
- **`core/downloader.zig`** — Resume-capable downloads with progress tracking
- **`core/image_cache.zig`** — Disk + memory cache for posters/backdrops

### Network Layer (`src/net/`)

- **`net/http.zig`** — HTTP client with gzip decompression, range requests
- **`net/plex/client.zig`** — Plex server discovery, library sync (XML parsing), auth
- **`net/plex/auth.zig`** — Plex OAuth flow
- **`net/tmdb/client.zig`** — TMDB metadata enrichment (search, details)
- **`net/connection_selector.zig`** — Local-first relay discovery with fallback

### GTK4 Frontend (`src/apprt/gtk/`)

Navigation uses `AdwNavigationSplitView` (sidebar + content) with `AdwNavigationView` for push/pop drill-down:
- Sidebar clicks → `replace()` entire content stack (no history accumulation)
- Poster click → `push()` detail view (back button appears)
- Views are singletons created once at startup; `showing` signal triggers `refresh()`

Key files: `app.zig` (state machine, navigation), `video_area.zig` (GtkGLArea + mpv render), `player_controls.zig` (overlay controls)

### Swift Frontend (`macos/Reel/Sources/`)

- **`Bridge/ReelBridge.swift`** — FFI wrapper calling `reel_*()` C functions
- **`Models/PlayerModel.swift`** — ObservableObject wrapping player C API
- **`Components/VideoView.swift`** — MTLView for mpv OpenGL rendering

### C ABI (`include/reel.h`)

Header declaring all exported Zig functions. Any new `export fn` in `lib.zig` should have a corresponding declaration here for Swift consumption.

## System Dependencies (via Nix flake)

- **zig** (0.15.0+), **mpv-unwrapped**, **sqlite**, **libepoxy**, **glib**
- Linux-only: **gtk4**, **libadwaita**, **gobject-introspection**
- macOS: env vars `REEL_MPV_LIBDIR`, `REEL_EPOXY_LIBDIR`, `REEL_SQLITE_LIBDIR` set by flake

## Build Artifacts

- `zig-out/lib/libreel.a` — Static library (consumed by Swift PM and tests)
- `zig-out/bin/reel` — Linux GTK executable
- `macos/.build/debug/Reel` — macOS app

## Conventions

- Zig 0.15.0+ idioms (no `try` in `pub fn main`, use `b.path()` not `LazyPath`)
- C ABI functions prefixed `reel_` and return `c_int` error codes (0 = success, negative = error)
- GTK bindings via `@cImport(@cInclude("gtk/gtk.h"))` — raw C interop, no wrapper library
- Tests are inline `test` blocks in Zig source files
- Documentation lives in `docs/` (plans, brainstorms, research)
- Runtime data (cache, config, database) goes under `data/` (gitignored)
