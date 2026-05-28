use crate::models::{
    detail::{CastMember, CollectionRef, MediaDetail, TechnicalInfo},
    library::{LibrarySection, LibraryType},
    media::{HdrFormat, MediaItem, MediaType, SourceType},
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
        "collection" => MediaType::Collection,
        _ => return None,
    };

    let id = MediaItem::make_id(SourceType::Plex, source_id, &metadata.rating_key);

    // Extract file path from first Part of first Media
    let file_path = metadata
        .media
        .first()
        .and_then(|m| m.parts.first())
        .map(|p| p.key.clone());

    // Extract video resolution from first Media
    let video_resolution = metadata
        .media
        .first()
        .and_then(|m| m.video_resolution.clone());

    // Extract HDR / Dolby Vision from first Media's videoDynamicRange field.
    let hdr = metadata
        .media
        .first()
        .and_then(|m| m.video_dynamic_range.as_deref())
        .and_then(HdrFormat::from_plex);

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
        video_resolution,
        hdr,
        added_at,
        updated_at,
        playback_position_ms: metadata.view_offset,
        library_section_id: metadata.library_section_id.clone(),
    })
}

/// Convert PlexMetadata to a MediaDetail (MediaItem + enrichment data).
pub fn plex_metadata_to_media_detail(
    metadata: &PlexMetadata,
    source_id: &str,
) -> Option<MediaDetail> {
    let item = plex_metadata_to_media_item(metadata, source_id)?;

    let cast: Vec<CastMember> = metadata
        .roles
        .iter()
        .map(|r| CastMember {
            name: r.tag.clone(),
            character: r.role.clone(),
            photo_path: r.thumb.clone(),
        })
        .collect();

    let directors: Vec<String> = metadata.directors.iter().map(|d| d.tag.clone()).collect();
    let writers: Vec<String> = metadata.writers.iter().map(|w| w.tag.clone()).collect();
    let collections: Vec<CollectionRef> = metadata
        .collections
        .iter()
        .map(|c| CollectionRef {
            name: c.tag.clone(),
        })
        .collect();

    let technical = metadata.media.first().map(|m| {
        let file_size = m.parts.first().and_then(|p| p.size);
        TechnicalInfo {
            video_resolution: m.video_resolution.clone(),
            video_codec: m.video_codec.clone(),
            audio_codec: m.audio_codec.clone(),
            audio_channels: m.audio_channels,
            container: m.container.clone(),
            bitrate_kbps: m.bitrate,
            file_size_bytes: file_size,
        }
    });

    Some(MediaDetail {
        item,
        cast,
        directors,
        writers,
        technical,
        collections,
    })
}

/// Recognised chapter tag substrings that indicate an intro sequence.
const INTRO_TAGS: &[&str] = &["intro", "opening", "opening credits", "title sequence"];

/// Recognised chapter tag substrings that indicate the credits sequence.
const CREDITS_TAGS: &[&str] = &["credits", "end credits", "closing credits"];

/// Convert Plex chapter markers to `SkipMarkers`.
///
/// Strategy (in priority order):
/// 1. Look for a chapter whose `tag` matches an intro tag — use its
///    start/end as the intro range.
/// 2. Look for a chapter whose `tag` matches a credits tag — use its
///    start as the credits start.
/// 3. **Intro heuristic**: use the *second* chapter if it starts within
///    the first 5 minutes and is ≤ 3 minutes long (common for TV intros).
/// 4. **Credits heuristic**: use the *last* chapter's start time, but
///    only if it begins in the final 25% of the media (avoids false
///    positives on short films with few chapters).
pub fn plex_chapters_to_skip_markers(
    chapters: &[super::models::PlexChapter],
    duration_secs: f64,
) -> Option<crate::player::SkipMarkers> {
    if chapters.is_empty() || duration_secs <= 0.0 {
        return None;
    }

    let tag_matches = |tag: &Option<String>, candidates: &[&str]| -> bool {
        tag.as_ref()
            .is_some_and(|t| candidates.iter().any(|c| t.to_lowercase().contains(c)))
    };

    // --- Intro detection ---
    let intro_chapter = chapters.iter().find(|ch| tag_matches(&ch.tag, INTRO_TAGS));
    let intro_start_secs;
    let intro_end_secs;

    if let Some(ch) = intro_chapter {
        intro_start_secs = (ch.start_time_offset as f64) / 1000.0;
        intro_end_secs = (ch.end_time_offset as f64) / 1000.0;
    } else if chapters.len() >= 2 {
        // Heuristic: second chapter as intro candidate.
        let second = &chapters[1];
        let start_s = (second.start_time_offset as f64) / 1000.0;
        let len_s = ((second.end_time_offset - second.start_time_offset) as f64) / 1000.0;
        if start_s <= 300.0 && len_s > 0.0 && len_s <= 180.0 {
            intro_start_secs = start_s;
            intro_end_secs = start_s + len_s;
        } else {
            // No plausible intro found.
            intro_start_secs = 0.0;
            intro_end_secs = 0.0;
        }
    } else {
        intro_start_secs = 0.0;
        intro_end_secs = 0.0;
    }

    // --- Credits detection ---
    let credits_chapter = chapters
        .iter()
        .rev()
        .find(|ch| tag_matches(&ch.tag, CREDITS_TAGS));
    let credits_start_secs;

    if let Some(ch) = credits_chapter {
        credits_start_secs = (ch.start_time_offset as f64) / 1000.0;
    } else if let Some(last) = chapters.last() {
        // Heuristic: last chapter, but only if it's in the final 25%.
        let start_s = (last.start_time_offset as f64) / 1000.0;
        if duration_secs > 0.0 && start_s / duration_secs >= 0.75 {
            credits_start_secs = start_s;
        } else {
            credits_start_secs = 0.0;
        }
    } else {
        credits_start_secs = 0.0;
    }

    // If neither range was detected, return None so the skip button stays
    // hidden entirely.
    if intro_end_secs <= 0.0 && credits_start_secs <= 0.0 {
        return None;
    }

    // Sanity-check: clamp to duration bounds.
    let clamp = |v: f64| v.clamp(0.0, duration_secs);

    Some(crate::player::SkipMarkers {
        intro_start_secs: clamp(intro_start_secs),
        intro_end_secs: clamp(intro_end_secs),
        credits_start_secs: clamp(credits_start_secs),
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
            parent_thumb: None,
            library_section_id: None,
            view_offset: None,
            view_count: None,
            last_viewed_at: None,
            genres: vec![
                PlexTag {
                    tag: "Science Fiction".to_string(),
                },
                PlexTag {
                    tag: "Adventure".to_string(),
                },
            ],
            roles: vec![],
            directors: vec![],
            writers: vec![],
            collections: vec![],
            media: vec![PlexMedia {
                video_resolution: None,
                video_codec: None,
                audio_codec: None,
                audio_channels: None,
                bitrate: None,
                container: None,
                width: None,
                height: None,
                video_dynamic_range: None,
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
            parent_thumb: None,
            library_section_id: None,
            view_offset: None,
            view_count: None,
            last_viewed_at: None,
            genres: vec![],
            roles: vec![],
            directors: vec![],
            writers: vec![],
            collections: vec![],
            media: vec![PlexMedia {
                video_resolution: None,
                video_codec: None,
                audio_codec: None,
                audio_channels: None,
                bitrate: None,
                container: None,
                width: None,
                height: None,
                video_dynamic_range: None,
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
    fn convert_carries_library_section_id_from_payload() {
        let mut plex = test_plex_movie();
        plex.library_section_id = Some("3".to_string());
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(item.library_section_id, Some("3".to_string()));
    }

    #[test]
    fn convert_missing_library_section_id_gives_none() {
        let plex = test_plex_movie(); // builder leaves library_section_id None
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(item.library_section_id, None);
    }

    #[test]
    fn convert_movie_extracts_video_resolution() {
        let mut plex = test_plex_movie();
        plex.media = vec![PlexMedia {
            video_resolution: Some("1080".to_string()),
            video_codec: None,
            audio_codec: None,
            audio_channels: None,
            bitrate: None,
            container: None,
            width: None,
            height: None,
            video_dynamic_range: None,
            parts: vec![PlexPart {
                key: "/library/parts/456/file.mkv".to_string(),
                file: None,
                size: None,
            }],
        }];
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(item.video_resolution, Some("1080".to_string()));
    }

    #[test]
    fn convert_no_media_gives_none_video_resolution() {
        let mut plex = test_plex_movie();
        plex.media.clear();
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(item.video_resolution, None);
    }

    #[test]
    fn convert_movie_extracts_hdr() {
        let mut plex = test_plex_movie();
        plex.media = vec![PlexMedia {
            video_resolution: Some("4k".to_string()),
            video_codec: None,
            audio_codec: None,
            audio_channels: None,
            bitrate: None,
            container: None,
            width: None,
            height: None,
            video_dynamic_range: Some("HDR".to_string()),
            parts: vec![],
        }];
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(item.hdr, Some(HdrFormat::Hdr));
    }

    #[test]
    fn convert_movie_extracts_dolby_vision() {
        let mut plex = test_plex_movie();
        plex.media = vec![PlexMedia {
            video_resolution: Some("4k".to_string()),
            video_codec: None,
            audio_codec: None,
            audio_channels: None,
            bitrate: None,
            container: None,
            width: None,
            height: None,
            video_dynamic_range: Some("Dolby Vision".to_string()),
            parts: vec![],
        }];
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(item.hdr, Some(HdrFormat::DolbyVision));
    }

    #[test]
    fn convert_movie_sdr_gives_none_hdr() {
        let mut plex = test_plex_movie();
        plex.media = vec![PlexMedia {
            video_resolution: Some("1080".to_string()),
            video_codec: None,
            audio_codec: None,
            audio_channels: None,
            bitrate: None,
            container: None,
            width: None,
            height: None,
            video_dynamic_range: Some("SDR".to_string()),
            parts: vec![],
        }];
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(item.hdr, None);
    }

    #[test]
    fn convert_movie_no_dynamic_range_field_gives_none_hdr() {
        let plex = test_plex_movie(); // builder sets video_dynamic_range to None
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(item.hdr, None);
    }

    #[test]
    fn convert_no_media_gives_none_hdr() {
        let mut plex = test_plex_movie();
        plex.media.clear();
        let item = plex_metadata_to_media_item(&plex, "http://localhost:32400").unwrap();
        assert_eq!(item.hdr, None);
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

    // --- MediaDetail conversion tests ---

    #[test]
    fn convert_detail_extracts_cast() {
        let mut plex = test_plex_movie();
        plex.roles = vec![
            PlexRole {
                tag: "Actor A".to_string(),
                role: Some("Character A".to_string()),
                thumb: Some("/photo/a".to_string()),
            },
            PlexRole {
                tag: "Actor B".to_string(),
                role: None,
                thumb: None,
            },
        ];

        let detail = plex_metadata_to_media_detail(&plex, "http://localhost:32400").unwrap();
        assert_eq!(detail.cast.len(), 2);
        assert_eq!(detail.cast[0].name, "Actor A");
        assert_eq!(detail.cast[0].character, Some("Character A".to_string()));
        assert_eq!(detail.cast[0].photo_path, Some("/photo/a".to_string()));
        assert_eq!(detail.cast[1].name, "Actor B");
        assert_eq!(detail.cast[1].character, None);
    }

    #[test]
    fn convert_detail_extracts_directors_and_writers() {
        let mut plex = test_plex_movie();
        plex.directors = vec![PlexTag {
            tag: "Denis Villeneuve".to_string(),
        }];
        plex.writers = vec![
            PlexTag {
                tag: "Jon Spaihts".to_string(),
            },
            PlexTag {
                tag: "Denis Villeneuve".to_string(),
            },
        ];

        let detail = plex_metadata_to_media_detail(&plex, "http://localhost:32400").unwrap();
        assert_eq!(detail.directors, vec!["Denis Villeneuve"]);
        assert_eq!(detail.writers, vec!["Jon Spaihts", "Denis Villeneuve"]);
    }

    #[test]
    fn convert_detail_extracts_collections() {
        let mut plex = test_plex_movie();
        plex.collections = vec![PlexTag {
            tag: "Dune Collection".to_string(),
        }];

        let detail = plex_metadata_to_media_detail(&plex, "http://localhost:32400").unwrap();
        assert_eq!(detail.collections.len(), 1);
        assert_eq!(detail.collections[0].name, "Dune Collection");
    }

    #[test]
    fn convert_detail_extracts_technical_info() {
        let mut plex = test_plex_movie();
        plex.media = vec![PlexMedia {
            video_resolution: Some("1080".to_string()),
            video_codec: Some("h264".to_string()),
            audio_codec: Some("aac".to_string()),
            audio_channels: Some(6),
            bitrate: Some(12000),
            container: Some("mkv".to_string()),
            width: Some(1920),
            height: Some(1080),
            video_dynamic_range: None,
            parts: vec![PlexPart {
                key: "/library/parts/456/file.mkv".to_string(),
                file: Some("/data/Dune.mkv".to_string()),
                size: Some(15_000_000_000),
            }],
        }];

        let detail = plex_metadata_to_media_detail(&plex, "http://localhost:32400").unwrap();
        let tech = detail.technical.unwrap();
        assert_eq!(tech.video_resolution, Some("1080".to_string()));
        assert_eq!(tech.video_codec, Some("h264".to_string()));
        assert_eq!(tech.audio_codec, Some("aac".to_string()));
        assert_eq!(tech.audio_channels, Some(6));
        assert_eq!(tech.container, Some("mkv".to_string()));
        assert_eq!(tech.bitrate_kbps, Some(12000));
        assert_eq!(tech.file_size_bytes, Some(15_000_000_000));
    }

    #[test]
    fn convert_detail_no_media_gives_none_technical() {
        let mut plex = test_plex_movie();
        plex.media.clear();

        let detail = plex_metadata_to_media_detail(&plex, "http://localhost:32400").unwrap();
        assert!(detail.technical.is_none());
    }

    #[test]
    fn convert_detail_empty_enrichment_fields() {
        let plex = test_plex_movie(); // roles, directors, writers, collections all empty
        let detail = plex_metadata_to_media_detail(&plex, "http://localhost:32400").unwrap();
        assert!(detail.cast.is_empty());
        assert!(detail.directors.is_empty());
        assert!(detail.writers.is_empty());
        assert!(detail.collections.is_empty());
    }

    #[test]
    fn convert_detail_preserves_base_item() {
        let plex = test_plex_movie();
        let detail = plex_metadata_to_media_detail(&plex, "http://localhost:32400").unwrap();
        assert_eq!(detail.item.title, "Dune");
        assert_eq!(detail.item.year, Some(2021));
        assert_eq!(detail.item.media_type, MediaType::Movie);
    }
}
