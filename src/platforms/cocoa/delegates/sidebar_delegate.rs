use crate::platforms::cocoa::controllers::{NavigationController, NavigationDestination};
use objc2::{msg_send, msg_send_id, rc::Retained, runtime::NSObject};
use objc2_app_kit::{NSOutlineView, NSTableView};
use objc2_foundation::{MainThreadMarker, NSInteger, NSString};
use std::sync::Arc;
use tracing::{debug, info};

/// Sidebar delegate handles selection changes and navigation
pub struct SidebarDelegate {
    navigation_controller: Arc<NavigationController>,
}

impl SidebarDelegate {
    pub fn new(navigation_controller: Arc<NavigationController>) -> Self {
        Self {
            navigation_controller,
        }
    }

    /// Handle selection change in the sidebar
    pub fn handle_selection_change(&self, outline_view: &NSOutlineView) {
        unsafe {
            let selected_row = outline_view.selectedRow();
            if selected_row >= 0 {
                debug!("Sidebar selection changed to row: {}", selected_row);

                // Map row index to navigation destination
                // This is a simplified mapping - in production we'd look up the actual item
                let destination = match selected_row {
                    0 => NavigationDestination::Home,
                    1 => NavigationDestination::Sources, // Sources/Accounts
                    2..=10 => {
                        // Library items (would need actual library IDs)
                        NavigationDestination::Library(format!("library_{}", selected_row - 2))
                    }
                    _ => NavigationDestination::Home,
                };

                info!("Navigating to {:?} from sidebar", destination);
                self.navigation_controller.navigate_to(destination);
            }
        }
    }

    /// Populate sidebar with initial items
    pub fn populate_sidebar(&self, outline_view: &NSOutlineView) {
        // For now, we'll just ensure the sidebar has some basic items
        // In production, this would be driven by the SidebarViewModel
        debug!("Populating sidebar with initial items");

        // The actual data source would be set up separately
        // This is just to ensure we have something to click on
    }
}
