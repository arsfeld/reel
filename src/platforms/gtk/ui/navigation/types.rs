use std::fmt::{self, Display};

/// Window state that should be preserved during navigation
#[derive(Clone, Debug, PartialEq)]
pub struct WindowState {
    /// Window size to restore when returning from player
    pub saved_size: Option<(i32, i32)>,
    /// Whether the window was maximized
    pub was_maximized: bool,
    /// Whether the window was fullscreen
    pub was_fullscreen: bool,
}

impl WindowState {
    pub fn new() -> Self {
        Self {
            saved_size: None,
            was_maximized: false,
            was_fullscreen: false,
        }
    }
}

impl Default for WindowState {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents different pages in the application navigation
#[derive(Clone, Debug, PartialEq)]
pub enum NavigationPage {
    Home {
        source_id: Option<String>,
    },
    Sources,
    Library {
        backend_id: String,
        library_id: String,
        title: String,
    },
    MovieDetails {
        movie_id: String,
        title: String,
    },
    ShowDetails {
        show_id: String,
        title: String,
    },
    Player {
        media_id: String,
        title: String,
    },
    Empty,
}

impl NavigationPage {
    /// Get the display name for this page (used in back button tooltips)
    pub fn display_name(&self) -> String {
        match self {
            NavigationPage::Home { source_id } => match source_id {
                Some(id) => format!("Home ({})", id),
                None => "Home".to_string(),
            },
            NavigationPage::Sources => "Sources".to_string(),
            NavigationPage::Library { title, .. } => title.clone(),
            NavigationPage::MovieDetails { title, .. } => title.clone(),
            NavigationPage::ShowDetails { title, .. } => title.clone(),
            NavigationPage::Player { title, .. } => format!("Playing: {}", title),
            NavigationPage::Empty => "Content".to_string(),
        }
    }

    /// Get the title to display in the header (None means no title)
    pub fn display_title(&self) -> Option<String> {
        match self {
            NavigationPage::Empty => None,
            _ => Some(self.display_name()),
        }
    }

    /// Get the page name for the GTK Stack
    pub fn stack_page_name(&self) -> String {
        match self {
            NavigationPage::Home { .. } => "home".to_string(),
            NavigationPage::Sources => "sources".to_string(),
            NavigationPage::Library { .. } => "library".to_string(),
            NavigationPage::MovieDetails { .. } => "movie_details".to_string(),
            NavigationPage::ShowDetails { .. } => "show_details".to_string(),
            NavigationPage::Player { .. } => "player".to_string(),
            NavigationPage::Empty => "empty".to_string(),
        }
    }
}

impl Display for NavigationPage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_page_display_name() {
        let home_page = NavigationPage::Home { source_id: None };
        assert_eq!(home_page.display_name(), "Home");

        let home_with_source = NavigationPage::Home {
            source_id: Some("plex".to_string()),
        };
        assert_eq!(home_with_source.display_name(), "Home (plex)");

        let sources_page = NavigationPage::Sources;
        assert_eq!(sources_page.display_name(), "Sources");

        let library_page = NavigationPage::Library {
            backend_id: "plex".to_string(),
            library_id: "1".to_string(),
            title: "Movies".to_string(),
        };
        assert_eq!(library_page.display_name(), "Movies");

        let movie_page = NavigationPage::MovieDetails {
            movie_id: "123".to_string(),
            title: "Test Movie".to_string(),
        };
        assert_eq!(movie_page.display_name(), "Test Movie");

        let player_page = NavigationPage::Player {
            media_id: "456".to_string(),
            title: "Test Movie".to_string(),
        };
        assert_eq!(player_page.display_name(), "Playing: Test Movie");

        let empty_page = NavigationPage::Empty;
        assert_eq!(empty_page.display_name(), "Content");
    }

    #[test]
    fn test_navigation_page_display_title() {
        let empty_page = NavigationPage::Empty;
        assert_eq!(empty_page.display_title(), None);

        let home_page = NavigationPage::Home { source_id: None };
        assert_eq!(home_page.display_title(), Some("Home".to_string()));

        let sources_page = NavigationPage::Sources;
        assert_eq!(sources_page.display_title(), Some("Sources".to_string()));
    }

    #[test]
    fn test_navigation_page_stack_name() {
        let home_page = NavigationPage::Home { source_id: None };
        assert_eq!(home_page.stack_page_name(), "home");

        let sources_page = NavigationPage::Sources;
        assert_eq!(sources_page.stack_page_name(), "sources");

        let library_page = NavigationPage::Library {
            backend_id: "plex".to_string(),
            library_id: "1".to_string(),
            title: "Movies".to_string(),
        };
        assert_eq!(library_page.stack_page_name(), "library");

        let movie_page = NavigationPage::MovieDetails {
            movie_id: "123".to_string(),
            title: "Test Movie".to_string(),
        };
        assert_eq!(movie_page.stack_page_name(), "movie_details");

        let player_page = NavigationPage::Player {
            media_id: "456".to_string(),
            title: "Test Movie".to_string(),
        };
        assert_eq!(player_page.stack_page_name(), "player");

        let empty_page = NavigationPage::Empty;
        assert_eq!(empty_page.stack_page_name(), "empty");
    }

    #[test]
    fn test_navigation_page_equality() {
        let page1 = NavigationPage::Home { source_id: None };
        let page2 = NavigationPage::Home { source_id: None };
        let page3 = NavigationPage::Home {
            source_id: Some("plex".to_string()),
        };

        assert_eq!(page1, page2);
        assert_ne!(page1, page3);

        let movie1 = NavigationPage::MovieDetails {
            movie_id: "123".to_string(),
            title: "Test Movie".to_string(),
        };
        let movie2 = NavigationPage::MovieDetails {
            movie_id: "123".to_string(),
            title: "Test Movie".to_string(),
        };
        assert_eq!(movie1, movie2);
    }
}
