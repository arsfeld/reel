//! Offline-download service logic.
//!
//! The persisted data enums (`DownloadState`, `FailReason`, …) live in
//! `crate::models::download`; this module adds the pure, GTK/IO-free logic
//! built on top of them — group-status derivation here, the queue scheduler in
//! [`queue`], snapshot/top-up in [`snapshot`], and prune selection in [`prune`].

use crate::models::download::DownloadState;

pub use crate::models::download::FailReason;

/// Runtime error returned by the transfer client and queue runner. Maps onto a
/// persisted [`FailReason`] so the queue can record why an item failed.
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum DownloadError {
    /// Transient connectivity failure (connect/timeout) — retryable.
    #[error("network error: {0}")]
    Network(String),
    /// Local disk is full.
    #[error("no space left on device")]
    DiskFull,
    /// Source rejected auth (e.g. 401).
    #[error("authentication expired")]
    AuthExpired,
    /// The remote file changed under a resumed transfer.
    #[error("source file changed during resume")]
    SourceFileChanged,
    /// A completed local file is missing or size-mismatched on disk.
    #[error("local file missing or incomplete")]
    FileMissing,
    /// Other I/O failure while writing the download.
    #[error("io error: {0}")]
    Io(String),
    /// Non-connectivity source/HTTP error (e.g. 404, 500).
    #[error("source error: {0}")]
    Source(String),
}

#[allow(dead_code)]
impl DownloadError {
    /// The persisted failure reason this error maps to. Non-connectivity source
    /// and I/O errors map to `Network` (retryable) by default; the queue runner
    /// applies its own retry policy on top.
    pub fn fail_reason(&self) -> FailReason {
        match self {
            Self::Network(_) | Self::Source(_) | Self::Io(_) => FailReason::Network,
            Self::DiskFull => FailReason::DiskFull,
            Self::AuthExpired => FailReason::AuthExpired,
            Self::SourceFileChanged => FailReason::SourceFileChanged,
            Self::FileMissing => FailReason::FileMissing,
        }
    }
}

/// Aggregate status of a grouped (show/season) download, derived purely from
/// its members' states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum GroupStatus {
    /// At least one member is still queued, downloading, or paused.
    Downloading,
    /// All members are terminal, with a mix of completed and failed.
    PartiallyFailed,
    /// All members are terminal and every one failed.
    Failed,
    /// All members completed (or the group has no outstanding members).
    Complete,
}

/// Whether a state counts toward the group's outstanding set. `Removed` and
/// `Pruned` members have left the group and are ignored by the aggregate.
fn is_outstanding(state: DownloadState) -> bool {
    !matches!(state, DownloadState::Removed | DownloadState::Pruned)
}

/// Derive the aggregate group status from member states.
///
/// Precedence: `Downloading` (any active) > `PartiallyFailed` (some failed,
/// some complete) > `Failed` (all failed) > `Complete`. `Removed`/`Pruned`
/// members are excluded; an all-removed group reports `Complete` (no work
/// outstanding).
#[allow(dead_code)]
pub fn derive_group_status(children: &[DownloadState]) -> GroupStatus {
    let outstanding: Vec<DownloadState> = children
        .iter()
        .copied()
        .filter(|s| is_outstanding(*s))
        .collect();

    let any_active = outstanding.iter().any(|s| {
        matches!(
            s,
            DownloadState::Queued | DownloadState::Downloading | DownloadState::Paused
        )
    });
    if any_active {
        return GroupStatus::Downloading;
    }

    let failed = outstanding
        .iter()
        .filter(|s| matches!(s, DownloadState::Failed))
        .count();
    let completed = outstanding
        .iter()
        .filter(|s| matches!(s, DownloadState::Completed))
        .count();

    match (failed, completed) {
        (f, c) if f > 0 && c > 0 => GroupStatus::PartiallyFailed,
        (f, 0) if f > 0 => GroupStatus::Failed,
        _ => GroupStatus::Complete,
    }
}

/// `(completed, total)` progress for a group, e.g. `(6, 10)` for "6/10
/// episodes". `total` counts all outstanding members; `Removed`/`Pruned` are
/// excluded.
#[allow(dead_code)]
pub fn group_progress(children: &[DownloadState]) -> (usize, usize) {
    let total = children.iter().filter(|s| is_outstanding(**s)).count();
    let completed = children
        .iter()
        .filter(|s| matches!(s, DownloadState::Completed))
        .count();
    (completed, total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::download::DownloadState::*;

    #[test]
    fn active_member_makes_group_downloading() {
        // 8 complete + 1 downloading + 1 failed -> Downloading (active wins).
        let states = [
            Completed,
            Completed,
            Completed,
            Completed,
            Completed,
            Completed,
            Completed,
            Completed,
            Downloading,
            Failed,
        ];
        assert_eq!(derive_group_status(&states), GroupStatus::Downloading);
    }

    #[test]
    fn queued_member_is_active() {
        assert_eq!(
            derive_group_status(&[Completed, Queued]),
            GroupStatus::Downloading
        );
    }

    #[test]
    fn mix_of_failed_and_complete_is_partially_failed() {
        let states = [
            Completed, Completed, Completed, Completed, Completed, Completed, Completed, Completed,
            Failed, Failed,
        ];
        assert_eq!(derive_group_status(&states), GroupStatus::PartiallyFailed);
    }

    #[test]
    fn all_failed_is_failed() {
        assert_eq!(
            derive_group_status(&[Failed, Failed, Failed]),
            GroupStatus::Failed
        );
    }

    #[test]
    fn all_complete_is_complete() {
        assert_eq!(
            derive_group_status(&[Completed, Completed]),
            GroupStatus::Complete
        );
    }

    #[test]
    fn removed_and_pruned_members_are_ignored() {
        // Only a completed and a removed/pruned member -> Complete.
        assert_eq!(
            derive_group_status(&[Completed, Removed, Pruned]),
            GroupStatus::Complete
        );
        // A pruned member alongside an active one still reports Downloading.
        assert_eq!(
            derive_group_status(&[Downloading, Pruned]),
            GroupStatus::Downloading
        );
    }

    #[test]
    fn group_progress_counts_completed_over_outstanding() {
        // 6 complete + 4 queued -> (6, 10).
        let states = [
            Completed, Completed, Completed, Completed, Completed, Completed, Queued, Queued,
            Queued, Queued,
        ];
        assert_eq!(group_progress(&states), (6, 10));
    }

    #[test]
    fn group_progress_excludes_removed() {
        // 2 complete + 1 removed -> total counts only the 2 outstanding.
        assert_eq!(group_progress(&[Completed, Completed, Removed]), (2, 2));
    }

    #[test]
    fn fail_reason_mapping() {
        assert_eq!(DownloadError::DiskFull.fail_reason(), FailReason::DiskFull);
        assert_eq!(
            DownloadError::AuthExpired.fail_reason(),
            FailReason::AuthExpired
        );
        assert_eq!(
            DownloadError::SourceFileChanged.fail_reason(),
            FailReason::SourceFileChanged
        );
        assert_eq!(
            DownloadError::FileMissing.fail_reason(),
            FailReason::FileMissing
        );
        assert_eq!(
            DownloadError::Network("x".into()).fail_reason(),
            FailReason::Network
        );
        assert_eq!(
            DownloadError::Source("404".into()).fail_reason(),
            FailReason::Network
        );
    }
}
