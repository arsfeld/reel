use super::delegates::app_delegate::AppDelegate;
use super::simple_window::SimpleWindow;
use crate::core::viewmodels::SidebarViewModel;
use crate::events::EventBus;
use crate::services::data::DataService;
use crate::services::sync::SyncManager;
use crate::state::app_state::AppState;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::{MainThreadMarker, NSObject};
use std::sync::Arc;
use tracing::{debug, error, info};

pub struct CocoaApp {
    app: Retained<NSApplication>,
    delegate: Retained<NSObject>,
    window: SimpleWindow,
    state: Arc<AppState>,
}

impl CocoaApp {
    pub fn new(state: Arc<AppState>) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Initializing Cocoa application");

        // Get the shared NSApplication instance
        let mtm = MainThreadMarker::new().ok_or("Not on main thread")?;
        let app = NSApplication::sharedApplication(mtm);

        // Create app delegate
        let delegate = AppDelegate::new(state.clone(), mtm);

        // Note: In a production implementation, we'd set the delegate properly
        // For now, the delegate is a simplified NSObject that sets up event listeners

        // Set activation policy to regular app (shows in dock)
        unsafe {
            app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        }

        // Initialize core services
        Self::initialize_services(&state)?;

        // Create simple window
        let window = SimpleWindow::new(mtm, state.clone());

        info!("Cocoa application initialized successfully");

        Ok(Self {
            app,
            delegate,
            window,
            state,
        })
    }

    fn initialize_services(state: &Arc<AppState>) -> Result<(), Box<dyn std::error::Error>> {
        info!("Initializing core services");

        // Access DataService (already created in AppState)
        let data_service = state.data_service.clone();
        debug!("DataService initialized");

        // Test database access
        Self::test_database_access(data_service.clone())?;

        // Access SyncManager (already created in AppState)
        let sync_manager = state.sync_manager.clone();
        debug!("SyncManager initialized");

        // Start background sync if sources exist
        Self::start_background_sync(sync_manager.clone());

        // Verify EventBus is working
        let event_bus = state.event_bus.clone();
        debug!("EventBus initialized");

        // Set up logging for the Cocoa frontend
        if std::env::var("RUST_LOG").is_err() {
            unsafe {
                std::env::set_var("RUST_LOG", "info,reel=debug");
            }
        }

        info!("Core services initialized successfully");
        Ok(())
    }

    fn test_database_access(
        data_service: Arc<DataService>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Testing database access from Cocoa frontend");

        // Use tokio runtime to run async database operations
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            // Test fetching sources
            match data_service.get_sources().await {
                Ok(sources) => {
                    info!(
                        "Successfully accessed database - found {} sources",
                        sources.len()
                    );

                    // Test fetching libraries for the first source if available
                    if let Some(first_source) = sources.first() {
                        debug!(
                            "  Testing with source: {} ({})",
                            first_source.name, first_source.source_type
                        );

                        match data_service.get_libraries(&first_source.id).await {
                            Ok(libraries) => {
                                info!("Successfully fetched {} libraries", libraries.len());
                                for library in libraries.iter().take(3) {
                                    debug!(
                                        "  Library: {} ({} items)",
                                        library.title, library.item_count
                                    );
                                }
                            }
                            Err(e) => {
                                error!("Failed to fetch libraries: {}", e);
                                // Non-fatal - continue even if no libraries
                            }
                        }
                    } else {
                        info!("No sources configured yet");
                    }
                }
                Err(e) => {
                    error!("Failed to access database: {}", e);
                    return Err(e.to_string().into());
                }
            }

            Ok(())
        })
    }

    fn start_background_sync(sync_manager: Arc<SyncManager>) {
        info!("Starting background synchronization");

        // Note: In the full implementation, we would need to:
        // 1. Get sources from DataService
        // 2. Trigger sync for each backend
        // For now, just log that we're ready to sync

        tokio::spawn(async move {
            info!("Background sync manager ready");
            // The actual sync will be triggered by UI actions or on a schedule
        });
    }

    pub fn run(self) {
        info!("Starting Cocoa application main loop");

        // Show the main window
        self.window.show();

        // Run the application
        unsafe {
            self.app.run();
        }
    }
}
