# Relm4 UI Implementation Plan

## Overview

This document outlines a plan to create a parallel Relm4-based UI implementation for Reel alongside the existing GTK4/libadwaita implementation. The goal is to demonstrate modern reactive UI patterns using Relm4's component architecture while maintaining feature parity with the current implementation.

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

### Current Patterns
- **Hybrid Architecture**: Mix of direct GTK usage and reactive ViewModels
- **Custom Property System**: Manual reactive properties with subscribers
- **Template-based UI**: GTK composite templates with Blueprint files
- **Manual Event Handling**: Explicit signal connections
- **State Management**: Shared Arc<AppState> with service layer

## Proposed Relm4 Architecture

### Directory Structure
```
src/platforms/relm4/
├── app.rs                    # Relm4 application root component
├── mod.rs                    # Platform module exports
├── platform_utils.rs        # Shared platform utilities
└── components/
    ├── main_window.rs        # Root window component
    ├── sidebar.rs           # Sidebar component
    ├── dialogs/
    │   ├── auth_dialog.rs    # Authentication dialog component
    │   └── preferences.rs    # Preferences dialog component
    ├── pages/               # Page components
    │   ├── home.rs          # Home page component
    │   ├── library.rs       # Library page component
    │   ├── movie_details.rs # Movie details component
    │   ├── show_details.rs  # Show details component
    │   ├── player.rs        # Player component
    │   └── sources.rs       # Sources management component
    ├── widgets/             # Reusable widget components
    │   ├── media_card.rs    # Media item card
    │   ├── player_controls.rs # Player control overlay
    │   ├── section_grid.rs  # Home section grid
    │   └── mod.rs
    └── shared/              # Shared component utilities
        ├── messages.rs      # Common message types
        ├── state.rs         # Relm4 state adapters
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
   - Create platform detection in main.rs

2. **Implement root app component**
   - Create `ReelApp` root component
   - Handle GTK application initialization
   - Integrate with existing AppState

3. **Build main window component**
   - Two-pane layout with AdwNavigationSplitView
   - Sidebar component placeholder
   - Content area stack placeholder

4. **Create sidebar component**
   - Convert SidebarViewModel to Relm4 component
   - Home/Sources navigation buttons
   - Source groups with libraries
   - Status display

### Phase 2: Core Pages (Week 3-4)
**Goal**: Home and Library pages working
**Success Criteria**: Can browse libraries and view media
**Tests**: Navigation works, media displays correctly

#### Tasks:
1. **Implement home page component**
   - Convert HomePage to Relm4 component
   - Section-based layout
   - Source filtering
   - Media card grid

2. **Build library page component**
   - Grid/list view toggle
   - Filtering and sorting
   - Virtual scrolling for performance
   - Media selection handling

3. **Create media card widget**
   - Reusable component for media items
   - Poster images with loading states
   - Progress indicators
   - Click handling

4. **Navigation system**
   - Message-based navigation
   - History management
   - Back button handling

### Phase 3: Details & Player (Week 5-6)
**Goal**: Full media browsing and playback
**Success Criteria**: Can play movies/shows end-to-end
**Tests**: Details load correctly, video playback works

#### Tasks:
1. **Movie details component**
   - Detailed metadata display
   - Cast/crew information
   - Play button integration
   - Related content

2. **Show details component**
   - Season/episode selection
   - Episode grid
   - Continue watching
   - Episode playback

3. **Player component**
   - MPV integration
   - OSD controls
   - Fullscreen handling
   - Progress tracking

4. **Player controls widget**
   - Reusable control overlay
   - Seek bar
   - Volume control
   - Playback buttons

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

### Base Component Pattern
```rust
#[derive(Debug)]
pub struct ComponentName {
    // State fields
}

#[derive(Debug)]
pub enum ComponentMsg {
    // Input messages
}

#[relm4::component]
impl SimpleComponent for ComponentName {
    type Init = InitData;
    type Input = ComponentMsg;
    type Output = OutputMsg;

    view! {
        // Declarative UI
    }

    fn init(init: Self::Init, root: &Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        // Initialization
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        // Message handling
    }
}
```

### State Integration Strategy

1. **Adapter Pattern**: Create Relm4 adapters for existing ViewModels
2. **Message Bridge**: Convert ViewModel property changes to Relm4 messages
3. **Shared Services**: Reuse existing DataService, SyncManager, etc.
4. **Event Bus Integration**: Bridge AppState event bus to Relm4 components

### Key Components

#### MainWindow Component
- **Purpose**: Root application window
- **Children**: Sidebar, ContentStack
- **Messages**: Navigation, WindowActions
- **State**: Current page, window size

#### Sidebar Component  
- **Purpose**: Navigation and source management
- **Children**: SourceGroup components
- **Messages**: SourceSelected, LibrarySelected, Refresh
- **State**: Connected sources, library visibility

#### Page Components
- **Purpose**: Content area pages (Home, Library, Details, Player)
- **Children**: Various widgets
- **Messages**: LoadData, SelectItem, Navigate
- **State**: Page-specific data and UI state

#### Widget Components
- **Purpose**: Reusable UI elements
- **Examples**: MediaCard, PlayerControls, SectionGrid
- **Messages**: Clicked, ValueChanged
- **State**: Widget-specific data

## Data Flow

### Message Flow
```
User Interaction → Widget Component → Page Component → Main Window → AppState Services
                                                     ↓
Navigation Manager ← Content Stack ← Page Router ← Navigation Messages
```

### State Flow  
```
AppState Services → Event Bus → ViewModel Adapters → Component State → UI Updates
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
relm4 = "0.10"
relm4-components = "0.10"
relm4-icons = "0.10"
```

### Integration Points
- **AppState**: Shared between both implementations
- **Services**: DataService, SyncManager, etc. remain unchanged
- **Models**: All data models reused
- **Platform Utils**: Shared video/audio utilities

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

## Future Considerations

### Platform Expansion
- Easier to add new platforms (Windows, macOS native)
- Component reuse across platforms
- Shared design system

### Feature Development
- Faster UI iteration
- Better design-dev collaboration
- Component library growth

### Maintenance
- Reduced UI-related bugs
- Clearer component responsibilities
- Easier refactoring

---

This plan provides a structured approach to implementing a modern, reactive UI using Relm4 while maintaining all current functionality and improving the development experience for future features.