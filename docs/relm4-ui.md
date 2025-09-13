# Relm4 UI Implementation Plan

## Overview

This document outlines a plan to create a parallel Relm4-based UI implementation for Reel alongside the existing GTK4/libadwaita implementation. The goal is to fully leverage Relm4's reactive architecture, abandoning the ViewModel pattern in favor of Relm4's native component state management, trackers, factories, and async patterns for a truly reactive UI.

## Current GTK4 Architecture Analysis

### Current Structure
```
src/platforms/gtk/
├── app.rs                    # GTK application initialization  
├── mod.rs                    # Platform module exports
├── platform_utils.rs        # Platform-specific utilities
└── ui/
    ├── main_window.rs        # Main window with sidebar/content split
    ├── auth_dialog.rs        # Authentication dialog
    ├── preferences_window.rs # Preferences window
    ├── filters.rs           # Filter types and utilities
    ├── pages/               # Content pages
    │   ├── home.rs          # Home page with sections
    │   ├── library.rs       # Library grid view
    │   ├── library_virtual.rs # Virtual scrolling library
    │   ├── movie_details.rs # Movie details page
    │   ├── show_details.rs  # Show details page
    │   ├── player.rs        # Video player page
    │   └── sources.rs       # Sources management
    ├── widgets/             # Reusable components
    │   ├── player_overlay.rs
    │   ├── virtual_media_list.rs
    │   └── mod.rs
    ├── viewmodels/          # UI state management
    │   └── mod.rs           # Re-exports core ViewModels
    ├── navigation/          # Navigation management
    └── reactive/            # Property bindings
```

### Current Patterns (To Be Replaced)
- **Hybrid Architecture**: Mix of direct GTK usage and reactive ViewModels
- **Custom Property System**: Manual reactive properties with subscribers
- **Template-based UI**: GTK composite templates with Blueprint files
- **Manual Event Handling**: Explicit signal connections
- **State Management**: Shared Arc<AppState> with service layer

### New Relm4 Patterns
- **Pure Relm4 Components**: No ViewModels, direct component state management
- **Tracker Pattern**: Efficient change tracking for minimal re-renders
- **Factory Pattern**: Dynamic collections with FactoryVecDeque
- **AsyncComponent**: Data-heavy pages with loading states
- **Worker Pattern**: Background tasks for sync, search, and media operations
- **Command Pattern**: Structured async operations with proper lifecycle
- **MessageBroker**: Inter-component communication without custom event bus

## Proposed Relm4 Architecture

### Directory Structure
```
src/platforms/relm4/
├── app.rs                    # Relm4 application root component
├── mod.rs                    # Platform module exports
├── platform_utils.rs        # Shared platform utilities
└── components/
    ├── main_window.rs        # Root window component (AsyncComponent)
    ├── sidebar.rs            # Sidebar component with tracker
    ├── dialogs/
    │   ├── auth_dialog.rs    # Authentication dialog component
    │   └── preferences.rs    # Preferences dialog component
    ├── pages/               # Page components (AsyncComponent)
    │   ├── home.rs          # Home page with factory sections
    │   ├── library.rs       # Library page with virtual factory
    │   ├── movie_details.rs # Movie details async component
    │   ├── show_details.rs  # Show details with episode factory
    │   ├── player.rs        # Player component with commands
    │   └── sources.rs       # Sources management component
    ├── factories/           # Factory components for collections
    │   ├── media_card.rs    # Media card factory component
    │   ├── episode_item.rs  # Episode item factory
    │   ├── source_item.rs   # Source list item factory
    │   └── section_row.rs   # Home section row factory
    ├── workers/             # Background worker components
    │   ├── sync_worker.rs   # Sync operations worker
    │   ├── search_worker.rs # Search indexing worker
    │   ├── image_loader.rs  # Image loading worker
    │   └── playback_tracker.rs # Progress tracking worker
    ├── widgets/             # Reusable widget components
    │   ├── player_controls.rs # Player control overlay
    │   ├── loading_spinner.rs # Loading state widget
    │   └── mod.rs
    └── shared/              # Shared component utilities
        ├── messages.rs      # Common message types
        ├── commands.rs      # Async command definitions
        ├── broker.rs        # MessageBroker setup
        └── navigation.rs    # Navigation message routing
```

## Implementation Phases

### Phase 1: Foundation (Week 1-2)
**Goal**: Basic Relm4 app structure with main window
**Success Criteria**: App launches with sidebar and content area
**Tests**: Window displays, sidebar shows source list

#### Tasks:
1. **Create Relm4 platform module**
   - Add `src/platforms/relm4/mod.rs`
   - Add Relm4 dependencies to `Cargo.toml`
     - `relm4 = "0.9"`
     - `relm4-components = "0.9"`
     - `relm4-icons = "0.9"`
     - `tracker = "0.2"`
   - Create platform detection in main.rs
   - Set up MessageBroker for component communication

2. **Implement root app component**
   - Create `ReelApp` root AsyncComponent
   - Handle GTK application initialization
   - Set up global MessageBroker
   - Initialize service connections (DataService, SyncManager)
   - Set up command handler for async operations

3. **Build main window component**
   - Implement as AsyncComponent with loading states
   - Two-pane layout with AdwNavigationSplitView
   - Navigation stack management
   - Content area with page switching
   - Window state tracking with tracker

4. **Create sidebar component with tracker**
   - Implement with `#[tracker::track]` for efficient updates
   - Factory pattern for source list items
   - Home/Sources navigation buttons
   - Real-time connection status updates
   - Efficient re-renders only on state changes

### Phase 2: Core Pages (Week 3-4)
**Goal**: Home and Library pages working
**Success Criteria**: Can browse libraries and view media
**Tests**: Navigation works, media displays correctly

#### Tasks:
1. **Implement home page as AsyncComponent**
   - Use AsyncComponent for data loading
   - Implement `init_loading_widgets()` for skeleton UI
   - Factory pattern for each section (Continue Watching, Recently Added, etc.)
   - Horizontal scrolling with FactoryVecDeque
   - Lazy loading of section content
   - Commands for fetching section data

2. **Build library page with virtual factory**
   - AsyncComponent with loading states
   - FactoryVecDeque for media grid
   - Virtual scrolling using factory pattern
   - Tracker for filter/sort state changes
   - Grid/list view toggle with efficient re-render
   - Search worker for filtering

3. **Create media card factory component**
   - Implement as FactoryComponent
   - Image loading via worker
   - Progress tracking with tracker
   - Hover states and animations
   - Output messages for selection

4. **Set up Workers for background tasks**
   - ImageLoader worker for async image fetching
   - SearchWorker for library filtering
   - SyncWorker for background data updates
   - Connect workers to MessageBroker

### Phase 3: Details & Player (Week 5-6)
**Goal**: Full media browsing and playback
**Success Criteria**: Can play movies/shows end-to-end
**Tests**: Details load correctly, video playback works

#### Tasks:
1. **Movie details as AsyncComponent**
   - AsyncComponent with loading skeleton
   - Commands for fetching full metadata
   - Factory for cast/crew list
   - Tracker for play state
   - Related content with lazy loading
   - Background blur effect for poster

2. **Show details with episode factory**
   - AsyncComponent for show data
   - FactoryVecDeque for season selector
   - FactoryVecDeque for episode grid
   - Tracker for watched episodes
   - Continue watching integration
   - Efficient updates on episode state changes

3. **Player component with commands**
   - Commands for playback control
   - MPV integration via commands
   - Fullscreen state with tracker
   - Progress tracking worker
   - OSD auto-hide with timers
   - Subtitle/audio track selection

4. **Player controls with tracker**
   - Tracker for control visibility
   - Seek bar with preview
   - Volume with gesture support
   - Playback speed controls
   - Chapter navigation

### Phase 4: Management & Polish (Week 7-8)
**Goal**: Complete feature parity
**Success Criteria**: All features from GTK implementation work
**Tests**: Full user workflows pass

#### Tasks:
1. **Sources management component**
   - Add/remove sources
   - Authentication flow
   - Source testing
   - Settings management

2. **Authentication dialog**
   - Server type selection
   - Credential input
   - OAuth flow handling
   - Error states

3. **Preferences dialog**
   - Theme selection
   - Player preferences
   - Library settings
   - Data management

4. **Polish and optimization**
   - Performance tuning
   - Error handling
   - Loading states
   - Accessibility

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

## Migration Strategy

### Parallel Development
- Keep existing GTK implementation fully functional
- Develop Relm4 implementation alongside
- Share core services and business logic
- Use feature flags for platform selection

### Gradual Adoption
- Start with new features in Relm4
- Migrate existing features incrementally
- Maintain UI/UX consistency
- Validate performance characteristics

### Testing Strategy
- Component unit tests
- Integration tests for data flow
- UI automation tests
- Performance benchmarks

## Technical Considerations

### Dependencies
```toml
[dependencies]
relm4 = "0.9"
relm4-components = "0.9"
relm4-icons = "0.9"
tracker = "0.2"
async-trait = "0.1"

[features]
relm4-ui = ["relm4", "relm4-components", "relm4-icons", "tracker"]
```

### Integration Points
- **Services**: Direct integration with DataService, SyncManager
- **Models**: All data models reused without modification
- **Platform Utils**: Shared video/audio utilities
- **Database**: Direct repository access from components
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

## Migration Path

### Phase 0: Preparation
1. Set up Relm4 dependencies with feature flag
2. Create parallel platform module structure
3. Set up MessageBroker infrastructure
4. Prepare service layer for direct access

### Incremental Migration
1. Start with new features in Relm4
2. Migrate simplest components first (widgets)
3. Then factories (media cards, lists)
4. Then pages (home, library)
5. Finally complex components (player, details)

### Validation Checkpoints
- After each component: Performance benchmarks
- After each phase: User acceptance testing
- Continuous: Memory profiling
- Final: Full feature parity verification

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

This updated plan fully embraces Relm4's architecture, abandoning ViewModels in favor of:
- **AsyncComponents** for data-heavy pages with built-in loading states
- **Tracker pattern** for efficient, minimal UI updates
- **Factory pattern** for all collections and lists
- **Worker components** for background operations
- **Command pattern** for structured async operations
- **MessageBroker** for clean inter-component communication

The result will be a more performant, maintainable, and truly reactive UI that leverages Relm4's strengths while maintaining all current functionality.