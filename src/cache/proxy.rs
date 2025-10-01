use anyhow::{Context, Result};
use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, head},
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::metadata::MediaCacheKey;
use super::state_machine::{CacheStateMachine, DownloadState};
use super::stats::ProxyStats;
use super::storage::CacheStorage;
use crate::models::{MediaItemId, SourceId};

/// Cache proxy server that serves cached files over HTTP
pub struct CacheProxy {
    port: u16,
    storage: Arc<RwLock<CacheStorage>>,
    state_machine: Arc<CacheStateMachine>,
    active_streams: Arc<RwLock<HashMap<String, MediaCacheKey>>>,
    stats: ProxyStats,
    enable_stats: bool,
    stats_interval_secs: u64,
}

impl CacheProxy {
    /// Create a new cache proxy server
    pub fn new(storage: Arc<RwLock<CacheStorage>>, state_machine: Arc<CacheStateMachine>) -> Self {
        // Find an available port
        let port = Self::find_available_port();

        Self {
            port,
            storage,
            state_machine,
            active_streams: Arc::new(RwLock::new(HashMap::new())),
            stats: ProxyStats::new(),
            enable_stats: true,
            stats_interval_secs: 30,
        }
    }

    /// Create with custom config
    pub fn with_config(
        storage: Arc<RwLock<CacheStorage>>,
        state_machine: Arc<CacheStateMachine>,
        enable_stats: bool,
        stats_interval_secs: u64,
    ) -> Self {
        let port = Self::find_available_port();

        Self {
            port,
            storage,
            state_machine,
            active_streams: Arc::new(RwLock::new(HashMap::new())),
            stats: ProxyStats::new(),
            enable_stats,
            stats_interval_secs,
        }
    }

    /// Find an available port for the proxy server
    fn find_available_port() -> u16 {
        // Try to bind to a random port in the 50000-60000 range
        for port in 50000..60000 {
            if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
                return port;
            }
        }
        // Fallback to any available port
        0
    }

    /// Start the proxy server
    pub async fn start(self: Arc<Self>) -> Result<()> {
        let addr = format!("127.0.0.1:{}", self.port);
        info!("Starting cache proxy server on {}", addr);

        // Start periodic stats reporting if enabled
        if self.enable_stats {
            self.start_stats_reporting();
        }

        let app = self.create_router();

        let listener = TcpListener::bind(&addr)
            .await
            .context("Failed to bind proxy server")?;

        // Get the actual port if we used 0
        let actual_addr = listener.local_addr()?;
        info!("Cache proxy server listening on {}", actual_addr);

        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                error!("Proxy server error: {}", e);
            }
        });

        Ok(())
    }

    /// Start periodic stats reporting
    fn start_stats_reporting(self: &Arc<Self>) {
        let stats = self.stats.clone();
        let active_streams = self.active_streams.clone();
        let interval_secs = self.stats_interval_secs;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
            ticker.tick().await; // Skip first immediate tick

            loop {
                ticker.tick().await;

                // Update active streams count
                let streams = active_streams.read().await;
                stats.set_active_streams(streams.len() as u64);

                let report = stats.format_report();
                info!("{}", report);
            }
        });
    }

    /// Create the router for the proxy server
    fn create_router(self: &Arc<Self>) -> Router {
        Router::new()
            .route(
                "/cache/:source_id/:media_id/:quality",
                get(Self::serve_cached_file).head(Self::serve_cached_file_head),
            )
            .route(
                "/stream/:stream_id",
                get(Self::serve_stream).head(Self::serve_stream_head),
            )
            .with_state(self.clone())
    }

    /// Register a stream and return its proxy URL
    pub async fn register_stream(&self, cache_key: MediaCacheKey) -> String {
        let stream_id = uuid::Uuid::new_v4().to_string();

        let mut streams = self.active_streams.write().await;
        streams.insert(stream_id.clone(), cache_key);

        format!("http://127.0.0.1:{}/stream/{}", self.port, stream_id)
    }

    /// Serve a cached file directly
    async fn serve_cached_file(
        Path((source_id, media_id, quality)): Path<(String, String, String)>,
        State(proxy): State<Arc<CacheProxy>>,
        headers: HeaderMap,
    ) -> impl IntoResponse {
        let cache_key = MediaCacheKey::new(
            SourceId::from(source_id),
            MediaItemId::from(media_id),
            quality,
        );

        proxy.serve_file(&cache_key, headers).await
    }

    /// Serve a registered stream
    async fn serve_stream(
        Path(stream_id): Path<String>,
        State(proxy): State<Arc<CacheProxy>>,
        headers: HeaderMap,
    ) -> impl IntoResponse {
        let cache_key = {
            let streams = proxy.active_streams.read().await;
            match streams.get(&stream_id) {
                Some(key) => key.clone(),
                None => {
                    error!("Stream not found: {}", stream_id);
                    return StatusCode::NOT_FOUND.into_response();
                }
            }
        };

        proxy.serve_file(&cache_key, headers).await
    }

    /// Handle HEAD request for cached file
    async fn serve_cached_file_head(
        Path((source_id, media_id, quality)): Path<(String, String, String)>,
        State(proxy): State<Arc<CacheProxy>>,
        headers: HeaderMap,
    ) -> impl IntoResponse {
        let cache_key = MediaCacheKey::new(
            SourceId::from(source_id),
            MediaItemId::from(media_id),
            quality,
        );

        proxy.serve_file_head(&cache_key, headers).await
    }

    /// Handle HEAD request for stream
    async fn serve_stream_head(
        Path(stream_id): Path<String>,
        State(proxy): State<Arc<CacheProxy>>,
        headers: HeaderMap,
    ) -> impl IntoResponse {
        let cache_key = {
            let streams = proxy.active_streams.read().await;
            match streams.get(&stream_id) {
                Some(key) => key.clone(),
                None => {
                    error!("Stream not found: {}", stream_id);
                    return StatusCode::NOT_FOUND.into_response();
                }
            }
        };

        proxy.serve_file_head(&cache_key, headers).await
    }

    /// Serve a file from cache (supports range requests)
    async fn serve_file(&self, cache_key: &MediaCacheKey, headers: HeaderMap) -> Response {
        // Debug log all headers for troubleshooting
        debug!("Proxy: GET request headers: {:?}", headers);

        // Increment request stats
        self.stats.increment_request();

        // Get cache entry
        let entry = {
            let mut storage = self.storage.write().await;
            storage.get_entry(cache_key)
        };

        let entry = match entry {
            Some(e) => {
                self.stats.increment_cache_hit();
                e
            }
            None => {
                self.stats.increment_cache_miss();
                error!("Cache entry not found for key: {:?}", cache_key);
                return StatusCode::NOT_FOUND.into_response();
            }
        };

        // Check if file exists
        if !entry.exists() {
            error!("Cache file not found: {:?}", entry.file_path);
            return StatusCode::NOT_FOUND.into_response();
        }

        // Check download state from state machine
        let state_info = self.state_machine.get_state(cache_key).await;
        let download_state = state_info.as_ref().map(|info| &info.state);

        // Get the actual file size on disk (what's been downloaded so far)
        let mut actual_file_size = match entry.file_size() {
            Ok(size) => size,
            Err(e) => {
                error!("Failed to get file size: {}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        debug!(
            "Proxy: Cache key {:?}, state: {:?}, file size: {} bytes",
            cache_key, download_state, actual_file_size
        );

        // Use the expected total size from metadata if available, otherwise use actual size
        // This is important for proper Content-Range headers
        let total_size = if entry.metadata.expected_total_size > 0 {
            // Use the expected total size from the server
            entry.metadata.expected_total_size
        } else if entry.metadata.is_complete && actual_file_size > 0 {
            // File is complete, use actual size
            actual_file_size
        } else {
            // No expected size known, use actual size
            actual_file_size
        };

        // Parse range header if present
        let range = if let Some(range_header) = headers.get(header::RANGE) {
            if let Ok(range_str) = range_header.to_str() {
                info!(
                    "Proxy: *** RANGE REQUEST *** Processing Range header: {}",
                    range_str
                );
                let parsed = parse_range_header(range_str, total_size);
                if let Some((start, end)) = parsed {
                    info!(
                        "Proxy: *** RANGE REQUEST *** Parsed range: {}-{} (total_size: {}, actual_file_size: {})",
                        start, end, total_size, actual_file_size
                    );
                } else {
                    warn!(
                        "Proxy: *** RANGE REQUEST *** Failed to parse range header: {}",
                        range_str
                    );
                }
                parsed
            } else {
                None
            }
        } else {
            None
        };

        // Track range vs full requests
        if range.is_some() {
            self.stats.increment_range_request();
        } else {
            self.stats.increment_full_request();
        }

        // Open the file
        let mut file = match File::open(&entry.file_path).await {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to open cache file: {}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        // Check if download is in progress and wait for initial data if needed
        match download_state {
            Some(DownloadState::Failed(msg)) => {
                error!("Cache download failed: {}", msg);
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
            Some(DownloadState::NotStarted) => {
                warn!("Download not started for cache key: {:?}", cache_key);
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
            Some(DownloadState::Initializing) => {
                // Download is initializing (making HTTP request, getting headers)
                // Wait longer for the first data to arrive
                info!("Download in Initializing state, waiting for initial data...");

                let initial_wait_start = std::time::Instant::now();
                let max_initial_wait = std::time::Duration::from_secs(30); // 30 seconds for initialization
                let mut check_interval = std::time::Duration::from_millis(100);
                let max_interval = std::time::Duration::from_secs(2);

                loop {
                    // Check if we've transitioned to downloading or have data
                    if let Some(info) = self.state_machine.get_state(cache_key).await {
                        match &info.state {
                            DownloadState::Failed(msg) => {
                                error!("Download failed during initialization: {}", msg);
                                return StatusCode::SERVICE_UNAVAILABLE.into_response();
                            }
                            DownloadState::Downloading | DownloadState::Complete => {
                                // Transitioned to downloading, data should be available soon
                                debug!("Transitioned to {:?} state", info.state);
                                if info.downloaded_bytes > 0 {
                                    actual_file_size = info.downloaded_bytes;
                                    let wait_time = initial_wait_start.elapsed();
                                    info!(
                                        "Initial data available: {} bytes after {:?}",
                                        actual_file_size, wait_time
                                    );
                                    self.stats.record_initial_wait(wait_time.as_millis() as u64);
                                    break;
                                }
                            }
                            DownloadState::Initializing => {
                                // Still initializing, check file size
                                if let Ok(size) = entry.file_size() {
                                    if size > 0 {
                                        actual_file_size = size;
                                        let wait_time = initial_wait_start.elapsed();
                                        info!(
                                            "Initial data written to disk: {} bytes after {:?}",
                                            size, wait_time
                                        );
                                        self.stats
                                            .record_initial_wait(wait_time.as_millis() as u64);
                                        break;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    // Check if we've exceeded the timeout
                    if initial_wait_start.elapsed() >= max_initial_wait {
                        warn!(
                            "Timeout waiting for initial data after {:?}",
                            initial_wait_start.elapsed()
                        );
                        self.stats.increment_initial_timeout();
                        return Response::builder()
                            .status(StatusCode::SERVICE_UNAVAILABLE)
                            .header("Retry-After", "5")
                            .body(vec![].into())
                            .unwrap();
                    }

                    // Progressive backoff
                    tokio::time::sleep(check_interval).await;
                    check_interval = (check_interval * 2).min(max_interval);
                }
            }
            Some(DownloadState::Downloading) | Some(DownloadState::Paused) => {
                // Download in progress or paused, wait for minimum data if needed
                if actual_file_size == 0 {
                    // Wait for minimum data to be available (shorter wait since already downloading)
                    let has_data = self
                        .state_machine
                        .wait_for_data(cache_key, std::time::Duration::from_secs(10))
                        .await
                        .unwrap_or(false);

                    if !has_data {
                        warn!("No data available after waiting in Downloading state");
                        return Response::builder()
                            .status(StatusCode::SERVICE_UNAVAILABLE)
                            .header("Retry-After", "2")
                            .body(vec![].into())
                            .unwrap();
                    }

                    // Re-check file size
                    match entry.file_size() {
                        Ok(size) if size > 0 => {
                            actual_file_size = size;
                            debug!("Data now available: {} bytes", size);
                        }
                        _ => {
                            return Response::builder()
                                .status(StatusCode::SERVICE_UNAVAILABLE)
                                .header("Retry-After", "1")
                                .body(vec![].into())
                                .unwrap();
                        }
                    }
                }
            }
            _ => {} // Complete or no state info, proceed normally
        }

        match range {
            Some((start, end)) => {
                // For incomplete files, adjust the end to what's actually available
                let available_end = if actual_file_size > 0 {
                    actual_file_size.saturating_sub(1).min(end)
                } else {
                    return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
                };

                // Check if the requested range is available
                if start >= actual_file_size {
                    // If file is still downloading, we might get the data soon
                    if matches!(download_state, Some(DownloadState::Downloading)) {
                        info!(
                            "Proxy: Range start {} beyond current file size {}, but download in progress",
                            start, actual_file_size
                        );
                        // Return 503 to indicate temporary unavailability
                        return Response::builder()
                            .status(StatusCode::SERVICE_UNAVAILABLE)
                            .header("Retry-After", "1")
                            .header(header::ACCEPT_RANGES, "bytes")
                            .body(vec![].into())
                            .unwrap();
                    } else {
                        warn!(
                            "Proxy: Range start {} exceeds available file size {} (download state: {:?})",
                            start, actual_file_size, download_state
                        );
                        return Response::builder()
                            .status(StatusCode::RANGE_NOT_SATISFIABLE)
                            .header(header::CONTENT_RANGE, format!("bytes */{}", total_size))
                            .header(header::ACCEPT_RANGES, "bytes")
                            .body(vec![].into())
                            .unwrap();
                    }
                }

                // Serve partial content with what's available
                let length = available_end - start + 1;

                // Seek to start position
                if let Err(e) = file.seek(std::io::SeekFrom::Start(start)).await {
                    error!("Failed to seek in file: {}", e);
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }

                // Read the requested range (up to what's available)
                let mut buffer = vec![0u8; length as usize];
                match file.read_exact(&mut buffer).await {
                    Ok(_) => {
                        info!(
                            "Proxy: Serving range {}-{}/{} (actual file size: {}, length: {} bytes)",
                            start, available_end, total_size, actual_file_size, length
                        );
                        self.stats.add_bytes_served(length);
                        Response::builder()
                            .status(StatusCode::PARTIAL_CONTENT)
                            .header(header::CONTENT_TYPE, "video/mp4")
                            .header(header::CONTENT_LENGTH, length.to_string())
                            .header(header::ACCEPT_RANGES, "bytes")
                            .header(
                                header::CONTENT_RANGE,
                                format!("bytes {}-{}/{}", start, available_end, total_size),
                            )
                            .header("Cache-Control", "no-cache, no-store, must-revalidate")
                            .header("Pragma", "no-cache")
                            .body(buffer.into())
                            .unwrap()
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                        // File is still downloading, try to read what we can
                        warn!(
                            "File still downloading, attempting partial read for range {}-{}",
                            start, available_end
                        );

                        // Reset file position and try reading what's available
                        if let Err(seek_err) = file.seek(std::io::SeekFrom::Start(start)).await {
                            error!("Failed to seek after EOF: {}", seek_err);
                            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                        }

                        let mut partial_buffer = Vec::new();
                        match file.read_to_end(&mut partial_buffer).await {
                            Ok(bytes_read) if bytes_read > 0 => {
                                let actual_end = start + bytes_read as u64 - 1;
                                info!(
                                    "Proxy: Serving partial range {}-{}/{} (read {} bytes from still-downloading file)",
                                    start, actual_end, total_size, bytes_read
                                );
                                self.stats.add_bytes_served(bytes_read as u64);
                                Response::builder()
                                    .status(StatusCode::PARTIAL_CONTENT)
                                    .header(header::CONTENT_TYPE, "video/mp4")
                                    .header(header::CONTENT_LENGTH, bytes_read.to_string())
                                    .header(header::ACCEPT_RANGES, "bytes")
                                    .header(
                                        header::CONTENT_RANGE,
                                        format!("bytes {}-{}/{}", start, actual_end, total_size),
                                    )
                                    .body(partial_buffer.into())
                                    .unwrap()
                            }
                            _ => {
                                warn!(
                                    "No data available yet for range {}-{}",
                                    start, available_end
                                );
                                // Return 503 Service Unavailable to indicate temporary unavailability
                                Response::builder()
                                    .status(StatusCode::SERVICE_UNAVAILABLE)
                                    .header("Retry-After", "1")
                                    .body(vec![].into())
                                    .unwrap()
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to read file: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR.into_response()
                    }
                }
            }
            None => {
                // Serve full file (only what's been downloaded)
                // For streaming, if file is incomplete, we should indicate it
                let mut buffer = Vec::new();
                match file.read_to_end(&mut buffer).await {
                    Ok(bytes_read) => {
                        info!(
                            "Proxy: Serving full file: {} bytes (total expected: {})",
                            bytes_read, total_size
                        );
                        self.stats.add_bytes_served(bytes_read as u64);

                        // ALWAYS use 206 Partial Content to signal seekability to GStreamer
                        // This forces GStreamer to recognize that the stream supports Range requests
                        let status = StatusCode::PARTIAL_CONTENT;

                        let mut response = Response::builder()
                            .status(status)
                            .header(header::CONTENT_TYPE, "video/mp4")
                            .header(header::CONTENT_LENGTH, bytes_read.to_string())
                            .header(header::ACCEPT_RANGES, "bytes")
                            .header(
                                header::CONTENT_RANGE,
                                format!("bytes 0-{}/{}", bytes_read.saturating_sub(1), total_size),
                            );

                        response
                            .header("Cache-Control", "no-cache, no-store, must-revalidate")
                            .header("Pragma", "no-cache")
                            .body(buffer.into())
                            .unwrap()
                    }
                    Err(e) => {
                        error!("Failed to read file: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR.into_response()
                    }
                }
            }
        }
    }

    /// Serve HEAD request for a file from cache
    async fn serve_file_head(&self, cache_key: &MediaCacheKey, headers: HeaderMap) -> Response {
        info!(
            "Proxy: Received HEAD request for cache key: {:?}",
            cache_key
        );

        // Increment request stats
        self.stats.increment_request();

        // Get cache entry
        let entry = {
            let mut storage = self.storage.write().await;
            storage.get_entry(cache_key)
        };

        let entry = match entry {
            Some(e) => e,
            None => {
                warn!("Cache entry not found for key: {:?}", cache_key);
                return StatusCode::NOT_FOUND.into_response();
            }
        };

        // Check if file exists
        let file_path = entry.file_path.clone();
        let metadata = match tokio::fs::metadata(&file_path).await {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to get file metadata: {}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        let actual_file_size = metadata.len();

        // Use the expected total size from metadata if available
        let total_size = if entry.metadata.expected_total_size > 0 {
            entry.metadata.expected_total_size
        } else {
            actual_file_size
        };

        // Parse range header if present (for validation)
        let range = headers
            .get(header::RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(|range_str| parse_range_header(range_str, total_size));

        // Build response headers without body
        let mut response = Response::builder();

        // For HEAD requests, always return 200 OK (not 206) unless there's an error
        response = response
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "video/mp4")
            .header(header::CONTENT_LENGTH, total_size.to_string())
            .header(header::ACCEPT_RANGES, "bytes");

        // Add cache headers
        response = response.header("Cache-Control", "public, max-age=3600");

        // Return empty body for HEAD request
        response.body(vec![].into()).unwrap()
    }

    /// Get the proxy server port
    pub fn port(&self) -> u16 {
        self.port
    }
}

/// Parse a Range header value
fn parse_range_header(range: &str, file_size: u64) -> Option<(u64, u64)> {
    if !range.starts_with("bytes=") {
        return None;
    }

    let range = &range[6..];
    let parts: Vec<&str> = range.split('-').collect();

    if parts.len() != 2 {
        return None;
    }

    let start = if parts[0].is_empty() {
        // Suffix range (e.g., "-500" means last 500 bytes)
        let suffix = parts[1].parse::<u64>().ok()?;
        if file_size > 0 {
            file_size.saturating_sub(suffix)
        } else {
            return None;
        }
    } else {
        parts[0].parse::<u64>().ok()?
    };

    let end = if parts[1].is_empty() {
        // Open-ended range (e.g., "500-" means from 500 to end)
        // For open-ended ranges, we use the maximum possible value
        // The actual end will be adjusted based on available data
        if file_size > 0 {
            file_size - 1
        } else {
            u64::MAX
        }
    } else {
        parts[1].parse::<u64>().ok()?
    };

    // Only validate that start is not beyond end
    // Don't validate against file_size here as file might still be downloading
    if start > end {
        return None;
    }

    // For validation against actual file size, let the caller handle it
    // This allows serving partial content for still-downloading files

    Some((
        start,
        end.min(if file_size > 0 {
            file_size - 1
        } else {
            u64::MAX
        }),
    ))
}
