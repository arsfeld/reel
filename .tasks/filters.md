# Comprehensive Filter System Implementation Plan

## Current State Analysis

### Existing Infrastructure
1. **Filter Types Defined** - `src/platforms/gtk/ui/filters.rs` contains comprehensive filter enums and structures:
   - `FilterType`: WatchStatus, SortOrder, Genre, Year, Rating, Resolution, ContentRating
   - `WatchStatus`: All, Watched, Unwatched, InProgress
   - `SortOrder`: Title, Year, Rating, DateAdded, DateWatched (asc/desc)
   - `FilterCriterion`: Specific filter implementations
   - `FilterSet`: Container for active filters
   - `FilterManager`: Application logic for filtering

2. **Reactive Architecture** - `src/core/viewmodels/library_view_model.rs`:
   - Reactive Properties for search, filters, and sorting
   - Event-driven updates through EventBus
   - 75% complete migration to reactive ViewModels

3. **UI Infrastructure** - **UPDATED ARCHITECTURE**:
   - **Filter Controls Widget** - `src/platforms/gtk/ui/widgets/library_filters.rs` (✅ NEW)
     - Dedicated `LibraryFilters` widget extracted from main window
     - Reusable component with callback-based architecture
     - Filter state management with `FilterState` struct
   - **Library Page** - `src/platforms/gtk/ui/pages/library.rs`:
     - Reactive search with debouncing (300ms, min 2 chars)
     - Two-way binding between UI widgets and reactive properties
     - FlowBox binding for automatic UI updates
   - **Main Window Integration** - Simplified to ~120 lines of library code (70% reduction)

4. **Data Models** - Rich metadata available:
   - Movies: title, year, rating, genres, cast, crew, added_at, watched, view_count, last_watched_at
   - Shows: title, year, rating, genres, cast, watched_episode_count, total_episode_count
   - Episodes: season/episode numbers, show metadata, duration, watch status
   - Database: SQLite with SeaORM, JSON metadata fields, proper indexing

### Current Limitations (**UPDATED - Stage 1 Complete**)
1. ✅ **UI Implementation**: LibraryFilters widget now implements all basic filter types (watch status, sort order, genre, year range, rating)
2. ✅ **Backend Integration**: FilterManager integrated with reactive ViewModels through unified enum system
3. ✅ **Basic Filter Types**: Genre, year range, rating UI fully implemented in LibraryFilters widget
4. ✅ **Backend Filtering**: Year range and rating filters fully integrated with ViewModel methods
5. **Search Integration**: Search works but not integrated with other filters
6. **Filter Persistence**: No saving/loading of user filter preferences
7. **Advanced Features**: No filter combinations, saved searches, or filter presets

### Recent Architecture Changes (✅ COMPLETED)
- **Filter Controls Extraction**: Successfully moved filter UI from main window to dedicated `LibraryFilters` widget
- **Code Simplification**: Reduced main window library code by 70% (from ~400 to ~120 lines)
- **Reusable Component**: Filter controls are now a standalone, reusable widget with callback architecture
- **Virtual Library Removal**: Eliminated unused virtual scrolling code that was causing complexity

## Implementation Plan

### Stage 1: Complete Basic Filter UI ✅ **COMPLETED**

**Goal**: Implement all basic filter controls in the library header bar
**Success Criteria**: All filter types have working UI controls that update the ViewModel

#### Tasks: ✅ **ALL COMPLETED**
1. ✅ **Extended LibraryFilters Widget** (`src/platforms/gtk/ui/widgets/library_filters.rs`)
   - ✅ Extended existing `LibraryFilters` widget with all basic filter types
   - ✅ Added Genre dropdown with 8 common genres + "All Genres" option
   - ✅ Added Year range selector (two spinbuttons: from/to year, 1900-2030)
   - ✅ Added Rating filter (minimum rating spinbutton, 0-10.0 with 0.1 precision)
   - ✅ Enhanced Sort order dropdown (8 options: Title, Year, Rating, Added - asc/desc)
   - ✅ Updated `FilterState` struct to include all filter types with conversion to `FilterOptions`
   - ✅ Filter reset button (completed in Stage 2)

2. ✅ **Unified Enum System** (resolved duplicate enums)
   - ✅ Consolidated `WatchStatus` and `SortOrder` enums between filters module and ViewModel
   - ✅ Fixed enum variant naming inconsistencies (`DateAdded` → `Added`)
   - ✅ Added type conversion layer for backward compatibility

3. ✅ **Integrated with Reactive Properties** (`src/platforms/gtk/ui/pages/library.rs`)
   - ✅ Connected UI controls to reactive properties with callback system
   - ✅ Implemented proper debouncing for year range changes
   - ✅ Added methods for all new filter types (all fully working: genres, year range, rating)

4. ✅ **Updated Integration** (`src/platforms/gtk/ui/main_window.rs`)
   - ✅ Enhanced filter callback to handle all new filter types
   - ✅ Added type conversion between ViewModel and UI enums
   - ✅ Maintained backward compatibility with existing library view methods

#### Files Modified: ✅ **ALL COMPLETED**
- ✅ `src/platforms/gtk/ui/widgets/library_filters.rs` (extended with 5 filter types)
- ✅ `src/platforms/gtk/ui/pages/library.rs` (added filter integration methods)
- ✅ `src/platforms/gtk/ui/main_window.rs` (enhanced filter callback handling)

#### Tests: ✅ **VERIFIED**
- ✅ All filter controls appear and function correctly in header bar
- ✅ Reactive properties update when UI changes
- ✅ Watch status and sort order filters fully functional with ViewModel
- ✅ Genre filter connected to existing `set_genres()` method
- ✅ Year range and rating filters fully integrated with ViewModel backend
- ✅ Code compiles successfully with no errors

#### **Stage 1 Results:**
- **5 Filter Types**: Watch Status, Sort Order, Genre, Year Range, Rating
- **Full UI Implementation**: All controls working with proper GTK4/libadwaita styling
- **Reactive Integration**: Connected to existing reactive architecture
- **Type Safety**: Full type conversion between UI and ViewModel layers
- **Backward Compatibility**: Existing library functionality unchanged

---

## 📋 **Stage 2 Initial Increment Completed** ✅

**Date**: 2025-09-10  
**Goal**: Complete immediate backend integration priorities and add reset functionality  
**Status**: ✅ **100% COMPLETE**

### **What Was Accomplished:**

1. **ViewModel Backend Integration** ✅
   - Added `set_year_range(min: i32, max: i32)` method to `LibraryViewModel`
   - Added `set_min_rating(rating: f32)` method to `LibraryViewModel`
   - Added `clear_year_range()` and `clear_min_rating()` methods for reset functionality
   - All methods follow existing reactive pattern with `filter_options.update()` and `apply_filters_and_sort()`

2. **Library Page Integration** ✅
   - Updated `update_year_range_filter()` method in `library.rs` to call ViewModel 
   - Updated `update_rating_filter()` method in `library.rs` to call ViewModel
   - Replaced placeholder logging with actual async ViewModel calls using `glib::spawn_future_local`
   - Maintained existing callback architecture and error handling patterns

3. **Filter Reset Button** ✅
   - Added reset button to `LibraryFilters` widget with clear icon (`edit-clear-symbolic`)
   - Reset button clears all filter controls to default values
   - Proper state management: resets internal `FilterState` to defaults
   - Callback integration: notifies main callback when reset is triggered
   - UI integration: button positioned at end of filter controls with separator

4. **Code Quality** ✅
   - All code compiles successfully with no errors or warnings
   - Follows existing project patterns and conventions
   - Uses proper Rust async patterns with GTK4/libadwaita
   - Maintains backward compatibility with existing filter functionality

### **Files Modified:**
- `src/core/viewmodels/library_view_model.rs` (added 4 new methods)
- `src/platforms/gtk/ui/pages/library.rs` (updated 2 placeholder methods)
- `src/platforms/gtk/ui/widgets/library_filters.rs` (added reset button + 30 lines)

### **Technical Achievement:**
- **Full Backend Integration**: All 5 filter types now connected to reactive ViewModel
- **Complete UI Controls**: All filter types have working controls + reset functionality  
- **Reactive Architecture**: Maintains event-driven updates and proper property synchronization
- **User Experience**: One-click reset of all filters to default state

### **Ready for Next Increment:**
The filter system now has a solid foundation for advanced features:
- Filter combinations and validation logic
- Filter persistence and user preferences
- Filter presets and quick access
- ✅ Visual filter indicators (chips/tags) - **COMPLETED**

---

## 📋 **Stage 2 Filter Chips Increment Completed** ✅

**Date**: 2025-09-11  
**Goal**: Add visual filter indicators (chips) to show active filters  
**Status**: ✅ **100% COMPLETE**

### **What Was Accomplished:**

1. **Filter Chips Display** ✅
   - Added `FlowBox` container below filter controls for dynamic chip display
   - Chips appear automatically when filters are active (watch status, genres, year range, rating)
   - Clean visual design using "pill" CSS class for proper GTK4/libadwaita styling
   - Auto-hide behavior: chips only show when filters are active, maintaining clean default state

2. **Real-time Updates** ✅
   - All filter changes now trigger automatic chip updates
   - Chips display format: "Unwatched", "Action", "2020-2025", "Rating ≥ 7.5"
   - Reset button properly clears both controls and chips
   - Shared state management using `Rc<RefCell<>>` pattern for GTK callbacks

3. **Architecture Improvements** ✅
   - Reorganized `LibraryFilters` widget to use vertical layout (controls + chips)
   - Added `update_chips_display()` static method for callback use
   - Proper GTK memory management with shared references
   - Maintained existing callback architecture and backward compatibility

4. **Code Quality** ✅
   - All compilation issues resolved (borrowing, cloning, etc.)
   - Follows existing project patterns and GTK4 best practices
   - No breaking changes to existing filter functionality
   - Clean separation between filter controls and chip display

### **Technical Implementation:**

- **Modified Files**:
  - `src/platforms/gtk/ui/widgets/library_filters.rs` (+80 lines)
    - Added `chips_box: gtk4::FlowBox` field
    - Added `update_chips_display()` and `add_chip_to_box()` methods
    - Updated all filter callbacks to trigger chip updates
    - Reorganized layout to vertical orientation

- **Filter Chip Features**:
  - **Watch Status**: Shows non-default states (Unwatched, Watched, In Progress)
  - **Genres**: Individual chips for each selected genre
  - **Year Range**: Formatted as "YYYY" or "YYYY-YYYY"
  - **Rating**: Formatted as "Rating ≥ X.X"
  - **Auto-Hide**: Empty state when no filters are active

### **User Experience Improvements:**

✅ **Immediate Visual Feedback** - Users can instantly see what filters are active  
✅ **Clear Filter State** - No confusion about whether filters are applied  
✅ **Clean Interface** - Chips only appear when needed, no visual clutter  
✅ **Consistent Styling** - Matches libadwaita design patterns with pill chips  
✅ **Responsive Updates** - Chips update in real-time as filters change  

### **Next Small Increment Ready:**

The chips foundation is perfect for adding:
- **Clickable chip removal** (next logical increment)
- **Chip hover states and tooltips**
- **Chip animations and transitions**
- **Chip grouping and organization**

---

## 🚀 **Next Steps for Stage 2**

### **Immediate Priority: Complete Backend Integration** ✅ **COMPLETED**
1. ✅ **Add ViewModel Methods** (`src/core/viewmodels/library_view_model.rs`):
   - ✅ Added `set_year_range(min: i32, max: i32)` method
   - ✅ Added `set_min_rating(rating: f32)` method  
   - ✅ Added `clear_year_range()` and `clear_min_rating()` methods
   - ✅ Follow the pattern of existing `set_genres()` method

2. ✅ **Update Library View** (`src/platforms/gtk/ui/pages/library.rs`):
   - ✅ Replaced placeholder methods with actual ViewModel calls
   - ✅ Year range and rating filtering integrated with backend

3. ✅ **Add Filter Reset Button** (`src/platforms/gtk/ui/widgets/library_filters.rs`):
   - ✅ Added "Reset All" button to clear all active filters
   - ✅ Reset to default state (All watch status, Title A-Z sort, no genres/years/rating)
   - ✅ Proper state management and callback integration

### **Quick Wins for Stage 2:** ✅ **COMPLETED**
- ✅ **Filter Reset Button**: Completed in working increment
- ✅ **ViewModel Integration**: Completed in working increment 
- ✅ **Filter Chips**: Visual indicators of active filters (completed 2025-09-11)

### Stage 2: Advanced Filter Features (Week 2)

**Goal**: Implement advanced filtering features for better user experience
**Success Criteria**: Users can save filter combinations and access quick filter presets

#### Tasks:
1. **Filter Combination Logic** (`src/platforms/gtk/ui/filters.rs`)
   - Enhance FilterManager to handle multiple active filters correctly
   - Implement filter intersection logic (AND operation)
   - Add filter validation and conflict resolution
   - Optimize filtering performance for large libraries

2. **Filter Persistence** (new service)
   - `src/services/filter_preferences.rs` - Save/load user filter preferences
   - Integration with existing keyring/preferences system
   - Per-library filter memory (remember last filters used)
   - SQLite storage for filter presets

3. **Quick Filter Presets** (UI enhancement)
   - "Recently Added" (last 30 days)
   - "Highly Rated" (rating > 8.0)
   - "Unwatched Favorites" (unwatched + rating > 7.0)
   - "In Progress" (partially watched)
   - User-defined custom presets

4. ✅ **Filter Chips/Tags** (UI enhancement) - **COMPLETED 2025-09-11**
   - ✅ Visual representation of active filters as display-only chips
   - ✅ Clear indication when filters are active
   - **TODO**: One-click filter removal (next increment)
   - ✅ Filter summary in header

#### Files to Create:
- `src/services/filter_preferences.rs`
- ~~`src/platforms/gtk/ui/widgets/filter_chip.rs`~~ (integrated into LibraryFilters)
- `src/platforms/gtk/ui/widgets/filter_preset_menu.rs`

#### Files to Modify:
- `src/platforms/gtk/ui/filters.rs` (enhance filtering logic)
- `src/platforms/gtk/ui/widgets/library_filters.rs` (add preset integration to existing widget) **UPDATED**
- `src/platforms/gtk/ui/main_window.rs` (minimal changes - presets accessed through existing LibraryFilters widget)
- `src/core/viewmodels/library_view_model.rs` (add preset support)

#### Tests:
- Filter combinations work correctly (multiple genres, year + rating, etc.)
- Filter preferences persist across app restarts
- Quick presets apply expected filter combinations
- ✅ Filter chips accurately represent active filters

### Stage 3: Advanced Filter Types (Week 3)

**Goal**: Implement resolution, content rating, and other advanced filters
**Success Criteria**: All metadata-based filtering options are available and functional

#### Tasks:
1. **Resolution Filter** (requires backend metadata enhancement)
   - Extract resolution data from backend APIs
   - Store resolution in database metadata JSON field
   - Create resolution filter UI (dropdown: 4K, 1080p, 720p, etc.)
   - Handle cases where resolution data is unavailable

2. **Content Rating Filter** (requires backend metadata enhancement)
   - Extract content ratings from backend APIs (G, PG, PG-13, R, etc.)
   - Store content rating in database metadata JSON field
   - Create content rating filter UI (multi-select checkboxes)
   - Handle different rating systems (MPAA, TV ratings, international)

3. **Cast/Crew Filter** (advanced feature)
   - Search by actor, director, producer names
   - Autocomplete from existing cast/crew data
   - Performance optimization for large cast databases
   - Integration with existing person data models

4. **Duration Filter** (useful for discovery)
   - Filter by movie/episode length ranges
   - Quick presets: "Short (<90min)", "Standard (90-150min)", "Long (>150min)"
   - Custom range selector
   - Handle duration data availability

#### Files to Create:
- `src/platforms/gtk/ui/widgets/resolution_filter.rs`
- `src/platforms/gtk/ui/widgets/content_rating_filter.rs`
- `src/platforms/gtk/ui/widgets/cast_crew_filter.rs`
- `src/platforms/gtk/ui/widgets/duration_filter.rs`

#### Files to Modify:
- `src/backends/traits.rs` (metadata extraction requirements)
- `src/backends/plex/mod.rs` (extract additional metadata)
- `src/backends/jellyfin/mod.rs` (extract additional metadata)
- `src/db/entities/media_items.rs` (metadata schema documentation)

#### Tests:
- Resolution filter works when metadata available
- Content rating filter handles different rating systems
- Cast/crew search performs well with large datasets
- Duration filter handles missing duration data gracefully

### Stage 4: Search Integration & Performance (Week 4)

**Goal**: Unify search with filters and optimize for large libraries
**Success Criteria**: Fast, responsive filtering with excellent search integration

#### Tasks:
1. **Unified Search + Filter Architecture** (`src/core/viewmodels/library_view_model.rs`)
   - Combine text search with active filters
   - Search within filtered results or filter within search results
   - Clear interaction model for search + filter combinations
   - Search history integration with filter states

2. **Performance Optimization** (database and caching)
   - Database query optimization for filter combinations
   - Implement filter result caching in DataService
   - Background filter index building for fast lookups
   - Lazy loading for large filtered result sets

3. **Advanced Search Features** (enhance existing search)
   - Search within specific fields (title, overview, cast)
   - Boolean search operators (AND, OR, NOT)
   - Fuzzy matching for typo tolerance
   - Search suggestions based on available metadata

4. **Filter Analytics & Recommendations** (smart features)
   - Track popular filter combinations
   - Suggest filters based on user viewing patterns
   - "Similar to filtered items" recommendations
   - Filter performance metrics and optimization

#### Files to Modify:
- `src/core/viewmodels/library_view_model.rs` (unified search+filter)
- `src/services/data.rs` (performance optimization)
- `src/platforms/gtk/ui/pages/library.rs` (search integration)
- `src/db/repository/media.rs` (optimized queries)

#### Files to Create:
- `src/services/filter_analytics.rs` (usage tracking)
- `src/utils/search_engine.rs` (advanced search logic)

#### Tests:
- Search + filter combinations perform well (< 100ms for 10k items)
- Advanced search operators work correctly
- Filter caching improves performance measurably
- Search suggestions are relevant and helpful

### Stage 5: Polish & User Experience (Week 5)

**Goal**: Perfect the user experience and add finishing touches
**Success Criteria**: Intuitive, polished filter experience that feels native to GNOME

#### Tasks:
1. **GNOME HIG Compliance** (design polish)
   - Ensure all filter controls follow GNOME Human Interface Guidelines
   - Proper spacing, sizing, and accessibility
   - Keyboard navigation support for all filter controls
   - Screen reader compatibility and ARIA labels

2. **Animation & Transitions** (visual polish)
   - Smooth transitions when applying/removing filters
   - Loading states for expensive filter operations
   - Visual feedback for filter state changes
   - Skeleton loading for filtered results

3. **Advanced UX Features** (power user features)
   - Keyboard shortcuts for common filter operations
   - Filter URL parameters for shareable filtered views
   - Export filtered results to various formats
   - Filter operation undo/redo stack

4. **Error Handling & Edge Cases** (robustness)
   - Graceful handling of missing metadata
   - Clear error messages for invalid filter combinations
   - Fallback behavior when backend data is incomplete
   - Progress indicators for long-running filter operations

#### Files to Modify:
- All UI widget files (accessibility and visual polish)
- `src/platforms/gtk/ui/style.css` (filter control styling)
- `src/platforms/gtk/ui/pages/library.rs` (transitions and animations)

#### Files to Create:
- `src/platforms/gtk/ui/widgets/filter_loading_state.rs`
- `src/utils/filter_url_params.rs`
- `src/services/filter_export.rs`

#### Tests:
- All filter controls are keyboard accessible
- Screen readers can navigate filter interface
- Animations are smooth and don't block UI
- Error states provide helpful guidance

## Technical Considerations

### Database Performance
- **Indexing Strategy**: Ensure proper database indexes for all filterable fields
- **Query Optimization**: Use SeaORM's query builder for efficient filter combinations
- **Caching**: Implement result caching for expensive filter operations
- **Pagination**: Handle large filtered result sets with proper pagination

### Reactive Architecture Integration
- **Property Synchronization**: Ensure all filter properties stay in sync with UI
- **Event Batching**: Batch rapid filter changes to avoid excessive updates
- **Debouncing**: Apply appropriate debouncing for real-time filters
- **Memory Management**: Properly dispose of property subscriptions

### Backend Compatibility
- **Metadata Availability**: Handle cases where backends don't provide all metadata
- **API Limitations**: Work around backend API restrictions for advanced filtering
- **Cross-Backend Consistency**: Ensure filters work consistently across Plex/Jellyfin
- **Error Resilience**: Graceful degradation when backend filtering fails

### User Experience
- **Progressive Disclosure**: Show advanced filters only when needed
- **Visual Feedback**: Clear indication of active filters and their effects
- **Performance**: Keep filter operations under 100ms for good UX
- **Accessibility**: Full keyboard and screen reader support

## Success Metrics

### Performance Targets
- Filter application: < 100ms for libraries with 10k+ items
- UI responsiveness: < 16ms for filter control interactions  
- Memory usage: < 50MB additional for all filter functionality
- Database queries: < 10ms for common filter combinations

### User Experience Goals
- Filter discoverability: Users find filter options within 30 seconds
- Filter efficiency: Users can apply 3+ filters in under 10 seconds
- Filter understanding: Clear visual indication of active filters
- Filter satisfaction: Filtered results match user expectations 95%+ of time

### Technical Goals
- Code coverage: 85%+ test coverage for all filter functionality
- Error handling: Graceful handling of all edge cases and missing data
- Accessibility: Full WCAG 2.1 compliance for filter interface
- Maintainability: Clean, documented code following project patterns

## Implementation Notes

### Phase Approach
- Each stage builds on the previous one
- Can be developed in parallel for different filter types
- Focus on core functionality before advanced features
- Thorough testing at each stage before proceeding

### Architectural Benefits from Library Cleanup
- **Clean Foundation**: LibraryFilters widget provides perfect starting point for filter expansion
- **Reduced Complexity**: Main window no longer cluttered with filter code
- **Reusable Design**: Filter functionality is now properly encapsulated and reusable
- **Callback Architecture**: Existing callback system makes it easy to add new filter types
- **Simplified Integration**: Main window already integrated with LibraryFilters widget

### Risk Mitigation
- **Performance Risk**: Implement caching and query optimization early
- **Complexity Risk**: Start with simple filters, add complexity gradually
- **UX Risk**: Get early user feedback on filter placement and behavior
- **Data Risk**: Handle missing/inconsistent metadata gracefully

### Future Enhancements
- AI-powered filter suggestions based on viewing history
- Social features: shared filter presets between users
- Advanced analytics: trending filters, popular combinations
- Integration with external services for enhanced metadata