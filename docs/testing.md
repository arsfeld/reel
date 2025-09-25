# Testing Strategy for Reel Media Player

## Overview

This document outlines the testing strategy for the Reel media player application. The testing approach focuses on practical, maintainable tests that verify core functionality, backend integrations, and database operations.

## Testing Philosophy

### Core Principles
1. **Practical Testing**: Focus on tests that provide real value and catch actual bugs
2. **Backend Coverage**: Comprehensive testing of Plex and Jellyfin integrations
3. **Database Testing**: SQLite with SeaORM for repository layer testing
4. **Mock-Based Testing**: Use mock backends for isolated component testing
5. **Async Testing**: Proper async/await testing with tokio::test
6. **Type Safety**: Leverage Rust's type system to prevent runtime errors

### Current Test Coverage
```
Backend Tests     <- Plex/Jellyfin API mocking (40%)
Database Tests    <- Repository layer tests (30%)
Mapper Tests      <- Data transformation tests (20%)
Worker Tests      <- Connection monitor, search (10%)
```

## Test Categories

### 1. Unit Tests

#### Backend API Tests
Test backend integrations with mocked HTTP responses:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use serde_json::json;

    #[tokio::test]
    async fn test_plex_get_libraries() {
        let mut server = Server::new_async().await;
        
        let mock = server.mock("GET", "/library/sections")
            .with_header("X-Plex-Token", "test_token")
            .with_body(json!({
                "MediaContainer": {
                    "Directory": [{
                        "key": "1",
                        "title": "Movies",
                        "type": "movie"
                    }]
                }
            }).to_string())
            .create_async().await;

        let backend = PlexBackend::new_for_auth(&server.url(), "test_token");
        let libraries = backend.get_libraries().await.unwrap();
        
        assert_eq!(libraries.len(), 1);
        assert_eq!(libraries[0].title, "Movies");
        mock.assert_async().await;
    }
}
```

#### Database Repository Tests
Test repository layer with SQLite:

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

#### Sync Strategy Tests
Test synchronization strategies for backends:

```rust
#[tokio::test]
async fn test_full_sync_strategy() {
    let db = create_test_database().await;
    let backend = MockBackend::new();
    
    // Add test data to mock backend
    backend.add_library(create_test_library());
    backend.add_movie(create_test_movie());
    
    // Run full sync
    let strategy = FullSyncStrategy::new();
    let result = strategy.sync(&db, &backend, &SourceId::new()).await;
    
    assert!(result.is_ok());
    
    // Verify data was synced to database
    let libraries = LibraryRepository::find_by_source(&db, &SourceId::new()).await.unwrap();
    assert_eq!(libraries.len(), 1);
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

#### Mapper Tests
Test data transformation between backend responses and internal models:

```rust
#[tokio::test]
async fn test_plex_movie_mapping() {
    let plex_response = json!({
        "ratingKey": "123",
        "title": "Test Movie",
        "year": 2024,
        "rating": 8.5,
        "duration": 7200000,
        "summary": "A test movie"
    });
    
    let movie: Movie = plex_mapper::map_movie(plex_response).unwrap();
    
    assert_eq!(movie.id.as_str(), "123");
    assert_eq!(movie.title, "Test Movie");
    assert_eq!(movie.year, Some(2024));
    assert_eq!(movie.rating, Some(8.5));
    assert_eq!(movie.duration_minutes, Some(120));
}
```



## Test Infrastructure

### Test Database Setup
```rust
// Located in src/test_utils.rs
pub struct TestDatabase {
    pub connection: Arc<SeaOrmConnection>,
    _temp_dir: TempDir,
}

impl TestDatabase {
    pub async fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");
        let db = Database::connect(&db_path).await?;
        db.migrate().await?;
        
        Ok(Self {
            connection: db.get_connection(),
            _temp_dir: temp_dir,
        })
    }
}
```

### Mock Backend Implementation
```rust
// Located in src/test_utils.rs
pub struct MockBackend {
    responses: HashMap<String, serde_json::Value>,
    error_mode: Option<ErrorMode>,
    delay: Option<Duration>,
}

impl MockBackend {
    pub fn with_error(mut self, error: ErrorMode) -> Self {
        self.error_mode = Some(error);
        self
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }
}

#[async_trait]
impl MediaBackend for MockBackend {
    async fn get_libraries(&self) -> Result<Vec<Library>> {
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }

        if let Some(error) = &self.error_mode {
            return Err(error.to_anyhow());
        }

        Ok(serde_json::from_value(
            self.responses.get("libraries").unwrap().clone()
        )?)
    }

    // All MediaBackend methods implemented with mock responses
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

### Current Test Structure
Tests are currently embedded within the source modules using `#[cfg(test)]`:
```
src/
├── backends/
│   ├── jellyfin/tests.rs   # Jellyfin backend tests
│   ├── plex/tests.rs        # Plex backend tests
│   └── sync_strategy.rs    # Sync strategy tests
├── db/
│   └── repository/
│       └── sync_repository_tests.rs  # Repository tests
├── mapper/
│   └── tests.rs             # Data mapper tests
├── workers/
│   ├── connection_monitor_tests.rs   # Connection monitor tests
│   └── search_worker_tests.rs        # Search worker tests
└── test_utils.rs            # Shared test utilities
```

### Test Naming Conventions
- Unit tests: `test_<component>_<behavior>`
- Integration tests: `test_<feature>_integration`
- UI tests: `test_<workflow>_e2e`
- Benchmarks: `bench_<operation>`

## Continuous Integration

### GitHub Actions CI Pipeline
The project uses GitHub Actions for CI/CD with the following workflow:

```yaml
# .github/workflows/ci.yml
- Install system dependencies (GTK4, libadwaita, GStreamer, MPV, SQLite)
- Setup Rust toolchain with rustfmt and clippy
- Cache cargo dependencies for faster builds
- Build release version
- Run tests with cargo test --release
- Generate coverage report with cargo-llvm-cov
```

### Test Execution
- Tests run on Ubuntu latest
- Parallel test execution with all available cores
- Coverage generation using llvm-cov for performance
- Test files, migrations, and generated code excluded from coverage

## Test Helpers

### Common Test Utilities
The `src/test_utils.rs` module provides:
- `TestDatabase`: Temporary SQLite database for testing
- `MockBackend`: In-memory mock for `MediaBackend` trait
- Helper functions for async testing with timeouts
- Test data creation utilities

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



## Common Test Patterns



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

## Areas Needing Test Coverage

### Missing Test Coverage
1. **UI Components**: No Relm4 component tests currently exist
2. **Service Layer**: Core service functions lack unit tests  
3. **Command Pattern**: No tests for command execution
4. **MessageBroker**: Inter-component communication untested
5. **Player Integration**: MPV and GStreamer backends lack tests
6. **Authentication Flow**: OAuth and credential management untested

### Recommended Testing Priorities
1. Add integration tests for critical user workflows
2. Test database migration and schema changes
3. Add performance benchmarks for large libraries
4. Test error recovery and retry logic
5. Add tests for concurrent sync operations
6. Test offline mode and cache invalidation

## Testing Best Practices

### Writing Effective Tests
- Use descriptive test names that explain the scenario
- Keep tests focused on a single behavior
- Use mocks to isolate components under test
- Test both success and failure cases
- Verify error messages are helpful

### Running Tests
```bash
# Run all tests
cargo test

# Run tests for a specific module
cargo test backends::plex

# Run tests with output
cargo test -- --nocapture

# Run a specific test
cargo test test_plex_get_libraries
```

## Conclusion

The Reel media player currently has foundational test coverage for backend integrations, database operations, and data mapping. While the testing infrastructure is in place, significant gaps exist in UI component testing, service layer testing, and integration testing. Future development should prioritize adding tests for critical user workflows and improving coverage of the service and command layers.