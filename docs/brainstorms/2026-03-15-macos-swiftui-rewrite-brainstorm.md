# Brainstorm: macOS Full SwiftUI Rewrite — Modern, Cinematic Media Center

**Date:** 2026-03-15
**Status:** Draft

## What We're Building

A complete rewrite of Reel's macOS frontend in SwiftUI, targeting macOS 26 Tahoe with Liquid Glass. The current AppKit implementation (sidebar, settings, placeholder views) gets replaced with a fully functional, visually stunning SwiftUI app inspired by Infuse for Mac.

The Zig backend and C API remain unchanged — this is purely a frontend rewrite. The C ABI already exposes everything needed: library queries, collections, search, downloads, favorites, Plex auth, settings, and image cache.

### Core Experience

**Home Screen** — Cinematic landing page with:
- Hero banner: large backdrop fanart from a featured/recent item, title overlay, Play button
- Horizontal poster carousels: Continue Watching, Recently Added, Favorites
- Dynamic genre rows auto-generated from library genres (Sci-Fi, Comedy, Drama, etc.)
- Backdrop changes as user browses/hovers different items

**Movies / TV Shows / Other** — Poster grid views with:
- Sort dropdown (Title, Year, Rating, Date Added)
- Lazy loading for large libraries
- Click poster → push to cinematic detail view

**Detail View** — Infuse-style cinematic treatment:
- Full-bleed backdrop image with blur/gradient
- Poster overlay, title, year, rating, runtime, summary
- Genre tags, cast list
- Play button, Fix Match, Add to Collection, Mark Watched, Favorite toggle, Download (Plex)
- TV shows: season picker, episode list with watch indicators

**Player** — Full window takeover:
- Play button hides sidebar, fills entire window with mpv player
- Existing mpv/OpenGL integration bridged via NSViewRepresentable
- Player controls overlay with auto-hide
- Escape returns to previous view
- F or double-click for true macOS fullscreen

**Sidebar** — SwiftUI NavigationSplitView:
- Library section: Home, Movies, TV Shows, Other
- Sources section: Favorites, Files
- Management section: Downloads, Settings
- SF Symbols icons, Liquid Glass translucency

**Settings** — Full Plex auth flow, library scan paths, TMDB config, playback preferences, download management

**Downloads** — Queue with progress bars, pause/resume/cancel, play completed items

**Files** — Browse connected Plex servers and local scan paths

**Favorites** — Grid of favorited items with remove capability

## Why This Approach

### Full SwiftUI over AppKit

- **No tech debt**: This is a new app. Building on AppKit means building on a framework Apple is actively deprecating in favor of SwiftUI.
- **macOS 26 Tahoe**: Liquid Glass is SwiftUI-first. The new translucent materials, glass effects, and system integration work best (and in some cases only) with SwiftUI.
- **Velocity**: SwiftUI's declarative syntax means faster iteration on UI. Layout, animation, and state management are dramatically simpler.
- **Future platforms**: If Reel ever targets iPadOS/tvOS/visionOS, SwiftUI code carries over. AppKit doesn't.

### Infuse as visual reference

Infuse is the gold standard for media center UI on Apple platforms. Its design language — dark, cinematic, large artwork, blurred backdrops, poster-forward — is exactly right for a media app. We're not copying Infuse's code, but matching its visual quality and interaction patterns.

### macOS 26 Tahoe minimum

- Access to Liquid Glass materials for sidebar, toolbar, and overlay translucency
- Latest SwiftUI APIs: best NavigationSplitView, ScrollView, animation, and Observable support
- No workarounds for older OS versions
- Most Mac users update quickly; by the time Reel ships, Tahoe will be widely adopted

## Key Decisions

1. **Full SwiftUI rewrite** — No incremental migration. Clean break from AppKit. Only bridge: mpv/OpenGL VideoView via NSViewRepresentable.
2. **macOS 26 Tahoe minimum** — Liquid Glass, latest SwiftUI APIs, no backwards compatibility hacks.
3. **Infuse-style cinematic design** — Dark, large artwork, blurred backdrops, poster-forward. Not Plex-dense or Apple TV-editorial.
4. **NavigationSplitView + NavigationStack** — Standard SwiftUI navigation: sidebar/content split with push/pop for grid → detail → player.
5. **Full window takeover for playback** — Player hides sidebar, fills window. Escape returns. Most immersive experience.
6. **Simple sort, no inline filtering** — Sort dropdown (Title, Year, Rating, Date Added) on grid views. No genre filter chips or inline search — keep it clean.
7. **Dynamic genre rows on Home** — Generated from genres actually present in the library, not a fixed list.
8. **Bridge only the mpv player** — NSViewRepresentable for the OpenGL video view. Everything else is pure SwiftUI.

## Architecture Notes

### State Management
- `@Observable` classes for view models (macOS 26 guarantees Observation framework)
- Thin Swift wrappers around C ABI calls
- Async/await for C ABI calls dispatched to background threads
- Image loading via AsyncImage or custom async image loader backed by the C image cache

### C ABI Bridge Layer
- Swift wrapper structs/classes that call C functions from `reel.h`
- Convert C strings/arrays to Swift types at the boundary
- Error handling: map `ReelError` codes to Swift errors

### Key SwiftUI Views
```
RealApp (SwiftUI App)
├── NavigationSplitView
│   ├── Sidebar
│   │   ├── Library: Home, Movies, TV Shows, Other
│   │   ├── Sources: Favorites, Files
│   │   └── Management: Downloads, Settings
│   └── NavigationStack (content)
│       ├── HomeView (hero + carousels)
│       ├── MediaGridView (poster grid, reused for Movies/TV/Other/Favorites)
│       ├── DetailView (cinematic metadata)
│       ├── PlayerView (full window takeover, NSViewRepresentable)
│       ├── FilesView (source browser)
│       ├── DownloadsView (queue list)
│       └── SettingsView (preferences)
```

### Liquid Glass Integration
- Sidebar uses system Liquid Glass material automatically via NavigationSplitView
- Player controls overlay: glass material behind transport controls
- Detail view: glass-effect metadata card over backdrop image
- Toolbar: standard Liquid Glass toolbar behavior

## Open Questions

*None — all key decisions resolved through brainstorming.*

## Out of Scope

- GTK/Linux frontend changes (this brainstorm is macOS-only)
- Zig backend changes (C API already covers all needed functionality)
- Trakt.tv sync, multi-user profiles, TVDB fallback
- iPadOS/tvOS/visionOS (future, but SwiftUI choice enables this later)
- Network share streaming (SMB/NFS/WebDAV)
- Jellyfin/Emby support
