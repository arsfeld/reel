# macOS Frontend Implementation Plan

## Overview
This plan details how to add a native macOS frontend to Reel while maximizing code reuse from the existing GTK frontend. The core services (EventBus, DataService, SyncManager) will remain unchanged and serve both frontends.

## Architecture Strategy

### 1. Core Platform-Agnostic Layer (Already Exists - No Changes Needed)
These components work perfectly for both frontends:

- **EventBus** (`src/events/event_bus.rs`) - Reactive event system
- **DataService** (`src/services/data.rs`) - Database and caching
- **SyncManager** (`src/services/sync.rs`) - Background synchronization
- **BackendManager** (`src/backends/`) - Media server integrations
- **AuthManager** (`src/services/auth_manager.rs`) - Authentication
- **Database** (`src/db/`) - SQLite storage layer
- **Models** (`src/models/`) - Domain models

### 2. Frontend Abstraction Layer (New)

Create a platform abstraction to isolate UI-specific code:

```
src/
├── core/                    # Platform-agnostic core (refactored from existing)
│   ├── state.rs            # Extract non-GTK parts from app_state.rs
│   ├── coordinator.rs      # Platform-agnostic coordination logic
│   └── mod.rs
├── frontends/              # New frontend abstraction
│   ├── mod.rs
│   ├── gtk/                # Move existing GTK code here
│   │   ├── app.rs         # Current app.rs
│   │   ├── main.rs        # GTK-specific main
│   │   ├── ui/            # Current ui/ directory
│   │   └── mod.rs
│   └── macos/             # New macOS frontend
│       ├── app.rs         # SwiftUI app wrapper
│       ├── main.rs        # macOS-specific main
│       ├── ui/            # SwiftUI views
│       ├── bridge.rs      # Swift-Rust bridge
│       └── mod.rs
```

### 3. Shared ViewModels Pattern

Create platform-agnostic ViewModels that both frontends can use:

```rust
// src/core/viewmodels/library_view_model.rs
pub struct LibraryViewModel {
    event_bus: Arc<EventBus>,
    data_service: Arc<DataService>,
    items: Arc<RwLock<Vec<MediaItem>>>,
}

impl LibraryViewModel {
    pub fn subscribe_to_updates(&self) -> EventSubscriber {
        self.event_bus.subscribe_to_types(vec![
            EventType::MediaCreated,
            EventType::MediaUpdated,
            EventType::MediaBatchCreated,
        ])
    }
    
    pub async fn refresh(&self) { /* ... */ }
    pub async fn get_items(&self) -> Vec<MediaItem> { /* ... */ }
}
```

## Implementation Phases

### Phase 1: Core Refactoring (Minimal Changes)
**Goal**: Extract platform-agnostic code without breaking GTK frontend

1. **Create `src/core/` module**:
   - Move non-GTK logic from `AppState` to `CoreState`
   - Extract business logic from UI components to ViewModels
   - Keep all existing functionality intact

2. **Introduce Frontend Trait**:
```rust
// src/frontends/mod.rs
#[async_trait]
pub trait Frontend: Send + Sync {
    async fn initialize(&self, core_state: Arc<CoreState>) -> Result<()>;
    async fn run(&self) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;
}
```

3. **Move GTK code to `src/frontends/gtk/`**:
   - Simple file moves, update import paths
   - Ensure GTK frontend still compiles and runs

### Phase 2: macOS Frontend Foundation
**Goal**: Basic macOS app that uses the EventBus and DataService

1. **Setup Swift-Rust Bridge**:
```rust
// src/frontends/macos/bridge.rs
use crate::core::CoreState;
use crate::events::{EventBus, EventSubscriber};

#[swift_bridge::bridge]
mod ffi {
    extern "Rust" {
        type CoreState;
        type EventSubscriber;
        
        fn initialize_core() -> Result<Arc<CoreState>>;
        fn subscribe_to_events(core: &CoreState) -> EventSubscriber;
        fn fetch_libraries(core: &CoreState) -> Vec<Library>;
    }
}
```

2. **Create SwiftUI App Structure**:
```swift
// macos/ReelApp/ReelApp.swift
import SwiftUI

@main
struct ReelApp: App {
    @StateObject private var appModel = AppModel()
    
    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(appModel)
        }
    }
}

// macos/ReelApp/Models/AppModel.swift
class AppModel: ObservableObject {
    private let core: OpaquePointer
    private var eventSubscriber: EventSubscriber?
    
    init() {
        self.core = initialize_core()
        self.eventSubscriber = subscribe_to_events(core)
        startEventListener()
    }
    
    private func startEventListener() {
        Task {
            while let event = await eventSubscriber?.recv() {
                await handleEvent(event)
            }
        }
    }
}
```

### Phase 3: Feature Parity Implementation
**Goal**: Implement all major features in macOS frontend

1. **Library Browser**:
   - SwiftUI views for library grid/list
   - Connect to LibraryViewModel
   - Handle MediaBatchCreated events for updates

2. **Media Player**:
   - AVPlayer integration for video playback
   - Connect to PlaybackProgress events
   - Sync position with DataService

3. **Settings & Authentication**:
   - Native macOS preferences window
   - Keychain integration for credentials
   - Backend configuration UI

### Phase 4: Platform-Specific Enhancements

1. **macOS-Specific Features**:
   - Menu bar integration
   - Touch Bar support
   - Handoff/Continuity
   - Quick Look previews
   - Spotlight integration

2. **Performance Optimizations**:
   - Native SwiftUI lazy loading
   - Core Animation for transitions
   - Metal for video rendering

## Required Architectural Modifications

### 1. Conditional Compilation
```toml
# Cargo.toml
[features]
default = ["gtk"]
gtk = ["gtk4", "libadwaita", "gtk4-macros"]
macos = ["swift-bridge", "objc", "cocoa"]

[target.'cfg(target_os = "macos")'.dependencies]
swift-bridge = "0.1"
objc = "0.2"
cocoa = "0.24"
```

### 2. Main Entry Point Refactoring
```rust
// src/main.rs
#[cfg(feature = "gtk")]
mod frontends_gtk {
    pub use crate::frontends::gtk::main as gtk_main;
}

#[cfg(feature = "macos")]
mod frontends_macos {
    pub use crate::frontends::macos::main as macos_main;
}

fn main() -> Result<()> {
    #[cfg(feature = "gtk")]
    frontends_gtk::gtk_main()?;
    
    #[cfg(feature = "macos")]
    frontends_macos::macos_main()?;
    
    Ok(())
}
```

### 3. Event Bus Extensions
No changes needed! The existing EventBus is perfect for cross-platform use.

### 4. DataService Adjustments
Minor additions for platform-specific paths:
```rust
impl DataService {
    pub fn get_cache_dir() -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            dirs::cache_dir()
                .unwrap()
                .join("dev.arsfeld.Reel")
        }
        #[cfg(not(target_os = "macos"))]
        {
            // existing Linux/GTK path
        }
    }
}
```

## Build System Changes

### 1. Workspace Structure
```toml
# Cargo.toml (root)
[workspace]
members = [
    "reel-core",      # Platform-agnostic core
    "reel-gtk",       # GTK frontend
    "reel-macos",     # macOS frontend
]

[workspace.dependencies]
reel-core = { path = "reel-core" }
```

### 2. macOS Build Setup
```yaml
# .github/workflows/macos.yml
- name: Build macOS App
  run: |
    cargo build --release --features macos
    swift build -c release
    
- name: Create App Bundle
  run: |
    mkdir -p Reel.app/Contents/MacOS
    cp target/release/reel Reel.app/Contents/MacOS/
    cp -r macos/Resources Reel.app/Contents/
```

### 3. Xcode Project
Create `macos/Reel.xcodeproj` for:
- SwiftUI views development
- Asset management
- Code signing
- App Store distribution

## Testing Strategy

### 1. Shared Core Tests
All existing tests for EventBus, DataService, and SyncManager remain valid.

### 2. Platform-Specific UI Tests
- GTK: Continue using existing GTK test infrastructure
- macOS: XCTest for SwiftUI views and integration

### 3. Cross-Platform Integration Tests
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_event_bus_cross_platform() {
        // Test that events work identically on both platforms
    }
    
    #[test]
    fn test_data_service_consistency() {
        // Ensure data operations are identical
    }
}
```

## Migration Path

### Step 1: Prepare (Week 1)
- Create feature flags in Cargo.toml
- Set up workspace structure
- Move GTK code to frontends/gtk/ (no functional changes)

### Step 2: Extract Core (Week 2)
- Create CoreState from AppState
- Extract ViewModels from UI components
- Ensure GTK frontend still works

### Step 3: macOS Foundation (Week 3-4)
- Set up Swift-Rust bridge
- Create basic SwiftUI app
- Connect to EventBus and DataService

### Step 4: Feature Implementation (Week 5-8)
- Implement library browser
- Add media playback
- Create settings UI

### Step 5: Polish (Week 9-10)
- Platform-specific features
- Performance optimization
- Testing and bug fixes

## Benefits of This Approach

1. **Maximum Code Reuse**: 80% of code (all services, backends, models) is shared
2. **Consistent Behavior**: Same EventBus ensures identical sync and update behavior
3. **Parallel Development**: Both frontends can be developed/maintained independently
4. **Native Experience**: Each platform gets native UI with platform-specific features
5. **Maintainability**: Business logic in one place, only UI is duplicated
6. **Testability**: Core logic can be tested once for both platforms

## Potential Challenges

1. **Swift-Rust Bridge Complexity**: Needs careful type mapping
2. **Event Serialization**: Events need to cross language boundary
3. **Build System**: Managing multi-platform builds
4. **Dependencies**: Some crates may not work on macOS
5. **Debugging**: Cross-language debugging can be challenging

## Alternative Approaches Considered

1. **Tauri**: Web-based UI (rejected: not native enough)
2. **Qt**: Cross-platform C++ (rejected: not Rust-native)
3. **Iced**: Pure Rust GUI (rejected: not mature enough for media apps)
4. **MAUI**: .NET based (rejected: not Rust)

## Conclusion

This plan provides a clear path to add a native macOS frontend while preserving the excellent architecture already in place. The EventBus, DataService, and SyncManager require zero changes and will power both frontends equally well. The main work is creating SwiftUI views and connecting them to the existing event-driven backend.