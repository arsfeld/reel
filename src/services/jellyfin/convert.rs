use crate::models::{
    detail::{CastMember, MediaDetail, TechnicalInfo},
    library::{LibrarySection, LibraryType},
    media::{MediaItem, MediaType, SourceType},
};
use crate::player::SkipMarkers;

use super::models::{BaseItemDto, MediaSegmentDto, Ticks};

/// Map a Jellyfin item height to the coarse resolution label the rest of the
/// app uses (mirrors the raw strings Plex reports, e.g. "1080", "4k"). Returns
/// `None` when the height is unknown or non-positive.
fn resolution_label(height: Option<i32>) -> Option<String> {
    match height {
        Some(h) if h >= 2000 => Some("4k".to_string()),
        Some(h) if h >= 1080 => Some("1080".to_string()),
        Some(h) if h >= 720 => Some("720".to_string()),
        Some(h) if h > 0 => Some("sd".to_string()),
        _ => None,
    }
}

/// Artwork descriptor for the item's own primary (poster) image, if any.
/// Format: `"{item_id}/Primary/{tag}"`.
fn primary_poster(dto: &BaseItemDto) -> Option<String> {
    dto.image_tags
        .as_ref()
        .and_then(|tags| tags.get("Primary"))
        .map(|tag| format!("{}/Primary/{}", dto.id, tag))
}

/// Backdrop descriptor: the item's own first backdrop, falling back (for
/// episodes) to the parent series/season backdrop.
fn backdrop_descriptor(dto: &BaseItemDto) -> Option<String> {
    dto.backdrop_image_tags
        .first()
        .map(|tag| format!("{}/Backdrop/{}", dto.id, tag))
        .or_else(|| {
            match (
                dto.parent_backdrop_item_id.as_ref(),
                dto.parent_backdrop_image_tags.first(),
            ) {
                (Some(parent_id), Some(tag)) => Some(format!("{parent_id}/Backdrop/{tag}")),
                _ => None,
            }
        })
}

/// Convert a Jellyfin `BaseItemDto` to a `MediaItem`.
///
/// Returns `None` for item types we don't model (music, etc.), mirroring the
/// Plex converter dropping non-movie/show metadata.
pub fn base_item_to_media_item(dto: &BaseItemDto, source_id: &str) -> Option<MediaItem> {
    let media_type = match dto.type_.as_deref() {
        Some("Movie") => MediaType::Movie,
        Some("Series") => MediaType::Show,
        Some("Season") => MediaType::Season,
        Some("Episode") => MediaType::Episode,
        Some("BoxSet") => MediaType::Collection,
        _ => return None,
    };

    let id = MediaItem::make_id(SourceType::Jellyfin, source_id, &dto.id);

    let runtime_minutes = dto
        .run_time_ticks
        .map(|t| (Ticks(t).to_ms() / 60_000) as i32);

    // Artwork descriptors: opaque "{item_id}/{image_type}/{tag}" strings the
    // source's artwork_url builder (U5) turns into a real URL.
    let poster_path = primary_poster(dto);
    let backdrop_path = backdrop_descriptor(dto);

    // For episode shelf cards: the parent series poster.
    let series_poster_path = match media_type {
        MediaType::Episode => {
            match (
                dto.series_id.as_ref(),
                dto.series_primary_image_tag.as_ref(),
            ) {
                (Some(series_id), Some(tag)) => Some(format!("{series_id}/Primary/{tag}")),
                _ => None,
            }
        }
        _ => None,
    };

    // TV hierarchy numbers.
    let season_number = match media_type {
        MediaType::Episode => dto.parent_index_number,
        MediaType::Season => dto.index_number,
        _ => None,
    };
    let episode_number = match media_type {
        MediaType::Episode => dto.index_number,
        _ => None,
    };

    // Parent id within the source.
    let parent_id = match media_type {
        MediaType::Episode => dto
            .season_id
            .as_ref()
            .map(|sid| MediaItem::make_id(SourceType::Jellyfin, source_id, sid)),
        MediaType::Season => dto
            .series_id
            .as_ref()
            .map(|sid| MediaItem::make_id(SourceType::Jellyfin, source_id, sid)),
        _ => None,
    };

    // Direct-play needs BOTH the item id and the media-source id; the stream
    // URL builder (`JellyfinClient::stream_url`) takes both. Store a composite
    // `"{item_id}|{media_source_id}"` so `JellyfinSource::playback_url` can split
    // it back apart. When the item has no media source but is itself playable
    // (Movie/Episode), fall back to the bare item id (used as both).
    let file_path = match dto.media_sources.first().and_then(|ms| ms.id.clone()) {
        Some(media_source_id) => Some(format!("{}|{}", dto.id, media_source_id)),
        None if matches!(media_type, MediaType::Movie | MediaType::Episode) => Some(dto.id.clone()),
        None => None,
    };

    let video_resolution = resolution_label(dto.height);

    let playback_position_ms = dto
        .user_data
        .as_ref()
        .and_then(|u| u.playback_position_ticks)
        .map(|t| Ticks(t).to_ms());

    let watched = dto.user_data.as_ref().map(|u| u.played).unwrap_or(false);

    Some(MediaItem {
        id,
        source_type: SourceType::Jellyfin,
        source_id: source_id.to_string(),
        external_id: dto.id.clone(),
        media_type,
        title: dto.name.clone().unwrap_or_default(),
        year: dto.production_year,
        overview: dto.overview.clone(),
        content_rating: dto.official_rating.clone(),
        rating: dto.community_rating,
        runtime_minutes,
        poster_path,
        series_poster_path,
        backdrop_path,
        genres: dto.genres.clone(),
        parent_id,
        season_number,
        episode_number,
        air_date: dto.premiere_date.clone(),
        file_path,
        video_resolution,
        // Jellyfin HDR detection needs MediaStreams; out of scope for the base item.
        hdr: None,
        // Jellyfin doesn't give a reliable unix timestamp on the base item;
        // degrade gracefully with empty strings (these only feed sort/display).
        added_at: String::new(),
        updated_at: String::new(),
        playback_position_ms,
        watched,
        // The owning library/collection id, used for visibility filtering.
        library_section_id: dto.parent_id.clone(),
    })
}

/// Convert a Jellyfin `BaseItemDto` to a `MediaDetail` (base item + enrichment).
pub fn base_item_to_media_detail(dto: &BaseItemDto, source_id: &str) -> Option<MediaDetail> {
    let item = base_item_to_media_item(dto, source_id)?;

    // Cast: actors (or anyone without a declared type, to degrade gracefully).
    let cast: Vec<CastMember> = dto
        .people
        .iter()
        .filter(|p| matches!(p.type_.as_deref(), Some("Actor") | None))
        .map(|p| CastMember {
            name: p.name.clone(),
            character: p.role.clone(),
            // Jellyfin person images need a separate tag; leave None for v1.
            photo_path: None,
        })
        .collect();

    let directors: Vec<String> = dto
        .people
        .iter()
        .filter(|p| p.type_.as_deref() == Some("Director"))
        .map(|p| p.name.clone())
        .collect();

    let writers: Vec<String> = dto
        .people
        .iter()
        .filter(|p| p.type_.as_deref() == Some("Writer"))
        .map(|p| p.name.clone())
        .collect();

    // Technical info: MediaSourceInfo only carries container/path/name/id in v1,
    // so most fields are None. Reuse the base item's height-derived resolution.
    let technical = dto.media_sources.first().map(|ms| TechnicalInfo {
        video_resolution: resolution_label(dto.height),
        video_codec: None,
        audio_codec: None,
        audio_channels: None,
        container: ms.container.clone(),
        bitrate_kbps: None,
        file_size_bytes: None,
    });

    Some(MediaDetail {
        item,
        cast,
        directors,
        writers,
        technical,
        // Jellyfin collection membership isn't on the base item without extra calls.
        collections: Vec::new(),
    })
}

/// Convert a Jellyfin library "view" (a `BaseItemDto` with a `CollectionType`)
/// to a `LibrarySection`. Returns `None` for views we don't browse as a flat
/// library (music, boxsets, livetv, etc.).
pub fn user_view_to_section(dto: &BaseItemDto) -> Option<LibrarySection> {
    let library_type = match dto.collection_type.as_deref() {
        Some("movies") => LibraryType::Movie,
        Some("tvshows") => LibraryType::Show,
        _ => return None,
    };

    Some(LibrarySection {
        key: dto.id.clone(),
        title: dto.name.clone().unwrap_or_default(),
        library_type,
        count: None,
    })
}

/// Convert Jellyfin Media Segments to `SkipMarkers`.
///
/// Jellyfin carries explicit segment types ("Intro", "Outro", …) with tick
/// ranges, so no heuristics are needed (unlike the Plex chapter converter).
/// Returns `None` when neither an intro nor an outro segment is present, so the
/// skip button stays hidden.
pub fn media_segments_to_skip_markers(segments: &[MediaSegmentDto]) -> Option<SkipMarkers> {
    let intro = segments
        .iter()
        .find(|s| s.type_.as_deref() == Some("Intro"));
    let outro = segments
        .iter()
        .find(|s| s.type_.as_deref() == Some("Outro"));

    if intro.is_none() && outro.is_none() {
        return None;
    }

    let (intro_start_secs, intro_end_secs) = match intro {
        Some(seg) => (
            seg.start_ticks.map(|t| Ticks(t).to_secs()).unwrap_or(0.0),
            seg.end_ticks.map(|t| Ticks(t).to_secs()).unwrap_or(0.0),
        ),
        None => (0.0, 0.0),
    };

    let credits_start_secs = outro
        .and_then(|seg| seg.start_ticks)
        .map(|t| Ticks(t).to_secs())
        .unwrap_or(0.0);

    Some(SkipMarkers {
        intro_start_secs,
        intro_end_secs,
        credits_start_secs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::jellyfin::models::{MediaSourceInfo, PersonDto, UserItemData};
    use std::collections::HashMap;

    const SOURCE: &str = "http://jelly.local:8096";

    fn test_movie_dto() -> BaseItemDto {
        let mut image_tags = HashMap::new();
        image_tags.insert("Primary".to_string(), "primtag".to_string());

        BaseItemDto {
            id: "abc123".to_string(),
            name: Some("Dune".to_string()),
            type_: Some("Movie".to_string()),
            overview: Some("A noble family.".to_string()),
            production_year: Some(2021),
            premiere_date: Some("2021-10-22T00:00:00.0000000Z".to_string()),
            run_time_ticks: Some(155 * 60 * 10_000_000), // 155 minutes
            official_rating: Some("PG-13".to_string()),
            community_rating: Some(8.0),
            genres: vec!["Science Fiction".to_string(), "Adventure".to_string()],
            people: vec![],
            index_number: None,
            parent_index_number: None,
            series_id: None,
            season_id: None,
            series_name: None,
            parent_id: Some("lib-movies".to_string()),
            image_tags: Some(image_tags),
            backdrop_image_tags: vec!["backtag".to_string()],
            parent_backdrop_item_id: None,
            parent_backdrop_image_tags: vec![],
            series_primary_image_tag: None,
            user_data: None,
            media_sources: vec![MediaSourceInfo {
                id: Some("src1".to_string()),
                path: Some("/movies/dune.mkv".to_string()),
                container: Some("mkv".to_string()),
                name: None,
            }],
            collection_type: None,
            width: Some(1920),
            height: Some(1080),
        }
    }

    fn test_episode_dto() -> BaseItemDto {
        BaseItemDto {
            id: "ep1".to_string(),
            name: Some("Pilot".to_string()),
            type_: Some("Episode".to_string()),
            overview: Some("The first episode.".to_string()),
            production_year: None,
            premiere_date: Some("2024-03-01T00:00:00.0000000Z".to_string()),
            run_time_ticks: Some(42 * 60 * 10_000_000),
            official_rating: None,
            community_rating: Some(9.0),
            genres: vec![],
            people: vec![],
            index_number: Some(1),
            parent_index_number: Some(2),
            series_id: Some("series9".to_string()),
            season_id: Some("season3".to_string()),
            series_name: Some("Some Show".to_string()),
            parent_id: Some("season3".to_string()),
            image_tags: None,
            backdrop_image_tags: vec![],
            parent_backdrop_item_id: Some("series9".to_string()),
            parent_backdrop_image_tags: vec!["parentback".to_string()],
            series_primary_image_tag: Some("seriesprim".to_string()),
            user_data: None,
            media_sources: vec![],
            collection_type: None,
            width: None,
            height: None,
        }
    }

    fn test_movie_view() -> BaseItemDto {
        BaseItemDto {
            id: "lib-movies".to_string(),
            name: Some("Movies".to_string()),
            collection_type: Some("movies".to_string()),
            ..BaseItemDto::default()
        }
    }

    #[test]
    fn convert_movie_maps_all_fields() {
        let dto = test_movie_dto();
        let item = base_item_to_media_item(&dto, SOURCE).unwrap();

        assert_eq!(item.media_type, MediaType::Movie);
        assert_eq!(item.title, "Dune");
        assert_eq!(item.year, Some(2021));
        assert_eq!(item.overview, Some("A noble family.".to_string()));
        assert_eq!(item.content_rating, Some("PG-13".to_string()));
        assert_eq!(item.rating, Some(8.0));
        assert_eq!(item.runtime_minutes, Some(155));
        assert_eq!(item.genres, vec!["Science Fiction", "Adventure"]);
        assert_eq!(item.poster_path, Some("abc123/Primary/primtag".to_string()));
        assert_eq!(
            item.backdrop_path,
            Some("abc123/Backdrop/backtag".to_string())
        );
        assert_eq!(item.video_resolution, Some("1080".to_string()));
        // Composite "{item_id}|{media_source_id}" for direct-play.
        assert_eq!(item.file_path, Some("abc123|src1".to_string()));
        assert_eq!(item.external_id, "abc123");
        assert_eq!(item.source_type, SourceType::Jellyfin);
        assert_eq!(item.source_id, SOURCE);
        assert_eq!(item.library_section_id, Some("lib-movies".to_string()));
        assert_eq!(item.hdr, None);
        assert_eq!(item.series_poster_path, None);
    }

    #[test]
    fn convert_movie_composite_id() {
        let dto = test_movie_dto();
        let item = base_item_to_media_item(&dto, SOURCE).unwrap();
        assert_eq!(item.id, format!("jellyfin:{SOURCE}:abc123"));
    }

    #[test]
    fn convert_movie_no_poster_when_no_primary_tag() {
        let mut dto = test_movie_dto();
        dto.image_tags = None;
        let item = base_item_to_media_item(&dto, SOURCE).unwrap();
        assert_eq!(item.poster_path, None);
    }

    #[test]
    fn convert_episode_sets_season_episode_series() {
        let dto = test_episode_dto();
        let item = base_item_to_media_item(&dto, SOURCE).unwrap();

        assert_eq!(item.media_type, MediaType::Episode);
        assert_eq!(item.season_number, Some(2));
        assert_eq!(item.episode_number, Some(1));
        assert_eq!(item.parent_id, Some(format!("jellyfin:{SOURCE}:season3")));
        assert_eq!(
            item.series_poster_path,
            Some("series9/Primary/seriesprim".to_string())
        );
    }

    #[test]
    fn convert_episode_backdrop_falls_back_to_parent() {
        let dto = test_episode_dto();
        let item = base_item_to_media_item(&dto, SOURCE).unwrap();
        assert_eq!(
            item.backdrop_path,
            Some("series9/Backdrop/parentback".to_string())
        );
    }

    #[test]
    fn convert_season_parent_id_from_series() {
        let mut dto = test_episode_dto();
        dto.type_ = Some("Season".to_string());
        dto.index_number = Some(2);
        let item = base_item_to_media_item(&dto, SOURCE).unwrap();
        assert_eq!(item.media_type, MediaType::Season);
        assert_eq!(item.season_number, Some(2));
        assert_eq!(item.episode_number, None);
        assert_eq!(item.parent_id, Some(format!("jellyfin:{SOURCE}:series9")));
    }

    #[test]
    fn convert_populates_watched_from_userdata() {
        let mut dto = test_movie_dto();
        dto.user_data = Some(UserItemData {
            played: true,
            playback_position_ticks: None,
            play_count: Some(2),
            is_favorite: false,
        });
        let item = base_item_to_media_item(&dto, SOURCE).unwrap();
        assert!(item.watched);
    }

    #[test]
    fn convert_populates_resume_position_from_ticks() {
        let mut dto = test_movie_dto();
        dto.user_data = Some(UserItemData {
            played: false,
            playback_position_ticks: Some(1122 * 10_000_000), // 1122 seconds
            play_count: None,
            is_favorite: false,
        });
        let item = base_item_to_media_item(&dto, SOURCE).unwrap();
        assert_eq!(item.playback_position_ms, Some(1_122_000));
    }

    #[test]
    fn convert_unwatched_has_no_resume() {
        let dto = test_movie_dto(); // user_data is None
        let item = base_item_to_media_item(&dto, SOURCE).unwrap();
        assert!(!item.watched);
        assert_eq!(item.playback_position_ms, None);
    }

    #[test]
    fn convert_no_media_sources_falls_back_to_bare_id_for_playable() {
        // A playable item (Movie/Episode) with no media sources falls back to
        // its bare id so it can still be streamed (used as both item + source).
        let mut dto = test_movie_dto();
        dto.media_sources.clear();
        let item = base_item_to_media_item(&dto, SOURCE).unwrap();
        assert_eq!(item.file_path, Some("abc123".to_string()));
    }

    #[test]
    fn convert_no_media_sources_gives_none_file_path_for_non_playable() {
        // A Series is not directly playable; no media sources → no file_path.
        let mut dto = test_movie_dto();
        dto.type_ = Some("Series".to_string());
        dto.media_sources.clear();
        let item = base_item_to_media_item(&dto, SOURCE).unwrap();
        assert_eq!(item.file_path, None);
    }

    #[test]
    fn convert_no_runtime_gives_none() {
        let mut dto = test_movie_dto();
        dto.run_time_ticks = None;
        let item = base_item_to_media_item(&dto, SOURCE).unwrap();
        assert_eq!(item.runtime_minutes, None);
    }

    #[test]
    fn convert_resolution_label_from_height() {
        let mut dto = test_movie_dto();
        dto.height = Some(2160);
        assert_eq!(
            base_item_to_media_item(&dto, SOURCE)
                .unwrap()
                .video_resolution,
            Some("4k".to_string())
        );
        dto.height = Some(720);
        assert_eq!(
            base_item_to_media_item(&dto, SOURCE)
                .unwrap()
                .video_resolution,
            Some("720".to_string())
        );
        dto.height = Some(480);
        assert_eq!(
            base_item_to_media_item(&dto, SOURCE)
                .unwrap()
                .video_resolution,
            Some("sd".to_string())
        );
        dto.height = None;
        assert_eq!(
            base_item_to_media_item(&dto, SOURCE)
                .unwrap()
                .video_resolution,
            None
        );
    }

    #[test]
    fn convert_unsupported_type_returns_none() {
        let mut dto = test_movie_dto();
        dto.type_ = Some("Audio".to_string());
        assert!(base_item_to_media_item(&dto, SOURCE).is_none());
    }

    #[test]
    fn convert_box_set_maps_to_collection() {
        let mut dto = test_movie_dto();
        dto.type_ = Some("BoxSet".to_string());
        let item = base_item_to_media_item(&dto, SOURCE).unwrap();
        assert_eq!(item.media_type, MediaType::Collection);
    }

    // --- Library view conversion ---

    #[test]
    fn user_view_movies_maps_to_movie_section() {
        let dto = test_movie_view();
        let section = user_view_to_section(&dto).unwrap();
        assert_eq!(section.key, "lib-movies");
        assert_eq!(section.title, "Movies");
        assert_eq!(section.library_type, LibraryType::Movie);
        assert_eq!(section.count, None);
    }

    #[test]
    fn user_view_tvshows_maps_to_show_section() {
        let mut dto = test_movie_view();
        dto.collection_type = Some("tvshows".to_string());
        let section = user_view_to_section(&dto).unwrap();
        assert_eq!(section.library_type, LibraryType::Show);
    }

    #[test]
    fn user_view_music_returns_none() {
        let mut dto = test_movie_view();
        dto.collection_type = Some("music".to_string());
        assert!(user_view_to_section(&dto).is_none());
    }

    #[test]
    fn user_view_no_collection_type_returns_none() {
        let mut dto = test_movie_view();
        dto.collection_type = None;
        assert!(user_view_to_section(&dto).is_none());
    }

    // --- Media segments → skip markers ---

    #[test]
    fn media_segments_to_skip_markers_maps_intro_outro() {
        let segments = vec![
            MediaSegmentDto {
                id: Some("s1".to_string()),
                type_: Some("Intro".to_string()),
                start_ticks: Some(10 * 10_000_000), // 10s
                end_ticks: Some(95 * 10_000_000),   // 95s
            },
            MediaSegmentDto {
                id: Some("s2".to_string()),
                type_: Some("Outro".to_string()),
                start_ticks: Some(3000 * 10_000_000), // 3000s
                end_ticks: Some(3100 * 10_000_000),
            },
        ];
        let markers = media_segments_to_skip_markers(&segments).unwrap();
        assert_eq!(markers.intro_start_secs, 10.0);
        assert_eq!(markers.intro_end_secs, 95.0);
        assert_eq!(markers.credits_start_secs, 3000.0);
    }

    #[test]
    fn media_segments_empty_returns_none() {
        assert!(media_segments_to_skip_markers(&[]).is_none());
    }

    #[test]
    fn media_segments_intro_only_zeroes_credits() {
        let segments = vec![MediaSegmentDto {
            id: None,
            type_: Some("Intro".to_string()),
            start_ticks: Some(0),
            end_ticks: Some(90 * 10_000_000),
        }];
        let markers = media_segments_to_skip_markers(&segments).unwrap();
        assert_eq!(markers.intro_start_secs, 0.0);
        assert_eq!(markers.intro_end_secs, 90.0);
        assert_eq!(markers.credits_start_secs, 0.0);
    }

    #[test]
    fn media_segments_unrelated_types_return_none() {
        let segments = vec![MediaSegmentDto {
            id: None,
            type_: Some("Commercial".to_string()),
            start_ticks: Some(0),
            end_ticks: Some(10 * 10_000_000),
        }];
        assert!(media_segments_to_skip_markers(&segments).is_none());
    }

    // --- MediaDetail conversion ---

    #[test]
    fn convert_detail_extracts_cast_directors_writers() {
        let mut dto = test_movie_dto();
        dto.people = vec![
            PersonDto {
                name: "Actor A".to_string(),
                role: Some("Paul".to_string()),
                type_: Some("Actor".to_string()),
            },
            PersonDto {
                name: "Denis Villeneuve".to_string(),
                role: None,
                type_: Some("Director".to_string()),
            },
            PersonDto {
                name: "Jon Spaihts".to_string(),
                role: None,
                type_: Some("Writer".to_string()),
            },
        ];

        let detail = base_item_to_media_detail(&dto, SOURCE).unwrap();
        assert_eq!(detail.cast.len(), 1);
        assert_eq!(detail.cast[0].name, "Actor A");
        assert_eq!(detail.cast[0].character, Some("Paul".to_string()));
        assert_eq!(detail.cast[0].photo_path, None);
        assert_eq!(detail.directors, vec!["Denis Villeneuve"]);
        assert_eq!(detail.writers, vec!["Jon Spaihts"]);
    }

    #[test]
    fn convert_detail_extracts_technical_from_media_source() {
        let dto = test_movie_dto();
        let detail = base_item_to_media_detail(&dto, SOURCE).unwrap();
        let tech = detail.technical.unwrap();
        assert_eq!(tech.container, Some("mkv".to_string()));
        assert_eq!(tech.video_resolution, Some("1080".to_string()));
        assert_eq!(tech.video_codec, None);
        assert_eq!(tech.audio_channels, None);
    }

    #[test]
    fn convert_detail_no_media_source_gives_none_technical() {
        let mut dto = test_movie_dto();
        dto.media_sources.clear();
        let detail = base_item_to_media_detail(&dto, SOURCE).unwrap();
        assert!(detail.technical.is_none());
    }

    #[test]
    fn convert_detail_empty_collections() {
        let dto = test_movie_dto();
        let detail = base_item_to_media_detail(&dto, SOURCE).unwrap();
        assert!(detail.collections.is_empty());
    }

    #[test]
    fn convert_detail_unsupported_type_returns_none() {
        let mut dto = test_movie_dto();
        dto.type_ = Some("MusicAlbum".to_string());
        assert!(base_item_to_media_detail(&dto, SOURCE).is_none());
    }

    #[test]
    fn convert_detail_preserves_base_item() {
        let dto = test_movie_dto();
        let detail = base_item_to_media_detail(&dto, SOURCE).unwrap();
        assert_eq!(detail.item.title, "Dune");
        assert_eq!(detail.item.media_type, MediaType::Movie);
    }
}
