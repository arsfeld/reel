use crate::platforms::cocoa::utils::{AutoLayout, NSEdgeInsets};
use objc2::{msg_send, msg_send_id, rc::Retained};
use objc2_app_kit::{NSStackView, NSUserInterfaceLayoutOrientation, NSView};
use objc2_foundation::MainThreadMarker;

/// A container view that manages layout using NSStackView
pub struct ContainerView {
    view: Retained<NSView>,
    stack: Retained<NSStackView>,
}

impl ContainerView {
    /// Create a new container with vertical stack layout
    pub fn vertical(mtm: MainThreadMarker) -> Self {
        Self::new(mtm, NSUserInterfaceLayoutOrientation::Vertical)
    }

    /// Create a new container with horizontal stack layout
    pub fn horizontal(mtm: MainThreadMarker) -> Self {
        Self::new(mtm, NSUserInterfaceLayoutOrientation::Horizontal)
    }

    /// Create a new container with specified orientation
    fn new(mtm: MainThreadMarker, orientation: NSUserInterfaceLayoutOrientation) -> Self {
        let view = unsafe { NSView::new(mtm) };
        let stack = unsafe { NSStackView::new(mtm) };

        // Configure stack view
        unsafe {
            stack.setOrientation(orientation);
            stack.setDistribution(objc2_app_kit::NSStackViewDistribution::Fill);
            stack.setAlignment(objc2_app_kit::NSLayoutAttribute::CenterY);
            stack.setSpacing(8.0);
        }

        // Add stack to container view
        unsafe {
            view.addSubview(&stack);
        }

        // Pin stack to edges
        let constraints = AutoLayout::pin_to_edges(&stack, NSEdgeInsets::zero());
        AutoLayout::activate(&constraints);

        Self { view, stack }
    }

    /// Add a view to the stack
    pub fn add_view(&self, view: &NSView) {
        self.add_view_with_gravity(view, objc2_app_kit::NSStackViewGravity::Center);
    }

    /// Add a view with specific gravity
    pub fn add_view_with_gravity(&self, view: &NSView, gravity: objc2_app_kit::NSStackViewGravity) {
        unsafe {
            self.stack.addView_inGravity(view, gravity);
        }
    }

    /// Insert a view at specific index
    pub fn insert_view(&self, view: &NSView, at_index: usize) {
        self.insert_view_with_gravity(view, at_index, objc2_app_kit::NSStackViewGravity::Center);
    }

    /// Insert a view at specific index with gravity
    pub fn insert_view_with_gravity(
        &self,
        view: &NSView,
        at_index: usize,
        gravity: objc2_app_kit::NSStackViewGravity,
    ) {
        unsafe {
            self.stack
                .insertView_atIndex_inGravity(view, at_index, gravity);
        }
    }

    /// Remove a view from the stack
    pub fn remove_view(&self, view: &NSView) {
        unsafe {
            self.stack.removeView(view);
        }
    }

    /// Remove all views
    pub fn remove_all_views(&self) {
        unsafe {
            let views = self.stack.arrangedSubviews();
            for view in views.iter() {
                self.stack.removeView(&view);
            }
        }
    }

    /// Set spacing between views
    pub fn set_spacing(&self, spacing: f64) {
        unsafe {
            self.stack.setSpacing(spacing as _);
        }
    }

    /// Set edge insets
    pub fn set_edge_insets(&self, insets: NSEdgeInsets) {
        unsafe {
            self.stack.setEdgeInsets(objc2_foundation::NSEdgeInsets {
                top: insets.top as _,
                left: insets.left as _,
                bottom: insets.bottom as _,
                right: insets.right as _,
            });
        }
    }

    /// Set distribution mode
    pub fn set_distribution(&self, distribution: objc2_app_kit::NSStackViewDistribution) {
        unsafe {
            self.stack.setDistribution(distribution);
        }
    }

    /// Get the underlying NSView
    pub fn view(&self) -> &NSView {
        &self.view
    }

    /// Get the underlying NSStackView
    pub fn stack_view(&self) -> &NSStackView {
        &self.stack
    }

    /// Create a spacer view with flexible width/height
    pub fn create_spacer(mtm: MainThreadMarker) -> Retained<NSView> {
        let spacer = unsafe { NSView::new(mtm) };
        unsafe {
            spacer.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        spacer
    }

    /// Add a flexible spacer to push content
    pub fn add_spacer(&self, mtm: MainThreadMarker) {
        let spacer = Self::create_spacer(mtm);
        self.add_view(&spacer);

        // Set low priority for compression resistance
        unsafe {
            spacer.setContentHuggingPriority_forOrientation(
                1.0,
                objc2_app_kit::NSLayoutConstraintOrientation::Horizontal,
            );
            spacer.setContentHuggingPriority_forOrientation(
                1.0,
                objc2_app_kit::NSLayoutConstraintOrientation::Vertical,
            );
        }
    }
}
