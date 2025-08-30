use crate::events::EventBus;
use crate::state::app_state::AppState;
use objc2::{rc::Retained, runtime::NSObject as NSObjectRuntime};
use objc2_app_kit::NSApplication;
use objc2_foundation::{MainThreadMarker, NSObject};
use std::sync::{Arc, OnceLock};
use tracing::{debug, info};

// Static storage for the AppState
static APP_STATE: OnceLock<Arc<AppState>> = OnceLock::new();

// For now, we'll use a simpler approach without declare_class macro
// The macro has issues with objc2 0.6 syntax changes
pub struct AppDelegate {
    state: Arc<AppState>,
}

impl AppDelegate {
    pub fn new(state: Arc<AppState>, _mtm: MainThreadMarker) -> Retained<NSObject> {
        // Store the AppState in static storage for later access
        APP_STATE.set(state.clone()).ok();

        // Setup event listeners
        Self::setup_event_listeners(state.clone());

        // For now, return a basic NSObject as the delegate
        // In production, we'd implement proper delegate methods
        NSObject::new()
    }

    fn setup_event_listeners(state: Arc<AppState>) {
        info!("Setting up event listeners for Cocoa frontend");

        // Get the event bus from AppState
        let event_bus = state.event_bus.clone();

        // Set up background task to listen for events
        tokio::spawn(async move {
            let mut subscriber = event_bus.subscribe();

            while let Ok(event) = subscriber.recv().await {
                // Dispatch UI updates to main thread
                dispatch::Queue::main().exec_async(move || {
                    debug!("Received event: {:?}", event);
                    // Handle events that require UI updates
                    // This will be expanded as we implement more UI components
                });
            }
        });

        info!("Event listeners configured");
    }

    pub fn application_did_finish_launching(&self) {
        info!("Application did finish launching");

        // Initialize event system connections
        if let Some(state) = APP_STATE.get() {
            debug!("Application state ready");
        }
    }

    pub fn application_will_terminate(&self) {
        info!("Application will terminate");

        // Clean shutdown of services
        if let Some(state) = APP_STATE.get() {
            debug!("Cleaning up application state");
        }
    }
}
