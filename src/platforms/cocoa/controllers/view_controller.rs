use objc2::{ClassType, msg_send, msg_send_id, rc::Retained};
use objc2_app_kit::{NSView, NSViewController};
use objc2_foundation::{MainThreadMarker, NSString};
use std::marker::PhantomData;
use std::sync::Arc;
use tracing::debug;

/// View handle that is Send+Sync safe
pub struct ViewHandle {
    // Store a raw pointer that we'll only access on the main thread
    view_ptr: *const NSView,
    _phantom: PhantomData<NSView>,
}

unsafe impl Send for ViewHandle {}
unsafe impl Sync for ViewHandle {}

impl ViewHandle {
    pub fn new(view: &NSView) -> Self {
        Self {
            view_ptr: view as *const NSView,
            _phantom: PhantomData,
        }
    }

    /// Get the view - must only be called on main thread
    pub unsafe fn get(&self) -> &NSView {
        unsafe { &*self.view_ptr }
    }
}

/// Base trait for view controllers in the Reel application
pub trait ReelViewController: Send + Sync {
    /// Get the view handle
    fn view_handle(&self) -> &ViewHandle;

    /// Called when the view will appear
    fn view_will_appear(&self) {
        debug!("View will appear");
    }

    /// Called when the view did appear
    fn view_did_appear(&self) {
        debug!("View did appear");
    }

    /// Called when the view will disappear
    fn view_will_disappear(&self) {
        debug!("View will disappear");
    }

    /// Called when the view did disappear
    fn view_did_disappear(&self) {
        debug!("View did disappear");
    }

    /// Handle cleanup when controller is deallocated
    fn cleanup(&self) {
        debug!("Cleaning up view controller");
    }

    /// Get the title for this view controller
    fn title(&self) -> String {
        "Untitled".to_string()
    }
}

/// Base view controller implementation
pub struct BaseViewController {
    view_handle: ViewHandle,
    title: String,
}

impl BaseViewController {
    pub fn new(mtm: MainThreadMarker, title: impl Into<String>) -> Self {
        let view = unsafe { NSView::new(mtm) };

        Self {
            view_handle: ViewHandle::new(&view),
            title: title.into(),
        }
    }

    pub fn with_view(view: &NSView, title: impl Into<String>) -> Self {
        Self {
            view_handle: ViewHandle::new(view),
            title: title.into(),
        }
    }
}

impl ReelViewController for BaseViewController {
    fn view_handle(&self) -> &ViewHandle {
        &self.view_handle
    }

    fn title(&self) -> String {
        self.title.clone()
    }
}

/// View controller container that manages lifecycle
pub struct ViewControllerContainer {
    controller: Arc<dyn ReelViewController>,
    is_visible: bool,
}

impl ViewControllerContainer {
    pub fn new(controller: Arc<dyn ReelViewController>) -> Self {
        Self {
            controller,
            is_visible: false,
        }
    }

    /// Show the view controller
    pub fn show(&mut self, in_container: &NSView) {
        if !self.is_visible {
            self.controller.view_will_appear();

            unsafe {
                let view = self.controller.view_handle().get();
                in_container.addSubview(view);
            }

            self.controller.view_did_appear();
            self.is_visible = true;
        }
    }

    /// Hide the view controller
    pub fn hide(&mut self) {
        if self.is_visible {
            self.controller.view_will_disappear();

            unsafe {
                let view = self.controller.view_handle().get();
                view.removeFromSuperview();
            }

            self.controller.view_did_disappear();
            self.is_visible = false;
        }
    }

    /// Get the underlying controller
    pub fn controller(&self) -> Arc<dyn ReelViewController> {
        self.controller.clone()
    }

    /// Check if visible
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }
}

/// Stack-based view controller manager
pub struct ViewControllerStack {
    controllers: Vec<ViewControllerContainer>,
    container_view: Retained<NSView>,
}

impl ViewControllerStack {
    pub fn new(container_view: Retained<NSView>) -> Self {
        Self {
            controllers: Vec::new(),
            container_view,
        }
    }

    /// Push a new view controller
    pub fn push(&mut self, controller: Arc<dyn ReelViewController>) {
        // Hide current top controller if any
        if let Some(current) = self.controllers.last_mut() {
            current.hide();
        }

        // Create and show new controller
        let mut container = ViewControllerContainer::new(controller);
        container.show(&self.container_view);

        self.controllers.push(container);
    }

    /// Pop the top view controller
    pub fn pop(&mut self) -> Option<Arc<dyn ReelViewController>> {
        if let Some(mut container) = self.controllers.pop() {
            container.hide();

            // Show previous controller if any
            if let Some(previous) = self.controllers.last_mut() {
                previous.show(&self.container_view);
            }

            Some(container.controller())
        } else {
            None
        }
    }

    /// Replace all controllers with a new one
    pub fn set_root(&mut self, controller: Arc<dyn ReelViewController>) {
        // Hide and remove all current controllers
        for mut container in self.controllers.drain(..) {
            container.hide();
        }

        // Push new root controller
        self.push(controller);
    }

    /// Get the current top controller
    pub fn top(&self) -> Option<Arc<dyn ReelViewController>> {
        self.controllers
            .last()
            .map(|container| container.controller())
    }

    /// Get the number of controllers in the stack
    pub fn count(&self) -> usize {
        self.controllers.len()
    }

    /// Check if stack is empty
    pub fn is_empty(&self) -> bool {
        self.controllers.is_empty()
    }
}
