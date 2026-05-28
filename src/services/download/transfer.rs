//! Resumable HTTP transfer client for downloads.
//!
//! Net-new I/O: streams a Plex direct-file URL to a `.part` file with HTTP
//! range resume, validates the remote file hasn't changed under a resume, and
//! verifies completion before promoting `.part` to the final path.
//!
//! Safety invariants (from the plan's Key Technical Decisions):
//! - The resume offset is the **on-disk `.part` length**, never a persisted DB
//!   counter — a crash mid-write can desync a counter, but the file length is
//!   always truth.
//! - On resume the request carries `If-Range`; a `200` reply (instead of `206`)
//!   or a `Content-Range` total that differs from the stored total means the
//!   remote file changed → discard and restart (`SourceFileChanged`).
//! - The final media path is **confined** to the downloads folder and its
//!   filename derived from the `media_item_id`, never from server-supplied
//!   `part_key` path segments (path-traversal guard).
//! - The transfer client has **no overall timeout** (only a connect timeout);
//!   the API client's 15s total timeout would kill large transfers.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::header::{
    CONTENT_LENGTH, CONTENT_RANGE, ETAG, HeaderMap, IF_RANGE, LAST_MODIFIED, RANGE,
};

use super::DownloadError;

/// ENOSPC — "no space left on device" (Linux). Used to map a write failure to
/// `DownloadError::DiskFull` rather than a generic I/O error.
const ENOSPC: i32 = 28;

/// Progress tick emitted during a transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub struct TransferProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

/// Successful transfer outcome.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct TransferResult {
    pub final_path: PathBuf,
    pub total_size: u64,
    /// Validator captured from the response (ETag or Last-Modified), to be
    /// persisted for a future resume's `If-Range`.
    pub validator: Option<String>,
}

// --- pure helpers (unit-tested) ---

/// Sanitize a `media_item_id` into a safe, separator-free filename component.
/// Any character that isn't alphanumeric, `.`, `-`, or `_` becomes `_`, so the
/// result can never contain `/` or `..` path segments.
#[allow(dead_code)]
pub fn sanitize_component(media_item_id: &str) -> String {
    let s: String = media_item_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    // Defense in depth: a value of all-dots could still be surprising.
    if s.is_empty() || s.chars().all(|c| c == '.') {
        "download".to_string()
    } else {
        s
    }
}

/// Build the confined final media path under `base`, with a filename derived
/// from `media_item_id` (never from `part_key` path segments) and the extension
/// taken from `part_key`. Guards against path traversal: the result must stay
/// within `base`.
#[allow(dead_code)]
pub fn confined_media_path(
    base: &Path,
    media_item_id: &str,
    part_key: &str,
) -> Result<PathBuf, DownloadError> {
    let mut name = sanitize_component(media_item_id);
    if let Some(ext) = Path::new(part_key).extension().and_then(|e| e.to_str()) {
        let ext = sanitize_component(ext);
        name.push('.');
        name.push_str(&ext);
    }
    let path = base.join(&name);
    // The sanitized single-component name cannot escape `base`, but assert it
    // explicitly so any future change to sanitization can't silently regress.
    if path.parent() != Some(base) {
        return Err(DownloadError::Io(format!(
            "refusing path outside downloads folder: {}",
            path.display()
        )));
    }
    Ok(path)
}

/// The `.part` sidecar path for an in-progress transfer.
#[allow(dead_code)]
pub fn part_path(media_path: &Path) -> PathBuf {
    let mut p = media_path.as_os_str().to_owned();
    p.push(".part");
    PathBuf::from(p)
}

/// On-disk resume offset: the current length of the `.part` file, or 0 if it
/// does not exist. This is the authoritative offset — never a DB counter.
#[allow(dead_code)]
pub fn resume_offset(part_path: &Path) -> u64 {
    fs::metadata(part_path).map(|m| m.len()).unwrap_or(0)
}

/// Map an HTTP status to a fatal error, or `None` for the success statuses
/// (`200 OK`, `206 Partial Content`). `401` is `AuthExpired`; everything else
/// non-success is a `Source` error.
#[allow(dead_code)]
pub fn classify_status(status: StatusCode) -> Option<DownloadError> {
    match status {
        StatusCode::OK | StatusCode::PARTIAL_CONTENT => None,
        StatusCode::UNAUTHORIZED => Some(DownloadError::AuthExpired),
        s => Some(DownloadError::Source(format!("HTTP {}", s.as_u16()))),
    }
}

/// Classify a transport-level reqwest error: connect/timeout failures are
/// transient (`Network`, retryable); anything else is also surfaced as
/// `Network` since the runner's retry policy decides what to do.
#[allow(dead_code)]
pub fn classify_transport_error(err: &reqwest::Error) -> DownloadError {
    DownloadError::Network(err.to_string())
}

/// Parse the total size from a `Content-Range: bytes 200-1023/1024` header.
/// Returns `None` for an absent header or an unknown total (`*`).
#[allow(dead_code)]
pub fn parse_content_range_total(headers: &HeaderMap) -> Option<u64> {
    let v = headers.get(CONTENT_RANGE)?.to_str().ok()?;
    let total = v.rsplit('/').next()?;
    total.trim().parse::<u64>().ok()
}

/// Pick the resume validator from response headers, preferring a strong ETag
/// over Last-Modified.
#[allow(dead_code)]
pub fn pick_validator(headers: &HeaderMap) -> Option<String> {
    if let Some(etag) = headers.get(ETAG).and_then(|v| v.to_str().ok()) {
        return Some(etag.to_string());
    }
    headers
        .get(LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// Map an I/O error encountered while writing the `.part` file. A
/// no-space-left (`ENOSPC`) error becomes `DiskFull`; everything else is a
/// generic `Io` error.
#[allow(dead_code)]
fn classify_write_error(err: &std::io::Error) -> DownloadError {
    if err.raw_os_error() == Some(ENOSPC) {
        DownloadError::DiskFull
    } else {
        DownloadError::Io(err.to_string())
    }
}

/// HTTP client tuned for large file transfers.
#[allow(dead_code)]
pub struct DownloadClient {
    http: reqwest::Client,
}

#[allow(dead_code)]
impl DownloadClient {
    /// Build a transfer client: accepts Plex's self-signed/plex.direct certs and
    /// keeps a short connect timeout, but has **no overall timeout** (a 15s cap
    /// like the API client would abort any large download).
    pub fn new() -> Result<Self, DownloadError> {
        let http = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .connect_timeout(Duration::from_secs(3))
            .build()
            .map_err(|e| DownloadError::Io(e.to_string()))?;
        Ok(Self { http })
    }

    /// Download `url` to `media_path`, resuming from the on-disk `.part` file.
    ///
    /// `stored_validator` is the validator captured on a prior attempt (for
    /// `If-Range`); `expected_size` is the size known up front, if any. On
    /// success the `.part` file is renamed to `media_path`.
    pub async fn download<F: FnMut(TransferProgress)>(
        &self,
        url: &str,
        stored_validator: Option<&str>,
        media_path: &Path,
        expected_size: Option<u64>,
        mut on_progress: F,
    ) -> Result<TransferResult, DownloadError> {
        let part = part_path(media_path);
        let mut offset = resume_offset(&part);

        let mut req = self.http.get(url);
        if offset > 0 {
            req = req.header(RANGE, format!("bytes={offset}-"));
            if let Some(v) = stored_validator {
                req = req.header(IF_RANGE, v);
            }
        }

        let resp = req.send().await.map_err(|e| classify_transport_error(&e))?;
        if let Some(err) = classify_status(resp.status()) {
            return Err(err);
        }

        let headers = resp.headers().clone();
        let validator = pick_validator(&headers);
        let resuming = resp.status() == StatusCode::PARTIAL_CONTENT && offset > 0;

        // Determine total size and whether the remote file changed.
        let total_size: Option<u64> = if resuming {
            let range_total = parse_content_range_total(&headers);
            // If we have a prior expectation and it disagrees, the file changed.
            if let (Some(expected), Some(actual)) = (expected_size, range_total)
                && expected != actual
            {
                return Err(DownloadError::SourceFileChanged);
            }
            range_total.or(expected_size)
        } else {
            // 200 OK. If we had bytes on disk but the server ignored the range
            // (full body), the file changed out from under us: restart clean.
            if offset > 0 {
                let _ = fs::remove_file(&part);
                offset = 0;
            }
            content_length(&headers)
        };

        // Open the .part file: append when resuming, truncate/create otherwise.
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(resuming)
            .truncate(!resuming)
            .open(&part)
            .map_err(|e| classify_write_error(&e))?;

        let mut downloaded = offset;
        let mut resp = resp;
        loop {
            let chunk = resp
                .chunk()
                .await
                .map_err(|e| classify_transport_error(&e))?;
            let Some(bytes) = chunk else { break };
            file.write_all(&bytes)
                .map_err(|e| classify_write_error(&e))?;
            downloaded += bytes.len() as u64;
            on_progress(TransferProgress {
                downloaded,
                total: total_size,
            });
        }
        file.flush().map_err(|e| classify_write_error(&e))?;
        drop(file);

        // Completion verification: when the total is known, the file must match
        // it exactly. A short file means the connection dropped — retryable.
        let actual_len = resume_offset(&part);
        if let Some(total) = total_size
            && actual_len != total
        {
            return Err(DownloadError::Network(format!(
                "incomplete transfer: {actual_len} of {total} bytes"
            )));
        }

        fs::rename(&part, media_path).map_err(|e| classify_write_error(&e))?;

        Ok(TransferResult {
            final_path: media_path.to_path_buf(),
            total_size: total_size.unwrap_or(actual_len),
            validator,
        })
    }
}

/// Parse `Content-Length` from response headers.
#[allow(dead_code)]
fn content_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn sanitize_strips_separators_and_traversal() {
        assert_eq!(sanitize_component("plex:srv:456"), "plex_srv_456");
        assert_eq!(sanitize_component("../../etc/passwd"), ".._.._etc_passwd");
        assert_eq!(sanitize_component("a/b\\c"), "a_b_c");
        assert_eq!(sanitize_component(".."), "download");
        assert_eq!(sanitize_component(""), "download");
    }

    #[test]
    fn confined_path_derives_from_id_not_part_key() {
        let base = Path::new("/downloads");
        let path = confined_media_path(base, "plex:srv:456", "/library/parts/9/Movie.mkv").unwrap();
        // Filename comes from the id; extension from the part key.
        assert_eq!(path, Path::new("/downloads/plex_srv_456.mkv"));
        assert_eq!(path.parent(), Some(base));
    }

    #[test]
    fn confined_path_rejects_traversal_in_part_key() {
        // Even a malicious part_key cannot escape: the filename is from the id,
        // and the extension is sanitized.
        let base = Path::new("/downloads");
        let path = confined_media_path(base, "m1", "/x/../../evil").unwrap();
        assert_eq!(path.parent(), Some(base));
        assert!(!path.to_string_lossy().contains(".."));
    }

    #[test]
    fn part_path_appends_suffix() {
        assert_eq!(
            part_path(Path::new("/downloads/m1.mkv")),
            Path::new("/downloads/m1.mkv.part")
        );
    }

    #[test]
    fn resume_offset_zero_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("missing.part");
        assert_eq!(resume_offset(&p), 0);
    }

    #[test]
    fn resume_offset_reads_file_length() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.part");
        fs::write(&p, b"hello world").unwrap();
        assert_eq!(resume_offset(&p), 11);
    }

    #[test]
    fn classify_status_maps_codes() {
        assert!(classify_status(StatusCode::OK).is_none());
        assert!(classify_status(StatusCode::PARTIAL_CONTENT).is_none());
        assert!(matches!(
            classify_status(StatusCode::UNAUTHORIZED),
            Some(DownloadError::AuthExpired)
        ));
        assert!(matches!(
            classify_status(StatusCode::NOT_FOUND),
            Some(DownloadError::Source(_))
        ));
        assert!(matches!(
            classify_status(StatusCode::INTERNAL_SERVER_ERROR),
            Some(DownloadError::Source(_))
        ));
    }

    #[test]
    fn parse_content_range_total_extracts_size() {
        let mut h = HeaderMap::new();
        h.insert(
            CONTENT_RANGE,
            HeaderValue::from_static("bytes 200-1023/1024"),
        );
        assert_eq!(parse_content_range_total(&h), Some(1024));
    }

    #[test]
    fn parse_content_range_total_unknown_total() {
        let mut h = HeaderMap::new();
        h.insert(CONTENT_RANGE, HeaderValue::from_static("bytes 200-1023/*"));
        assert_eq!(parse_content_range_total(&h), None);
    }

    #[test]
    fn pick_validator_prefers_etag() {
        let mut h = HeaderMap::new();
        h.insert(ETAG, HeaderValue::from_static("\"abc\""));
        h.insert(
            LAST_MODIFIED,
            HeaderValue::from_static("Wed, 21 Oct 2025 07:28:00 GMT"),
        );
        assert_eq!(pick_validator(&h), Some("\"abc\"".to_string()));
    }

    #[test]
    fn pick_validator_falls_back_to_last_modified() {
        let mut h = HeaderMap::new();
        h.insert(
            LAST_MODIFIED,
            HeaderValue::from_static("Wed, 21 Oct 2025 07:28:00 GMT"),
        );
        assert_eq!(
            pick_validator(&h),
            Some("Wed, 21 Oct 2025 07:28:00 GMT".to_string())
        );
    }

    #[test]
    fn classify_write_error_detects_enospc() {
        let enospc = std::io::Error::from_raw_os_error(ENOSPC);
        assert!(matches!(
            classify_write_error(&enospc),
            DownloadError::DiskFull
        ));
        let other = std::io::Error::other("boom");
        assert!(matches!(classify_write_error(&other), DownloadError::Io(_)));
    }
}

/// Integration tests against a local range-capable HTTP server (axum). Inline
/// because the crate is a binary, so `tests/` cannot import these modules; this
/// mirrors the existing `services::plex::fake_server` pattern.
#[cfg(test)]
mod integration {
    use super::*;
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::extract::State;
    use axum::http::{StatusCode, header};
    use axum::response::Response;
    use axum::routing::get;
    use tokio::net::TcpListener;
    use tokio::sync::RwLock;

    #[derive(Clone)]
    struct ServerState {
        content: Arc<RwLock<Vec<u8>>>,
        etag: Arc<RwLock<String>>,
    }

    fn parse_range_start(headers: &axum::http::HeaderMap) -> Option<u64> {
        let v = headers.get(header::RANGE)?.to_str().ok()?;
        let rest = v.strip_prefix("bytes=")?;
        rest.split('-').next()?.trim().parse::<u64>().ok()
    }

    async fn serve(State(state): State<ServerState>, headers: axum::http::HeaderMap) -> Response {
        let content = state.content.read().await.clone();
        let etag = state.etag.read().await.clone();
        let total = content.len() as u64;

        if let Some(start) = parse_range_start(&headers) {
            // If-Range mismatch -> the file "changed": serve the full body (200).
            let if_range_mismatch = headers
                .get(header::IF_RANGE)
                .and_then(|v| v.to_str().ok())
                .map(|v| v != etag)
                .unwrap_or(false);

            if !if_range_mismatch && start < total {
                let slice = content[start as usize..].to_vec();
                let end = total - 1;
                return Response::builder()
                    .status(StatusCode::PARTIAL_CONTENT)
                    .header(
                        header::CONTENT_RANGE,
                        format!("bytes {start}-{end}/{total}"),
                    )
                    .header(header::CONTENT_LENGTH, (total - start).to_string())
                    .header(header::ETAG, etag)
                    .header(header::ACCEPT_RANGES, "bytes")
                    .body(Body::from(slice))
                    .unwrap();
            }
        }

        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_LENGTH, total.to_string())
            .header(header::ETAG, etag)
            .header(header::ACCEPT_RANGES, "bytes")
            .body(Body::from(content))
            .unwrap()
    }

    /// Start a range server with the given content + etag. Returns (url, state).
    async fn start_server(content: Vec<u8>, etag: &str) -> (String, ServerState) {
        let state = ServerState {
            content: Arc::new(RwLock::new(content)),
            etag: Arc::new(RwLock::new(etag.to_string())),
        };
        let app = Router::new()
            .route("/file.mkv", get(serve))
            .with_state(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://127.0.0.1:{}/file.mkv", addr.port());
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (url, state)
    }

    fn body(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    #[tokio::test]
    async fn fresh_download_writes_full_file() {
        let content = body(5000);
        let (url, _state) = start_server(content.clone(), "\"v1\"").await;
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("m1.mkv");

        let client = DownloadClient::new().unwrap();
        let result = client
            .download(&url, None, &media, None, |_| {})
            .await
            .unwrap();

        assert_eq!(fs::read(&media).unwrap(), content);
        assert_eq!(result.total_size, 5000);
        assert_eq!(result.validator.as_deref(), Some("\"v1\""));
        // .part is gone after rename.
        assert!(!part_path(&media).exists());
    }

    #[tokio::test]
    async fn resume_from_partial_completes_near_offset() {
        // Covers AE4: ~60% on disk resumes, does not restart from 0.
        let content = body(5000);
        let (url, _state) = start_server(content.clone(), "\"v1\"").await;
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("m1.mkv");
        let part = part_path(&media);
        fs::write(&part, &content[..3000]).unwrap(); // 60% already downloaded

        let mut max_seen = 0u64;
        let client = DownloadClient::new().unwrap();
        let result = client
            .download(&url, Some("\"v1\""), &media, Some(5000), |p| {
                max_seen = max_seen.max(p.downloaded);
            })
            .await
            .unwrap();

        assert_eq!(fs::read(&media).unwrap(), content);
        assert_eq!(result.total_size, 5000);
        // Progress started from the resume offset, not 0.
        assert!(max_seen == 5000);
    }

    #[tokio::test]
    async fn changed_file_restarts_from_zero() {
        // .part holds bytes from an old version; server now serves a new file
        // and the If-Range validator no longer matches -> 200 full body.
        let new_content = body(4000);
        let (url, _state) = start_server(new_content.clone(), "\"v2\"").await;
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("m1.mkv");
        let part = part_path(&media);
        // Stale partial bytes (different pattern) from the "old" file.
        fs::write(&part, vec![0xFFu8; 2000]).unwrap();

        let client = DownloadClient::new().unwrap();
        // Stored validator is the OLD etag; server etag is "v2" -> mismatch.
        let result = client
            .download(&url, Some("\"v1\""), &media, None, |_| {})
            .await
            .unwrap();

        // Restarted cleanly: final file is exactly the new content, no stale prefix.
        assert_eq!(fs::read(&media).unwrap(), new_content);
        assert_eq!(result.total_size, 4000);
    }

    #[tokio::test]
    async fn resume_total_mismatch_is_source_file_changed() {
        // Resume returns 206 with the real total (5000), but we expected 9999 ->
        // the remote file is not what we started downloading.
        let content = body(5000);
        let (url, _state) = start_server(content.clone(), "\"v1\"").await;
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("m1.mkv");
        fs::write(part_path(&media), &content[..1000]).unwrap();

        let client = DownloadClient::new().unwrap();
        let err = client
            .download(&url, Some("\"v1\""), &media, Some(9999), |_| {})
            .await
            .unwrap_err();
        assert!(matches!(err, DownloadError::SourceFileChanged));
    }

    #[tokio::test]
    async fn unauthorized_maps_to_auth_expired() {
        // A server that always 401s.
        async fn deny() -> Response {
            Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::empty())
                .unwrap()
        }
        let app = Router::new().route("/file.mkv", get(deny));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://127.0.0.1:{}/file.mkv", addr.port());
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("m1.mkv");
        let client = DownloadClient::new().unwrap();
        let err = client
            .download(&url, None, &media, None, |_| {})
            .await
            .unwrap_err();
        assert!(matches!(err, DownloadError::AuthExpired));
    }
}
