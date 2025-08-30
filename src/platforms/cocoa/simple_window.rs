use objc2::{ClassType, msg_send, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSBezelStyle, NSButton, NSColor, NSControlSize, NSFont,
    NSSplitView, NSSplitViewDividerStyle, NSStackView, NSStackViewDistribution, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};
use std::sync::Arc;
use tracing::info;

use crate::core::viewmodels::SourcesViewModel;
use crate::platforms::cocoa::controllers::{NavigationController, NavigationDestination};
use crate::platforms::cocoa::views::native_sources_view::NativeSourcesView;
use crate::state::app_state::AppState;

pub struct SimpleWindow {
    window: Retained<NSWindow>,
    navigation_controller: Arc<NavigationController>,
}

impl SimpleWindow {
    pub fn new(mtm: MainThreadMarker, app_state: Arc<AppState>) -> Self {
        info!("Creating simple production window");

        // Create window
        let window = Self::create_window(mtm);

        // Create split view
        let split_view = unsafe { NSSplitView::new(mtm) };
        unsafe {
            split_view.setVertical(true);
            split_view.setDividerStyle(NSSplitViewDividerStyle::Thin);
        }

        // Create sidebar
        let sidebar = Self::create_sidebar(mtm);

        // Create content area
        let content = unsafe { NSView::new(mtm) };

        // Add to split view
        unsafe {
            split_view.addSubview(&sidebar);
            split_view.addSubview(&content);
            split_view.setPosition_ofDividerAtIndex(250.0, 0);

            window.setContentView(Some(&split_view));
        }

        // Create navigation controller
        let navigation_controller = Arc::new(NavigationController::new(content, app_state.clone()));

        // Navigate to Sources by default for testing
        navigation_controller.navigate_to(NavigationDestination::Sources);

        Self {
            window,
            navigation_controller,
        }
    }

    fn create_window(mtm: MainThreadMarker) -> Retained<NSWindow> {
        let frame = NSRect::new(NSPoint::new(100.0, 100.0), NSSize::new(1200.0, 800.0));

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

        unsafe {
            window.setTitle(&NSString::from_str("Reel - Media Player"));
            window.setContentMinSize(NSSize::new(900.0, 600.0));
            window.center();
        }

        window
    }

    fn create_sidebar(mtm: MainThreadMarker) -> Retained<NSView> {
        let sidebar = unsafe { NSView::new(mtm) };

        // Create stack view for sidebar items
        let stack = unsafe { NSStackView::new(mtm) };
        unsafe {
            stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            stack.setDistribution(NSStackViewDistribution::Fill);
            stack.setSpacing(8.0);
            stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        // Add sidebar title
        let title = unsafe { NSTextField::labelWithString(&NSString::from_str("Navigation"), mtm) };
        unsafe {
            title.setFont(Some(&NSFont::boldSystemFontOfSize(14.0)));
            title.setTextColor(Some(&NSColor::labelColor()));
        }

        // Add navigation items
        let home_label =
            unsafe { NSTextField::labelWithString(&NSString::from_str("🏠 Home"), mtm) };

        let sources_label =
            unsafe { NSTextField::labelWithString(&NSString::from_str("⚙️ Sources"), mtm) };
        unsafe {
            sources_label.setTextColor(Some(&NSColor::systemBlueColor()));
        }

        // Add to stack
        unsafe {
            stack.addArrangedSubview(&title);
            stack.addArrangedSubview(&home_label);
            stack.addArrangedSubview(&sources_label);

            // Add spacer
            let spacer = NSView::new(mtm);
            stack.addArrangedSubview(&spacer);

            sidebar.addSubview(&stack);

            // Layout constraints
            use crate::platforms::cocoa::utils::AutoLayout;
            let constraints = vec![
                AutoLayout::top(&stack, 20.0),
                AutoLayout::leading(&stack, 20.0),
                AutoLayout::trailing(&stack, -20.0),
                AutoLayout::bottom(&stack, -20.0),
            ];
            AutoLayout::activate(&constraints);
        }

        sidebar
    }

    pub fn show(&self) {
        unsafe {
            self.window.makeKeyAndOrderFront(None);

            // Activate app
            let app = NSApplication::sharedApplication(MainThreadMarker::new().unwrap());
            app.activateIgnoringOtherApps(true);
        }
    }
}
