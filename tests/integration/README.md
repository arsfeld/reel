# Integration Tests

This directory contains integration tests for the Reel media player that test the complete stack from backend to database.

## Overview

Integration tests verify:
- Authentication with Plex and Jellyfin servers
- Library discovery and sync
- Media item fetching
- Playback progress tracking
- Error handling scenarios

## Test Structure

```
tests/integration/
├── common/          # Shared test utilities and database helpers
├── fixtures/        # Test data fixtures (movies, shows, episodes)
├── plex/           # Plex integration tests
├── jellyfin/       # Jellyfin integration tests
└── README.md       # This file
```

## Running Tests

```bash
# Run all integration tests
cargo test --test integration_tests

# Run specific backend tests
cargo test --test integration_tests plex
cargo test --test integration_tests jellyfin

# Run with output
cargo test --test integration_tests -- --nocapture
```

## Test Approach

### Mocking with Mockito

The integration tests use [mockito](https://docs.rs/mockito) to create mock HTTP servers that simulate Plex and Jellyfin responses. This provides:

- **Fast execution**: No need to spin up real servers
- **Reliability**: Deterministic, repeatable tests
- **CI/CD friendly**: No external dependencies

### Test Flow

1. Create test database with migrations
2. Start mock HTTP server (mockito)
3. Create backend with mock server URL
4. Execute backend operations
5. Verify data in database
6. Check error handling

## Docker-Based E2E Testing (Optional)

For true end-to-end testing with real Plex/Jellyfin servers:

1. **Using Testcontainers**: The `testcontainers` crate is included for spinning up Docker containers

```rust
use testcontainers::{GenericImage, runners::AsyncRunner};

let plex = GenericImage::new("linuxserver/plex", "latest")
    .with_exposed_port(32400.into())
    .start()
    .await?;

let url = format!("http://localhost:{}", plex.get_host_port_ipv4(32400).await?);
```

2. **Using Docker Compose**: Create a `docker-compose.test.yml`:

```yaml
version: '3.8'
services:
  plex:
    image: linuxserver/plex:latest
    ports:
      - "32400:32400"
    environment:
      - PLEX_CLAIM=claim-test

  jellyfin:
    image: linuxserver/jellyfin:latest
    ports:
      - "8096:8096"
```

## Test Fixtures

Test fixtures provide sample data:

```rust
use crate::fixtures::*;

// Sample media
let movie = sample_movie_1();        // Test movie with metadata
let show = sample_show_1();          // Test TV show
let episode = sample_episode_1();    // Test episode

// Sample libraries
let movie_lib = sample_movie_library();
let tv_lib = sample_tv_library();

// Sample user and stream info
let user = sample_user();
let stream = sample_stream_info();
```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Integration Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      - name: Run integration tests
        run: cargo test --test integration_tests
```

### With Docker (Optional)

```yaml
      - name: Set up Docker
        uses: docker/setup-buildx-action@v2

      - name: Start test servers
        run: docker-compose -f docker-compose.test.yml up -d

      - name: Run E2E tests
        run: cargo test --test integration_tests -- --ignored

      - name: Stop test servers
        run: docker-compose -f docker-compose.test.yml down
```

## Adding New Tests

1. **Create test file** in appropriate backend directory
2. **Import test utilities**: `use crate::common::TestDb;`
3. **Use fixtures**: `use crate::fixtures::*;`
4. **Follow existing patterns**: See `plex/auth_and_sync.rs` for examples

Example test:

```rust
#[tokio::test]
async fn test_my_integration() {
    let db = TestDb::new().await.unwrap();
    let mut server = mockito::Server::new_async().await;

    let _mock = server
        .mock("GET", "/api/endpoint")
        .with_status(200)
        .with_body(r#"{"data": "test"}"#)
        .create_async()
        .await;

    // Test your integration flow...
}
```

## Known Limitations

- Mock servers simulate API responses but not actual media streaming
- Database tests use SQLite in-memory which may differ from production
- Network error simulations are limited to HTTP status codes

## Future Enhancements

- [ ] Add Docker-based E2E tests with real servers
- [ ] Test media streaming and transcoding
- [ ] Test concurrent access scenarios
- [ ] Performance benchmarks
- [ ] Integration with test media files
