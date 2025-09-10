# Relm4 Integration Plan

## Executive Summary

This document outlines a strategy to integrate Relm4 as the UI framework for Reel while preserving our existing reactive property system and ViewModels. By building a bridge between our Properties and Relm4's message system, we can leverage Relm4's mature declarative UI capabilities without losing our business logic investments.

## Current State Analysis

### What We Keep
- **All ViewModels**: LibraryViewModel, HomeViewModel, etc. remain unchanged
- **Reactive Property System**: Property<T>, ComputedProperty<T>, operators (map, filter, debounce)
- **Business Logic**: DataService, EventBus, backend integrations
- **Database Layer**: SeaORM repositories and entities

### What We Replace
- **Manual GTK widget construction** → Relm4 declarative `view!` macros
- **Manual binding setup** → Automatic reactive binding through bridge
- **Imperative UI updates** → Message-driven UI updates
- **Custom component system** → Relm4 component architecture

## Architecture Design

### Bridge System

Create a bridge trait that connects our Property system to Relm4's message-driven architecture:

```rust
// Core bridge trait
trait PropertyBridge<T> {
    fn bind_to_relm4<M: 'static>(
        &self, 
        sender: relm4::Sender<M>,
        message_fn: impl Fn(T) -> M + 'static
    );
    
    fn bind_to_relm4_debounced<M: 'static>(
        &self,
        sender: relm4::Sender<M>, 
        message_fn: impl Fn(T) -> M + 'static,
        duration: Duration
    );
}

// Reverse bridge for Relm4 → Property updates
trait Relm4Bridge {
    fn update_property<T>(&self, property: &Property<T>, value: T);
    fn create_two_way_binding<T, M>(&self, property: Property<T>) -> TwoWayBinding<T, M>;
}
```

### Component Hierarchy

```
ReelMainWindow (Relm4 Component)
├── HeaderBar (Relm4 Component)
├── Sidebar (Relm4 Component) 
├── ContentStack (Relm4 Component)
    ├── HomePage (Relm4 Component)
    ├── LibraryPage (Relm4 Component)  
    ├── PlayerPage (Relm4 Component)
    ├── SourcesPage (Relm4 Component)
    └── DetailsPage (Relm4 Component)
```

## Implementation Roadmap

### Phase 1: Foundation (Week 1-2)

#### Week 1: Bridge Infrastructure
- [ ] Add Relm4 dependency to Cargo.toml
- [ ] Implement PropertyBridge trait
- [ ] Create TwoWayBinding utility for search inputs
- [ ] Add Relm4Bridge for Property updates
- [ ] Write comprehensive bridge tests

**Deliverable**: Working bridge between Property<T> and Relm4 messages

```rust
// Example test
#[tokio::test]
async fn test_property_to_relm4_bridge() {
    let property = Property::new(42i32, "test");
    let (sender, receiver) = relm4::channel();
    
    property.bind_to_relm4(sender, |value| TestMessage::ValueChanged(value));
    
    property.set(100).await;
    assert_eq!(receiver.recv().await, TestMessage::ValueChanged(100));
}
```

#### Week 2: Simple Component Conversion
- [ ] Convert MediaCard to Relm4 component
- [ ] Implement reactive image loading in Relm4 context
- [ ] Create CardFactory for dynamic card generation
- [ ] Test performance vs current implementation

**Deliverable**: MediaCard as Relm4 component with equivalent functionality

### Phase 2: Core Pages (Week 3-6)

#### Week 3: Library Page Conversion
- [ ] Convert LibraryView to Relm4 component
- [ ] Bridge LibraryViewModel reactive properties
- [ ] Implement FlowBox with reactive item binding
- [ ] Add search, filtering, and sorting through bridge
- [ ] Preserve progressive loading and performance optimizations

**Target**: 70% reduction in LibraryView code size

```rust
#[relm4::component]
impl SimpleComponent for LibraryComponent {
    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            
            gtk::SearchEntry {
                set_text: &model.search_query,
                connect_search_changed[sender] => move |entry| {
                    sender.input(LibraryMsg::SearchChanged(entry.text().to_string()));
                },
            },
            
            // Reactive stack with automatic state management
            gtk::Stack {
                set_visible_child_name: &model.stack_state,
                // ... declarative children
            }
        }
    }
}
```

#### Week 4: Home Page Conversion  
- [ ] Convert HomePage to Relm4 component
- [ ] Bridge HomeViewModel reactive sections
- [ ] Implement horizontal carousels with reactive content
- [ ] Fix multiple backend section replacement issues through Relm4 state management

#### Week 5: Simple Pages
- [ ] Convert SourcesPage to Relm4 component  
- [ ] Convert AuthenticationView components
- [ ] Convert PreferencesView components
- [ ] Bridge authentication and preferences ViewModels

#### Week 6: Integration Testing
- [ ] End-to-end testing of converted pages
- [ ] Performance benchmarking vs original implementation
- [ ] Memory leak testing with Relm4 components
- [ ] Bug fixes and optimization

### Phase 3: Complex Pages (Week 7-10)

#### Week 7-8: Player Page
- [ ] Convert PlayerView to Relm4 component
- [ ] Bridge PlayerViewModel reactive properties
- [ ] Implement reactive playback controls
- [ ] Handle complex state (playing, paused, seeking)
- [ ] Preserve MPV/GStreamer integration

#### Week 9-10: Details Pages
- [ ] Convert MovieDetailsPage to Relm4 component
- [ ] Convert ShowDetailsPage to Relm4 component  
- [ ] Bridge DetailsViewModel reactive properties
- [ ] Implement reactive cast/crew grids
- [ ] Handle navigation state management

### Phase 4: Advanced Features (Week 11-12)

#### Week 11: Main Window Integration
- [ ] Convert ReelMainWindow to Relm4 application
- [ ] Implement reactive navigation state
- [ ] Bridge global app state to Relm4
- [ ] Handle window-level events (resize, focus)

#### Week 12: Polish and Optimization
- [ ] Performance profiling and optimization
- [ ] Memory usage optimization
- [ ] Code cleanup and documentation
- [ ] Migration guide for future pages

## Technical Implementation Details

### Dependency Management

```toml
[dependencies]
# Existing dependencies remain
relm4 = "0.9"
relm4-macros = "0.9"
relm4-components = "0.9"

# Keep existing reactive system
tokio = { version = "1.0", features = ["full"] }
# ... other existing deps
```

### Bridge Implementation

```rust
// Property → Relm4 Bridge
impl<T: Clone + Send + Sync + 'static> PropertyBridge<T> for Property<T> {
    fn bind_to_relm4<M: 'static>(
        &self, 
        sender: relm4::Sender<M>,
        message_fn: impl Fn(T) -> M + Send + Sync + 'static
    ) {
        let mut subscriber = self.subscribe();
        let message_fn = Arc::new(message_fn);
        let property_clone = self.clone();
        
        relm4::spawn_local(async move {
            // Set initial value
            let initial = property_clone.get_sync();
            let _ = sender.send(message_fn(initial));
            
            // Subscribe to changes
            while subscriber.wait_for_change().await {
                let value = property_clone.get().await;
                let _ = sender.send(message_fn(value));
            }
        });
    }
    
    fn bind_to_relm4_debounced<M: 'static>(
        &self,
        sender: relm4::Sender<M>,
        message_fn: impl Fn(T) -> M + Send + Sync + 'static,
        duration: Duration
    ) {
        let debounced = self.debounce(duration);
        debounced.bind_to_relm4(sender, message_fn);
    }
}

// Relm4 → Property Bridge  
struct Relm4Bridge;

impl Relm4Bridge {
    fn update_property<T: Clone + Send + Sync + 'static>(
        property: &Property<T>, 
        value: T
    ) {
        let property_clone = property.clone();
        relm4::spawn_local(async move {
            property_clone.set(value).await;
        });
    }
    
    fn create_two_way_binding<T, M>(
        property: Property<T>
    ) -> (relm4::Sender<M>, PropertySubscriber)
    where 
        T: Clone + Send + Sync + 'static,
        M: From<T> + 'static
    {
        let (sender, _) = relm4::channel::<M>();
        let subscriber = property.subscribe();
        
        // Setup bidirectional sync
        // ... implementation
        
        (sender, subscriber)
    }
}
```

### Error Handling

```rust
// Bridge error types
#[derive(Debug, thiserror::Error)]
enum BridgeError {
    #[error("Property bridge disconnected")]
    PropertyDisconnected,
    
    #[error("Relm4 component not initialized")]
    ComponentNotInitialized,
    
    #[error("Message send failed: {0}")]
    MessageSendFailed(String),
}

// Graceful degradation for bridge failures
impl PropertyBridge<T> for Property<T> {
    fn bind_to_relm4<M: 'static>(
        &self, 
        sender: relm4::Sender<M>,
        message_fn: impl Fn(T) -> M + 'static
    ) {
        // ... implementation with error handling
        relm4::spawn_local(async move {
            while subscriber.wait_for_change().await {
                let value = property_clone.get().await;
                match sender.send(message_fn(value)) {
                    Ok(_) => {},
                    Err(e) => {
                        tracing::warn!("Bridge message failed: {}", e);
                        // Continue trying - component might reconnect
                    }
                }
            }
        });
    }
}
```

## Migration Strategy

### Backward Compatibility

1. **Gradual Migration**: Convert one page at a time
2. **Shared ViewModels**: Both old and new pages can use same ViewModels
3. **Feature Parity**: Each converted page must match current functionality
4. **Performance Requirements**: No regression in performance or memory usage

### Testing Strategy

```rust
// Integration tests for each converted component
#[cfg(test)]
mod relm4_integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_library_component_reactive_search() {
        let data_service = create_test_data_service().await;
        let view_model = Arc::new(LibraryViewModel::new(data_service));
        
        let component = LibraryComponent::builder()
            .launch(view_model.clone())
            .detach();
        
        // Test reactive search through bridge
        view_model.search("test query".to_string()).await;
        
        // Verify UI updates
        // ... assertions
    }
    
    #[test]
    fn test_bridge_memory_leaks() {
        // Test that property bridges don't create memory leaks
        // when components are destroyed
    }
}
```

### Performance Benchmarks

- **Page Load Time**: < 100ms (current baseline)
- **Memory Usage**: No increase > 10% 
- **Image Loading**: Maintain current progressive loading
- **Scrolling Performance**: 60fps maintained
- **Search Debouncing**: 300ms delay preserved

## Risk Assessment and Mitigation

### Technical Risks

1. **Bridge Complexity**: Property ↔ Relm4 message translation overhead
   - *Mitigation*: Benchmark bridge performance, optimize hot paths
   
2. **Memory Leaks**: Relm4 components holding Property references
   - *Mitigation*: Use weak references, comprehensive leak testing
   
3. **Breaking Changes**: Relm4 API changes
   - *Mitigation*: Pin to stable version, monitor release notes

### Project Risks

1. **Timeline Overrun**: Complex migration taking longer than estimated
   - *Mitigation*: Focus on core pages first, defer advanced features
   
2. **Performance Regression**: Relm4 overhead impacting responsiveness  
   - *Mitigation*: Continuous benchmarking, fallback to current implementation
   
3. **Developer Learning Curve**: Team unfamiliar with Relm4 patterns
   - *Mitigation*: Start with simple components, comprehensive documentation

## Success Metrics

### Quantitative Goals
- [ ] **Code Reduction**: 60% less UI code in converted pages
- [ ] **Performance**: No regression in page load times
- [ ] **Memory**: No increase > 10% in memory usage
- [ ] **Maintainability**: 50% reduction in UI-related bugs

### Qualitative Goals  
- [ ] **Developer Experience**: Faster implementation of new UI features
- [ ] **Code Quality**: More readable and maintainable UI code
- [ ] **Architecture**: Clean separation between business logic and UI
- [ ] **Future-Proofing**: Easier to add new features and pages

## Rollback Plan

If Relm4 integration proves problematic:

1. **Week 1-6**: Easy rollback - keep existing pages alongside new ones
2. **Week 7+**: Selective rollback - convert back to current architecture
3. **Emergency**: Complete rollback using git history and feature flags

## Conclusion

This Relm4 integration plan leverages the maturity and declarative power of Relm4 while preserving our valuable reactive architecture. The bridge system allows incremental migration with minimal risk, while the phased approach ensures we can validate each step before proceeding.

The end result will be a more maintainable, declarative UI architecture that builds on our existing reactive foundations while benefiting from Relm4's mature ecosystem and GTK4 integration.