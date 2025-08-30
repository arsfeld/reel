use crate::platforms::cocoa::controllers::{NavigationController, NavigationDestination};
use objc2::{msg_send, rc::Retained};
use objc2_app_kit::{NSButton, NSStackView, NSUserInterfaceLayoutOrientation, NSView};
use objc2_foundation::{MainThreadMarker, NSString};
use std::sync::Arc;
use tracing::{debug, info};

/// A simple sidebar with clickable buttons for navigation
pub struct SimpleSidebar {
    container: Retained<NSView>,
    navigation_controller: Option<Arc<NavigationController>>,
}

impl SimpleSidebar {
    pub fn new(mtm: MainThreadMarker) -> Self {
        debug!("Creating simple sidebar");

        // Create container
        let container = unsafe { NSView::new(mtm) };
        unsafe {
            container.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        // Create stack view for buttons
        let stack = unsafe { NSStackView::new(mtm) };
        unsafe {
            stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            stack.setSpacing(10.0);
            stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        // Create Home button
        let home_button = unsafe {
            let btn = NSButton::new(mtm);
            btn.setTitle(&NSString::from_str("🏠 Home"));
            btn.setTranslatesAutoresizingMaskIntoConstraints(false);
            btn
        };

        // Create Sources button
        let sources_button = unsafe {
            let btn = NSButton::new(mtm);
            btn.setTitle(&NSString::from_str("⚙️ Sources/Accounts"));
            btn.setTranslatesAutoresizingMaskIntoConstraints(false);
            btn
        };

        // Add buttons to stack
        unsafe {
            stack.addArrangedSubview(&home_button);
            stack.addArrangedSubview(&sources_button);
        }

        // Add stack to container
        unsafe {
            container.addSubview(&stack);

            // Simple constraints to make stack fill container with padding
            use crate::platforms::cocoa::utils::AutoLayout;
            let constraints = vec![
                AutoLayout::top(&stack, 20.0),
                AutoLayout::leading(&stack, 10.0),
                AutoLayout::trailing(&stack, -10.0),
            ];
            AutoLayout::activate(&constraints);
        }

        Self {
            container,
            navigation_controller: None,
        }
    }

    pub fn set_navigation_controller(&mut self, nav: Arc<NavigationController>) {
        self.navigation_controller = Some(nav.clone());

        // Set up button actions
        self.setup_button_actions();
    }

    fn setup_button_actions(&self) {
        // Note: In objc2 0.6, setting up actions is complex
        // For now we'll use a polling approach similar to the sidebar monitor
        // In production, you'd use proper target-action pattern

        if let Some(nav) = &self.navigation_controller {
            info!("Sidebar buttons configured - click detection would be implemented here");
            // TODO: Implement actual button click handlers
            // For testing, we'll immediately navigate to Sources
            nav.navigate_to(NavigationDestination::Sources);
        }
    }

    pub fn view(&self) -> &NSView {
        &self.container
    }
}
