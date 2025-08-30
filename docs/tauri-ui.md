# Tauri + Leptos Frontend Implementation Plan

## Overview
This plan unifies the best aspects of both previous approaches, using **Leptos** - the most popular and mature Rust frontend framework - to build a 100% Rust application with native macOS look and feel via Tauri.

## Why Leptos + Tauri

### Why Leptos (over other Rust frameworks)
- **Most Popular**: Largest community, best documentation, most examples
- **Performance**: Fine-grained reactivity, minimal WASM size (~200KB gzipped)
- **Server Functions**: Direct Rust-to-Rust communication with Tauri backend
- **Signals**: Reactive primitives similar to SolidJS, perfect for event-driven architecture
- **Component Model**: Familiar to React developers but with Rust's type safety
- **Mature Ecosystem**: Production-ready with router, forms, animations support

### Comparison with Alternatives
- **Yew**: Older but more verbose, larger WASM bundles, less reactive
- **Dioxus**: Good but smaller community, less mature ecosystem
- **Sycamore**: Solid but less feature-complete than Leptos
- **Egui**: Immediate mode, not web-native, doesn't fit macOS aesthetic

## Architecture

### Repository Layout (Integrated Approach)
```
reel/                           # Root repository
├── src/
│   ├── core/                   # Existing core (unchanged)
│   ├── backends/               # Existing backends (unchanged)
│   ├── services/               # Existing services (unchanged)
│   ├── platforms/
│   │   ├── gtk/                # Existing GTK platform
│   │   ├── macos/              # Existing Swift bridge attempts
│   │   ├── cocoa/              # Existing Cocoa attempts
│   │   └── tauri/              # NEW: Tauri platform
│   │       ├── mod.rs
│   │       ├── app.rs          # Tauri app initialization
│   │       ├── commands.rs     # Command handlers
│   │       ├── events.rs       # Event forwarding
│   │       ├── menu.rs         # Native menu setup
│   │       ├── window.rs       # Window configuration
│   │       └── ui/             # Leptos UI components
│   │           ├── mod.rs
│   │           ├── app.rs      # Leptos app root
│   │           ├── api/        # Tauri API bindings
│   │           │   ├── mod.rs
│   │           │   ├── commands.rs
│   │           │   └── events.rs
│   │           ├── components/ # Reusable components
│   │           │   ├── mod.rs
│   │           │   ├── sidebar.rs
│   │           │   ├── media_card.rs
│   │           │   ├── video_player.rs
│   │           │   └── toolbar.rs
│   │           ├── pages/      # Route components
│   │           │   ├── mod.rs
│   │           │   ├── home.rs
│   │           │   ├── library.rs
│   │           │   ├── details.rs
│   │           │   ├── player.rs
│   │           │   └── settings.rs
│   │           ├── state/      # Global state management
│   │           │   ├── mod.rs
│   │           │   ├── library.rs
│   │           │   ├── playback.rs
│   │           │   └── settings.rs
│   │           └── design/     # Design system
│   │               ├── mod.rs
│   │               ├── tokens.rs
│   │               └── icons.rs
│   └── main.rs                 # Unified entry point
├── tauri.conf.json             # Tauri config at root
└── Cargo.toml                  # Workspace configuration
```

### Hybrid Approach (For Tauri CLI Compatibility)
Since Tauri's tooling expects `src-tauri/`, we use a minimal shim:

```
reel/
├── src/platforms/tauri/        # All real Tauri code here (as above)
├── src-tauri/                   # Minimal shim for Tauri CLI
│   ├── Cargo.toml              # Just depends on main crate
│   └── src/
│       └── main.rs             # Calls platforms::tauri::run()
└── tauri.conf.json
```

**src-tauri/src/main.rs** (minimal shim):
```rust
fn main() {
    reel::platforms::tauri::run().expect("Failed to run Tauri");
}
```

**src-tauri/Cargo.toml**:
```toml
[package]
name = "reel-tauri"
version = "0.1.0"
edition = "2021"

[dependencies]
reel = { path = "..", features = ["tauri"] }
tauri = { version = "2.0", features = ["macos-private-api"] }
```

### Workspace Configuration
```toml
# Root Cargo.toml
[workspace]
members = [".", "src-tauri"]
resolver = "2"

[workspace.dependencies]
tokio = { version = "1.40", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
tracing = "0.1"

[features]
default = ["gtk"]
gtk = ["dep:gtk4", "dep:libadwaita", "dep:gdk4", "dep:gdk-pixbuf"]
swift = ["dep:swift-bridge", "dep:objc", "dep:cocoa"]
cocoa = ["dep:objc2", "dep:objc2-foundation", "dep:objc2-app-kit"]
tauri = ["dep:tauri", "dep:leptos", "dep:leptos_router", "dep:leptos_use"]

[target.'cfg(feature = "tauri")'.dependencies]
tauri = { version = "2.0", features = ["macos-private-api"] }
leptos = { version = "0.6", features = ["csr", "nightly"] }
leptos_router = { version = "0.6", features = ["csr"] }
leptos_use = "0.10"
wasm-bindgen = "0.2"
web-sys = { version = "0.3", features = [
    "HtmlVideoElement", "HtmlElement", "Window", "Document",
    "Element", "Event", "EventTarget", "CustomEvent",
    "IntersectionObserver", "IntersectionObserverEntry"
]}
serde-wasm-bindgen = "0.6"
```

## Platform Integration

### Main Entry Point
```rust
// src/main.rs
fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Platform-specific initialization
    #[cfg(feature = "gtk")]
    {
        platforms::gtk::run()?;
    }
    
    #[cfg(feature = "tauri")]
    {
        platforms::tauri::run()?;
    }
    
    #[cfg(all(feature = "cocoa", not(feature = "tauri")))]
    {
        platforms::cocoa::run()?;
    }
    
    Ok(())
}
```

### Tauri Platform Module
```rust
// src/platforms/tauri/mod.rs
pub mod app;
pub mod commands;
pub mod events;
pub mod menu;
pub mod window;
pub mod ui;

use crate::core::state::AppState;
use std::sync::Arc;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize core state
    let runtime = tokio::runtime::Runtime::new()?;
    let app_state = runtime.block_on(AppState::new())?;
    
    // Build Tauri app
    tauri::Builder::default()
        .manage(Arc::new(app_state))
        .invoke_handler(commands::handler())
        .menu(menu::create())
        .setup(|app| {
            let handle = app.handle();
            let state = app.state::<Arc<AppState>>();
            
            // Setup event forwarding
            events::setup_event_bridge(&handle, state.inner().clone());
            
            // Configure window
            let window = app.get_window("main").unwrap();
            window::setup_macos_window(&window);
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    
    Ok(())
}
```

### Platform Module Registration
```rust
// src/platforms/mod.rs
#[cfg(feature = "gtk")]
pub mod gtk;

#[cfg(feature = "swift")]
pub mod macos;

#[cfg(feature = "cocoa")]
pub mod cocoa;

#[cfg(feature = "tauri")]
pub mod tauri;
```

## Leptos Frontend Architecture

### 1. Main Application Structure
```rust
// src/platforms/tauri/ui/app.rs
use leptos::*;
use leptos_router::*;
use crate::{pages::*, components::*, state::*};

#[component]
pub fn App() -> impl IntoView {
    // Global state providers
    provide_context(LibraryState::new());
    provide_context(PlaybackState::new());
    provide_context(SettingsState::new());
    
    // Initialize event listeners
    crate::api::events::initialize_listeners();
    
    view! {
        <Router>
            <div class="app-container" data-tauri-drag-region>
                <Sidebar />
                <main class="main-content">
                    <Toolbar />
                    <Routes>
                        <Route path="/" view=HomePage />
                        <Route path="/library/:id" view=LibraryPage />
                        <Route path="/media/:id" view=DetailsPage />
                        <Route path="/player/:id" view=PlayerPage />
                        <Route path="/settings" view=SettingsPage />
                    </Routes>
                </main>
            </div>
        </Router>
    }
}
```

### 2. Type-Safe Tauri Commands
```rust
// src/platforms/tauri/ui/api/commands.rs
use leptos::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "tauri"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Library {
    pub id: String,
    pub name: String,
    pub backend_id: String,
    pub media_type: MediaType,
    pub item_count: u32,
}

pub async fn get_libraries(source_id: String) -> Result<Vec<Library>, String> {
    #[derive(Serialize)]
    struct Args { source_id: String }
    
    let args = serde_wasm_bindgen::to_value(&Args { source_id })
        .map_err(|e| e.to_string())?;
    
    let result = invoke("get_libraries", args).await;
    serde_wasm_bindgen::from_value(result)
        .map_err(|e| e.to_string())
}

// Leptos resource for reactive data fetching
pub fn use_libraries(source_id: Signal<String>) -> Resource<String, Vec<Library>> {
    create_resource(
        move || source_id.get(),
        |id| async move {
            get_libraries(id).await.unwrap_or_default()
        }
    )
}
```

### 3. Event System Integration
```rust
// src/platforms/tauri/ui/api/events.rs
use leptos::*;
use wasm_bindgen::prelude::*;
use serde::Deserialize;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"])]
    async fn listen(event: &str, handler: &Closure<dyn FnMut(JsValue)>) -> JsValue;
}

#[derive(Deserialize, Clone)]
pub struct MediaEvent {
    pub event_type: String,
    pub payload: MediaPayload,
}

pub fn initialize_listeners() {
    spawn_local(async {
        listen_to_media_events().await;
        listen_to_sync_events().await;
        listen_to_playback_events().await;
    });
}

async fn listen_to_media_events() {
    let library_state = use_context::<LibraryState>()
        .expect("LibraryState not provided");
    
    let closure = Closure::new(move |event: JsValue| {
        if let Ok(media_event) = serde_wasm_bindgen::from_value::<MediaEvent>(event) {
            match media_event.event_type.as_str() {
                "media:created" => library_state.add_item(media_event.payload),
                "media:updated" => library_state.update_item(media_event.payload),
                "media:batch_created" => library_state.add_batch(media_event.payload),
                _ => {}
            }
        }
    });
    
    listen("media:*", &closure).await;
    closure.forget(); // Keep closure alive
}
```

### 4. Reactive State Management
```rust
// src/platforms/tauri/ui/state/library.rs
use leptos::*;
use std::collections::HashMap;

#[derive(Clone)]
pub struct LibraryState {
    items: RwSignal<HashMap<String, MediaItem>>,
    filter: RwSignal<FilterOptions>,
    selected: RwSignal<Option<String>>,
}

impl LibraryState {
    pub fn new() -> Self {
        Self {
            items: create_rw_signal(HashMap::new()),
            filter: create_rw_signal(FilterOptions::default()),
            selected: create_rw_signal(None),
        }
    }
    
    pub fn filtered_items(&self) -> Memo<Vec<MediaItem>> {
        let items = self.items;
        let filter = self.filter;
        
        create_memo(move |_| {
            let all_items = items.get();
            let current_filter = filter.get();
            
            all_items
                .values()
                .filter(|item| current_filter.matches(item))
                .cloned()
                .collect()
        })
    }
    
    pub fn add_item(&self, item: MediaItem) {
        self.items.update(|items| {
            items.insert(item.id.clone(), item);
        });
    }
}
```

### 5. macOS-Native Components
```rust
// src/platforms/tauri/ui/components/sidebar.rs
use leptos::*;
use leptos_router::*;

#[component]
pub fn Sidebar() -> impl IntoView {
    let sources = use_sources();
    
    view! {
        <aside class="sidebar">
            <div class="sidebar-header">
                <h1 class="app-title">"Reel"</h1>
            </div>
            
            <nav class="sidebar-nav">
                <section class="nav-section">
                    <h2 class="nav-section-title">"Library"</h2>
                    <A href="/" class="nav-item">
                        <Icon name="house" />
                        <span>"Home"</span>
                    </A>
                    <A href="/library/movies" class="nav-item">
                        <Icon name="film" />
                        <span>"Movies"</span>
                    </A>
                    <A href="/library/shows" class="nav-item">
                        <Icon name="tv" />
                        <span>"Shows"</span>
                    </A>
                </section>
                
                <section class="nav-section">
                    <h2 class="nav-section-title">"Sources"</h2>
                    <Suspense fallback=|| view! { <div>"Loading..."</div> }>
                        {move || sources.get().map(|sources| {
                            sources.into_iter().map(|source| {
                                view! {
                                    <SourceItem source=source />
                                }
                            }).collect_view()
                        })}
                    </Suspense>
                </section>
            </nav>
        </aside>
    }
}
```

### 6. Media Grid with Virtual Scrolling
```rust
// src/platforms/tauri/ui/components/media_grid.rs
use leptos::*;
use leptos_use::{use_intersection_observer, use_window_scroll};

#[component]
pub fn MediaGrid(
    items: Signal<Vec<MediaItem>>,
) -> impl IntoView {
    let items_per_page = 50;
    let loaded_count = create_rw_signal(items_per_page);
    
    // Virtual scrolling with intersection observer
    let sentinel = create_node_ref::<html::Div>();
    use_intersection_observer(
        sentinel,
        move |entries, _| {
            if entries[0].is_intersecting() {
                loaded_count.update(|n| *n += items_per_page);
            }
        },
    );
    
    view! {
        <div class="media-grid">
            <For
                each=move || {
                    items.get()
                        .into_iter()
                        .take(loaded_count.get())
                        .collect::<Vec<_>>()
                }
                key=|item| item.id.clone()
                children=move |item| {
                    view! { <MediaCard item=item /> }
                }
            />
            <div ref=sentinel class="loading-sentinel" />
        </div>
    }
}

#[component]
pub fn MediaCard(item: MediaItem) -> impl IntoView {
    let navigate = use_navigate();
    
    view! {
        <article 
            class="media-card"
            on:click=move |_| navigate(&format!("/media/{}", item.id), Default::default())
        >
            <div class="media-poster">
                <img 
                    src=item.poster_url 
                    alt=item.title.clone()
                    loading="lazy"
                />
                <div class="media-overlay">
                    <PlayButton item_id=item.id.clone() />
                </div>
            </div>
            <div class="media-info">
                <h3 class="media-title">{item.title}</h3>
                <p class="media-year">{item.year}</p>
            </div>
        </article>
    }
}
```

### 7. Video Player Component
```rust
// src/platforms/tauri/ui/components/video_player.rs
use leptos::*;
use leptos_use::{use_event_listener, use_interval};
use web_sys::HtmlVideoElement;

#[component]
pub fn VideoPlayer(
    source: Signal<String>,
    on_progress: Callback<f64>,
) -> impl IntoView {
    let video_ref = create_node_ref::<html::Video>();
    let is_playing = create_rw_signal(false);
    let current_time = create_rw_signal(0.0);
    let duration = create_rw_signal(0.0);
    
    // Report progress every 5 seconds
    let _ = use_interval(5000, move || {
        if is_playing.get() {
            on_progress.call(current_time.get());
        }
    });
    
    // Video event handlers
    create_effect(move |_| {
        if let Some(video) = video_ref.get() {
            let _ = use_event_listener(video.clone(), ev::loadedmetadata, move |_| {
                duration.set(video.duration());
            });
            
            let _ = use_event_listener(video.clone(), ev::timeupdate, move |_| {
                current_time.set(video.current_time());
            });
        }
    });
    
    view! {
        <div class="video-player">
            <video
                ref=video_ref
                class="video-element"
                src=source
                on:play=move |_| is_playing.set(true)
                on:pause=move |_| is_playing.set(false)
            />
            
            <PlayerControls
                video_ref=video_ref
                is_playing=is_playing
                current_time=current_time.read_only()
                duration=duration.read_only()
            />
        </div>
    }
}
```

## macOS Native Design System

### 1. CSS Design Tokens
```css
/* src/platforms/tauri/ui/style/main.css */
@layer base {
  :root {
    /* macOS System Colors */
    --color-background: rgba(30, 30, 30, 0.85);
    --color-surface: rgba(45, 45, 45, 0.95);
    --color-elevated: rgba(60, 60, 60, 0.95);
    
    /* Text Colors with macOS opacity values */
    --color-text-primary: rgba(255, 255, 255, 0.85);
    --color-text-secondary: rgba(255, 255, 255, 0.55);
    --color-text-tertiary: rgba(255, 255, 255, 0.25);
    
    /* Accent Colors (macOS defaults) */
    --color-accent-blue: #007AFF;
    --color-accent-purple: #BF5AF2;
    --color-accent-pink: #FF375F;
    --color-accent-green: #30D158;
    
    /* Spacing (macOS standard) */
    --spacing-xs: 4px;
    --spacing-sm: 8px;
    --spacing-md: 12px;
    --spacing-lg: 20px;
    --spacing-xl: 24px;
    
    /* Border Radius */
    --radius-sm: 6px;
    --radius-md: 10px;
    --radius-lg: 12px;
    --radius-xl: 16px;
    
    /* Shadows */
    --shadow-sm: 0 1px 3px rgba(0, 0, 0, 0.12);
    --shadow-md: 0 4px 6px rgba(0, 0, 0, 0.16);
    --shadow-lg: 0 10px 40px rgba(0, 0, 0, 0.2);
    
    /* Typography */
    --font-system: -apple-system, BlinkMacSystemFont, 'SF Pro Text', system-ui;
    --font-display: -apple-system, BlinkMacSystemFont, 'SF Pro Display', system-ui;
  }
}

/* Window styling */
.app-container {
  background: var(--color-background);
  backdrop-filter: blur(50px);
  -webkit-backdrop-filter: blur(50px);
  font-family: var(--font-system);
  height: 100vh;
  display: grid;
  grid-template-columns: 240px 1fr;
}

/* Draggable regions for custom titlebar */
[data-tauri-drag-region] {
  -webkit-app-region: drag;
}

.toolbar button,
.sidebar-nav {
  -webkit-app-region: no-drag;
}

/* Sidebar with vibrancy */
.sidebar {
  background: rgba(30, 30, 30, 0.65);
  backdrop-filter: blur(50px);
  border-right: 1px solid rgba(255, 255, 255, 0.1);
  padding: var(--spacing-lg);
  padding-top: 48px; /* Space for traffic lights */
}

/* Native-feeling animations */
* {
  transition: background-color 200ms ease,
              transform 200ms cubic-bezier(0.4, 0, 0.2, 1),
              opacity 200ms ease;
}

/* Reduced motion support */
@media (prefers-reduced-motion: reduce) {
  * {
    transition: none !important;
    animation: none !important;
  }
}
```

### 2. Tauri Window Configuration
```json
// tauri.conf.json
{
  "tauri": {
    "windows": [{
      "title": "Reel",
      "width": 1280,
      "height": 800,
      "minWidth": 900,
      "minHeight": 600,
      "transparent": true,
      "decorations": true,
      "titleBarStyle": "Overlay",
      "theme": "Dark",
      "center": true,
      "resizable": true,
      "fullscreen": false
    }],
    "bundle": {
      "identifier": "dev.arsfeld.Reel",
      "icon": [
        "icons/32x32.png",
        "icons/128x128.png",
        "icons/128x128@2x.png",
        "icons/icon.icns",
        "icons/icon.ico"
      ],
      "category": "public.app-category.entertainment",
      "macOS": {
        "entitlements": "entitlements.plist",
        "exceptionDomain": "NSAllowsArbitraryLoads",
        "frameworks": [],
        "minimumSystemVersion": "10.15"
      }
    }
  }
}
```

## Tauri Backend Integration

### 1. Command Handlers
```rust
// src/platforms/tauri/commands.rs
use tauri::State;
use crate::core::state::AppState;

#[tauri::command]
pub async fn initialize_core(
    state: State<'_, Arc<AppState>>
) -> Result<(), String> {
    // Core is already initialized in run()
    Ok(())
}

#[tauri::command]
pub async fn get_libraries(
    source_id: String,
    state: State<'_, Arc<AppState>>
) -> Result<Vec<Library>, String> {
    state.data_service()
        .get_libraries(&source_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_media_items(
    library_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
    state: State<'_, Arc<AppState>>
) -> Result<Vec<MediaItem>, String> {
    state.data_service()
        .get_library_items(&library_id, limit, offset)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn play_media(
    item_id: String,
    state: State<'_, Arc<AppState>>
) -> Result<PlaybackInfo, String> {
    // Get stream URL from backend
    let media_item = state.data_service()
        .get_media_item(&item_id)
        .await
        .map_err(|e| e.to_string())?;
    
    // Return playback info with stream URL
    Ok(PlaybackInfo {
        url: media_item.stream_url,
        position: media_item.playback_position,
    })
}

#[tauri::command]
pub async fn update_progress(
    item_id: String,
    position_ms: u64,
    state: State<'_, Arc<AppState>>
) -> Result<(), String> {
    state.data_service()
        .update_playback_position(&item_id, position_ms)
        .await
        .map_err(|e| e.to_string())
}
```

### 2. Event Forwarding
```rust
// src/platforms/tauri/events.rs
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;

pub fn setup_event_bridge(
    app: &AppHandle,
    event_bus: Arc<EventBus>
) {
    let app_handle = app.clone();
    let mut subscriber = event_bus.subscribe_all();
    
    tauri::async_runtime::spawn(async move {
        while let Ok(event) = subscriber.recv().await {
            let event_name = match &event {
                Event::MediaCreated(_) => "media:created",
                Event::MediaUpdated(_) => "media:updated",
                Event::MediaBatchCreated(_) => "media:batch_created",
                Event::LibraryUpdated(_) => "library:updated",
                Event::SyncStarted(_) => "sync:started",
                Event::SyncProgress(_) => "sync:progress",
                Event::SyncCompleted(_) => "sync:completed",
                Event::PlaybackStarted(_) => "playback:started",
                Event::PlaybackProgress(_) => "playback:progress",
                _ => "event:unknown",
            };
            
            app_handle.emit_all(event_name, &event).ok();
        }
    });
}
```

### 3. macOS-Specific Setup
```rust
// src/platforms/tauri/window.rs
#[cfg(target_os = "macos")]
use cocoa::appkit::{NSWindow, NSWindowStyleMask};
use tauri::Window;

#[cfg(target_os = "macos")]
pub fn setup_macos_window(window: &Window) {
    use cocoa::base::id;
    use objc::runtime::YES;
    
    let ns_window = window.ns_window().expect("no ns window") as id;
    
    unsafe {
        // Enable vibrancy
        ns_window.setOpaque_(NO);
        ns_window.setBackgroundColor_(nil);
        
        // Set titlebar style
        ns_window.setTitlebarAppearsTransparent_(YES);
        ns_window.setTitleVisibility_(NSWindowTitleHidden);
        
        // Enable full size content view
        let masks = NSWindowStyleMask::NSFullSizeContentViewWindowMask
            | NSWindowStyleMask::NSResizableWindowMask
            | NSWindowStyleMask::NSMiniaturizableWindowMask
            | NSWindowStyleMask::NSClosableWindowMask
            | NSWindowStyleMask::NSTitledWindowMask;
        ns_window.setStyleMask_(masks);
    }
}
```

## Native Menu System
```rust
// src/platforms/tauri/menu.rs
use tauri::{Menu, Submenu, MenuItem, CustomMenuItem};

pub fn create_menu() -> Menu {
    let app_menu = Submenu::new(
        "Reel",
        Menu::new()
            .add_native_item(MenuItem::About("Reel".to_string(), Default::default()))
            .add_native_item(MenuItem::Separator)
            .add_item(CustomMenuItem::new("preferences", "Preferences...")
                .accelerator("Cmd+,"))
            .add_native_item(MenuItem::Separator)
            .add_native_item(MenuItem::Quit),
    );
    
    let file_menu = Submenu::new(
        "File",
        Menu::new()
            .add_item(CustomMenuItem::new("add_source", "Add Media Source...")
                .accelerator("Cmd+N"))
            .add_item(CustomMenuItem::new("refresh", "Refresh Library")
                .accelerator("Cmd+R"))
            .add_native_item(MenuItem::Separator)
            .add_item(CustomMenuItem::new("import", "Import Media...")
                .accelerator("Cmd+I")),
    );
    
    let view_menu = Submenu::new(
        "View",
        Menu::new()
            .add_item(CustomMenuItem::new("view_grid", "Grid View")
                .accelerator("Cmd+1"))
            .add_item(CustomMenuItem::new("view_list", "List View")
                .accelerator("Cmd+2"))
            .add_native_item(MenuItem::Separator)
            .add_item(CustomMenuItem::new("sidebar", "Toggle Sidebar")
                .accelerator("Cmd+S"))
            .add_item(CustomMenuItem::new("fullscreen", "Enter Full Screen")
                .accelerator("Cmd+Ctrl+F")),
    );
    
    let playback_menu = Submenu::new(
        "Playback",
        Menu::new()
            .add_item(CustomMenuItem::new("play_pause", "Play/Pause")
                .accelerator("Space"))
            .add_item(CustomMenuItem::new("next", "Next")
                .accelerator("Cmd+Right"))
            .add_item(CustomMenuItem::new("previous", "Previous")
                .accelerator("Cmd+Left"))
            .add_native_item(MenuItem::Separator)
            .add_item(CustomMenuItem::new("volume_up", "Volume Up")
                .accelerator("Cmd+Up"))
            .add_item(CustomMenuItem::new("volume_down", "Volume Down")
                .accelerator("Cmd+Down")),
    );
    
    Menu::new()
        .add_submenu(app_menu)
        .add_submenu(file_menu)
        .add_submenu(view_menu)
        .add_submenu(playback_menu)
}
```

## Build Configuration

### 1. Leptos UI Build Configuration
```toml
# src/platforms/tauri/ui/Cargo.toml (if separate crate)
# OR add to main Cargo.toml under [target.'cfg(feature = "tauri")'.dependencies]
[package]
name = "reel-ui"
version = "0.1.0"
edition = "2021"

[dependencies]
leptos = { version = "0.6", features = ["csr", "nightly"] }
leptos_router = { version = "0.6", features = ["csr"] }
leptos_use = "0.10"
serde = { workspace = true }
serde_json = { workspace = true }
wasm-bindgen = "0.2"
web-sys = { version = "0.3", features = [
    "HtmlVideoElement",
    "HtmlElement", 
    "Window",
    "Document",
    "Element",
    "Event",
    "EventTarget",
    "CustomEvent",
    "IntersectionObserver",
    "IntersectionObserverEntry"
]}
serde-wasm-bindgen = "0.6"
gloo-timers = { version = "0.3", features = ["futures"] }
wasm-bindgen-futures = "0.4"

[build-dependencies]
leptos_build = { version = "0.6" }
```

### 2. Build Script
```rust
// build.rs (at root, with feature gate)
fn main() {
    // Existing build logic...
    
    #[cfg(feature = "tauri")]
    {
        // Compile Tailwind CSS for Tauri UI
        println!("cargo:rerun-if-changed=src/platforms/tauri/ui/style/main.css");
        println!("cargo:rerun-if-changed=tailwind.config.js");
        
        if std::path::Path::new("tailwind.config.js").exists() {
            std::process::Command::new("npx")
                .args(&[
                    "tailwindcss", 
                    "-i", "src/platforms/tauri/ui/style/main.css", 
                    "-o", "dist/main.css", 
                    "--minify"
                ])
                .status()
                .ok();
        }
        
        // Create src-tauri shim if needed
        if !std::path::Path::new("src-tauri").exists() {
            std::fs::create_dir_all("src-tauri/src").ok();
            std::fs::write(
                "src-tauri/src/main.rs",
                "fn main() { reel::platforms::tauri::run().unwrap(); }"
            ).ok();
        }
    }
}
```

### 3. Development Workflow
```bash
# Install dependencies
cargo install trunk
cargo install wasm-bindgen-cli
npm install -D tailwindcss

# Development (from root)
cargo tauri dev

# Build for production
cargo tauri build --target universal-apple-darwin

# Run tests
cargo test --workspace
cargo leptos test # Component tests
```

## Testing Strategy

### 1. Component Tests
```rust
// src/platforms/tauri/ui/components/media_card.test.rs
#[cfg(test)]
mod tests {
    use super::*;
    use leptos::*;
    use leptos_test::*;
    
    #[test]
    fn test_media_card_renders() {
        let item = MediaItem {
            id: "1".to_string(),
            title: "Test Movie".to_string(),
            year: 2024,
            poster_url: "/test.jpg".to_string(),
        };
        
        mount(|| view! { <MediaCard item=item.clone() /> });
        
        assert_text_content!(".media-title", "Test Movie");
        assert_text_content!(".media-year", "2024");
        assert_attr!("img", "src", "/test.jpg");
    }
}
```

### 2. Integration Tests
```rust
// tests/tauri_integration.rs
#[cfg(test)]
mod tests {
    use tauri::test::{mock_builder, MockRuntime};
    
    #[test]
    fn test_command_get_libraries() {
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![get_libraries])
            .build(tauri::generate_context!())
            .expect("failed to build app");
        
        let window = app.get_window("main").unwrap();
        let result = tauri::test::get_ipc_response::<Vec<Library>>(
            &window,
            tauri::test::InvokeRequest {
                cmd: "get_libraries".into(),
                args: serde_json::json!({ "source_id": "test" }),
            },
        );
        
        assert!(result.is_ok());
    }
}
```

## Implementation Timeline

### Week 1: Foundation
- **Day 1-2**: Setup Leptos + Tauri project structure
- **Day 3**: Implement core Tauri commands and state management
- **Day 4**: Create event forwarding system
- **Day 5**: Basic routing and layout components

### Week 2: Core UI Components
- **Day 1-2**: Sidebar, toolbar, and navigation
- **Day 3**: Media grid with virtual scrolling
- **Day 4**: Media card and list components
- **Day 5**: Search and filter components

### Week 3: Pages Implementation
- **Day 1**: Home page with carousels
- **Day 2**: Library page with sorting/filtering
- **Day 3**: Media details page
- **Day 4**: Settings and sources pages
- **Day 5**: Integration with backend events

### Week 4: Video Player
- **Day 1-2**: HTML5 video player component
- **Day 3**: Custom controls and progress tracking
- **Day 4**: HLS.js integration for streaming
- **Day 5**: Playback state synchronization

### Week 5: macOS Polish
- **Day 1**: Window vibrancy and titlebar
- **Day 2**: Native menus and keyboard shortcuts
- **Day 3**: System integration (media keys, dock)
- **Day 4**: Animations and transitions
- **Day 5**: Performance optimization

### Week 6: Production Readiness
- **Day 1-2**: Testing suite completion
- **Day 3**: Build and packaging setup
- **Day 4**: Code signing and notarization
- **Day 5**: Documentation and release

## Performance Optimizations

### 1. WASM Size Reduction
```toml
# Profile for WASM optimization
[profile.wasm-release]
inherits = "release"
opt-level = "z"
lto = true
codegen-units = 1
strip = true
```

### 2. Lazy Loading
- Components load on-demand with Suspense boundaries
- Images use native lazy loading
- Virtual scrolling for large lists

### 3. Caching Strategy
- Leptos memos for computed values
- Browser caching for static assets
- Persistent state in localStorage

## Security Considerations

### 1. Content Security Policy
```rust
// src-tauri/src/main.rs
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_csp::init(
            tauri_plugin_csp::Config::new()
                .default_src(vec!["'self'"])
                .img_src(vec!["'self'", "https:", "data:"])
                .media_src(vec!["'self'", "https:", "blob:"])
                .connect_src(vec!["'self'", "https://plex.tv", "https://jellyfin.org"])
        ))
        .run(tauri::generate_context!())
        .expect("error while running application");
}
```

### 2. Input Validation
- All commands validate input on Rust side
- No direct SQL queries from frontend
- Sanitized file paths and URLs

## Advantages of This Approach

### Over JavaScript/React
- **Type Safety**: Compile-time guarantees across entire stack
- **Performance**: Smaller bundle, faster execution
- **Unified Language**: One language for entire application
- **Better IDE Support**: Full IntelliSense and refactoring

### Over Native Swift
- **Cross-Platform**: Same code runs on Windows/Linux
- **Faster Development**: Hot reload, web tooling
- **Easier Maintenance**: Single codebase
- **Community**: Leverage Rust ecosystem

### Over GTK
- **Modern UI**: Web-based flexibility
- **Better macOS Integration**: Native window chrome
- **Easier Styling**: CSS instead of GTK themes
- **Faster Iteration**: Hot reload during development

## Conclusion

This Leptos + Tauri approach delivers a 100% Rust application with native macOS look and feel, while maintaining the flexibility of web technologies for UI. The architecture maximizes code reuse from the existing core, provides type safety across the entire stack, and delivers performance comparable to native applications.

The six-week timeline is achievable with the existing backend already in place, and the resulting application will be maintainable, performant, and truly cross-platform.