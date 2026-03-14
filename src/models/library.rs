/// A library section from a media source (e.g., "Movies", "TV Shows").
#[derive(Debug, Clone, PartialEq)]
pub struct LibrarySection {
    /// The section key/ID on the source
    pub key: String,
    /// Display name (e.g., "Movies", "4K Movies")
    pub title: String,
    /// Content type: "movie" or "show"
    pub library_type: LibraryType,
    /// Number of items in this section
    pub count: Option<i32>,
}

/// Type of content in a library section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryType {
    Movie,
    Show,
}

impl LibraryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Movie => "movie",
            Self::Show => "show",
        }
    }

    pub fn from_plex_type(s: &str) -> Option<Self> {
        match s {
            "movie" => Some(Self::Movie),
            "show" => Some(Self::Show),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_type_roundtrip() {
        assert_eq!(
            LibraryType::from_plex_type("movie"),
            Some(LibraryType::Movie)
        );
        assert_eq!(LibraryType::from_plex_type("show"), Some(LibraryType::Show));
    }

    #[test]
    fn library_type_unknown_returns_none() {
        assert_eq!(LibraryType::from_plex_type("artist"), None);
        assert_eq!(LibraryType::from_plex_type("photo"), None);
    }

    #[test]
    fn library_section_equality() {
        let a = LibrarySection {
            key: "1".to_string(),
            title: "Movies".to_string(),
            library_type: LibraryType::Movie,
            count: Some(500),
        };
        let b = a.clone();
        assert_eq!(a, b);
    }
}
