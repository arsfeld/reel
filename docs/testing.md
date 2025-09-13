# Testing Strategy for Relm4 Application

## Overview

This document outlines the comprehensive testing strategy for the Relm4-based media player application. The testing approach leverages Relm4's component-based architecture to ensure reliability, performance, and maintainability across all layers of the application.

## Testing Philosophy

### Core Principles
1. **Component Isolation**: Test components independently using Relm4's testing utilities
2. **Behavior-Driven**: Focus on testing user-visible behavior rather than implementation details
3. **Type Safety First**: Leverage Rust's type system and our typed IDs to prevent runtime errors
4. **Async-Aware**: Properly test async components, commands, and workers
5. **Real Database Testing**: Use in-memory SQLite for integration tests

### Testing Pyramid
```
         /\
        /UI\        <- UI Automation (10%)
       /----\
      /Integ.\      <- Integration Tests (30%)
     /--------\
    /   Unit   \    <- Unit Tests (60%)
   /____________\
```

## Test Categories

### 1. Unit Tests

#### Component State Tests
Test individual Relm4 components in isolation:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use relm4::test::{TestComponent, TestMacros};

    #[test]
    fn test_sidebar_component_initialization() {
        let component = TestComponent::<SidebarComponent>::new()
            .with_model(SidebarModel::default());

        assert_eq!(component.model().selected_library, None);
        assert!(component.model().sources.is_empty());
    }

    #[test]
    fn test_sidebar_library_selection() {
        let mut component = TestComponent::<SidebarComponent>::new()
            .with_model(SidebarModel::default());

        let library_id = LibraryId::new();
        component.send(SidebarInput::SelectLibrary(library_id.clone()));

        assert_eq!(component.model().selected_library, Some(library_id));
        assert!(component.has_output(SidebarOutput::LibrarySelected(library_id)));
    }
}
```

#### Tracker Pattern Tests
Verify that tracker patterns minimize re-renders:

```rust
#[test]
fn test_tracker_efficiency() {
    #[tracker::track]
    struct TestModel {
        unchanged_field: String,
        #[tracker::do_not_track]
        changing_field: i32,
    }

    let mut model = TestModel {
        unchanged_field: "static".to_string(),
        changing_field: 0,
    };

    // Changing non-tracked field should not trigger update
    model.changing_field = 42;
    assert!(!model.changed(TestModel::unchanged_field()));

    // Changing tracked field should trigger update
    model.set_unchanged_field("new".to_string());
    assert!(model.changed(TestModel::unchanged_field()));
}
```

#### Factory Component Tests
Test factory patterns for collections:

```rust
#[test]
fn test_media_card_factory() {
    let factory = FactoryVecDeque::<MediaCardFactory>::new();

    // Add items
    let item1 = MediaItemModel {
        id: MediaItemId::new(),
        title: "Test Movie".to_string(),
        // ...
    };

    factory.guard().push_back(item1.clone());
    assert_eq!(factory.guard().len(), 1);

    // Test factory updates
    factory.guard().get_mut(0).unwrap().set_watched(true);
    assert!(factory.guard().get(0).unwrap().watched);
}
```

#### Command Tests
Test async command execution:

```rust
#[tokio::test]
async fn test_fetch_media_command() {
    let db = create_test_database().await;

    let command = FetchMediaCommand {
        library_id: LibraryId::new(),
        page: 1,
        page_size: 20,
    };

    let result = command.execute(db.clone()).await;
    assert!(result.is_ok());

    let items = result.unwrap();
    assert!(items.len() <= 20);
}
```

#### Service Function Tests
Test stateless service functions:

```rust
#[tokio::test]
async fn test_media_service_pagination() {
    let db = create_test_database().await;
    seed_test_data(&db).await;

    let page1 = MediaService::get_media_items(
        &db,
        &LibraryId::new(),
        1,
        10
    ).await.unwrap();

    let page2 = MediaService::get_media_items(
        &db,
        &LibraryId::new(),
        2,
        10
    ).await.unwrap();

    // Ensure no overlap between pages
    let page1_ids: HashSet<_> = page1.iter().map(|m| m.id.clone()).collect();
    let page2_ids: HashSet<_> = page2.iter().map(|m| m.id.clone()).collect();
    assert!(page1_ids.is_disjoint(&page2_ids));
}
```

### 2. Integration Tests

#### Component Communication Tests
Test message passing between components:

```rust
#[tokio::test]
async fn test_sidebar_to_library_navigation() {
    let app = TestApp::new().await;

    // Setup components
    let sidebar = app.get_component::<SidebarComponent>();
    let main_window = app.get_component::<MainWindowComponent>();

    // Send library selection from sidebar
    let library_id = LibraryId::new();
    sidebar.send(SidebarInput::SelectLibrary(library_id.clone()));

    // Verify main window receives navigation message
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        main_window.model().current_page,
        Page::Library(library_id)
    );
}
```

#### Worker Integration Tests
Test worker components with real async operations:

```rust
#[tokio::test]
async fn test_sync_worker_integration() {
    let db = create_test_database().await;
    let (tx, mut rx) = channel::<SyncWorkerOutput>(32);

    let worker = SyncWorker::builder()
        .detach_worker(move |msg| {
            match msg {
                SyncWorkerInput::StartSync(source_id) => {
                    // Perform sync operation
                    let result = perform_sync(&db, &source_id).await;
                    tx.send(SyncWorkerOutput::SyncComplete(result)).await;
                }
            }
        });

    worker.emit(SyncWorkerInput::StartSync(SourceId::new()));

    // Verify sync completion
    let output = rx.recv().await.unwrap();
    assert!(matches!(output, SyncWorkerOutput::SyncComplete(Ok(_))));
}
```

#### Database Transaction Tests
Test complex database operations:

```rust
#[tokio::test]
async fn test_library_deletion_cascade() {
    let db = create_test_database().await;

    // Create library with media items
    let library = create_test_library(&db).await;
    let items = create_test_media_items(&db, &library.id, 10).await;

    // Delete library
    LibraryRepository::delete(&db, &library.id).await.unwrap();

    // Verify cascade deletion
    for item in items {
        let result = MediaRepository::find_by_id(&db, &item.id).await;
        assert!(result.is_err() || result.unwrap().is_none());
    }
}
```

#### MessageBroker Tests
Test inter-component messaging:

```rust
#[tokio::test]
async fn test_message_broker_routing() {
    let broker = MessageBroker::<AppMessage>::new();

    // Register subscribers
    let mut sub1 = broker.subscribe();
    let mut sub2 = broker.subscribe();

    // Send message
    broker.send(AppMessage::MediaUpdated(MediaItemId::new()));

    // Verify all subscribers receive message
    let msg1 = sub1.recv().await.unwrap();
    let msg2 = sub2.recv().await.unwrap();
    assert_eq!(msg1, msg2);
}
```

### 3. UI Automation Tests

#### User Workflow Tests
Test complete user journeys:

```rust
#[test]
fn test_movie_playback_workflow() {
    let app = TestApp::launch();

    // Navigate to library
    app.click(".sidebar-library-item:first-child");
    app.wait_for(".library-grid");

    // Select movie
    app.click(".media-card:first-child");
    app.wait_for(".movie-details");

    // Start playback
    app.click(".play-button");
    app.wait_for(".player-view");

    // Verify player state
    assert!(app.is_visible(".player-controls"));
    assert!(app.has_class(".play-button", "playing"));
}
```

#### Responsive Layout Tests
Test adaptive UI behavior:

```rust
#[test]
fn test_responsive_breakpoints() {
    let app = TestApp::launch();

    // Test desktop layout
    app.resize_window(1920, 1080);
    assert!(app.is_visible(".sidebar"));
    assert_eq!(app.get_css_property(".media-grid", "grid-template-columns"), "repeat(6, 1fr)");

    // Test mobile layout
    app.resize_window(375, 812);
    assert!(!app.is_visible(".sidebar"));
    assert_eq!(app.get_css_property(".media-grid", "grid-template-columns"), "repeat(2, 1fr)");
}
```

### 4. Performance Tests

#### Component Render Performance
Measure render times and re-render efficiency:

```rust
#[bench]
fn bench_media_grid_render(b: &mut Bencher) {
    let items = generate_media_items(1000);
    let factory = FactoryVecDeque::<MediaCardFactory>::new();

    b.iter(|| {
        factory.guard().clear();
        for item in &items {
            factory.guard().push_back(item.clone());
        }
    });
}
```

#### Memory Usage Tests
Monitor memory consumption:

```rust
#[test]
fn test_factory_memory_efficiency() {
    let initial_mem = get_memory_usage();

    let factory = FactoryVecDeque::<MediaCardFactory>::new();
    for _ in 0..10000 {
        factory.guard().push_back(create_test_media_item());
    }

    let peak_mem = get_memory_usage();

    factory.guard().clear();
    force_gc();

    let final_mem = get_memory_usage();

    // Memory should be reclaimed after clearing
    assert!(final_mem - initial_mem < (peak_mem - initial_mem) * 0.1);
}
```

## Test Infrastructure

### Test Database Setup
```rust
pub async fn create_test_database() -> Arc<DatabaseConnection> {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    migration::Migrator::up(&db, None).await.unwrap();
    Arc::new(db)
}

pub async fn seed_test_data(db: &DatabaseConnection) {
    // Add test sources, libraries, and media items
}
```

### Test Component Builders
```rust
pub struct TestComponentBuilder<C: Component> {
    model: C::Init,
    parent: Option<gtk::Window>,
}

impl<C: Component> TestComponentBuilder<C> {
    pub fn with_model(mut self, model: C::Init) -> Self {
        self.model = model;
        self
    }

    pub fn build(self) -> TestComponent<C> {
        // Initialize component with test harness
    }
}
```

### Mock Services
```rust
pub struct MockBackend {
    responses: HashMap<String, serde_json::Value>,
}

#[async_trait]
impl MediaBackend for MockBackend {
    async fn get_libraries(&self) -> Result<Vec<Library>> {
        Ok(serde_json::from_value(
            self.responses.get("libraries").unwrap().clone()
        )?)
    }
}
```

## Testing Best Practices

### 1. Component Testing Guidelines
- Test each component's input/output contract
- Verify tracker patterns minimize updates
- Test error states and edge cases
- Ensure proper cleanup in Drop implementations

### 2. Async Testing Guidelines
- Use `tokio::test` for async tests
- Set appropriate timeouts for async operations
- Test cancellation and error scenarios
- Verify proper resource cleanup

### 3. Factory Testing Guidelines
- Test item addition/removal/updates
- Verify efficient rendering with large datasets
- Test virtual scrolling behavior
- Ensure proper memory management

### 4. Worker Testing Guidelines
- Test message handling and routing
- Verify background task cancellation
- Test error propagation
- Ensure thread safety

### 5. Database Testing Guidelines
- Use in-memory databases for speed
- Test transactions and rollbacks
- Verify cascade operations
- Test concurrent access patterns

## Test Organization

### Directory Structure
```
tests/
├── unit/
│   ├── components/
│   │   ├── sidebar_test.rs
│   │   ├── main_window_test.rs
│   │   └── factories/
│   │       └── media_card_test.rs
│   ├── services/
│   │   ├── media_service_test.rs
│   │   └── auth_service_test.rs
│   ├── commands/
│   │   └── media_commands_test.rs
│   └── workers/
│       └── sync_worker_test.rs
├── integration/
│   ├── navigation_test.rs
│   ├── playback_flow_test.rs
│   └── sync_flow_test.rs
├── ui/
│   ├── workflows/
│   │   └── movie_playback_test.rs
│   └── responsive/
│       └── layout_test.rs
└── performance/
    ├── render_bench.rs
    └── memory_bench.rs
```

### Test Naming Conventions
- Unit tests: `test_<component>_<behavior>`
- Integration tests: `test_<feature>_integration`
- UI tests: `test_<workflow>_e2e`
- Benchmarks: `bench_<operation>`

## Continuous Integration

### Test Pipeline
```yaml
test:
  stage: test
  script:
    - cargo fmt --check
    - cargo clippy -- -D warnings
    - cargo test --all-features
    - cargo test --no-default-features
    - cargo bench --no-run
```

### Coverage Requirements
- Minimum 80% overall coverage
- 90% coverage for critical paths (commands, services)
- 70% coverage for UI components
- 100% coverage for type conversions and validators

## Test Data Management

### Fixtures
```rust
pub mod fixtures {
    pub fn movie_fixture() -> MediaItemModel {
        MediaItemModel {
            id: MediaItemId::from_str("test-movie-1").unwrap(),
            title: "Test Movie".to_string(),
            media_type: MediaType::Movie,
            // ...
        }
    }

    pub fn library_fixture() -> LibraryModel {
        LibraryModel {
            id: LibraryId::from_str("test-library-1").unwrap(),
            name: "Test Library".to_string(),
            library_type: LibraryType::Movies,
            // ...
        }
    }
}
```

### Test Data Builders
```rust
pub struct MediaItemBuilder {
    item: MediaItemModel,
}

impl MediaItemBuilder {
    pub fn new() -> Self {
        Self {
            item: MediaItemModel::default(),
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.item.title = title.into();
        self
    }

    pub fn with_watched(mut self) -> Self {
        self.item.watched = true;
        self
    }

    pub fn build(self) -> MediaItemModel {
        self.item
    }
}
```

## Debugging Tests

### Test Logging
```rust
#[test]
fn test_with_logging() {
    env_logger::init();

    log::debug!("Starting test");
    // Test implementation
    log::debug!("Test completed");
}
```

### Visual Test Debugging
```rust
#[test]
fn test_with_visual_debugging() {
    let app = TestApp::launch()
        .with_visual_mode(true)  // Shows actual window
        .with_slow_mode(true);   // Slows down interactions

    // Test implementation
}
```

## Common Test Patterns

### Testing State Transitions
```rust
#[test]
fn test_player_state_transitions() {
    let player = TestComponent::<PlayerComponent>::new();

    // Initial state
    assert_eq!(player.model().state, PlayerState::Stopped);

    // Play transition
    player.send(PlayerInput::Play);
    assert_eq!(player.model().state, PlayerState::Playing);

    // Pause transition
    player.send(PlayerInput::Pause);
    assert_eq!(player.model().state, PlayerState::Paused);
}
```

### Testing Error Handling
```rust
#[tokio::test]
async fn test_network_error_handling() {
    let backend = MockBackend::new()
        .with_error("network_error");

    let result = backend.get_libraries().await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        AppError::Network(_)
    ));
}
```

### Testing Concurrent Operations
```rust
#[tokio::test]
async fn test_concurrent_sync_operations() {
    let db = create_test_database().await;

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let db = db.clone();
            tokio::spawn(async move {
                SyncService::sync_source(&db, &SourceId::new()).await
            })
        })
        .collect();

    let results = futures::future::join_all(handles).await;

    // All operations should complete without conflicts
    for result in results {
        assert!(result.unwrap().is_ok());
    }
}
```

## Test Maintenance

### Regular Test Audits
- Review and update tests when architecture changes
- Remove redundant tests
- Add tests for new features
- Update test data and fixtures

### Test Documentation
- Document complex test scenarios
- Explain non-obvious assertions
- Link tests to requirements/issues
- Keep test names descriptive

### Test Performance
- Monitor test execution time
- Parallelize independent tests
- Use test filtering for development
- Cache test dependencies

## Conclusion

This testing strategy ensures comprehensive coverage of the Relm4 application architecture. By following these guidelines and patterns, we maintain high code quality, catch regressions early, and ensure a reliable user experience. The component-based testing approach aligns perfectly with Relm4's architecture, making tests maintainable and easy to understand.