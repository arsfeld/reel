use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json;
use std::time::Duration;
use tokio::sync::oneshot;
use tracing::{debug, info};

use super::api::create_standard_headers;

const PLEX_TV_URL: &str = "https://plex.tv";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlexPin {
    pub id: String,
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PlexPinResponse {
    id: i32,
    code: String,
    #[serde(rename = "authToken")]
    auth_token: Option<String>,
}

#[allow(dead_code)]
pub struct PlexAuth;

impl PlexAuth {
    /// Request a new PIN from Plex for authentication
    pub async fn get_pin() -> Result<PlexPin> {
        let client = reqwest::Client::new();

        let response = client
            .post(format!("{}/api/v2/pins", PLEX_TV_URL))
            .headers(create_standard_headers(None))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get PIN: {}", response.status()));
        }

        let text = response.text().await?;
        debug!("PIN response: {}", text);

        let pin_response: PlexPinResponse = serde_json::from_str(&text)?;

        info!("Got Plex PIN: {}", pin_response.code);

        Ok(PlexPin {
            id: pin_response.id.to_string(),
            code: pin_response.code,
        })
    }

    /// Poll for the auth token after user has entered the PIN with cancellation support
    pub async fn poll_for_token(
        pin_id: &str,
        mut cancel_rx: Option<oneshot::Receiver<()>>,
    ) -> Result<String> {
        let client = reqwest::Client::new();
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 120; // 4 minutes with 2 second intervals

        loop {
            // Check for cancellation
            if let Some(ref mut cancel) = cancel_rx
                && cancel.try_recv().is_ok()
            {
                debug!("Polling cancelled by user");
                return Err(anyhow!("Authentication cancelled"));
            }

            if attempts >= MAX_ATTEMPTS {
                return Err(anyhow!("Authentication timeout"));
            }

            let response = client
                .get(format!("{}/api/v2/pins/{}", PLEX_TV_URL, pin_id))
                .headers(create_standard_headers(None))
                .send()
                .await;

            match response {
                Ok(resp) => {
                    // A 404 after linking means the PIN was consumed (success)
                    if resp.status() == 404 {
                        // PIN is gone, which likely means it was used successfully
                        // We should have gotten the token in a previous iteration
                        // This is a known Plex API behavior
                        return Err(anyhow!(
                            "PIN was consumed. If you saw 'Account Linked' on plex.tv, authentication was successful but we couldn't retrieve the token. Please try using manual connection instead."
                        ));
                    }

                    if resp.status().is_success() {
                        let text = resp.text().await.unwrap_or_default();
                        debug!("PIN check response: {}", text);

                        match serde_json::from_str::<PlexPinResponse>(&text) {
                            Ok(pin_response) => {
                                // Check if we have a token
                                if let Some(token) = pin_response.auth_token
                                    && !token.is_empty()
                                {
                                    info!(
                                        "Authentication successful! Token received: {}...",
                                        &token[..8.min(token.len())]
                                    );
                                    return Ok(token);
                                }
                                // No token yet, continue polling
                                debug!("No token yet, continuing to poll...");
                            }
                            Err(e) => {
                                debug!("Failed to parse response: {}. Text was: {}", e, text);
                            }
                        }
                    } else {
                        debug!("Unexpected status: {}", resp.status());
                    }
                }
                Err(e) => {
                    debug!("Request error: {}", e);
                    // Continue polling on network errors
                }
            }

            debug!(
                "Waiting for authentication... (attempt {}/{})",
                attempts + 1,
                MAX_ATTEMPTS
            );

            // Use select to make the sleep cancellable
            if let Some(ref mut cancel) = cancel_rx {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(2)) => {},
                    _ = cancel => {
                        debug!("Polling cancelled during sleep");
                        return Err(anyhow!("Authentication cancelled"));
                    }
                }
            } else {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }

            attempts += 1;
        }
    }

    /// Get user information with the auth token
    pub async fn get_user(auth_token: &str) -> Result<PlexUser> {
        let client = reqwest::Client::new();

        let response = match client
            .get(format!("{}/api/v2/user", PLEX_TV_URL))
            .headers(create_standard_headers(Some(auth_token)))
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                // Network error - don't treat as auth failure
                return Err(anyhow!("Network error while fetching user info: {}", e));
            }
        };

        // Check for authentication errors specifically
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(anyhow!("Authentication failed: Invalid or expired token"));
        }

        if !response.status().is_success() {
            return Err(anyhow!(
                "Server error while fetching user info: {}",
                response.status()
            ));
        }

        let user: PlexUser = response.json().await?;
        Ok(user)
    }

    /// Discover available Plex servers for the authenticated user
    pub async fn discover_servers(auth_token: &str) -> Result<Vec<PlexServer>> {
        let client = reqwest::Client::new();

        let response = client
            .get(format!("{}/api/v2/resources", PLEX_TV_URL))
            .headers(create_standard_headers(Some(auth_token)))
            .query(&[("includeHttps", "1"), ("includeRelay", "1")])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            debug!("Server discovery failed with status {}: {}", status, text);
            return Err(anyhow!("Failed to discover servers: {}", status));
        }

        // Parse the response as an array of resources
        let resources: Vec<PlexServer> = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse server discovery response: {}", e))?;

        // Filter for actual Plex Media Servers (not players/controllers)
        let servers: Vec<PlexServer> = resources
            .into_iter()
            .filter(|r| r.provides.contains("server"))
            .collect();

        info!("Found {} Plex servers", servers.len());
        Ok(servers)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlexUser {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub thumb: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlexServer {
    pub name: String,
    #[serde(default)]
    pub product: String,
    #[serde(rename = "productVersion", default)]
    pub product_version: String,
    #[serde(default)]
    pub platform: String,
    #[serde(rename = "platformVersion", default)]
    pub platform_version: String,
    #[serde(default)]
    pub device: String,
    #[serde(rename = "clientIdentifier")]
    pub client_identifier: String,
    #[serde(rename = "createdAt", default)]
    pub created_at: String,
    #[serde(rename = "lastSeenAt", default)]
    pub last_seen_at: String,
    pub provides: String,
    #[serde(default)]
    pub owned: bool,
    #[serde(default)]
    pub home: bool,
    #[serde(default)]
    pub connections: Vec<PlexConnection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlexConnection {
    pub protocol: String,
    pub address: String,
    pub port: i32,
    pub uri: String,
    pub local: bool,
    pub relay: bool,
}
