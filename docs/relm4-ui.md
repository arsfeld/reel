# Relm4 UI Implementation

## Overview

This document describes the Relm4-based UI implementation for Reel, which is the PRIMARY and ONLY UI platform. The implementation fully leverages Relm4's reactive architecture using Relm4's native component state management, trackers, factories, and async patterns for a truly reactive UI.

**Status**: The Relm4 implementation is the default and only UI. The project compiles and runs successfully with major functionality working. There is no separate GTK implementation - the project uses Relm4 with GTK4 and libadwaita as the underlying toolkit.

## Implementation Status (January 2025)

### ✅ Major Achievements
- **Architecture Fixed**: MainWindow properly runs as root component via `app.run_async::<MainWindow>(db)`
- **Project Compiles**: All compilation errors resolved, builds successfully with Relm4 feature
- **Database Integration**: Proper initialization and passing to components
- **Multi-Connection Support**: Plex can handle multiple server URLs (local/remote/relay)
- **Authentication Working**: Both Plex OAuth and Jellyfin authentication functional
- **Source Management**: Complete CRUD operations for media sources
- **Real Data Loading**: Sidebar loads actual sources and libraries from database
- **Sync Functionality**: Libraries properly sync and save to database

### 🔧 Key Fixes Applied
1. **Sidebar Mock Data** → Now loads real data from database
2. **App Initialization** → Properly loads sources and libraries on startup
3. **Source Addition** → Auth dialog working for both Plex and Jellyfin
4. **Authentication** → Backend integration completed for both providers
5. **Source Operations** → Connection testing and sync wired to services
6. **Navigation** → Fixed duplication and widget parent conflicts
7. **Library Sync** → Libraries properly saved with upsert pattern
8. **Stream URLs** → Player can fetch actual stream URLs from backends

### 📊 Component Implementation (~50% Complete)
- ✅ **6 of 6 Main Pages**: Home, Library, MovieDetails, ShowDetails, Player, Sources
- ✅ **3 Workers**: ImageLoader, SearchWorker, SyncWorker
- ✅ **Authentication Dialogs**: Plex OAuth and Jellyfin login
- ✅ **Factory Components**: MediaCard, SectionRow, SourceItem
- ✅ **Player Integration**: Full OSD controls with MPV/GStreamer backends
- ✅ **Preferences Page**: Theme switching and player settings

## Architecture Overview

### Core Relm4 Patterns Used
- **Pure Relm4 Components**: Direct component state management without ViewModels
- **Tracker Pattern**: Efficient change tracking for minimal re-renders
- **Factory Pattern**: Dynamic collections with FactoryVecDeque
- **AsyncComponent**: Data-heavy pages with loading states
- **Worker Pattern**: Background tasks for sync, search, and media operations
- **Command Pattern**: Structured async operations with proper lifecycle
- **MessageBroker**: Inter-component communication via Relm4's broker system

## Current Implementation Structure

### Directory Structure
```
src/
├── app/                      # Application root
│   ├── mod.rs               # Platform module exports  
│   └── app.rs               # ✅ Relm4 application initialization
├── ui/                      # All UI components
│   ├── main_window.rs       # ✅ Root window with per-pane HeaderBars
│   ├── sidebar.rs           # ✅ Sidebar with real data loading
│   ├── dialogs/
│   │   ├── auth_dialog.rs   # ✅ Plex OAuth and Jellyfin auth working
│   │   └── preferences_dialog.rs # ✅ Preferences dialog
│   ├── pages/               # Page components (AsyncComponent)
│   │   ├── home.rs          # ✅ Home page with media sections
│   │   ├── library.rs       # ✅ Library with virtual scrolling
│   │   ├── movie_details.rs # ✅ Movie details with metadata
│   │   ├── show_details.rs  # ✅ Show details with episodes
│   │   ├── player.rs        # ✅ Player with full OSD controls
│   │   ├── sources.rs       # ✅ Sources management working
│   │   └── preferences.rs   # ✅ Preferences page implemented
│   ├── factories/           # Factory components
│   │   ├── media_card.rs    # ✅ Media card with hover/progress
│   │   ├── source_item.rs   # ✅ Source list item factory
│   │   └── section_row.rs   # ✅ Home section factory
│   └── shared/              # Shared utilities
│       ├── messages.rs      # ✅ Navigation and data messages
│       ├── commands.rs      # ✅ 24+ async commands
│       └── broker.rs        # ✅ MessageBroker setup
├── workers/                 # Background workers
│   ├── connection_monitor.rs # ✅ Connection health monitoring
│   ├── sync_worker.rs       # ✅ Sync with BackendService
│   ├── search_worker.rs     # ✅ Tantivy search indexing
│   └── image_loader.rs      # ✅ LRU + disk cache
└── services/                # Service layer
    ├── core/                # Core services
    │   ├── auth.rs          # ✅ Authentication service
    │   ├── backend.rs       # ✅ Stateless backend operations
    │   ├── connection.rs    # ✅ Multi-URL connection selection
    │   ├── media.rs         # ✅ Media operations
    │   └── sync.rs          # ✅ Sync operations
    ├── brokers/             # MessageBroker implementations
    │   ├── connection_broker.rs # ✅ Connection events
    │   ├── media_broker.rs  # ✅ Media events
    │   └── sync_broker.rs   # ✅ Sync events
    └── commands/            # Command implementations
        ├── media_commands.rs # ✅ 14 media commands
        ├── auth_commands.rs  # ✅ 8 auth commands
        └── sync_commands.rs  # ✅ 2 sync commands
```

## Architecture Highlights

### 🚀 Stateless Backend Architecture
The implementation uses pure stateless functions instead of persistent backend instances:
- Backends created on-demand per request
- All state loaded from database/keyring as needed
- No global state or thread-local storage
- Follows Relm4's stateless principles

### 🔄 Multi-Connection Support (Plex)
Complete implementation of Plex's multi-server, multi-connection architecture:
- One Plex account can access multiple servers
- Each server has multiple connection URLs (local/remote/relay)
- Automatic best connection selection based on network
- ConnectionMonitor worker switches connections dynamically
- All connections stored in database JSON column

### 📦 Command Pattern Implementation
24+ commands covering all async operations:
- **Media Commands**: GetLibraries, GetMediaItems, GetItemDetails, etc.
- **Auth Commands**: CreateSource, TestConnection, RefreshToken
- **Sync Commands**: SyncSource, SyncLibrary
- All commands use stateless services with explicit dependencies

### 🎮 Player Integration
Full player implementation with professional controls:
- MPV and GStreamer backend support
- Channel-based PlayerController for thread safety
- Full OSD overlay with auto-hide controls
- Keyboard shortcuts (Space, F11, ESC)
- Window chrome hiding for immersive experience
- Aspect ratio resizing

## Implementation Details

### Key Implementation Notes

1. **Application Structure**
   - The app runs via `AppPlatform::run_relm4()` in `main.rs`
   - Uses Relm4 with GTK4 and libadwaita as the underlying toolkit
   - No separate platform implementations - Relm4 is the only UI
   - MessageBroker for component communication

2. **Core Components**
   - `ReelApp` root AsyncComponent with GTK application initialization
   - Global MessageBroker for inter-component communication
   - Direct service connections without ViewModels
   - Command handler for async operations

3. **Main Window Component**
   - Implemented as AsyncComponent with loading states
   - Two-pane layout with AdwNavigationSplitView
   - Navigation stack management
   - Content area with page switching
   - Window state tracking with tracker

4. **Sidebar Component**
   - Uses `#[tracker::track]` for efficient updates
   - Factory pattern for source list items
   - Home/Sources navigation buttons
   - Real-time connection status updates
   - Efficient re-renders only on state changes

## Component Architecture

### AsyncComponent Pattern (for data-heavy pages)
```rust
#[derive(Debug)]
#[tracker::track]
pub struct HomePage {
    #[no_eq]
    sections: Vec<Section>,
    loading: bool,
    tracker: u8,
}

#[derive(Debug)]
pub enum HomeMsg {
    LoadSections,
    SectionsLoaded(Vec<Section>),
    RefreshSection(usize),
}

#[relm4::component(async)]
impl AsyncComponent for HomePage {
    type Init = ();
    type Input = HomeMsg;
    type Output = NavigationMsg;
    type CommandOutput = CmdMsg;

    view! {
        gtk::Box {
            #[track(self.changed(HomePage::loading()))]
            set_visible: !self.loading,
            
            gtk::ScrolledWindow {
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    
                    #[local_ref]
                    section_factory -> gtk::Box {},
                }
            }
        }
    }

    async fn init_loading_widgets(root: &mut Self::Root) -> Option<LoadingWidgets> {
        Some(LoadingWidgets {
            spinner: gtk::Spinner { set_spinning: true },
        })
    }

    async fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let model = HomePage {
            sections: vec![],
            loading: true,
            tracker: 0,
        };
        
        // Initialize factory for sections
        let mut section_factory = FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), |msg| match msg {
                SectionOutput::MediaSelected(id) => HomeMsg::Navigate(id),
            });
        
        // Send command to load data
        sender.oneshot_command(async {
            CmdMsg::LoadSections
        });
        
        AsyncComponentParts { model, widgets }
    }

    async fn update_cmd(&mut self, msg: Self::CommandOutput, sender: AsyncComponentSender<Self>) {
        match msg {
            CmdMsg::LoadSections => {
                let sections = load_sections_from_service().await;
                sender.input(HomeMsg::SectionsLoaded(sections));
            }
        }
    }
}
```

### Factory Pattern (for collections)
```rust
#[derive(Debug)]
#[tracker::track]
pub struct MediaCard {
    media: MediaItem,
    progress: f32,
    hover: bool,
    tracker: u8,
}

#[relm4::factory]
impl FactoryComponent for MediaCard {
    type Init = MediaItem;
    type Input = MediaCardMsg;
    type Output = MediaCardOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::FlowBox;

    view! {
        gtk::Box {
            add_css_class: "media-card",
            
            #[track(self.changed(MediaCard::hover()))]
            add_css_class: if self.hover { "hover" } else { "" },
            
            gtk::Overlay {
                gtk::Picture {
                    set_filename: Some(&self.media.poster_path),
                },
                
                #[track(self.changed(MediaCard::progress()))]
                add_overlay = &gtk::ProgressBar {
                    set_fraction: self.progress as f64,
                    set_visible: self.progress > 0.0,
                }
            }
        }
    }
}
```

### Worker Pattern (for background tasks)
```rust
impl Worker for ImageLoader {
    type Init = ();
    type Input = ImageLoadRequest;
    type Output = ImageLoadResult;

    fn init(_init: Self::Init, sender: ComponentSender<Self>) -> Self {
        ImageLoader { cache: HashMap::new() }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            ImageLoadRequest { url, target_id } => {
                // Load image in background
                let image = fetch_and_cache_image(&url);
                sender.output(ImageLoadResult { target_id, image });
            }
        }
    }
}
```

### State Integration Strategy

1. **Direct Service Integration**: Components directly call DataService/SyncManager
2. **MessageBroker for Events**: Replace custom event bus with MessageBroker
3. **Commands for Async**: All async operations use command pattern
4. **Workers for Background**: Heavy operations run in worker components

### Key Components

#### MainWindow (AsyncComponent)
- **Purpose**: Root application window with navigation
- **Pattern**: AsyncComponent with tracker for window state
- **Children**: Sidebar, dynamic page components
- **State**: Navigation stack, window size, current page
- **Commands**: Page transitions, window state persistence

#### Sidebar (Component with Tracker)
- **Purpose**: Navigation and source management
- **Pattern**: Component with tracker for efficient updates
- **Factory**: SourceItem factory for dynamic source list
- **State**: Sources list, connection status, selected library
- **Tracker fields**: Connection status, selected item

#### HomePage (AsyncComponent)
- **Purpose**: Landing page with media sections
- **Pattern**: AsyncComponent with loading states
- **Factory**: SectionRow factory for each section
- **Commands**: Load sections, refresh data
- **Workers**: ImageLoader for posters

#### Library (AsyncComponent with Virtual Factory)
- **Purpose**: Browse full media library
- **Pattern**: AsyncComponent + Virtual FactoryVecDeque
- **Factory**: MediaCard factory with virtual scrolling
- **Workers**: SearchWorker for filtering
- **Tracker**: View mode, sort order, filters

#### Player (Component with Commands)
- **Purpose**: Media playback interface
- **Pattern**: Component with command-based playback
- **Commands**: Play, pause, seek, volume
- **Workers**: PlaybackTracker for progress
- **Tracker**: Playback state, fullscreen, controls visibility

## Data Flow

### Message Flow (Pure Relm4)
```
User Input → Factory/Component → AsyncComponent → Commands → Services
                ↓                      ↓              ↓
            Tracker Update      Worker Messages   Database
                ↓                      ↓              ↓
            Efficient Render    Background Task   Cache Update
```

### Component Communication
```
Component A → MessageBroker → Component B
     ↓            ↓              ↓
  Output Msg   Broadcast    Input Handler
```

### Async Operations
```
Component → Command → Async Task → CommandOutput → State Update → Tracker → View
```

## Benefits of Relm4 Implementation

### Developer Experience
- **Declarative UI**: More maintainable than manual GTK setup
- **Type Safety**: Compile-time UI validation
- **Hot Reload**: Faster development iteration
- **Component Reuse**: Better modularity

### Architecture Benefits
- **Clear Data Flow**: Unidirectional message flow
- **Testable Components**: Isolated component logic
- **Reactive Updates**: Automatic UI updates from state changes
- **Modular Design**: Easy to add/remove features

### Performance Benefits
- **Optimized Updates**: Only re-render changed components
- **Memory Efficiency**: Automatic cleanup of component resources
- **Lazy Loading**: Components created only when needed

## Known Issues & Next Steps

### 🟡 Remaining Gaps
1. **Media Library Display**: Needs data loading implementation
2. **Playback Progress**: Database sync not yet implemented
3. **Auto-play**: Next episode functionality incomplete
4. **Trending Section**: Returns empty list (low priority)
5. **Error Handling**: Some unwrap() calls need proper handling

### ✅ Recent Fixes (January 2025)
- Fixed all compilation errors (54 resolved)
- Implemented Plex OAuth with PIN flow
- Added Jellyfin authentication
- Fixed navigation duplication issues
- Resolved library sync saving
- Connected sync and test operations to backends
- Fixed stream URL fetching for playback

### 🎯 Priority Roadmap
1. **Immediate**: Complete media library data loading
2. **High**: Implement playback progress tracking
3. **Medium**: Add auto-play for TV episodes
4. **Low**: Polish error handling and edge cases

## Development Guidelines

**Status**: Relm4 is the ONLY UI implementation.
- All development uses pure Relm4 components
- No ViewModel pattern - components manage their own state
- UI/UX follows GNOME HIG with Adwaita styling

### Testing Strategy
- Component unit tests
- Integration tests for data flow
- UI automation tests
- Performance benchmarks

## Technical Considerations

### Dependencies
```toml
[dependencies]
relm4 = { version = "0.9", features = ["macros", "libadwaita"] }
relm4-components = "0.9"
relm4-icons = "0.9"
tracker = "0.2"
async-trait = "0.1"
```

### Integration Points
- **Services**: Direct integration with service layer
- **Models**: All data models reused without modification
- **Database**: Direct repository access from components
- **Workers**: Background tasks for sync, search, and media operations
- **No ViewModels**: Components manage their own state with trackers

### Performance Targets
- **Startup Time**: < 500ms to first render
- **Navigation**: < 100ms page transitions  
- **Memory**: < 200MB for typical library sizes
- **Scroll Performance**: 60fps in large lists

## Success Metrics

### Functionality
- [ ] All current features implemented
- [ ] Feature parity with GTK version
- [ ] No regressions in user workflows

### Performance
- [ ] Startup time competitive with GTK version
- [ ] Smooth 60fps scrolling and animations
- [ ] Memory usage within 20% of GTK version

### Code Quality
- [ ] >90% test coverage for components
- [ ] Clear component boundaries
- [ ] Minimal code duplication

### Developer Experience
- [ ] Faster development of new features
- [ ] Easier UI debugging and testing
- [ ] Better component reusability

## Key Architectural Improvements

### Tracker Pattern Benefits
- **Minimal Re-renders**: Only update changed UI elements
- **Automatic Change Detection**: No manual property notifications
- **Type-safe Updates**: Compiler ensures correct field access
- **Performance**: O(1) change detection vs O(n) diffing

### Factory Pattern Advantages
- **Virtual Scrolling**: Built-in support for large lists
- **Efficient Updates**: RAII guards batch changes
- **Memory Management**: Automatic cleanup of unused items
- **Reusability**: Same factory across different contexts

### AsyncComponent Benefits
- **Loading States**: Built-in skeleton UI support
- **Command Pattern**: Structured async operations
- **Error Handling**: Centralized error management
- **Cancellation**: Automatic cleanup on component destroy

### Worker Pattern Advantages
- **Thread Isolation**: Heavy tasks don't block UI
- **Message Passing**: Type-safe communication
- **Resource Management**: Automatic worker lifecycle
- **Scalability**: Easy to add more workers

## Testing Strategy

### Component Testing
```rust
#[cfg(test)]
mod tests {
    use relm4::ComponentTest;
    
    #[tokio::test]
    async fn test_home_page_loading() {
        let app = HomePage::builder()
            .launch_test()
            .await;
        
        // Test loading state
        assert!(app.model().loading);
        
        // Send loaded message
        app.send(HomeMsg::SectionsLoaded(vec![]));
        
        // Verify state change
        assert!(!app.model().loading);
    }
}
```

### Factory Testing
```rust
#[test]
fn test_media_card_factory() {
    let factory = MediaCard::builder()
        .launch_test(media_item);
    
    // Test hover state
    factory.send(MediaCardMsg::MouseEnter);
    assert!(factory.model().hover);
}
```



## Future Considerations

### Platform Expansion
- Easier to add new platforms (Windows, macOS native)
- Component reuse across platforms
- Shared design system

### Feature Development
- Faster UI iteration with hot reload
- Better testability with component isolation
- Component library growth

### Maintenance
- Reduced UI-related bugs through type safety
- Clearer component responsibilities
- Easier refactoring with tracker pattern
- Better performance monitoring

---

## Summary

The Relm4 implementation fully leverages modern reactive patterns:
- **AsyncComponents** for data-heavy pages with built-in loading states
- **Tracker pattern** for efficient, minimal UI updates
- **Factory pattern** for all collections and lists
- **Worker components** for background operations
- **Command pattern** for structured async operations
- **MessageBroker** for clean inter-component communication

The implementation provides a performant, maintainable, and truly reactive UI that leverages Relm4's strengths. There is no separate GTK implementation - Relm4 with GTK4/libadwaita is the sole UI framework.