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
    /// Stable per-(source, library) key used to store visibility and other
    /// per-library preferences. Format: `"{source_type}:{source_id}:{section_key}"`.
    /// Source-scoped so multiple sources never collide. Built from parts so
    /// callers can compute the key from a bare section key (e.g. a
    /// `MediaItem.library_section_id`) without a full `LibrarySection`.
    pub fn visibility_key_for(source_type: &str, source_id: &str, section_key: &str) -> String {
        format!("{source_type}:{source_id}:{section_key}")
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
    fn visibility_key_for_formats_source_and_section() {
        assert_eq!(
            LibrarySection::visibility_key_for("plex", "http://host:32400", "4"),
            "plex:http://host:32400:4"
        );
    }
}
