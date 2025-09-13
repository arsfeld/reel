# Relm4 UI Implementation Checklist

**🚨 PRIORITY CHANGE**: Relm4 is now the DEFAULT and PRIMARY UI implementation.
- GTK implementation is DEPRECATED but serves as UI/UX reference
- All new development happens in Relm4
- No more ViewModel pattern - pure Relm4 components with trackers
- **IMPORTANT**: Keep GTK4/libadwaita styling and UI patterns - just reimplement with Relm4 architecture

## ⚠️ Type Safety Dependencies

**IMPORTANT**: The Relm4 implementation depends on the type-safety refactoring being completed in parallel or first.

### Critical Dependencies from Type-Safety Checklist:
1. **Phase 1 (Core Type Definitions)** - ✅ COMPLETED
   - SourceId, LibraryId, MediaItemId, ShowId, etc.
   - All typed IDs are now available for use in Relm4 components!

2. **Phase 2 (CacheKey System)** - Required for proper cache interaction
   - CacheKey enum to replace string-based keys
   - Should be done early to avoid refactoring Relm4 components later

3. **Phase 3-4 (Service Updates)** - DataService and SyncManager type safety
   - Can be done in parallel with Relm4 development
   - Relm4 components will adapt to new signatures as they're updated

### Coordination Strategy:
- ✅ Type-safety Phase 1 COMPLETE - typed IDs ready to use!
- ⚠️ Start type-safety Phase 2 (CacheKey) ASAP to avoid refactoring
- Relm4 development can proceed NOW with typed IDs
- Use typed IDs (SourceId, LibraryId, etc.) in ALL new Relm4 components
- Service updates (Phase 3-4) can happen in parallel

## 🎯 Immediate Priority Tasks

### Week 1 Critical Path
1. **Set up Relm4 infrastructure** (Phase 0)
   - Configure as default in Cargo.toml
   - Create platform module structure
   - Set up MessageBroker and Workers

2. **Create foundation components** (Phase 1)
   - AsyncComponent app root
   - Main window with tracker
   - Sidebar with factory

3. **Implement first factory** (Phase 2.1)
   - MediaCard factory component
   - Prove tracker pattern works
   - Validate performance

### Success Criteria for Week 1
- [✅] App launches with Relm4 by default
- [ ] Sidebar shows sources using factory pattern
- [ ] At least one page loads with AsyncComponent
- [ ] Tracker pattern proven to work

## Phase 0: Preparation & Setup
**Goal**: Set up Relm4 as default platform with all necessary infrastructure
**Success Criteria**: Project builds with Relm4 as default

### 1. Configure Relm4 as Default Platform
- [✅] Set Relm4 as default feature in `Cargo.toml`
- [✅] Add Relm4 dependencies to `Cargo.toml`
  - [✅] relm4 = "0.10"
  - [✅] relm4-components = "0.10"
  - [✅] relm4-icons = "0.10"
  - [✅] tracker = "0.2"
  - [✅] async-trait = "0.1"
- [✅] Update main.rs to default to Relm4 platform
- [✅] Create `src/platforms/relm4/mod.rs`
- [✅] Set up MessageBroker infrastructure
- [✅] Create worker thread pool setup
- [ ] Document GTK implementation as deprecated/reference-only

## Phase 1: Foundation with Best Practices (Week 1-2)
**Goal**: Basic Relm4 app with AsyncComponents, Trackers, and Workers
**Success Criteria**: App launches with reactive sidebar and navigation
**Type Safety Note**: Components should use typed IDs (SourceId, LibraryId, etc.) from Phase 1 of type-safety refactoring

### 2. Implement root app as AsyncComponent
- [✅] Create `ReelApp` as AsyncComponent in `src/platforms/relm4/app.rs`
- [✅] Handle GTK/Adwaita application initialization
- [✅] Set up global MessageBroker
- [✅] Initialize DataService and SyncManager connections
- [✅] Set up command handler infrastructure
- [🟡] Create worker coordinator (partial - needs more work)

### 3. Build main window as AsyncComponent
- [ ] Create `src/platforms/relm4/components/main_window.rs` as AsyncComponent
- [ ] Implement with `#[tracker::track]` for window state
- [ ] Add `init_loading_widgets()` for initial load
- [ ] **KEEP GTK4 LAYOUT**: Two-pane with AdwNavigationSplitView
- [ ] **KEEP GTK4 STYLE**: Same header bar, buttons, spacing
- [ ] Navigation stack with history management
- [ ] Content area with dynamic page loading
- [ ] Track window state changes efficiently

### 4. Create sidebar with Tracker pattern
- [ ] Create `src/platforms/relm4/components/sidebar.rs`
- [ ] Implement with `#[tracker::track]` for all state
- [ ] NO ViewModels - direct component state
- [ ] **KEEP GTK4 DESIGN**: Same list style, icons, grouping
- [ ] **KEEP GTK4 BEHAVIOR**: Same selection, hover effects
- [ ] Factory pattern for source list items
- [ ] Track connection status changes
- [ ] Track selected library changes (use LibraryId from type-safety)
- [ ] Efficient re-renders only on tracked changes
- [ ] Output messages for navigation
- [ ] **Type Safety**: Use SourceId and LibraryId types instead of strings

## Phase 2: Core Pages with Factories & Workers (Week 3-4)
**Goal**: Reactive pages with efficient updates
**Success Criteria**: Smooth browsing with virtual scrolling

### 1. Create Factory Components First
- [ ] Create `src/platforms/relm4/components/factories/media_card.rs`
  - [ ] Implement as FactoryComponent with tracker
  - [ ] **KEEP GTK4 CARD DESIGN**: Same dimensions, shadows, rounded corners
  - [ ] **KEEP GTK4 OVERLAY**: Progress bar, play button overlay
  - [ ] Track hover state, progress, selection
  - [ ] Lazy image loading via worker
  - [ ] **Type Safety**: Use MediaItemId for item identification
- [ ] Create `src/platforms/relm4/components/factories/section_row.rs`
  - [ ] **KEEP GTK4 CAROUSEL**: Same horizontal scrolling behavior
  - [ ] Horizontal scrolling factory
  - [ ] Lazy loading of items
- [ ] Create `src/platforms/relm4/components/factories/source_item.rs`
  - [ ] **KEEP GTK4 LIST STYLE**: Same row height, padding, icons
  - [ ] Track connection status
  - [ ] Show library count
  - [ ] **Type Safety**: Use SourceId for source identification

### 2. Set up Worker Components
- [ ] Create `src/platforms/relm4/components/workers/image_loader.rs`
  - [ ] Async image fetching with cache
  - [ ] Thumbnail generation
- [ ] Create `src/platforms/relm4/components/workers/search_worker.rs`
  - [ ] Full-text search indexing
  - [ ] Filter processing
- [ ] Create `src/platforms/relm4/components/workers/sync_worker.rs`
  - [ ] Background data synchronization
  - [ ] Progress reporting

### 3. Implement HomePage as AsyncComponent
- [ ] Create `src/platforms/relm4/components/pages/home.rs`
- [ ] NO ViewModels - pure Relm4 state
- [ ] **KEEP GTK4 LAYOUT**: Same section headers, spacing, typography
- [ ] **KEEP GTK4 SECTIONS**: Continue Watching, Recently Added, etc.
- [ ] Use AsyncComponent with `init_loading_widgets()`
- [ ] FactoryVecDeque for each section
- [ ] Commands for loading section data
- [ ] Tracker for section visibility
- [ ] Lazy loading with intersection observer

### 4. Build Library with Virtual Factory
- [ ] Create `src/platforms/relm4/components/pages/library.rs`
- [ ] AsyncComponent with loading skeleton
- [ ] **KEEP GTK4 GRID**: Same spacing, responsive columns
- [ ] **KEEP GTK4 FILTERS**: Same filter bar, dropdown styles
- [ ] Virtual FactoryVecDeque for media grid
- [ ] Tracker for filters and sort state
- [ ] SearchWorker integration
- [ ] Efficient grid/list toggle
- [ ] Pagination via commands

## Phase 3: Details & Player with Commands (Week 5-6)
**Goal**: Reactive playback with efficient state management
**Success Criteria**: Smooth playback with minimal UI overhead

### 1. Create Episode Factory First
- [ ] Create `src/platforms/relm4/components/factories/episode_item.rs`
  - [ ] Track watched state
  - [ ] Show progress bar
  - [ ] Thumbnail with number overlay

### 2. MovieDetails as AsyncComponent
- [ ] Create `src/platforms/relm4/components/pages/movie_details.rs`
- [ ] AsyncComponent with loading skeleton
- [ ] **KEEP GTK4 LAYOUT**: Hero section, metadata pills, description
- [ ] **KEEP GTK4 STYLE**: Background blur, gradient overlay
- [ ] Commands for fetching full metadata
- [ ] Factory for cast/crew carousel
- [ ] Tracker for play button state
- [ ] Lazy load related content
- [ ] Background blur with poster

### 3. ShowDetails with Episode Factory
- [ ] Create `src/platforms/relm4/components/pages/show_details.rs`
- [ ] AsyncComponent for show loading
- [ ] **KEEP GTK4 DESIGN**: Season dropdown, episode cards
- [ ] **KEEP GTK4 LAYOUT**: Two-column on desktop, single on mobile
- [ ] FactoryVecDeque for season tabs
- [ ] FactoryVecDeque for episode grid
- [ ] Tracker for watched episodes
- [ ] Commands for season switching
- [ ] Efficient state updates on episode watch

### 4. Player Component with Commands
- [ ] Create `src/platforms/relm4/components/pages/player.rs`
- [ ] Commands for all playback operations
- [ ] **KEEP GTK4 OSD**: Same overlay controls, fade in/out
- [ ] **KEEP GTK4 STYLE**: Same seek bar, volume slider, buttons
- [ ] NO direct MPV calls - all via commands
- [ ] Tracker for playback state
- [ ] Tracker for fullscreen mode
- [ ] Tracker for controls visibility
- [ ] Auto-hide timer for OSD

### 5. Create Playback Worker
- [ ] Create `src/platforms/relm4/components/workers/playback_tracker.rs`
  - [ ] Progress tracking every second
  - [ ] Sync with database
  - [ ] Resume position management

## Phase 4: Management & Polish (Week 7-8)
**Goal**: Complete feature parity
**Success Criteria**: All features from GTK implementation work

### 1. Sources management component
- [ ] Create `src/platforms/relm4/components/pages/sources.rs`
- [ ] Implement add/remove sources
- [ ] Add authentication flow
- [ ] Create source testing functionality
- [ ] Handle settings management
- [ ] Display sync status
- [ ] Add refresh controls

### 2. Authentication dialog
- [ ] Create `src/platforms/relm4/components/dialogs/auth_dialog.rs`
- [ ] Implement server type selection (Plex/Jellyfin)
- [ ] Add credential input forms
- [ ] Handle OAuth flow for Plex
- [ ] Handle username/password for Jellyfin
- [ ] Display error states
- [ ] Add connection testing

### 3. Preferences dialog
- [ ] Create `src/platforms/relm4/components/dialogs/preferences.rs`
- [ ] Add theme selection
- [ ] Implement player preferences
  - [ ] Default player backend
  - [ ] Hardware acceleration
  - [ ] Subtitle settings
- [ ] Add library settings
  - [ ] Default view mode
  - [ ] Items per page
- [ ] Create data management section
  - [ ] Cache settings
  - [ ] Offline content

### 4. Polish and optimization
- [ ] Performance tuning
  - [ ] Component render optimization
  - [ ] Memory usage profiling
  - [ ] Lazy loading implementation
- [ ] Error handling
  - [ ] Network error recovery
  - [ ] Graceful degradation
  - [ ] User-friendly error messages
- [ ] Loading states
  - [ ] Skeleton loaders
  - [ ] Progress indicators
  - [ ] Smooth transitions
- [ ] Accessibility
  - [ ] Keyboard navigation
  - [ ] Screen reader support
  - [ ] High contrast mode

## UI/UX Preservation Guidelines

### GTK4/libadwaita Elements to Keep
- [ ] **Window Chrome**: Same header bar, window controls, title
- [ ] **Navigation**: AdwNavigationSplitView behavior
- [ ] **Lists**: AdwPreferencesGroup styling for source lists
- [ ] **Cards**: Same shadow, border radius, hover effects
- [ ] **Buttons**: AdwButtonContent with icons and labels
- [ ] **Animations**: Same fade/slide transitions
- [ ] **Spacing**: GNOME HIG spacing (6, 12, 18, 24px)
- [ ] **Typography**: Same font sizes and weights
- [ ] **Colors**: Adwaita color palette
- [ ] **Icons**: Same symbolic icons from icon theme

### CSS Classes to Preserve
- [ ] `.card` for media cards
- [ ] `.dim-label` for secondary text
- [ ] `.title-1` through `.title-4` for headings
- [ ] `.destructive-action` for dangerous buttons
- [ ] `.suggested-action` for primary buttons
- [ ] `.flat` for borderless buttons
- [ ] `.circular` for round buttons
- [ ] `.osd` for overlay controls

### Behavior to Maintain
- [ ] Responsive breakpoints (mobile/desktop)
- [ ] Keyboard navigation patterns
- [ ] Focus indicators
- [ ] Touch gestures
- [ ] Drag and drop where applicable
- [ ] Context menus
- [ ] Tooltips

## Component Infrastructure

### Core Infrastructure
- [✅] Create `src/platforms/relm4/components/shared/messages.rs`
  - [✅] Navigation messages
  - [✅] Data loading messages  
  - [✅] Error messages
  - [✅] Worker messages
  - [ ] **Type Safety**: Update messages to use typed IDs (SourceId, LibraryId, MediaItemId, etc.)
- [✅] Create `src/platforms/relm4/components/shared/commands.rs`
  - [✅] Async command definitions
  - [✅] Command result types
  - [✅] Command error handling
  - [ ] **Type Safety**: Update command parameters to use typed IDs
- [✅] Create `src/platforms/relm4/components/shared/broker.rs`
  - [✅] MessageBroker setup
  - [✅] Component registration
  - [✅] Message routing

### Factory Infrastructure
- [ ] Set up factory base traits
- [ ] Create factory testing utilities
- [ ] Document factory patterns
- [ ] Create factory examples

### Worker Infrastructure  
- [ ] Worker thread pool configuration
- [ ] Worker message queuing
- [ ] Worker lifecycle management
- [ ] Worker error recovery

### NO ViewModels - Direct Service Integration
- [ ] Components directly use DataService
  - [ ] **Type Safety**: DataService methods will use typed IDs after Phase 3 of type-safety refactoring
- [ ] Components directly use SyncManager
  - [ ] **Type Safety**: SyncManager methods will use typed IDs after Phase 4 of type-safety refactoring
- [ ] Components manage their own state with trackers
- [ ] MessageBroker replaces custom event bus
- [ ] **Type Safety**: Use CacheKey enum instead of string-based cache keys (Phase 2 of type-safety)

## Testing

### Component Unit Tests
- [ ] Test AsyncComponent initialization
- [ ] Test tracker state changes
- [ ] Test factory updates
- [ ] Test worker message passing
- [ ] Test command execution
- [ ] Test MessageBroker routing
- [ ] Test loading states

### Integration Tests
- [ ] Test data flow from services to components
- [ ] Test navigation between pages
- [ ] Test playback workflow
- [ ] Test source management
- [ ] Test authentication flow
- [ ] Test offline mode

### UI Automation Tests
- [ ] Test complete user workflows
- [ ] Test keyboard navigation
- [ ] Test responsive layout
- [ ] Test error recovery

### Performance Benchmarks
- [ ] Measure startup time
- [ ] Measure page transition speed
- [ ] Measure memory usage
- [ ] Measure scroll performance
- [ ] Compare with GTK implementation

## Success Metrics

### Functionality
- [ ] All current features implemented
- [ ] Feature parity with GTK version
- [ ] No regressions in user workflows
- [ ] All backends working (Plex, Jellyfin)

### Performance
- [ ] Startup time < 500ms
- [ ] Page transitions < 100ms
- [ ] Memory usage < 200MB for typical libraries
- [ ] 60fps scrolling in large lists
- [ ] Within 20% of GTK version performance

### Code Quality
- [ ] >90% test coverage for components
- [ ] Clear component boundaries
- [ ] Minimal code duplication
- [ ] Consistent code style
- [ ] Comprehensive documentation

### Developer Experience
- [ ] Faster development of new features
- [ ] Easier UI debugging and testing
- [ ] Better component reusability
- [ ] Clear error messages
- [ ] Hot reload working

## Architecture Decisions

### Core Principles
- [✅] **Relm4 First**: Default UI implementation
- [✅] **No ViewModels**: Pure Relm4 state management
- [✅] **Tracker Pattern**: Efficient minimal updates
- [✅] **Factory Pattern**: All collections use factories
- [✅] **AsyncComponents**: Data-heavy pages
- [✅] **Worker Pattern**: Background operations
- [✅] **Command Pattern**: Async operations

### Implementation Notes
- [ ] Document tracker usage patterns
- [ ] Document factory best practices
- [ ] Document worker communication
- [ ] Document command patterns
- [ ] Create component templates

### Migration Strategy
- [✅] Relm4 is PRIMARY implementation
- [✅] GTK serves as UI/UX reference
- [✅] **KEEP GTK4 STYLE**: Reimplement exact same UI with Relm4
- [ ] Port all GTK4 widgets to Relm4 components
- [ ] Maintain CSS classes and styling
- [ ] Keep Blueprint UI structure where applicable
- [ ] Remove GTK implementation after Phase 4
- [ ] Migrate all tests to Relm4
- [ ] Update documentation

### Technical Optimizations
- [ ] Virtual scrolling with factories
- [ ] MPV integration via commands
- [ ] Lazy loading everywhere
- [ ] Image caching strategy
- [ ] Memory profiling

### Future Enhancements
- [ ] Component library package
- [ ] Design system with CSS
- [ ] Plugin architecture
- [ ] Theme system
- [ ] Accessibility features

---

## Summary of Changes

### What's Different from Original Plan
1. **NO ViewModels** - Components manage their own state
2. **Tracker Pattern Everywhere** - Efficient minimal updates
3. **Factory Pattern Required** - All lists/grids use factories
4. **AsyncComponents Default** - Data pages are async
5. **Workers for Background** - All heavy ops in workers
6. **Commands for Async** - Structured async operations
7. **MessageBroker** - Replaces custom event bus
8. **KEEP GTK4 UI/UX** - Exact same look and feel, just Relm4 architecture

### Key Benefits
- **Performance**: Minimal re-renders with trackers
- **Simplicity**: No dual state management
- **Type Safety**: Pure Relm4 patterns
- **Testability**: Component isolation
- **Maintainability**: Clear patterns

### Timeline Impact
- **Faster Development**: After initial setup
- **Better Performance**: From day one
- **Easier Testing**: Component-based
- **Cleaner Architecture**: No adapter layer

**Legend**:
- [ ] Not started
- [🟡] In progress
- [✅] Complete / Decided
- [❌] Blocked
- [⏭️] Skipped / No longer needed