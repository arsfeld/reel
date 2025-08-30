use super::controllers::{NavigationController, NavigationDestination};
use super::delegates::SidebarDelegate;
use super::error::{CocoaError, CocoaResult};
use super::utils::{AutoLayout, NSEdgeInsets};
use super::views::{ContainerView, SidebarDataSource, SidebarDestination, SidebarView};
use crate::core::viewmodels::SidebarViewModel;
use objc2::{ClassType, msg_send, msg_send_id, rc::Retained};
use objc2_app_kit::{NSBackingStoreType, NSSplitView, NSView, NSWindow, NSWindowStyleMask};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};
use std::sync::{Arc, Mutex};

pub struct MainWindow {
    window: Retained<NSWindow>,
    navigation_controller: Option<Arc<NavigationController>>,
    sidebar_view: Option<SidebarView>,
}

impl MainWindow {
    pub fn new(mtm: MainThreadMarker) -> CocoaResult<Self> {
        // Create window frame
        let frame = NSRect::new(NSPoint::new(100.0, 100.0), NSSize::new(1200.0, 800.0));

        // Create window with standard style
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable;

        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                mtm.alloc::<NSWindow>(),
                frame,
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };

        // Window is guaranteed to be valid if initWithContentRect returns without panic

        // Set window title
        unsafe {
            window.setTitle(&NSString::from_str("Reel"));
        }

        // Center the window
        unsafe {
            window.center();
        }

        // Set minimum size
        unsafe {
            window.setContentMinSize(NSSize::new(800.0, 600.0));
        }

        Ok(Self {
            window,
            navigation_controller: None,
            sidebar_view: None,
        })
    }

    pub fn show(&self) {
        unsafe {
            self.window.makeKeyAndOrderFront(None);
        }
    }

    /// Set up the window layout with sidebar and navigation
    pub fn setup_layout(
        &mut self,
        mtm: MainThreadMarker,
        sidebar_vm: Arc<SidebarViewModel>,
        app_state: Arc<crate::state::app_state::AppState>,
    ) -> CocoaResult<()> {
        // Create split view for sidebar and content
        let split_view = unsafe { NSSplitView::new(mtm) };
        unsafe {
            split_view.setVertical(true);
            split_view.setDividerStyle(objc2_app_kit::NSSplitViewDividerStyle::Thin);
        }

        // Create simple sidebar for testing
        // TODO: Replace with full SidebarView once delegates are working
        use crate::platforms::cocoa::views::simple_sidebar::SimpleSidebar;
        let mut simple_sidebar = SimpleSidebar::new(mtm);

        // Keep the old sidebar code commented for reference
        // let mut sidebar = SidebarView::new(mtm, sidebar_vm);
        // sidebar.set_width(250.0);

        // Create content container
        let content_container = unsafe { NSView::new(mtm) };

        // Add views to split view
        unsafe {
            split_view.addSubview(simple_sidebar.view());
            split_view.addSubview(&content_container);

            // Set minimum width for sidebar
            split_view.setHoldingPriority_forSubviewAtIndex(250.0, 0);
        }

        // Set split view as window content
        unsafe {
            self.window.setContentView(Some(&split_view));
        }

        // Create navigation controller
        let nav_controller = Arc::new(NavigationController::new(content_container, app_state));

        // Connect simple sidebar to navigation controller
        simple_sidebar.set_navigation_controller(nav_controller.clone());

        // Navigate to Sources immediately for testing
        nav_controller.navigate_to(NavigationDestination::Sources);

        // Store references
        self.navigation_controller = Some(nav_controller);
        // self.sidebar_view = Some(sidebar);  // Commented out for now

        Ok(())
    }

    /// Get the navigation controller
    pub fn navigation_controller(&self) -> Option<Arc<NavigationController>> {
        self.navigation_controller.clone()
    }

    /// Handle window resize
    pub fn handle_resize(&self) {
        // This will be called when window resizes
        // Views using Auto Layout will automatically adjust
    }

    /// Toggle fullscreen
    pub fn toggle_fullscreen(&self) {
        unsafe {
            self.window.toggleFullScreen(None);
        }
    }

    /// Check if window is in fullscreen
    pub fn is_fullscreen(&self) -> bool {
        unsafe {
            let style_mask = self.window.styleMask();
            style_mask.contains(NSWindowStyleMask::FullScreen)
        }
    }

    /// Set window title
    pub fn set_title(&self, title: &str) {
        unsafe {
            self.window.setTitle(&NSString::from_str(title));
        }
    }
}
