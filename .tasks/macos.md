# macOS Implementation Plan: objc2 Platform

## Overview

This document outlines the comprehensive plan to implement a modern, native macOS UI for Reel using the objc2 Rust framework, replacing the current GTK4/libadwaita implementation while preserving all core business logic and reactive architecture.

## Current Architecture Analysis

### GTK Platform Structure
- **ReelApp**: Main application entry point with GTK4/libadwaita
- **MainWindow**: Central window management with navigation
- **Pages**: Modular UI views (Home, Library, Player, Sources, MovieDetails, ShowDetails)
- **Widgets**: Reusable components (MediaCard, VirtualMediaList, PlayerOverlay)
- **Reactive System**: Property-based UI updates with ViewModels
- **Navigation**: Stack-based view management

### Core Architecture (Platform-Agnostic)
The following components will remain unchanged:
- **ViewModels**: `LibraryViewModel`, `HomeViewModel`, `PlayerViewModel`, etc.
- **Properties**: Reactive property system with change notifications
- **Event System**: EventBus for system-wide communication
- **Data Layer**: Database, repositories, services, and sync logic
- **Business Logic**: Media backends (Plex, Jellyfin), authentication, and coordination

## Component Mapping: GTK to macOS

| GTK4/libadwaita Component | macOS AppKit Equivalent | objc2 Implementation |
|--------------------------|-------------------------|---------------------|
| `adw::Application` | `NSApplication` | `objc2_app_kit::NSApplication` |
| `adw::ApplicationWindow` | `NSWindow` | `objc2_app_kit::NSWindow` |
| `gtk4::Box` | `NSStackView` | `objc2_app_kit::NSStackView` |
| `gtk4::FlowBox` | `NSCollectionView` | `objc2_app_kit::NSCollectionView` |
| `gtk4::ScrolledWindow` | `NSScrollView` | `objc2_app_kit::NSScrollView` |
| `gtk4::Picture` | `NSImageView` | `objc2_app_kit::NSImageView` |
| `gtk4::Button` | `NSButton` | `objc2_app_kit::NSButton` |
| `gtk4::Label` | `NSTextField` | `objc2_app_kit::NSTextField` |
| `gtk4::SearchEntry` | `NSSearchField` | `objc2_app_kit::NSSearchField` |
| `gtk4::Stack` | `NSTabView` (tabless) | `objc2_app_kit::NSTabView` |
| `gtk4::Overlay` | `NSView` with subviews | Manual view hierarchy |
| `adw::StatusPage` | Custom `NSView` | Custom view composition |

## Implementation Phases

### Phase 1: Core Framework (4-5 weeks)

#### Week 1-2: Application Foundation
- **Dependencies Setup**
  ```toml
  [dependencies]
  objc2 = "0.5"
  objc2-app-kit = "0.2"
  objc2-foundation = "0.2"
  metal = "0.28"  # For video rendering
  block2 = "0.5"  # For Objective-C blocks
  dispatch2 = "0.2"  # For GCD integration
  ```

- **Basic Application Structure**
  - `src/platforms/macos/mod.rs` - Platform module
  - `src/platforms/macos/app.rs` - NSApplication wrapper
  - `src/platforms/macos/frontend.rs` - Frontend trait implementation
  - Application delegate with lifecycle management
  - Menu bar setup with standard macOS menus

#### Week 3: Window Management
- **Main Window Implementation**
  - `src/platforms/macos/ui/main_window.rs`
  - NSWindow with proper sizing and positioning
  - Window delegate for lifecycle events
  - Toolbar and title bar configuration
  - Split view controller for sidebar/content layout

#### Week 4-5: Navigation System
- **Navigation Framework**
  - `src/platforms/macos/ui/navigation/mod.rs`
  - Custom navigation controller pattern
  - View lifecycle management (viewDidLoad, viewWillAppear, etc.)
  - Back/forward navigation with history
  - State preservation and restoration
  - Integration with existing NavigationRequest system

### Phase 2: Basic UI Components (2-3 weeks)

#### Week 6: Foundation Components
- **Basic UI Elements**
  - `src/platforms/macos/ui/components/` directory structure
  - Label, button, and text field wrappers
  - NSStackView layouts with Auto Layout
  - Search field with debounced input
  - Loading spinners and progress indicators

#### Week 7-8: Layout and Styling
- **Auto Layout Integration**
  - Constraint-based layouts replacing GTK box model
  - Responsive design for different window sizes
  - Dark mode support following system appearance
  - Custom styling for media application aesthetic

### Phase 3: Media UI Implementation (6-7 weeks)

#### Week 9-10: Collection Views
- **Media Collection Framework**
  - `src/platforms/macos/ui/widgets/media_collection.rs`
  - NSCollectionView with custom layout
  - NSCollectionViewItem for media cards
  - Virtual scrolling for large libraries
  - Dynamic sizing based on window width

#### Week 11-12: Media Cards
- **Media Card Components**
  - `src/platforms/macos/ui/widgets/media_card.rs`
  - Custom NSView-based media cards
  - Image loading with placeholder states
  - Progress indicators and watch status
  - Hover effects and click handling
  - Integration with existing ImageLoader

#### Week 13-14: Library Pages
- **Library View Implementation**
  - `src/platforms/macos/ui/pages/library.rs`
  - Integration with LibraryViewModel
  - Reactive binding to filtered_items property
  - Search functionality with NSSearchField
  - Filter controls (watch status, sort order)
  - Loading/empty/content states

#### Week 15: Home Page
- **Home View Implementation**
  - `src/platforms/macos/ui/pages/home.rs`
  - Horizontal scrolling sections
  - Integration with HomeViewModel
  - Section management and dynamic content
  - Continue watching and recommendations

### Phase 4: Video Integration (3-4 weeks)

#### Week 16-17: MPV Player Integration
- **Video Player Foundation**
  - `src/platforms/macos/ui/pages/player.rs`
  - NSView hosting for MPV rendering
  - Metal/OpenGL context management
  - Player factory integration (unchanged)
  - Basic playback controls

#### Week 18: Video Controls
- **Player Overlay Implementation**
  - `src/platforms/macos/ui/widgets/player_overlay.rs`
  - Custom control overlay with fade animations
  - Progress scrubbing and time display
  - Volume and playback rate controls
  - Subtitle and audio track selection

#### Week 19: Fullscreen Support
- **Fullscreen Video Experience**
  - Fullscreen window management
  - Menu bar and dock hiding
  - Cursor auto-hide functionality
  - Keyboard shortcuts (space, escape, arrow keys)

### Phase 5: Additional Pages and Features (2-3 weeks)

#### Week 20: Details Pages
- **Media Details Implementation**
  - `src/platforms/macos/ui/pages/movie_details.rs`
  - `src/platforms/macos/ui/pages/show_details.rs`
  - Cast and crew information display
  - Episode listings for shows
  - Watch/unwatched controls

#### Week 21: Sources and Settings
- **Configuration UI**
  - `src/platforms/macos/ui/pages/sources.rs`
  - Server connection management
  - Authentication flows
  - Preferences window with NSTabView
  - Settings persistence

#### Week 22: Polish and Platform Integration
- **macOS Platform Features**
  - Dock badge updates
  - System notification integration
  - Media keys support
  - Touch Bar support (if applicable)
  - App Store preparation (code signing, etc.)

## Technical Implementation Details

### Reactive Property Binding

```rust
// Example: Binding NSTextField to reactive property
impl MacOSLibraryView {
    fn setup_reactive_search(&self, view_model: Arc<LibraryViewModel>) {
        let search_field = self.search_field.clone();
        let search_property = view_model.search_query();
        
        // Two-way binding
        let mut subscriber = search_property.subscribe();
        glib::spawn_future_local(async move {
            while subscriber.wait_for_change().await {
                let query = search_property.get_sync();
                search_field.set_string_value(&NSString::from_str(&query));
            }
        });
        
        // Handle user input
        search_field.set_target_action(/* ... */);
    }
}
```

### Collection View Integration

```rust
// Example: NSCollectionView with reactive data source
impl MediaCollectionView {
    fn setup_reactive_binding(&self, items_property: Property<Vec<MediaItem>>) {
        let collection_view = self.collection_view.clone();
        let mut subscriber = items_property.subscribe();
        
        glib::spawn_future_local(async move {
            while subscriber.wait_for_change().await {
                let items = items_property.get_sync();
                // Update collection view data source
                collection_view.reload_data();
            }
        });
    }
}
```

### Video Player Integration

```rust
// Example: MPV integration with NSView
impl MacOSPlayerView {
    fn setup_mpv_rendering(&self) {
        let render_view = NSView::new();
        // Configure Metal/OpenGL context
        // Attach MPV player output to view
        self.add_subview(&render_view);
    }
}
```

## Key Technical Challenges and Solutions

### 1. Memory Management
**Challenge**: objc2 object lifetime management
**Solution**: Use `Retained<T>` for owned objects, `&T` for borrowed references, proper weak reference patterns

### 2. Threading Model
**Challenge**: UI updates must be on main thread
**Solution**: Use `dispatch::main_queue()` for UI updates, maintain async data loading on background threads

### 3. Video Rendering
**Challenge**: Integrating C-based MPV with AppKit
**Solution**: Use NSView hosting with proper context management, Metal for hardware acceleration

### 4. Layout System Translation
**Challenge**: Converting GTK box model to Auto Layout
**Solution**: Create constraint-based layout helpers, responsive design patterns

### 5. Property Binding
**Challenge**: Implementing reactive patterns without built-in data binding
**Solution**: Use KVO-style observation patterns, manual binding helpers

## File Structure

```
src/platforms/macos/
├── mod.rs                          # Platform module exports
├── app.rs                          # NSApplication wrapper
├── frontend.rs                     # Frontend trait implementation
├── platform_utils.rs              # macOS-specific utilities
└── ui/
    ├── mod.rs                      # UI module exports
    ├── main_window.rs              # Main application window
    ├── navigation/
    │   ├── mod.rs                  # Navigation system
    │   ├── manager.rs              # Navigation controller
    │   └── types.rs                # Navigation types
    ├── components/
    │   ├── mod.rs                  # Basic UI components
    │   ├── label.rs                # NSTextField wrapper
    │   ├── button.rs               # NSButton wrapper
    │   └── search_field.rs         # NSSearchField wrapper
    ├── widgets/
    │   ├── mod.rs                  # Complex widgets
    │   ├── media_card.rs           # Media card component
    │   ├── media_collection.rs     # Collection view
    │   └── player_overlay.rs       # Video controls
    └── pages/
        ├── mod.rs                  # Page components
        ├── home.rs                 # Home page
        ├── library.rs              # Library browsing
        ├── player.rs               # Video player
        ├── movie_details.rs        # Movie details
        ├── show_details.rs         # Show details
        └── sources.rs              # Source management
```

## Dependencies and Requirements

### Required Crates
```toml
[dependencies.macos]
objc2 = "0.5"
objc2-app-kit = "0.2"
objc2-foundation = "0.2"
metal = "0.28"
block2 = "0.5"
dispatch2 = "0.2"
core-graphics = "0.23"
```

### Development Environment
- macOS 12.0+ (for latest AppKit features)
- Xcode 14+ (for Metal and modern APIs)
- Rust 1.71+ (required by objc2)

### Runtime Requirements
- macOS 10.15+ (minimum deployment target)
- Metal-capable hardware (for video acceleration)

## Testing Strategy

### Unit Testing
- Mock AppKit components for ViewModel testing
- Property binding verification
- Navigation flow testing

### Integration Testing
- Full application lifecycle testing
- Video playback testing
- Memory leak detection

### Manual Testing
- Cross-device compatibility
- Performance testing with large libraries
- Accessibility compliance

## Success Metrics

### Functional Requirements
- ✅ Feature parity with GTK implementation
- ✅ Native macOS look and feel
- ✅ Smooth video playback (60fps)
- ✅ Responsive UI (< 16ms frame time)
- ✅ Memory efficiency (< 200MB base usage)

### Performance Targets
- Library loading: < 100ms for cached content
- Image loading: < 200ms for medium-sized posters
- Video startup: < 1 second
- Search responsiveness: < 50ms keystroke to UI update

## Risk Assessment

### High Risk
- **Video integration complexity**: MPV + AppKit integration
- **Memory management**: objc2 lifetime issues
- **Performance**: Collection view with large datasets

### Medium Risk
- **Layout system**: Auto Layout learning curve
- **Threading**: Main thread UI update coordination
- **Platform APIs**: AppKit API familiarity

### Mitigation Strategies
- Early prototyping of video integration
- Incremental development with frequent testing
- Regular memory profiling
- Fallback to simpler implementations if needed

## Timeline Summary

**Total Duration**: 22 weeks (5.5 months)

| Phase | Duration | Key Deliverables |
|-------|----------|-----------------|
| Core Framework | 5 weeks | Application structure, window management, navigation |
| Basic UI | 3 weeks | Components, layouts, styling |
| Media UI | 7 weeks | Collection views, media cards, library pages |
| Video Integration | 4 weeks | MPV player, controls, fullscreen |
| Polish & Features | 3 weeks | Details pages, settings, platform integration |

**Recommended Approach**: Start with Phase 1 to validate technical feasibility, then proceed incrementally with regular milestone reviews.