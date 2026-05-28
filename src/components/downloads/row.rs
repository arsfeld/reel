//! Pure row derivation for the Downloads view.
//!
//! Mirrors the `derive_rows`-style pattern from `components::sidebar`: the GTK
//! component holds the raw download/group state and this module turns it into an
//! ordered list of renderable rows — standalone movie (or single-episode) rows
//! and grouped show rows carrying the aggregate "6/10 episodes" status. No
//! widgets, so the information architecture is unit-testable without a display.

use crate::models::download::{Download, DownloadGroup, DownloadState, FailReason};
use crate::models::media::MediaType;
use crate::services::download::{GroupStatus, derive_group_status, group_progress};

/// A single download projected for rendering (movie, or one episode).
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct DownloadItemView {
    pub media_item_id: String,
    pub title: String,
    pub state: DownloadState,
    pub fail_reason: Option<FailReason>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    /// Fractional progress `[0,1]` when a total size is known, else `None`.
    pub progress: Option<f64>,
    /// Bytes transferred so far (advisory), for the size-aware status line.
    pub byte_count: i64,
    /// Expected total size when known, for the size-aware status line.
    pub total_size: Option<i64>,
    pub poster_path: Option<String>,
}

impl DownloadItemView {
    fn from_download(d: &Download) -> Self {
        let progress = d
            .total_size
            .filter(|t| *t > 0)
            .map(|t| (d.byte_count as f64 / t as f64).clamp(0.0, 1.0));
        Self {
            media_item_id: d.media_item_id.clone(),
            title: d.title.clone(),
            state: d.state,
            fail_reason: d.fail_reason,
            season_number: d.season_number,
            episode_number: d.episode_number,
            progress,
            byte_count: d.byte_count,
            total_size: d.total_size,
            poster_path: d.poster_path.clone(),
        }
    }
}

/// One rendered row: a standalone item or an expandable show group.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum DownloadRow {
    /// A standalone movie or a single, group-less episode.
    Standalone(DownloadItemView),
    /// A grouped show/season download with aggregate progress + members.
    Group {
        group_id: String,
        title: String,
        status: GroupStatus,
        /// Completed-over-outstanding count for the "6/10 episodes" label.
        done: usize,
        total: usize,
        episodes: Vec<DownloadItemView>,
    },
}

/// A download is shown only when it is in an outstanding (non-terminal-removed)
/// state. `Removed`/`Pruned` rows have left the view.
fn is_visible(state: DownloadState) -> bool {
    !matches!(state, DownloadState::Removed | DownloadState::Pruned)
}

/// Derive the ordered Downloads rows from raw download + group state.
///
/// Movies and group-less episodes become standalone rows; episodes tagged with
/// a `group_id` cluster under that group's row, sorted by season then episode.
/// Group rows come first (by title), then standalone rows (by title) for a
/// stable, source-independent ordering that works offline.
#[allow(dead_code)]
pub fn derive_rows(downloads: &[Download], groups: &[DownloadGroup]) -> Vec<DownloadRow> {
    let visible: Vec<&Download> = downloads.iter().filter(|d| is_visible(d.state)).collect();

    // Group rows, in the order `groups` lists them, but only when they have at
    // least one visible member.
    let mut group_rows: Vec<DownloadRow> = Vec::new();
    for g in groups {
        let mut members: Vec<&Download> = visible
            .iter()
            .copied()
            .filter(|d| d.group_id.as_deref() == Some(g.id.as_str()))
            .collect();
        if members.is_empty() {
            continue;
        }
        members.sort_by_key(|d| (d.season_number, d.episode_number, d.media_item_id.clone()));
        let states: Vec<DownloadState> = members.iter().map(|d| d.state).collect();
        let (done, total) = group_progress(&states);
        group_rows.push(DownloadRow::Group {
            group_id: g.id.clone(),
            title: g.title.clone(),
            status: derive_group_status(&states),
            done,
            total,
            episodes: members
                .iter()
                .map(|d| DownloadItemView::from_download(d))
                .collect(),
        });
    }
    group_rows.sort_by(|a, b| group_title(a).cmp(group_title(b)));

    // Standalone rows: movies and any episode without a group.
    let mut standalone: Vec<DownloadRow> = visible
        .iter()
        .filter(|d| d.media_type == MediaType::Movie || d.group_id.is_none())
        .map(|d| DownloadRow::Standalone(DownloadItemView::from_download(d)))
        .collect();
    standalone.sort_by(|a, b| standalone_title(a).cmp(standalone_title(b)));

    group_rows.extend(standalone);
    group_rows
}

fn group_title(row: &DownloadRow) -> &str {
    match row {
        DownloadRow::Group { title, .. } => title,
        DownloadRow::Standalone(v) => &v.title,
    }
}

fn standalone_title(row: &DownloadRow) -> &str {
    match row {
        DownloadRow::Standalone(v) => &v.title,
        DownloadRow::Group { title, .. } => title,
    }
}

/// The members of a group that a "retry" action applies to — only the failed
/// ones (retrying a completed or in-flight episode is a no-op).
#[allow(dead_code)]
pub fn group_retry_targets(episodes: &[DownloadItemView]) -> Vec<String> {
    episodes
        .iter()
        .filter(|e| e.state == DownloadState::Failed)
        .map(|e| e.media_item_id.clone())
        .collect()
}

/// Whether a group "delete" should require confirmation: true when any member
/// has a completed file on disk that would be removed.
#[allow(dead_code)]
pub fn group_delete_needs_confirm(episodes: &[DownloadItemView]) -> bool {
    episodes.iter().any(|e| e.state == DownloadState::Completed)
}

/// Whether a row can be reordered in the queue. Only `Queued` items move; an
/// in-flight `Downloading` item keeps its slot (reorder is not preemptive).
#[allow(dead_code)]
pub fn is_reorderable(state: DownloadState) -> bool {
    state == DownloadState::Queued
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::download::GroupScope;
    use crate::models::media::SourceType;

    fn movie(id: &str, title: &str, state: DownloadState) -> Download {
        Download {
            media_item_id: id.to_string(),
            part_key: "/p".to_string(),
            source_type: SourceType::Plex,
            source_id: "s".to_string(),
            state,
            fail_reason: None,
            byte_count: 0,
            total_size: Some(100),
            validator: None,
            file_path: None,
            group_id: None,
            queue_order: 0,
            enqueued_at: "2026-05-28T10:00:00Z".to_string(),
            completed_at: None,
            media_type: MediaType::Movie,
            title: title.to_string(),
            year: Some(2021),
            parent_id: None,
            season_number: None,
            episode_number: None,
            poster_path: None,
        }
    }

    fn episode(
        id: &str,
        group: Option<&str>,
        season: i32,
        ep: i32,
        state: DownloadState,
    ) -> Download {
        Download {
            media_item_id: id.to_string(),
            media_type: MediaType::Episode,
            group_id: group.map(String::from),
            season_number: Some(season),
            episode_number: Some(ep),
            state,
            title: format!("Episode {ep}"),
            ..movie(id, &format!("Episode {ep}"), state)
        }
    }

    fn group(id: &str, title: &str) -> DownloadGroup {
        DownloadGroup {
            id: id.to_string(),
            scope: GroupScope::Show,
            parent_media_id: "show1".to_string(),
            title: title.to_string(),
            snapshot_at: "2026-05-28T10:00:00Z".to_string(),
        }
    }

    #[test]
    fn movies_render_standalone() {
        let rows = derive_rows(&[movie("m1", "Dune", DownloadState::Completed)], &[]);
        assert_eq!(rows.len(), 1);
        assert!(matches!(&rows[0], DownloadRow::Standalone(v) if v.title == "Dune"));
    }

    #[test]
    fn show_group_aggregates_six_of_ten() {
        // Covers AE1: 10-episode show, 6 complete + 4 queued -> "6/10".
        let mut downloads = Vec::new();
        for e in 1..=6 {
            downloads.push(episode(
                &format!("e{e}"),
                Some("g1"),
                1,
                e,
                DownloadState::Completed,
            ));
        }
        for e in 7..=10 {
            downloads.push(episode(
                &format!("e{e}"),
                Some("g1"),
                1,
                e,
                DownloadState::Queued,
            ));
        }
        let rows = derive_rows(&downloads, &[group("g1", "The Show")]);
        assert_eq!(rows.len(), 1);
        let DownloadRow::Group {
            done,
            total,
            status,
            episodes,
            ..
        } = &rows[0]
        else {
            panic!("expected a group row");
        };
        assert_eq!((*done, *total), (6, 10));
        // A queued member keeps the group active.
        assert_eq!(*status, GroupStatus::Downloading);
        assert_eq!(episodes.len(), 10);
        // Each episode carries its own state.
        assert_eq!(episodes[0].state, DownloadState::Completed);
        assert_eq!(episodes[9].state, DownloadState::Queued);
    }

    #[test]
    fn group_excludes_separately_enqueued_episode() {
        // ep3 of the same show is enqueued on its own (group_id None) -> it is a
        // standalone row, not a group member.
        let downloads = vec![
            episode("e1", Some("g1"), 1, 1, DownloadState::Completed),
            episode("e2", Some("g1"), 1, 2, DownloadState::Completed),
            episode("e3", None, 1, 3, DownloadState::Queued),
        ];
        let rows = derive_rows(&downloads, &[group("g1", "The Show")]);
        // One group (2 members) + one standalone episode.
        let group_row = rows
            .iter()
            .find_map(|r| match r {
                DownloadRow::Group { episodes, .. } => Some(episodes),
                _ => None,
            })
            .unwrap();
        assert_eq!(group_row.len(), 2);
        assert!(!group_row.iter().any(|e| e.media_item_id == "e3"));
        assert!(
            rows.iter()
                .any(|r| matches!(r, DownloadRow::Standalone(v) if v.media_item_id == "e3"))
        );
    }

    #[test]
    fn group_retry_targets_only_failed() {
        let downloads = vec![
            episode("e1", Some("g1"), 1, 1, DownloadState::Completed),
            episode("e2", Some("g1"), 1, 2, DownloadState::Failed),
            episode("e3", Some("g1"), 1, 3, DownloadState::Failed),
        ];
        let rows = derive_rows(&downloads, &[group("g1", "The Show")]);
        let DownloadRow::Group { episodes, .. } = &rows[0] else {
            panic!();
        };
        assert_eq!(group_retry_targets(episodes), vec!["e2", "e3"]);
    }

    #[test]
    fn all_failed_group_is_failed_partial_is_partial() {
        let all_failed = vec![
            episode("e1", Some("g1"), 1, 1, DownloadState::Failed),
            episode("e2", Some("g1"), 1, 2, DownloadState::Failed),
        ];
        let rows = derive_rows(&all_failed, &[group("g1", "S")]);
        assert!(matches!(
            &rows[0],
            DownloadRow::Group {
                status: GroupStatus::Failed,
                ..
            }
        ));

        let partial = vec![
            episode("e1", Some("g2"), 1, 1, DownloadState::Completed),
            episode("e2", Some("g2"), 1, 2, DownloadState::Failed),
        ];
        let rows = derive_rows(&partial, &[group("g2", "S")]);
        assert!(matches!(
            &rows[0],
            DownloadRow::Group {
                status: GroupStatus::PartiallyFailed,
                ..
            }
        ));
    }

    #[test]
    fn delete_confirm_required_only_when_completed_present() {
        let with_completed = vec![DownloadItemView {
            media_item_id: "e1".into(),
            title: "E1".into(),
            state: DownloadState::Completed,
            fail_reason: None,
            season_number: Some(1),
            episode_number: Some(1),
            progress: None,
            byte_count: 0,
            total_size: None,
            poster_path: None,
        }];
        assert!(group_delete_needs_confirm(&with_completed));

        let only_queued = vec![DownloadItemView {
            state: DownloadState::Queued,
            ..with_completed[0].clone()
        }];
        assert!(!group_delete_needs_confirm(&only_queued));
    }

    #[test]
    fn only_queued_is_reorderable() {
        assert!(is_reorderable(DownloadState::Queued));
        for s in [
            DownloadState::Downloading,
            DownloadState::Paused,
            DownloadState::Completed,
            DownloadState::Failed,
        ] {
            assert!(!is_reorderable(s));
        }
    }

    #[test]
    fn removed_and_pruned_rows_are_hidden() {
        let downloads = vec![
            movie("m1", "Shown", DownloadState::Completed),
            movie("m2", "Gone", DownloadState::Removed),
            movie("m3", "Pruned", DownloadState::Pruned),
        ];
        let rows = derive_rows(&downloads, &[]);
        assert_eq!(rows.len(), 1);
        assert!(matches!(&rows[0], DownloadRow::Standalone(v) if v.title == "Shown"));
    }

    #[test]
    fn progress_fraction_derived_from_bytes() {
        let mut d = movie("m1", "Dune", DownloadState::Downloading);
        d.total_size = Some(200);
        d.byte_count = 50;
        let view = DownloadItemView::from_download(&d);
        assert_eq!(view.progress, Some(0.25));
    }
}
