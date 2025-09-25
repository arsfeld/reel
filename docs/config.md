# Configuration System Documentation

## Overview
The configuration management system in the Reel application provides simplified settings management using TOML format. The system has been refactored to be minimal and focused only on playback settings.

## Configuration Architecture

### Core Configuration Module
- **Location**: `src/config.rs`
- **Structure**: Simple two-level configuration structure
- **Format**: TOML file format
- **Pattern**: On-demand loading by components

### Configuration Structure

```
Config (root)
└── PlaybackConfig
    ├── player_backend: String (default: "mpv")
    ├── hardware_acceleration: bool (default: true)
    ├── mpv_verbose_logging: bool (default: false)
    ├── mpv_cache_size_mb: u32 (default: 150)
    ├── mpv_cache_backbuffer_mb: u32 (default: 50)
    ├── mpv_cache_secs: u32 (default: 30)
    ├── auto_resume: bool (default: true)
    ├── resume_threshold_seconds: u32 (default: 10)
    ├── progress_update_interval_seconds: u32 (default: 5)
    └── mpv_upscaling_mode: String (default: "bilinear")
```

## Configuration File Location

- **macOS**: `~/Library/Application Support/Reel/config.toml`
- **Linux/Other**: `~/.config/reel/config.toml`

The configuration directory and file are created automatically on first save if they don't exist.

## Configuration Loading

### Loading Pattern
Configuration is loaded on-demand using `Config::load()`:
- If config file exists, it is parsed from TOML
- If no config file exists, default values are used
- Errors in parsing are propagated to the caller

### Usage Examples

```rust
// Load configuration
let config = Config::load()?;

// Access playback settings
let player_backend = &config.playback.player_backend;
let cache_size = config.playback.mpv_cache_size_mb;
```

## Configuration Saving

The configuration can be saved using `Config::save()`:
- Creates parent directory if it doesn't exist
- Serializes to TOML format using pretty printing
- Overwrites existing file completely

```rust
// Modify and save configuration
let mut config = Config::load()?;
config.playback.player_backend = "gstreamer".to_string();
config.save()?;
```

## Default Values

| Field | Default Value | Description |
|-------|--------------|-------------|
| **Playback** | | |
| player_backend | "mpv" | Video player backend (mpv or gstreamer) |
| hardware_acceleration | true | Enable hardware acceleration |
| mpv_verbose_logging | false | Enable verbose MPV logging |
| mpv_cache_size_mb | 150 | MPV cache size in MB |
| mpv_cache_backbuffer_mb | 50 | MPV backward cache size in MB |
| mpv_cache_secs | 30 | MPV cache duration in seconds |
| auto_resume | true | Resume playback from last position |
| resume_threshold_seconds | 10 | Minimum seconds watched before resuming |
| progress_update_interval_seconds | 5 | How often to save playback progress |
| mpv_upscaling_mode | "bilinear" | MPV upscaling algorithm |

## Configuration Usage in Components

### Player Components
The player system is the primary consumer of configuration:

1. **Player Page** (`src/ui/pages/player.rs`)
   - Loads config during initialization
   - Caches frequently-used values to avoid repeated I/O
   - Passes config to player controller

2. **Player Controller** (`src/player/controller.rs`)
   - Receives config from player page
   - Passes to player factory for backend selection

3. **Player Factory** (`src/player/factory.rs`)
   - Uses `player_backend` field to select MPV or GStreamer
   - Passes config to the selected backend

4. **MPV Player** (`src/player/mpv_player.rs`)
   - Uses cache settings: `mpv_cache_size_mb`, `mpv_cache_backbuffer_mb`, `mpv_cache_secs`
   - Uses `mpv_verbose_logging` for debug output
   - Uses `mpv_upscaling_mode` for video scaling

5. **GStreamer Player** (`src/player/gstreamer_player.rs`)
   - Currently doesn't use configuration values directly
   - Relies on environment variables for some settings

### UI Components

1. **Preferences Page** (`src/ui/pages/preferences.rs`)
   - Loads current config when saving changes
   - Updates `player_backend` and `hardware_acceleration` fields
   - Saves configuration after modifications

## Environment Variables

While the main configuration uses the file-based system, some player-specific settings still use environment variables:

**GStreamer-specific**:
- `REEL_FORCE_FALLBACK_SINK` - Forces fallback audio sink
- `REEL_USE_GL_SINK` - Enables GL sink for video
- `GST_DEBUG_DUMP_DOT_DIR` - GStreamer debug output directory

## Simplified Architecture Benefits

The current simplified configuration system:
- **Minimal Complexity**: Only playback settings, no complex hierarchies
- **Type Safety**: All fields are strongly typed with serde
- **Clear Defaults**: All defaults defined in one place
- **Easy Testing**: Simple structure makes testing straightforward

## Known Limitations

1. **No Configuration UI Coverage**
   - Most config fields aren't exposed in the preferences UI
   - Only `player_backend` and `hardware_acceleration` can be changed via UI

2. **No Change Notification**
   - Components must manually reload config
   - No event system for configuration changes

3. **Limited Validation**
   - No range validation for numeric values
   - Invalid values could cause runtime issues

4. **No Migration Support**
   - Old config formats aren't automatically migrated
   - Users may need to delete old config files

## Future Considerations

While keeping the system simple, potential improvements could include:
- Exposing more settings in the preferences UI
- Adding basic validation for numeric ranges
- Implementing config migration for version updates
- Adding a configuration reload mechanism

## Summary

The configuration system is intentionally minimal, focusing only on essential playback settings. This simplification reduces complexity and maintenance burden while providing the necessary functionality for player configuration. The system uses standard Rust patterns with serde for serialization and provides clear defaults for all settings.
