use anyhow::{Result, anyhow};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::header::{HeaderMap, HeaderValue};
use std::time::Duration;
use tracing::{debug, error};

use super::types::PlexIdentityResponse;

/// Standard Plex headers used across all API requests.
/// These constants ensure consistent client identification across the entire Plex API integration.
///
/// ## Header Management Pattern
///
/// All Plex API requests should use the `standard_headers()` method to ensure consistent
/// header application. This centralized approach:
///
/// 1. **Ensures consistency**: All requests include the same client identification headers
/// 2. **Simplifies maintenance**: Headers can be updated in one place
/// 3. **Reduces errors**: No missing or inconsistent headers across different API calls
/// 4. **Supports extensibility**: Additional headers can be added via `headers_with_extras()`
///
/// ### Usage Examples
///
/// ```rust
/// // Standard API call with default headers
/// let response = self.client
///     .get(&url)
///     .headers(self.standard_headers())
///     .send()
///     .await?;
///
/// // API call with additional custom headers
/// let mut extra_headers = HeaderMap::new();
/// extra_headers.insert("X-Plex-Container-Size", HeaderValue::from_static("50"));
/// let response = self.client
///     .get(&url)
///     .headers(self.headers_with_extras(extra_headers))
///     .send()
///     .await?;
/// ```
pub const PLEX_PRODUCT: &str = "Reel";
pub const PLEX_VERSION: &str = "0.1.0";
pub const PLEX_CLIENT_IDENTIFIER: &str = "reel-media-player";
pub const PLEX_PLATFORM: &str = "Linux";

/// Create standard Plex headers for use in API calls that don't have an ApiClient instance.
/// This is particularly useful for authentication and connection testing scenarios.
///
/// # Arguments
/// * `auth_token` - Optional authentication token. Pass None for unauthenticated requests.
///
/// # Returns
/// A HeaderMap containing all standard Plex headers
pub fn create_standard_headers(auth_token: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();

    // Add auth token if provided
    if let Some(token) = auth_token {
        headers.insert(
            "X-Plex-Token",
            HeaderValue::from_str(token).expect("Invalid auth token"),
        );
    }

    // Always include accept header for JSON responses
    headers.insert("Accept", HeaderValue::from_static("application/json"));

    // Include Plex client identification headers
    headers.insert(
        "X-Plex-Client-Identifier",
        HeaderValue::from_static(PLEX_CLIENT_IDENTIFIER),
    );
    headers.insert("X-Plex-Product", HeaderValue::from_static(PLEX_PRODUCT));
    headers.insert("X-Plex-Version", HeaderValue::from_static(PLEX_VERSION));
    headers.insert("X-Plex-Platform", HeaderValue::from_static(PLEX_PLATFORM));

    headers
}

#[derive(Clone)]
#[allow(dead_code)] // Used internally by PlexBackend
pub struct PlexApi {
    pub(super) client: reqwest::Client,
    pub(super) base_url: String,
    pub(super) auth_token: String,
    pub(super) backend_id: String,
}

impl PlexApi {
    pub fn with_backend_id(base_url: String, auth_token: String, backend_id: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url,
            auth_token,
            backend_id,
        }
    }

    pub(super) fn build_url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Build standard headers that should be included in all Plex API requests
    pub(super) fn standard_headers(&self) -> HeaderMap {
        create_standard_headers(Some(&self.auth_token))
    }

    /// Build standard headers with optional additional headers merged in
    pub(super) fn headers_with_extras(&self, extras: HeaderMap) -> HeaderMap {
        let mut headers = self.standard_headers();
        headers.extend(extras);
        headers
    }

    /// Build full image URL from Plex path
    pub(super) fn build_image_url(&self, path: &str) -> String {
        if path.starts_with("http") {
            path.to_string()
        } else {
            // Use Plex transcoding endpoint for server-side image resizing
            // This dramatically reduces bandwidth and client-side processing
            let encoded_url = utf8_percent_encode(path, NON_ALPHANUMERIC).to_string();
            format!(
                "{}/photo/:/transcode?width=320&height=480&minSize=1&upscale=1&url={}&X-Plex-Token={}",
                self.base_url, encoded_url, self.auth_token
            )
        }
    }

    pub async fn get_machine_id(&self) -> Result<String> {
        let url = self.build_url("/identity");
        debug!("Fetching machine ID from: {}", url);

        let response = self
            .client
            .get(&url)
            .headers(self.standard_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            error!("Failed to fetch machine ID: {} - {}", status, text);
            return Err(anyhow!("Failed to fetch machine ID: {}", status));
        }

        let identity: PlexIdentityResponse = response.json().await?;

        debug!(
            "Got machine ID: {} (version: {})",
            identity.media_container.machine_identifier, identity.media_container.version
        );

        Ok(identity.media_container.machine_identifier)
    }
}
