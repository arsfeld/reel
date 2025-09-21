use crate::db::entities::media_items::MediaType;
use crate::models::{LibraryId, MediaItemId, ShowId, SourceId};
use std::fmt;

/// Type-safe cache key system to replace string-based cache key construction
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CacheKey {
    /// Simple cache key for any media item by ID (used for memory cache)
    Media(String),

    /// Cache key for list of libraries for a source
    Libraries(SourceId),

    /// Cache key for list of items in a library
    LibraryItems(SourceId, LibraryId),

    /// Cache key for a specific media item
    MediaItem {
        source: SourceId,
        library: LibraryId,
        media_type: MediaType,
        item_id: MediaItemId,
    },

    /// Cache key for home sections of a source
    HomeSections(SourceId),

    /// Cache key for episodes of a show
    ShowEpisodes(SourceId, LibraryId, ShowId),

    /// Cache key for a specific episode
    Episode(SourceId, LibraryId, MediaItemId),

    /// Cache key for a show
    Show(SourceId, LibraryId, ShowId),

    /// Cache key for a movie
    Movie(SourceId, LibraryId, MediaItemId),
}

impl CacheKey {
    /// Convert the cache key to its string representation
    pub fn to_string(&self) -> String {
        match self {
            CacheKey::Media(id) => {
                format!("media:{}", id)
            }
            CacheKey::Libraries(source) => {
                format!("{}:libraries", source.as_str())
            }
            CacheKey::LibraryItems(source, library) => {
                format!("{}:library:{}:items", source.as_str(), library.as_str())
            }
            CacheKey::MediaItem {
                source,
                library,
                media_type,
                item_id,
            } => {
                let type_str = match media_type {
                    MediaType::Movie => "movie",
                    MediaType::Show => "show",
                    MediaType::Episode => "episode",
                    MediaType::Album => "album",
                    MediaType::Track => "track",
                    MediaType::Photo => "photo",
                };
                format!(
                    "{}:{}:{}:{}",
                    source.as_str(),
                    library.as_str(),
                    type_str,
                    item_id.as_str()
                )
            }
            CacheKey::HomeSections(source) => {
                format!("{}:home_sections", source.as_str())
            }
            CacheKey::ShowEpisodes(source, library, show) => {
                format!(
                    "{}:{}:show:{}:episodes",
                    source.as_str(),
                    library.as_str(),
                    show.as_str()
                )
            }
            CacheKey::Episode(source, library, episode) => {
                format!(
                    "{}:{}:episode:{}",
                    source.as_str(),
                    library.as_str(),
                    episode.as_str()
                )
            }
            CacheKey::Show(source, library, show) => {
                format!(
                    "{}:{}:show:{}",
                    source.as_str(),
                    library.as_str(),
                    show.as_str()
                )
            }
            CacheKey::Movie(source, library, movie) => {
                format!(
                    "{}:{}:movie:{}",
                    source.as_str(),
                    library.as_str(),
                    movie.as_str()
                )
            }
        }
    }

    /// Parse a cache key from its string representation
    pub fn parse(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split(':').collect();

        match parts.as_slice() {
            ["media", id] => Ok(CacheKey::Media(id.to_string())),
            [source, "libraries"] => Ok(CacheKey::Libraries(SourceId::from(*source))),
            [source, "library", library, "items"] => Ok(CacheKey::LibraryItems(
                SourceId::from(*source),
                LibraryId::from(*library),
            )),
            [source, "home_sections"] => Ok(CacheKey::HomeSections(SourceId::from(*source))),
            [source, library, "show", show, "episodes"] => Ok(CacheKey::ShowEpisodes(
                SourceId::from(*source),
                LibraryId::from(*library),
                ShowId::from(*show),
            )),
            [source, library, "episode", episode] => Ok(CacheKey::Episode(
                SourceId::from(*source),
                LibraryId::from(*library),
                MediaItemId::from(*episode),
            )),
            [source, library, "show", show] => Ok(CacheKey::Show(
                SourceId::from(*source),
                LibraryId::from(*library),
                ShowId::from(*show),
            )),
            [source, library, "movie", movie] => Ok(CacheKey::Movie(
                SourceId::from(*source),
                LibraryId::from(*library),
                MediaItemId::from(*movie),
            )),
            [source, library, media_type, item] => {
                let media_type = match *media_type {
                    "movie" => MediaType::Movie,
                    "show" => MediaType::Show,
                    "episode" => MediaType::Episode,
                    "album" => MediaType::Album,
                    "track" => MediaType::Track,
                    "photo" => MediaType::Photo,
                    _ => return Err(format!("Unknown media type: {}", media_type)),
                };
                Ok(CacheKey::MediaItem {
                    source: SourceId::from(*source),
                    library: LibraryId::from(*library),
                    media_type,
                    item_id: MediaItemId::from(*item),
                })
            }
            _ => Err(format!("Invalid cache key format: {}", s)),
        }
    }

    /// Extract the source ID from the cache key, if present
    pub fn source_id(&self) -> Option<&SourceId> {
        match self {
            CacheKey::Media(_) => None,
            CacheKey::Libraries(source) | CacheKey::HomeSections(source) => Some(source),
            CacheKey::LibraryItems(source, _)
            | CacheKey::ShowEpisodes(source, _, _)
            | CacheKey::Episode(source, _, _)
            | CacheKey::Show(source, _, _)
            | CacheKey::Movie(source, _, _) => Some(source),
            CacheKey::MediaItem { source, .. } => Some(source),
        }
    }

    /// Extract the library ID from the cache key, if present
    pub fn library_id(&self) -> Option<&LibraryId> {
        match self {
            CacheKey::Media(_) | CacheKey::Libraries(_) | CacheKey::HomeSections(_) => None,
            CacheKey::LibraryItems(_, library)
            | CacheKey::ShowEpisodes(_, library, _)
            | CacheKey::Episode(_, library, _)
            | CacheKey::Show(_, library, _)
            | CacheKey::Movie(_, library, _) => Some(library),
            CacheKey::MediaItem { library, .. } => Some(library),
        }
    }
}

impl fmt::Display for CacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_key() {
        let key = CacheKey::Media("item-123".to_string());
        assert_eq!(key.to_string(), "media:item-123");

        let parsed = CacheKey::parse("media:item-456").unwrap();
        assert_eq!(parsed, CacheKey::Media("item-456".to_string()));
        assert_eq!(parsed.source_id(), None);
        assert_eq!(parsed.library_id(), None);
    }

    #[test]
    fn test_libraries_key() {
        let source = SourceId::from("plex-server-1");
        let key = CacheKey::Libraries(source.clone());
        assert_eq!(key.to_string(), "plex-server-1:libraries");

        let parsed = CacheKey::parse("plex-server-1:libraries").unwrap();
        assert_eq!(parsed, key);
        assert_eq!(
            parsed.source_id().map(|s| s.as_str()),
            Some("plex-server-1")
        );
        assert_eq!(parsed.library_id(), None);
    }

    #[test]
    fn test_library_items_key() {
        let source = SourceId::from("jellyfin-1");
        let library = LibraryId::from("movies-lib");
        let key = CacheKey::LibraryItems(source.clone(), library.clone());
        assert_eq!(key.to_string(), "jellyfin-1:library:movies-lib:items");

        let parsed = CacheKey::parse("jellyfin-1:library:movies-lib:items").unwrap();
        assert_eq!(parsed, key);
        assert_eq!(parsed.source_id().map(|s| s.as_str()), Some("jellyfin-1"));
        assert_eq!(parsed.library_id().map(|l| l.as_str()), Some("movies-lib"));
    }

    #[test]
    fn test_media_item_key() {
        // Test with Album type which doesn't have a specific variant
        let key = CacheKey::MediaItem {
            source: SourceId::from("plex-1"),
            library: LibraryId::from("lib-1"),
            media_type: MediaType::Album,
            item_id: MediaItemId::from("album-123"),
        };
        assert_eq!(key.to_string(), "plex-1:lib-1:album:album-123");

        let parsed = CacheKey::parse("plex-1:lib-1:album:album-123").unwrap();
        assert_eq!(parsed, key);

        // Test that movie/episode/show types use their specific variants
        let movie_parsed = CacheKey::parse("plex-1:lib-1:movie:movie-123").unwrap();
        assert!(matches!(movie_parsed, CacheKey::Movie(_, _, _)));

        let episode_parsed = CacheKey::parse("plex-1:lib-1:episode:ep-123").unwrap();
        assert!(matches!(episode_parsed, CacheKey::Episode(_, _, _)));
    }

    #[test]
    fn test_home_sections_key() {
        let source = SourceId::from("jellyfin-home");
        let key = CacheKey::HomeSections(source.clone());
        assert_eq!(key.to_string(), "jellyfin-home:home_sections");

        let parsed = CacheKey::parse("jellyfin-home:home_sections").unwrap();
        assert_eq!(parsed, key);
    }

    #[test]
    fn test_show_episodes_key() {
        let key = CacheKey::ShowEpisodes(
            SourceId::from("plex"),
            LibraryId::from("tv-lib"),
            ShowId::from("show-456"),
        );
        assert_eq!(key.to_string(), "plex:tv-lib:show:show-456:episodes");

        let parsed = CacheKey::parse("plex:tv-lib:show:show-456:episodes").unwrap();
        assert_eq!(parsed, key);
    }

    #[test]
    fn test_round_trip_conversion() {
        let keys = vec![
            CacheKey::Media("test-media-id".to_string()),
            CacheKey::Libraries(SourceId::from("src1")),
            CacheKey::LibraryItems(SourceId::from("src2"), LibraryId::from("lib1")),
            CacheKey::MediaItem {
                source: SourceId::from("src3"),
                library: LibraryId::from("lib2"),
                media_type: MediaType::Album,
                item_id: MediaItemId::from("album1"),
            },
            CacheKey::HomeSections(SourceId::from("src4")),
            CacheKey::Show(
                SourceId::from("src5"),
                LibraryId::from("lib3"),
                ShowId::from("show1"),
            ),
            CacheKey::Episode(
                SourceId::from("src6"),
                LibraryId::from("lib4"),
                MediaItemId::from("ep1"),
            ),
            CacheKey::Movie(
                SourceId::from("src7"),
                LibraryId::from("lib5"),
                MediaItemId::from("mov1"),
            ),
        ];

        for key in keys {
            let string = key.to_string();
            let parsed = CacheKey::parse(&string).unwrap();
            assert_eq!(parsed, key);
        }
    }

    #[test]
    fn test_invalid_parse() {
        assert!(CacheKey::parse("invalid").is_err());
        assert!(CacheKey::parse("").is_err());
        assert!(CacheKey::parse("too:many:colons:here:and:more").is_err()); // Too many parts
    }
}
