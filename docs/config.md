# Configuration System Review and Documentation

## Overview
This document provides a comprehensive review of the configuration management system in the Reel application, detailing how configuration is loaded, saved, and used throughout the codebase.

## Configuration Architecture

### Core Configuration Module
- **Location**: `src/config.rs`
- **Structure**: Hierarchical configuration using Serde for serialization/deserialization
- **Format**: TOML file format with conditional field serialization to minimize file size

### Configuration Hierarchy

```
Config (root)
├── GeneralConfig
│   ├── theme: String (default: "auto")
│   ├── language: String (default: "system")
│   └── preferred_source_id: Option<String>
├── PlaybackConfig
│   ├── player_backend: String (default: "mpv")
│   ├── hardware_acceleration: bool (default: true)
│   ├── default_subtitle: String (default: "none")
│   ├── default_audio: String (default: "original")
│   ├── skip_intro: bool (default: true)
│   ├── skip_credits: bool (default: true)
│   ├── auto_play_next: bool (default: true)
│   ├── auto_play_delay: u64 (default: 10)
│   ├── mpv_verbose_logging: bool (default: false)
│   ├── mpv_cache_size_mb: u32 (default: 1500)
│   ├── mpv_cache_backbuffer_mb: u32 (default: 500)
│   ├── mpv_cache_secs: u32 (default: 1800)
│   ├── auto_resume: bool (default: true)
│   ├── resume_threshold_seconds: u32 (default: 30)
│   └── progress_update_interval_seconds: u32 (default: 10)
├── NetworkConfig
│   ├── connection_timeout: u64 (default: 30)
│   ├── max_retries: u32 (default: 3)
│   └── cache_size: u64 (default: 1000)
├── BackendsConfig
│   ├── PlexConfig
│   │   ├── server_url: String (default: "https://plex.tv")
│   │   └── auth_token: Option<String>
│   └── JellyfinConfig
│       └── server_url: String
└── RuntimeConfig
    ├── legacy_backends: Vec<String>
    ├── last_sync_times: HashMap<String, String>
    ├── library_visibility: HashMap<String, bool>
    ├── auth_providers: HashMap<String, AuthProvider>
    ├── cached_sources: HashMap<String, Vec<Source>>
    └── sources_last_fetched: HashMap<String, DateTime<Utc>>
```

## Configuration Sources

### 1. File System
- **Primary Source**: TOML configuration file
- **Location**:
  - macOS: `~/Library/Application Support/Reel/config.toml`
  - Linux/Other: `~/.config/reel/config.toml`
- **Loading**: Via `Config::load()` which reads and deserializes the TOML file
- **Creation**: File is created on first save if it doesn't exist

### 2. Default Values
- **Implementation**: Hardcoded defaults in `src/config.rs`
- **Application**: Applied through Serde's `#[serde(default)]` attributes
- **Optimization**: Default values are not serialized to keep config file minimal

### 3. Environment Variables
- **Limited Usage**: Only specific player configurations use environment variables
- **Examples**:
  - `REEL_FORCE_FALLBACK_SINK` - Forces fallback audio sink in GStreamer
  - `REEL_USE_GL_SINK` - Enables GL sink in GStreamer
  - `GST_DEBUG_DUMP_DOT_DIR` - GStreamer debug output directory

## Configuration Loading Flow

### Application Startup
1. **Main Entry** (`src/main.rs`): Initializes platform but doesn't load config
2. **ReelApp** (`src/platforms/relm4/app.rs`): Creates database, doesn't load config
3. **MainWindow** (`src/platforms/relm4/components/main_window.rs`): No config loading
4. **Component-Level Loading**: Each component loads config as needed:
   - Player page loads config once during initialization
   - Preferences page loads config when opened
   - Player controller receives config as parameter

### Lazy Loading Pattern
- Configuration is **NOT** loaded globally at startup
- Components load configuration on-demand when needed
- This prevents unnecessary I/O if certain features aren't used

## Configuration Save/Modify Operations

### Direct Modifications
1. **Preferences Page** (`preferences.rs:305`)
   - Saves player backend selection
   - Saves hardware acceleration setting
   - Note: Some preferences shown in UI aren't saved to config yet

### Programmatic Updates
2. **Auth Provider Management** (`config.rs`)
   - `add_auth_provider()`: Adds new auth provider and saves
   - `remove_auth_provider()`: Removes provider and cleans up related data
   - `set_auth_providers()`: Batch update of all providers

3. **Runtime State** (`config.rs`)
   - `set_library_visibility()`: Updates library visibility settings
   - `set_cached_sources()`: Caches source lists with timestamps
   - `set_preferred_source_id()`: Sets preferred media source
   - `remove_legacy_backend()`: Removes legacy backend entries

4. **Backend Specific** (`config.rs`)
   - `set_plex_token()`: Updates Plex authentication token

### Save Mechanism
- All modification methods call `self.save()` internally
- Save serializes entire config to TOML
- Parent directory is created if it doesn't exist
- Only non-default values are written to file

## Configuration Usage Patterns

### 1. Player Components
**Player Page** (`player.rs`)
- Loads config once during initialization
- Caches frequently used values to avoid repeated I/O:
  - `config_auto_resume`
  - `config_resume_threshold_seconds`
  - `config_progress_update_interval_seconds`

**Player Factory** (`factory.rs`)
- Receives config as parameter
- Uses config to determine player backend (MPV vs GStreamer)
- Passes config to player implementation

**MPV Player** (`mpv_player.rs`)
- Uses config for cache settings and logging verbosity
- Configures MPV instance based on config values

### 2. Backend Modules
**Test Fixtures Only**
- Backends create `Config::default()` in tests
- Production backends don't directly access configuration
- Configuration is managed at higher levels

### 3. Preferences Management
**Preferences Page** (`preferences.rs`)
- Loads config when component initializes
- Reloads config before saving to preserve other settings
- Limited subset of config is actually exposed in UI

## Issues and Inconsistencies Identified

### 1. Fixed Issues
- **Config Reload Loop** (task-057): Previously reloaded config every second in player update loop. Fixed by caching values.

### 2. Current Issues

#### Incomplete Configuration Coverage
- Many configuration fields exist but aren't exposed in UI
- Some UI preferences aren't persisted to config (e.g., items_per_page)

#### No Global Configuration Instance
- Each component loads config independently
- Potential for inconsistency if config changes during runtime
- Multiple file reads for the same data

#### Missing Configuration Change Notification
- No mechanism to notify components when config changes
- Components must manually reload to get updates

#### Limited Validation
- No validation of configuration values beyond type checking
- Invalid values could cause runtime errors

#### Inconsistent Error Handling
- Some components use `.unwrap_or_default()` silently
- Others log warnings but continue with defaults
- No user notification of config load failures

### 3. Architecture Observations

#### Separation of Concerns
- Configuration is well-separated from business logic
- Clear distinction between config structure and usage

#### Performance Optimizations
- Conditional serialization reduces file size
- Default values aren't written to disk
- Recent fix prevents repeated file I/O

#### Type Safety
- Strong typing through Rust's type system
- Serde ensures type consistency

## Recommendations for Improvement

### 1. Implement Global Configuration Service
- Load config once at application startup
- Provide shared read access across components
- Implement write synchronization for updates

### 2. Add Configuration Change Events
- Integrate with event bus for config change notifications
- Allow components to subscribe to specific config sections
- Enable live configuration updates without restart

### 3. Enhance UI Coverage
- Expose all relevant config options in preferences
- Group related settings logically
- Add advanced settings panel for power users

### 4. Improve Validation
- Add value range validation for numeric fields
- Validate URLs and paths before saving
- Provide user-friendly error messages

### 5. Consider Configuration Profiles
- Allow multiple configuration profiles
- Quick switching between profiles
- Export/import configuration settings

### 6. Add Configuration Migration
- Version configuration schema
- Implement migration logic for schema changes
- Preserve user settings during updates

### 7. Implement Hot Reload (Optional)
- Watch config file for external changes
- Reload and apply changes without restart
- Useful for development and debugging

## Summary

The configuration system in Reel is functional but has room for improvement. The recent fix for the reload loop shows good progress in optimization. The main areas for enhancement are:

1. **Centralization**: Moving from component-level loading to a global service
2. **Coverage**: Exposing more settings in the UI
3. **Reactivity**: Adding change notifications and hot reload
4. **Robustness**: Improving validation and error handling

The current architecture provides a solid foundation with good type safety and performance optimizations through conditional serialization. The recommended improvements would enhance user experience and developer ergonomics while maintaining the existing strengths.