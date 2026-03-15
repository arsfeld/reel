---
title: "feat: M5 Polish - MPRIS, Settings, Error Audit, Desktop Integration"
type: feat
status: active
date: 2026-03-14
---

# M5: Polish & Desktop Integration

## Overview

M0-M4 are complete. Reel is a functional Plex-integrated media player with a library browser, detail pages, search/filter/sort, watch state tracking, scrobble, resume, and a feature-rich player with overlay controls and keyboard shortcuts. There are 463 tests, a fake Plex server for e2e testing, and in-memory SQLite for DB tests.

What's missing: no MPRIS (desktop media controls), no settings UI, inconsistent error handling, no desktop integration files (.desktop, icon, AppStream), and several deferred items from earlier milestones (skip buttons, subtitle customization). Two TODOs in `app.rs` leave Plex scrobble/timeline calls unwired.

M5 closes these gaps. Users will get desktop media control integration, a preferences window, polished error handling, and the app will be ready for packaging.

## Problem Statement

1. **No desktop integration**: GNOME/KDE media widgets cannot control Reel. No `.desktop` file means Reel doesn't appear in app launchers or as a default video player.
2. **No settings UI**: HW acceleration, skip intervals, subtitle preferences, dark mode, and volume defaults are all hardcoded. Users cannot configure behavior.
3. **Inconsistent error handling**: Mix of `thiserror` enums, raw `String` errors, and `unwrap()`/`expect()` in production code. Two Plex API TODOs remain unwired.
4. **No first-run guidance**: Empty library with no direction on first launch.
5. **Code debt**: 75 `#[allow(dead_code)]` annotations, hand-rolled TOML in `window_state.rs`.

## Proposed Solution

Seven phases, ordered by dependency:

1. Error handling audit and Plex scrobble/timeline wiring
2. Window state serde migration
3. Settings model and TOML persistence
4. Settings UI (AdwPreferencesDialog + AdwAboutDialog)
5. MPRIS2 D-Bus integration via `mpris-server` crate
6. UI polish (skip buttons, subtitle customization, first-run)
7. Desktop integration files and code cleanup

## Technical Approach

### Architecture

```
┌──────────────────────────────────────────────────────────┐
│ Desktop Environment (GNOME Shell, KDE Plasma)             │
│  Media widget: play/pause/seek/volume/metadata            │
└────────────────┬──────────────────────┬──────────────────┘
                 │ D-Bus (MPRIS2)       │ PropertiesChanged
┌────────────────┴──────────────────────┴──────────────────┐
│ MPRIS Service (src/services/mpris.rs)                     │
│  mpris-server crate, tokio task, watch channels           │
│  Pure functions: state -> MPRIS properties (testable)     │
└────────────────┬──────────────────────┬──────────────────┘
                 │ MprisCommand         │ watch::Sender
┌────────────────┴──────────────────────┴──────────────────┐
│ App (Relm4 root component)                                │
│  Settings model │ now_playing │ PlexSource ref             │
│  dispatch_watch_events → Plex scrobble/timeline           │
└────────────────┬──────────────────────┬──────────────────┘
                 │                      │
┌────────────────┴────────┐  ┌──────────┴──────────────────┐
│ Settings (TOML)          │  │ AdwPreferencesDialog          │
│  config_dir()/settings   │  │  Playback│Subtitles│Library   │
│  .toml                   │  │  About (AdwAboutDialog)       │
└─────────────────────────┘  └─────────────────────────────┘
```

---

### Phase 1: Error Handling Audit & Plex Wiring

**Goal**: Unified error strategy, wire the two remaining Plex TODOs, eliminate unsafe unwraps.

#### 1a. Wire Plex Scrobble/Timeline

The two TODOs at `src/app.rs:864` and `src/app.rs:875` exist because `dispatch_watch_events()` has no reference to the active `PlexSource`. The `PlexSource` has `scrobble()`, `report_progress()`, and `unscrobble()` methods already implemented.

**Changes:**

Store the active source in `App`:

```rust
// In App model
pub struct App {
    // ... existing fields ...
    active_source: Option<Arc<dyn MediaSource>>,
}
```

Update `dispatch_watch_events` to accept the source and use `sender.oneshot_command()` for async calls:

```rust
fn dispatch_watch_events(
    events: &[WatchStateEvent],
    source: Option<&Arc<dyn MediaSource>>,
    sender: &ComponentSender<App>,
    // ...
) {
    for event in events {
        match event {
            WatchStateEvent::Scrobble { media_id } => {
                if let Some(source) = source.cloned() {
                    let id = media_id.clone();
                    sender.oneshot_command(async move {
                        if let Err(e) = source.scrobble(&id).await {
                            tracing::warn!("Plex scrobble failed: {e}");
                        }
                        AppCommandOutput::Noop
                    });
                }
            }
            WatchStateEvent::ReportTimeline { rating_key, state, time_ms, duration_ms } => {
                if let Some(source) = source.cloned() {
                    let key = rating_key.clone();
                    let st = state.clone();
                    let t = *time_ms;
                    let d = *duration_ms;
                    sender.oneshot_command(async move {
                        if let Err(e) = source.report_progress(&key, t, d, &st).await {
                            tracing::warn!("Plex timeline failed: {e}");
                        }
                        AppCommandOutput::Noop
                    });
                }
            }
            // ... other events handled synchronously ...
        }
    }
}
```

#### 1b. Window State Error Type

Replace `Result<(), String>` in `window_state.rs` with a proper error enum:

```rust
// src/services/window_state.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WindowStateError {
    #[error("Could not determine config directory")]
    NoConfigDir,
    #[error("Failed to create config directory: {0}")]
    CreateDir(#[from] std::io::Error),
    #[error("Failed to write window state: {0}")]
    Write(std::io::Error),
    #[error("Failed to parse window state")]
    Parse,
}
```

#### 1c. Unwrap/Expect Audit

| Location | Current | Fix |
|----------|---------|-----|
| `app.rs:1056` | `reqwest::Client::builder().build().unwrap()` | Use `?` or `map_err` with tracing |
| `mpv/mod.rs:16` | `CString::new("...").unwrap()` | Safe for static strings, add comment |
| `mpv/gl_render.rs:94` | `CString::new().unwrap()` | Safe for static strings, add comment |
| `services/artwork.rs:46` | `semaphore.acquire().expect()` | Replace with `?` in async context |
| `services/plex/api.rs:32` | `Client::builder().build().expect()` | Return `Result`, propagate |

#### 1d. Error Display Strategy

| Error Type | Display Mechanism | Example |
|------------|------------------|---------|
| Network timeout/failure | Toast (3s, auto-dismiss) | "Could not reach Plex server" |
| Authentication failure | Toast + action button | "Plex token expired. Reconnect?" |
| Playback error | Toast (persistent until dismissed) | "Failed to load: codec not found" |
| Database init failure | AdwStatusPage (blocking) | "Database error" with retry button |
| No sources on first run | AdwStatusPage (guidance) | "Welcome to Reel" with connect/open buttons |
| File not found | Toast | "File not found: /path/to/file.mkv" |

**Files:**
- `src/services/window_state.rs` -- new error type
- `src/app.rs` -- add `active_source`, update `dispatch_watch_events`
- `src/services/plex/api.rs` -- remove `expect()` from `PlexClient::new`
- `src/services/artwork.rs` -- propagate semaphore error

**Tests:**
- `dispatch_watch_events` emits scrobble command when source is Some
- `dispatch_watch_events` skips scrobble when source is None
- `WindowStateError` variants are distinct and `Display`-able
- `PlexClient::new` returns `Result` instead of panicking

---

### Phase 2: Window State Serde Migration

**Goal**: Replace hand-rolled TOML serialization/deserialization with `serde + toml` crate.

**Dependencies to add:**

```toml
# In [dependencies]
toml = "0.8"
```

**New `window_state.rs`:**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowState {
    pub width: i32,
    pub height: i32,
    pub maximized: bool,
    pub volume: f64,
    pub view_mode: String,
    pub grid_density: String,
}

impl Default for WindowState { /* same as current */ }

pub fn save(state: &WindowState) -> Result<(), WindowStateError> {
    let path = state_file_path().ok_or(WindowStateError::NoConfigDir)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(state)
        .map_err(|e| WindowStateError::Serialize(e.to_string()))?;
    std::fs::write(&path, content)?;
    Ok(())
}

pub fn load() -> WindowState {
    let Some(path) = state_file_path() else {
        return WindowState::default();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return WindowState::default();
    };
    toml::from_str(&content).unwrap_or_default()
}
```

**Backward compatibility**: The serde TOML output format is compatible with the current hand-rolled format. `#[serde(default)]` ensures old files with missing fields parse correctly.

**Files:**
- `src/services/window_state.rs` -- replace serialize/deserialize functions
- `Cargo.toml` -- add `toml = "0.8"`

**Tests:**
- Existing roundtrip tests pass unchanged
- New: deserializing old-format TOML (hand-written) produces correct `WindowState`
- New: unknown keys are silently ignored (serde default behavior with `#[serde(default)]`)
- New: corrupt/unparseable TOML falls back to defaults

---

### Phase 3: Settings Model & TOML Persistence

**Goal**: Define the `Settings` struct with all preferences, persist to `config_dir()/settings.toml`.

```rust
// src/settings.rs

use serde::{Deserialize, Serialize};
use crate::config;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub playback: PlaybackSettings,
    pub subtitles: SubtitleSettings,
    pub library: LibrarySettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackSettings {
    /// Default volume (0-150)
    pub default_volume: f64,
    /// Hardware decoding mode: "auto", "auto-safe", "vaapi", "nvdec", "none"
    pub hwdec_mode: String,
    /// Whether to show resume overlay for in-progress media
    pub resume_playback: bool,
    /// Short skip interval in seconds (arrow keys)
    pub skip_short_secs: f64,
    /// Long skip interval in seconds (Shift+arrow keys)
    pub skip_long_secs: f64,
    /// Default playback speed (0.25-4.0)
    pub default_speed: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SubtitleSettings {
    /// Preferred subtitle language (ISO 639-1, e.g. "en", "es")
    pub preferred_language: Option<String>,
    /// Subtitle font family
    pub font_family: String,
    /// Subtitle font size (16-72)
    pub font_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LibrarySettings {
    /// Default sort field: "title", "year", "added", "rating"
    pub default_sort: String,
    /// Sort ascending
    pub sort_ascending: bool,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            default_volume: 100.0,
            hwdec_mode: "auto-safe".to_string(),
            resume_playback: true,
            skip_short_secs: 10.0,
            skip_long_secs: 60.0,
            default_speed: 1.0,
        }
    }
}

impl Default for SubtitleSettings {
    fn default() -> Self {
        Self {
            preferred_language: None,
            font_family: "Sans".to_string(),
            font_size: 36,
        }
    }
}

impl Default for LibrarySettings {
    fn default() -> Self {
        Self {
            default_sort: "title".to_string(),
            sort_ascending: true,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            playback: PlaybackSettings::default(),
            subtitles: SubtitleSettings::default(),
            library: LibrarySettings::default(),
        }
    }
}

impl Settings {
    pub fn settings_path() -> std::path::PathBuf {
        config::config_dir().join("settings.toml")
    }

    pub fn load() -> Self {
        let path = Self::settings_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<(), SettingsError> {
        let path = Self::settings_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

/// Validate a setting value, clamping to valid range.
pub fn clamp_volume(v: f64) -> f64 {
    v.clamp(0.0, 150.0)
}

pub fn clamp_skip_interval(v: f64) -> f64 {
    v.clamp(1.0, 120.0)
}

pub fn clamp_speed(v: f64) -> f64 {
    v.clamp(0.25, 4.0)
}

pub fn clamp_font_size(v: u32) -> u32 {
    v.clamp(16, 72)
}
```

**Settings vs. WindowState**: These remain separate files. WindowState tracks runtime geometry (width, height, maximized). Settings tracks user preferences. `volume` stays in WindowState as it represents the last-used volume, not a "default" -- the Settings `default_volume` is applied on first launch only.

**Files:**
- `src/settings.rs` -- new module
- `src/main.rs` -- add `mod settings`
- Remove `#[allow(dead_code)]` from `config::config_dir()`

**Tests (25+ cases):**
- Default settings have sensible values
- Roundtrip serialize/deserialize
- Partial TOML (forward compat): file with only `[playback]` section, missing fields get defaults
- Empty file falls back to defaults
- Corrupt TOML falls back to defaults
- Validation: `clamp_volume`, `clamp_skip_interval`, `clamp_speed`, `clamp_font_size`
- Each clamp function tested at boundaries and out-of-range values

---

### Phase 4: Settings UI

**Goal**: `AdwPreferencesDialog` with Playback, Subtitles, Library, and Connections pages. `AdwAboutDialog`.

**Dependency**: Bump libadwaita feature to `v1_5` for `PreferencesDialog` and `AboutDialog`:

```toml
libadwaita = { version = "0.8", features = ["v1_5"] }
```

#### 4a. Preferences Dialog Component

New Relm4 component `src/components/settings_dialog.rs`:

```
PreferencesDialog
  ├── Page: Playback (preferences-system-symbolic)
  │     ├── Group: General
  │     │     ├── SwitchRow: Resume Playback
  │     │     ├── ComboRow: Hardware Decoding (Auto, VAAPI, NVDEC, None)
  │     │     └── SpinRow: Default Volume (0-150)
  │     └── Group: Controls
  │           ├── SpinRow: Short Skip (seconds, 1-120)
  │           └── SpinRow: Long Skip (seconds, 1-120)
  │
  ├── Page: Subtitles (media-view-subtitles-symbolic)
  │     └── Group: Subtitles
  │           ├── EntryRow: Preferred Language (ISO 639-1)
  │           ├── EntryRow: Font Family
  │           └── SpinRow: Font Size (16-72)
  │
  ├── Page: Library (folder-videos-symbolic)
  │     └── Group: Display
  │           ├── ComboRow: Default Sort (Title, Year, Date Added, Rating)
  │           └── SwitchRow: Sort Ascending
  │
  └── Page: Connections (network-server-symbolic)
        └── Group: Plex Servers
              ├── ActionRow per saved source (title, URL, remove button)
              └── ActionRow: "Add Server..." (opens existing connection dialog)
```

**Widget-to-Settings binding pattern**: Load `Settings` on dialog open, update fields on widget change, save on dialog close:

```rust
impl SettingsDialog {
    fn build(settings: Settings) -> PreferencesDialog {
        // Create rows, set values from settings
        // Connect signals to update local settings copy
        // On dialog closed -> settings.save()
    }
}
```

**Connections page**: Reads sources from `SourceRepo` (SQLite), not the TOML config. Each source shows as an `ActionRow` with title, subtitle (URL), and a delete button suffix. Delete removes from DB and refreshes the list.

#### 4b. About Dialog

```rust
fn show_about(parent: &impl IsA<gtk::Widget>) {
    let about = adw::AboutDialog::builder()
        .application_name("Reel")
        .application_icon("dev.arsfeld.Reel")
        .version(env!("CARGO_PKG_VERSION"))
        .comments("A modern, native media player for the Linux desktop")
        .website("https://github.com/arosenfeld/reel")
        .issue_url("https://github.com/arosenfeld/reel/issues")
        .license_type(gtk::License::Gpl30)
        .developers(vec!["Alexandre Rosenfeld".to_string()])
        .build();
    about.present(Some(parent));
}
```

#### 4c. Settings Menu Integration

Add a menu button to the header bar (or sidebar) with items:
- Preferences (opens `PreferencesDialog`)
- About Reel (opens `AboutDialog`)

**Files:**
- `src/components/settings_dialog.rs` -- new module
- `src/components/mod.rs` -- add `pub mod settings_dialog`
- `src/app.rs` -- add `AppMsg::OpenSettings`, `AppMsg::OpenAbout`, menu button
- `Cargo.toml` -- bump libadwaita to `v1_5`

**Tests:**
- Settings dialog component is not directly testable (GTK), but:
- Settings model load/save roundtrip (Phase 3 tests cover this)
- About dialog fields derived from `Cargo.toml` metadata

---

### Phase 5: MPRIS2 D-Bus Integration

**Goal**: Desktop environments can see and control Reel via MPRIS2. The `mpris-server` crate (v0.9, wraps zbus v5) handles D-Bus plumbing.

**Dependency to add:**

```toml
mpris-server = "0.9"
```

#### 5a. Pure Property Derivation Functions

New module `src/services/mpris.rs` with testable pure functions:

```rust
use crate::player::backend::PlayState;

/// PlayState -> MPRIS PlaybackStatus
pub fn playback_status_from_state(state: PlayState) -> mpris_server::PlaybackStatus {
    match state {
        PlayState::Playing => mpris_server::PlaybackStatus::Playing,
        PlayState::Paused => mpris_server::PlaybackStatus::Paused,
        PlayState::Stopped => mpris_server::PlaybackStatus::Stopped,
    }
}

/// Seconds (f64, mpv) -> Microseconds (i64, MPRIS)
pub fn seconds_to_micros(seconds: f64) -> i64 {
    (seconds * 1_000_000.0) as i64
}

/// Microseconds (i64, MPRIS) -> Seconds (f64, mpv)
pub fn micros_to_seconds(micros: i64) -> f64 {
    micros as f64 / 1_000_000.0
}

/// Data needed to build MPRIS Metadata dict.
#[derive(Debug, Clone, Default)]
pub struct MprisMetadata {
    pub track_id: String,          // D-Bus object path, e.g. "/org/reel/track/1"
    pub title: Option<String>,
    pub duration_secs: Option<f64>,
    pub art_url: Option<String>,   // file:///path or https://...
    pub url: Option<String>,       // media URI
    pub album: Option<String>,     // show name or collection
    pub artist: Option<Vec<String>>, // directors/actors
}

/// Build Metadata from local struct. Pure, no I/O.
pub fn build_metadata(meta: &MprisMetadata) -> mpris_server::Metadata { /* ... */ }

/// Derive MprisMetadata from now_playing + file path
pub fn metadata_from_media_item(item: Option<&MediaItem>, path: &str, duration: f64) -> MprisMetadata { /* ... */ }

/// Derive MprisMetadata from mpv properties when no MediaItem (local file playback)
pub fn metadata_from_file(path: &str, media_title: Option<&str>, duration: f64) -> MprisMetadata { /* ... */ }
```

#### 5b. MPRIS Command Enum

```rust
pub enum MprisCommand {
    Play,
    Pause,
    PlayPause,
    Stop,
    Seek(i64),           // offset in microseconds
    SetPosition(i64),    // absolute in microseconds
    SetVolume(f64),      // 0.0-1.0
    OpenUri(String),
    Raise,
    Quit,
}
```

#### 5c. MPRIS Server (mpris-server Traits)

Implement `RootInterface` and `PlayerInterface` traits from `mpris-server`:

- `RootInterface`: Identity="Reel", CanQuit=true, CanRaise=true, HasTrackList=false, DesktopEntry="dev.arsfeld.Reel", SupportedUriSchemes=["file"], SupportedMimeTypes=[video types]
- `PlayerInterface`: Delegates to channels for state reads and command dispatch

State is read from `tokio::sync::watch` channels. Commands are sent via `tokio::sync::mpsc::UnboundedSender<MprisCommand>`.

#### 5d. Communication Architecture

```
GTK Thread (glib main loop)          │  Tokio Runtime
─────────────────────────────────────│──────────────────────────
App.update()                          │  MPRIS Server task
  │                                   │    │
  ├─ on state change:                │    ├─ on D-Bus method call:
  │   status_tx.send(PlayState)      │    │   cmd_tx.send(MprisCommand)
  │   meta_tx.send(MprisMetadata)    │    │
  │   vol_tx.send(volume)            │    ├─ property_watcher task:
  │   pos_tx.send(position_us)      │    │   watches status_rx, meta_rx, vol_rx
  │                                   │    │   calls server.properties_changed()
  ├─ on MprisCommand received:       │    │
  │   (via glib channel from tokio)  │    └─ Seeked signal emission
  │   dispatch to VideoAreaMsg       │
```

**Key implementation details:**

1. MPRIS server spawned on tokio runtime in `main.rs` or `App::init()`
2. `tokio::sync::watch` channels for state: PlayState, MprisMetadata, volume (f64), position (i64 microseconds)
3. `tokio::sync::mpsc::UnboundedSender` for commands back from D-Bus to GTK
4. `glib::spawn_future_local` or `sender.oneshot_command()` to relay MprisCommands into Relm4 message loop
5. Property watcher uses `tokio::select!` on all watch channels, emits `server.properties_changed()` on change
6. `Position` is NOT emitted via PropertiesChanged (per MPRIS spec) -- desktop environments poll it
7. `Seeked` signal emitted only on user-initiated seeks, not poll-driven position updates
8. Distinguish user seeks: emit `Seeked` from `App` when processing `SeekAbsolute`/`SeekRelative` messages, push new position through the position watch channel

#### 5e. MPRIS Metadata for Local Files

When `now_playing` is `None` (local file via CLI or drag-and-drop):
- `mpris:trackid`: `/org/reel/track/local` (fixed)
- `xesam:title`: mpv's `media-title` property (often derived from filename)
- `mpris:length`: duration in microseconds from mpv
- No artwork, no artist, no album

When `now_playing` is `Some(MediaItem)` (Plex media):
- `mpris:trackid`: `/org/reel/track/{media_item.id_hash}` (derive valid D-Bus path from media ID)
- `xesam:title`: media item title
- `mpris:length`: duration in microseconds
- `mpris:artUrl`: `file:///path/to/cached/artwork.jpg` (from artwork cache)
- `xesam:artist`: directors for movies, empty for TV

#### 5f. Bus Name and Lifecycle

- Bus name: `org.mpris.MediaPlayer2.Reel`
- Object path: `/org/mpris/MediaPlayer2`
- Server starts on app launch, stays active for app lifetime
- When nothing is playing: `PlaybackStatus=Stopped`, empty Metadata (with NoTrack trackid)
- When file loaded: properties update via watch channels

**Files:**
- `src/services/mpris.rs` -- pure functions, MprisCommand, MprisMetadata, trait impls
- `src/services/mpris_bridge.rs` -- server spawn, property watcher, command relay
- `src/services/mod.rs` -- add modules
- `src/app.rs` -- create watch channel senders, relay MprisCommands, emit Seeked
- `Cargo.toml` -- add `mpris-server = "0.9"`

**Tests (20+ cases):**
- `playback_status_from_state` for all three states
- `seconds_to_micros` / `micros_to_seconds` conversions (including edge cases: 0, negative, very large)
- `build_metadata` with full fields, with all-None optional fields, with empty track_id
- `metadata_from_media_item` with movie, with TV episode, with None
- `metadata_from_file` with various filenames
- `MprisCommand` channel: method call produces correct command
- Seeked signal only emitted on user seeks (test via tracker flag or explicit emission)
- Integration test (gated behind `#[cfg(feature = "integration")]`): full D-Bus roundtrip with `dbus-run-session`

---

### Phase 6: UI Polish

#### 6a. Skip Forward/Back Buttons

Add skip buttons to the player controls bar in `src/components/player/controls.rs`.

**Layout order**: skip-back, play/pause, skip-forward, then chapter prev/next.

```
[⏮ skip back] [⏯ play/pause] [⏭ skip forward] | [◁ chapter prev] [▷ chapter next] | position / duration | ... | speed | tracks | volume | fullscreen
```

- Skip intervals read from `Settings.playback.skip_short_secs`
- Button tooltips show the configured interval (e.g., "Skip back 10 seconds")
- Icons: `media-skip-backward-symbolic`, `media-skip-forward-symbolic`
- On click: emit `VideoAreaMsg::SeekRelative(-skip_secs)` / `VideoAreaMsg::SeekRelative(skip_secs)`

**Keyboard shortcuts** in `shortcuts.rs` also read from settings instead of hardcoded 10.0/60.0.

#### 6b. Subtitle Customization

Apply subtitle settings from `Settings.subtitles` to mpv on file load:

```rust
// In MpvBackend or VideoArea, on FileLoaded:
mpv.set_property("sub-font", &settings.subtitles.font_family)?;
mpv.set_property("sub-font-size", settings.subtitles.font_size as i64)?;
if let Some(ref lang) = settings.subtitles.preferred_language {
    mpv.set_property("slang", lang)?;
}
```

Settings changes apply live during playback (mpv supports runtime `sub-font` and `sub-font-size` changes).

#### 6c. First-Run Experience

On app startup, after loading sources from DB:

```rust
if sources.is_empty() {
    // Show welcome status page instead of empty library
    self.current_view = CurrentView::Welcome;
}
```

Welcome view (`AdwStatusPage`):
- Icon: `video-display-symbolic`
- Title: "Welcome to Reel"
- Description: "Connect to your Plex server or open a local video file"
- Buttons: "Connect to Plex" (opens OAuth dialog), "Open File" (opens file chooser)

After a source is added, automatically navigate to the library view.

**Files:**
- `src/components/player/controls.rs` -- add skip buttons
- `src/components/player/shortcuts.rs` -- read skip intervals from settings
- `src/player/mpv/mod.rs` -- apply subtitle settings on file load
- `src/app.rs` -- first-run detection, welcome view
- `src/components/welcome.rs` -- new welcome status page component (optional, could be inline in app)

**Tests:**
- Skip button sends correct seek relative value from settings
- Subtitle mpv properties set correctly from settings
- First-run detection: empty source list triggers welcome view
- First-run detection: non-empty source list shows library

---

### Phase 7: Desktop Integration & Code Cleanup

#### 7a. Desktop Entry File

`data/dev.arsfeld.Reel.desktop`:

```ini
[Desktop Entry]
Type=Application
Name=Reel
GenericName=Media Player
Comment=A modern, native media player for the Linux desktop
Icon=dev.arsfeld.Reel
Exec=reel %U
Terminal=false
Categories=AudioVideo;Video;Player;GTK;
Keywords=video;media;player;movie;mpv;plex;
StartupNotify=true
StartupWMClass=dev.arsfeld.Reel
MimeType=video/mp4;video/x-matroska;video/webm;video/x-msvideo;video/mpeg;video/ogg;video/quicktime;video/x-m4v;video/mp2t;application/x-matroska;
```

#### 7b. AppStream MetaInfo

`data/dev.arsfeld.Reel.metainfo.xml`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<component type="desktop-application">
  <id>dev.arsfeld.Reel</id>
  <metadata_license>CC0-1.0</metadata_license>
  <project_license>GPL-3.0-or-later</project_license>
  <name>Reel</name>
  <summary>A modern, native media player for the Linux desktop</summary>
  <developer id="dev.arsfeld">
    <name>Alexandre Rosenfeld</name>
  </developer>
  <description>
    <p>Reel is a beautiful media player for the Linux desktop with Plex integration,
       automatic metadata, and universal format support powered by mpv.</p>
  </description>
  <launchable type="desktop-id">dev.arsfeld.Reel.desktop</launchable>
  <url type="homepage">https://github.com/arosenfeld/reel</url>
  <url type="bugtracker">https://github.com/arosenfeld/reel/issues</url>
  <content_rating type="oars-1.1" />
  <releases>
    <release version="0.1.0" date="2026-03-14">
      <description><p>Initial release.</p></description>
    </release>
  </releases>
</component>
```

#### 7c. SVG App Icon

Create `data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg`:
- Film reel / play button motif
- Rounded rectangle shape (GNOME HIG)
- Works at 16px to 512px

#### 7d. Flatpak Manifest (Draft)

`dev.arsfeld.Reel.yml` -- draft manifest with GNOME 48 runtime, Rust SDK extension, libmpv module, and appropriate `finish-args` (DRI, PulseAudio, network, filesystem, Wayland/X11).

#### 7e. mpv Log Integration

Bridge mpv log messages into Rust tracing:

```rust
// In MpvBackend initialization
mpv.request_log_messages("warn")?;  // or configurable via REEL_MPV_LOG env var

// In poll loop, check for log messages
if let Some(log_msg) = mpv.wait_event_log_message() {
    match log_msg.level {
        "fatal" | "error" => tracing::error!(target: "mpv", "{}: {}", log_msg.prefix, log_msg.text),
        "warn" => tracing::warn!(target: "mpv", "{}: {}", log_msg.prefix, log_msg.text),
        "info" => tracing::info!(target: "mpv", "{}: {}", log_msg.prefix, log_msg.text),
        _ => tracing::debug!(target: "mpv", "{}: {}", log_msg.prefix, log_msg.text),
    }
}
```

#### 7f. Dead Code Cleanup

Strategy:
1. Remove `#[allow(dead_code)]` annotations one file at a time
2. Run `cargo check` -- compiler will flag truly dead code (since `warnings = "deny"`)
3. For code that IS dead: delete it (it's in git history if needed)
4. For code that's used only in tests: add `#[cfg(test)]`
5. For code that's genuinely needed but not yet wired: keep `#[allow(dead_code)]` with a `// M6:` or `// M7:` comment explaining when it will be used

Priority files (most annotations):
- `src/player/backend.rs` (17 annotations)
- `src/services/plex/models.rs` (many serde fields)
- `src/models/` (fields parsed but not yet displayed)

#### 7g. Window State Serde Cleanup

Already done in Phase 2, but verify:
- Remove the hand-rolled `serialize()` and `deserialize()` functions
- Tests updated to use serde-based equivalents

**Files:**
- `data/dev.arsfeld.Reel.desktop` -- new
- `data/dev.arsfeld.Reel.metainfo.xml` -- new
- `data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg` -- new
- `dev.arsfeld.Reel.yml` -- new (project root)
- `src/player/mpv/mod.rs` -- mpv log integration
- All files with `#[allow(dead_code)]` -- audit and remove

---

## System-Wide Impact

### Interaction Graph

- **MPRIS**: D-Bus method calls -> MprisCommand channel -> glib relay -> AppMsg -> VideoAreaMsg -> mpv. State changes flow in reverse via watch channels.
- **Settings**: Settings loaded at app start -> distributed to components. Changes trigger live mpv property updates (subtitles) or are applied on next use (sort order).
- **Scrobble wiring**: PlaybackTracker -> WatchStateTracker -> WatchStateEvent::Scrobble -> dispatch_watch_events -> sender.oneshot_command -> PlexSource.scrobble(). Fire-and-forget on failure.

### Error Propagation

- MPRIS D-Bus errors: logged, never surface to user (desktop environment handles gracefully)
- Settings save errors: logged via `tracing::warn!()`, settings still in memory until next save attempt
- Scrobble/timeline errors: fire-and-forget with `tracing::warn!()`, never interrupt playback
- Settings load errors: fall back to defaults silently

### State Lifecycle Risks

- **MPRIS server + app shutdown**: Server dropped with app, D-Bus name released automatically. No orphan risk.
- **Settings saved mid-edit**: Each widget change saves immediately (or batched on dialog close). No partial-write risk with `toml::to_string_pretty` + `fs::write` (atomic at TOML level, not filesystem level -- acceptable for preferences).
- **PlexSource replaced mid-playback**: If user removes/re-adds Plex server while playing, `active_source` reference (Arc) keeps the old source alive until scrobble completes. No dangling reference.

### API Surface Parity

- `MediaSource` trait: No new methods needed (scrobble/report_progress already exist from M4)
- `VideoBackend` trait: No changes
- `Settings`: New struct, consumed by App and components, not a trait boundary

### Integration Test Scenarios

1. Start app -> MPRIS server visible on D-Bus -> query PlaybackStatus == "Stopped"
2. Open file -> MPRIS Metadata updates with title and duration -> desktop widget shows info
3. Pause via D-Bus PlayPause -> player pauses -> PlaybackStatus == "Paused"
4. Seek via D-Bus Seek(5000000) -> player seeks 5 seconds -> Seeked signal emitted
5. Change settings -> close dialog -> reopen -> settings persisted
6. Play Plex movie past 90% -> scrobble API called (verify with wiremock)

---

## Acceptance Criteria

### Phase 1: Error Handling & Plex Wiring

- [ ] `dispatch_watch_events` calls `PlexSource.scrobble()` via `sender.oneshot_command()`
- [ ] `dispatch_watch_events` calls `PlexSource.report_progress()` via `sender.oneshot_command()`
- [ ] `App` stores `active_source: Option<Arc<dyn MediaSource>>`
- [ ] `WindowStateError` thiserror enum replaces `Result<(), String>`
- [ ] `reqwest::Client::builder().build()` uses `?` instead of `unwrap()` in `app.rs`
- [ ] `PlexClient::new` returns `Result` instead of `expect()`
- [ ] All `unwrap()`/`expect()` in non-test code audited with comments or replaced
- [ ] Error display follows toast vs. status page criteria table
- [ ] 10+ tests for error types, dispatch_watch_events with/without source

### Phase 2: Window State Serde

- [ ] `window_state.rs` uses `serde + toml` crate, hand-rolled serialize/deserialize removed
- [ ] `WindowState` derives `Serialize, Deserialize` with `#[serde(default)]`
- [ ] Old-format TOML files parse correctly (backward compat)
- [ ] Corrupt/empty TOML falls back to defaults
- [ ] Existing tests pass, 3+ new tests added

### Phase 3: Settings Model

- [ ] `Settings` struct with `PlaybackSettings`, `SubtitleSettings`, `LibrarySettings`
- [ ] Persisted to `config_dir()/settings.toml`
- [ ] `#[serde(default)]` ensures forward/backward compatibility
- [ ] Validation functions for volume, skip interval, speed, font size
- [ ] 25+ unit tests for settings model and validation

### Phase 4: Settings UI

- [ ] `AdwPreferencesDialog` with Playback, Subtitles, Library, Connections pages
- [ ] Settings changes saved to TOML on dialog close
- [ ] Connections page shows saved Plex servers from DB with remove button
- [ ] "Add Server" button opens existing OAuth connection dialog
- [ ] `AdwAboutDialog` shows app name, version, license, author, website
- [ ] Menu button in header/sidebar with Preferences and About actions
- [ ] libadwaita bumped to `v1_5`

### Phase 5: MPRIS2

- [ ] `mpris-server` dependency added
- [ ] `org.mpris.MediaPlayer2.Reel` appears on session D-Bus
- [ ] `PlaybackStatus` reflects current PlayState
- [ ] `Metadata` dict populated from now_playing or filename
- [ ] `Position` returns current position in microseconds
- [ ] `Volume` readable and writable (setting volume via D-Bus changes player volume)
- [ ] `Play`, `Pause`, `PlayPause`, `Stop`, `Seek`, `SetPosition` methods functional
- [ ] `Seeked` signal emitted on user-initiated seeks only
- [ ] `Identity` = "Reel", `DesktopEntry` = "dev.arsfeld.Reel"
- [ ] Property changes emitted when state changes (PlaybackStatus, Metadata, Volume)
- [ ] GNOME/KDE media widget shows Reel with correct metadata during playback
- [ ] 20+ unit tests for pure derivation functions
- [ ] Integration test with `dbus-run-session` (gated behind `integration` feature)

### Phase 6: UI Polish

- [ ] Skip forward/back buttons in controls bar
- [ ] Skip intervals read from settings (not hardcoded)
- [ ] Keyboard shortcuts in `shortcuts.rs` use configurable skip intervals
- [ ] Subtitle font/size/language applied from settings on file load
- [ ] Subtitle settings apply live during playback
- [ ] Welcome status page shown on first run (no saved sources)
- [ ] Welcome page has "Connect to Plex" and "Open File" buttons
- [ ] After adding a source, navigates to library view

### Phase 7: Desktop Integration & Cleanup

- [ ] `.desktop` file with correct MIME types and app ID
- [ ] AppStream metainfo XML with required fields
- [ ] SVG app icon at `data/icons/hicolor/scalable/apps/dev.arsfeld.Reel.svg`
- [ ] Flatpak manifest draft
- [ ] mpv log messages bridged to Rust tracing
- [ ] `#[allow(dead_code)]` annotations reduced (target: <20 remaining, all with justification comments)
- [ ] Zero clippy warnings, formatted with cargo fmt

---

## Success Metrics

- GNOME/KDE media widgets can play/pause/seek Reel and display now-playing metadata
- Users can configure playback, subtitle, and library preferences via a native-looking settings dialog
- All Plex scrobble and timeline calls actually fire (verifiable in Plex dashboard within 10 seconds)
- Zero `unwrap()` panics in production code paths
- First launch guides users to connect or open a file
- `appstreamcli validate` passes on the metainfo XML
- App appears in GNOME Software / app launchers via `.desktop` file
- Test count: 500+ (from current 463)

## Dependencies & Prerequisites

**New crate dependencies:**
- `mpris-server = "0.9"` -- MPRIS D-Bus server
- `toml = "0.8"` -- TOML serialization (serde-based)

**Existing dependency changes:**
- `libadwaita`: bump features from `v1_4` to `v1_5` (for `PreferencesDialog`, `AboutDialog`)

**No new dev-dependencies required.**

**External tools (for validation, not build):**
- `appstreamcli` -- validate metainfo XML
- `desktop-file-validate` -- validate .desktop file
- `dbus-run-session` -- run MPRIS integration tests in CI

## Risk Analysis & Mitigation

| Risk | Severity | Probability | Mitigation |
|------|----------|-------------|------------|
| MPRIS threading model deadlocks | High | Medium | Prototype the channel-based communication first as a spike. Use `tokio::sync::watch` (lock-free) for state, `mpsc` for commands. |
| `mpris-server` v0.9 incompatible with dep tree | Medium | Low | Check `cargo add mpris-server` early. If conflict, fall back to raw `zbus`. |
| libadwaita v1_5 not available on target distros | Medium | Low | v1_5 = GNOME 46 (Mar 2024), widely available. Fall back to v1_4 (`PreferencesWindow` + `AboutWindow`) if needed. |
| Settings/WindowState migration loses user data | Medium | Low | `#[serde(default)]` ensures old files parse. The TOML format is compatible. |
| PlexSource lifetime in async scrobble calls | Low | Low | `Arc<dyn MediaSource>` ensures source stays alive until async call completes. |
| Dead code removal breaks build | Low | Medium | Remove annotations one file at a time, compile after each. `warnings = "deny"` catches issues immediately. |

## Future Considerations

**Not in M5 scope:**
- Auto-advance to next episode (M6/M7)
- Trakt/Jellyfin/Emby integration (M7)
- TMDb metadata fetching for local files (M6)
- Playback history / viewing statistics
- MPRIS TrackList interface (playlist support)
- GSettings migration (if needed for Flatpak store compliance)
- File logging with rotation (only env-var file logging in M5)

## Sources & References

### Internal References
- `src/app.rs:864-875` -- Plex scrobble/timeline TODOs
- `src/services/window_state.rs` -- hand-rolled TOML, string errors
- `src/config.rs` -- XDG path helpers (config_dir unused, marked dead_code)
- `src/player/backend.rs` -- PlayState enum, speed presets (17 dead_code annotations)
- `src/player/playback_tracker.rs` -- state machine feeding MPRIS properties
- `src/components/player/controls.rs` -- controls bar (skip button location)
- `src/components/player/shortcuts.rs:39-42` -- hardcoded skip intervals
- `src/services/plex/source.rs` -- PlexSource with scrobble/report_progress implemented
- `src/main.rs:25` -- app ID `dev.arsfeld.Reel`
- `Cargo.toml:13` -- libadwaita v1_4 (needs v1_5 bump)
- `Cargo.toml:74` -- `warnings = "deny"` constraining dead code removal

### External References
- [MPRIS D-Bus Specification v2.2](https://specifications.freedesktop.org/mpris/latest/)
- [mpris-server crate](https://docs.rs/mpris-server) -- Rust MPRIS server library
- [mpv-mpris2](https://github.com/eNV25/mpv-mpris2) -- mpv MPRIS plugin (reference implementation)
- [Amberol](https://gitlab.gnome.org/World/amberol) -- Rust/GTK4/libadwaita music player with MPRIS
- [AppStream Metadata quickstart](https://freedesktop.org/software/appstream/docs/chap-Quickstart.html)
- [Flathub MetaInfo guidelines](https://docs.flathub.org/docs/for-app-authors/metainfo-guidelines)
- [libadwaita PreferencesDialog](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/1-latest/class.PreferencesDialog.html)
- [Celluloid](https://github.com/celluloid-player/celluloid) -- GTK4 mpv frontend (.desktop and metainfo reference)

### Existing Plans
- [M4 Watch State plan](docs/plans/2026-03-14-feat-m4-watch-state-plan.md) -- predecessor milestone, all items completed
