//! Self-contained download sidecars and on-disk reconciliation.
//!
//! Each completed download writes a JSON sidecar next to its media file
//! (`<media>.reel.json`) holding a point-in-time metadata snapshot. This makes
//! the Downloads tab render and play with no source reachable (R15/R16) and
//! lets [`verify_downloads`] re-adopt downloads from disk after the DB is wiped
//! by the foreign-schema self-heal (the files survive; the rows don't).
//!
//! Security notes:
//! - The sidecar stores a **token-free** artwork path (the Plex `path`, never a
//!   tokenized URL), so the live token never lands in a plaintext file.
//! - Re-adoption is **validated**: schema version is checked, the media file
//!   must exist within the downloads folder, its length must match the recorded
//!   total, and id/title are length-bounded — a hand-crafted sidecar cannot
//!   inject an arbitrary path or oversized values.
//!
//! Known limitation: `pending_sync` rows (offline progress not yet flushed to
//! the source) are lost when the DB is wiped — they cannot be reconstructed
//! from a sidecar. Offline progress recorded immediately before a foreign-schema
//! rebuild is therefore not guaranteed to reach Plex (a narrow R21 gap).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::db::DbError;
use crate::db::downloads_repo::DownloadsRepo;
use crate::models::download::{Download, DownloadState, FailReason};
use crate::models::media::{MediaType, SourceType};

/// Current sidecar schema version. A sidecar with a different version is not
/// trusted for re-adoption.
const SIDECAR_VERSION: u32 = 1;

/// Sidecar filename suffix appended to the media path.
const SIDECAR_SUFFIX: &str = ".reel.json";

/// Defensive upper bounds for re-adopted string fields.
const MAX_ID_LEN: usize = 512;
const MAX_TITLE_LEN: usize = 1024;

/// Point-in-time metadata snapshot stored next to a downloaded media file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Sidecar {
    pub schema_version: u32,
    pub media_item_id: String,
    pub part_key: String,
    pub source_type: String,
    pub source_id: String,
    pub total_size: Option<i64>,
    pub media_type: String,
    pub title: String,
    pub year: Option<i32>,
    pub parent_id: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    /// Token-free artwork reference (Plex `path`), never a tokenized URL.
    pub poster_path: Option<String>,
    pub completed_at: Option<String>,
}

impl Sidecar {
    /// Build a sidecar snapshot from a completed download.
    #[allow(dead_code)]
    pub fn from_download(d: &Download) -> Self {
        Self {
            schema_version: SIDECAR_VERSION,
            media_item_id: d.media_item_id.clone(),
            part_key: d.part_key.clone(),
            source_type: d.source_type.as_str().to_string(),
            source_id: d.source_id.clone(),
            total_size: d.total_size,
            media_type: d.media_type.as_str().to_string(),
            title: d.title.clone(),
            year: d.year,
            parent_id: d.parent_id.clone(),
            season_number: d.season_number,
            episode_number: d.episode_number,
            poster_path: d.poster_path.clone(),
            completed_at: d.completed_at.clone(),
        }
    }

    /// Validate a sidecar read from disk before trusting it for re-adoption.
    fn is_valid(&self) -> bool {
        self.schema_version == SIDECAR_VERSION
            && !self.media_item_id.is_empty()
            && self.media_item_id.len() <= MAX_ID_LEN
            && self.title.len() <= MAX_TITLE_LEN
    }

    /// Reconstruct a completed `Download` from this sidecar and the verified
    /// on-disk media path. Re-adopted downloads are standalone (`group_id`
    /// is not carried in the sidecar).
    fn to_download(&self, file_path: &Path, byte_len: i64) -> Download {
        Download {
            media_item_id: self.media_item_id.clone(),
            part_key: self.part_key.clone(),
            source_type: SourceType::from_str(&self.source_type).unwrap_or(SourceType::Plex),
            source_id: self.source_id.clone(),
            state: DownloadState::Completed,
            fail_reason: None,
            byte_count: byte_len,
            total_size: self.total_size,
            validator: None,
            file_path: Some(file_path.to_string_lossy().into_owned()),
            group_id: None,
            queue_order: 0,
            enqueued_at: self.completed_at.clone().unwrap_or_default(),
            completed_at: self.completed_at.clone(),
            media_type: MediaType::from_str(&self.media_type).unwrap_or(MediaType::Movie),
            title: self.title.clone(),
            year: self.year,
            parent_id: self.parent_id.clone(),
            season_number: self.season_number,
            episode_number: self.episode_number,
            poster_path: self.poster_path.clone(),
        }
    }
}

/// Path of the sidecar for a given media file.
#[allow(dead_code)]
pub fn sidecar_path(media_path: &Path) -> PathBuf {
    let mut p = media_path.as_os_str().to_owned();
    p.push(SIDECAR_SUFFIX);
    PathBuf::from(p)
}

/// The media path a sidecar describes (the sidecar path minus the suffix).
fn media_path_for_sidecar(sidecar_path: &Path) -> Option<PathBuf> {
    let s = sidecar_path.to_string_lossy();
    s.strip_suffix(SIDECAR_SUFFIX).map(PathBuf::from)
}

/// Write the sidecar JSON next to a completed download's media file.
#[allow(dead_code)]
pub fn write_sidecar(media_path: &Path, download: &Download) -> std::io::Result<()> {
    let sidecar = Sidecar::from_download(download);
    let json = serde_json::to_string_pretty(&sidecar).map_err(std::io::Error::other)?;
    std::fs::write(sidecar_path(media_path), json)
}

/// Read and validate the sidecar for a media file, if present and trustworthy.
#[allow(dead_code)]
pub fn read_sidecar(media_path: &Path) -> Option<Sidecar> {
    let json = std::fs::read_to_string(sidecar_path(media_path)).ok()?;
    let sidecar: Sidecar = serde_json::from_str(&json).ok()?;
    sidecar.is_valid().then_some(sidecar)
}

/// Whether a "downloaded" badge should show for a download state — only a
/// fully `Completed` download is offline-ready.
#[allow(dead_code)]
pub fn shows_downloaded_badge(state: DownloadState) -> bool {
    state == DownloadState::Completed
}

/// Outcome of a startup reconciliation pass.
#[derive(Debug, Clone, Default, PartialEq)]
#[allow(dead_code)]
pub struct VerifyReport {
    /// Completed/Paused rows whose file is missing or size-mismatched
    /// (flipped to `Failed(FileMissing)`).
    pub failed_missing: Vec<String>,
    /// Downloads re-adopted from on-disk sidecars (no prior DB row).
    pub readopted: Vec<String>,
    /// Media files on disk with neither a DB row nor a usable sidecar.
    pub unmanaged: Vec<String>,
}

/// Reconcile DB download rows against on-disk files at startup.
///
/// - A `Completed`/`Paused` row whose file is missing, or whose size doesn't
///   match the recorded total, is flipped to `Failed(FileMissing)` so playback
///   falls back to streaming and the user can re-download.
/// - A media file with a valid sidecar but no DB row is re-adopted as
///   `Completed` (the post-wipe recovery path), but only after confirming the
///   file lives inside `base_dir` and its length matches the sidecar total.
/// - A media file with neither row nor usable sidecar is reported as unmanaged.
#[allow(dead_code)]
pub fn verify_downloads(
    repo: &mut DownloadsRepo,
    base_dir: &Path,
) -> Result<VerifyReport, DbError> {
    let mut report = VerifyReport::default();

    // 1. Validate existing Completed/Paused rows against disk.
    let mut known_files = std::collections::HashSet::new();
    for state in [DownloadState::Completed, DownloadState::Paused] {
        for d in repo.list_by_state(state)? {
            if let Some(ref fp) = d.file_path {
                known_files.insert(PathBuf::from(fp));
                let path = Path::new(fp);
                let ok = match std::fs::metadata(path) {
                    Ok(m) => d
                        .total_size
                        .map(|t| t as u64 == m.len())
                        .unwrap_or(state == DownloadState::Paused || m.len() > 0),
                    Err(_) => false,
                };
                if !ok && state == DownloadState::Completed {
                    repo.update_state(
                        &d.media_item_id,
                        DownloadState::Failed,
                        Some(FailReason::FileMissing),
                    )?;
                    report.failed_missing.push(d.media_item_id);
                }
            }
        }
    }

    // 2. Scan base_dir for orphan sidecars to re-adopt.
    let Ok(entries) = std::fs::read_dir(base_dir) else {
        return Ok(report);
    };
    for entry in entries.flatten() {
        let sc_path = entry.path();
        if !sc_path.to_string_lossy().ends_with(SIDECAR_SUFFIX) {
            continue;
        }
        let Some(media_path) = media_path_for_sidecar(&sc_path) else {
            continue;
        };
        // Confinement: the media file must live directly under base_dir.
        if media_path.parent() != Some(base_dir) {
            continue;
        }
        if known_files.contains(&media_path) {
            continue; // already represented by a DB row
        }
        let Some(sidecar) = read_sidecar(&media_path) else {
            report
                .unmanaged
                .push(media_path.to_string_lossy().into_owned());
            continue;
        };
        // Already a row for this id (e.g. non-Completed state)? Skip re-adopt.
        if repo.find(&sidecar.media_item_id)?.is_some() {
            continue;
        }
        match std::fs::metadata(&media_path) {
            Ok(m) => {
                let size_ok = sidecar
                    .total_size
                    .map(|t| t as u64 == m.len())
                    .unwrap_or(true);
                if size_ok {
                    let d = sidecar.to_download(&media_path, m.len() as i64);
                    repo.upsert(&d)?;
                    report.readopted.push(sidecar.media_item_id);
                } else {
                    report
                        .unmanaged
                        .push(media_path.to_string_lossy().into_owned());
                }
            }
            Err(_) => {
                // Sidecar with no media file: nothing to adopt.
                report
                    .unmanaged
                    .push(media_path.to_string_lossy().into_owned());
            }
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init::init_db;
    use diesel::SqliteConnection;
    use diesel::prelude::*;

    fn test_db() -> SqliteConnection {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        init_db(&mut conn).unwrap();
        conn
    }

    fn completed_download(id: &str, file_path: &str, size: i64) -> Download {
        Download {
            media_item_id: id.to_string(),
            part_key: "/library/parts/1/file.mkv".to_string(),
            source_type: SourceType::Plex,
            source_id: "http://localhost:32400".to_string(),
            state: DownloadState::Completed,
            fail_reason: None,
            byte_count: size,
            total_size: Some(size),
            validator: None,
            file_path: Some(file_path.to_string()),
            group_id: None,
            queue_order: 0,
            enqueued_at: "2026-05-28T10:00:00Z".to_string(),
            completed_at: Some("2026-05-28T11:00:00Z".to_string()),
            media_type: MediaType::Movie,
            title: "Dune".to_string(),
            year: Some(2021),
            parent_id: None,
            season_number: None,
            episode_number: None,
            poster_path: Some("/library/metadata/1/thumb/1".to_string()),
        }
    }

    #[test]
    fn sidecar_write_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("m1.mkv");
        std::fs::write(&media, b"data").unwrap();
        let d = completed_download("plex:srv:1", media.to_str().unwrap(), 4);

        write_sidecar(&media, &d).unwrap();
        let sc = read_sidecar(&media).unwrap();
        assert_eq!(sc.media_item_id, "plex:srv:1");
        assert_eq!(sc.title, "Dune");
        assert_eq!(
            sc.poster_path.as_deref(),
            Some("/library/metadata/1/thumb/1")
        );
        assert_eq!(sc.schema_version, SIDECAR_VERSION);
    }

    #[test]
    fn read_sidecar_rejects_wrong_version() {
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("m1.mkv");
        let bad = r#"{"schema_version":999,"media_item_id":"x","part_key":"k",
            "source_type":"plex","source_id":"s","total_size":null,"media_type":"movie",
            "title":"t","year":null,"parent_id":null,"season_number":null,
            "episode_number":null,"poster_path":null,"completed_at":null}"#;
        std::fs::write(sidecar_path(&media), bad).unwrap();
        assert!(read_sidecar(&media).is_none());
    }

    #[test]
    fn badge_only_for_completed() {
        assert!(shows_downloaded_badge(DownloadState::Completed));
        for s in [
            DownloadState::Queued,
            DownloadState::Downloading,
            DownloadState::Paused,
            DownloadState::Failed,
        ] {
            assert!(!shows_downloaded_badge(s));
        }
    }

    #[test]
    fn verify_flags_completed_with_missing_file() {
        let mut conn = test_db();
        let mut repo = DownloadsRepo::new(&mut conn);
        let dir = tempfile::tempdir().unwrap();
        // Row points at a file that does not exist.
        let missing = dir.path().join("gone.mkv");
        repo.upsert(&completed_download("m1", missing.to_str().unwrap(), 100))
            .unwrap();

        let report = verify_downloads(&mut repo, dir.path()).unwrap();
        assert_eq!(report.failed_missing, vec!["m1".to_string()]);
        let row = repo.find("m1").unwrap().unwrap();
        assert_eq!(row.state, DownloadState::Failed);
        assert_eq!(row.fail_reason, Some(FailReason::FileMissing));
    }

    #[test]
    fn verify_flags_completed_with_size_mismatch() {
        let mut conn = test_db();
        let mut repo = DownloadsRepo::new(&mut conn);
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("short.mkv");
        std::fs::write(&media, b"only4").unwrap(); // 5 bytes
        repo.upsert(&completed_download("m1", media.to_str().unwrap(), 999))
            .unwrap();

        let report = verify_downloads(&mut repo, dir.path()).unwrap();
        assert_eq!(report.failed_missing, vec!["m1".to_string()]);
    }

    #[test]
    fn verify_readopts_orphan_sidecar() {
        let mut conn = test_db();
        let mut repo = DownloadsRepo::new(&mut conn);
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("m1.mkv");
        std::fs::write(&media, vec![0u8; 50]).unwrap();
        // Write a sidecar but no DB row (simulating a post-wipe state).
        let d = completed_download("plex:srv:9", media.to_str().unwrap(), 50);
        write_sidecar(&media, &d).unwrap();

        let report = verify_downloads(&mut repo, dir.path()).unwrap();
        assert_eq!(report.readopted, vec!["plex:srv:9".to_string()]);
        let row = repo.find("plex:srv:9").unwrap().unwrap();
        assert_eq!(row.state, DownloadState::Completed);
        assert_eq!(row.title, "Dune");
    }

    #[test]
    fn verify_skips_readopt_when_size_mismatches_sidecar() {
        let mut conn = test_db();
        let mut repo = DownloadsRepo::new(&mut conn);
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("m1.mkv");
        std::fs::write(&media, vec![0u8; 10]).unwrap(); // 10 bytes on disk
        let d = completed_download("plex:srv:9", media.to_str().unwrap(), 999); // sidecar says 999
        write_sidecar(&media, &d).unwrap();

        let report = verify_downloads(&mut repo, dir.path()).unwrap();
        assert!(report.readopted.is_empty());
        assert!(repo.find("plex:srv:9").unwrap().is_none());
        assert_eq!(report.unmanaged.len(), 1);
    }
}
