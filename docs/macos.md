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
swift = ["swift-bridge", "objc", "cocoa"]
cocoa = ["objc2", "objc2-foundation", "objc2-app-kit", "block2"]

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

#[cfg(feature = "swift")]
mod frontends_swift {
    pub use crate::frontends::macos::main as macos_main;
}

#[cfg(feature = "cocoa")]
mod frontends_cocoa {
    pub use crate::frontends::cocoa::main as cocoa_main;
}

fn main() -> Result<()> {
    #[cfg(feature = "gtk")]
    frontends_gtk::gtk_main()?;
    
    #[cfg(feature = "swift")]
    frontends_swift::macos_main()?;
    
    #[cfg(feature = "cocoa")]
    frontends_cocoa::cocoa_main()?;
    
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
- Rust dylib: `nix develop -c cargo build --release --features swift`
- Xcode app (production):
  - Open `macos/Reel.xcodeproj`
  - Build the `Reel` scheme. Build phases:
    - Build Rust Core (cargo, via nix if available)
    - Copy Swift Bridge (from `target/release/build/reel-*/out` to `macos/ReelApp/Generated`)
    - Embed Rust dylib into the app bundle Frameworks
  - Run the app; it initializes Rust core, lists backends, subscribes to events

CI sketch:
- Use Xcode build to drive both phases and sign the app; embed `libreel_ffi.dylib`.

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

## Implementation Progress Checklist

### Phase 1: Core Refactoring ✅
**Status**: Completed

#### 1.1 Core Module Setup
- [x] Create `src/core/` directory structure
- [x] Create `src/core/mod.rs` with module exports
- [x] Create `src/core/state.rs` with AppState (moved as-is, already platform-agnostic)
- [x] Create `src/core/frontend.rs` for Frontend trait

#### 1.2 Extract Platform-Agnostic State
- [x] Analyzed `src/app_state.rs` - found it's already platform-agnostic
- [x] Moved AppState to `src/core/state.rs` without changes
- [x] State initialization methods preserved
- [x] State getters/setters preserved
- [x] State subscription mechanisms preserved

#### 1.3 Create ViewModels
- [x] Moved `src/ui/viewmodels/` to `src/core/viewmodels/`
- [x] All ViewModels already platform-agnostic (no GTK dependencies)
- [x] `LibraryViewModel` - fully functional
- [x] `PlayerViewModel` - fully functional
- [x] `DetailsViewModel` - fully functional
- [x] `HomeViewModel` - fully functional
- [x] `SidebarViewModel` - fully functional
- [x] `SourcesViewModel` - fully functional

#### 1.4 Frontend Trait Definition
- [x] Created `src/core/frontend.rs`
- [x] Defined `Frontend` trait with async methods
- [x] Added initialize, run, shutdown methods
- [x] Frontend lifecycle management defined

#### 1.5 Reorganize GTK Code
- [x] Created `src/platforms/gtk/` directory
- [x] Moved `src/app.rs` to `src/platforms/gtk/app.rs`
- [x] Moved `src/ui/` to `src/platforms/gtk/ui/`
- [x] Updated all import paths in GTK code
- [x] Updated `main.rs` to use new structure
- [x] Verified GTK build still works
- [x] All tests pass (126 tests)

### Phase 2: macOS Frontend Foundation 🟡
**Status**: Partially Started

#### 2.1 Project Setup
- [x] Create `src/platforms/macos/` directory structure (Note: used `platforms` not `frontends`)
- [x] Add swift feature flag to `Cargo.toml`
- [x] Add Swift-bridge dependency (added but not configured for build)
- [x] Configure conditional compilation
- [x] Create macOS-specific main entry point

#### 2.2 Swift-Rust Bridge
- [x] Create `src/platforms/macos/bridge.rs`
- [x] Configure swift-bridge-build in build.rs (CRITICAL - needed for bridge to work)
- [x] Establish minimal swift-bridge module (MacOSCore + get_build_info)
- [x] Add blocking wrapper to initialize core via swift-bridge (`sb_initialize`)
- [ ] Define FFI interface for EventBus
- [ ] Define FFI interface for DataService
- [ ] Implement type conversions for models
- [ ] Add structured error handling across FFI boundary
- [ ] Create bridge initialization code on Swift side

#### 2.3 Xcode Project Setup
- [ ] Create `macos/Reel.xcodeproj`
- [ ] Configure Swift package dependencies
- [ ] Set up build phases for Rust compilation
- [ ] Configure code signing
- [ ] Add Info.plist with app metadata
- [ ] Configure entitlements for network and file access

#### 2.4 SwiftUI App Structure
- [ ] Create `ReelApp.swift` main app file
- [ ] Create `AppModel` ObservableObject
- [ ] Implement core initialization in AppModel
- [ ] Create ContentView skeleton
- [ ] Set up environment objects
- [ ] Add app lifecycle handling

#### 2.5 Event System Integration
- [ ] Create Swift EventSubscriber wrapper
- [ ] Implement async event receiving in Swift
- [ ] Create event handler dispatch system
- [ ] Map Rust events to Swift UI updates
- [ ] Test event flow from Rust to Swift

#### 2.6 Additional Foundation Work Needed
- [x] Fix ImageLoader to handle actual image parsing (dimensions + format via `image` crate)
- [ ] Implement proper error handling in bridge
- [x] Create C-compatible FFI as alternative to swift-bridge (minimal new/initialize/free)
- [ ] Test basic Rust library loading from Swift
- [ ] Document build process for macOS

### Phase 3: Feature Implementation 🔲
**Status**: Not Started

#### 3.1 Library Browser
- [ ] Create `LibraryView.swift`
- [ ] Implement grid layout for media items
- [ ] Implement list layout alternative
- [ ] Add sorting and filtering UI
- [ ] Connect to LibraryViewModel
- [ ] Implement pull-to-refresh
- [ ] Add loading states
- [ ] Handle empty states
- [ ] Implement item selection
- [ ] Add context menus for items

#### 3.2 Media Details View
- [ ] Create `MediaDetailView.swift`
- [ ] Design detail layout
- [ ] Display metadata (title, year, rating, etc.)
- [ ] Show cast and crew information
- [ ] Add synopsis/description
- [ ] Implement play button
- [ ] Add to watchlist functionality
- [ ] Show related/similar items

#### 3.3 Media Player
- [ ] Create `PlayerView.swift`
- [ ] Integrate AVPlayer for video playback
- [ ] Implement custom video controls
- [ ] Add playback progress tracking
- [ ] Implement seek functionality
- [ ] Add volume controls
- [ ] Support fullscreen mode
- [ ] Handle AirPlay
- [ ] Implement picture-in-picture
- [ ] Sync playback position with DataService
- [ ] Handle playback errors

#### 3.4 Search Functionality
- [ ] Create `SearchView.swift`
- [ ] Implement search bar
- [ ] Add search suggestions
- [ ] Display search results
- [ ] Support search filters
- [ ] Connect to SearchViewModel
- [ ] Add recent searches

#### 3.5 Settings & Preferences
- [ ] Create macOS Preferences window
- [ ] Implement General settings tab
- [ ] Add Accounts tab for backend management
- [ ] Create Playback settings tab
- [ ] Add Library settings tab
- [ ] Implement appearance settings
- [ ] Add keyboard shortcuts configuration

#### 3.6 Authentication
- [ ] Create login flow UI
- [ ] Implement Plex authentication
- [ ] Implement Jellyfin authentication
- [ ] Add local library setup
- [ ] Integrate with macOS Keychain
- [ ] Handle authentication errors
- [ ] Support multiple accounts

#### 3.7 Sync UI
- [ ] Create sync status indicator
- [ ] Show sync progress
- [ ] Display sync errors
- [ ] Add manual sync trigger
- [ ] Show last sync time
- [ ] Implement sync settings

### Phase 4: Platform-Specific Features 🔲
**Status**: Not Started

#### 4.1 macOS Integration
- [ ] Implement menu bar with all actions
- [ ] Add keyboard shortcuts (Cmd+Q, Cmd+,, etc.)
- [ ] Support Touch Bar on compatible Macs
- [ ] Implement Quick Look preview extension
- [ ] Add Spotlight indexing for media
- [ ] Support Handoff between devices
- [ ] Implement Universal Clipboard support
- [ ] Add AppleScript support

#### 4.2 System Integration
- [ ] Add media key support (play/pause/next/previous)
- [ ] Implement Now Playing integration
- [ ] Support notification center
- [ ] Add dock menu items
- [ ] Implement Services menu items
- [ ] Support drag and drop
- [ ] Add share sheet integration

#### 4.3 Performance
- [ ] Implement lazy loading for large libraries
- [ ] Add image caching with NSCache
- [ ] Optimize scroll performance
- [ ] Implement virtualized lists
- [ ] Add background queue management
- [ ] Profile and optimize memory usage
- [ ] Implement disk cache management

#### 4.4 Accessibility
- [ ] Add VoiceOver support
- [ ] Implement keyboard navigation
- [ ] Support Dynamic Type
- [ ] Add accessibility labels
- [ ] Test with Accessibility Inspector
- [ ] Support reduced motion
- [ ] Implement high contrast mode

### Phase 5: Testing & Quality 🔲
**Status**: Not Started

#### 5.1 Unit Tests
- [ ] Test CoreState functionality
- [ ] Test ViewModels logic
- [ ] Test Swift-Rust bridge
- [ ] Test event handling
- [ ] Test data transformations
- [ ] Test error handling

#### 5.2 Integration Tests
- [ ] Test full sync flow
- [ ] Test authentication flow
- [ ] Test playback flow
- [ ] Test offline mode
- [ ] Test backend switching
- [ ] Test concurrent operations

#### 5.3 UI Tests
- [ ] Create XCUITest suite
- [ ] Test navigation flows
- [ ] Test user interactions
- [ ] Test error states
- [ ] Test loading states
- [ ] Test accessibility

#### 5.4 Performance Tests
- [ ] Measure app launch time
- [ ] Test memory usage under load
- [ ] Measure scroll performance
- [ ] Test with large libraries (10k+ items)
- [ ] Profile CPU usage
- [ ] Test background sync impact

### Phase 6: Distribution & Packaging 🔲
**Status**: Not Started

#### 6.1 Build System
- [ ] Create universal binary build script
- [ ] Set up GitHub Actions for macOS
- [ ] Configure release builds
- [ ] Add build number automation
- [ ] Create debug/release configurations

#### 6.2 App Bundle
- [ ] Create proper app bundle structure
- [ ] Add app icons (all sizes)
- [ ] Create launch screen
- [ ] Add required Info.plist entries
- [ ] Configure entitlements
- [ ] Add privacy policy

#### 6.3 Code Signing
- [ ] Obtain Developer ID certificate
- [ ] Configure automatic code signing
- [ ] Sign all binaries and frameworks
- [ ] Notarize the app with Apple
- [ ] Test Gatekeeper compliance

#### 6.4 Distribution
- [ ] Create DMG installer
- [ ] Add Sparkle for updates
- [ ] Prepare App Store submission (optional)
- [ ] Create Homebrew formula
- [ ] Set up release notes automation
- [ ] Create installation documentation

### Phase 7: Documentation 🔲
**Status**: Not Started

#### 7.1 Developer Documentation
- [ ] Document architecture decisions
- [ ] Create Swift-Rust bridge guide
- [ ] Document ViewModels pattern
- [ ] Add contribution guidelines
- [ ] Create debugging guide

#### 7.2 User Documentation
- [ ] Create user manual
- [ ] Add in-app help
- [ ] Create video tutorials
- [ ] Document keyboard shortcuts
- [ ] Add FAQ section

## Migration Path

### Step 1: Prepare (Week 1)
- Complete Phase 1.1-1.5 from checklist above

### Step 2: Extract Core (Week 2)
- Complete remaining Phase 1 items
- Ensure all tests pass

### Step 3: macOS Foundation (Week 3-4)
- Complete Phase 2 items
- Get basic app running

### Step 4: Feature Implementation (Week 5-8)
- Complete Phase 3 items
- Achieve feature parity with GTK

### Step 5: Polish (Week 9-10)
- Complete Phase 4-7 items
- Prepare for release

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

## Progress Summary

### Overall Status
- **Phase 1**: ✅ Completed (23/23 tasks completed)
- **Phase 2**: 🟡 Partially Started (5/29 tasks completed, bridge non-functional)
- **Phase 3**: 🔲 Not Started (0/57 tasks)
- **Phase 4**: 🔲 Not Started (0/31 tasks)
- **Phase 5**: 🔲 Not Started (0/24 tasks)
- **Phase 6**: 🔲 Not Started (0/21 tasks)
- **Phase 7**: 🔲 Not Started (0/10 tasks)

**Total Progress**: 28/195 tasks completed (14%)

### Key Milestones
- [x] 🎯 Core refactoring complete (GTK still functional)
- [ ] ⚠️ macOS app launches successfully (compiles but no UI)
- [ ] 🎯 Basic library browsing works
- [ ] 🎯 Video playback functional
- [ ] 🎯 Feature parity with GTK frontend
- [ ] 🎯 First beta release
- [ ] 🎯 Production release

### Current Status - Critical Assessment

#### What Actually Works:
1. **Builds Compile**: Both GTK and macOS builds compile without errors
2. **Platform Separation**: GTK code properly isolated in `src/platforms/gtk/`
3. **Conditional Compilation**: Feature flags work correctly
4. **ImageLoader Stub**: Created platform-agnostic image loader that doesn't break GTK

#### What Doesn't Work:
1. **Swift-Rust Bridge**: Entirely commented out, non-functional
   - swift-bridge requires special build configuration not implemented
   - No actual FFI interface available
   - Type conversions not implemented
2. **macOS App**: Compiles but doesn't actually do anything
   - No UI implementation
   - No NSApplication setup
   - No actual macOS window or views
3. **Event System Integration**: No way for Swift to receive Rust events
4. **Data Access**: No way for Swift to access Rust data

### Next Critical Steps
1. **Fix Swift-Bridge Build Configuration**: 
   - Set up swift-bridge-build properly in build.rs
   - Create actual FFI interfaces (uncomment and fix bridge.rs)
   - Test basic FFI calls work

2. **Create Minimal Xcode Project**:
   - Set up basic SwiftUI app
   - Link to Rust library
   - Test basic interop

3. **Implement Basic FFI**:
   - Start with simple string passing
   - Then move to complex types
   - Handle async operations

### Current Focus
Need to make the Swift-Rust bridge actually functional before any UI work can begin.

## Implementation Notes

### Current Implementation Details

#### Directory Structure
- Used `src/platforms/` instead of `src/frontends/` for platform-specific code
- macOS code is in `src/platforms/macos/` not `src/frontends/macos/`
- This aligns better with the existing codebase structure

#### Build Configuration
```toml
# Current Cargo.toml setup
[features]
default = ["gtk"]
gtk = ["dep:gtk4", "dep:gdk4", "dep:gdk-pixbuf", "dep:libadwaita", "dep:glib-build-tools"]
swift = ["dep:swift-bridge", "dep:objc", "dep:cocoa", "dep:core-foundation", "dep:dispatch"]
```

#### Bridge Status
The Swift-Rust bridge (`src/platforms/macos/bridge.rs`) exists but is entirely commented out because:
1. swift-bridge requires special build configuration not implemented
2. The swift-bridge macro doesn't compile without proper setup
3. Need to decide between swift-bridge or standard C FFI

#### Image Loader
Created a platform-agnostic ImageLoader that:
- Works with GTK (returns gdk::Texture)
- Has stub implementation for macOS (returns ImageData struct)
- TODO: Needs actual image parsing (currently returns 0 for dimensions)

## Swift Bridge (Production)

We use swift-bridge for all Rust↔Swift interop. C FFI is removed.

Codegen:
- Build.rs generates Swift bridge files from `src/platforms/macos/bridge.rs` into `target/<profile>/build/reel-*/out/`.

Integrating in Xcode/SwiftPM:
1) Add a build phase before Swift compilation that copies generated Swift from `OUT_DIR` into your project (or adds the folder to the target’s Sources during build).
2) Link `libreel_ffi.dylib` (name from Cargo [lib] name = `reel_ffi`).
3) Ensure runtime search paths include the dylib location when running.

Public Swift APIs (generated):
- `MacOSCore()` constructor, `.sb_initialize()`, `.sb_is_initialized()`
- `.list_backends() -> [BackendBridge]`
- `.get_cached_libraries(backendId: String) -> [LibraryBridge]`
- `.subscribe(eventKinds: [String]) -> EventSub`, then `eventSub.next_event_blocking(timeoutMs:)`

Local build (Rust dylib):
- `nix develop`
- `cargo build --no-default-features --features swift`

## Conclusion

This plan provides a clear path to add a native macOS frontend while preserving the excellent architecture already in place. The EventBus, DataService, and SyncManager require zero changes and will power both frontends equally well. The main work is creating SwiftUI views and connecting them to the existing event-driven backend.

The comprehensive checklist above allows tracking progress at a granular level and ensures no critical tasks are missed during implementation. Current progress shows the foundation is partially in place but the critical Swift-Rust bridge needs to be made functional before UI work can begin.
