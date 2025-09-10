use std::cell::RefCell;
use std::sync::Arc;

use gtk4::{glib, prelude::*};
use libadwaita as adw;

use super::{NavigationPage, NavigationState};

/// Central navigation manager that coordinates all navigation operations
#[derive(Debug)]
pub struct NavigationManager {
    state: NavigationState,
    content_stack: RefCell<Option<gtk4::Stack>>,
    content_header: adw::HeaderBar,
    back_button: RefCell<Option<gtk4::Button>>,
}

impl NavigationManager {
    /// Create a new NavigationManager with the given header bar
    pub fn new(content_header: adw::HeaderBar) -> Self {
        let state = NavigationState::new();

        let manager = Self {
            state,
            content_stack: RefCell::new(None),
            content_header,
            back_button: RefCell::new(None),
        };

        manager.setup_reactive_bindings();
        manager
    }

    /// Set the content stack that this manager should control
    pub fn set_content_stack(&self, stack: gtk4::Stack) {
        self.content_stack.replace(Some(stack));
    }

    /// Setup back button callback after NavigationManager is in Arc
    /// This must be called after the NavigationManager is wrapped in Arc to avoid circular dependencies
    pub fn setup_back_button_callback(self: &Arc<Self>) {
        if let Some(button) = self.back_button.borrow().as_ref() {
            let manager_weak = Arc::downgrade(self);
            button.connect_clicked(move |_| {
                if let Some(manager) = manager_weak.upgrade() {
                    glib::spawn_future_local(async move {
                        manager.go_back().await;
                    });
                }
            });
        }
    }

    /// Navigate to a new page, adding it to history
    pub async fn navigate_to(&self, page: NavigationPage) {
        let mut history = self.state.navigation_history.get_sync();
        history.push(page.clone());
        self.state.navigation_history.set(history).await;
        self.state.current_page.set(page.clone()).await;

        // Temporary manual UI updates until reactive bindings are implemented
        self.update_ui_for_current_state();
    }

    /// Navigate back to the previous page
    pub async fn go_back(&self) {
        let mut history = self.state.navigation_history.get_sync();
        if history.len() > 1 {
            history.pop(); // Remove current page
            let previous_page = history.last().cloned().unwrap();
            self.state.navigation_history.set(history).await;
            self.state.current_page.set(previous_page.clone()).await;

            // Temporary manual UI updates until reactive bindings are implemented
            self.update_ui_for_current_state();
        }
    }

    /// Replace the current page without adding to history (useful for redirects)
    pub async fn replace_current(&self, page: NavigationPage) {
        let mut history = self.state.navigation_history.get_sync();
        if !history.is_empty() {
            history.pop(); // Remove current page
            history.push(page.clone()); // Add new page
            self.state.navigation_history.set(history).await;
            self.state.current_page.set(page.clone()).await;

            // Temporary manual UI updates until reactive bindings are implemented
            self.update_ui_for_current_state();
        } else {
            self.navigate_to(page).await;
        }
    }

    /// Clear navigation history and go to page (useful for login/logout)
    pub async fn navigate_to_root(&self, page: NavigationPage) {
        self.state.navigation_history.set(vec![page.clone()]).await;
        self.state.current_page.set(page.clone()).await;

        // Temporary manual UI updates until reactive bindings are implemented
        self.update_ui_for_current_state();
    }

    /// Get current navigation context
    pub fn current_page(&self) -> NavigationPage {
        self.state.current_page()
    }

    /// Get full navigation history
    pub fn navigation_history(&self) -> Vec<NavigationPage> {
        self.state.navigation_history()
    }

    /// Check if back navigation is possible
    pub fn can_go_back(&self) -> bool {
        self.state.can_navigate_back()
    }

    /// Get access to the navigation state for advanced use cases
    pub fn state(&self) -> &NavigationState {
        &self.state
    }

    /// Set up reactive bindings between navigation state and UI
    fn setup_reactive_bindings(&self) {
        self.setup_back_button_bindings();
        self.setup_header_title_bindings();
        self.setup_stack_bindings();
    }

    /// Set up back button reactive bindings
    fn setup_back_button_bindings(&self) {
        // Create back button reactively
        let _back_button = self.get_or_create_back_button();

        // For now, reactive bindings will be implemented in a future update
        // The weak reference issues mentioned in the documentation need to be resolved first
        // TODO: Implement reactive bindings once weak reference patterns are established
    }

    /// Set up header title reactive bindings
    fn setup_header_title_bindings(&self) {
        // For now, reactive bindings will be implemented in a future update
        // The weak reference issues mentioned in the documentation need to be resolved first
        // TODO: Implement reactive bindings once weak reference patterns are established
    }

    /// Set up stack page switching reactive bindings
    fn setup_stack_bindings(&self) {
        // For now, reactive bindings will be implemented in a future update
        // The weak reference issues mentioned in the documentation need to be resolved first
        // TODO: Implement reactive bindings once weak reference patterns are established
    }

    /// Get or create the back button, ensuring it's properly set up
    fn get_or_create_back_button(&self) -> gtk4::Button {
        if let Some(button) = self.back_button.borrow().as_ref() {
            button.clone()
        } else {
            let button = gtk4::Button::builder()
                .icon_name("go-previous-symbolic")
                .build();

            // Back navigation will be connected via setup_back_button_callback()
            // after NavigationManager is wrapped in Arc

            self.content_header.pack_start(&button);
            self.back_button.replace(Some(button.clone()));

            // Set initial state
            button.set_visible(self.state.should_show_back_button());
            button.set_tooltip_text(Some(&self.state.back_button_tooltip_text()));

            button
        }
    }

    /// Temporary manual UI updates until reactive bindings are implemented
    /// This method reads from the reactive state properties but updates UI manually
    fn update_ui_for_current_state(&self) {
        // Update header title using computed property
        if let Some(title) = self.state.header_title.get_sync() {
            let label = gtk4::Label::builder()
                .label(&title)
                .single_line_mode(true)
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .build();
            self.content_header.set_title_widget(Some(&label));
        } else {
            self.content_header.set_title_widget(gtk4::Widget::NONE);
        }

        // Update back button visibility and tooltip using computed properties
        if let Some(button) = self.back_button.borrow().as_ref() {
            let should_show = self.state.show_back_button.get_sync();
            button.set_visible(should_show);

            let tooltip = self.state.back_button_tooltip.get_sync();
            button.set_tooltip_text(Some(&tooltip));
        }

        // Update stack page using current page property
        if let Some(stack) = self.content_stack.borrow().as_ref() {
            let page = self.state.current_page.get_sync();
            let page_name = page.stack_page_name();
            if stack.child_by_name(&page_name).is_some() {
                stack.set_visible_child_name(&page_name);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full NavigationManager tests are skipped because they require GTK initialization
    // and can be problematic in test environments. These tests focus on core navigation logic.

    #[tokio::test]
    async fn test_navigation_state_operations() {
        // Test that the navigation state operations work correctly
        let state = NavigationState::new();

        // Test initial state
        assert_eq!(state.current_page(), NavigationPage::Empty);
        assert!(!state.can_navigate_back());
        assert!(!state.can_navigate_forward());
        assert_eq!(state.header_title(), None);
        assert!(!state.should_show_back_button());
        assert_eq!(state.back_button_tooltip_text(), "Back");

        // Test navigation to Sources
        let sources_page = NavigationPage::Sources;
        let mut history = state.navigation_history();
        history.push(sources_page.clone());

        state.navigation_history.set(history).await;
        state.current_page.set(sources_page.clone()).await;

        // Small delay for computed properties to update
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        assert_eq!(state.current_page(), sources_page);
        assert!(state.can_navigate_back());
        assert_eq!(state.header_title(), Some("Sources".to_string()));
        assert!(state.should_show_back_button());
        assert_eq!(state.back_button_tooltip_text(), "Back to Content");
    }
}
