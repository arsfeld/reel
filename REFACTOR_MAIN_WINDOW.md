# Main Window Refactoring Guide

## Overview
The `src/ui/main_window.rs` file has grown to 2273 lines and handles too many responsibilities. This document outlines a comprehensive refactoring plan to improve maintainability, testability, and code organization.

## Current State (Updated January 2025)

### Completed Authentication Refactoring
✅ **AuthProvider/Source Architecture**: Separated authentication from backend implementation
✅ **AuthManager Service**: Centralized credential management with keyring integration  
✅ **Backend Constructors**: PlexBackend and JellyfinBackend support `from_auth()` pattern
✅ **AppState Integration**: AuthManager is now part of core application state
⚠️ **Partial Auth Dialog Migration**: Started but has UI compilation errors

### Remaining Problems

#### 1. File Size and Complexity
- **2273 lines** in a single file
- Multiple responsibilities mixed together
- Difficult to navigate and understand
- High cognitive load for developers

#### 2. Violation of Single Responsibility Principle
Current responsibilities include:
- Window initialization and setup
- Navigation between pages
- Backend management (needs migration to use AuthProvider)
- Library display and management
- Sync coordination
- Cache management
- UI state management
- Authentication flow (partially migrated)
- Player control
- Filter controls creation

### 3. Code Duplication
- Similar patterns for showing different page types (movie/show details, player)
- Repeated navigation logic
- Duplicated UI update patterns

### 4. State Management Issues
- Multiple `RefCell` fields managing related state
- No clear state ownership
- Difficult to track state changes
- Potential for inconsistent state

## Refactoring Strategy

### Phase 1: High Priority Extractions (Week 1)

#### 1.1 Extract Navigation System
**New Module**: `src/ui/navigation/`

```rust
// src/ui/navigation/mod.rs
pub struct NavigationController {
    stack: gtk4::Stack,
    history: NavigationHistory,
    page_factory: Arc<PageFactory>,
}

impl NavigationController {
    pub async fn navigate_to(&self, destination: NavigationDestination) -> Result<()>;
    pub fn go_back(&self) -> bool;
    pub fn current_page(&self) -> Option<PageType>;
    pub fn clear_history(&self);
}

// src/ui/navigation/destination.rs
pub enum NavigationDestination {
    Home,
    Library { source_id: String, library: Library },
    MovieDetails { movie: Movie },
    ShowDetails { show: Show },
    Player { media: MediaItem },
}

// src/ui/navigation/history.rs
pub struct NavigationHistory {
    stack: Vec<PageType>,
    max_size: usize,
}
```

**Methods to Extract**:
- `show_home_page()` → `navigate_to(Home)`
- `show_library_view()` → `navigate_to(Library)`
- `show_movie_details()` → `navigate_to(MovieDetails)`
- `show_show_details()` → `navigate_to(ShowDetails)`
- `show_player()` → `navigate_to(Player)`
- Navigation stack management

#### 1.2 Extract Source Coordinator  
**New Module**: `src/ui/source_coordinator.rs`

**Note**: This builds upon the completed AuthProvider/AuthManager work.

```rust
use crate::services::auth_manager::AuthManager;
use crate::models::{AuthProvider, Source};

pub struct SourceCoordinator {
    state: Arc<AppState>,
    auth_manager: Arc<AuthManager>, // Use existing AuthManager
    backend_manager: Arc<BackendManager>,
    sync_manager: Arc<SyncManager>,
    cache_manager: Arc<CacheManager>,
}

impl SourceCoordinator {
    pub fn new(
        state: Arc<AppState>,
        auth_manager: Arc<AuthManager>,
        backend_manager: Arc<BackendManager>,
        sync_manager: Arc<SyncManager>,
        cache_manager: Arc<CacheManager>,
    ) -> Self {
        Self {
            state,
            auth_manager,
            backend_manager,
            sync_manager,
            cache_manager,
        }
    }

    // Integration with existing AuthManager
    pub async fn add_plex_account(&self, token: &str) -> Result<Vec<Source>> {
        let (provider_id, sources) = self.auth_manager.add_plex_account(token).await?;
        
        // Create backends for each discovered source
        for source in &sources {
            if let Some(provider) = self.auth_manager.get_provider(&provider_id).await {
                let backend = PlexBackend::from_auth(
                    provider,
                    source.clone(),
                    self.auth_manager.clone(),
                    Some(self.cache_manager.clone()),
                )?;
                
                self.backend_manager.register_backend(source.id.clone(), Arc::new(backend));
            }
        }
        
        Ok(sources)
    }
    
    pub async fn add_jellyfin_source(&self, server_url: &str, username: &str, password: &str) -> Result<Source> {
        let (provider_id, source) = self.auth_manager.add_jellyfin_auth(
            server_url, username, password, &token, &user_id
        ).await?;
        
        // Create backend for the source
        if let Some(provider) = self.auth_manager.get_provider(&provider_id).await {
            let backend = JellyfinBackend::from_auth(
                provider,
                source.clone(),
                self.auth_manager.clone(),
                Some(self.cache_manager.clone()),
            )?;
            
            self.backend_manager.register_backend(source.id.clone(), Arc::new(backend));
        }
        
        Ok(source)
    }
    
    pub async fn initialize_all_sources(&self) -> Result<Vec<SourceStatus>> {
        let providers = self.auth_manager.get_all_providers().await;
        let mut source_statuses = Vec::new();
        
        for provider in providers {
            match &provider {
                AuthProvider::PlexAccount { .. } => {
                    if let Ok(sources) = self.auth_manager.discover_plex_sources(provider.id()).await {
                        for source in sources {
                            // Initialize backend and check status
                            let status = self.initialize_source(provider.clone(), source).await?;
                            source_statuses.push(status);
                        }
                    }
                }
                AuthProvider::JellyfinAuth { .. } => {
                    // Create source from auth provider and initialize
                    let source = self.create_source_from_provider(&provider)?;
                    let status = self.initialize_source(provider.clone(), source).await?;
                    source_statuses.push(status);
                }
                AuthProvider::LocalFiles { .. } => {
                    // Handle local files
                    let status = self.initialize_local_source(&provider).await?;
                    source_statuses.push(status);
                }
                _ => {}
            }
        }
        
        Ok(source_statuses)
    }
    
    async fn initialize_source(&self, provider: AuthProvider, source: Source) -> Result<SourceStatus> {
        // Create appropriate backend based on provider type
        let backend: Arc<dyn MediaBackend> = match &provider {
            AuthProvider::PlexAccount { .. } => Arc::new(PlexBackend::from_auth(
                provider,
                source.clone(),
                self.auth_manager.clone(),
                Some(self.cache_manager.clone()),
            )?),
            AuthProvider::JellyfinAuth { .. } => Arc::new(JellyfinBackend::from_auth(
                provider,
                source.clone(),
                self.auth_manager.clone(),
                Some(self.cache_manager.clone()),
            )?),
            _ => return Err(anyhow!("Unsupported provider type")),
        };
        
        // Register with backend manager
        self.backend_manager.register_backend(source.id.clone(), backend.clone());
        
        // Check connection status
        let connection_status = if backend.initialize().await.is_ok() {
            ConnectionStatus::Connected
        } else {
            ConnectionStatus::Offline
        };
        
        Ok(SourceStatus {
            source_id: source.id,
            source_name: source.name,
            source_type: match source.source_type {
                crate::models::SourceType::PlexServer { .. } => SourceType::Plex,
                crate::models::SourceType::JellyfinServer => SourceType::Jellyfin,
                _ => SourceType::Local,
            },
            connection_status,
            library_count: 0, // Will be populated by sync
        })
    }

    pub async fn sync_source(&self, source_id: &str) -> Result<SyncResult>;
    pub async fn sync_all_visible_sources(&self) -> Result<Vec<SyncResult>>;
    pub async fn get_visible_libraries(&self) -> Result<Vec<(Source, Library)>>;
}

pub enum AuthStatus {
    Authenticated(User),
    NeedsAuth,
    Offline,
}

pub struct SourceStatus {
    pub source_id: String,
    pub source_name: String,
    pub source_type: SourceType,
    pub connection_status: ConnectionStatus,
    pub library_count: usize,
}
```

**Methods to Extract from main_window.rs**:
- `check_and_load_sources()` → `initialize_all_sources()`
- `sync_and_update_libraries()` → `sync_all_visible_sources()` 
- `trigger_sync()` → `sync_source()`
- Backend registration logic from auth dialog → `add_plex_account()` / `add_jellyfin_source()`
- Manual backend creation → SourceCoordinator methods

**Integration Points**:
- Auth dialog should use SourceCoordinator instead of directly creating backends
- Main window initialization should call `source_coordinator.initialize_all_sources()`
- Remove direct AuthManager/BackendManager usage from UI code

#### 1.3 Refactor Auth Dialog Integration
**Updated Module**: `src/ui/auth_dialog.rs`

**Current Issues**:
- Auth dialog directly creates and registers backends
- Bypasses the SourceCoordinator pattern
- Mixes authentication with backend lifecycle management

**Required Changes**:
```rust
impl ReelAuthDialog {
    pub fn new(source_coordinator: Arc<SourceCoordinator>) -> Self {
        let dialog: Self = glib::Object::builder().build();
        dialog.imp().source_coordinator.replace(Some(source_coordinator));
        dialog
    }

    fn on_auth_success(&self, token: String) {
        // Replace direct backend creation with SourceCoordinator usage
        if let Some(coordinator) = self.imp().source_coordinator.borrow().as_ref() {
            let coordinator_clone = coordinator.clone();
            
            glib::spawn_future_local(async move {
                match coordinator_clone.add_plex_account(&token).await {
                    Ok(sources) => {
                        info!("Successfully added Plex account with {} sources", sources.len());
                        // Dialog closes automatically
                    }
                    Err(e) => {
                        error!("Failed to add Plex account: {}", e);
                        // Show error in UI
                    }
                }
            });
        }
    }
    
    fn connect_jellyfin(&self, url: String, username: String, password: String) {
        if let Some(coordinator) = self.imp().source_coordinator.borrow().as_ref() {
            let coordinator_clone = coordinator.clone();
            
            glib::spawn_future_local(async move {
                match coordinator_clone.add_jellyfin_source(&url, &username, &password).await {
                    Ok(source) => {
                        info!("Successfully added Jellyfin source: {}", source.name);
                        // Dialog closes automatically
                    }
                    Err(e) => {
                        error!("Failed to add Jellyfin source: {}", e);
                        // Show error in UI
                    }
                }
            });
        }
    }
}
```

**Benefits**:
- Centralizes all source/backend management through SourceCoordinator
- Removes duplicate backend registration logic
- Improves error handling and user feedback
- Enables better testing and maintainability

### Phase 2: Medium Priority Extractions (Week 2)

#### 2.1 Extract Library Manager
**New Module**: `src/ui/library_manager.rs`

```rust
pub struct LibraryManager {
    libraries_list: gtk4::ListBox,
    source_sections: HashMap<String, SourceSection>,
    visibility_settings: LibraryVisibilitySettings,
    edit_mode: bool,
}

pub struct SourceSection {
    expander: gtk::Expander,
    source_id: String,
    library_rows: Vec<LibraryRow>,
    visibility_toggle: gtk::Switch,
}

pub struct LibraryRow {
    library_id: String,
    label: gtk::Label,
    count_badge: gtk::Label,
    sync_indicator: gtk::Spinner,
    visibility_checkbox: gtk::CheckButton,
}

pub struct LibraryVisibilitySettings {
    visible_sources: HashSet<String>,
    visible_libraries: HashSet<String>,
    library_sort_order: Vec<String>,
}

impl LibraryManager {
    pub fn update_sources_and_libraries(&self, sources: Vec<(Source, Vec<Library>)>);
    pub fn toggle_edit_mode(&self) -> bool;
    pub fn set_source_visibility(&self, source_id: &str, visible: bool);
    pub fn set_library_visibility(&self, library_id: &str, visible: bool);
    pub fn get_visible_libraries(&self) -> Vec<(String, String)>; // (source_id, library_id)
    pub fn reorder_libraries(&self, library_ids: Vec<String>);
    pub fn pin_library(&self, library_id: &str);
    pub fn load_visibility_settings(&self, config: &Config);
    pub fn save_visibility_settings(&self, config: &mut Config) -> Result<()>;
}
```

**Methods to Extract**:
- `update_libraries()`
- `update_libraries_display()`
- `toggle_edit_mode()`
- `load_library_visibility()`
- `save_library_visibility()`

#### 2.2 Extract Page Factory
**New Module**: `src/ui/pages/page_factory.rs`

```rust
pub struct PageFactory {
    state: Arc<AppState>,
    pages: Mutex<HashMap<PageId, Box<dyn Page>>>,
}

impl PageFactory {
    pub fn get_or_create<P: Page>(&self, id: PageId) -> Arc<P>;
    pub fn clear_page(&self, id: PageId);
    pub fn clear_all(&self);
    
    // Specific page creators
    pub fn create_home_page(&self) -> HomePage;
    pub fn create_library_view(&self) -> LibraryView;
    pub fn create_player_page(&self) -> PlayerPage;
    pub fn create_movie_details(&self) -> MovieDetailsPage;
    pub fn create_show_details(&self) -> ShowDetailsPage;
}
```

#### 2.3 Extract UI State Manager
**New Module**: `src/ui/state/ui_state.rs`

```rust
pub struct UIStateManager {
    inner: Rc<RefCell<UIState>>,
    subscribers: Vec<Box<dyn Fn(&UIState)>>,
}

pub struct UIState {
    pub edit_mode: bool,
    pub library_visibility: HashMap<String, bool>,
    pub saved_window_size: (i32, i32),
    pub current_filter: FilterState,
    pub sync_in_progress: bool,
}

impl UIStateManager {
    pub fn update<F>(&self, updater: F) where F: FnOnce(&mut UIState);
    pub fn subscribe<F>(&self, callback: F) where F: Fn(&UIState) + 'static;
    pub fn get(&self) -> Ref<UIState>;
}
```

### Phase 3: Low Priority Extractions (Week 3)

#### 3.1 Extract Status Bar Controller
**New Module**: `src/ui/components/status_bar.rs`

```rust
pub struct StatusBarController {
    status_row: adw::ActionRow,
    status_icon: gtk4::Image,
    sync_spinner: gtk4::Spinner,
}

impl StatusBarController {
    pub fn update_connection(&self, status: ConnectionStatus);
    pub fn show_sync_progress(&self, active: bool, message: Option<&str>);
    pub fn show_error(&self, error: &str);
    pub fn show_offline_mode(&self);
}
```

**Methods to Extract**:
- `update_connection_status()`
- `update_user_display()`
- `update_user_display_with_backend()`
- `show_sync_progress()`

#### 3.2 Extract Filter Controls Builder
**New Module**: `src/ui/components/filter_controls.rs`

```rust
pub struct FilterControlsBuilder {
    library_type: LibraryType,
}

impl FilterControlsBuilder {
    pub fn build(&self) -> gtk4::Box;
    pub fn connect_handlers(&self, library_view: &LibraryView);
    pub fn update_from_state(&self, filter_state: &FilterState);
}
```

**Methods to Extract**:
- `create_filter_controls()`

## Implementation Plan

### Step 1: Create New Module Structure
```bash
mkdir -p src/ui/navigation
mkdir -p src/ui/components
mkdir -p src/ui/state
mkdir -p src/ui/coordinators
```

### Step 2: Gradual Extraction Process

#### Week 1: Complete Auth Integration and Core Infrastructure
1. **Day 1**: Fix Auth Dialog Compilation Issues
   - Resolve compilation errors in auth_dialog.rs
   - Update UI templates if needed
   - Test existing auth flows still work

2. **Day 2-3**: Extract and Integrate SourceCoordinator
   - Create SourceCoordinator that wraps existing AuthManager
   - Move backend creation logic from auth dialog
   - Update main window to use SourceCoordinator for initialization
   - Test Plex and Jellyfin source addition through coordinator

3. **Day 4**: Extract NavigationController
   - Create navigation module (lower priority since auth is more critical)
   - Move basic navigation methods
   - Update navigation calls

4. **Day 5**: Integration Testing and Cleanup
   - Test complete auth flow through SourceCoordinator
   - Verify backend registration and initialization
   - Remove legacy backend creation code
   - Test source switching and sync operations

#### Week 2: State and UI Management
1. **Day 1-2**: Extract LibraryManager
   - Create library manager
   - Move library display logic
   - Test edit mode functionality

2. **Day 3-4**: Extract PageFactory and UIStateManager
   - Implement page caching
   - Consolidate state management
   - Test state updates

3. **Day 5**: Refactor Main Window
   - Remove extracted code
   - Simplify main window structure
   - Update initialization

#### Week 3: Polish and Optimization
1. **Day 1-2**: Extract remaining components
   - StatusBarController
   - FilterControlsBuilder
   
2. **Day 3-4**: Documentation and Testing
   - Update documentation
   - Add unit tests for new modules
   - Integration testing

3. **Day 5**: Performance Optimization
   - Profile the refactored code
   - Optimize hot paths
   - Final cleanup

## Expected Outcome

### Before (main_window.rs):
- 2273 lines
- 30+ methods
- Complex state management
- Difficult to test

### After:
```rust
// main_window.rs (~400 lines)
pub struct ReelMainWindow {
    // UI Components (from template)
    imp: imp::ReelMainWindow,
    
    // Coordinators
    navigation: NavigationController,
    source_coordinator: SourceCoordinator,
    
    // Managers
    library_manager: LibraryManager,
    ui_state: UIStateManager,
    
    // Controllers
    status_bar: StatusBarController,
}

impl ReelMainWindow {
    pub fn new(app: &adw::Application, state: Arc<AppState>, config: Arc<Config>) -> Self {
        // Initialize components (~50 lines)
        // Wire up event handlers (~50 lines)
        // Start initial load (~20 lines)
    }
    
    // High-level coordination methods only
    fn setup_actions(&self, app: &adw::Application);
    fn apply_theme(&self);
    fn setup_state_subscriptions(&self);
    fn show_auth_dialog(&self);
    fn show_preferences(&self);
    fn show_about(&self);
}
```

## Benefits

### Immediate Benefits
1. **Better Code Organization**: Each module has a single, clear responsibility
2. **Improved Testability**: Components can be unit tested in isolation
3. **Easier Debugging**: Issues are localized to specific modules
4. **Parallel Development**: Multiple developers can work on different modules

### Long-term Benefits
1. **Maintainability**: Easier to understand and modify
2. **Reusability**: Components can be reused in other windows/dialogs
3. **Performance**: Better opportunity for optimization
4. **Documentation**: Each module can have focused documentation

## Testing Strategy

### Unit Tests
Each new module should have comprehensive unit tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_navigation_push_pop() { /* ... */ }
    
    #[test]
    fn test_source_switching() { /* ... */ }
    
    #[test]
    fn test_account_discovery() { /* ... */ }
    
    #[test]
    fn test_library_visibility() { /* ... */ }
    
    #[test]
    fn test_source_visibility_toggle() { /* ... */ }
}
```

### Integration Tests
- Test the interaction between modules
- Verify the refactored window behaves identically to the original
- Test all user workflows

## Migration Strategy from Current Auth System

### Current State Analysis
✅ **Completed**:
- AuthProvider and Source models created
- AuthManager service implemented  
- PlexBackend and JellyfinBackend updated with `from_auth()` constructors
- AuthManager integrated into AppState
- Basic auth flow working

❌ **Remaining Issues**:
- Auth dialog has compilation errors
- UI still creates backends directly
- Mixed legacy and new credential handling
- No central coordination of source lifecycle

### Migration Path

#### Phase A: Immediate Fixes (Complete First)
```bash
# 1. Fix compilation errors
- Resolve UI template issues in auth_dialog.rs
- Update any missing imports or type mismatches
- Test basic auth dialog functionality

# 2. Create SourceCoordinator wrapper
- Implement SourceCoordinator that uses existing AuthManager
- Move backend creation logic from UI to coordinator
- Update auth dialog to use coordinator instead of direct backend creation
```

#### Phase B: Integration and Cleanup
```bash
# 3. Update main window initialization
- Replace direct AuthManager usage with SourceCoordinator
- Update startup flow to use coordinator.initialize_all_sources()
- Remove legacy backend creation code

# 4. Clean up mixed credential systems
- Ensure all credentials go through AuthManager
- Remove duplicate keyring/file storage code
- Standardize error handling
```

## Updated Migration Checklist

### Phase 1: Auth System Completion
- [ ] Fix auth dialog compilation errors
- [ ] Create SourceCoordinator module
- [ ] Update auth dialog to use SourceCoordinator
- [ ] Update main window initialization
- [ ] Test Plex and Jellyfin auth flows
- [ ] Remove direct backend creation from UI

### Phase 2: Core Module Extraction  
- [ ] Extract NavigationController
- [ ] Extract LibraryManager (with unified sidebar support)
- [ ] Extract PageFactory
- [ ] Extract UIStateManager
- [ ] Extract StatusBarController
- [ ] Extract FilterControlsBuilder

### Phase 3: Integration and Testing
- [ ] Update main_window.rs to use extracted modules
- [ ] Add unit tests for new modules
- [ ] Add integration tests
- [ ] Performance testing
- [ ] Update documentation
- [ ] Code review
- [ ] Merge to main

## Risk Mitigation

### Potential Risks
1. **Breaking existing functionality**: Mitigate with comprehensive testing
2. **Performance regression**: Profile before and after
3. **Increased complexity**: Keep interfaces simple and well-documented
4. **Team disruption**: Gradual migration, maintain backwards compatibility

### Rollback Plan
- Keep the refactoring in a separate branch
- Maintain feature parity at each step
- Only merge after full validation

## Success Metrics

1. **Code Quality**
   - Reduce main_window.rs to under 500 lines
   - Achieve 80%+ test coverage on new modules
   - Pass all existing tests

2. **Performance**
   - No regression in startup time
   - No increase in memory usage
   - Maintain or improve UI responsiveness

3. **Developer Experience**
   - Reduce time to implement new features by 30%
   - Improve code review turnaround
   - Reduce bug fix time

## New Patterns and Opportunities from Auth Work

### Discovered Architecture Improvements

#### 1. Provider-Based Backend Creation
The `from_auth()` constructor pattern enables:
- **Type-safe backend construction**: Ensures correct provider/backend pairing
- **Centralized credential management**: All auth goes through AuthManager
- **Better error handling**: Clear separation of auth vs connection failures

```rust
// Example of the improved pattern
let backend = PlexBackend::from_auth(
    auth_provider,
    source,  
    auth_manager,
    cache_manager,
)?;
```

#### 2. Source Discovery and Management
The AuthProvider system enables:
- **Multi-source accounts**: One Plex account can discover multiple servers
- **Unified source model**: Consistent handling across Plex, Jellyfin, local, network
- **Connection state tracking**: Sources know their online/offline status

#### 3. Separation of Concerns Achieved
- **Authentication**: AuthManager + AuthProvider models
- **Source Management**: Source model + discovery logic
- **Backend Creation**: Type-safe constructors with proper dependency injection
- **Credential Storage**: Centralized keyring management with fallbacks

### Impact on Original Refactoring Goals

#### Enhanced Benefits
1. **Better Testability**: AuthManager and Source models can be unit tested independently
2. **Improved Maintainability**: Clear separation between auth, source discovery, and backend lifecycle
3. **Enhanced Security**: Centralized credential management with keyring integration
4. **Multi-Backend Support**: Foundation for managing multiple sources simultaneously

#### Reduced Complexity in Main Window
The AuthManager/SourceCoordinator pattern removes these responsibilities from main_window.rs:
- Direct backend creation and registration
- Credential management and storage
- Server discovery and connection testing
- Auth state management

### Future Enhancements Enabled

#### 1. Advanced Source Management
```rust
impl SourceCoordinator {
    pub async fn refresh_all_sources(&self) -> Result<Vec<SourceStatus>>;
    pub async fn test_source_connectivity(&self, source_id: &str) -> Result<bool>;
    pub async fn update_source_priority(&self, source_order: Vec<String>) -> Result<()>;
    pub async fn disable_source(&self, source_id: &str) -> Result<()>;
}
```

#### 2. Smart Backend Selection
- Automatic failover between local/remote connections
- Load balancing across multiple Plex servers
- Priority-based source ordering

#### 3. Enhanced Security Features
- Token refresh automation
- Credential expiration handling
- Secure storage with encryption options

## Conclusion

The authentication provider refactoring has laid a solid foundation for the main window refactoring by:
1. **Proving the viability** of the coordinator pattern approach
2. **Establishing clean boundaries** between authentication, source management, and backend lifecycle
3. **Creating reusable patterns** that can be extended to other coordinators (Navigation, Library, etc.)
4. **Reducing the scope** of the main window refactoring by handling the most complex architectural challenge first

The next phase should focus on **completing the SourceCoordinator integration** before proceeding with other extractions, as this will provide the strongest foundation for the remaining refactoring work. The auth system's success demonstrates that the overall refactoring approach is sound and achievable.