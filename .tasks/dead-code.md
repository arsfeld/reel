# Dead Code Analysis

This document lists all code that was marked with `#[allow(dead_code)]` to fix compilation warnings. Each item needs to be analyzed to determine if it should be:
- Removed (truly unused)
- Kept as future feature (incomplete implementation)
- Connected to existing code (missing integration)

## Backend Implementations

### Jellyfin Backend (`src/backends/jellyfin/`)

**mod.rs:**
- `client: Client` field - HTTP client for API calls
- `source: Option<Source>` field - Source configuration
- `set_base_url()` method - Configure server URL
- `get_api_client()` method - Get API client instance  
- `load_credentials()` method - Load saved credentials

**api.rs:**
- `JellyfinItem` struct - Made pub(crate) for visibility
- `ServerInfo.version` field - Server version info
- `ServerInfo.operating_system` field - OS info
- `ServerInfo.id` field - Server ID
- `AuthResponse.server_id` field - Server ID from auth
- `ItemsResponse.total_record_count` field - Total items count
- `MediaSegment.id` field - Segment ID
- `MediaSegment.item_id` field - Media item ID
- `MediaSource.supports_transcoding` field - Transcoding support flag

### Plex Backend (`src/backends/plex/`)

**mod.rs:**
- `client: Client` field - HTTP client (already had allow)
- `ServerInfo.uri` field - Server URI
- Entire `PlexBackend` impl block - All methods unused
- `new()` method - Constructor
- `with_id()` method - Constructor with ID
- `get_server_info()` method - Get server info
- `get_api_client()` method - Get API client
- `save_token()` method - Save auth token

**api.rs:**
- Entire `PlexApi` impl block - All methods unused
- `new()` method - Constructor
- `get_on_deck()` method - Get on deck items
- `get_recently_added()` method - Get recent items
- `get_library_hubs()` method - Get library hubs

### Local Backend (`src/backends/local/`)

**mod.rs:**
- Entire `LocalBackend` impl block - Incomplete local files support
- `new()` method - Constructor
- `with_id()` method - Constructor with ID
- `add_directory()` method - Add media directory

### Sync Strategy (`src/backends/sync_strategy.rs`)

- Entire file marked - `SyncStrategy` struct and impl
- Future sync optimization features

## Core Architecture

### State Management (`src/core/state.rs`)
- `backend_manager: Arc<RwLock<BackendManager>>` field - Backend coordination

### Frontend Trait (`src/core/frontend.rs`)
- `Frontend` trait - Platform abstraction for future macOS support

### ViewModels (`src/core/viewmodels/`)

**mod.rs:**
- `dispose()` method in `ViewModel` trait - Cleanup lifecycle method

**player_view_model.rs:**
- `PlaybackInfo` struct - Playback metadata
- `AutoPlayState::Counting(u32)` variant - Auto-play countdown
- `AutoPlayState::Disabled` variant - Auto-play disabled state
- Entire `PlayerViewModel` impl block - All player control methods

**sidebar_view_model.rs:**
- `on_library_created()` method - Event handler
- `on_library_updated()` method - Event handler  
- `on_source_added()` method - Event handler
- `on_source_status_changed()` method - Event handler
- `on_sync_started()` method - Event handler
- `on_sync_completed()` method - Event handler

## Database Layer

### Repository Traits (`src/db/repository/`)

**mod.rs:**
- `count()` method in base `Repository` trait

**library_repository.rs:**
- Entire `LibraryRepository` trait - Database operations

**media_repository.rs:**
- Entire `MediaRepository` trait - All methods:
  - `find_by_type()` - Filter by media type
  - `search()` - Text search
  - `find_recently_added()` - Recent items
  - `find_by_genre()` - Filter by genre
  - `bulk_insert()` - Batch insert
  - `update_metadata()` - Update metadata

**playback_repository.rs:**
- Entire `PlaybackRepository` trait - All methods:
  - `find_watched()` - Get watched items
  - `find_in_progress()` - Get in-progress items
  - `mark_watched()` - Mark as watched
  - `mark_unwatched()` - Mark as unwatched
  - `find_recently_watched()` - Recent watched
  - `cleanup_old_entries()` - Cleanup old data

**source_repository.rs:**
- Entire `SourceRepository` trait - All methods:
  - `find_by_type()` - Filter by source type
  - `find_online()` - Get online sources
  - `update_online_status()` - Update status
  - `update_last_sync()` - Update sync time
  - `find_archived()` - Get archived sources

**sync_repository.rs:**
- Entire file marked - `SyncRepository` trait, `SyncStats` struct, `SyncRepositoryImpl`

### Migrations (`src/db/migrations/`)
- `MediaItems::Id` enum variant in episode fields migration

## Player System

### Player Implementations (`src/player/`)

**mpv_player.rs:**
- `MPV_RENDER_UPDATE_FRAME` constant - Render update flag
- `PlayerState::Error(String)` variant - Error state
- `UpscalingMode.next()` method - Cycle upscaling modes
- `frame_pending: Arc<AtomicBool>` field in UpdateContext
- Player control methods:
  - `get_video_widget()` - Get video widget
  - `get_buffer_percentage()` - Get buffer status
  - `get_upscaling_mode()` - Get current upscaling
  - `cycle_upscaling_mode()` - Change upscaling
  - `clear_video_widget_state()` - Clear video state

**gstreamer_player.rs:**
- `create_subtitle_filter()` method - Subtitle filtering
- `get_video_widget()` method - Get video widget  
- `get_buffer_percentage()` method - Get buffer status

**factory.rs:**
- `PlayerState::Error(String)` variant - Error state
- Player utility methods:
  - `get_video_widget()` - Get video widget
  - `get_buffer_percentage()` - Get buffer status
  - `get_backend_name()` - Get backend name
  - `get_backend_type()` - Get backend type
  - `log_player_info()` - Debug logging
  - `clear_video_widget_state()` - Clear state

**traits.rs:**
- File was cleaned up - old unused traits removed

## Event System (`src/events/`)

**mod.rs:**
- `EventHandler` trait - Event handling interface

**event_bus.rs:**
- `EventBusStats.subscriber_count` field - Subscriber count tracking

## Services (`src/services/`)

**initialization.rs:**
- `InitializationEvent` enum - Initialization events
- `InitializationStage` enum - Initialization stages  
- `ConnectionDetails` struct and impl - Connection details

**source_coordinator.rs:**
- `initialize_source()` method - Source initialization

## Platform Implementation

### GTK Platform (`src/platforms/gtk/`)

**app.rs:**
- `APP_ID` constant - Application ID
- Entire `ReelApp` struct and impl - GTK app wrapper

**platform_utils.rs:**
- `configure_video_output()` function - Video configuration
- `configure_linux_video()` function - Linux video setup
- `check_hw_acceleration()` function - Hardware acceleration check

### UI Components (`src/platforms/gtk/ui/`)

**main_window.rs:**
- Window management methods:
  - `move_backend_up()` - Reorder backends
  - `move_backend_down()` - Reorder backends  
  - `toggle_edit_mode()` - Edit mode toggle
  - `load_library_visibility()` - Load visibility settings
  - `save_library_visibility()` - Save visibility settings

**pages/library.rs:**
- Entire `LibraryView` impl block - All UI utility methods

**pages/player.rs:**
- UI component fields:
  - `hover_controller` - Mouse hover detection
  - `loading_overlay` - Loading state UI
  - `loading_spinner` - Loading spinner
  - `loading_label` - Loading text
  - `error_overlay` - Error state UI
  - `error_label` - Error message
  - `time_label` - Time display
  - `end_time_label` - End time display

**pages/show_details.rs:**
- `create_episode_card_widget()` method - Episode card creation
- `convert_media_item_to_episode()` method - Data conversion

**pages/movie_details.rs:**
- `IMAGE_LOADER` static - Global image loader

**reactive_bindings.rs:**
- `bind_visibility()` function - Widget visibility binding

## Constants (`src/constants.rs`)

- `SCROLL_DEBOUNCE_MS` - Scroll debouncing delay
- `IMAGE_VIEWPORT_BUFFER` - Image loading buffer  
- `CARD_BATCH_SIZE` - Batch loading size

## Main Entry Point (`src/main.rs`)

- `gtk4::prelude::*` import - GTK prelude (platform-specific)

## Analysis Categories

### 🔥 High Priority - Likely Needed Soon
- Player control methods (volume, seek, fullscreen, etc.)
- Repository traits (database operations will be needed)
- Event handlers in ViewModels
- UI reactive bindings

### 🚧 Medium Priority - Incomplete Features  
- Backend API methods (Plex, Jellyfin local)
- Sync strategy and repository
- Initialization service
- Platform utilities

### 🤔 Low Priority - Analyze Later
- UI component fields (may be used in templates)
- Constants (may be used conditionally)
- Error variants and debug methods

### ❌ Candidates for Removal
- Duplicate functionality
- Abandoned approaches  
- Truly unused helper methods

## Next Steps

1. **Connect Player Methods**: Wire up player control methods to UI
2. **Repository Implementation**: Connect repository traits to actual database operations  
3. **Event Integration**: Connect ViewModel event handlers to EventBus
4. **Backend Completion**: Finish Plex/Jellyfin API implementations
5. **UI Polish**: Connect unused UI components or remove them
6. **Code Review**: Review each item for removal vs keeping