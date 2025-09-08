# Slint Platform Implementation Guide

This document provides a comprehensive guide for implementing a Slint-based UI platform for the Reel media player, maximizing reuse of existing cross-platform architecture while providing a modern, declarative UI experience.

## Overview

Slint is a declarative GUI toolkit for Rust that offers:
- Declarative UI with `.slint` markup files
- Native performance with GPU acceleration
- OpenGL integration for video playback
- Cross-platform support (Linux, Windows, macOS, embedded)
- Memory-efficient runtime (< 300KB RAM)
- Reactive property system

This guide outlines how to create a `src/platforms/slint/` implementation that leverages the existing reactive ViewModels and core architecture.

## Architecture Integration

### Reusable Components from Current Architecture

The Slint platform implementation can reuse **90%** of the existing codebase:

#### ✅ Fully Reusable (No Changes Needed)
- **Core ViewModels** (`src/core/viewmodels/`): LibraryViewModel, PlayerViewModel, HomeViewModel, etc.
- **Event System** (`src/events/`): EventBus and all event types
- **Database Layer** (`src/db/`): All repositories, entities, and migrations
- **Services Layer** (`src/services/`): DataService, SyncManager, AuthManager, SourceCoordinator
- **Backend System** (`src/backends/`): Plex, Jellyfin, Local implementations
- **Models** (`src/models/`): All data models and auth providers
- **State Management** (`src/state.rs`): Complete AppState with shared config
- **Configuration** (`src/config/`): All configuration management

#### 🔄 Platform-Specific Implementation Required
- **UI Layer**: New Slint components replacing GTK widgets
- **Player Integration**: Custom MPV player using Slint's OpenGL integration
- **Platform App**: Slint application initialization and window management
- **Frontend Implementation**: Slint-specific Frontend trait implementation

### Directory Structure

```
src/platforms/slint/
├── mod.rs                    # Platform module exports
├── app.rs                    # Slint application initialization
├── platform_utils.rs        # Platform-specific utilities
├── frontend.rs               # Frontend trait implementation
├── player/
│   ├── mod.rs
│   └── slint_mpv_player.rs   # MPV integration with Slint OpenGL
├── ui/
│   ├── mod.rs
│   ├── slint_app.slint       # Main application UI definition
│   ├── components/
│   │   ├── media_card.slint
│   │   ├── media_list.slint
│   │   ├── player_controls.slint
│   │   └── sidebar.slint
│   ├── pages/
│   │   ├── home.slint
│   │   ├── library.slint
│   │   ├── player.slint
│   │   ├── sources.slint
│   │   ├── movie_details.slint
│   │   └── show_details.slint
│   └── adapters/             # ViewModel to Slint adapters
│       ├── mod.rs
│       ├── library_adapter.rs
│       ├── player_adapter.rs
│       ├── home_adapter.rs
│       └── sidebar_adapter.rs
```

## ViewModels Integration Strategy

The existing ViewModels use a `Property<T>` system with `PropertySubscriber` for reactive updates. Slint has its own property system, so we need adapter layers:

### ViewModel Adapter Pattern

```rust
// Example: LibraryAdapter bridges LibraryViewModel to Slint
use crate::core::viewmodels::{LibraryViewModel, PropertySubscriber};
use slint::{ComponentHandle, SharedString, VecModel, Rc};

pub struct LibraryAdapter {
    view_model: Arc<LibraryViewModel>,
    // Slint property handles
    items_model: Rc<VecModel<MediaItemData>>,
    is_loading: Arc<AtomicBool>,
    // Property subscribers
    subscribers: Vec<PropertySubscriber>,
}

impl LibraryAdapter {
    pub fn new(view_model: Arc<LibraryViewModel>, slint_handle: LibraryPageHandle) -> Self {
        let items_model = Rc::new(VecModel::default());
        let is_loading = Arc::new(AtomicBool::new(false));
        
        // Connect Slint UI to adapter
        slint_handle.set_items_model(items_model.clone().into());
        
        let adapter = Self {
            view_model,
            items_model,
            is_loading: is_loading.clone(),
            subscribers: Vec::new(),
        };
        
        // Subscribe to ViewModel property changes
        adapter.setup_subscriptions(slint_handle);
        adapter
    }
    
    fn setup_subscriptions(&mut self, slint_handle: LibraryPageHandle) {
        // Subscribe to items changes
        if let Some(mut subscriber) = self.view_model.subscribe_to_property("filtered_items") {
            let items_model = self.items_model.clone();
            let view_model = self.view_model.clone();
            
            tokio::spawn(async move {
                while subscriber.wait_for_change().await {
                    let items = view_model.get_filtered_items().await;
                    let slint_items: Vec<MediaItemData> = items.into_iter()
                        .map(|item| MediaItemData::from(item))
                        .collect();
                    
                    slint::invoke_from_event_loop(move || {
                        items_model.set_vec(slint_items);
                    }).unwrap();
                }
            });
            
            self.subscribers.push(subscriber);
        }
        
        // Subscribe to loading state
        if let Some(mut subscriber) = self.view_model.subscribe_to_property("is_loading") {
            let is_loading = self.is_loading.clone();
            let view_model = self.view_model.clone();
            let handle = slint_handle.as_weak();
            
            tokio::spawn(async move {
                while subscriber.wait_for_change().await {
                    let loading = view_model.get_is_loading().await;
                    is_loading.store(loading, Ordering::Relaxed);
                    
                    let handle = handle.clone();
                    slint::invoke_from_event_loop(move || {
                        if let Some(handle) = handle.upgrade() {
                            handle.set_is_loading(loading);
                        }
                    }).unwrap();
                }
            });
            
            self.subscribers.push(subscriber);
        }
    }
}
```

## Video Player Integration with Slint

Slint officially supports GStreamer integration through AppSink, which is the **recommended approach** over MPV integration. This leverages Slint's native video buffer handling capabilities.

### GStreamer Player Implementation (Recommended)

```rust
// src/platforms/slint/player/slint_gstreamer_player.rs
use crate::player::traits::Player;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer, Weak};
use std::sync::{Arc, Mutex};
use anyhow::Result;

pub struct SlintGStreamerPlayer {
    pipeline: gst::Pipeline,
    appsink: gst_app::AppSink,
    video_info: Arc<Mutex<Option<gst_video::VideoInfo>>>,
    slint_handle: Option<Weak<slint::ComponentHandle<PlayerPage>>>,
}

impl SlintGStreamerPlayer {
    pub fn new() -> Result<Self> {
        // Initialize GStreamer
        gst::init()?;
        
        let pipeline = gst::Pipeline::new(Some("video-player"));
        
        // Create elements
        let src = gst::ElementFactory::make("uridecodebin", Some("src"))?;
        let videoconvert = gst::ElementFactory::make("videoconvert", Some("videoconvert"))?;
        let videoscale = gst::ElementFactory::make("videoscale", Some("videoscale"))?;
        let appsink = gst_app::AppSink::builder()
            .caps(&gst_video::VideoCapsBuilder::new()
                .format(gst_video::VideoFormat::Rgba)
                .build())
            .build();
        
        // Configure appsink for optimal performance
        appsink.set_property("sync", false);
        appsink.set_property("drop", true);
        appsink.set_property("max-buffers", 1u32);
        
        // Add elements to pipeline
        pipeline.add_many(&[src.upcast_ref(), videoconvert.upcast_ref(), 
                          videoscale.upcast_ref(), appsink.upcast_ref()])?;
        
        // Link elements
        gst::Element::link_many(&[&videoconvert, &videoscale, appsink.upcast_ref()])?;
        
        // Connect src pad-added signal for dynamic linking
        let videoconvert_weak = videoconvert.downgrade();
        src.connect_pad_added(move |_src, src_pad| {
            if let Some(videoconvert) = videoconvert_weak.upgrade() {
                let sink_pad = videoconvert.static_pad("sink").unwrap();
                let _ = src_pad.link(&sink_pad);
            }
        });
        
        let player = Self {
            pipeline,
            appsink,
            video_info: Arc::new(Mutex::new(None)),
            slint_handle: None,
        };
        
        // Set up frame callback
        player.setup_frame_callback();
        
        Ok(player)
    }
    
    pub fn set_slint_handle(&mut self, handle: Weak<slint::ComponentHandle<PlayerPage>>) {
        self.slint_handle = Some(handle);
    }
    
    fn setup_frame_callback(&self) {
        let video_info = self.video_info.clone();
        let slint_handle = self.slint_handle.clone();
        
        self.appsink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    if let Ok(sample) = appsink.pull_sample() {
                        if let Some(buffer) = sample.buffer() {
                            if let Some(slint_handle) = &slint_handle {
                                if let Some(handle) = slint_handle.upgrade() {
                                    // Convert GStreamer buffer to Slint image
                                    if let Ok(image) = Self::gst_buffer_to_slint_image(
                                        buffer, 
                                        &sample.caps().unwrap(),
                                        &video_info
                                    ) {
                                        // Update Slint UI from event loop thread
                                        let handle_clone = handle.clone();
                                        slint::invoke_from_event_loop(move || {
                                            handle_clone.set_video_frame(image);
                                        }).unwrap();
                                    }
                                }
                            }
                        }
                    }
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
    }
    
    fn gst_buffer_to_slint_image(
        buffer: &gst::Buffer,
        caps: &gst::Caps,
        video_info: &Arc<Mutex<Option<gst_video::VideoInfo>>>,
    ) -> Result<Image> {
        // Parse video info from caps if needed
        let info = {
            let mut info_guard = video_info.lock().unwrap();
            if info_guard.is_none() {
                *info_guard = Some(gst_video::VideoInfo::from_caps(caps)?);
            }
            info_guard.clone().unwrap()
        };
        
        // Map buffer for reading
        let map = buffer.map_readable()?;
        let width = info.width() as u32;
        let height = info.height() as u32;
        
        // Create Slint pixel buffer
        let pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
            &map, width, height
        );
        
        Ok(Image::from_rgba8(pixel_buffer))
    }
}

#[async_trait::async_trait]
impl Player for SlintGStreamerPlayer {
    async fn load_media(&self, url: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Set URI on the source element
        let src = self.pipeline.by_name("src").unwrap();
        src.set_property("uri", url);
        Ok(())
    }
    
    async fn play(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.pipeline.set_state(gst::State::Playing)?;
        Ok(())
    }
    
    async fn pause(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.pipeline.set_state(gst::State::Paused)?;
        Ok(())
    }
    
    async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.pipeline.set_state(gst::State::Null)?;
        Ok(())
    }
    
    async fn seek(&self, position: std::time::Duration) -> Result<(), Box<dyn std::error::Error>> {
        self.pipeline.seek_simple(
            gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
            position.as_nanos() as i64,
        )?;
        Ok(())
    }
    
    async fn get_position(&self) -> Result<std::time::Duration, Box<dyn std::error::Error>> {
        if let Some(position) = self.pipeline.query_position::<gst::ClockTime>() {
            Ok(std::time::Duration::from_nanos(position.nseconds()))
        } else {
            Ok(std::time::Duration::ZERO)
        }
    }
    
    async fn get_duration(&self) -> Result<std::time::Duration, Box<dyn std::error::Error>> {
        if let Some(duration) = self.pipeline.query_duration::<gst::ClockTime>() {
            Ok(std::time::Duration::from_nanos(duration.nseconds()))
        } else {
            Ok(std::time::Duration::ZERO)
        }
    }
    
    async fn set_volume(&self, volume: f32) -> Result<(), Box<dyn std::error::Error>> {
        // Find audio sink and set volume
        if let Some(audio_sink) = self.pipeline.by_interface(&gst::StreamVolume::static_type()) {
            audio_sink
                .dynamic_cast::<gst::StreamVolume>()
                .unwrap()
                .set_volume(gst::StreamVolumeFormat::Cubic, volume.into());
        }
        Ok(())
    }
    
    async fn get_volume(&self) -> Result<f32, Box<dyn std::error::Error>> {
        if let Some(audio_sink) = self.pipeline.by_interface(&gst::StreamVolume::static_type()) {
            let volume = audio_sink
                .dynamic_cast::<gst::StreamVolume>()
                .unwrap()
                .volume(gst::StreamVolumeFormat::Cubic);
            Ok(volume as f32)
        } else {
            Ok(1.0)
        }
    }
}
```

### Alternative: MPV Integration (Advanced)

For projects requiring MPV specifically, the Lumiere approach with OpenGL integration is possible:

```rust
// src/platforms/slint/player/slint_mpv_player.rs (Alternative approach)
use crate::player::traits::Player;
use libmpv2::{Mpv, Format};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

pub struct SlintMPVPlayer {
    mpv: Mpv,
    // MPV implementation details...
}

// Implementation follows the Lumiere project pattern
// with OpenGL framebuffer to Slint Image conversion
```

### Slint UI Integration

```slint
// ui/pages/player.slint
import { Button, Slider, VerticalBox, HorizontalBox } from "std-widgets.slint";

export component PlayerPage inherits Rectangle {
    // Properties for video and controls
    in property <bool> is-playing: false;
    in property <duration> position: 0ms;
    in property <duration> duration: 0ms;
    in property <image> video-frame;
    
    // Callbacks for player control
    callback play-pause();
    callback seek(duration);
    callback toggle-fullscreen();
    
    background: black;
    
    // Video display area
    video-container := Rectangle {
        width: 100%;
        height: 100%;
        
        // Video frame from MPV
        video-display := Image {
            source: video-frame;
            width: 100%;
            height: 100%;
            image-fit: contain;
        }
        
        // Player controls overlay
        controls-overlay := Rectangle {
            background: @linear-gradient(180deg, transparent 0%, rgba(0, 0, 0, 0.7) 100%);
            height: 120px;
            y: parent.height - self.height;
            
            VerticalBox {
                padding: 16px;
                spacing: 8px;
                
                // Progress bar
                progress-bar := Slider {
                    minimum: 0;
                    maximum: duration / 1ms;
                    value: position / 1ms;
                    changed => { seek(self.value * 1ms); }
                }
                
                // Control buttons
                HorizontalBox {
                    spacing: 16px;
                    alignment: center;
                    
                    play-button := Button {
                        text: is-playing ? "⏸" : "▶";
                        clicked => { play-pause(); }
                    }
                    
                    fullscreen-button := Button {
                        text: "⛶";
                        clicked => { toggle-fullscreen(); }
                    }
                }
            }
        }
    }
}
```

## Implementation Steps

### Phase 1: Foundation (Week 1) ✅ COMPLETED
1. **Create Platform Structure** ✅
   ```bash
   mkdir -p src/platforms/slint/{ui/{components,pages,adapters},player}
   ```

2. **Basic Slint App Setup** ✅
   - ✅ Implement `SlintApp` struct with basic window
   - ✅ Add dependency on `slint` crate to `Cargo.toml`
   - ✅ Create premium `.slint` files for main window

3. **Platform Integration** ✅
   - ✅ Platform detection logic for slint-only builds
   - ✅ Build system integration with `build.rs`
   - ✅ Basic event loop and window management

### Current Implementation Status

The basic Slint platform is now **fully functional** with a premium Netflix-like UI design:

#### ✅ Completed Features
- **Premium Landing Page**: Netflix-inspired design with hero section, feature highlights, and responsive layout
- **Slick UI Components**: Modern gradient backgrounds, typography, and interactive elements
- **Platform Detection**: Automatic detection of slint vs gtk features
- **Build System**: Full Slint compilation pipeline
- **Event Handling**: Callback system for navigation and user interactions
- **Responsive Design**: 1400x900 window with proper scaling

#### UI Design Features
- **Dark Theme**: Professional dark color scheme with Netflix red (#e50914) accents
- **Layered Layout**: 
  - Navigation bar with brand logo and menu items
  - Hero section with welcome messaging and primary actions
  - Features section highlighting core capabilities
  - Footer with status information
- **Interactive Elements**: TouchArea components with hover states and pointer cursors
- **Typography**: Modern font weights and sizes for visual hierarchy
- **Premium Branding**: Consistent with "premium media player experience" positioning

### Phase 2: ViewModels Integration (Week 2)
1. **Property Adapters**
   - Create adapter for each core ViewModel
   - Set up Property -> Slint property synchronization
   - Handle async property updates with `invoke_from_event_loop`

2. **Basic Pages**
   - Implement Home page with HomeViewModel
   - Implement Library page with LibraryViewModel  
   - Create reusable components (MediaCard, MediaList)

### Phase 3: Media Player (Week 3)
1. **GStreamer Integration** (Recommended)
   - Study Slint's official GStreamer example
   - Implement SlintGStreamerPlayer with AppSink
   - Set up buffer-to-Image conversion pipeline

2. **Player Implementation**
   - Build SlintGStreamerPlayer implementing Player trait
   - Set up GStreamer pipeline with video conversion
   - Implement player controls UI with Slint callbacks

3. **Alternative MPV Integration** (Advanced/Optional)
   - Study the Lumiere project for OpenGL approach
   - Implement OpenGL texture sharing if MPV is required

### Phase 4: Complete UI (Week 4)
1. **Remaining Pages**
   - Sources page with SourcesViewModel
   - Movie/Show details with DetailsViewModel
   - Authentication dialogs

2. **Polish & Testing**
   - Error handling and edge cases
   - Performance optimization
   - Platform-specific testing

## Cargo.toml Dependencies ✅ COMPLETED

The following dependencies have been added to the existing `Cargo.toml`:

```toml
[dependencies]
# Existing dependencies remain...

# Slint UI framework (✅ Added)
slint = { version = "1.5", features = ["backend-qt"], optional = true }

# GStreamer integration for video playback (✅ Added)
gstreamer = "0.24"
gstreamer-video = "0.24"
gstreamer-app = "0.24"  # Added for Slint integration
gstreamer-player = "0.24"
gstreamer-pbutils = "0.24"

# Alternative MPV integration (already present)
libmpv2 = "5.0"
libmpv2-sys = "4.0"

[build-dependencies]
# Slint build support (✅ Added)
slint-build = { version = "1.5", optional = true }

[features]
default = ["gtk"]
gtk = ["dep:gtk4", "dep:gdk4", "dep:gdk-pixbuf", "dep:libadwaita", "dep:glib-build-tools"]
slint = ["dep:slint", "dep:slint-build"]  # ✅ Added slint feature
```

### Dependency Notes
- **Backend Choice**: Currently using `backend-qt` as `backend-gl` is not available in Slint 1.5
- **GStreamer Version**: Using 0.24 to match existing project dependencies
- **Optional Dependencies**: Both slint and build dependencies are optional via feature gates

## Build Configuration ✅ COMPLETED

The existing `build.rs` has been enhanced to support Slint compilation:

```rust
// build.rs (excerpt showing Slint integration)
fn main() {
    // Only compile GTK resources when GTK feature is enabled
    #[cfg(feature = "gtk")]
    {
        compile_gtk_resources();
    }
    
    // Only compile Slint resources when Slint feature is enabled ✅ Added
    #[cfg(feature = "slint")]
    {
        compile_slint_resources();
    }
}

#[cfg(feature = "slint")]
fn compile_slint_resources() {
    println!("cargo:rerun-if-changed=src/platforms/slint/ui/slint_app.slint");
    
    slint_build::compile("src/platforms/slint/ui/slint_app.slint")
        .expect("Failed to compile Slint UI");
}
```

### Build System Features ✅
- **Feature Gating**: Only compiles Slint when `slint` feature is enabled
- **Incremental Builds**: Properly configured with `rerun-if-changed` directives
- **Error Handling**: Clear error messages for compilation failures
- **Multi-Platform**: Supports both GTK and Slint compilation in the same build system

## Testing Strategy

### Unit Tests
- Test ViewModel adapters in isolation
- Mock Slint components for ViewModel integration tests
- Test property synchronization logic

### Integration Tests
- Test complete user workflows (login, browse, play)
- Test multi-backend scenarios
- Test offline functionality

### Performance Tests
- Memory usage comparison with GTK version
- Startup time benchmarks
- Video playback performance metrics

## Platform Entry Point ✅ COMPLETED

The `main.rs` has been updated to support intelligent Slint platform detection:

```rust
// src/main.rs (excerpt showing platform detection logic)
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("reel=debug")
        .init();

    // Determine platform from environment variable or use default based on available features ✅
    let platform = std::env::var("REEL_PLATFORM")
        .unwrap_or_else(|_| {
            #[cfg(all(feature = "gtk", not(feature = "slint")))]
            { "gtk".to_string() }
            #[cfg(all(feature = "slint", not(feature = "gtk")))]
            { "slint".to_string() }  // ✅ Auto-detects slint-only builds
            #[cfg(all(feature = "gtk", feature = "slint"))]
            { "gtk".to_string() } // Default to GTK when both are available
            #[cfg(not(any(feature = "gtk", feature = "slint")))]
            { compile_error!("At least one platform feature must be enabled") }
        });
    
    info!("Starting Reel with {} frontend", platform);

    match platform.as_str() {
        #[cfg(feature = "gtk")]
        "gtk" => run_gtk_frontend().await,
        #[cfg(feature = "slint")]  // ✅ Added slint platform support
        "slint" => run_slint_frontend().await,
        _ => {
            eprintln!("Unknown platform: {}. Available platforms:", platform);
            #[cfg(feature = "gtk")]
            eprintln!("  - gtk");
            #[cfg(feature = "slint")]
            eprintln!("  - slint");
            std::process::exit(1);
        }
    }
}

#[cfg(feature = "slint")]  // ✅ Implemented
async fn run_slint_frontend() -> Result<()> {
    use platforms::slint::ReelSlintApp;
    
    info!("Initializing Slint frontend");
    
    // Initialize GStreamer for video playback
    gstreamer::init()?;
    
    // Load configuration
    let config = std::sync::Arc::new(tokio::sync::RwLock::new(
        crate::config::Config::load()?
    ));
    
    // Create and initialize the Slint app
    let mut app = ReelSlintApp::new()?;
    app.initialize(config)?;
    
    // Run the application
    let exit_code = app.run()?;
    
    std::process::exit(exit_code);
}
```

### Platform Detection Features ✅
- **Smart Default Selection**: Automatically chooses `slint` when only slint feature is enabled
- **Environment Override**: `REEL_PLATFORM=slint` environment variable support  
- **Feature-Based Compilation**: Only compiles platform code when corresponding features are enabled
- **Error Handling**: Clear messages when unsupported platforms are requested
- **GStreamer Integration**: Automatic GStreamer initialization for video playback

## Key Advantages of This Approach

### 1. Maximum Code Reuse
- **90% of existing codebase** remains unchanged
- All business logic, data access, and backend integrations work as-is
- ViewModels provide clean abstraction layer

### 2. Reactive Architecture Benefits
- Existing Property system maps well to Slint's reactive model
- Event-driven updates already implemented
- Consistent state management across platforms

### 3. Modern UI Development
- Declarative UI with `.slint` markup
- Component-based architecture
- Hot-reload during development
- Cross-platform consistency

### 4. Performance Benefits
- Native compilation
- GPU acceleration  
- Memory-efficient runtime
- Fast startup times

## Challenges and Solutions

### Challenge 1: Property System Mismatch
**Problem**: Reel uses async Property<T> with PropertySubscriber, while Slint has its own property system.

**Solution**: Adapter pattern with `invoke_from_event_loop` to bridge async updates to Slint's synchronous properties.

### Challenge 2: Video Playback Integration
**Problem**: Complex video rendering integration with media frameworks and Slint's rendering system.

**Solution**: 
- **Primary**: Use official GStreamer integration with AppSink for native video buffer handling
- **Alternative**: Use proven MPV approach from Lumiere project with OpenGL framebuffer textures

### Challenge 3: Platform Detection
**Problem**: Need to maintain both GTK and Slint platforms simultaneously.

**Solution**: Environment variable or feature flags to select platform at runtime/compile-time.

## Migration Strategy

### Gradual Migration Approach
1. **Parallel Development**: Build Slint platform alongside existing GTK
2. **Feature Parity**: Ensure Slint version matches GTK functionality
3. **A/B Testing**: Allow users to switch between platforms
4. **GTK Deprecation**: Eventually deprecate GTK once Slint is stable

### Risk Mitigation
- Keep GTK version as fallback during transition
- Extensive testing on target platforms
- Community feedback integration
- Performance monitoring and optimization

## Conclusion

Implementing a Slint platform for Reel is highly feasible due to the existing clean architecture separation between UI and business logic. The reactive ViewModel system provides an excellent foundation for Slint integration, while the proven MPV integration approach ensures video playback capabilities.

The key success factors are:
1. **Leveraging existing ViewModels** for maximum code reuse
2. **Proper adapter pattern implementation** for property synchronization  
3. **Official GStreamer integration** with AppSink for reliable video playback
4. **Gradual migration approach** to minimize risk

This implementation would provide a modern, declarative UI experience while maintaining all the robust media player functionality that Reel currently offers.

## Additional Benefits of GStreamer Approach

### Advantages over MPV Integration
- **Official Support**: Slint officially supports and maintains GStreamer integration examples
- **Native Performance**: Direct video buffer transfer without OpenGL texture conversion overhead
- **Hardware Acceleration**: Built-in support for hardware-accelerated decoding and rendering
- **Cross-Platform**: Better tested across Linux, Windows, and macOS environments
- **Simpler Implementation**: Less complex than OpenGL framebuffer approaches
- **Existing Codebase Synergy**: Reel already uses GStreamer as a fallback player, enabling code reuse

### Platform Requirements
- **Linux**: GStreamer packages available in all major distributions
- **Windows**: Official GStreamer MSI installers with full plugin support  
- **macOS**: Available via Homebrew with comprehensive codec support

The GStreamer approach aligns perfectly with Reel's existing architecture while providing the most maintainable and officially supported video integration path for the Slint platform.