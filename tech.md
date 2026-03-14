# Reel - Technical Architecture

## Stack

| Layer | Technology | Version | Purpose |
|-------|-----------|---------|---------|
| Language | Rust | 1.93+ | Core language |
| UI Framework | Relm4 | 0.10 | Elm-architecture component framework |
| Toolkit | GTK4 | 4.14+ | Widget toolkit |
| Design System | libadwaita | 1.4+ | GNOME HIG, adaptive layouts |
| Media Engine | libmpv (via `libmpv2`) | 5.0.3 | Playback, codecs, HW accel, subtitles |
| Video Rendering | GtkGLArea + mpv render API | - | OpenGL-based video surface |
| Database | SQLite (rusqlite) | 0.38 | Local metadata cache, watch state |
| HTTP | reqwest | 0.12 | Plex API, TMDb API, OpenSubtitles |
| Serialization | serde + serde_json | 1.x | API responses, config files |
| Async Runtime | tokio | 1.x | Async I/O (via Relm4 commands) |
| Logging | tracing + tracing-subscriber | 0.1 / 0.3 | Structured logging |
| Config | toml | - | Configuration file format |
| Change Tracking | tracker | 0.2 | Efficient UI updates |
| EGL Bindings | khronos-egl | - | GL function resolution for mpv |

### Future Dependencies (Architecture Ready)
| Crate | Purpose |
|-------|---------|
| gstreamer / gst-plugin-gtk4 | Alternative playback backend (GStreamer) |
| pavao | SMB/CIFS share access |
| mdns-sd | Network service discovery |
| notify | Filesystem change watching |

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        Reel Application                      │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                    UI Layer (Relm4)                    │   │
│  │                                                        │   │
│  │  ┌─────────┐ ┌──────────┐ ┌────────┐ ┌────────────┐ │   │
│  │  │ Sidebar │ │ Library  │ │ Detail │ │   Player   │ │   │
│  │  │  Nav    │ │  Grid    │ │  Page  │ │   View     │ │   │
│  │  └─────────┘ └──────────┘ └────────┘ └────────────┘ │   │
│  └──────────────────────┬───────────────────────────────┘   │
│                         │ Messages                           │
│  ┌──────────────────────┴───────────────────────────────┐   │
│  │                  Service Layer                         │   │
│  │                                                        │   │
│  │  ┌───────────┐ ┌──────────┐ ┌──────────┐ ┌────────┐ │   │
│  │  │ Playback  │ │ Library  │ │ Metadata │ │ Config │ │   │
│  │  │ Service   │ │ Service  │ │ Service  │ │Service │ │   │
│  │  └─────┬─────┘ └────┬─────┘ └────┬─────┘ └────────┘ │   │
│  └────────┼─────────────┼────────────┼──────────────────┘   │
│           │             │            │                        │
│  ┌────────┼─────────────┼────────────┼──────────────────┐   │
│  │        │       Backend Layer      │                    │   │
│  │  ┌─────┴─────┐ ┌────┴────┐ ┌─────┴─────┐            │   │
│  │  │ Video     │ │ SQLite  │ │ HTTP APIs │            │   │
│  │  │ Backend   │ │   DB    │ │ (Plex,    │            │   │
│  │  │ (trait)   │ │         │ │  TMDb,    │            │   │
│  │  │  ├─ mpv   │ │         │ │  Trakt)   │            │   │
│  │  │  └─ gst   │ │         │ │           │            │   │
│  │  └───────────┘ └─────────┘ └───────────┘            │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

## Project Structure

```
reel/
├── Cargo.toml
├── Cargo.lock
├── build.aux/                    # Meson build support (for Flatpak)
├── data/
│   ├── com.reel.Reel.desktop     # Desktop entry
│   ├── com.reel.Reel.metainfo.xml
│   ├── icons/                    # App icons
│   └── resources/
│       └── resources.gresource.xml
├── src/
│   ├── main.rs                   # Entry point: adw::Application, CSS, backend init
│   ├── app.rs                    # Root App component (window, navigation shell)
│   ├── config.rs                 # Constants, XDG paths, version
│   │
│   ├── components/               # Relm4 UI components
│   │   ├── mod.rs
│   │   ├── sidebar.rs            # Navigation sidebar component
│   │   ├── library/
│   │   │   ├── mod.rs            # Library grid/list view component
│   │   │   ├── media_card.rs     # FactoryComponent for poster cards
│   │   │   └── filters.rs       # Filter/sort controls component
│   │   ├── detail/
│   │   │   ├── mod.rs
│   │   │   ├── movie_detail.rs   # Movie detail page component
│   │   │   ├── show_detail.rs    # TV show detail page component
│   │   │   ├── season_view.rs    # Season episode list component
│   │   │   └── cast_row.rs      # FactoryComponent for cast members
│   │   ├── player/
│   │   │   ├── mod.rs            # Player view component
│   │   │   ├── controls.rs      # Playback controls overlay
│   │   │   ├── video_area.rs    # Video surface widget (GtkGLArea + GraphicsOffload)
│   │   │   └── track_selector.rs # Audio/subtitle track popover
│   │   ├── search.rs            # Search component
│   │   ├── settings/
│   │   │   ├── mod.rs           # Settings page (AdwPreferencesWindow)
│   │   │   ├── playback.rs
│   │   │   ├── subtitles.rs
│   │   │   └── connections.rs
│   │   └── onboarding.rs        # First-run source connection wizard
│   │
│   ├── models/                   # Shared data types
│   │   ├── mod.rs
│   │   ├── media.rs             # Movie, TvShow, Season, Episode, MediaItem enum
│   │   ├── library.rs           # LibrarySection, Collection, Genre
│   │   ├── playback.rs          # PlaybackState, TrackInfo, SubtitleTrack
│   │   ├── source.rs            # MediaSource enum (Plex, Local, etc.)
│   │   └── settings.rs         # UserSettings, PlaybackSettings, SubtitleSettings
│   │
│   ├── services/                 # Business logic (no UI)
│   │   ├── mod.rs
│   │   ├── media_source.rs      # MediaSource trait + registry
│   │   ├── plex/
│   │   │   ├── mod.rs           # PlexSource: impl MediaSource
│   │   │   ├── api.rs           # Plex API client (HTTP)
│   │   │   ├── auth.rs          # Plex authentication
│   │   │   └── models.rs        # Plex-specific API response types
│   │   ├── tmdb/
│   │   │   ├── mod.rs           # TMDb API client
│   │   │   └── models.rs       # TMDb response types
│   │   ├── trakt/
│   │   │   ├── mod.rs           # Trakt API client
│   │   │   └── models.rs
│   │   ├── opensubtitles/
│   │   │   ├── mod.rs           # OpenSubtitles API client
│   │   │   └── models.rs
│   │   ├── metadata.rs          # Metadata resolution (orchestrates TMDb, filename parsing)
│   │   ├── filename_parser.rs   # Extract title/year/season/episode from filenames
│   │   ├── artwork.rs           # Image downloading and caching
│   │   └── subtitle.rs         # Subtitle search and management
│   │
│   ├── player/                   # Abstract video backend + implementations
│   │   ├── mod.rs               # Re-exports, backend selection
│   │   ├── backend.rs           # VideoBackend trait definition
│   │   ├── mpv/
│   │   │   ├── mod.rs           # MpvBackend: impl VideoBackend
│   │   │   ├── context.rs       # mpv handle + render context lifecycle
│   │   │   ├── events.rs        # mpv event loop → BackendEvent translation
│   │   │   ├── properties.rs    # Property observation helpers
│   │   │   └── gl_bridge.rs     # GtkGLArea ↔ mpv render API integration
│   │   ├── gstreamer/           # (future) GStreamer backend
│   │   │   └── mod.rs
│   │   ├── state.rs             # Playback state machine (shared across backends)
│   │   └── mpris.rs             # MPRIS2 D-Bus interface
│   │
│   ├── db/                       # Database layer
│   │   ├── mod.rs
│   │   ├── schema.rs            # Table definitions, migrations
│   │   ├── media_repo.rs        # CRUD for media items
│   │   ├── watch_state_repo.rs  # Watch progress, watched status
│   │   └── settings_repo.rs    # Persisted settings
│   │
│   └── style.css                # Application-specific CSS overrides
│
├── tests/
│   ├── filename_parser_test.rs
│   ├── plex_api_test.rs
│   └── db_test.rs
│
└── flatpak/
    └── com.reel.Reel.yml         # Flatpak manifest
```

---

## Component Architecture (Relm4)

### Component Hierarchy

```
App (Component)
├── Sidebar (SimpleComponent)
│   └── SourceList (FactoryVecDeque<SourceItem>)
├── LibraryView (Component)
│   ├── FilterBar (SimpleComponent)
│   └── MediaGrid (TypedGridView<MediaCard>)
├── MovieDetailView (AsyncComponent)
│   ├── CastRow (FactoryVecDeque<CastMember>)
│   └── CollectionStrip (FactoryVecDeque<CollectionItem>)
├── ShowDetailView (AsyncComponent)
│   └── EpisodeList (FactoryVecDeque<EpisodeRow>)
├── PlayerView (Component)
│   ├── VideoArea (SimpleComponent)
│   └── PlayerControls (SimpleComponent)
│       └── TrackSelector (SimpleComponent)
├── SearchView (Component)
│   └── SearchResults (TypedListView<SearchResultItem>)
└── SettingsWindow (SimpleComponent)
```

### Message Flow

```
                    ┌──────────────┐
                    │     App      │
                    │              │
                    │ AppInput:    │
                    │  Navigate    │◄─── Sidebar::Output::Navigate
                    │  PlayMedia   │◄─── LibraryView::Output::PlayItem
                    │  ShowDetail  │◄─── LibraryView::Output::ShowDetail
                    │  PlayerEvent │◄─── PlayerView::Output::StateChanged
                    │  SearchOpen  │
                    └──────┬───────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
     ┌────────▼──┐  ┌──────▼────┐ ┌────▼──────┐
     │ Sidebar   │  │ Library   │ │  Player   │
     │           │  │ View      │ │  View     │
     │ Input:    │  │           │ │           │
     │  SetActive│  │ Input:    │ │ Input:    │
     │           │  │  LoadLib  │ │  PlayUri  │
     │ Output:   │  │  Filter   │ │  Seek     │
     │  Navigate │  │  Sort     │ │  Pause    │
     └───────────┘  │           │ │           │
                    │ Output:   │ │ Output:   │
                    │  PlayItem │ │  State    │
                    │  ShowDet  │ │  Changed  │
                    └───────────┘ └───────────┘
```

### Global State via MessageBroker

A `PlaybackBroker` broadcasts playback state changes to any component that needs to react:

```rust
// Any component can observe playback state
static PLAYBACK_BROKER: MessageBroker<PlaybackEvent> = MessageBroker::new();

enum PlaybackEvent {
    NowPlaying { media_id: String, title: String, artwork_url: Option<String> },
    PositionChanged { position: Duration, duration: Duration },
    StateChanged(PlayState),   // Playing, Paused, Stopped
    TrackChanged { audio: Vec<TrackInfo>, subtitle: Vec<TrackInfo> },
}
```

This lets the sidebar show "now playing" info, the library show progress bars, and the header show playback state - without threading messages through the entire component tree.

---

## Service Layer Architecture

### MediaSource Trait

The core abstraction enabling multiple backends:

```rust
#[async_trait]
pub trait MediaSource: Send + Sync {
    /// Display name for this source
    fn name(&self) -> &str;

    /// Source type identifier
    fn source_type(&self) -> SourceType;

    /// Test connectivity
    async fn test_connection(&self) -> Result<(), SourceError>;

    /// Fetch all movies from this source
    async fn movies(&self, options: &FetchOptions) -> Result<Vec<Movie>, SourceError>;

    /// Fetch all TV shows from this source
    async fn shows(&self, options: &FetchOptions) -> Result<Vec<TvShow>, SourceError>;

    /// Fetch seasons for a show
    async fn seasons(&self, show_id: &str) -> Result<Vec<Season>, SourceError>;

    /// Fetch episodes for a season
    async fn episodes(&self, show_id: &str, season_number: u32) -> Result<Vec<Episode>, SourceError>;

    /// Get a playable URI for a media item
    async fn playback_uri(&self, media_id: &str) -> Result<String, SourceError>;

    /// Report watch progress back to the source
    async fn report_progress(&self, media_id: &str, position: Duration, duration: Duration) -> Result<(), SourceError>;

    /// Mark as watched/unwatched
    async fn set_watched(&self, media_id: &str, watched: bool) -> Result<(), SourceError>;

    /// Search across this source
    async fn search(&self, query: &str) -> Result<Vec<MediaItem>, SourceError>;

    /// Get collections/sets
    async fn collections(&self) -> Result<Vec<Collection>, SourceError>;

    /// Get recently added items
    async fn recently_added(&self, limit: usize) -> Result<Vec<MediaItem>, SourceError>;

    /// Get continue watching / on deck
    async fn continue_watching(&self) -> Result<Vec<MediaItem>, SourceError>;
}
```

This trait is the integration seam. `PlexSource` implements it first. `JellyfinSource`, `EmbySource`, and `LocalSource` implement it later without changing any UI code.

### Plex API Client

```rust
pub struct PlexClient {
    http: reqwest::Client,
    base_url: String,
    auth_token: String,
    client_identifier: String,
}

impl PlexClient {
    pub async fn libraries(&self) -> Result<Vec<PlexLibrary>>;
    pub async fn library_items(&self, library_key: &str, options: &PlexFetchOptions) -> Result<Vec<PlexMediaItem>>;
    pub async fn metadata(&self, rating_key: &str) -> Result<PlexMetadata>;
    pub async fn children(&self, rating_key: &str) -> Result<Vec<PlexMediaItem>>;
    pub async fn search(&self, query: &str) -> Result<Vec<PlexMediaItem>>;
    pub async fn on_deck(&self) -> Result<Vec<PlexMediaItem>>;
    pub async fn recently_added(&self, library_key: &str) -> Result<Vec<PlexMediaItem>>;
    pub async fn timeline(&self, rating_key: &str, state: &str, time: u64, duration: u64) -> Result<()>;
    pub async fn scrobble(&self, rating_key: &str) -> Result<()>;
    pub async fn unscrobble(&self, rating_key: &str) -> Result<()>;
    pub async fn transcode_image_url(&self, url: &str, width: u32, height: u32) -> String;
}
```

### Metadata Resolution Pipeline

For standalone sources (not Plex/Jellyfin), Reel resolves metadata through a pipeline:

```
Filename → Parse (title, year, season, episode)
         → TMDb Search (title + year)
         → TMDb Details (cast, crew, synopsis, artwork URLs)
         → Download artwork → Cache to disk
         → Store in SQLite
```

For Plex sources, metadata comes directly from the Plex API - no TMDb lookup needed.

---

## Video Backend Architecture

### Design Goals

1. **Backend-agnostic** - The UI and services never interact with mpv or GStreamer directly
2. **mpv first** - mpv provides superior playback quality, codec support, and hardware acceleration
3. **Swappable** - A GStreamer backend can be added later without touching UI code
4. **GTK4-native rendering** - Video renders into GtkGLArea via the mpv render API

### VideoBackend Trait

The playback abstraction that all backends implement:

```rust
/// Events emitted by the video backend, consumed by the player UI
#[derive(Debug, Clone)]
pub enum BackendEvent {
    /// Playback state changed
    StateChanged(PlayState),
    /// Position updated (position_secs, duration_secs)
    PositionChanged { position: f64, duration: f64 },
    /// File loaded - track info now available
    FileLoaded,
    /// Track list changed (available audio, subtitle, video tracks)
    TracksChanged(Vec<TrackInfo>),
    /// End of file reached (with reason)
    EndOfFile(EndReason),
    /// Error occurred
    Error(String),
    /// A new video frame is ready for rendering
    FrameReady,
}

#[derive(Debug, Clone)]
pub enum PlayState {
    Playing,
    Paused,
    Stopped,
    Buffering,
}

#[derive(Debug, Clone)]
pub enum EndReason {
    Finished,    // Normal EOF
    Stopped,     // User stopped
    Error,       // Playback error
}

#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub id: i64,
    pub track_type: TrackType,  // Audio, Video, Subtitle
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
    pub fps: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackType {
    Audio,
    Video,
    Subtitle,
}

/// Subtitle style configuration
#[derive(Debug, Clone)]
pub struct SubtitleStyle {
    pub font_family: String,
    pub font_size: f64,
    pub color: String,          // "#RRGGBB"
    pub border_size: f64,
    pub border_color: String,
    pub position: i64,          // 0=top, 100=bottom
    pub visibility: bool,
}

/// The core video backend trait.
/// Implementations: MpvBackend, (future) GStreamerBackend
pub trait VideoBackend: Send {
    // --- Lifecycle ---

    /// Load and start playing a URI (file path or URL)
    fn load_file(&self, uri: &str);

    /// Stop playback and release resources for the current file
    fn stop(&self);

    // --- Playback control ---

    fn play(&self);
    fn pause(&self);
    fn toggle_pause(&self);
    fn seek_absolute(&self, position_secs: f64);
    fn seek_relative(&self, offset_secs: f64);
    fn set_speed(&self, speed: f64);

    // --- Volume ---

    fn set_volume(&self, volume: f64);   // 0-100, can exceed 100 for boost
    fn set_mute(&self, mute: bool);

    // --- Track selection ---

    fn set_audio_track(&self, track_id: i64);
    fn set_subtitle_track(&self, track_id: i64);
    fn set_subtitle_none(&self);
    fn add_subtitle_file(&self, path: &str);

    // --- Subtitle style ---

    fn set_subtitle_style(&self, style: &SubtitleStyle);
    fn set_subtitle_delay(&self, delay_secs: f64);

    // --- Chapter navigation ---

    fn set_chapter(&self, chapter: i64);
    fn chapter_count(&self) -> i64;

    // --- State queries ---

    fn position(&self) -> Option<f64>;
    fn duration(&self) -> Option<f64>;
    fn is_paused(&self) -> bool;
    fn current_speed(&self) -> f64;
    fn volume(&self) -> f64;
    fn is_muted(&self) -> bool;
    fn tracks(&self) -> Vec<TrackInfo>;
    fn hwdec_active(&self) -> Option<String>;

    // --- Rendering (for GL-based backends) ---

    /// Render current video frame into the given OpenGL FBO.
    /// Called from the GL thread during GtkGLArea::render signal.
    fn render_gl(&self, fbo: i32, width: i32, height: i32);

    /// Check if a new frame is available (called after update callback fires)
    fn needs_render(&self) -> bool;

    /// Report that the rendered frame was swapped/displayed (for frame timing)
    fn report_swap(&self);
}
```

### Why mpv Over GStreamer

| Aspect | mpv (libmpv) | GStreamer |
|--------|-------------|----------|
| Playback quality | Superior - 100+ video output options, advanced upscaling (ewa_lanczos), debanding, color management, HDR tone mapping, interpolation | Good but fewer post-processing options |
| Codec support | Excellent via FFmpeg - every format out of the box | Excellent but depends on installed plugins (gst-plugins-ugly/bad) |
| Subtitle rendering | Excellent - libass with full ASS/SSA support, extensive customization | Varies by element, less mature |
| Hardware acceleration | Comprehensive - VA-API, NVDEC, Vulkan Video, auto-detection | Good - VA-API via separate plugin, less seamless |
| Embedding complexity | Simple - render API + GL FBO, well-proven pattern (Celluloid) | Complex - pipeline assembly, sink configuration |
| Configuration | 200+ runtime-changeable properties via simple API | Pipeline reconfiguration is complex |
| Battle-tested | Used by Celluloid, Haruna, Stremio, every mpv frontend | Used by GNOME Videos, Clapper |
| Rust bindings | `libmpv2` v5.0.3 (active, Dec 2025, LGPL-2.1) | `gstreamer-rs` (mature, well-maintained) |

mpv is the better choice for a media player focused on playback quality. GStreamer would be better if Reel needed to build custom media pipelines (transcoding, streaming, etc.), which it doesn't.

---

## mpv Backend Implementation

### Crate Selection: `libmpv2`

| Crate | Status | Last Update | mpv Compat | Render API | License |
|-------|--------|-------------|------------|------------|---------|
| **`libmpv2`** | **Active** | **Dec 2025** | **>= 0.35** | **Yes (OpenGL)** | **LGPL-2.1** |
| `libmpv` (original) | Dead | Sep 2020 | < 0.35 only | Yes | LGPL-2.1 |
| `libmpv-sirno` | Temp fork | Dec 2022 | >= 0.35 | Yes | LGPL-2.1 |
| `mpv` (Cobrand) | Deprecated | Jan 2017 | Very old | Partial (removed API) | MIT |
| `mpv-client` | Active | Jun 2025 | Modern | No (plugin-only) | GPL-3.0 |

**`libmpv2`** is the clear choice: actively maintained, supports the render API, modern mpv compatibility, LGPL-2.1 license.

### MpvBackend Architecture

```rust
/// Owns the mpv handle, render context, and event processing.
pub struct MpvBackend {
    mpv: Mpv,
    render_ctx: RenderContext,
    event_sender: glib::Sender<BackendEvent>,
}
```

### Initialization Flow

```
1. Mpv::with_initializer()
   ├── set_property("vo", "libmpv")       // Use render API, not own window
   ├── set_property("hwdec", "auto")      // Automatic hardware acceleration
   ├── set_property("keep-open", "yes")   // Don't close at EOF
   └── set_property("idle", "yes")        // Start in idle mode

2. GtkGLArea realize signal fires
   ├── gl_area.make_current()
   ├── Detect platform (Wayland or X11)
   ├── Get native display handle:
   │   ├── Wayland: gdk_wayland_display_get_wl_display()
   │   └── X11:     gdk_x11_display_get_xdisplay()
   ├── Resolve get_proc_address:
   │   ├── Wayland: eglGetProcAddress
   │   └── X11:     glXGetProcAddressARB (or eglGetProcAddress)
   └── mpv.create_render_context([
         ApiType(OpenGl),
         InitParams(OpenGLInitParams { get_proc_address, ctx }),
         WaylandDisplay(wl_display_ptr),  // or X11Display
       ])

3. Set update callback
   └── render_ctx.set_update_callback(|| {
         // Called from mpv thread - must NOT call mpv API
         // Schedule redraw on main thread:
         glib::idle_add_once(|| gl_area.queue_render());
       })

4. Set wakeup callback for events
   └── mpv.set_wakeup_callback(|| {
         glib::idle_add_once(|| process_mpv_events());
       })
```

### GTK4 Video Rendering Pipeline

The rendering follows the same proven pattern used by Celluloid and Cine:

```
mpv internal thread
  │
  ▼ (new frame ready)
update_callback()                    [mpv thread - cannot call mpv API]
  │
  ▼ (schedule on main thread)
glib::idle_add(gl_area.queue_render) [marshals to GTK main thread]
  │
  ▼ (GTK render cycle)
GtkGLArea::render signal             [GL context is current]
  │
  ├── glGetIntegerv(GL_FRAMEBUFFER_BINDING) → fbo
  ├── get widget allocation → width, height
  ├── multiply by scale_factor for HiDPI
  │
  ▼
mpv_render_context_render(fbo, w, h, flip_y=true)
  │
  ▼
mpv_render_context_report_swap()     [optional, for frame timing]
```

### Widget Hierarchy for Video Area

```
GtkOverlay
├── GtkGraphicsOffload               # Wayland zero-copy optimization (GTK 4.14+)
│   └── GtkGLArea                    # OpenGL surface where mpv renders
│       ├── signal::realize → init mpv render context
│       └── signal::render  → call mpv_render_context_render()
└── overlay: PlayerControls          # Playback controls drawn on top
    ├── GtkBox (top: title bar)
    ├── GtkBox (center: big play button)
    └── GtkBox (bottom: progress bar, volume, track selector)
```

`GtkGraphicsOffload` (GTK 4.14+) enables the Wayland compositor to bypass GTK's GSK renderer for the video surface, allowing DMA-BUF direct scanout when possible. Falls back to normal compositing on X11 or when unsupported. Disabled on NVIDIA GPUs where it can cause issues (same approach as Celluloid).

### mpv Event Processing

Events are processed on the GTK main thread via wakeup callback:

```rust
fn process_mpv_events(mpv: &Mpv, sender: &glib::Sender<BackendEvent>) {
    loop {
        let event = mpv.wait_event(0.0);  // 0 = never block
        match event {
            Event::None => break,  // No more events
            Event::FileLoaded => {
                sender.send(BackendEvent::FileLoaded).ok();
            }
            Event::EndFile(reason) => {
                let end_reason = match reason {
                    EndFileReason::Eof => EndReason::Finished,
                    EndFileReason::Stop => EndReason::Stopped,
                    EndFileReason::Error(_) => EndReason::Error,
                    _ => EndReason::Stopped,
                };
                sender.send(BackendEvent::EndOfFile(end_reason)).ok();
            }
            Event::PropertyChange { name, data, .. } => {
                match name.as_str() {
                    "playback-time" => { /* send PositionChanged */ }
                    "pause" => { /* send StateChanged */ }
                    "track-list" => { /* parse tracks, send TracksChanged */ }
                    "volume" | "mute" => { /* send volume change */ }
                    _ => {}
                }
            }
            Event::Seek => { /* seeking started */ }
            Event::PlaybackRestart => { /* seek completed */ }
            _ => {}
        }
    }
}
```

### Property Observation

The mpv backend observes these properties for UI updates:

| Property | Format | Purpose |
|----------|--------|---------|
| `playback-time` | Double | Progress bar position |
| `duration` | Double | Total length display |
| `pause` | Flag | Play/pause button state |
| `volume` | Double | Volume slider |
| `mute` | Flag | Mute button state |
| `speed` | Double | Speed indicator |
| `track-list` | Node | Audio/subtitle track menus |
| `chapter` | Int64 | Chapter navigation |
| `chapters` | Int64 | Chapter count |
| `media-title` | String | Window title, MPRIS |
| `eof-reached` | Flag | End-of-file detection |
| `idle-active` | Flag | Idle state detection |
| `hwdec-current` | String | HW acceleration status display |
| `sub-visibility` | Flag | Subtitle toggle state |

### Hardware Acceleration

mpv handles hardware acceleration automatically. Reel sets `hwdec=auto` and mpv selects the best available decoder:

| Hardware | API | mpv Elements | Notes |
|----------|-----|-------------|-------|
| Intel | VA-API | vaapi | Zero-copy via EGL interop |
| AMD | VA-API | vaapi | Zero-copy via EGL interop |
| NVIDIA | NVDEC | nvdec | Requires nvidia-vaapi-driver for GL interop |
| Any | Vulkan Video | vulkan | Emerging, H.264/H.265/AV1/VP9 |
| Fallback | Software | ffmpeg | Always works |

When using the render API with OpenGL, hardware-decoded frames stay in GPU memory and are composited directly into the FBO - zero CPU-GPU transfer in the optimal path.

The Wayland or X11 display handle passed during render context creation is critical for hwdec interop - without it, hardware decoding silently falls back to software.

### Track List Parsing

mpv's `track-list` property returns a structured node array. Each entry contains:

```rust
struct MpvTrackEntry {
    id: i64,           // Track ID for aid/sid/vid selection
    r#type: String,    // "audio", "video", "sub"
    title: Option<String>,
    lang: Option<String>,
    codec: Option<String>,
    decoder_desc: Option<String>,
    default: bool,
    forced: bool,
    selected: bool,
    external: bool,
    external_filename: Option<String>,
    // Audio-specific
    demux_channel_count: Option<i64>,
    demux_samplerate: Option<i64>,
    // Video-specific
    demux_w: Option<i64>,
    demux_h: Option<i64>,
    demux_fps: Option<f64>,
}
```

This is parsed from `MpvNode` and converted to the backend-agnostic `TrackInfo` type.

### Subtitle Handling

mpv provides comprehensive subtitle support:

```rust
// Select embedded subtitle track
mpv.set_property("sid", track_id)?;

// Disable subtitles
mpv.set_property("sid", "no")?;

// Load external subtitle file
mpv.command("sub-add", &[path, "select"])?;

// Style customization (all runtime-changeable)
mpv.set_property("sub-font", "Sans Serif")?;
mpv.set_property("sub-font-size", 48.0)?;
mpv.set_property("sub-color", "#FFFFFFFF")?;
mpv.set_property("sub-border-size", 3.0)?;
mpv.set_property("sub-border-color", "#FF000000")?;
mpv.set_property("sub-pos", 95)?;        // 0=top, 100=bottom
mpv.set_property("sub-visibility", true)?;
mpv.set_property("sub-delay", 0.0)?;     // timing offset in seconds
mpv.set_property("sub-ass-override", "force")?;  // override ASS styles
```

### Ownership and Lifetime Challenges

The main challenge in Rust: `RenderContext` borrows `&Mpv`, but both need to live in widget state managed by GTK.

**Solution pattern** (from libmpv2's design):
- `Mpv` is created once in `main.rs` or the `App` component
- `RenderContext` is created when the GLArea is realized
- Both are stored in an `Rc<RefCell<...>>` or wrapped in a custom struct
- The event processing callback and render callback both access the mpv handle via cloned `Rc` references
- `RenderContext` is dropped before `Mpv` (enforced by drop order in the wrapper struct)

```rust
pub struct MpvState {
    mpv: Mpv,
    render_ctx: Option<RenderContext<'static>>,
    // render_ctx holds a reference into mpv, but we use unsafe lifetime
    // extension because both live in the same struct and drop order is
    // guaranteed (fields drop in declaration order)
}
```

Alternative: use `libmpv2-sys` directly with raw pointers for the render context, managing lifetimes manually. This is what Celluloid effectively does in C.

---

## MPRIS2 Integration

Reel implements the MPRIS2 D-Bus specification for desktop integration:

```
org.mpris.MediaPlayer2           - Application identity
org.mpris.MediaPlayer2.Player    - Playback control
```

**Exposed properties:** PlaybackStatus, Rate, Metadata (title, artist, artwork), Volume, Position, CanSeek, CanPause, CanGoNext, CanGoPrevious

**Signals:** Seeked, PropertiesChanged

**Implementation:** A background task (Relm4 Worker or tokio task) owns the D-Bus connection and translates between D-Bus method calls and Relm4 messages. The MPRIS layer talks to the `VideoBackend` trait, not to mpv directly, so it works with any backend.

---

## Database Schema

SQLite database at `$XDG_DATA_HOME/reel/reel.db`:

```sql
-- Media items from all sources
CREATE TABLE media_items (
    id TEXT PRIMARY KEY,              -- source_type:source_id:item_id
    source_type TEXT NOT NULL,        -- "plex", "local", etc.
    source_id TEXT NOT NULL,          -- server URL or path
    external_id TEXT NOT NULL,        -- ID within the source
    media_type TEXT NOT NULL,         -- "movie", "show", "season", "episode"
    title TEXT NOT NULL,
    original_title TEXT,
    year INTEGER,
    sort_title TEXT,
    overview TEXT,
    content_rating TEXT,              -- "PG-13", "TV-MA", etc.
    rating REAL,                      -- TMDb or source rating (0-10)
    runtime_minutes INTEGER,
    poster_path TEXT,                 -- Local cached path
    backdrop_path TEXT,               -- Local cached path
    genre_ids TEXT,                   -- JSON array of genre strings
    -- TV-specific
    parent_id TEXT REFERENCES media_items(id),  -- show_id for seasons, season_id for episodes
    season_number INTEGER,
    episode_number INTEGER,
    air_date TEXT,
    -- File info
    file_path TEXT,                   -- Playable URI or path
    file_size INTEGER,
    video_codec TEXT,
    audio_codec TEXT,
    resolution TEXT,                  -- "1080p", "4K", etc.
    -- Collection
    collection_id TEXT,
    collection_name TEXT,
    collection_order INTEGER,
    -- Timestamps
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Watch state (separate table for fast updates)
CREATE TABLE watch_state (
    media_id TEXT PRIMARY KEY REFERENCES media_items(id),
    watched INTEGER NOT NULL DEFAULT 0,
    position_ms INTEGER NOT NULL DEFAULT 0,  -- Resume position
    duration_ms INTEGER NOT NULL DEFAULT 0,
    last_watched_at TEXT,
    watch_count INTEGER NOT NULL DEFAULT 0
);

-- Cast and crew
CREATE TABLE people (
    id TEXT PRIMARY KEY,              -- tmdb_id or source-specific
    name TEXT NOT NULL,
    photo_path TEXT                   -- Local cached path
);

CREATE TABLE media_people (
    media_id TEXT NOT NULL REFERENCES media_items(id),
    person_id TEXT NOT NULL REFERENCES people(id),
    role TEXT NOT NULL,               -- "cast", "director", "writer"
    character_name TEXT,              -- For cast
    display_order INTEGER,
    PRIMARY KEY (media_id, person_id, role)
);

-- Configured media sources
CREATE TABLE sources (
    id TEXT PRIMARY KEY,
    source_type TEXT NOT NULL,
    name TEXT NOT NULL,
    config TEXT NOT NULL,             -- JSON: connection details
    enabled INTEGER NOT NULL DEFAULT 1,
    last_synced_at TEXT
);

-- Genres lookup
CREATE TABLE genres (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

-- Indexes
CREATE INDEX idx_media_type ON media_items(media_type);
CREATE INDEX idx_media_parent ON media_items(parent_id);
CREATE INDEX idx_media_collection ON media_items(collection_id);
CREATE INDEX idx_media_source ON media_items(source_type, source_id);
CREATE INDEX idx_watch_state_watched ON watch_state(watched);
CREATE INDEX idx_media_added ON media_items(added_at DESC);
```

---

## Async Architecture

### Threading Model

```
┌─────────────────────────┐
│   Main Thread (GTK)      │  UI rendering, widget updates, signal handling
│   Relm4 event loop      │  Component update() and update_view()
│                          │  mpv event processing (via wakeup callback)
│                          │  GtkGLArea render → mpv_render_context_render()
└──────────┬──────────────┘
           │ Messages
┌──────────┴──────────────┐
│   Tokio Runtime          │  Managed by Relm4 for Commands
│                          │  HTTP requests (Plex API, TMDb, Trakt)
│   oneshot_command()      │  Image downloads
│   command()              │  Subtitle search
└─────────────────────────┘

┌─────────────────────────┐
│   Worker Threads         │  Relm4 Workers (dedicated threads)
│                          │
│   LibraryScanner         │  Sequential media scanning
│   DatabaseWorker         │  SQLite read/write (single connection)
└─────────────────────────┘

┌─────────────────────────┐
│   mpv Internal Threads   │  Managed by mpv internally
│                          │  Decoding, audio output, demuxing
│   update_callback ──────────► glib::idle_add → GLArea queue_render
│   wakeup_callback ─────────► glib::idle_add → process_mpv_events
└─────────────────────────┘
```

### Key Design Decisions

1. **SQLite on a Worker thread** - SQLite connections are not Send. A single `DatabaseWorker` owns the connection and processes queries sequentially. Components send query messages and receive results as `Output`.

2. **HTTP via Commands** - API calls use `sender.oneshot_command(async { ... })` for parallel execution. Multiple metadata/artwork fetches can run simultaneously.

3. **mpv events via wakeup callback** - mpv's wakeup callback fires from an internal thread and schedules event processing on the GTK main thread via `glib::idle_add`. Events are drained in a loop with `mpv_wait_event(0)` until `MPV_EVENT_NONE`. This keeps all state updates on the main thread.

4. **mpv rendering via GLArea** - The `render` signal on `GtkGLArea` is triggered when mpv's update callback fires and signals `queue_render()`. The actual `mpv_render_context_render()` call happens in the render signal handler where the GL context is already current.

5. **Artwork loading** - Poster images are loaded asynchronously. The `MediaCard` factory component displays a placeholder, kicks off a command to fetch/load the image, then updates the widget when the image arrives.

---

## Error Handling Strategy

```rust
/// Application-wide error type
#[derive(Debug, thiserror::Error)]
pub enum ReelError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Playback error: {0}")]
    Playback(String),

    #[error("mpv error: {0}")]
    Mpv(#[from] libmpv2::Error),

    #[error("Source error: {source_name} - {message}")]
    Source { source_name: String, message: String },

    #[error("Metadata not found for: {query}")]
    MetadataNotFound { query: String },

    #[error("Configuration error: {0}")]
    Config(String),
}
```

**Strategy:**
- Network errors → Toast notification + retry option
- Playback errors → Error dialog with codec/format details
- Database errors → Log + graceful degradation (skip caching)
- Metadata errors → Show media with filename-only info, no crash
- All errors logged via `tracing` with context

---

## Packaging & Distribution

### Primary: Flatpak (Flathub)

```yaml
# flatpak/com.reel.Reel.yml
app-id: com.reel.Reel
runtime: org.gnome.Platform
runtime-version: '46'
sdk: org.gnome.Sdk
sdk-extensions:
  - org.freedesktop.Sdk.Extension.rust-stable
command: reel

finish-args:
  - --share=ipc
  - --socket=fallback-x11
  - --socket=wayland
  - --socket=pulseaudio
  - --device=dri                    # GPU access for HW accel
  - --share=network                 # Plex API, metadata, subtitles
  - --talk-name=org.freedesktop.ScreenSaver  # Inhibit screensaver
  - --own-name=org.mpris.MediaPlayer2.reel   # MPRIS

modules:
  - name: mpv
    buildsystem: meson             # mpv must be built as a dependency
    config-opts:
      - -Dlibmpv=true
      - -Dcplayer=false            # Only need libmpv, not the mpv binary
    sources:
      - type: archive
        url: https://github.com/mpv-player/mpv/archive/v0.39.0.tar.gz
  - name: reel
    buildsystem: simple
    # ... build steps
```

### Secondary: Native Packages

- Nix / NixOS flake
- Arch Linux AUR
- Debian/Ubuntu .deb (future)
- Fedora COPR (future)

### Build System

Primary: `cargo` with standard Rust toolchain.

For Flatpak: Meson wrapper that invokes cargo, installs desktop files, icons, and metainfo. mpv built as a Flatpak module dependency.

### System Dependencies

Reel requires libmpv to be installed on the system (or bundled in Flatpak):

| Distro | Package |
|--------|---------|
| Arch Linux | `mpv` (provides libmpv.so) |
| Fedora | `mpv-libs-devel` |
| Ubuntu/Debian | `libmpv-dev` |
| NixOS | `mpv` (in buildInputs) |
| Flatpak | Built as module (see above) |

---

## Performance Considerations

1. **Virtual scrolling** - `TypedGridView` / `TypedListView` for library grids. Only visible items are rendered, enabling smooth scrolling through thousands of items.

2. **Lazy image loading** - Poster artwork loaded on-demand as items scroll into view. Placeholder shown during load. Loaded textures cached in memory (LRU cache with configurable size).

3. **Database queries** - All queries parameterized and indexed. Library view loads page-by-page if needed. Watch state updates are batched.

4. **Artwork cache** - Downloaded images stored on disk at appropriate resolution (poster: 300w, backdrop: 1280w). No re-download unless cache cleared.

5. **Video rendering** - mpv renders directly into the GLArea's FBO with hardware acceleration. `GtkGraphicsOffload` on GTK 4.14+ enables zero-copy compositor pass-through on Wayland. HW-decoded frames stay in GPU memory via EGL interop.

6. **Startup time** - Minimal startup work: open DB, load settings, show UI. Library content loaded asynchronously after UI is visible. mpv initialized lazily on first playback.

---

## Testing Strategy

| Layer | Approach | Tools |
|-------|----------|-------|
| Filename parser | Unit tests | `#[cfg(test)]` |
| API clients (Plex, TMDb) | Unit tests with mock responses | `mockito` or `wiremock` |
| Database repos | Integration tests with in-memory SQLite | `rusqlite::Connection::open_in_memory()` |
| Models / data types | Unit tests | Standard Rust tests |
| Services | Integration tests | Combination of mocks + real dependencies |
| VideoBackend trait | Integration tests with test media files | libmpv in headless mode |
| UI components | Manual testing | GTK Inspector |

### CI Pipeline

```
cargo fmt --check → cargo clippy → cargo test → cargo build --release → flatpak-builder
```

---

## Cargo Dependencies

```toml
[dependencies]
# UI
relm4 = { version = "0.10", features = ["macros", "libadwaita"] }
relm4-components = "0.10"
relm4-css = "0.10"
gtk4 = { version = "0.10", features = ["v4_14"] }
libadwaita = { version = "0.9", features = ["v1_4"] }

# Video backend (mpv)
libmpv2 = { version = "5.0", features = ["render"] }

# GL interop
gdk4-wayland = { version = "0.10", features = ["egl", "wayland_crate"], optional = true }
gdk4-x11 = { version = "0.10", features = ["xlib", "egl"], optional = true }
khronos-egl = { version = "6", features = ["dynamic"] }

# Database
rusqlite = { version = "0.38", features = ["bundled"] }

# HTTP & serialization
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# Async
tokio = { version = "1", features = ["full"] }

# Error handling
thiserror = "2"

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# Change tracking
tracker = "0.2"

[features]
default = ["wayland", "x11"]
wayland = ["dep:gdk4-wayland"]
x11 = ["dep:gdk4-x11"]
```
