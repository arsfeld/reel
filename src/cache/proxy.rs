use anyhow::{Context, Result};
use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{error, info};

use super::metadata::MediaCacheKey;
use super::storage::CacheStorage;
use crate::models::{MediaItemId, SourceId};

/// Cache proxy server that serves cached files over HTTP
pub struct CacheProxy {
    port: u16,
    storage: Arc<RwLock<CacheStorage>>,
    active_streams: Arc<RwLock<HashMap<String, MediaCacheKey>>>,
}

impl CacheProxy {
    /// Create a new cache proxy server
    pub fn new(storage: Arc<RwLock<CacheStorage>>) -> Self {
        // Find an available port
        let port = Self::find_available_port();

        Self {
            port,
            storage,
            active_streams: Arc::new(RwLock::new(HashMap::new())),
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

    /// Create the router for the proxy server
    fn create_router(self: &Arc<Self>) -> Router {
        Router::new()
            .route(
                "/cache/:source_id/:media_id/:quality",
                get(Self::serve_cached_file),
            )
            .route("/stream/:stream_id", get(Self::serve_stream))
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

    /// Serve a file from cache (supports range requests)
    async fn serve_file(&self, cache_key: &MediaCacheKey, headers: HeaderMap) -> Response {
        // Get cache entry
        let entry = {
            let mut storage = self.storage.write().await;
            storage.get_entry(cache_key)
        };

        let entry = match entry {
            Some(e) => e,
            None => {
                error!("Cache entry not found for key: {:?}", cache_key);
                return StatusCode::NOT_FOUND.into_response();
            }
        };

        // Check if file exists
        if !entry.exists() {
            error!("Cache file not found: {:?}", entry.file_path);
            return StatusCode::NOT_FOUND.into_response();
        }

        // Get file size
        let file_size = match entry.file_size() {
            Ok(size) => size,
            Err(e) => {
                error!("Failed to get file size: {}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        // Parse range header if present
        let range = headers
            .get(header::RANGE)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| parse_range_header(s, file_size));

        // Open the file
        let mut file = match File::open(&entry.file_path).await {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to open cache file: {}", e);
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        match range {
            Some((start, end)) => {
                // Serve partial content
                let length = end - start + 1;

                // Seek to start position
                if let Err(e) = file.seek(std::io::SeekFrom::Start(start)).await {
                    error!("Failed to seek in file: {}", e);
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }

                // Read the requested range
                let mut buffer = vec![0u8; length as usize];
                match file.read_exact(&mut buffer).await {
                    Ok(_) => Response::builder()
                        .status(StatusCode::PARTIAL_CONTENT)
                        .header(header::CONTENT_TYPE, "video/mp4")
                        .header(header::CONTENT_LENGTH, length.to_string())
                        .header(header::ACCEPT_RANGES, "bytes")
                        .header(
                            header::CONTENT_RANGE,
                            format!("bytes {}-{}/{}", start, end, file_size),
                        )
                        .body(buffer.into())
                        .unwrap(),
                    Err(e) => {
                        error!("Failed to read file: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR.into_response()
                    }
                }
            }
            None => {
                // Serve full file
                let mut buffer = Vec::new();
                match file.read_to_end(&mut buffer).await {
                    Ok(_) => Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, "video/mp4")
                        .header(header::CONTENT_LENGTH, file_size.to_string())
                        .header(header::ACCEPT_RANGES, "bytes")
                        .body(buffer.into())
                        .unwrap(),
                    Err(e) => {
                        error!("Failed to read file: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR.into_response()
                    }
                }
            }
        }
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
        file_size.saturating_sub(suffix)
    } else {
        parts[0].parse::<u64>().ok()?
    };

    let end = if parts[1].is_empty() {
        // Open-ended range (e.g., "500-" means from 500 to end)
        file_size - 1
    } else {
        parts[1].parse::<u64>().ok()?
    };

    if start > end || end >= file_size {
        return None;
    }

    Some((start, end))
}
