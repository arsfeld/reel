//! Snapshot and top-up logic for grouped (show/season) downloads.
//!
//! Pure functions over already-fetched `MediaItem` lists — no network. The
//! caller fetches a show's or season's children; these functions decide which
//! episodes are downloadable now and, on a re-trigger, which are missing.
//!
//! A "snapshot" is the set of episodes that exist *and have a downloadable
//! file* at trigger time (R5). Episodes with metadata but no media file
//! (unaired, not-yet-available) are excluded so they don't become spurious
//! failures. Episodes added later are not auto-enqueued; a re-trigger tops up
//! only the missing ones (R6).

use std::collections::HashSet;

use crate::models::media::MediaItem;

/// Whether an item has a downloadable file (a non-empty `file_path` / Plex part
/// key). Items lacking one (unaired/unavailable episodes) are not downloadable.
#[allow(dead_code)]
pub fn is_downloadable(item: &MediaItem) -> bool {
    item.file_path.as_deref().is_some_and(|p| !p.is_empty())
}

/// The episode set to enqueue for a show/season download: every downloadable
/// episode in the provided list. The caller scopes the list (whole show vs one
/// season) before calling; see [`filter_to_season`].
#[allow(dead_code)]
pub fn snapshot_set(episodes: &[MediaItem]) -> Vec<&MediaItem> {
    episodes.iter().filter(|e| is_downloadable(e)).collect()
}

/// Restrict an episode list to a single season number.
#[allow(dead_code)]
pub fn filter_to_season(episodes: &[MediaItem], season_number: i32) -> Vec<&MediaItem> {
    episodes
        .iter()
        .filter(|e| e.season_number == Some(season_number))
        .collect()
}

/// The top-up delta on a re-trigger: downloadable episodes from the current set
/// that are not already present as a download.
///
/// `existing_ids` is the set of `media_item_id`s that already have a download
/// in any non-removed state (Queued/Downloading/Paused/Completed/Failed). Items
/// in that set — including ones currently transferring — are skipped, so a
/// re-trigger never re-enqueues an in-flight or completed episode (AE2).
#[allow(dead_code)]
pub fn top_up_delta<'a>(
    current: &'a [MediaItem],
    existing_ids: &HashSet<String>,
) -> Vec<&'a MediaItem> {
    current
        .iter()
        .filter(|e| is_downloadable(e) && !existing_ids.contains(&e.id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::media::{MediaType, SourceType};

    fn episode(id: &str, season: i32, ep: i32, has_file: bool) -> MediaItem {
        MediaItem {
            id: id.to_string(),
            source_type: SourceType::Plex,
            source_id: "http://localhost:32400".to_string(),
            external_id: id.to_string(),
            media_type: MediaType::Episode,
            title: format!("Episode {ep}"),
            year: None,
            overview: None,
            content_rating: None,
            rating: None,
            runtime_minutes: Some(45),
            poster_path: None,
            series_poster_path: None,
            backdrop_path: None,
            genres: vec![],
            parent_id: Some("show1".to_string()),
            season_number: Some(season),
            episode_number: Some(ep),
            air_date: None,
            file_path: if has_file {
                Some(format!("/library/parts/{id}/file.mkv"))
            } else {
                None
            },
            video_resolution: None,
            hdr: None,
            added_at: "2026-01-01".to_string(),
            updated_at: "2026-01-01".to_string(),
            playback_position_ms: None,
            watched: false,
            library_section_id: None,
        }
    }

    fn season_one(count: i32) -> Vec<MediaItem> {
        (1..=count)
            .map(|e| episode(&format!("ep{e}"), 1, e, true))
            .collect()
    }

    #[test]
    fn snapshot_set_includes_only_downloadable() {
        let mut eps = season_one(3);
        eps.push(episode("ep4_unaired", 1, 4, false)); // metadata only, no file
        let set = snapshot_set(&eps);
        assert_eq!(set.len(), 3);
        assert!(!set.iter().any(|e| e.id == "ep4_unaired"));
    }

    #[test]
    fn top_up_delta_returns_only_new_episodes() {
        // Covers AE2: snapshotted at 8, source now has 10 -> 2 new enqueued,
        // the 8 existing untouched.
        let current = season_one(10);
        let existing: HashSet<String> = (1..=8).map(|e| format!("ep{e}")).collect();
        let delta = top_up_delta(&current, &existing);
        assert_eq!(delta.len(), 2);
        let ids: Vec<&str> = delta.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["ep9", "ep10"]);
    }

    #[test]
    fn top_up_delta_empty_when_nothing_new() {
        let current = season_one(8);
        let existing: HashSet<String> = (1..=8).map(|e| format!("ep{e}")).collect();
        assert!(top_up_delta(&current, &existing).is_empty());
    }

    #[test]
    fn top_up_delta_skips_unaired_episodes() {
        let mut current = season_one(8);
        current.push(episode("ep9_unaired", 1, 9, false));
        let existing: HashSet<String> = (1..=8).map(|e| format!("ep{e}")).collect();
        // ep9 has no file -> not part of the delta.
        assert!(top_up_delta(&current, &existing).is_empty());
    }

    #[test]
    fn top_up_delta_does_not_re_enqueue_active_episode() {
        // An episode currently Downloading/Paused is in existing_ids, so it is
        // not re-enqueued by a top-up.
        let current = season_one(3);
        let existing: HashSet<String> = ["ep2".to_string()].into_iter().collect();
        let delta = top_up_delta(&current, &existing);
        let ids: Vec<&str> = delta.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["ep1", "ep3"]);
    }

    #[test]
    fn filter_to_season_returns_only_that_season() {
        let mut eps = vec![
            episode("s1e1", 1, 1, true),
            episode("s1e2", 1, 2, true),
            episode("s2e1", 2, 1, true),
        ];
        eps.push(episode("s2e2", 2, 2, true));
        let s1 = filter_to_season(&eps, 1);
        assert_eq!(s1.len(), 2);
        assert!(s1.iter().all(|e| e.season_number == Some(1)));
    }
}
