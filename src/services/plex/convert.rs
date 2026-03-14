use crate::models::{
    library::{LibrarySection, LibraryType},
    media::{MediaItem, MediaType, SourceType},
};

use super::models::{PlexLibrary, PlexMetadata};

/// Convert a PlexLibrary to a LibrarySection.
pub fn plex_library_to_section(lib: &PlexLibrary) -> Option<LibrarySection> {
    let library_type = LibraryType::from_plex_type(&lib.library_type)?;
    Some(LibrarySection {
        key: lib.key.clone(),
        title: lib.title.clone(),
        library_type,
        count: lib.item_count,
    })
}

/// Convert PlexMetadata to a MediaItem.
pub fn plex_metadata_to_media_item(metadata: &PlexMetadata, source_id: &str) -> Option<MediaItem> {
    let media_type = match metadata.metadata_type.as_str() {
        "movie" => MediaType::Movie,
        "show" => MediaType::Show,
        "season" => MediaType::Season,
        "episode" => MediaType::Episode,
        _ => return None,
    };

    let id = MediaItem::make_id(SourceType::Plex, source_id, &metadata.rating_key);

    // Extract file path from first Part of first Media
    let file_path = metadata
        .media
        .first()
        .and_then(|m| m.parts.first())
        .map(|p| p.key.clone());

    // Convert duration from milliseconds to minutes
    let runtime_minutes = metadata.duration.map(|ms| (ms / 60_000) as i32);

    // Extract genres from PlexTag list
    let genres: Vec<String> = metadata.genres.iter().map(|g| g.tag.clone()).collect();

    // Build parent_id for TV hierarchy
    let parent_id = match media_type {
        MediaType::Season => metadata
            .parent_rating_key
            .as_ref()
            .map(|pk| MediaItem::make_id(SourceType::Plex, source_id, pk)),
        MediaType::Episode => metadata
            .parent_rating_key
            .as_ref()
            .map(|pk| MediaItem::make_id(SourceType::Plex, source_id, pk)),
        _ => None,
    };

    // Convert unix timestamps
    let added_at = metadata
        .added_at
        .map(|ts| ts.to_string())
        .unwrap_or_default();
    let updated_at = metadata
        .updated_at
        .map(|ts| ts.to_string())
        .unwrap_or_else(|| added_at.clone());

    Some(MediaItem {
        id,
        source_type: SourceType::Plex,
        source_id: source_id.to_string(),
        external_id: metadata.rating_key.clone(),
        media_type,
        title: metadata.title.clone(),
        year: metadata.year,
        overview: metadata.summary.clone(),
        content_rating: metadata.content_rating.clone(),
        rating: metadata.rating,
        runtime_minutes,
        poster_path: metadata.thumb.clone(),
        backdrop_path: metadata.art.clone(),
        genres,
        parent_id,
        season_number: metadata.parent_index.or(metadata.index),
        episode_number: if media_type == MediaType::Episode {
            metadata.index
        } else {
            None
        },
        air_date: metadata.originally_available_at.clone(),
        file_path,
        added_at,
        updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::plex::models::*;

    fn test_plex_movie() -> PlexMetadata {
        PlexMetadata {
            rating_key: "123".to_string(),
            title: "Dune".to_string(),
            metadata_type: "movie".to_string(),
            year: Some(2021),
            summary: Some("A noble family.".to_string()),
            content_rating: Some("PG-13".to_string()),
            rating: Some(8.0),
            duration: Some(9_300_000), // 155 minutes in ms
            thumb: Some("/library/metadata/123/thumb/1234".to_string()),
            art: Some("/library/metadata/123/art/1234".to_string()),
            added_at: Some(1705276800),
            updated_at: Some(1705276800),
            parent_rating_key: None,
            grandparent_rating_key: None,
            parent_index: None,
            index: None,
            originally_available_at: None,
            genres: vec![
                PlexTag {
                    tag: "Science Fiction".to_string(),
                },
                PlexTag {
                    tag: "Adventure".to_string(),
                },
            ],
            media: vec![PlexMedia {
                parts: vec![PlexPart {
                    key: "/library/parts/456/file.mkv".to_string(),
                    file: Some("/data/movies/Dune.mkv".to_string()),
                    size: Some(15_000_000_000),
                }],
            }],
        }
    }

    fn test_plex_episode() -> PlexMetadata {
        PlexMetadata {
            rating_key: "500".to_string(),
            title: "Pilot".to_string(),
            metadata_type: "episode".to_string(),
            year: None,
            summary: Some("The first episode.".to_string()),
            content_rating: None,
            rating: Some(9.0),
            duration: Some(2_520_000), // 42 minutes
            thumb: Some("/library/metadata/500/thumb/1".to_string()),
            art: None,
            added_at: Some(1705276800),
            updated_at: Some(1705276800),
            parent_rating_key: Some("400".to_string()),
            grandparent_rating_key: Some("300".to_string()),
            parent_index: Some(1),
            index: Some(1),
            originally_available_at: Some("2024-03-01".to_string()),
            genres: vec![],
            media: vec![PlexMedia {
                parts: vec![PlexPart {
                    key: "/library/parts/501/file.mkv".to_string(),
                    file: Some("/data/tv/Show/S01E01.mkv".to_string()),
                    size: Some(2_000_000_000),
                }],
            }],
        }
    }

    #[test]
    fn convert_movie_maps_all_fields() {
        let plex = test_plex_movie();
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();

        assert_eq!(item.media_type, MediaType::Movie);
        assert_eq!(item.title, "Dune");
        assert_eq!(item.year, Some(2021));
        assert_eq!(item.overview, Some("A noble family.".to_string()));
        assert_eq!(item.content_rating, Some("PG-13".to_string()));
        assert_eq!(item.rating, Some(8.0));
        assert_eq!(item.external_id, "123");
        assert_eq!(item.source_type, SourceType::Plex);
        assert_eq!(item.source_id, "http://localhost:32400");
    }

    #[test]
    fn convert_movie_duration_ms_to_minutes() {
        let plex = test_plex_movie();
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(item.runtime_minutes, Some(155));
    }

    #[test]
    fn convert_movie_extracts_genres() {
        let plex = test_plex_movie();
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(item.genres, vec!["Science Fiction", "Adventure"]);
    }

    #[test]
    fn convert_movie_extracts_file_path() {
        let plex = test_plex_movie();
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(
            item.file_path,
            Some("/library/parts/456/file.mkv".to_string())
        );
    }

    #[test]
    fn convert_movie_poster_and_backdrop() {
        let plex = test_plex_movie();
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(
            item.poster_path,
            Some("/library/metadata/123/thumb/1234".to_string())
        );
        assert_eq!(
            item.backdrop_path,
            Some("/library/metadata/123/art/1234".to_string())
        );
    }

    #[test]
    fn convert_movie_composite_id() {
        let plex = test_plex_movie();
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(item.id, "plex:http://localhost:32400:123");
    }

    #[test]
    fn convert_episode_season_and_episode_numbers() {
        let plex = test_plex_episode();
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(item.season_number, Some(1));
        assert_eq!(item.episode_number, Some(1));
    }

    #[test]
    fn convert_episode_parent_id() {
        let plex = test_plex_episode();
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(
            item.parent_id,
            Some("plex:http://localhost:32400:400".to_string())
        );
    }

    #[test]
    fn convert_episode_air_date() {
        let plex = test_plex_episode();
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(item.air_date, Some("2024-03-01".to_string()));
    }

    #[test]
    fn convert_movie_no_parent() {
        let plex = test_plex_movie();
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(item.parent_id, None);
    }

    #[test]
    fn convert_no_media_parts_gives_none_file_path() {
        let mut plex = test_plex_movie();
        plex.media.clear();
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(item.file_path, None);
    }

    #[test]
    fn convert_no_duration_gives_none_runtime() {
        let mut plex = test_plex_movie();
        plex.duration = None;
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(item.runtime_minutes, None);
    }

    #[test]
    fn convert_unknown_type_returns_none() {
        let mut plex = test_plex_movie();
        plex.metadata_type = "artist".to_string();
        assert!(plex_metadata_to_media_item(&plex, "http://localhost:32400").is_none());
    }

    #[test]
    fn convert_library_movie_type() {
        let lib = PlexLibrary {
            key: "1".to_string(),
            title: "Movies".to_string(),
            library_type: "movie".to_string(),
            item_count: Some(500),
        };
        let section = plex_library_to_section(&lib).unwrap();
        assert_eq!(section.key, "1");
        assert_eq!(section.title, "Movies");
        assert_eq!(section.library_type, LibraryType::Movie);
        assert_eq!(section.count, Some(500));
    }

    #[test]
    fn convert_library_show_type() {
        let lib = PlexLibrary {
            key: "2".to_string(),
            title: "TV Shows".to_string(),
            library_type: "show".to_string(),
            item_count: Some(100),
        };
        let section = plex_library_to_section(&lib).unwrap();
        assert_eq!(section.library_type, LibraryType::Show);
    }

    #[test]
    fn convert_library_unknown_type_returns_none() {
        let lib = PlexLibrary {
            key: "3".to_string(),
            title: "Music".to_string(),
            library_type: "artist".to_string(),
            item_count: None,
        };
        assert!(plex_library_to_section(&lib).is_none());
    }
}
