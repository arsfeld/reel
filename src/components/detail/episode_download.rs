//! Pure derivation of per-episode download controls for the Show detail page.
//!
//! The Show detail episode cards surface a download trigger, a state badge, and
//! a delete affordance driven entirely by the episode's current [`Download`]
//! row (if any) plus whether the live item is downloadable. Keeping the
//! state→control mapping and the trigger→intent mapping as pure functions here
//! lets the GTK `update()` stay a thin dispatcher and makes the logic fully
//! unit-testable without a display (mirrors `components::downloads::row`).

use crate::models::download::{Download, DownloadState};

/// What a single episode card should render for its download affordance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpisodeDownloadControl {
    /// The episode has no playable file and no download row — nothing to offer.
    NotDownloadable,
    /// Downloadable but not yet downloaded: offer the download trigger.
    Downloadable,
    /// Queued behind other transfers.
    Queued,
    /// Actively transferring.
    Downloading,
    /// Paused mid-transfer.
    Paused,
    /// The transfer failed: offer a tappable retry.
    Failed,
    /// A completed local file exists: offer delete.
    Downloaded,
}

/// What tapping the download trigger on an episode should do, decided from its
/// persisted row. The Show detail UI only ever fires this for the `Downloadable`
/// and `Failed` controls; `NoOp` is a safety net for the other states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadIntent {
    /// No usable row exists — enqueue a fresh download.
    Enqueue,
    /// A failed row exists — resume/retry it (preserving any `.part`).
    Retry,
    /// Already queued/downloading/completed — do nothing.
    NoOp,
}

/// Derive the card control from the episode's download row and whether the live
/// item exposes a playable file. `Removed`/`Pruned` rows have left the view, so
/// they read as `Downloadable` again (the user can re-download).
pub fn derive_episode_control(
    existing: Option<&Download>,
    has_file_path: bool,
) -> EpisodeDownloadControl {
    match existing.map(|d| d.state) {
        Some(DownloadState::Completed) => EpisodeDownloadControl::Downloaded,
        Some(DownloadState::Downloading) => EpisodeDownloadControl::Downloading,
        Some(DownloadState::Queued) => EpisodeDownloadControl::Queued,
        Some(DownloadState::Paused) => EpisodeDownloadControl::Paused,
        Some(DownloadState::Failed) => EpisodeDownloadControl::Failed,
        // No row, or a terminal-removed row: downloadable iff the item has a file.
        Some(DownloadState::Removed | DownloadState::Pruned) | None => {
            if has_file_path {
                EpisodeDownloadControl::Downloadable
            } else {
                EpisodeDownloadControl::NotDownloadable
            }
        }
    }
}

/// Decide enqueue-vs-retry-vs-no-op for a download trigger tap.
pub fn episode_download_intent(existing: Option<&Download>) -> DownloadIntent {
    match existing.map(|d| d.state) {
        None | Some(DownloadState::Removed | DownloadState::Pruned) => DownloadIntent::Enqueue,
        Some(DownloadState::Failed) => DownloadIntent::Retry,
        // Queued / Downloading / Paused / Completed: a badge is shown, not a
        // trigger, so a tap here is a no-op.
        Some(_) => DownloadIntent::NoOp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::media::{MediaType, SourceType};

    fn download(state: DownloadState) -> Download {
        Download {
            media_item_id: "ep1".into(),
            part_key: "/p".into(),
            source_type: SourceType::Plex,
            source_id: "s".into(),
            state,
            fail_reason: None,
            byte_count: 0,
            total_size: None,
            validator: None,
            file_path: None,
            group_id: Some("group:show1".into()),
            queue_order: 0,
            enqueued_at: "2026-05-29T00:00:00Z".into(),
            completed_at: None,
            media_type: MediaType::Episode,
            title: "Pilot".into(),
            year: None,
            parent_id: Some("season1".into()),
            season_number: Some(1),
            episode_number: Some(1),
            poster_path: None,
        }
    }

    #[test]
    fn derive_control_none_with_file_path_is_downloadable() {
        assert_eq!(
            derive_episode_control(None, true),
            EpisodeDownloadControl::Downloadable
        );
    }

    #[test]
    fn derive_control_none_without_file_path_is_not_downloadable() {
        assert_eq!(
            derive_episode_control(None, false),
            EpisodeDownloadControl::NotDownloadable
        );
    }

    #[test]
    fn derive_control_completed_is_downloaded() {
        assert_eq!(
            derive_episode_control(Some(&download(DownloadState::Completed)), true),
            EpisodeDownloadControl::Downloaded
        );
    }

    #[test]
    fn derive_control_in_progress_states_map_distinctly() {
        assert_eq!(
            derive_episode_control(Some(&download(DownloadState::Queued)), true),
            EpisodeDownloadControl::Queued
        );
        assert_eq!(
            derive_episode_control(Some(&download(DownloadState::Downloading)), true),
            EpisodeDownloadControl::Downloading
        );
        assert_eq!(
            derive_episode_control(Some(&download(DownloadState::Paused)), true),
            EpisodeDownloadControl::Paused
        );
    }

    #[test]
    fn derive_control_failed_is_failed() {
        assert_eq!(
            derive_episode_control(Some(&download(DownloadState::Failed)), true),
            EpisodeDownloadControl::Failed
        );
    }

    #[test]
    fn derive_control_pruned_row_is_downloadable_again() {
        // A pruned download has left the cache; offer to re-download it.
        assert_eq!(
            derive_episode_control(Some(&download(DownloadState::Pruned)), true),
            EpisodeDownloadControl::Downloadable
        );
    }

    #[test]
    fn intent_none_is_enqueue() {
        assert_eq!(episode_download_intent(None), DownloadIntent::Enqueue);
    }

    #[test]
    fn intent_failed_is_retry() {
        assert_eq!(
            episode_download_intent(Some(&download(DownloadState::Failed))),
            DownloadIntent::Retry
        );
    }

    #[test]
    fn intent_pruned_is_enqueue() {
        assert_eq!(
            episode_download_intent(Some(&download(DownloadState::Pruned))),
            DownloadIntent::Enqueue
        );
    }

    #[test]
    fn intent_completed_and_in_progress_are_noop() {
        for s in [
            DownloadState::Completed,
            DownloadState::Queued,
            DownloadState::Downloading,
            DownloadState::Paused,
        ] {
            assert_eq!(
                episode_download_intent(Some(&download(s))),
                DownloadIntent::NoOp
            );
        }
    }
}
