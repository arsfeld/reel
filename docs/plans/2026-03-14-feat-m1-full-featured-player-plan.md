---
title: "M1: Full-Featured Player"
type: feat
status: completed
date: 2026-03-14
---

# M1: Full-Featured Player

## Overview

Transform the M0 walking skeleton into a polished video player with overlay controls, keyboard shortcuts, audio/subtitle track selection, fullscreen, screensaver inhibition, and drag-and-drop. All playback interaction goes through expanded `MpvBackend` methods, following TDD as described in CLAUDE.md.

## Design Decisions

These resolve the critical gaps identified during spec analysis:

1. **Controls overlay behavior:** Always-visible bottom bar in windowed mode. Auto-hide overlay in fullscreen only (matches Celluloid/GNOME Videos pattern).
2. **Controls while paused:** Always visible when paused, regardless of mode.
3. **Mouse cursor in fullscreen:** Hide with overlay after timeout, show on mouse movement.
4. **Keyboard shortcut scoping:** Check if focused widget is a text entry before processing shortcuts. Skip shortcut handling when text inputs are focused.
5. **Escape priority chain:** Close popover > exit fullscreen > do nothing.
6. **Volume range:** 0-150% slider with a visual marker at 100%. mpv's `audio-pitch-correction=yes` by default.
7. **Seek on drag:** Seek on release (not real-time preview). Position label shows target time during drag.
8. **Drag-and-drop new file:** Replace immediately. Subtitle files (.srt/.ass/.vtt/.sub) load as subtitles when a video is playing.
9. **Speed on new file:** Reset to 1.0x on new file load.
10. **Double-click:** Fullscreen toggle on double-click (per product.md F1.6).
11. **Screensaver:** Release immediately on pause (per F1.7 spec). No inhibition for audio-only files.
12. **Forced subtitles:** Always display regardless of subtitle preference (mpv default).
13. **Seek boundaries:** Clamp to 0 at start, clamp to duration at end (mpv handles this natively).

## Implementation Phases

M1 is split into 4 phases. Each phase produces a working, testable increment.

---

### Phase 1: MpvBackend Expansion + Controls Foundation

**Goal:** Expand MpvBackend with all playback control methods. Create the PlayerControls component with play/pause, progress bar, position/duration labels, and volume.

**Files to create/modify:**

#### 1a. Expand MpvBackend (`src/player/mpv/mod.rs`)

Add methods (all delegate to mpv commands/properties):

```
seek_absolute(position_secs: f64)     → mpv.command("seek", &[pos, "absolute"])
seek_relative(offset_secs: f64)       → mpv.command("seek", &[offset, "relative"])
set_volume(volume: f64)               → mpv.set_property("volume", volume)
set_mute(mute: bool)                  → mpv.set_property("mute", mute)
set_speed(speed: f64)                 → mpv.set_property("speed", speed)
set_audio_track(track_id: i64)        → mpv.set_property("aid", track_id)
set_subtitle_track(track_id: i64)     → mpv.set_property("sid", track_id)
set_subtitle_none()                   → mpv.set_property("sid", "no")
add_subtitle_file(path: &str)         → mpv.command("sub-add", &[path, "select"])
set_chapter(chapter: i64)             → mpv.set_property("chapter", chapter)
chapter_count() -> i64                → mpv.get_property("chapters")
volume() -> f64                       → mpv.get_property("volume")
is_muted() -> bool                    → mpv.get_property("mute")
current_speed() -> f64                → mpv.get_property("speed")
tracks() -> Vec<TrackInfo>            → parse mpv.get_property("track-list") MpvNode
```

#### 1b. Add TrackInfo types (`src/player/backend.rs`)

```rust
pub struct TrackInfo {
    pub id: i64,
    pub track_type: TrackType,
    pub title: Option<String>,
    pub language: Option<String>,
    pub codec: Option<String>,
    pub selected: bool,
    pub default: bool,
    pub forced: bool,
    pub external: bool,
    // Audio-specific
    pub channels: Option<i64>,
    pub sample_rate: Option<i64>,
    // Video-specific
    pub width: Option<i64>,
    pub height: Option<i64>,
}

pub enum TrackType { Audio, Video, Subtitle }
```

Add display formatting functions with tests:

```rust
pub fn format_track_label(track: &TrackInfo) -> String  // "English (AAC 5.1)" / "Japanese (DTS-HD)"
pub fn format_volume_label(volume: f64) -> String        // "75%" / "150%"
pub fn format_speed_label(speed: f64) -> String          // "1.0x" / "1.5x" / "0.5x"
```

#### 1c. Expand PollData and PlaybackTracker (`src/player/playback_tracker.rs`)

Add fields to PollData:
```rust
pub volume: Option<f64>,
pub muted: Option<bool>,
pub speed: Option<f64>,
```

Add events:
```rust
PlaybackEvent::VolumeChanged { volume: f64, muted: bool },
PlaybackEvent::SpeedChanged(f64),
```

#### 1d. Expand VideoAreaOutput + VideoAreaMsg

New messages:
```rust
VideoAreaMsg::SeekAbsolute(f64),
VideoAreaMsg::SeekRelative(f64),
VideoAreaMsg::SetVolume(f64),
VideoAreaMsg::ToggleMute,
VideoAreaMsg::SetSpeed(f64),
VideoAreaMsg::SetAudioTrack(i64),
VideoAreaMsg::SetSubtitleTrack(i64),
VideoAreaMsg::DisableSubtitles,
VideoAreaMsg::SetChapter(i64),
```

New outputs:
```rust
VideoAreaOutput::VolumeChanged { volume: f64, muted: bool },
VideoAreaOutput::SpeedChanged(f64),
VideoAreaOutput::TracksChanged(Vec<TrackInfo>),
```

#### 1e. Create PlayerControls component (`src/components/player/controls.rs`)

Widget layout (bottom bar):
```
GtkBox (horizontal, bottom of overlay)
├── GtkButton (play/pause icon)
├── GtkLabel (position "1:23:45")
├── GtkScale (progress bar, range 0.0-1.0)
├── GtkLabel (remaining "-0:45:30")
├── GtkButton (volume icon: speaker/muted)
├── GtkScale (volume, range 0.0-1.5)
└── GtkButton (fullscreen icon)
```

Component messages:
```rust
ControlsInput::UpdatePosition { position: f64, duration: f64 },
ControlsInput::UpdatePlayState(PlayState),
ControlsInput::UpdateVolume { volume: f64, muted: bool },
ControlsInput::UpdateSpeed(f64),
ControlsInput::UpdateTracks(Vec<TrackInfo>),

ControlsOutput::TogglePause,
ControlsOutput::SeekTo(f64),
ControlsOutput::SetVolume(f64),
ControlsOutput::ToggleMute,
ControlsOutput::ToggleFullscreen,
```

#### 1f. Wire controls into VideoArea

- Add PlayerControls as overlay child on the GtkOverlay
- Forward VideoArea outputs to PlayerControls inputs
- Forward PlayerControls outputs back through VideoArea to mpv

#### 1g. CSS (`src/style.css`)

- Semi-transparent dark background for controls bar
- Progress bar styling (played portion colored)
- Volume marker at 100%
- Button icon sizing

**Tests (Phase 1):**

- `backend.rs`: `format_track_label` for various track configs (language+codec, no language, surround channels, stereo)
- `backend.rs`: `format_volume_label` for 0%, 50%, 100%, 150%
- `backend.rs`: `format_speed_label` for 0.25x, 0.5x, 1.0x, 1.5x, 2.0x, 4.0x
- `playback_tracker.rs`: volume change events, speed change events, mute transitions
- `playback_tracker.rs`: tracks changed event
- TrackInfo display formatting edge cases

**Acceptance Criteria:**
- [x] Play/pause button toggles playback state
- [x] Progress bar shows position, clicking seeks
- [x] Position and remaining time labels update during playback
- [x] Volume slider adjusts volume, mute button works
- [x] Fullscreen button enters fullscreen
- [x] All new MpvBackend methods compile and are callable

---

### Phase 2: Keyboard Shortcuts + Fullscreen + Auto-Hide

**Goal:** Full keyboard control, proper fullscreen behavior, auto-hide controls overlay in fullscreen.

#### 2a. Keyboard shortcut map (`src/components/player/shortcuts.rs`)

Pure function mapping key + modifiers to player actions:

```rust
pub enum PlayerAction {
    TogglePause,
    SeekForward(f64),    // seconds
    SeekBackward(f64),
    VolumeUp(f64),       // percent increment
    VolumeDown(f64),
    ToggleMute,
    ToggleFullscreen,
    ExitFullscreen,
    SpeedUp,             // step through presets: 1.0 → 1.25 → 1.5 → 1.75 → 2.0 → 3.0 → 4.0
    SpeedDown,           // reverse
    SpeedReset,
    NextChapter,
    PrevChapter,
}

/// Map a key press to a player action. Returns None if the key is not a shortcut.
/// Pure function — fully testable.
pub fn map_key_to_action(
    key: gdk::Key,
    modifiers: gdk::ModifierType,
    is_text_input_focused: bool,
) -> Option<PlayerAction> {
    if is_text_input_focused {
        return None;
    }
    match (key, modifiers.is_empty()) {
        (Key::space, true)    => Some(PlayerAction::TogglePause),
        (Key::Right, true)    => Some(PlayerAction::SeekForward(10.0)),
        (Key::Left, true)     => Some(PlayerAction::SeekBackward(10.0)),
        (Key::Up, true)       => Some(PlayerAction::VolumeUp(5.0)),
        (Key::Down, true)     => Some(PlayerAction::VolumeDown(5.0)),
        (Key::m, true)        => Some(PlayerAction::ToggleMute),
        (Key::F11, _)         => Some(PlayerAction::ToggleFullscreen),
        (Key::Escape, _)      => Some(PlayerAction::ExitFullscreen),
        (Key::bracketright, true) => Some(PlayerAction::SpeedUp),
        (Key::bracketleft, true)  => Some(PlayerAction::SpeedDown),
        (Key::BackSpace, true)    => Some(PlayerAction::SpeedReset),
        // Shift+Right = 60s seek, Shift+Left = 60s seek back
        (Key::Right, false) if modifiers.contains(gdk::ModifierType::SHIFT_MASK)
            => Some(PlayerAction::SeekForward(60.0)),
        (Key::Left, false) if modifiers.contains(gdk::ModifierType::SHIFT_MASK)
            => Some(PlayerAction::SeekBackward(60.0)),
        _ => None,
    }
}
```

#### 2b. Fullscreen management

In App component:
- `AppMsg::ToggleFullscreen` → `root.set_fullscreened(!root.is_fullscreened())`
- `AppMsg::ExitFullscreen` → `root.set_fullscreened(false)` (only if fullscreen)
- Track fullscreen state for overlay mode switching

#### 2c. Auto-hide overlay (`src/components/player/overlay_controller.rs`)

Pure state machine for overlay visibility:

```rust
pub struct OverlayState {
    pub visible: bool,
    pub is_fullscreen: bool,
    pub is_paused: bool,
    pub popover_open: bool,
}

pub enum OverlayAction {
    MouseMoved,
    TimeoutFired,
    PauseChanged(bool),
    FullscreenChanged(bool),
    PopoverOpened,
    PopoverClosed,
}

/// Pure function: given current state and an action, return new visibility.
pub fn compute_overlay_visibility(state: &OverlayState, action: OverlayAction) -> bool
```

Rules:
- Windowed mode → always visible
- Fullscreen + paused → always visible
- Fullscreen + playing + popover open → visible
- Fullscreen + playing + mouse moved → visible, restart 3s timer
- Fullscreen + playing + timeout fired → hidden

#### 2d. Mouse cursor hiding

- Hide cursor on overlay hide (fullscreen only)
- Show cursor on mouse move
- Use `gtk4::Widget::set_cursor_from_name("none")` / `set_cursor_from_name("default")`

#### 2e. Double-click fullscreen

- Add `GtkGestureClick` on GLArea
- Single click: no-op (or toggle pause — TBD, can defer)
- Double click: toggle fullscreen

**Tests (Phase 2):**

- `shortcuts.rs`: Every key combination maps to correct action
- `shortcuts.rs`: Text input focused → returns None for all keys
- `shortcuts.rs`: Unknown keys return None
- `shortcuts.rs`: Modifier key combinations (Shift+Right = 60s)
- `overlay_controller.rs`: All state transitions (8+ tests covering each rule)
- `overlay_controller.rs`: Popover prevents auto-hide
- `overlay_controller.rs`: Windowed mode always visible
- `overlay_controller.rs`: Paused always visible
- Speed preset stepping (SpeedUp/SpeedDown logic)

**Acceptance Criteria:**
- [x] Space toggles pause, arrows seek, up/down change volume
- [x] F11 toggles fullscreen, Escape exits fullscreen
- [x] Controls auto-hide in fullscreen after 3s, reappear on mouse move
- [x] Controls stay visible when paused or in windowed mode
- [x] Mouse cursor hides with overlay in fullscreen
- [x] Double-click on video toggles fullscreen
- [x] Keyboard shortcuts do not fire when typing in a text input

---

### Phase 3: Track Selection + Subtitles

**Goal:** Audio and subtitle track selection popovers, external subtitle loading, auto-detection.

#### 3a. Track list parsing (`src/player/mpv/tracks.rs`)

Parse mpv's `track-list` MpvNode property into `Vec<TrackInfo>`. This is the most complex data transformation in the mpv integration.

```rust
pub fn parse_track_list(node: &MpvNode) -> Vec<TrackInfo>
```

Tests with synthetic MpvNode data covering:
- Multiple audio tracks (different languages, codecs, channel counts)
- Multiple subtitle tracks (embedded + external)
- Forced subtitle tracks
- Default track marking
- Files with no subtitle tracks
- Files with no audio tracks (rare but possible)

#### 3b. TrackSelector component (`src/components/player/track_selector.rs`)

A popover showing available tracks, grouped by type:

```
GtkMenuButton (in controls bar) → GtkPopover
├── "Audio" section header
│   ├── RadioButton: "English (AAC 5.1)" ✓
│   ├── RadioButton: "Japanese (DTS-HD 7.1)"
│   └── RadioButton: "Commentary (AAC Stereo)"
├── Separator
├── "Subtitles" section header
│   ├── RadioButton: "None" ✓
│   ├── RadioButton: "English (SRT)"
│   ├── RadioButton: "Spanish (ASS)"
│   └── RadioButton: "English (PGS) [Forced]"
├── Separator
└── Button: "Load Subtitle File..."
```

Messages:
```rust
TrackSelectorInput::SetTracks(Vec<TrackInfo>),
TrackSelectorOutput::SelectAudioTrack(i64),
TrackSelectorOutput::SelectSubtitleTrack(i64),
TrackSelectorOutput::DisableSubtitles,
TrackSelectorOutput::LoadSubtitleFile,
```

#### 3c. External subtitle auto-detection (`src/player/subtitles.rs`)

Pure function:
```rust
/// Given a video file path, find matching subtitle files in the same directory.
pub fn find_matching_subtitles(video_path: &Path) -> Vec<PathBuf>
```

Matching rules:
- Same filename stem with subtitle extension (.srt, .ass, .ssa, .vtt, .sub, .idx)
- Same stem + language suffix (e.g., `movie.en.srt`, `movie.japanese.ass`)

#### 3d. Subtitle file loading via file chooser

Add "Load Subtitle File..." button in track selector that opens a `FileDialog` filtered to subtitle extensions.

**Tests (Phase 3):**

- `tracks.rs`: Parse track list with multiple audio/subtitle tracks
- `tracks.rs`: Parse empty track list
- `tracks.rs`: Parse with forced/default/external flags
- `subtitles.rs`: Find matching subtitles by stem
- `subtitles.rs`: Find language-suffixed subtitles
- `subtitles.rs`: No matches when no subtitle files exist
- `subtitles.rs`: Ignore non-subtitle extensions
- `backend.rs`: `format_track_label` for all track type variants

**Acceptance Criteria:**
- [x] Audio track popover shows all tracks with language/codec/channels
- [x] Selecting an audio track switches audio without interruption
- [x] Subtitle popover shows all tracks plus "None" option
- [x] Selecting a subtitle track enables it
- [x] "None" disables subtitles
- [x] "Load Subtitle File..." opens file chooser, loads selected file
- [x] External subtitles with matching filenames auto-detected on file load
- [x] Forced subtitle tracks labeled as "[Forced]"

---

### Phase 4: Speed + Chapters + Screensaver + DnD + Polish

**Goal:** Remaining M1 features and polish.

#### 4a. Speed control UI

Add speed button to controls bar (shows current speed, e.g. "1.0x"):
- Click opens popover with preset speeds: 0.25x, 0.5x, 0.75x, 1.0x, 1.25x, 1.5x, 1.75x, 2.0x, 3.0x, 4.0x
- Current speed highlighted
- Speed resets to 1.0x on new file load

Speed stepping logic (for keyboard shortcuts):
```rust
pub fn next_speed(current: f64) -> f64
pub fn prev_speed(current: f64) -> f64
```

#### 4b. Chapter navigation

- Add prev/next chapter buttons (only visible when chapters > 0)
- Chapter markers on progress bar (subtle tick marks)
- `chapter_count()` and `set_chapter()` from MpvBackend

#### 4c. Screensaver inhibition (`src/services/screensaver.rs`)

```rust
pub struct ScreensaverInhibitor { /* D-Bus connection, cookie */ }

impl ScreensaverInhibitor {
    pub async fn inhibit(&mut self) -> Result<(), dbus::Error>
    pub async fn uninhibit(&mut self) -> Result<(), dbus::Error>
    pub fn is_inhibited(&self) -> bool
}
```

Integration:
- Inhibit when PlayState::Playing (video tracks present)
- Uninhibit on Pause, Stop, EOF, or app exit
- Do NOT inhibit for audio-only files

#### 4d. Drag-and-drop (`src/components/player/drop_target.rs`)

- `GtkDropTarget` on the window
- Accept `text/uri-list` MIME type
- If file is video → load as new file
- If file is subtitle (.srt/.ass/.vtt/.sub) and video is playing → load as subtitle
- If file is subtitle and no video is playing → ignore or show toast

#### 4e. Window state persistence

Save/restore in `$XDG_CONFIG_HOME/reel/window.toml`:
```toml
width = 1280
height = 720
x = 100
y = 100
maximized = false
volume = 100.0
```

Do NOT persist fullscreen (surprise on startup) or speed (per-file).

#### 4f. Error handling with toasts

- Add `AdwToastOverlay` to the App component
- Playback errors → descriptive toast (e.g., "Cannot play: codec not supported")
- Subtitle load errors → toast ("Could not load subtitle file")
- mpv init failure → `AdwStatusPage` with error message instead of black screen

**Tests (Phase 4):**

- `screensaver.rs`: State transitions (inhibit/uninhibit/is_inhibited)
- Speed stepping: `next_speed(1.0) == 1.25`, `prev_speed(1.0) == 0.75`, boundary clamping
- Drop target: classify file extension as video vs subtitle vs unknown
- Window state: serialize/deserialize TOML round-trip
- `playback_tracker.rs`: test that audio-only detection works (no video track in TrackInfo)

**Acceptance Criteria:**
- [x] Speed control via ] and [ keyboard shortcuts (SpeedUp/SpeedDown/SpeedReset)
- [x] Speed control popover UI with 10 presets
- [x] Chapter prev/next buttons visible when chapters exist
- [x] Screensaver inhibited during video playback, released on pause
- [x] Drag-and-drop video file replaces current playback
- [x] Drag-and-drop subtitle file loads as subtitle
- [x] Window size/position restored on launch
- [x] Playback errors shown as toasts via AdwToastOverlay
- [ ] mpv init failure shows error page instead of black screen (deferred to M5 polish)

---

## Technical Considerations

### Architecture Impacts

- **MpvBackend expansion** adds ~15 methods. All are thin wrappers around mpv properties/commands. No trait formalization yet — defer `VideoBackend` trait to when a second backend (GStreamer) is needed.
- **PlayerControls** is the first multi-widget Relm4 component. Establishes the pattern for all future UI components (sidebar, library, detail pages).
- **CSS theming** establishes the visual language for the entire app.

### Performance

- Progress bar updates at 100ms (poll interval). Smooth enough for a progress bar; no need to increase frequency.
- Track list parsing happens once on file load, not per-poll.
- Screensaver D-Bus calls are infrequent (play/pause transitions).

### TDD Approach Per Phase

| Phase | Testable Pure Functions | Mock-based Tests | Manual Testing |
|-------|------------------------|------------------|----------------|
| 1 | format_track_label, format_volume_label, format_speed_label, PollData expansion | None yet | Controls layout, click behavior |
| 2 | map_key_to_action, compute_overlay_visibility, speed stepping | None yet | Auto-hide timing, cursor hiding |
| 3 | parse_track_list, find_matching_subtitles, format_track_label edge cases | None yet | Popover layout, track switching |
| 4 | next_speed/prev_speed, file extension classification, TOML round-trip | Screensaver mock | DnD, toasts, window restore |

### Dependencies

- Phase 1 is independent
- Phase 2 depends on Phase 1 (controls exist to show/hide)
- Phase 3 depends on Phase 1 (MpvBackend track methods, controls bar has track button)
- Phase 4 depends on Phases 1-2 (speed UI in controls, fullscreen for screensaver logic)

### New Dev Dependencies

```toml
[dependencies]
# For screensaver inhibition (Phase 4)
zbus = "5"
```

---

## Success Criteria (M1 Complete)

- [x] Can play any format mpv/FFmpeg supports with full control
- [x] Controls overlay with play/pause, progress bar, volume, track selector
- [x] Controls auto-hide in fullscreen, stay visible when paused/windowed
- [x] Audio/subtitle tracks switchable during playback
- [x] External subtitle auto-detection and manual loading
- [x] All keyboard shortcuts functional (space, arrows, F11, Esc, m, ], [)
- [x] Fullscreen via F11, Escape, button, and double-click
- [x] Playback speed adjustable 0.25x-4x (keyboard + popover)
- [x] Chapter navigation when chapters present
- [x] Screensaver inhibited during video playback
- [x] Drag-and-drop video and subtitle files
- [x] Window size/position persisted across sessions
- [x] Playback errors shown as toasts
- [x] All pure functions have tests (186 tests, target was 100+)
- [x] `cargo clippy` clean, `cargo fmt` clean, zero warnings

## Sources

- Roadmap M1: `roadmap.md:58-91`
- Product spec F1: `product.md:29-97`
- VideoBackend trait: `tech.md:360-502`
- Widget hierarchy: `tech.md:611-623`
- TDD guide: `CLAUDE.md`
- Current App keyboard handler: `src/app.rs:46-55`
- Current VideoArea: `src/components/player/video_area.rs`
- Current MpvBackend: `src/player/mpv/mod.rs`
- Current PlaybackTracker: `src/player/playback_tracker.rs`
