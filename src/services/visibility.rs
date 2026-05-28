use std::collections::HashSet;

use crate::models::library::LibrarySection;
use crate::models::media::MediaItem;

/// Drop items whose library is hidden. Each item self-describes its library via
/// `source_type`, `source_id`, and `library_section_id`, so the composite key is
/// rebuilt per item and checked against `hidden` (the set of hidden keys from
/// `LibrarySection::visibility_key_for`). Items with no known section are kept
/// (treated as visible — see KTD4: best-effort for sources that omit the section).
pub fn retain_visible_items(items: Vec<MediaItem>, hidden: &HashSet<String>) -> Vec<MediaItem> {
    if hidden.is_empty() {
        return items;
    }
    items
        .into_iter()
        .filter(|item| match &item.library_section_id {
            Some(section_key) => !hidden.contains(&LibrarySection::visibility_key_for(
                item.source_type.as_str(),
                &item.source_id,
                section_key,
            )),
            None => true,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::media::{MediaType, SourceType};

    fn item(source_id: &str, section: Option<&str>) -> MediaItem {
        MediaItem {
            id: format!("plex:{source_id}:x"),
            source_type: SourceType::Plex,
            source_id: source_id.to_string(),
            external_id: "x".to_string(),
            media_type: MediaType::Movie,
            title: "X".to_string(),
            year: None,
            overview: None,
            content_rating: None,
            rating: None,
            runtime_minutes: None,
            poster_path: None,
            backdrop_path: None,
            genres: vec![],
            parent_id: None,
            season_number: None,
            episode_number: None,
            air_date: None,
            file_path: None,
            video_resolution: None,
            hdr: None,
            added_at: String::new(),
            updated_at: String::new(),
            playback_position_ms: None,
            library_section_id: section.map(String::from),
        }
    }

    fn hidden(keys: &[&str]) -> HashSet<String> {
        keys.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_hidden_set_returns_all() {
        let items = vec![item("srv", Some("1")), item("srv", Some("2"))];
        let out = retain_visible_items(items, &HashSet::new());
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn drops_items_from_hidden_library() {
        let items = vec![item("srv", Some("1")), item("srv", Some("2"))];
        let out = retain_visible_items(items, &hidden(&["plex:srv:2"]));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].library_section_id, Some("1".to_string()));
    }

    #[test]
    fn keeps_items_with_no_section() {
        let items = vec![item("srv", None), item("srv", Some("2"))];
        let out = retain_visible_items(items, &hidden(&["plex:srv:2"]));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].library_section_id, None);
    }

    #[test]
    fn visibility_is_source_scoped() {
        // Section "2" is hidden only on srvA; the same key on srvB stays visible.
        let items = vec![item("srvA", Some("2")), item("srvB", Some("2"))];
        let out = retain_visible_items(items, &hidden(&["plex:srvA:2"]));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].source_id, "srvB");
    }
}
