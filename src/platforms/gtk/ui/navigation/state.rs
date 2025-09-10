use std::sync::Arc;

use super::types::NavigationPage;
use crate::core::viewmodels::property::{ComputedProperty, Property, PropertyLike};

/// Reactive navigation state that manages all navigation-related properties
#[derive(Debug)]
pub struct NavigationState {
    // Primary navigation state
    pub current_page: Property<NavigationPage>,
    pub navigation_history: Property<Vec<NavigationPage>>,

    // Header state (computed from navigation)
    pub header_title: ComputedProperty<Option<String>>,
    pub show_back_button: ComputedProperty<bool>,
    pub back_button_tooltip: ComputedProperty<String>,

    // Page-specific header content (not reactive due to GTK widget constraints)
    // These will be managed directly by NavigationManager

    // Navigation capabilities (computed)
    pub can_go_back: ComputedProperty<bool>,
    pub can_go_forward: ComputedProperty<bool>,
}

impl NavigationState {
    pub fn new() -> Self {
        // Create base properties
        let current_page = Property::new(NavigationPage::Empty, "current_page");
        let navigation_history = Property::new(vec![NavigationPage::Empty], "navigation_history");

        // Create computed properties
        let current_page_arc: Arc<dyn PropertyLike> = Arc::new(current_page.clone());
        let header_title = ComputedProperty::new("header_title", vec![current_page_arc], {
            let current_page = current_page.clone();
            move || current_page.get_sync().display_title()
        });

        let navigation_history_arc: Arc<dyn PropertyLike> = Arc::new(navigation_history.clone());
        let show_back_button =
            ComputedProperty::new("show_back_button", vec![navigation_history_arc.clone()], {
                let navigation_history = navigation_history.clone();
                move || navigation_history.get_sync().len() > 1
            });

        let back_button_tooltip = ComputedProperty::new(
            "back_button_tooltip",
            vec![navigation_history_arc.clone()],
            {
                let navigation_history = navigation_history.clone();
                move || {
                    let history = navigation_history.get_sync();
                    if history.len() > 1 {
                        let previous = &history[history.len() - 2];
                        format!("Back to {}", previous.display_name())
                    } else {
                        "Back".to_string()
                    }
                }
            },
        );

        let can_go_back =
            ComputedProperty::new("can_go_back", vec![navigation_history_arc.clone()], {
                let navigation_history = navigation_history.clone();
                move || navigation_history.get_sync().len() > 1
            });

        // For now, forward navigation is not implemented, so always false
        let can_go_forward =
            ComputedProperty::new("can_go_forward", vec![navigation_history_arc.clone()], {
                move || false
            });

        Self {
            current_page,
            navigation_history,
            header_title,
            show_back_button,
            back_button_tooltip,
            can_go_back,
            can_go_forward,
        }
    }

    /// Get the current page synchronously
    pub fn current_page(&self) -> NavigationPage {
        self.current_page.get_sync()
    }

    /// Get the navigation history synchronously
    pub fn navigation_history(&self) -> Vec<NavigationPage> {
        self.navigation_history.get_sync()
    }

    /// Check if back navigation is possible
    pub fn can_navigate_back(&self) -> bool {
        self.can_go_back.get_sync()
    }

    /// Check if forward navigation is possible
    pub fn can_navigate_forward(&self) -> bool {
        self.can_go_forward.get_sync()
    }

    /// Get the current header title
    pub fn header_title(&self) -> Option<String> {
        self.header_title.get_sync()
    }

    /// Check if back button should be shown
    pub fn should_show_back_button(&self) -> bool {
        self.show_back_button.get_sync()
    }

    /// Get the back button tooltip text
    pub fn back_button_tooltip_text(&self) -> String {
        self.back_button_tooltip.get_sync()
    }
}

impl Default for NavigationState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, sleep};

    #[tokio::test]
    async fn test_navigation_state_initial_values() {
        let state = NavigationState::new();

        // Check initial values
        assert_eq!(state.current_page(), NavigationPage::Empty);
        assert_eq!(state.navigation_history(), vec![NavigationPage::Empty]);
        assert!(!state.can_navigate_back());
        assert!(!state.can_navigate_forward());
        assert_eq!(state.header_title(), None);
        assert!(!state.should_show_back_button());
        assert_eq!(state.back_button_tooltip_text(), "Back");
    }

    #[tokio::test]
    async fn test_navigation_state_single_navigation() {
        let state = NavigationState::new();

        // Navigate to sources page
        let sources_page = NavigationPage::Sources;
        let mut history = state.navigation_history();
        history.push(sources_page.clone());

        state.navigation_history.set(history).await;
        state.current_page.set(sources_page.clone()).await;

        // Small delay to allow computed properties to update
        sleep(Duration::from_millis(10)).await;

        // Check updated values
        assert_eq!(state.current_page(), sources_page);
        assert!(state.can_navigate_back());
        assert!(!state.can_navigate_forward());
        assert_eq!(state.header_title(), Some("Sources".to_string()));
        assert!(state.should_show_back_button());
        assert_eq!(state.back_button_tooltip_text(), "Back to Content");
    }

    #[tokio::test]
    async fn test_navigation_state_multiple_navigations() {
        let state = NavigationState::new();

        // Navigate to sources, then to a library
        let sources_page = NavigationPage::Sources;
        let library_page = NavigationPage::Library {
            backend_id: "plex".to_string(),
            library_id: "1".to_string(),
            title: "Movies".to_string(),
        };

        // First navigation
        let mut history = vec![NavigationPage::Empty, sources_page.clone()];
        state.navigation_history.set(history.clone()).await;
        state.current_page.set(sources_page.clone()).await;

        sleep(Duration::from_millis(10)).await;

        // Second navigation
        history.push(library_page.clone());
        state.navigation_history.set(history).await;
        state.current_page.set(library_page.clone()).await;

        sleep(Duration::from_millis(10)).await;

        // Check final values
        assert_eq!(state.current_page(), library_page);
        assert!(state.can_navigate_back());
        assert!(!state.can_navigate_forward());
        assert_eq!(state.header_title(), Some("Movies".to_string()));
        assert!(state.should_show_back_button());
        assert_eq!(state.back_button_tooltip_text(), "Back to Sources");
    }

    #[tokio::test]
    async fn test_navigation_state_back_navigation() {
        let state = NavigationState::new();

        // Set up history with multiple pages
        let sources_page = NavigationPage::Sources;
        let library_page = NavigationPage::Library {
            backend_id: "plex".to_string(),
            library_id: "1".to_string(),
            title: "Movies".to_string(),
        };

        let mut history = vec![
            NavigationPage::Empty,
            sources_page.clone(),
            library_page.clone(),
        ];
        state.navigation_history.set(history.clone()).await;
        state.current_page.set(library_page.clone()).await;

        sleep(Duration::from_millis(10)).await;

        // Simulate going back (remove last page from history)
        history.pop();
        let previous_page = history.last().cloned().unwrap();

        state.navigation_history.set(history).await;
        state.current_page.set(previous_page.clone()).await;

        sleep(Duration::from_millis(10)).await;

        // Check values after back navigation
        assert_eq!(state.current_page(), sources_page);
        assert!(state.can_navigate_back());
        assert!(!state.can_navigate_forward());
        assert_eq!(state.header_title(), Some("Sources".to_string()));
        assert!(state.should_show_back_button());
        assert_eq!(state.back_button_tooltip_text(), "Back to Content");
    }
}
