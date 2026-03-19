---
title: "feat: Playback Features Infuse Parity"
type: feat
status: active
date: 2026-03-17
origin: docs/brainstorms/2026-03-17-playback-features-infuse-parity-brainstorm.md
---

# feat: Playback Features Infuse Parity

## Overview

Five playback features to close the gap between Reel and Infuse's player experience: chapter navigation, playback speed controls, external subtitle loading, subtitle appearance customization, and auto-play next episode. All features are implemented in the Zig core and exposed via C ABI, with GTK4/libadwaita UI first (macOS later).

(see brainstorm: `docs/brainstorms/2026-03-17-playback-features-infuse-parity-brainstorm.md`)

## Problem Statement

Reel's player currently offers only basic controls: play/pause, seek, volume, mute, fullscreen, and subtitle/audio track cycling. Infuse users expect chapter markers on the seek bar, playback speed adjustment, automatic episode advancement, external subtitle file support, and customizable subtitle appearance. These are table-stakes for a serious media center.

## Proposed Solution

Five features implemented in five phases, ordered by complexity:

1. **Chapter navigation** — mpv property reads + seek bar tick marks + prev/next buttons
2. **Playback speed controls** — mpv speed property + popover picker in controls
3. **External subtitle loading** — filesystem scan + mpv `sub-add` + subtitle popover
4. **Subtitle appearance customization** — settings UI + mpv `sub-*` properties
5. **Auto-play next episode** — Plex credits markers + countdown overlay + next-episode DB query

## Technical Approach

### Architecture

All five features follow the established Ghostty-pattern: logic lives in the Zig core (`src/core/player.zig`), exposed via C ABI (`src/lib.zig` + `include/reel.h`), consumed by the GTK frontend (`src/apprt/gtk/player_controls.zig`). mpv does the heavy lifting — chapters, speed, subtitle loading, and subtitle styling are all first-class mpv properties/commands.

**Key architectural decisions:**

- **`current_media_item_id` tracking** — Add `current_media_item_id: ?i64` to `AppState` in `app.zig`, set in `playMediaItem()`. Required for auto-play next episode and proper watch progress.
- **Chapter tick marks** — Overlay small widgets on top of the GtkScale at computed positions, repositioned on window resize. GtkScale has no native API for arbitrary position marks.
- **Auto-play state machine** — States: `idle` → `monitoring` → `countdown_active` → `transitioning`. Cancel returns to `monitoring`. Seek backward past trigger cancels countdown.
- **Subtitle settings** — Only set mpv properties for which the user has explicitly configured a value in Reel's settings. Leave mpv defaults for unconfigured properties (respects user's `mpv.conf`).

### Design Decisions from SpecFlow Analysis

These decisions resolve edge cases identified during specification analysis:

| Question | Decision | Rationale |
|----------|----------|-----------|
| Chapter tick mark rendering | Overlay positioned widgets on GtkScale | GtkScale has no native mark API; overlays are simpler than a custom widget |
| Seek backward during countdown | Cancel countdown, re-trigger on next crossing | Matches Infuse; user clearly wants to re-watch |
| Speed reset on auto-play | Preserve speed across auto-play transitions | Binge-watchers at 1.5x would be frustrated; only reset on manual new playback |
| Countdown timer: real-time vs media-time | Real-time (wall clock) | Predictable for user regardless of playback speed |
| Countdown paused when playback paused | Yes, pause countdown too | Intuitive; countdown represents "time until next episode plays" |
| Plex markers fetch timing | Fetch at playback start, cache in session | Avoids mid-playback API calls; markers are small data |
| External sub filename matching | Case-insensitive, video stem must be prefix | `movie.mkv` matches `movie.srt`, `movie.en.srt`, `movie.forced.en.srt` but not `movie2.srt` |
| Subtitle language detection | Parse from filename suffix (ISO 639-1/2) | `movie.en.srt` → "en"; no suffix → "unknown" |
| Sub color format | GdkRGBA → `#AARRGGBB` hex for mpv | mpv uses `#AARRGGBB`; conversion in settings apply code |
| Audio pitch correction | Enabled by default, no toggle | mpv default; matches Infuse |
| Font picker | All system fonts via PangoFontMap with search entry | Curated list is too opinionated |
| Subtitle file formats | `.srt`, `.ass`, `.ssa`, `.sub`, `.idx`, `.vtt` | All natively supported by mpv |
| Chapter buttons when no chapters | Hidden entirely | Reduces clutter |
| Auto-play overlay vs controls overlap | Overlay positioned above controls area | Bottom-right with margin clearing the controls bar height |
| Watch progress on auto-play | Mark current episode `watched=1`, start next at position 0 (or resume if partial progress exists) | Correct On Deck/Continue Watching behavior |
| Direct play mode auto-play | Disabled (no library context) | No media item ID available |
| Default subtitle appearance | Use mpv defaults unless user explicitly sets values | Respects power users' mpv.conf |

### Implementation Phases

#### Phase 1: Chapter Navigation

**Goal:** Chapter markers on seek bar, prev/next navigation, keyboard shortcuts.

**Files to modify:**

| File | Changes |
|------|---------|
| `src/core/player.zig` | Add `getChapterCount()`, `getChapterList()`, `getCurrentChapter()`, `nextChapter()`, `prevChapter()` methods |
| `src/lib.zig` | Export `reel_player_get_chapter_count`, `reel_player_get_chapter`, `reel_player_next_chapter`, `reel_player_prev_chapter` |
| `include/reel.h` | Add `ReelChapter` struct (`title`, `time_pos`), declare chapter functions |
| `src/apprt/gtk/player_controls.zig` | Add chapter prev/next buttons to bottom row (flanking play button), render tick marks on seek bar |
| `src/apprt/gtk/keys.zig` | Add `,` (prev chapter) and `.` (next chapter) key bindings |

**Core implementation (`player.zig`):**

```zig
// Chapter data from mpv's "chapter-list" property (MPV_FORMAT_NODE)
pub const Chapter = struct {
    title: ?[]const u8,
    time_pos: f64, // seconds
};

pub fn getChapterCount(self: *Player) i32 {
    var count: i64 = 0;
    _ = c.mpv_get_property(self.handle, "chapter-list/count", c.MPV_FORMAT_INT64, @ptrCast(&count));
    return @intCast(count);
}

pub fn nextChapter(self: *Player) !void {
    const cmd = [_:null]?[*:0]const u8{ "add", "chapter", "1", null };
    const err = c.mpv_command(self.handle, @constCast(@ptrCast(&cmd)));
    if (err < 0) return error.CommandFailed;
}

pub fn prevChapter(self: *Player) !void {
    const cmd = [_:null]?[*:0]const u8{ "add", "chapter", "-1", null };
    const err = c.mpv_command(self.handle, @constCast(@ptrCast(&cmd)));
    if (err < 0) return error.CommandFailed;
}
```

**GTK seek bar tick marks:**
- After file loads, query chapter list from player
- For each chapter, create a small `GtkBox` (4px wide, full height) with CSS class `"chapter-mark"` styled as a semi-transparent white line
- Position each mark as an overlay child of the seek bar container using `gtk_widget_set_margin_start()` computed from `(chapter.time_pos / duration) * seek_bar_width`
- Reposition all marks on window `notify::default-width` signal
- Hide/destroy marks when file unloads or has no chapters

**Success criteria:**
- [x] Chapter tick marks visible on seek bar for MKV files with chapters
- [ ] Prev/next chapter buttons appear when chapters exist, hidden when not
- [x] `,` and `.` keyboard shortcuts navigate chapters
- [ ] Tooltip on chapter mark hover shows chapter title
- [x] Marks reposition correctly on window resize

---

#### Phase 2: Playback Speed Controls

**Goal:** Speed preset picker in player controls, reset on new playback.

**Files to modify:**

| File | Changes |
|------|---------|
| `src/core/player.zig` | Add `setSpeed(f64)`, `getSpeed() f64` methods |
| `src/lib.zig` | Export `reel_player_set_speed`, `reel_player_get_speed` |
| `include/reel.h` | Declare speed functions |
| `src/apprt/gtk/player_controls.zig` | Add speed button + GtkPopover with preset list |

**Core implementation (`player.zig`):**

```zig
pub fn setSpeed(self: *Player, speed: f64) !void {
    var s = speed;
    const err = c.mpv_set_property(self.handle, "speed", c.MPV_FORMAT_DOUBLE, @ptrCast(&s));
    if (err < 0) return error.SetPropertyFailed;
}

pub fn getSpeed(self: *Player) f64 {
    var speed: f64 = 1.0;
    _ = c.mpv_get_property(self.handle, "speed", c.MPV_FORMAT_DOUBLE, @ptrCast(&speed));
    return speed;
}
```

**GTK popover:**
- Speed button in bottom row after volume button, label shows current speed (e.g., "1x")
- Click opens `GtkPopover` containing a `GtkListBox` with 6 rows: 0.5x, 0.75x, 1x, 1.25x, 1.5x, 2x
- Current speed row has a checkmark or bold styling
- On row click: set speed via C ABI, update button label, dismiss popover
- On new file load (`loadFile`): reset speed to 1.0, update button label
- **Exception:** Auto-play transitions do NOT reset speed (handled in Phase 5)

**Success criteria:**
- [x] Speed button visible in player controls showing current speed
- [x] Popover shows 6 preset speeds with current highlighted
- [x] Selecting a speed immediately changes playback rate
- [x] Speed resets to 1x on manual new playback
- [x] Audio pitch correction is active (mpv default)

---

#### Phase 3: External Subtitle Loading

**Goal:** Auto-detect subtitle files alongside video, manual file picker, subtitle track popover.

**Files to modify:**

| File | Changes |
|------|---------|
| `src/core/player.zig` | Add `scanExternalSubs(video_path)`, `loadSubtitleFile(path)`, `getSubtitleTracks()` methods |
| `src/lib.zig` | Export subtitle scanning and loading functions |
| `include/reel.h` | Add `ReelSubtitleTrack` struct, declare functions |
| `src/apprt/gtk/player_controls.zig` | Replace simple subtitle cycle button with popover listing all tracks + "Add file..." option |

**Core implementation (`player.zig`):**

```zig
pub const SubtitleTrack = struct {
    id: i32,
    title: ?[]const u8,
    lang: ?[]const u8,
    external: bool,
};

pub fn loadSubtitleFile(self: *Player, path: []const u8) !void {
    // mpv "sub-add" command loads an external subtitle file
    const cmd = [_:null]?[*:0]const u8{ "sub-add", path_z, null };
    const err = c.mpv_command(self.handle, @constCast(@ptrCast(&cmd)));
    if (err < 0) return error.CommandFailed;
}
```

**External subtitle scanning logic:**
- Input: video file path (e.g., `/media/movies/movie.mkv`)
- Skip if path starts with `http://` or `https://` (Plex stream)
- Extract directory and filename stem (case-insensitive)
- Scan directory for files where:
  - Extension is one of: `.srt`, `.ass`, `.ssa`, `.sub`, `.idx`, `.vtt`
  - Filename starts with the video's stem (case-insensitive)
- Parse language from suffix: `movie.en.srt` → lang="en", `movie.srt` → lang=null
- Load each found subtitle via `sub-add`

**GTK subtitle popover:**
- Subtitle button (icon: `media-view-subtitles-symbolic` or similar) in bottom row
- Click opens `GtkPopover` with `GtkListBox`:
  - Section header: "Embedded" — list embedded tracks from mpv `track-list`
  - Section header: "External" — list auto-detected external tracks (if any)
  - Separator
  - "Add subtitle file..." row → opens `GtkFileDialog` with filter for subtitle extensions
- Current active subtitle track has a checkmark
- Clicking a track switches to it via `mpv_set_property("sid", track_id)`
- "None" option to disable subtitles

**Success criteria:**
- [x] Auto-detects subtitle files in same directory as local video
- [x] Skips auto-scan for HTTP URLs (Plex streams)
- [x] Subtitle popover lists all embedded and external tracks
- [x] "Add subtitle file..." opens file picker and loads selected file
- [x] Language suffix detection works (`.en.srt`, `.pt-BR.srt`)
- [x] Selecting a track switches to it; "None" disables subtitles

---

#### Phase 4: Subtitle Appearance Customization

**Goal:** Settings UI for subtitle rendering appearance, persisted globally, applied on playback.

**Files to modify:**

| File | Changes |
|------|---------|
| `src/core/settings.zig` | Add well-known keys for `sub_font`, `sub_font_size`, `sub_color`, `sub_border_color`, `sub_border_size`, `sub_back_color`, `sub_pos` |
| `src/core/player.zig` | Add `applySubtitleSettings(settings)` method that reads settings and sets mpv properties |
| `src/lib.zig` | Export `reel_player_apply_subtitle_settings` |
| `include/reel.h` | Declare subtitle settings functions |
| `src/apprt/gtk/settings_view.zig` | Add "Subtitle Appearance" preferences group with font dropdown, size slider, color pickers, position toggle |

**Settings keys (`settings.zig`):**

```zig
pub const sub_font = "sub_font";
pub const sub_font_size = "sub_font_size";
pub const sub_color = "sub_color";           // stored as #AARRGGBB
pub const sub_border_color = "sub_border_color";
pub const sub_border_size = "sub_border_size";
pub const sub_back_color = "sub_back_color";
pub const sub_pos = "sub_pos";               // 0-100, 100=bottom
```

**Player apply method:**
- Called after `loadFile()` in the GTK playback flow
- For each setting key: check if value exists in settings table
- Only set mpv property if user has explicitly configured a value (respects mpv.conf defaults)
- Color conversion: stored `#AARRGGBB` hex → mpv `sub-color` format

**GTK Settings UI (`settings_view.zig`):**
- New `AdwPreferencesGroup` titled "Subtitle Appearance" in the existing Playback section
- **Font**: `AdwComboRow` populated from `PangoFontMap` (searchable)
- **Size**: `AdwSpinRow` with range 16-72, default 55 (mpv default)
- **Primary color**: `AdwActionRow` with `GtkColorDialogButton`
- **Outline color**: `AdwActionRow` with `GtkColorDialogButton`
- **Border size**: `AdwSpinRow` with range 0-10, step 0.5
- **Background**: `AdwActionRow` with `GtkColorDialogButton` (alpha for transparency)
- **Position**: `AdwSwitchRow` labeled "Subtitles at top" (off = bottom, default)
- Each control's signal handler saves to settings immediately via `settings.setString(key, value)`

**Success criteria:**
- [x] Settings UI shows all subtitle appearance controls
- [x] Changes persist across app restarts
- [x] Subtitle appearance applied on playback start
- [x] Only explicitly-set values override mpv defaults
- [ ] Color conversion produces correct subtitle colors

---

#### Phase 5: Auto-Play Next Episode

**Goal:** Countdown overlay at end of TV episodes, auto-advance to next episode.

**Files to modify:**

| File | Changes |
|------|---------|
| `src/core/player.zig` | Add auto-play state machine, credits marker tracking |
| `src/core/library.zig` | Add `getNextEpisode(item_id)` query |
| `src/net/plex/client.zig` | Add `getMarkers(rating_key)` API method |
| `src/net/plex/types.zig` | Add `PlexMarker` struct |
| `src/lib.zig` | Export `reel_player_get_next_episode`, auto-play state queries |
| `include/reel.h` | Declare auto-play types and functions |
| `src/apprt/gtk/app.zig` | Add `current_media_item_id` to `AppState`, set in `playMediaItem()` |
| `src/apprt/gtk/player_controls.zig` | Add auto-play countdown overlay widget |

**Prerequisites (in `app.zig`):**
- Add `current_media_item_id: ?i64 = null` to `AppState`
- Set it in `playMediaItem()` before calling `switchToPlayer()`
- Clear it when player stops/pops

**Next episode query (`library.zig`):**

```sql
-- Same season, next episode
SELECT * FROM media_items
WHERE parent_id = :season_id AND episode_number > :current_episode
ORDER BY episode_number ASC LIMIT 1;

-- If no result, cross-season: find next season, get first episode
SELECT * FROM media_items
WHERE parent_id = (
    SELECT id FROM media_items
    WHERE parent_id = :show_id AND season_number > :current_season
    ORDER BY season_number ASC LIMIT 1
) ORDER BY episode_number ASC LIMIT 1;
```

**Plex markers API (`client.zig`):**

```zig
pub fn getMarkers(self: *PlexClient, rating_key: []const u8) ![]PlexMarker {
    // GET {server}/library/metadata/{key}?includeMarkers=1
    // Parse Marker array from response: type, startTimeOffset, endTimeOffset
}
```

**Auto-play state machine:**

```
States:
  idle            — No auto-play active (movie, no next episode, direct play)
  monitoring      — Watching position, waiting for trigger
  countdown_active — Overlay visible, counting down
  transitioning   — Loading next episode

Transitions:
  idle → monitoring       : Episode starts playing AND next episode exists
  monitoring → countdown  : Position crosses credits marker (or 30s-remaining fallback, or EOF)
  countdown → idle        : User clicks Cancel
  countdown → monitoring  : User seeks backward past trigger point
  countdown → transitioning : Countdown reaches 0 OR user clicks Play Now
  transitioning → monitoring : Next episode loaded (if it also has a next episode)
  transitioning → idle     : Next episode loaded (no further episodes)
  any → idle              : Playback stops, user navigates away
```

**Countdown overlay (GTK):**
- A `GtkBox` added as an overlay on the video area, `GTK_ALIGN_END` horizontal, with 64px bottom margin (clears controls bar)
- Contains: next episode poster (80x120 `GtkPicture`), vertical box with title label + "Playing in Xs..." label, Cancel button, Play Now button
- Poster loaded from image cache via existing async pipeline
- Countdown driven by `g_timeout_add_seconds(1, ...)` decrementing a counter
- On playback pause: remove the timeout source (pauses countdown)
- On playback resume: re-add the timeout source
- On seek backward past trigger: hide overlay, return to `monitoring` state

**Trigger logic (in poll loop):**
- Check if auto-play state is `monitoring`
- If Plex credits markers available: trigger when `time_pos >= credits_marker.startTimeOffset / 1000`
- Else if duration known: trigger when `time_pos >= duration - 30`
- Else: trigger on EOF event
- On trigger: transition to `countdown_active`, show overlay, start 15-second real-time countdown

**Speed preservation on auto-play:**
- When transitioning to next episode, read current speed before loading new file
- After `loadFile()`, re-apply the speed value
- Speed only resets on manual new playback (user clicks play on a different item from library)

**Watch progress on transition:**
- Mark current episode as `watched = 1` in `watch_progress`
- Report timeline to Plex with `state=stopped`
- Load next episode; if it has existing partial progress, resume from that position; otherwise start at 0

**Success criteria:**
- [x] Countdown overlay appears at credits marker time for Plex episodes
- [x] Falls back to 30 seconds remaining for non-Plex content
- [x] Cancel dismisses overlay; Play Now skips immediately
- [x] Countdown pauses when playback is paused
- [x] Seek backward past trigger cancels countdown
- [x] Next episode loads and plays automatically
- [x] Speed preserved across auto-play transitions
- [x] Watch progress updated correctly (current marked watched, next starts appropriately)
- [x] No overlay for movies, standalone content, or last episode
- [x] No overlay in direct play mode (CLI argument)

## Acceptance Criteria

### Functional Requirements

- [ ] Chapter markers visible on seek bar for files with chapters
- [ ] Chapter prev/next via buttons and keyboard (`,` / `.`)
- [ ] Speed picker with 6 presets (0.5x–2x), resets on manual new playback
- [ ] External subtitle auto-detection for local files
- [ ] Subtitle track popover with embedded + external tracks + file picker
- [ ] Subtitle appearance settings in Settings view (font, size, colors, position)
- [ ] Auto-play countdown overlay for TV episodes with Cancel/Play Now
- [ ] Plex credits markers used as primary auto-play trigger

### Non-Functional Requirements

- [ ] All features work in both windowed and fullscreen modes
- [ ] No additional network requests during playback (markers fetched at start)
- [ ] Settings changes persist across app restarts
- [ ] Chapter marks reposition correctly on window resize
- [ ] Auto-play state machine handles all edge cases (pause, seek, stop)

## Dependencies & Prerequisites

- **mpv chapter properties** — `chapter-list/count`, `chapter`, `add chapter` command (stable mpv API)
- **mpv speed property** — `speed` (stable)
- **mpv sub-add command** — Loads external subtitle files (stable)
- **mpv sub-* appearance properties** — `sub-font`, `sub-font-size`, `sub-color`, etc. (stable)
- **Plex markers API** — `GET /library/metadata/{key}?includeMarkers=1` (available since Plex Media Server 1.25)
- **GtkFileDialog** — GTK 4.10+ (already in use for other features)
- **GtkColorDialogButton** — GTK 4.10+ (new dependency for color pickers)
- **PangoFontMap** — For font enumeration (already linked via GTK)

## Sources & References

### Origin

- **Brainstorm document:** [docs/brainstorms/2026-03-17-playback-features-infuse-parity-brainstorm.md](docs/brainstorms/2026-03-17-playback-features-infuse-parity-brainstorm.md) — Key decisions: chapter markers on seek bar (not list panel), speed presets (not fine-grained), Plex credits markers for auto-play trigger, local-only external subs, global subtitle settings in Settings view

### Internal References

- Player core: `src/core/player.zig:38-270` (Player struct, all methods)
- C ABI exports: `src/lib.zig:23-222` (PlayerWrapper, all exports)
- GTK player controls: `src/apprt/gtk/player_controls.zig:8-243` (Controls struct, layout)
- GTK keyboard shortcuts: `src/apprt/gtk/keys.zig:41-98` (all key bindings)
- Settings storage: `src/core/settings.zig:4-74` (get/set, well-known keys)
- Library queries: `src/core/library.zig:263-276` (getItemsByParent)
- Plex client: `src/net/plex/client.zig:6-341` (API methods)
- GTK settings view: `src/apprt/gtk/settings_view.zig:114-126` (existing Playback group)
- GTK app state: `src/apprt/gtk/app.zig:68-103` (AppState struct)
