# Playback Features: Infuse Parity

**Date:** 2026-03-17
**Status:** Draft

## What We're Building

Five playback features that close the gap between Reel and Infuse's player experience, implemented in the Zig core and exposed via the GTK4/libadwaita frontend first (macOS later).

### 1. Chapter Navigation

**What:** Display chapter markers as dots/ticks on the seek bar. Add previous/next chapter buttons to the player controls overlay. mpv already parses chapter metadata from MKV/MP4 — we just need to expose it.

**Core API surface:**
- `reel_player_get_chapter_count() -> i32`
- `reel_player_get_chapter(index) -> { title, time_pos }`
- `reel_player_next_chapter()`
- `reel_player_prev_chapter()`
- Keyboard shortcuts: `,` (prev chapter), `.` (next chapter)

**GTK UI:** Chapter tick marks rendered on the GtkScale progress bar. Tooltip on hover shows chapter title.

### 2. Playback Speed Controls

**What:** Preset speed options (0.5x, 0.75x, 1x, 1.25x, 1.5x, 2x) accessible from a picker in the player controls overlay.

**Core API surface:**
- `reel_player_set_speed(f64)`
- `reel_player_get_speed() -> f64`
- Speed persists per-session, resets to 1x on new playback

**GTK UI:** Speed button in controls bar showing current speed (e.g., "1x"). Click opens a popover with preset options. Current speed highlighted.

### 3. Auto-Play Next Episode

**What:** When credits begin on a TV episode, show a countdown overlay (15 seconds) with next episode info (title, thumbnail). User can cancel or play immediately. Auto-advances when countdown reaches zero. Does not appear for movies or standalone content — only for items that have a next episode.

**Core requirements:**
- Query next episode in series order from database
- Expose `reel_player_get_next_episode(current_media_id) -> nullable media_item`
- Fetch Plex credits markers from API (marker type "credits" with start/end timestamps)
- Trigger countdown at credits marker start time when available
- Fall back to ~30 seconds remaining for non-Plex content or items without markers
- EOF as final fallback
- Countdown timer logic in core, UI renders it

**GTK UI:** Overlay in bottom-right of player showing next episode poster, title, "Playing in 15s..." countdown, Cancel and Play Now buttons.

### 4. External Subtitle Loading

**What:** Auto-detect subtitle files (.srt, .ass, .ssa, .sub, .idx) in the same directory as the video file (matching by filename stem, including language suffixes like `movie.en.srt`). Also allow manual file selection via a file picker dialog.

**Applies to:** Local files and downloaded content only. For Plex streams (HTTP playback), auto-scan is skipped — only the manual file picker and embedded tracks are available.

**Core API surface:**
- `reel_player_scan_external_subs(video_path) -> []{path, lang, format}`
- `reel_player_load_subtitle_file(path)`
- Auto-scan happens on playback start for local paths; skipped for HTTP URLs
- Results merged with embedded tracks in the subtitle cycle

**GTK UI:** Subtitle button in controls opens a popover listing all tracks (embedded + external). "Add subtitle file..." option at bottom opens GtkFileDialog.

### 5. Subtitle Appearance Customization

**What:** Full control over subtitle rendering: font family, size, primary color, outline/shadow color, background (semi-transparent box or none), and position (top/bottom). Settings persist globally in the database.

**Core API surface (mapped to mpv sub-* options):**
- `reel_player_set_sub_font(name)`
- `reel_player_set_sub_font_size(i32)`
- `reel_player_set_sub_color(rgba_hex)`
- `reel_player_set_sub_border_color(rgba_hex)`
- `reel_player_set_sub_border_size(f64)`
- `reel_player_set_sub_back_color(rgba_hex)` (background box)
- `reel_player_set_sub_pos(i32)` (vertical position, 0-100)
- Settings stored in `settings` table, applied on player init

**GTK UI:** Subtitle settings panel in the Settings view (not in-player). Live preview if possible, but not required for v1. Controls: font dropdown, size slider, color pickers, position toggle (top/bottom).

## Why This Approach

- **mpv does the heavy lifting** — All five features map directly to mpv properties and commands. No custom codec work needed. Chapter metadata, speed, subtitle loading, and subtitle styling are all first-class mpv features.
- **Core + C ABI pattern** — Following the established Ghostty-pattern architecture. Features live in the Zig core, exposed via C ABI, consumed by platform frontends. This ensures macOS gets the same features later with minimal effort.
- **GTK-first** — The GTK frontend is more complete and is the primary development platform. macOS frontend can adopt these features later using the same C API.
- **Infuse-matching UX** — Countdown overlay for auto-play, preset speed picker, chapter markers on seek bar — these match Infuse's UX patterns that users expect.

## Key Decisions

1. **Chapter UI is seek bar markers + buttons, not a list panel** — Keeps the player overlay simple. A chapter list can be added later if needed.
2. **Speed presets, not fine-grained** — 0.5x to 2x in standard increments. Matches Infuse. Avoids UI complexity of a slider.
3. **Auto-play uses Plex credits markers when available** — Plex provides credits marker data via its API indicating exactly when credits begin. Use that as the trigger when available; fall back to ~30 seconds remaining for non-Plex content or items without markers. Countdown is 15 seconds, matching Netflix/Infuse convention.
4. **External subs are local-only** — No OpenSubtitles API integration. Auto-scan same directory + manual file picker. Network subtitle sources deferred to when network shares are implemented.
5. **Subtitle appearance settings are global, configured in Settings view** — Stored in settings table, applied on every playback. Not in-player — keeps the player overlay clean. Per-video overrides are a future enhancement.
6. **External subtitle scanning is local-only** — Auto-scan applies to local files and downloads. For Plex streams, only the manual file picker and embedded tracks are available.
7. **GTK-first, macOS later** — All core/C ABI work benefits both platforms. GTK UI is implemented now; SwiftUI UI deferred.

## Resolved Questions

1. **Auto-play trigger** — Use Plex credits markers when available. Fall back to ~30s remaining, then EOF. *(See Key Decision #3 for details.)*
2. **Subtitle settings location** — Global Settings view, not in-player. *(See Key Decision #5.)*
3. **Speed persistence** — Reset to 1x per-session, matching Infuse. *(See Key Decision #2.)*

## Implementation Order (Suggested)

1. **Chapter navigation** — Smallest scope, high impact, pure mpv property reads
2. **Playback speed** — Simple mpv property + small UI addition
3. **External subtitle loading** — Auto-scan logic + file picker integration
4. **Subtitle appearance** — More UI work (settings panel with multiple controls)
5. **Auto-play next episode** — Most complex (database queries, countdown timer, overlay UI, state machine)
