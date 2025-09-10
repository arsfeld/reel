# Reactive UI Architecture Documentation

## Executive Summary

This document provides a comprehensive analysis of the reactive UI architecture implemented in the Reel application. The application demonstrates a modern reactive approach using ViewModels, Properties, and an EventBus for state management and UI updates.

## Architecture Overview

### Core Components

#### 1. Property System (`src/core/viewmodels/property.rs`)

The foundation of our reactive architecture is the `Property<T>` system, which provides:

- **Observable State**: Properties use `tokio::sync::watch` for efficient current value access
- **Change Notifications**: Built on `tokio::sync::broadcast` for efficient multi-subscriber updates
- **Subscription Model**: Each property can have multiple subscribers via `PropertySubscriber`
- **Hybrid Architecture**: `watch` for state, `broadcast` for notifications (backward compatible)
- **Lagged Broadcast Tolerance**: Subscribers gracefully handle broadcast lag, preventing dropped updates

```rust
pub struct Property<T: Clone + Send + Sync> {
    watch_sender: Arc<watch::Sender<T>>,
    watch_receiver: watch::Receiver<T>,
    broadcast_sender: broadcast::Sender<()>, // Backward compatibility
    name: String,
}
```

**Core API:**
- `get()` / `get_sync()`: Get current value (sync version is truly synchronous)
- `try_get()`: Non-blocking value access (returns `Option<T>`)
- `set()`: Updates value and notifies all subscribers
- `update()`: Mutates value in-place with a closure
- `subscribe()`: Returns a unique `PropertySubscriber` for change notifications
- `name()`: Get property name for debugging

**PropertySubscriber API:**
- `wait_for_change()`: Async wait for next change notification
- `try_recv()`: Non-blocking check for pending changes

**Reactive Operators:**
- `.map(f)`: Transform property with function `f` → `ComputedProperty<U>`
- `.filter(predicate)`: Filter property values → `ComputedProperty<Option<T>>`
- `.debounce(duration)`: Debounce rapid changes → `ComputedProperty<T>`
- **Chainable**: `property.map(|x| x * 2).filter(|&x| x > 10).debounce(100ms)`

**ComputedProperty:**
- Automatically updates based on dependencies
- Type-safe dependency management via `PropertyLike` trait
- Automatic cleanup with task handles that abort on Drop
- Same API as Property (get/set/subscribe/operators)

#### 2. ViewModel Pattern (`src/ui/viewmodels/`)

ViewModels act as the reactive layer between data services and UI components:

**Base Trait**:
```rust
#[async_trait]
pub trait ViewModel: Send + Sync {
    async fn initialize(&self, event_bus: Arc<EventBus>);
    fn subscribe_to_property(&self, property_name: &str) -> Option<PropertySubscriber>;
    async fn refresh(&self);
    fn dispose(&self);
}
```

**Implemented ViewModels**:
- `LibraryViewModel`: Manages library content with filtering, sorting, and batch updates
- `DetailsViewModel`: Handles media details with targeted updates on MediaUpdated events
- `PlayerViewModel`: Controls playback state reactively
- `HomeViewModel`: Manages homepage content sections
- `SidebarViewModel`: Handles navigation state
- `SourcesViewModel`: Manages backend sources

#### 3. EventBus System (`src/events/event_bus.rs`)

Central nervous system for application-wide reactive updates:

- **Typed Events**: Strongly-typed event system with `DatabaseEvent` and `EventPayload`
- **Filtered Subscriptions**: `EventFilter` allows selective event listening
- **Priority System**: Events have priority levels for handling importance
- **Event History**: Maintains recent events for debugging
- **Statistics**: Tracks event metrics for performance monitoring

```rust
pub struct EventBus {
    sender: broadcast::Sender<DatabaseEvent>,
    stats: Arc<RwLock<EventBusStats>>,
    event_history: Arc<RwLock<Vec<DatabaseEvent>>>,
}
```

## Reactive Patterns in Use

### 1. Property-Based Reactivity

**Strengths**:
- Clean separation between state and UI
- Automatic UI updates via subscriptions
- Thread-safe concurrent access
- No manual state synchronization needed

**Example**: LibraryViewModel filtered items
```rust
// ViewModel updates filtered items
self.filtered_items.set(filtered).await;

// UI subscribes and reacts
let mut subscriber = view_model.filtered_items().subscribe();
while subscriber.wait_for_change().await {
    let items = vm.filtered_items().get().await;
    view.update_items_from_viewmodel(items);
}
```

### 2. Event-Driven Updates

**DetailsViewModel** demonstrates sophisticated event handling:
- Subscribes to `MediaUpdated`, `MediaDeleted`, `PlaybackPositionUpdated` events
- Performs targeted, in-place updates without UI disruption
- Maintains UI state during background updates

```rust
async fn handle_event(&self, event: DatabaseEvent) {
    if let EventType::MediaUpdated = event.event_type {
        // Targeted update without toggling is_loading
        self.merge_updated_data(updated_item).await;
    }
}
```

### 3. Batch Update Strategy

**LibraryViewModel** implements intelligent batching:
- Debounces rapid updates (250ms window)
- Merges incremental changes during sync
- Prevents UI thrashing during bulk operations

```rust
struct UpdateBatch {
    last_update: Option<Instant>,
    pending_refresh: bool,
}
```

## Areas of Excellence

### 1. Fully Reactive Components

- **ViewModels**: All ViewModels properly implement reactive patterns
- **Property System**: Robust, thread-safe observable implementation
- **EventBus**: Well-designed pub-sub system with filtering
- **Subscription Management**: Proper cleanup and lifecycle handling

### 2. Smart Update Strategies

- **Incremental Updates**: LibraryViewModel merges changes without full refresh
- **Targeted Updates**: DetailsViewModel updates specific fields
- **Debouncing**: Prevents excessive UI updates during sync

### 3. Clean Architecture

- **Separation of Concerns**: Clear boundaries between UI, ViewModels, and Services
- **Type Safety**: Strongly-typed events and properties
- **Testability**: ViewModels can be tested independently of UI

### 4. Reactive UI Binding Pattern

**Show Details Page** demonstrates the successful implementation of a reusable reactive binding pattern:

- **Reactive Helpers**: Generic binding functions for text, labels, images, and visibility
- **Property Subscriptions**: Automatic UI updates when ViewModel properties change
- **Transform Functions**: Clean separation of data transformation logic from UI code
- **Lifecycle Management**: Proper subscription cleanup using weak references

```rust
// Implemented reactive binding helpers
fn bind_text_to_property<T, F>(&self, widget: &gtk4::Label, property: Property<T>, transform: F)
fn bind_visibility_to_property<T, F>(&self, widget: &impl WidgetExt, property: Property<T>, transform: F)
fn bind_label_to_property<T, F>(&self, widget: &gtk4::Label, property: Property<T>, transform: F)
fn bind_image_to_property<T, F>(&self, widget: &gtk4::Picture, property: Property<T>, transform: F)
```

These patterns can be extracted into a reusable GTK reactive binding library for other pages.

## Areas Needing Improvement

### 1. Recent Progress: Show Details Page Migration ✅

#### Completed Reactive Implementation
The Show Details page (`src/platforms/gtk/ui/pages/show_details.rs`) has been successfully migrated to a fully reactive architecture:

- **100% ViewModel integration**: All data flows through DetailsViewModel
- **Reactive UI bindings**: Title, year, rating, synopsis, images update automatically via Property subscriptions
- **Zero manual widget manipulation**: Eliminated all `.set_text()`, `.set_visible()`, `.set_paintable()` calls
- **Reactive loading states**: Placeholder visibility and image loading handled declaratively
- **Event-driven updates**: Genre chips and episode lists update via reactive bindings

**Example of implemented reactive approach**:
```rust
// Reactive binding for show title
self.bind_label_to_property(
    &imp.show_title,
    viewmodel.current_item().clone(),
    |detailed_info| {
        if let Some(info) = detailed_info {
            if let MediaItem::Show(show) = &info.media {
                return show.title.clone();
            }
        }
        String::new()
    },
);

// Reactive binding for image loading
self.bind_image_to_property(
    &imp.show_poster,
    viewmodel.current_item().clone(),
    |detailed_info| {
        if let Some(info) = detailed_info {
            if let MediaItem::Show(show) = &info.media {
                return show.poster_url.clone();
            }
        }
        None
    },
);
```

#### Remaining Imperative Code
While significant progress has been made, some UI pages still rely on GTK's imperative patterns:

- **Library Page**: Still uses manual widget manipulation and RefCell state
- **Player Page**: Direct GStreamer management
- **Movie Details Page**: Similar patterns to the old show details page

#### Direct Service Calls from UI
Some UI components still make direct service calls instead of going through ViewModels:
- Player page directly manages GStreamer
- Authentication dialog bypasses ViewModel layer
- Preferences window directly modifies state

### 2. Missing Reactive Patterns

#### Limited Computed Properties Usage
While `ComputedProperty` is implemented and available, it's underutilized:
- **Show Details**: Successfully implemented reactive helper functions but could benefit from computed properties for complex state combinations
- **Library Page**: Filter results could be computed from search + items
- **Player Page**: Playback progress percentage could be computed
- **General**: UI visibility states could be derived from multiple properties

#### Limited Two-Way Binding
Current implementation is mostly one-way (ViewModel → UI):
- Form inputs require manual event handling
- Settings changes need explicit handlers
- Search/filter inputs lack declarative binding

#### No Reactive Collections
Arrays/Vecs are replaced wholesale instead of using reactive collections:
- Could implement `ObservableList` with add/remove/update events
- Would enable efficient UI updates for large lists
- Reduce memory churn during updates

### 3. Event System Gaps

#### Missing Event Types
- UI interaction events (navigation, selection)
- Configuration change events
- Network status events
- Error events with context

#### No Event Replay/Sourcing
- Can't replay events for debugging
- No event persistence for crash recovery
- Missing undo/redo capability

#### Limited Event Metadata
- No correlation IDs for tracking related events
- Missing timestamps in some events
- No user/session context

### 4. Performance Considerations

#### Subscription Leaks Risk
- Some subscriptions may not be properly cleaned up
- Missing weak references in some closures
- Potential memory leaks in long-running subscriptions

#### Excessive Cloning
- Properties clone values on get()
- Could use `Arc` for large data structures
- Consider copy-on-write semantics

#### No Subscription Batching
- Multiple property changes trigger multiple updates
- Could batch subscriptions in same tick
- Missing transaction-like updates

## Recommendations

### Immediate Improvements

1. **Migrate UI Pages to Declarative Bindings**
   - Create binding helpers for GTK widgets
   - Reduce RefCell usage in favor of Properties
   - Implement two-way binding framework

2. **Utilize Computed Properties**
   - Convert derived state to ComputedProperty
   - Reduce redundant calculations
   - Improve consistency

3. **Implement Observable Collections**
   - Create `ObservableVec<T>` with granular updates
   - Add collection-specific events
   - Optimize large list updates

### Medium-Term Enhancements

1. **Enhanced Event System**
   - Add correlation IDs and metadata
   - Implement event replay capability
   - Create event middleware for logging/metrics

2. **Reactive Forms Framework**
   - Declarative form validation
   - Two-way binding helpers
   - Form state management

3. **Performance Optimizations**
   - Implement subscription batching
   - Add memory pooling for events
   - Use weak references where appropriate

### Long-Term Vision

1. **Full Reactive UI DSL**
   - Create GTK reactive wrapper library
   - Declarative UI composition
   - Hot-reload capability

2. **Time-Travel Debugging**
   - Event sourcing with replay
   - State snapshots
   - Redux DevTools-like experience

3. **Reactive Persistence**
   - Auto-save on property changes
   - Optimistic updates with rollback
   - Conflict-free replicated data types (CRDTs)

## Implementation Priority Matrix

| Priority | Effort | Impact | Recommendation | Status |
|----------|--------|--------|----------------|--------|
| ~~High~~ | ~~Low~~ | ~~High~~ | ~~Show Details reactive migration~~ | ✅ **COMPLETED** |
| High | Low | High | Migrate Movie Details page to reactive patterns | 🔄 **NEXT** |
| High | Medium | High | Migrate Library page to reactive bindings | 📋 **PLANNED** |
| High | Medium | High | Implement Observable Collections | 📋 **PLANNED** |
| Medium | Low | Medium | Add Computed Properties usage | 📋 **PLANNED** |
| Medium | Medium | High | Create two-way binding framework | 📋 **PLANNED** |
| Low | High | Medium | Build reactive UI DSL | 📋 **FUTURE** |
| Low | Medium | Low | Add time-travel debugging | 📋 **FUTURE** |

## Metrics for Success

### Quantitative Metrics
- Reduce RefCell usage by 80%
- Decrease UI update latency by 50%
- Reduce memory allocations during updates by 60%
- Zero subscription leaks in production

### Qualitative Metrics
- Improved code maintainability
- Better testability of UI logic
- Reduced coupling between components
- Enhanced developer experience

## Conclusion

The Reel application demonstrates a solid foundation for reactive UI with its Property system, ViewModels, and EventBus. The architecture successfully separates concerns and provides reactive updates for most components.

**Recent Progress**: The Show Details page migration represents a significant milestone, proving that GTK applications can successfully implement fully reactive patterns while maintaining performance and usability. The implemented reactive binding helpers provide a reusable foundation for migrating other pages.

**Key Achievement**: The elimination of manual widget manipulation (.set_text(), .set_visible(), .set_paintable()) demonstrates that declarative UI updates are both practical and beneficial in GTK applications, resulting in cleaner, more maintainable code.

**Next Steps**: With the reactive patterns proven and helper functions established, the next logical step is migrating the Movie Details page using the same patterns, followed by the more complex Library page. The foundation is now solid for achieving a fully reactive architecture across the entire application.

The gradual migration path allows for incremental improvements without disrupting existing functionality, making it practical to evolve the architecture while maintaining stability. Each migration builds on proven patterns, reducing implementation risk and development time.
