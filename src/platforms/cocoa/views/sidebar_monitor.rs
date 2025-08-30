use crate::platforms::cocoa::views::SidebarView;
use dispatch::Queue;
use objc2_app_kit::NSOutlineView;
use objc2_foundation::NSInteger;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::debug;

/// Monitors sidebar selection changes
/// This is a workaround since objc2 0.6 doesn't support custom delegates easily
pub struct SidebarMonitor {
    sidebar_view: Arc<Mutex<Option<Arc<SidebarView>>>>,
    last_selection: Arc<Mutex<NSInteger>>,
}

impl SidebarMonitor {
    pub fn new() -> Self {
        Self {
            sidebar_view: Arc::new(Mutex::new(None)),
            last_selection: Arc::new(Mutex::new(-1)),
        }
    }
    
    pub fn set_sidebar_view(&self, sidebar: Arc<SidebarView>) {
        let mut view = self.sidebar_view.lock().unwrap();
        *view = Some(sidebar);
    }
    
    /// Start monitoring the sidebar for selection changes
    pub fn start_monitoring(&self) {
        let sidebar_view = self.sidebar_view.clone();
        let last_selection = self.last_selection.clone();
        
        // Use a timer to periodically check for selection changes
        // This is not ideal but works around objc2 0.6 limitations
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(100));
                
                let sidebar_opt = sidebar_view.lock().unwrap();
                if let Some(sidebar) = sidebar_opt.as_ref() {
                    // Check current selection
                    let current_selection = unsafe {
                        sidebar.outline_view().selectedRow()
                    };
                    
                    let last = *last_selection.lock().unwrap();
                    if current_selection != last && current_selection >= 0 {
                        debug!("Sidebar selection changed from {} to {}", last, current_selection);
                        
                        // Update last selection
                        *last_selection.lock().unwrap() = current_selection;
                        
                        // Handle the selection change on main thread
                        let sidebar_clone = sidebar.clone();
                        Queue::main().exec_async(move || {
                            sidebar_clone.handle_selection_change();
                        });
                    }
                }
            }
        });
    }
}