//! Streaming-cache decision logic and filesystem helpers.
//!
//! GStreamer's `download` play flag routes a network stream through a
//! `downloadbuffer` element that progressively writes the file to a temp file
//! on disk (read-ahead margin + in-buffer seeks served locally). This module
//! holds the testable, GStreamer-free pieces: deciding *whether* to cache a
//! given URL, building the `downloadbuffer` temp-template, preparing the cache
//! directory, and reclaiming crash-orphaned temp files at startup.
//!
//! Architecture rule: the service layer is pure Rust — no `gstreamer`/`gtk`
//! imports live here. The pipeline wiring (U3) consumes these functions.

use std::path::Path;

/// True when `url` is a network stream that should be routed through the disk
/// cache. The only network-vs-local signal available today is the URL scheme:
/// `http(s)` is a network stream, everything else (`file://`, empty, bare
/// paths) plays directly. This is also the clean boundary against Offline
/// Downloads — a local copy or offline redirect arrives as `file://` and
/// bypasses the cache.
pub fn should_cache_stream(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// Build the `downloadbuffer` `temp-template` for a cache directory: an
/// mkstemp-style template (`<dir>/reel-XXXXXX`). The trailing `XXXXXX` is
/// replaced by `downloadbuffer` with random characters when it opens the file.
pub fn temp_template(dir: &Path) -> String {
    dir.join("reel-XXXXXX").to_string_lossy().into_owned()
}

/// Ensure the cache directory exists. Called at startup and defensively before
/// pipeline setup so the directory is present before `downloadbuffer` opens
/// its temp file.
pub fn prepare(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

/// Reclaim crash-orphaned temp files: remove everything under the cache
/// directory, then recreate the (empty) directory. Tolerates a missing dir.
///
/// Nothing in the stream-cache dir is ever meant to survive a session, so a
/// blunt wipe is correct and simplest. A clean stop already removes its own
/// temp file via `downloadbuffer`'s `temp-remove=true`; this sweep reclaims
/// what a crash/SIGKILL left behind. Modeled on `ArtworkCache::clear()`.
pub fn reclaim(dir: &Path) {
    if dir.exists()
        && let Err(e) = std::fs::remove_dir_all(dir)
    {
        tracing::warn!("stream-cache reclaim failed to wipe {}: {e}", dir.display());
    }
    if let Err(e) = std::fs::create_dir_all(dir) {
        tracing::warn!(
            "stream-cache reclaim failed to recreate {}: {e}",
            dir.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_cache_stream_returns_true_for_http_and_https() {
        assert!(should_cache_stream("http://host/library/parts/1/file.mkv"));
        assert!(should_cache_stream("https://host/library/parts/1/file.mkv"));
    }

    #[test]
    fn should_cache_stream_returns_false_for_file_url() {
        assert!(!should_cache_stream("file:///home/user/video.mkv"));
    }

    #[test]
    fn should_cache_stream_returns_false_for_empty_or_unknown_scheme() {
        assert!(!should_cache_stream(""));
        assert!(!should_cache_stream("/home/user/video.mkv"));
        assert!(!should_cache_stream("smb://host/share/video.mkv"));
    }

    #[test]
    fn temp_template_contains_dir_and_mkstemp_placeholder() {
        let dir = Path::new("/tmp/reel-test/stream-cache");
        let tmpl = temp_template(dir);
        assert!(tmpl.starts_with("/tmp/reel-test/stream-cache"));
        assert!(tmpl.ends_with("reel-XXXXXX"));
    }

    #[test]
    fn prepare_creates_dir() {
        let base = tempfile::tempdir().unwrap();
        let nested = base.path().join("a/b/stream-cache");
        assert!(!nested.exists());
        prepare(&nested).unwrap();
        assert!(nested.is_dir());
    }

    #[test]
    fn reclaim_removes_orphaned_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("stream-cache");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(cache.join("reel-AbC123"), b"orphan").unwrap();
        std::fs::write(cache.join("reel-XyZ789"), b"orphan").unwrap();

        reclaim(&cache);

        assert!(cache.is_dir());
        assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 0);
    }

    #[test]
    fn reclaim_tolerates_missing_dir() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("never-created/stream-cache");
        reclaim(&missing);
        assert!(missing.is_dir());
    }
}
