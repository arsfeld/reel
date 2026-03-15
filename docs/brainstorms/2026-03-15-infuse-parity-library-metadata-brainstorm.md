# Brainstorm: Infuse Feature Parity — Library & Metadata

**Date:** 2026-03-15
**Status:** Draft

## What We're Building

Three features to bring reel's library and metadata experience up to Infuse's level:

### 1. Rich Home Screen (Infuse-style)

A visually rich landing page with large backdrop fanart and horizontal poster carousels. Rows include:

- **Continue Watching** — Items with saved progress, sorted by last watched
- **Recently Added** — Newest items in the library
- **Favorites** — User-favorited items
- **By Genre** — Dynamic rows generated from genres present in the library (e.g., "Sci-Fi", "Comedy", "Drama")

The active/focused row's item shows its backdrop fanart as a blurred/dimmed background behind the entire view, similar to Infuse's immersive style.

**What exists today:** The GTK home view is a placeholder. The database already supports `getContinueWatching`, `getRecentlyAdded`, and favorites queries. TMDB integration fetches backdrop images. The image cache stores them. The building blocks are there — the UI layer needs to be built.

### 2. Collections (Smart + Manual)

User-created groupings of media items, both manual and rule-based:

**Manual collections:**
- User creates a named collection, adds/removes items by hand
- Custom artwork (optional) or auto-generated from first 4 posters

**Smart collections (rule-based, auto-populated):**
- Filter by: genre, year/decade, content rating, watched/unwatched status, media type (movie/TV), resolution, source (Plex server / local library)
- Multiple rules combined with AND logic
- Auto-update when library changes (new items matching rules appear automatically)

**Examples:** "Unwatched Movies", "2020s Sci-Fi", "4K Content", "Plex Server A — TV Shows"

**What exists today:** The `favorites` table provides a simple boolean favorite flag. No collection concept exists in the schema. This needs a new `collections` table, a `collection_items` join table, and a `collection_rules` table for smart collections.

### 3. Metadata Match Correction

When the scanner or TMDB auto-match picks the wrong movie/show, the user can re-match to the correct TMDB entry:

- From the detail view, offer a "Fix Match" action
- Opens a search dialog pre-populated with the filename-parsed title
- Shows TMDB search results with poster, year, and overview for disambiguation
- Selecting a result replaces all metadata fields and re-downloads artwork
- Optionally lock the match so future rescans don't overwrite it

**What exists today:** TMDB search and detail-fetch APIs are implemented. The media_items table stores `tmdb_id`. What's missing is the UI flow and a `match_locked` flag to prevent rescan overwrite.

## Why This Approach

These three features were chosen because they represent the most visible gap between reel and Infuse from a daily-use perspective. Infuse's home screen is what users see first and interact with most. Collections are how power users organize large libraries. Match correction is the #1 frustration when metadata is wrong.

Trakt sync, multi-user profiles, and TVDB fallback are deferred — they add cross-device value but aren't needed for a great single-device experience.

## Key Decisions

1. **Infuse-style home over Plex-style** — Large fanart backdrops with carousels, not dense grid hubs
2. **Smart + manual collections from day one** — Schema supports both; no need to retrofit later
3. **Match correction, not full metadata editing** — Re-match to correct TMDB entry rather than manually editing individual fields. TMDB is the source of truth.
4. **Genre rows are dynamic** — Generated from genres actually present in the library, not a fixed list

## Resolved Questions

1. **Collection placement** — Both sidebar and home screen. Collections get their own sidebar entry for direct access, and optionally appear as carousel rows on the home screen.
2. **Genre storage** — Genres are NOT currently stored in the database. TMDB fetches them (`tmdb/types.zig:Genre`) but they're discarded after display. Need a new `genres` table or a `genres` TEXT column (comma-separated or JSON) on `media_items`. A separate `media_item_genres` join table is cleanest for smart collection filtering.
3. **Home screen data freshness** — Cache + background refresh. Show cached data immediately on navigate, refresh in the background. Smoother UX, slightly stale on first load.

## Out of Scope (for now)

- Trakt.tv sync
- Multi-user profiles
- TVDB as fallback metadata source
- Network share streaming (SMB/NFS/WebDAV)
- Jellyfin/Emby support
- Full manual metadata field editing
