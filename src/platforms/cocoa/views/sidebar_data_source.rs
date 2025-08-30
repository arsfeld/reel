use objc2::{msg_send, msg_send_id, rc::Retained, runtime::NSObject};
use objc2_app_kit::{NSOutlineView, NSTableColumn};
use objc2_foundation::{MainThreadMarker, NSInteger, NSString};
use std::sync::Arc;
use tracing::debug;

/// Simple sidebar data source to provide basic navigation items
pub struct SidebarDataSource {
    items: Vec<SidebarMenuItem>,
}

#[derive(Debug, Clone)]
pub struct SidebarMenuItem {
    pub title: String,
    pub destination: SidebarDestination,
}

#[derive(Debug, Clone)]
pub enum SidebarDestination {
    Home,
    Sources,
    Library(String),
}

impl SidebarDataSource {
    pub fn new() -> Self {
        // Create basic menu items
        let items = vec![
            SidebarMenuItem {
                title: "Home".to_string(),
                destination: SidebarDestination::Home,
            },
            SidebarMenuItem {
                title: "Sources".to_string(),
                destination: SidebarDestination::Sources,
            },
        ];

        Self { items }
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn item_at(&self, index: usize) -> Option<&SidebarMenuItem> {
        self.items.get(index)
    }

    /// Map row index to navigation destination
    pub fn destination_for_row(&self, row: NSInteger) -> Option<SidebarDestination> {
        if row >= 0 && (row as usize) < self.items.len() {
            Some(self.items[row as usize].destination.clone())
        } else {
            None
        }
    }

    /// Add a library to the sidebar
    pub fn add_library(&mut self, id: String, title: String) {
        self.items.push(SidebarMenuItem {
            title,
            destination: SidebarDestination::Library(id),
        });
    }

    /// Update libraries from sources
    pub fn update_from_sources(&mut self, sources: Vec<(String, String)>) {
        // Remove existing library items
        self.items
            .retain(|item| !matches!(item.destination, SidebarDestination::Library(_)));

        // Add new library items
        for (id, name) in sources {
            self.add_library(id, name);
        }
    }
}
