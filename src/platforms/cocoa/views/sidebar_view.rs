use crate::core::viewmodels::SidebarViewModel;
use crate::platforms::cocoa::controllers::{NavigationController, NavigationDestination};
use crate::platforms::cocoa::utils::{AutoLayout, NSEdgeInsets, ReelColors};
use crate::platforms::cocoa::views::sidebar_data_source::{SidebarDataSource, SidebarDestination};
use objc2::{ClassType, msg_send, msg_send_id, rc::Retained};
use objc2_app_kit::{NSOutlineView, NSScrollView, NSTableColumn, NSView};
use objc2_foundation::{MainThreadMarker, NSString};
use std::sync::{Arc, Mutex};
use tracing::debug;

/// Sidebar navigation view
pub struct SidebarView {
    scroll_view: Retained<NSScrollView>,
    outline_view: Retained<NSOutlineView>,
    view_model: Arc<SidebarViewModel>,
    data_source: Arc<Mutex<SidebarDataSource>>,
    navigation_controller: Option<Arc<NavigationController>>,
}

impl SidebarView {
    pub fn new(mtm: MainThreadMarker, view_model: Arc<SidebarViewModel>) -> Self {
        debug!("Creating sidebar view");

        // Create scroll view container
        let scroll_view = unsafe { NSScrollView::new(mtm) };

        // Create outline view
        let outline_view = unsafe { NSOutlineView::new(mtm) };

        // Configure scroll view
        unsafe {
            scroll_view.setHasVerticalScroller(true);
            scroll_view.setHasHorizontalScroller(false);
            scroll_view.setAutohidesScrollers(true);
            scroll_view.setBorderType(objc2_app_kit::NSBorderType::NoBorder);
            scroll_view.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        // Configure outline view
        unsafe {
            // Basic setup
            outline_view.setFloatsGroupRows(false);
            outline_view.setRowSizeStyle(objc2_app_kit::NSTableViewRowSizeStyle::Default);
            outline_view.setIndentationPerLevel(16.0);

            // Selection
            outline_view.setAllowsMultipleSelection(false);
            outline_view.setAllowsEmptySelection(false);

            // Appearance
            outline_view.setUsesAlternatingRowBackgroundColors(false);
            outline_view.setGridStyleMask(objc2_app_kit::NSTableViewGridLineStyle::empty());

            // Create single column
            let column = NSTableColumn::initWithIdentifier(
                mtm.alloc::<NSTableColumn>(),
                &NSString::from_str("title"),
            );
            column.setTitle(&NSString::from_str("Navigation"));
            column.setWidth(200.0);
            column.setMinWidth(150.0);
            column.setMaxWidth(300.0);

            outline_view.addTableColumn(&column);
            outline_view.setOutlineTableColumn(Some(&column));

            // Hide header
            outline_view.setHeaderView(None);
        }

        // Set outline view as document view
        unsafe {
            scroll_view.setDocumentView(Some(&outline_view));
        }

        // Apply sidebar styling
        Self::apply_styling(&scroll_view);

        // Create data source with basic items
        let data_source = Arc::new(Mutex::new(SidebarDataSource::new()));

        let sidebar = Self {
            scroll_view,
            outline_view,
            view_model,
            data_source,
            navigation_controller: None,
        };

        // Set up bindings
        sidebar.setup_bindings();

        sidebar
    }

    /// Set the navigation controller
    pub fn set_navigation_controller(&mut self, nav_controller: Arc<NavigationController>) {
        self.navigation_controller = Some(nav_controller);
    }

    fn apply_styling(scroll_view: &NSScrollView) {
        unsafe {
            // Set background color
            scroll_view.setBackgroundColor(&ReelColors::sidebar_background());
            scroll_view.setDrawsBackground(true);
        }
    }

    fn setup_bindings(&self) {
        debug!("Setting up sidebar bindings");

        // TODO: Implement delegate and data source
        // This will require creating custom Objective-C classes using declare_class!
        // For now, we'll prepare the structure

        // Note: Property subscription requires async handling
        // In a full implementation, we would spawn a task to listen for changes
        // and update the outline view when sources change.
        // For now, this is a placeholder for the binding setup.
    }

    /// Get the underlying scroll view
    pub fn view(&self) -> &NSScrollView {
        &self.scroll_view
    }

    /// Get the underlying outline view
    pub fn outline_view(&self) -> &NSOutlineView {
        &self.outline_view
    }

    /// Set the width of the sidebar
    pub fn set_width(&self, width: f64) {
        let constraint = AutoLayout::width(&**self.scroll_view, width);
        AutoLayout::activate(&[constraint]);
    }

    /// Handle selection change
    pub fn handle_selection_change(&self) {
        unsafe {
            let selected_row = self.outline_view.selectedRow();
            if selected_row >= 0 {
                debug!("Sidebar selection changed to row: {}", selected_row);

                // Get the destination from our data source
                let data_source = self.data_source.lock().unwrap();
                if let Some(destination) = data_source.destination_for_row(selected_row) {
                    drop(data_source); // Release lock before navigating

                    // Navigate if we have a navigation controller
                    if let Some(nav) = &self.navigation_controller {
                        let nav_dest = match destination {
                            SidebarDestination::Home => NavigationDestination::Home,
                            SidebarDestination::Sources => NavigationDestination::Sources,
                            SidebarDestination::Library(id) => NavigationDestination::Library(id),
                        };
                        nav.navigate_to(nav_dest);
                    }
                }
            }
        }
    }
}

/// Sidebar item types
#[derive(Debug, Clone)]
pub enum SidebarItem {
    Header(String),
    Library {
        id: String,
        title: String,
        item_count: u32,
    },
    Source {
        id: String,
        name: String,
        source_type: String,
    },
    Separator,
}

impl SidebarItem {
    pub fn title(&self) -> String {
        match self {
            Self::Header(title) => title.clone(),
            Self::Library { title, .. } => title.clone(),
            Self::Source { name, .. } => name.clone(),
            Self::Separator => String::new(),
        }
    }

    pub fn is_selectable(&self) -> bool {
        matches!(self, Self::Library { .. } | Self::Source { .. })
    }

    pub fn is_expandable(&self) -> bool {
        matches!(self, Self::Header(_))
    }
}
