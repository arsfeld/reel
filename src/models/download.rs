//! Data models for offline downloads.
//!
//! Downloads are deliberately self-contained: a [`Download`] row carries its own
//! snapshot of the media metadata (title, artwork path, hierarchy) so the
//! Downloads tab renders and plays with no source reachable, and so a download
//! survives independently of the live `media_items` library cache. There is no
//! foreign key to `media_items` for the same reason.
//!
//! The persisted state/error enums live here (the data layer); the pure
//! group-aggregate derivation built on top of them lives in
//! `crate::services::download`.

use crate::models::media::{MediaType, SourceType};

/// Lifecycle state of a single download item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum DownloadState {
    Queued,
    Downloading,
    Paused,
    Completed,
    Failed,
    Removed,
    Pruned,
}

#[allow(dead_code)]
impl DownloadState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Downloading => "downloading",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Removed => "removed",
            Self::Pruned => "pruned",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "queued" => Some(Self::Queued),
            "downloading" => Some(Self::Downloading),
            "paused" => Some(Self::Paused),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "removed" => Some(Self::Removed),
            "pruned" => Some(Self::Pruned),
            _ => None,
        }
    }
}

/// Why a download is in the `Failed` state. Each variant maps to a distinct
/// recovery posture in the queue runner (see `services::download`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FailReason {
    /// Transient connectivity failure — eligible for auto-retry with backoff.
    Network,
    /// Local disk is full — pauses the whole queue, no auto-retry.
    DiskFull,
    /// Source rejected auth (401) — parks the item pending re-auth.
    AuthExpired,
    /// The remote file changed under a resumed transfer — restart from zero.
    SourceFileChanged,
    /// A previously-completed local file is missing or mismatched on disk.
    FileMissing,
}

#[allow(dead_code)]
impl FailReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Network => "network",
            Self::DiskFull => "disk_full",
            Self::AuthExpired => "auth_expired",
            Self::SourceFileChanged => "source_file_changed",
            Self::FileMissing => "file_missing",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "network" => Some(Self::Network),
            "disk_full" => Some(Self::DiskFull),
            "auth_expired" => Some(Self::AuthExpired),
            "source_file_changed" => Some(Self::SourceFileChanged),
            "file_missing" => Some(Self::FileMissing),
            _ => None,
        }
    }
}

/// Scope of a grouped (TV) download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum GroupScope {
    Show,
    Season,
}

#[allow(dead_code)]
impl GroupScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Show => "show",
            Self::Season => "season",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "show" => Some(Self::Show),
            "season" => Some(Self::Season),
            _ => None,
        }
    }
}

/// Kind of deferred progress report queued while offline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SyncKind {
    Timeline,
    Scrobble,
}

#[allow(dead_code)]
impl SyncKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Timeline => "timeline",
            Self::Scrobble => "scrobble",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "timeline" => Some(Self::Timeline),
            "scrobble" => Some(Self::Scrobble),
            _ => None,
        }
    }
}

/// A single download item. Keyed by `media_item_id` (the library item's
/// composite id); self-contained via the snapshot fields so it renders offline
/// and survives a `media_items` rebuild.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct Download {
    pub media_item_id: String,
    /// Plex part key (e.g. `/library/parts/456/file.mkv`). The download URL is
    /// rebuilt from this plus the live token, so a token refresh never strands
    /// the transfer. Persisted instead of the tokenized URL.
    pub part_key: String,
    pub source_type: SourceType,
    pub source_id: String,
    pub state: DownloadState,
    pub fail_reason: Option<FailReason>,
    /// Advisory byte counter for UI only. The authoritative resume offset is the
    /// on-disk `.part` file length, never this value.
    pub byte_count: i64,
    /// Expected total size when known (often `None` until the first response).
    pub total_size: Option<i64>,
    /// `If-Range` validator captured on the first response (ETag or
    /// Last-Modified) used to detect a changed remote file on resume.
    pub validator: Option<String>,
    /// Local filesystem path of the downloaded media once known.
    pub file_path: Option<String>,
    /// Group this item belongs to, set only when enqueued as part of a
    /// show/season download. A separately-enqueued episode has `None` even if
    /// its parent show was downloaded as a group.
    pub group_id: Option<String>,
    pub queue_order: i64,
    pub enqueued_at: String,
    pub completed_at: Option<String>,
    // --- self-contained snapshot ---
    pub media_type: MediaType,
    pub title: String,
    pub year: Option<i32>,
    pub parent_id: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    /// Token-free artwork reference (Plex `path`, not a tokenized URL).
    pub poster_path: Option<String>,
}

/// A grouped (show/season) download. Members reference this via
/// `Download::group_id`.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct DownloadGroup {
    pub id: String,
    pub scope: GroupScope,
    /// Media id of the show (for `Show` scope) or season (for `Season` scope).
    pub parent_media_id: String,
    pub title: String,
    /// When the episode set was snapshotted (trigger time).
    pub snapshot_at: String,
}

/// A progress report recorded while the source was unreachable, queued for
/// flush back to the source on reconnect.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct PendingSync {
    /// `None` before insert; populated with the rowid after.
    pub id: Option<i64>,
    pub media_item_id: String,
    /// Source rating key used when reporting back (Plex ratingKey).
    pub rating_key: String,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub kind: SyncKind,
    pub offline_recorded_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_state_str_roundtrip() {
        for s in [
            DownloadState::Queued,
            DownloadState::Downloading,
            DownloadState::Paused,
            DownloadState::Completed,
            DownloadState::Failed,
            DownloadState::Removed,
            DownloadState::Pruned,
        ] {
            assert_eq!(DownloadState::from_db_str(s.as_str()), Some(s));
        }
        assert_eq!(DownloadState::from_db_str("bogus"), None);
    }

    #[test]
    fn fail_reason_str_roundtrip() {
        for r in [
            FailReason::Network,
            FailReason::DiskFull,
            FailReason::AuthExpired,
            FailReason::SourceFileChanged,
            FailReason::FileMissing,
        ] {
            assert_eq!(FailReason::from_db_str(r.as_str()), Some(r));
        }
        assert_eq!(FailReason::from_db_str("bogus"), None);
    }

    #[test]
    fn group_scope_str_roundtrip() {
        assert_eq!(
            GroupScope::from_db_str(GroupScope::Show.as_str()),
            Some(GroupScope::Show)
        );
        assert_eq!(
            GroupScope::from_db_str(GroupScope::Season.as_str()),
            Some(GroupScope::Season)
        );
        assert_eq!(GroupScope::from_db_str("bogus"), None);
    }

    #[test]
    fn sync_kind_str_roundtrip() {
        assert_eq!(
            SyncKind::from_db_str(SyncKind::Timeline.as_str()),
            Some(SyncKind::Timeline)
        );
        assert_eq!(
            SyncKind::from_db_str(SyncKind::Scrobble.as_str()),
            Some(SyncKind::Scrobble)
        );
        assert_eq!(SyncKind::from_db_str("bogus"), None);
    }
}
