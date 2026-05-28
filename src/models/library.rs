use crate::models::media::SourceType;

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

impl LibrarySection {
    /// Stable identifier for this section within a given source, used as the
    /// key for persisted per-library filter/sort state. Shape mirrors
    /// `MediaItem::make_id`: `"{source_type}:{source_id}:{section_key}"`.
    #[allow(dead_code)]
    pub fn library_id(&self, source_type: SourceType, source_id: &str) -> String {
        format!("{}:{}:{}", source_type.as_str(), source_id, self.key)
    }
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

    #[test]
    fn library_id_composite_shape() {
        let section = LibrarySection {
            key: "1".to_string(),
            title: "Movies".to_string(),
            library_type: LibraryType::Movie,
            count: Some(500),
        };
        assert_eq!(
            section.library_id(SourceType::Plex, "http://localhost:32400"),
            "plex:http://localhost:32400:1"
        );
    }

    #[test]
    fn library_id_distinct_for_different_sections() {
        let movies = LibrarySection {
            key: "1".to_string(),
            title: "Movies".to_string(),
            library_type: LibraryType::Movie,
            count: None,
        };
        let shows = LibrarySection {
            key: "2".to_string(),
            title: "TV".to_string(),
            library_type: LibraryType::Show,
            count: None,
        };
        let src = "http://localhost:32400";
        assert_ne!(
            movies.library_id(SourceType::Plex, src),
            shows.library_id(SourceType::Plex, src)
        );
    }
}
