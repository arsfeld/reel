# Swift-Rust Bridge Implementation Guide

This document describes the Swift-Rust bridge that enables the native macOS frontend to communicate with Reel's Rust core. The bridge uses [swift-bridge](https://github.com/chinedufn/swift-bridge) for safe, efficient interoperability.

## Quick Start

### Building the Bridge

```bash
# Enter development environment
nix develop

# Build with Swift feature
cargo build --release --features swift

# Generated Swift files will be in:
# target/debug/build/reel-*/out/
```

### Using from Swift

```swift
let core = MacOSCore()
if core.sb_initialize() {
    let backends = core.list_backends()
    for backend in backends {
        print("Backend: \(backend.get_name())")
    }
}
```

## Architecture

The bridge follows a thin, stable interface principle with three layers:

```
Swift UI → Bridge (Bridge Models) → Rust Core (Full Models)
```

- **Swift UI**: Native macOS views and controllers
- **Bridge**: Lightweight data transfer objects and opaque handles
- **Rust Core**: EventBus, DataService, ViewModels, and business logic

### Key Design Principles

1. **Single Global Runtime**: One Tokio runtime for all async operations
2. **Opaque Handles**: Rust owns complex state, Swift holds opaque references
3. **Bridge Models**: Simple, stable structs for data transfer
4. **Polling First**: Event subscription uses polling, callbacks added later if needed
5. **Progressive Enhancement**: Start simple, add complexity only when proven necessary

## API Reference

### Core Operations

#### Initialization

```rust
// Create a new core instance
fn new() -> MacOSCore

// Initialize the core (blocking)
fn sb_initialize(&mut self) -> bool

// Check initialization status
fn sb_is_initialized(&self) -> bool

// Get build information
fn get_build_info() -> String
```

#### Data Access

```rust
// List all configured backends
fn list_backends(&self) -> Vec<BackendBridge>

// Get cached libraries for a backend
fn get_cached_libraries(&self, backend_id: String) -> Vec<LibraryBridge>

// Get media items from a library
fn get_library_items(&self, library_id: String, limit: u32, offset: u32) -> Vec<MediaItemBridge>

// Get playback progress for an item
fn get_playback_progress(&self, item_id: String) -> Option<PlaybackProgressBridge>
```

#### Event Subscription

```rust
// Subscribe to event types
fn subscribe(&self, event_kinds: Vec<String>) -> EventSub

// Poll for next event (with timeout)
fn next_event_blocking(&self, timeout_ms: u32) -> Option<EventBridge>

// Unsubscribe from events
fn unsubscribe(self)
```

#### Playback Control (Future)

```rust
// Start playing a media item
fn play_item(&self, item_id: String) -> ResultBridge<()>

// Pause playback
fn pause(&self) -> bool

// Seek to position
fn seek(&self, position_ms: u64) -> bool

// Get current playback state
fn get_playback_state(&self) -> Option<PlaybackBridge>
```

## Data Models

### Bridge Models

All data crossing the bridge uses "Bridge" models - simple, stable structs with primitive types:

```rust
pub struct BackendBridge {
    pub id: String,
    pub name: String,
    pub kind: String,  // "plex", "jellyfin", "local"
}

pub struct LibraryBridge {
    pub id: String,
    pub name: String,
    pub item_count: u32,
}

pub struct MediaItemBridge {
    pub id: String,
    pub title: String,
    pub year: Option<u32>,
    pub duration_ms: Option<u64>,
    pub rating: Option<f32>,
    pub poster_url: Option<String>,
}

pub struct EventBridge {
    pub kind: String,  // "MediaCreated", "MediaUpdated", etc.
    pub entity_id: String,
    pub payload_json: Option<String>,
}

pub struct ErrorBridge {
    pub code: String,
    pub message: String,
}
```

### Model Evolution

- Add new fields as `Option<T>` to maintain backward compatibility
- Never remove or rename existing fields
- Use versioning comments to track additions

## Event System

### Subscription Flow

1. Swift subscribes to event types
2. Rust creates a receiver channel
3. Swift polls for events periodically
4. Events are serialized as `EventBridge`

### Event Types

- `MediaCreated` - New media item added
- `MediaUpdated` - Media item metadata changed
- `MediaBatchCreated` - Multiple items added
- `PlaybackProgress` - Playback position updated
- `SyncStarted` - Synchronization began
- `SyncCompleted` - Synchronization finished

### Swift Usage Example

```swift
class EventListener {
    private let core: MacOSCore
    private let subscription: EventSub
    
    init(core: MacOSCore) {
        self.core = core
        self.subscription = core.subscribe(["MediaCreated", "MediaUpdated"])
        startListening()
    }
    
    private func startListening() {
        Task {
            while !Task.isCancelled {
                if let event = subscription.next_event_blocking(100) {
                    handleEvent(event)
                }
            }
        }
    }
    
    private func handleEvent(_ event: EventBridge) {
        switch event.kind {
        case "MediaCreated":
            // Handle new media
        case "MediaUpdated":
            // Handle updated media
        default:
            break
        }
    }
}
```

## Error Handling

### Error Codes

All errors map to stable codes:

- `InitFailed` - Core initialization failed
- `NotAuthenticated` - Authentication required
- `Network` - Network operation failed
- `Db` - Database operation failed
- `NotFound` - Resource not found
- `InvalidArg` - Invalid argument provided
- `Timeout` - Operation timed out
- `Unknown` - Unexpected error

### Result Type

Operations that can fail return `ResultBridge<T>`:

```rust
pub enum ResultBridge<T> {
    Ok(T),
    Err(ErrorBridge),
}
```

## Runtime Management

The bridge uses a single global Tokio runtime for all async operations:

```rust
static RUNTIME: Lazy<Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("reel-bridge")
        .enable_all()
        .build()
        .expect("Failed to create runtime")
});
```

This ensures:
- No runtime duplication
- Consistent async execution
- Predictable resource usage

## Memory Management

### Ownership Rules

1. **Rust owns complex state** - Core objects live in Rust
2. **Swift holds opaque handles** - References to Rust objects
3. **Data is copied** - Bridge models are value types
4. **Automatic cleanup** - swift-bridge handles deallocation

### Best Practices

- Never pass raw pointers across the boundary
- Use opaque types for stateful objects
- Copy data into Bridge models for transfer
- Let swift-bridge manage lifetimes

## Build Configuration

### Cargo.toml

```toml
[lib]
crate-type = ["cdylib", "rlib"]

[features]
swift = ["dep:swift-bridge", "dep:once_cell"]

[target.'cfg(feature = "swift")'.dependencies]
swift-bridge = "0.1"
once_cell = "1.19"
```

### build.rs

```rust
#[cfg(feature = "swift")]
fn compile_swift_resources() {
    let bridges = vec!["src/platforms/macos/bridge.rs"];
    swift_bridge_build::parse_bridges(bridges, Default::default());
}
```

### Xcode Integration

**Note**: The macOS build system has been fully automated using XcodeGen. See `macos/README.md` for details.

Automated build process:
1. XcodeGen generates the Xcode project from `macos/project.yml`
2. Build phases automatically compile Rust library and copy Swift bridge files
3. The dylib is linked and embedded in the app bundle
4. No manual Xcode configuration required - just run `make build` in the `macos/` directory

For manual integration:
1. Add build phase to run: `cargo build --release --features swift`
2. Link `target/release/libreel_ffi.dylib`
3. Include generated Swift files from `OUT_DIR`
4. Set library search paths

## Testing

### Rust Tests

```rust
#[test]
fn test_bridge_initialization() {
    let mut core = MacOSCore::new();
    assert!(core.sb_initialize());
    assert!(core.sb_is_initialized());
}

#[test]
fn test_backend_listing() {
    let core = setup_test_core();
    let backends = core.list_backends();
    assert!(!backends.is_empty());
}
```

### Swift Tests

```swift
func testCoreLifecycle() {
    let core = MacOSCore()
    XCTAssertTrue(core.sb_initialize())
    XCTAssertTrue(core.sb_is_initialized())
    
    let info = get_build_info()
    XCTAssertTrue(info.contains("Reel"))
}
```

## Performance Considerations

### Optimization Strategies

1. **Batch Operations** - Reduce FFI crossings
2. **Lazy Loading** - Fetch data on demand
3. **Caching** - Cache frequently accessed data in Swift
4. **Async Operations** - Never block the main thread

### Profiling

Monitor:
- FFI call frequency
- Data serialization overhead
- Memory allocations
- Event queue depth

## Troubleshooting

### Common Issues

**Bridge doesn't compile**
- Ensure swift-bridge is in dependencies
- Check build.rs is running
- Verify feature flags are set

**Runtime panics**
- Check single runtime is initialized
- Verify core is initialized before use
- Ensure proper error handling

**Memory leaks**
- Verify opaque types are properly released
- Check for circular references
- Use Instruments to profile

**Events not received**
- Verify subscription is active
- Check polling frequency
- Monitor event queue size

## Migration Path

### Current Status

- ✅ Phase A: Core initialization
- ✅ Phase B: Basic data access
- 🚧 Phase C: Event subscription
- ⏳ Phase D: Error handling
- ⏳ Phase E: Playback/Auth/Sync

### Next Steps

1. Complete event subscription implementation
2. Add comprehensive error handling
3. Implement playback controls
4. Add authentication flows
5. Enable sync operations

## API Stability

### Versioning Policy

- Bridge models are stable within major versions
- New fields added as `Option<T>`
- Deprecation notices for 2 releases minimum
- Breaking changes only in major versions

### Compatibility

The bridge maintains compatibility with:
- Swift 5.5+
- macOS 11.0+
- Rust 1.70+

## Resources

- [swift-bridge Documentation](https://github.com/chinedufn/swift-bridge)
- [Tokio Async Runtime](https://tokio.rs)
- [Reel Architecture Overview](./architecture.md)
- [macOS Frontend Plan](./macos.md)

## Contributing

When adding new bridge functionality:

1. Add Rust implementation in `src/platforms/macos/bridge/`
2. Update swift-bridge module definition
3. Add Bridge model if needed
4. Write Rust unit tests
5. Document in this file
6. Test from Swift side

## License

See the main project LICENSE file.