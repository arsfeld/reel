# Navigation System Refactoring Plan

## Progress Summary (Updated: 2025-09-11 - 19:50)

### ✅ Phase 1: Strengthen NavigationManager - **COMPLETED**
- Extended NavigationRequest enum with new variants (ShowLibraryByKey, RefreshCurrentPage, ClearHistory)
- Added strongly-typed LibraryIdentifier struct
- Centralized navigation through navigate_to method
- Updated Sidebar to use NavigationRequest instead of direct methods
- MainWindow navigation methods now delegate to centralized navigation
- **FIXED**: Sidebar navigation now working (both Home and Library clicks)

### ✅ Phase 2: Page Lifecycle Management - **COMPLETED**
### 🟡 Phase 3: Type Safety Improvements - **PARTIALLY COMPLETED** (3.1 done)
### ⚪ Phase 4: Dependency Simplification - **NOT STARTED**
### ⚪ Phase 5: State Management Consolidation - **NOT STARTED**

## Current State Analysis

### Problems Identified

1. **Hybrid Navigation System**
   - NavigationManager exists but is bypassed by direct method calls
   - Sidebar uses direct MainWindow methods instead of NavigationRequest
   - Inconsistent navigation paths create complexity and potential bugs

2. **Circular Dependencies**
   - MainWindow ↔ Sidebar circular reference pattern
   - Complex weak reference management in callbacks
   - Difficult to reason about component lifecycles

3. **Page Lifecycle Issues**
   - Page creation mixed with navigation logic
   - Special cases for player page (always destroyed/recreated)
   - TODO comments indicate need for refactoring in `ensure_page_exists_for_request()`

4. **State Management Fragmentation**
   - Navigation state split between NavigationManager, MainWindow, and individual pages
   - Window state saved/restored manually for player transitions
   - No single source of truth for navigation state

5. **Type Safety Issues**
   - String-based library identifiers ("source_id:library_id")
   - Widget naming used to store navigation data
   - Fallback to "movies" type when library type not found

## Refactoring Goals

1. **Unified Navigation System**: All navigation flows through NavigationManager
2. **Clear Separation of Concerns**: Navigation, page lifecycle, and state management separated
3. **Type Safety**: Strongly-typed navigation data throughout
4. **Simplified Dependencies**: Reduce circular dependencies and callback complexity
5. **Performance**: Maintain smooth transitions while reducing complexity

## Implementation Plan

### Phase 1: Strengthen NavigationManager (Priority: High) ✅ COMPLETED

#### 1.1 Extend NavigationRequest Enum ✅
- [x] Add all navigation targets currently using direct methods
- [x] Include strongly-typed data (no string parsing needed)
- [x] Support navigation context/parameters

#### 1.2 Centralize Navigation Logic ✅
- [x] Move all navigation logic from MainWindow methods to NavigationManager
- [x] Convert direct navigation methods to use NavigationRequest
- [x] Ensure NavigationManager handles all navigation state

#### 1.3 Update Sidebar Navigation ✅
- [x] Replace direct MainWindow method calls with NavigationRequest dispatches
- [x] Use proper event/message passing instead of direct coupling
- [x] Remove MainWindow dependency from Sidebar

**Files to modify:**
- `src/platforms/gtk/ui/navigation.rs` (NavigationManager)
- `src/platforms/gtk/ui/navigation_request.rs`
- `src/platforms/gtk/ui/main_window.rs`
- `src/platforms/gtk/ui/widgets/sidebar.rs`

### Phase 2: Page Lifecycle Management (Priority: High)

#### 2.1 Create PageFactory ✅
- [x] Extract page creation logic from MainWindow
- [x] Centralize page configuration and callbacks
- [x] Handle page caching/reuse logic

#### 2.2 Separate Page Creation from Navigation ✅
- [x] Remove `ensure_page_exists_for_request()`
- [x] Pre-create pages or use lazy initialization in PageFactory
- [x] Clean separation between page management and navigation

#### 2.3 Standardize Page Cleanup ✅
- [x] Create consistent cleanup protocol for all pages
- [x] Special handling for player page in one place
- [x] Automatic cleanup when navigating away

**Files to create:**
- `src/platforms/gtk/ui/page_factory.rs` ✅

**Files to modify:**
- `src/platforms/gtk/ui/main_window.rs`
- `src/platforms/gtk/ui/pages/*.rs`

### Phase 3: Type Safety Improvements (Priority: Medium) ✅ PARTIALLY COMPLETED

#### 3.1 Create Typed Navigation Data ✅
- [x] Replace string-based library identifiers with struct
- [x] Create `LibraryIdentifier { source_id: String, library_id: String }`
- [x] Add type-safe navigation context structs

#### 3.2 Remove Widget Name Navigation Data
- [ ] Store navigation data in proper data structures
- [ ] Use widget data or associated storage instead of names
- [ ] Eliminate string parsing in navigation callbacks

**Files to create:**
- `src/platforms/gtk/ui/navigation_types.rs`

**Files to modify:**
- `src/platforms/gtk/ui/widgets/sidebar.rs`
- `src/platforms/gtk/ui/navigation_request.rs`

### Phase 4: Dependency Simplification (Priority: Medium)

#### 4.1 Event-Based Navigation
- [ ] Use EventBus for navigation requests from Sidebar
- [ ] Remove direct MainWindow reference from Sidebar
- [ ] Subscribe to navigation events in MainWindow

#### 4.2 Reduce Callback Complexity
- [ ] Simplify navigation callback chains
- [ ] Use async/await where appropriate
- [ ] Reduce weak reference juggling

**Files to modify:**
- `src/platforms/gtk/ui/widgets/sidebar.rs`
- `src/platforms/gtk/ui/main_window.rs`
- `src/events/types.rs` (add NavigationEvent)

### Phase 5: State Management Consolidation (Priority: Low)

#### 5.1 Unified Navigation State
- [ ] Move all navigation state to NavigationManager
- [ ] Remove redundant state from MainWindow
- [ ] Create NavigationState struct with all relevant data

#### 5.2 Automatic State Preservation
- [ ] Handle window state saving in NavigationManager
- [ ] Automatic restoration on back navigation
- [ ] Consistent state management for all pages

**Files to modify:**
- `src/platforms/gtk/ui/navigation.rs`
- `src/platforms/gtk/ui/main_window.rs`

## Testing Strategy

### Unit Tests
- [ ] NavigationManager state transitions
- [ ] NavigationRequest handling
- [ ] PageFactory page creation
- [ ] Type-safe identifier conversions

### Integration Tests
- [ ] Full navigation flows (sidebar → library → details → player → back)
- [ ] Multi-backend navigation scenarios
- [ ] State preservation across navigation
- [ ] Error handling for invalid navigation

### Manual Testing Checklist
- [x] All sidebar navigation works correctly ✅ (Fixed 2025-09-11)
- [ ] Back button functionality preserved
- [ ] Player page transitions smooth
- [ ] Window state properly saved/restored
- [ ] No memory leaks from circular references
- [ ] Performance unchanged or improved

## Migration Strategy

1. **Parallel Implementation**: Build new system alongside old
2. **Feature Flag**: Use feature flag to switch between old/new navigation
3. **Incremental Migration**: Migrate one navigation path at a time
4. **Validation**: Ensure each migrated path works before proceeding
5. **Cleanup**: Remove old code once all paths migrated

## Success Metrics

- **Code Reduction**: 20-30% less navigation-related code
- **Complexity**: Cyclomatic complexity reduced by 40%
- **Type Safety**: Zero string-based navigation data parsing
- **Performance**: Navigation latency ≤ current implementation
- **Maintainability**: Clear single responsibility for each component

## Risk Mitigation

### Risks
1. **Breaking existing navigation**: Mitigate with comprehensive testing
2. **Performance regression**: Profile before/after, optimize hot paths
3. **GTK4 limitations**: Research constraints early, adapt design if needed
4. **Scope creep**: Stick to plan, defer nice-to-haves

### Rollback Plan
- Keep old navigation code until new system fully validated
- Feature flag allows instant rollback if issues found
- Git branches for each phase allow partial rollback

## Timeline Estimate

- **Phase 1**: 2-3 days (critical path)
- **Phase 2**: 2-3 days (critical path)
- **Phase 3**: 1-2 days (can be parallelized)
- **Phase 4**: 1-2 days (can be parallelized)
- **Phase 5**: 1 day (optional optimization)
- **Testing**: 2-3 days (throughout)

**Total**: 9-14 days of focused development

## Implementation Notes

### Critical Bug Fixes (2025-09-11)

**Sidebar Navigation Issues Fixed:**
1. **Infinite Loop in Home Navigation**:
   - Problem: `show_home_page_for_source()` was calling `navigate_to()` which then called `show_home_page_for_source()` again
   - Solution: Implemented actual home page display logic directly in `show_home_page_for_source()`

2. **Library Navigation Not Working**:
   - Problem 1: Similar infinite loop - `navigate_to_library()` was calling back to `navigate_to()`
   - Solution: Implemented actual library loading logic in `navigate_to_library()`
   - Problem 2: Click handlers weren't being connected to library rows
   - Root cause: `setup_source_group_navigation()` couldn't find ListBox in PreferencesGroup widget tree
   - Solution: Connected `row-activated` handler directly when creating ListBox

3. **Home Row Click Not Working**:
   - Problem: Used `home_row.connect_activated()` instead of `home_list.connect_row_activated()`
   - Solution: Connected to parent ListBox's `row-activated` signal

**Temporary Workaround Still in Place:**
- `navigate_to()` method still contains duplicate page loading logic after NavigationManager state update
- This should be moved into NavigationManager itself for proper separation of concerns

### Phase 2 Progress Details (2025-09-11)

**Phase 2.2 Completed - All Page Creation Migrated to PageFactory:**
1. **Migrated all page creation methods:**
   - `show_movie_details` now uses PageFactory
   - `show_show_details` now uses PageFactory
   - `show_player` now uses PageFactory (always creates new)
   - `show_library_view` now uses PageFactory
   - Sources page already using PageFactory

2. **Removed obsolete code:**
   - Deleted `ensure_page_exists_for_request()` method
   - Page creation is now centralized in PageFactory
   - MainWindow only handles navigation and setup

3. **Improved separation of concerns:**
   - PageFactory owns page lifecycle
   - MainWindow handles navigation flow
   - Clear boundaries between components

**Phase 2.1 Completed - PageFactory Created:**
1. **Created PageFactory module** (`page_factory.rs`):
   - Centralized page creation and caching logic
   - Handles all page types: HomePage, SourcesPage, LibraryView, MovieDetailsPage, ShowDetailsPage, PlayerPage
   - Implements smart caching (reuses most pages, always creates new PlayerPage)
   - Provides configuration methods for setting up callbacks

2. **Integrated PageFactory into MainWindow:**
   - Added PageFactory initialization during MainWindow setup
   - Started migration with SourcesPage as example
   - Maintains backward compatibility with existing navigation

**Phase 2.2 In Progress - Separation of Concerns:**
- PageFactory now owns page creation logic
- MainWindow delegates page creation to PageFactory
- Clear separation between page lifecycle and navigation
- Still need to migrate remaining page creation and remove `ensure_page_exists_for_request()`

**Phase 2.3 Completed - Standardized Page Cleanup (2025-09-11 - 19:50):**
1. **Added centralized cleanup system:**
   - Created `cleanup_current_page()` method in MainWindow
   - Integrated cleanup calls into all navigation methods
   - Cleanup happens automatically before navigating to new pages

2. **Enhanced PageFactory with cleanup support:**
   - Added `needs_cleanup()` method to check if a page type needs cleanup
   - Added `cleanup_page_async()` method for future extensibility
   - PlayerPage cleanup handled specially due to its non-GObject nature

3. **Cleanup integration points:**
   - `show_movie_details()` - calls cleanup before showing
   - `show_show_details()` - calls cleanup before showing  
   - `show_library_view()` - calls cleanup before showing
   - `show_player()` - calls cleanup before showing (preserves existing player cleanup logic)

**Next Immediate Steps:**
- Phase 3.2: Remove Widget Name Navigation Data
- Phase 4: Implement Event-Based Navigation

### Phase 1 Completion Details (2025-09-11)

**Key Changes Made:**
1. **Extended NavigationRequest enum** (`navigation_request.rs`):
   - Added `LibraryIdentifier` struct for type-safe library identification
   - Added `NavigationContext` and `WindowState` structs for future use
   - New variants: `ShowLibraryByKey`, `RefreshCurrentPage`, `ClearHistory`

2. **Centralized Navigation** (`main_window.rs`):
   - All navigation now flows through `navigate_to()` method
   - `show_home_page_for_source()` and `navigate_to_library()` now delegate to `navigate_to()`
   - Updated `navigation_request_to_page()` to handle all new variants
   - Updated `ensure_page_exists_for_request()` for new navigation types

3. **Sidebar Integration** (`sidebar.rs`):
   - Library navigation uses `NavigationRequest::show_library_by_key()`
   - Home navigation uses `NavigationRequest::show_home()`
   - Removed direct MainWindow method calls

**Backward Compatibility:**
- Old string-based library keys still work via `ShowLibraryByKey` variant
- Existing navigation paths remain functional
- No breaking changes to external APIs

**Next Steps:**
- Phase 2 should focus on extracting page creation logic into PageFactory
- Consider implementing event-based navigation (Phase 4) sooner to further decouple components
- Phase 3.2 (removing widget name navigation data) can be done incrementally

## Original Notes

- Priority on Phases 1 & 2 as they address the most critical architectural issues
- Phases 3-5 can be done incrementally after core refactoring
- Consider creating a feature branch for this work
- Document new navigation architecture in ARCHITECTURE.md once complete