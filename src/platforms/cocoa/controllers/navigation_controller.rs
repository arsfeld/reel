use crate::state::app_state::AppState;
use objc2::{msg_send, msg_send_id, rc::Retained};
use objc2_app_kit::{NSView, NSViewController};
use objc2_foundation::MainThreadMarker;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info};

/// Navigation destinations
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum NavigationDestination {
    Home,
    Library(String),      // Library ID
    MovieDetails(String), // Movie ID
    ShowDetails(String),  // Show ID
    Player(String),       // Media ID
    Sources,
    Settings,
}

/// Navigation controller manages view transitions
pub struct NavigationController {
    current_view: Arc<Mutex<Option<Retained<NSView>>>>,
    current_destination: Arc<Mutex<Option<NavigationDestination>>>,
    view_cache: Arc<Mutex<HashMap<NavigationDestination, Retained<NSView>>>>,
    container_view: Retained<NSView>,
    navigation_stack: Arc<Mutex<Vec<NavigationDestination>>>,
    app_state: Arc<AppState>,
}

impl NavigationController {
    pub fn new(container_view: Retained<NSView>, app_state: Arc<AppState>) -> Self {
        info!("Creating navigation controller");

        Self {
            current_view: Arc::new(Mutex::new(None)),
            current_destination: Arc::new(Mutex::new(None)),
            view_cache: Arc::new(Mutex::new(HashMap::new())),
            container_view,
            navigation_stack: Arc::new(Mutex::new(Vec::new())),
            app_state,
        }
    }

    /// Navigate to a destination
    pub fn navigate_to(&self, destination: NavigationDestination) {
        debug!("Navigating to {:?}", destination);

        // Check if we're already at this destination
        {
            let current = self.current_destination.lock().unwrap();
            if let Some(ref current_dest) = *current {
                if current_dest == &destination {
                    debug!("Already at destination {:?}", destination);
                    return;
                }
            }
        }

        // Get or create view for destination
        let view = self.get_or_create_view(&destination);

        // Perform transition
        self.transition_to_view(view.clone(), destination.clone());

        // Update navigation stack
        {
            let mut stack = self.navigation_stack.lock().unwrap();
            stack.push(destination.clone());

            // Limit stack size to prevent memory issues
            if stack.len() > 20 {
                stack.remove(0);
            }
        }

        // Update current destination
        {
            let mut current = self.current_destination.lock().unwrap();
            *current = Some(destination);
        }
    }

    /// Navigate back in history
    pub fn navigate_back(&self) -> bool {
        let mut stack = self.navigation_stack.lock().unwrap();

        // Need at least 2 items to go back
        if stack.len() < 2 {
            debug!("Cannot navigate back - insufficient history");
            return false;
        }

        // Remove current destination
        stack.pop();

        // Get previous destination
        if let Some(destination) = stack.last().cloned() {
            drop(stack); // Release lock before navigating

            debug!("Navigating back to {:?}", destination);
            let view = self.get_or_create_view(&destination);
            self.transition_to_view(view, destination.clone());

            // Update current destination
            let mut current = self.current_destination.lock().unwrap();
            *current = Some(destination);

            true
        } else {
            false
        }
    }

    /// Clear navigation history
    pub fn clear_history(&self) {
        let mut stack = self.navigation_stack.lock().unwrap();
        stack.clear();

        // Keep current destination if any
        if let Some(current) = &*self.current_destination.lock().unwrap() {
            stack.push(current.clone());
        }
    }

    /// Get or create view for destination
    fn get_or_create_view(&self, destination: &NavigationDestination) -> Retained<NSView> {
        let mut cache = self.view_cache.lock().unwrap();

        // Check cache first
        if let Some(view) = cache.get(destination) {
            debug!("Using cached view for {:?}", destination);
            return view.clone();
        }

        // Create new view
        debug!("Creating new view for {:?}", destination);
        let view = self.create_view_for_destination(destination);

        // Cache the view (except for player views which shouldn't be cached)
        if !matches!(destination, NavigationDestination::Player(_)) {
            cache.insert(destination.clone(), view.clone());
        }

        view
    }

    /// Create view for specific destination
    fn create_view_for_destination(&self, destination: &NavigationDestination) -> Retained<NSView> {
        let mtm = MainThreadMarker::new().expect("Must be on main thread");

        // Set up the view based on destination
        match destination {
            NavigationDestination::Home => {
                debug!("Creating home view");
                // Create a home view instance
                use crate::core::viewmodels::HomeViewModel;
                use crate::platforms::cocoa::views::HomeView;

                // For now, create a placeholder since HomeView isn't fully implemented
                let view = unsafe { NSView::new(mtm) };
                view
            }
            NavigationDestination::Library(id) => {
                debug!("Creating library view for {}", id);
                // Create library view
                use crate::core::viewmodels::LibraryViewModel;
                use crate::platforms::cocoa::views::LibraryView;

                // For now, create a placeholder since LibraryView isn't fully wired up
                let view = unsafe { NSView::new(mtm) };
                view
            }
            NavigationDestination::MovieDetails(id) => {
                debug!("Creating movie details view for {}", id);
                let view = unsafe { NSView::new(mtm) };
                view
            }
            NavigationDestination::ShowDetails(id) => {
                debug!("Creating show details view for {}", id);
                let view = unsafe { NSView::new(mtm) };
                view
            }
            NavigationDestination::Player(id) => {
                debug!("Creating player view for {}", id);
                let view = unsafe { NSView::new(mtm) };
                view
            }
            NavigationDestination::Sources => {
                debug!("Creating sources view");
                // Create native sources view
                use crate::core::viewmodels::SourcesViewModel;
                use crate::platforms::cocoa::views::native_sources_view::NativeSourcesView;

                let sources_vm =
                    Arc::new(SourcesViewModel::new(self.app_state.data_service.clone()));
                let sources_view = NativeSourcesView::new(mtm, sources_vm);
                sources_view.into_view()
            }
            NavigationDestination::Settings => {
                debug!("Creating settings view");
                let view = unsafe { NSView::new(mtm) };
                view
            }
        }
    }

    /// Perform view transition
    fn transition_to_view(&self, new_view: Retained<NSView>, destination: NavigationDestination) {
        debug!("Transitioning to view for {:?}", destination);

        // Remove current view if any
        if let Some(current) = &*self.current_view.lock().unwrap() {
            unsafe {
                current.removeFromSuperview();
            }
        }

        // Add new view to container
        unsafe {
            self.container_view.addSubview(&new_view);

            // Make view fill container
            new_view.setTranslatesAutoresizingMaskIntoConstraints(false);

            // Pin to edges
            use crate::platforms::cocoa::utils::{AutoLayout, NSEdgeInsets};
            let constraints = AutoLayout::pin_to_edges(&new_view, NSEdgeInsets::zero());
            AutoLayout::activate(&constraints);
        }

        // Update current view reference
        let mut current = self.current_view.lock().unwrap();
        *current = Some(new_view);
    }

    /// Get current destination
    pub fn current_destination(&self) -> Option<NavigationDestination> {
        self.current_destination.lock().unwrap().clone()
    }

    /// Get navigation history
    pub fn history(&self) -> Vec<NavigationDestination> {
        self.navigation_stack.lock().unwrap().clone()
    }

    /// Check if can navigate back
    pub fn can_go_back(&self) -> bool {
        self.navigation_stack.lock().unwrap().len() > 1
    }

    /// Clear view cache (useful for memory management)
    pub fn clear_cache(&self) {
        let mut cache = self.view_cache.lock().unwrap();

        // Keep current view in cache if it exists
        let current_dest = self.current_destination.lock().unwrap().clone();
        if let Some(dest) = current_dest {
            if let Some(view) = cache.get(&dest) {
                let view = view.clone();
                cache.clear();
                cache.insert(dest, view);
            } else {
                cache.clear();
            }
        } else {
            cache.clear();
        }

        debug!("View cache cleared, {} views remaining", cache.len());
    }
}
