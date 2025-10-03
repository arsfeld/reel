use anyhow::{Context, Result};
use axum::{
    Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, head},
};
use futures::stream::Stream;
use reqwest::Client;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::task::{Context as TaskContext, Poll};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// State machine for handling cache read failures with retries and fallbacks
#[derive(Debug, Clone)]
enum ServeStrategy {
    /// Try to serve from cache
    TryCache,
    /// Cache failed, retry downloading the chunks
    RetryDownload { attempt: u32 },
    /// Retries failed, fall back to passthrough streaming
    Passthrough,
}

impl ServeStrategy {
    const MAX_RETRY_ATTEMPTS: u32 = 1;

    fn next_on_failure(&self) -> Self {
        match self {
            ServeStrategy::TryCache => ServeStrategy::RetryDownload { attempt: 1 },
            ServeStrategy::RetryDownload { attempt } if *attempt < Self::MAX_RETRY_ATTEMPTS => {
                ServeStrategy::RetryDownload {
                    attempt: attempt + 1,
                }
            }
            ServeStrategy::RetryDownload { .. } => ServeStrategy::Passthrough,
            ServeStrategy::Passthrough => ServeStrategy::Passthrough, // Terminal state
        }
    }

    fn is_terminal(&self) -> bool {
        matches!(self, ServeStrategy::Passthrough)
    }
}

use super::chunk_manager::{ChunkManager, Priority};
use super::chunk_store::ChunkStore;
use super::metadata::MediaCacheKey;
use super::state_computer::StateComputer;
use super::state_types::DownloadState;
use super::stats::ProxyStats;
use super::storage::CacheStorage;
use crate::db::repository::CacheRepository;
use crate::models::{MediaItemId, SourceId};

/// Cache proxy server that serves cached files over HTTP
pub struct CacheProxy {
    port: u16,
    storage: Arc<RwLock<CacheStorage>>,
    state_computer: Arc<StateComputer>,
    repository: Arc<dyn CacheRepository>,
    chunk_manager: Arc<ChunkManager>,
    chunk_store: Arc<ChunkStore>,
    active_streams: Arc<RwLock<HashMap<String, MediaCacheKey>>>,
    stats: ProxyStats,
    enable_stats: bool,
    stats_interval_secs: u64,
    client: Client,
}

impl CacheProxy {
    /// Create a new cache proxy server
    pub fn new(
        storage: Arc<RwLock<CacheStorage>>,
        state_computer: Arc<StateComputer>,
        repository: Arc<dyn CacheRepository>,
        chunk_manager: Arc<ChunkManager>,
        chunk_store: Arc<ChunkStore>,
    ) -> Self {
        // Find an available port
        let port = Self::find_available_port();

        Self {
            port,
            storage,
            state_computer,
            repository,
            chunk_manager,
            chunk_store,
            active_streams: Arc::new(RwLock::new(HashMap::new())),
            stats: ProxyStats::new(),
            enable_stats: true,
            stats_interval_secs: 30,
            client: Client::new(),
        }
    }

    /// Create with custom config
    pub fn with_config(
        storage: Arc<RwLock<CacheStorage>>,
        state_computer: Arc<StateComputer>,
        repository: Arc<dyn CacheRepository>,
        chunk_manager: Arc<ChunkManager>,
        chunk_store: Arc<ChunkStore>,
        enable_stats: bool,
        stats_interval_secs: u64,
    ) -> Self {
        let port = Self::find_available_port();

        Self {
            port,
            storage,
            state_computer,
            repository,
            chunk_manager,
            chunk_store,
            active_streams: Arc::new(RwLock::new(HashMap::new())),
            stats: ProxyStats::new(),
            enable_stats,
            stats_interval_secs,
            client: Client::new(),
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

                // Only log stats if proxy has been used
                if stats.requests_served.load(Ordering::Relaxed) > 0 {
                    let report = stats.format_report();
                    info!("{}", report);
                }
            }
        });
    }

    /// Create the router for the proxy server
    fn create_router(self: &Arc<Self>) -> Router {
        Router::new()
            .route(
                "/cache/{source_id}/{media_id}/{quality}",
                get(Self::serve_cached_file).head(Self::serve_cached_file_head),
            )
            .route(
                "/stream/{stream_id}",
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
    /// NEW IMPLEMENTATION: Uses ChunkManager for database-driven chunk availability
    async fn serve_file(&self, cache_key: &MediaCacheKey, headers: HeaderMap) -> Response {
        // Increment request stats
        self.stats.increment_request();

        // Look up cache entry in database to get entry_id
        let db_entry = match self
            .repository
            .find_cache_entry(
                &cache_key.source_id.to_string(),
                &cache_key.media_id.to_string(),
                &cache_key.quality,
            )
            .await
        {
            Ok(Some(entry)) => {
                self.stats.increment_cache_hit();
                entry
            }
            Ok(None) => {
                self.stats.increment_cache_miss();
                warn!("Cache entry not found in database for key: {:?}", cache_key);
                return StatusCode::NOT_FOUND.into_response();
            }
            Err(e) => {
                // AC #3: Database lookup fails
                // Cannot fall back to original URL because we don't have it
                error!(
                    "Database error looking up cache entry for key {:?}: {}. Cannot fall back without URL.",
                    cache_key, e
                );
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
        };

        // Get entry ID and total size from database
        let entry_id = db_entry.id;
        let original_url = db_entry.original_url.clone();
        let total_size = match db_entry.expected_total_size {
            Some(size) if size > 0 => size as u64,
            _ => {
                // AC #4: Invalid cache entry (incomplete with no total size) → fetch from original
                warn!(
                    "Cache entry missing expected_total_size (entry_id={}), falling back to original URL",
                    entry_id
                );
                return self.stream_from_original_url(&original_url, headers).await;
            }
        };

        // Parse range header - if missing, treat as full file (bytes=0-)
        let range_result = headers
            .get(header::RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(|range_str| parse_range_header(range_str, total_size))
            .or_else(|| {
                // No Range header - treat as full file request (bytes=0-end)
                Some((0, total_size - 1))
            });

        // Track range vs full requests
        if headers.get(header::RANGE).is_some() {
            self.stats.increment_range_request();
        } else {
            self.stats.increment_full_request();
        }

        // Always use range-based serving (even for full file)
        // This ensures GStreamer knows the stream is seekable
        match range_result {
            Some((start, end)) => {
                // For large ranges, use progressive streaming to avoid loading GB into memory
                // For small ranges (< 50MB), check if available and read directly for better performance
                let length = end - start + 1;
                const MAX_DIRECT_READ: u64 = 50 * 1024 * 1024; // 50MB

                if length <= MAX_DIRECT_READ {
                    // Small range - check if available and wait if needed, then read directly into memory
                    let has_range = match self
                        .chunk_manager
                        .has_byte_range(entry_id, start, end)
                        .await
                    {
                        Ok(available) => available,
                        Err(e) => {
                            error!("Failed to check byte range availability: {}", e);
                            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                        }
                    };

                    if !has_range {
                        // Request missing chunks with HIGH priority
                        if let Err(e) = self
                            .chunk_manager
                            .request_chunks_for_range(
                                entry_id,
                                start,
                                end,
                                total_size,
                                Priority::HIGH,
                            )
                            .await
                        {
                            error!("Failed to request chunks: {}", e);
                            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                        }

                        // Wait for range to become available (30 second timeout)
                        let timeout = std::time::Duration::from_secs(30);
                        if let Err(_) = self
                            .chunk_manager
                            .wait_for_range(entry_id, start, end, timeout)
                            .await
                        {
                            warn!("Timeout waiting for range {}-{}", start, end);
                            return Response::builder()
                                .status(StatusCode::SERVICE_UNAVAILABLE)
                                .header("Retry-After", "5")
                                .header(header::ACCEPT_RANGES, "bytes")
                                .body(vec![].into())
                                .unwrap();
                        }
                    }

                    // Use state machine to handle cache reads with retries and fallback
                    let data = match self
                        .read_range_with_fallback(
                            entry_id,
                            start,
                            end,
                            length as usize,
                            total_size,
                            &original_url,
                            &headers,
                        )
                        .await
                    {
                        Ok(data) => data,
                        Err(response) => return response,
                    };

                    self.stats.add_bytes_served(data.len() as u64);

                    Response::builder()
                        .status(StatusCode::PARTIAL_CONTENT)
                        .header(header::CONTENT_TYPE, "video/mp4")
                        .header(header::CONTENT_LENGTH, data.len().to_string())
                        .header(header::ACCEPT_RANGES, "bytes")
                        .header(
                            header::CONTENT_RANGE,
                            format!("bytes {}-{}/{}", start, end, total_size),
                        )
                        .header("Cache-Control", "no-cache, no-store, must-revalidate")
                        .body(data.into())
                        .unwrap()
                } else {
                    // Large range (including full file) - use progressive streaming
                    // AC #2: Fall back to passthrough streaming if cache fails
                    let stream = create_range_based_progressive_stream(
                        self.chunk_manager.clone(),
                        self.chunk_store.clone(),
                        self.client.clone(),
                        original_url.clone(),
                        entry_id,
                        start,
                        end,
                        total_size,
                    );

                    Response::builder()
                        .status(StatusCode::PARTIAL_CONTENT)
                        .header(header::CONTENT_TYPE, "video/mp4")
                        .header(header::CONTENT_LENGTH, length.to_string())
                        .header(header::ACCEPT_RANGES, "bytes")
                        .header(
                            header::CONTENT_RANGE,
                            format!("bytes {}-{}/{}", start, end, total_size),
                        )
                        .header("Cache-Control", "public, max-age=3600")
                        .body(Body::from_stream(stream))
                        .unwrap()
                }
            }
            None => {
                // This should never happen now that we default to full file range
                unreachable!("All requests should be range requests (including full file)")
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

        // Determine total size - MUST be accurate for Accept-Ranges header
        let total_size = if entry.metadata.is_complete {
            // File is complete, use actual size
            actual_file_size
        } else if entry.metadata.expected_total_size > 0 {
            // File is incomplete, MUST have expected_total_size from upstream server
            entry.metadata.expected_total_size
        } else {
            // CRITICAL ERROR: Incomplete file without expected_total_size
            error!(
                "CRITICAL: HEAD request for incomplete file missing expected_total_size: {:?}",
                cache_key
            );
            error!(
                "  actual_file_size: {}, is_complete: {}",
                actual_file_size, entry.metadata.is_complete
            );
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(vec![].into())
                .unwrap();
        };

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

    /// Read a byte range with automatic retry and fallback logic
    /// Returns the data bytes or an error response
    async fn read_range_with_fallback(
        &self,
        entry_id: i32,
        start: u64,
        end: u64,
        length: usize,
        total_size: u64,
        _original_url: &str,
        _headers: &HeaderMap,
    ) -> std::result::Result<Vec<u8>, Response> {
        let mut strategy = ServeStrategy::TryCache;

        loop {
            match strategy {
                ServeStrategy::TryCache => {
                    // Try to read from cache
                    match self.chunk_store.read_range(entry_id, start, length).await {
                        Ok(data) => return Ok(data),
                        Err(e) => {
                            warn!(
                                "Cache read failed for range {}-{} (entry_id={}): {}",
                                start, end, entry_id, e
                            );
                            strategy = strategy.next_on_failure();
                        }
                    }
                }
                ServeStrategy::RetryDownload { attempt } => {
                    info!(
                        "Attempting to retry download for range {}-{} (attempt {})",
                        start, end, attempt
                    );

                    // Delete corrupted chunks and re-download
                    let retry_timeout = std::time::Duration::from_secs(30);
                    match self
                        .chunk_manager
                        .retry_range(entry_id, start, end, total_size, retry_timeout)
                        .await
                    {
                        Ok(()) => {
                            // Retry successful, try reading again
                            match self.chunk_store.read_range(entry_id, start, length).await {
                                Ok(data) => {
                                    info!("Successfully read range {}-{} after retry", start, end);
                                    return Ok(data);
                                }
                                Err(e) => {
                                    warn!(
                                        "Read failed even after retry for range {}-{}: {}",
                                        start, end, e
                                    );
                                    strategy = strategy.next_on_failure();
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Retry download failed for range {}-{}: {}", start, end, e);
                            strategy = strategy.next_on_failure();
                        }
                    }
                }
                ServeStrategy::Passthrough => {
                    error!(
                        "All cache strategies failed for range {}-{}. Returning error - passthrough not applicable for small ranges",
                        start, end
                    );
                    return Err(StatusCode::INTERNAL_SERVER_ERROR.into_response());
                }
            }
        }
    }

    /// Stream directly from the original URL (fallback when cache fails)
    async fn stream_from_original_url(&self, original_url: &str, headers: HeaderMap) -> Response {
        info!(
            "Fallback: Streaming directly from original URL: {}",
            original_url
        );

        // Build request with range headers if present
        let mut request = self.client.get(original_url);

        if let Some(range) = headers.get(header::RANGE) {
            if let Ok(range_str) = range.to_str() {
                request = request.header(header::RANGE, range_str);
            }
        }

        // Send request
        let response = match request.send().await {
            Ok(resp) => resp,
            Err(e) => {
                error!("Failed to fetch from original URL: {}", e);
                return StatusCode::BAD_GATEWAY.into_response();
            }
        };

        // Check status
        let status = response.status();
        if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
            error!("Original URL returned error status: {}", status);
            return StatusCode::BAD_GATEWAY.into_response();
        }

        // Get content length
        let content_length = response.content_length();

        // Convert response to stream
        let stream = response.bytes_stream();

        // Build response with appropriate status and headers
        let mut response_builder = Response::builder();

        if status == reqwest::StatusCode::PARTIAL_CONTENT {
            response_builder = response_builder.status(StatusCode::PARTIAL_CONTENT);
        } else {
            response_builder = response_builder.status(StatusCode::OK);
        }

        response_builder = response_builder
            .header(header::CONTENT_TYPE, "video/mp4")
            .header(header::ACCEPT_RANGES, "bytes");

        if let Some(length) = content_length {
            response_builder = response_builder.header(header::CONTENT_LENGTH, length.to_string());
        }

        // Content-range is automatically handled by the upstream server's response headers

        response_builder.body(Body::from_stream(stream)).unwrap()
    }
}

/// Create a chunk-based progressive file stream with fallback to passthrough streaming
/// Requests and waits for chunks as needed during streaming. If cache operations fail
/// (e.g., disk full, permissions), falls back to streaming directly from source.
fn create_range_based_progressive_stream(
    chunk_manager: Arc<ChunkManager>,
    chunk_store: Arc<ChunkStore>,
    client: Client,
    original_url: String,
    entry_id: i32,
    start: u64,
    end: u64,
    total_size: u64,
) -> impl Stream<Item = std::result::Result<bytes::Bytes, std::io::Error>> {
    async_stream::stream! {
        let _chunk_size = chunk_manager.chunk_size();
        let mut current_byte = start;
        let end_byte = end;
        let mut cache_failed = false;
        let mut passthrough_stream: Option<Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>> = None;

        while current_byte <= end_byte {
            // If cache has failed, use passthrough streaming for the remainder
            if cache_failed {
                if passthrough_stream.is_none() {
                    // Initialize passthrough streaming from current position
                    warn!(
                        "Cache operations failed, switching to passthrough streaming from byte {} to {}",
                        current_byte, end_byte
                    );

                    // Make HTTP range request for the remaining bytes
                    let response = match client
                        .get(&original_url)
                        .header("Range", format!("bytes={}-{}", current_byte, end_byte))
                        .send()
                        .await
                    {
                        Ok(resp) => resp,
                        Err(e) => {
                            error!("Passthrough: Failed to fetch from original URL: {}", e);
                            yield Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("Passthrough streaming failed: {}", e),
                            ));
                            break;
                        }
                    };

                    // Check status
                    let status = response.status();
                    if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
                        error!("Passthrough: Original URL returned error status: {}", status);
                        yield Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Passthrough streaming failed with status: {}", status),
                        ));
                        break;
                    }

                    // Convert to stream
                    passthrough_stream = Some(Box::pin(response.bytes_stream()));
                }

                // Stream from passthrough
                if let Some(ref mut stream) = passthrough_stream {
                    use futures::StreamExt;
                    match stream.next().await {
                        Some(Ok(chunk)) => {
                            current_byte += chunk.len() as u64;
                            yield Ok(chunk);
                        }
                        Some(Err(e)) => {
                            error!("Passthrough: Stream error: {}", e);
                            yield Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("Passthrough stream error: {}", e),
                            ));
                            break;
                        }
                        None => {
                            // Stream ended
                            break;
                        }
                    }
                }
                continue;
            }

            // Try cache-based streaming
            // Calculate current chunk
            let chunk_index = chunk_manager.byte_to_chunk_index(current_byte);
            let chunk_start = chunk_manager.chunk_start_byte(chunk_index);
            let chunk_end = chunk_manager.chunk_end_byte(chunk_index, total_size);

            // Calculate what portion of this chunk we need
            let read_start = current_byte.max(chunk_start);
            let read_end = end_byte.min(chunk_end);
            let read_length = (read_end - read_start + 1) as usize;

            // Check if chunk is available
            let has_chunk = match chunk_manager.has_chunk(entry_id, chunk_index).await {
                Ok(available) => available,
                Err(e) => {
                    // AC #1: Detect write failures during cache operations
                    warn!("Cache: Failed to check chunk availability ({}), falling back to passthrough", e);
                    cache_failed = true;
                    continue;
                }
            };

            if !has_chunk {
                // Request chunk with CRITICAL priority
                if let Err(e) = chunk_manager.request_chunk(entry_id, chunk_index, Priority::CRITICAL).await {
                    // AC #1: Detect write failures during cache operations
                    // AC #3: Log cache write failures but continue streaming
                    warn!("Cache: Failed to request chunk {} ({}), falling back to passthrough", chunk_index, e);
                    cache_failed = true;
                    continue;
                }

                // Wait for chunk (30 second timeout)
                let timeout = std::time::Duration::from_secs(30);
                if let Err(e) = chunk_manager.wait_for_chunk(entry_id, chunk_index, timeout).await {
                    // AC #1: Detect write failures during cache operations
                    // AC #3: Log cache write failures but continue streaming
                    warn!("Cache: Timeout waiting for chunk {} ({}), falling back to passthrough", chunk_index, e);
                    cache_failed = true;
                    continue;
                }
            }

            // Read the required portion from this chunk - try with one retry on failure
            let data = match chunk_store.read_range(entry_id, read_start, read_length).await {
                Ok(data) => data,
                Err(e) => {
                    // First failure - attempt retry
                    warn!("Cache: Failed to read chunk {} at byte {}: {}. Attempting retry...", chunk_index, read_start, e);

                    let retry_timeout = std::time::Duration::from_secs(30);
                    match chunk_manager.retry_range(entry_id, read_start, read_end, total_size, retry_timeout).await {
                        Ok(()) => {
                            // Retry successful, try reading again
                            match chunk_store.read_range(entry_id, read_start, read_length).await {
                                Ok(data) => {
                                    info!("Cache: Successfully read chunk {} after retry", chunk_index);
                                    data
                                }
                                Err(e) => {
                                    // AC #1: Detect write failures during cache operations
                                    // AC #3: Log cache write failures but continue streaming
                                    warn!("Cache: Read failed for chunk {} even after retry ({}), falling back to passthrough", chunk_index, e);
                                    cache_failed = true;
                                    continue;
                                }
                            }
                        }
                        Err(e) => {
                            // AC #1: Detect write failures during cache operations
                            // AC #3: Log cache write failures but continue streaming
                            warn!("Cache: Retry failed for chunk {} ({}), falling back to passthrough", chunk_index, e);
                            cache_failed = true;
                            continue;
                        }
                    }
                }
            };

            current_byte += data.len() as u64;
            yield Ok(bytes::Bytes::from(data));
        }
    }
}

fn create_chunk_based_progressive_stream(
    chunk_manager: Arc<ChunkManager>,
    chunk_store: Arc<ChunkStore>,
    entry_id: i32,
    total_size: u64,
) -> impl Stream<Item = std::result::Result<bytes::Bytes, std::io::Error>> {
    async_stream::stream! {
        let _chunk_size = chunk_manager.chunk_size();
        let mut current_byte = 0u64;

        while current_byte < total_size {
            // Calculate current chunk
            let chunk_index = chunk_manager.byte_to_chunk_index(current_byte);
            let chunk_start = chunk_manager.chunk_start_byte(chunk_index);
            let chunk_end = chunk_manager.chunk_end_byte(chunk_index, total_size);
            let chunk_length = (chunk_end - chunk_start + 1) as usize;

            // Check if chunk is available
            let has_chunk = match chunk_manager.has_chunk(entry_id, chunk_index).await {
                Ok(available) => available,
                Err(e) => {
                    error!("Failed to check chunk availability: {}", e);
                    yield Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Chunk availability check failed: {}", e),
                    ));
                    break;
                }
            };

            if !has_chunk {
                // Request chunk with CRITICAL priority
                if let Err(e) = chunk_manager.request_chunk(entry_id, chunk_index, Priority::CRITICAL).await {
                    error!("Failed to request chunk: {}", e);
                    yield Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Chunk request failed: {}", e),
                    ));
                    break;
                }

                // Wait for chunk (30 second timeout)
                let timeout = std::time::Duration::from_secs(30);
                match chunk_manager.wait_for_chunk(entry_id, chunk_index, timeout).await {
                    Ok(_) => {
                        debug!("Progressive stream: Chunk {} now available", chunk_index);
                    }
                    Err(e) => {
                        warn!("Timeout waiting for chunk {}: {}", chunk_index, e);
                        yield Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            format!("Chunk {} timeout", chunk_index),
                        ));
                        break;
                    }
                }
            }

            // Read chunk from store
            match chunk_store.read_range(entry_id, chunk_start, chunk_length).await {
                Ok(data) => {
                    debug!(
                        "Progressive stream: Streaming chunk {} ({} bytes)",
                        chunk_index,
                        data.len()
                    );
                    current_byte += data.len() as u64;
                    yield Ok(bytes::Bytes::from(data));
                }
                Err(e) => {
                    error!("Failed to read chunk {}: {}", chunk_index, e);
                    yield Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Chunk read failed: {}", e),
                    ));
                    break;
                }
            }
        }

        info!("Progressive stream finished: {} bytes streamed", current_byte);
    }
}

/// OLD IMPLEMENTATION - Create a progressive file stream that reads from a file as it's being downloaded
/// TODO: Remove this after verifying chunk-based implementation works
fn create_progressive_stream(
    mut file: File,
    cache_key: MediaCacheKey,
    state_computer: Arc<StateComputer>,
    total_size: u64,
) -> impl Stream<Item = std::result::Result<bytes::Bytes, std::io::Error>> {
    async_stream::stream! {
        const CHUNK_SIZE: usize = 256 * 1024; // 256KB chunks
        let mut position: u64 = 0;
        let mut eof_wait_count: u32 = 0;
        let max_eof_waits: u32 = 30; // Wait up to 30 times (30 seconds with 1s intervals)

        while position < total_size {
            // Calculate how much to read (don't exceed total_size)
            let remaining = (total_size - position) as usize;
            let read_size = remaining.min(CHUNK_SIZE);

            // Try to read from the file
            let mut buffer = vec![0u8; read_size];
            match file.read(&mut buffer).await {
                Ok(0) => {
                    // Hit EOF - check if download is still in progress
                    let state_info = state_computer.get_state(&cache_key).await;

                    match state_info.as_ref().map(|info| &info.state) {
                        Some(DownloadState::Downloading) | Some(DownloadState::Paused) => {
                            // Still downloading, wait and retry
                            eof_wait_count += 1;

                            if eof_wait_count >= max_eof_waits {
                                warn!(
                                    "Exceeded max EOF waits ({}) at position {}",
                                    max_eof_waits, position
                                );
                                yield Err(std::io::Error::new(
                                    std::io::ErrorKind::TimedOut,
                                    "Timeout waiting for download to progress",
                                ));
                                break;
                            }

                            debug!(
                                "Hit EOF at position {}, download in progress, waiting... (attempt {}/{})",
                                position, eof_wait_count, max_eof_waits
                            );

                            // Wait for 1 second before retrying
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            continue;
                        }
                        Some(DownloadState::Complete) => {
                            // Download complete, we've reached actual EOF
                            debug!(
                                "Download complete, reached EOF at position {} (total: {})",
                                position, total_size
                            );
                            break;
                        }
                        Some(DownloadState::Failed(msg)) => {
                            error!("Download failed during stream: {}", msg);
                            yield Err(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("Download failed: {}", msg),
                            ));
                            break;
                        }
                        _ => {
                            // Other states or no state info, treat as EOF
                            debug!("No active download state, treating as EOF at position {}", position);
                            break;
                        }
                    }
                }
                Ok(n) => {
                    // Successfully read data
                    position += n as u64;
                    eof_wait_count = 0; // Reset wait count on successful read

                    debug!(
                        "Read {} bytes at position {} (total: {})",
                        n,
                        position - n as u64,
                        total_size
                    );

                    yield Ok(bytes::Bytes::copy_from_slice(&buffer[..n]));
                }
                Err(e) => {
                    error!("Error reading from file at position {}: {}", position, e);
                    yield Err(e);
                    break;
                }
            }
        }

        info!("Progressive stream finished at position {} (total: {})", position, total_size);
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
