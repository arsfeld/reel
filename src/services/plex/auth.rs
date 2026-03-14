use serde::Deserialize;
use std::path::PathBuf;

use super::error::PlexError;

const PLEX_TV_URL: &str = "https://plex.tv";
const PLEX_AUTH_URL: &str = "https://app.plex.tv/auth#";
const POLL_INTERVAL_MS: u64 = 1000;
const POLL_TIMEOUT_SECS: u64 = 300; // 5 minutes

/// Generate or load a persistent client identifier (UUID).
pub fn client_identifier(data_dir: &PathBuf) -> String {
    let id_path = data_dir.join("client_id");
    if let Ok(id) = std::fs::read_to_string(&id_path) {
        let id = id.trim().to_string();
        if !id.is_empty() {
            return id;
        }
    }
    // Generate a new one
    let id = format!(
        "reel-{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        rand_u32(),
        rand_u16(),
        rand_u16(),
        rand_u16(),
        rand_u64() & 0xFFFF_FFFF_FFFF,
    );
    let _ = std::fs::create_dir_all(data_dir);
    let _ = std::fs::write(&id_path, &id);
    id
}

// Simple random using /dev/urandom (always available on Linux)
fn rand_bytes(buf: &mut [u8]) {
    use std::io::Read;
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(buf);
    }
}

fn rand_u32() -> u32 {
    let mut buf = [0u8; 4];
    rand_bytes(&mut buf);
    u32::from_le_bytes(buf)
}

fn rand_u16() -> u16 {
    let mut buf = [0u8; 2];
    rand_bytes(&mut buf);
    u16::from_le_bytes(buf)
}

fn rand_u64() -> u64 {
    let mut buf = [0u8; 8];
    rand_bytes(&mut buf);
    u64::from_le_bytes(buf)
}

fn plex_headers(client_id: &str) -> reqwest::header::HeaderMap {
    use reqwest::header::{HeaderMap, HeaderValue};
    let mut headers = HeaderMap::new();
    headers.insert("Accept", HeaderValue::from_static("application/json"));
    headers.insert("X-Plex-Product", HeaderValue::from_static("Reel"));
    headers.insert("X-Plex-Version", HeaderValue::from_static("0.1.0"));
    headers.insert("X-Plex-Platform", HeaderValue::from_static("Linux"));
    headers.insert("X-Plex-Device", HeaderValue::from_static("PC"));
    headers.insert("X-Plex-Provides", HeaderValue::from_static("player"));
    if let Ok(v) = HeaderValue::from_str(client_id) {
        headers.insert("X-Plex-Client-Identifier", v);
    }
    headers
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinResponse {
    pub id: i64,
    pub code: String,
    pub auth_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlexResource {
    pub name: String,
    #[serde(default)]
    pub provides: String,
    #[serde(default)]
    pub connections: Vec<PlexConnection>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlexConnection {
    #[serde(default)]
    pub uri: String,
    #[serde(default)]
    pub local: bool,
    #[serde(default)]
    pub relay: bool,
}

/// Request a PIN from plex.tv. Returns (pin_id, code, auth_url).
pub async fn request_pin(client_id: &str) -> Result<(i64, String, String), PlexError> {
    let http = reqwest::Client::new();
    let resp = http
        .post(format!("{PLEX_TV_URL}/api/v2/pins"))
        .headers(plex_headers(client_id))
        .form(&[("strong", "true")])
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(PlexError::Server {
            status: resp.status().as_u16(),
            message: "Failed to create PIN".to_string(),
        });
    }

    let pin: PinResponse = resp.json().await?;

    let auth_url = format!(
        "{PLEX_AUTH_URL}?clientID={client_id}&code={code}\
         &context%5Bdevice%5D%5Bproduct%5D=Reel\
         &context%5Bdevice%5D%5Blayout%5D=desktop\
         &context%5Bdevice%5D%5Bplatform%5D=Linux",
        code = pin.code,
    );

    Ok((pin.id, pin.code, auth_url))
}

/// Poll plex.tv until the user authenticates or timeout.
pub async fn poll_for_token(client_id: &str, pin_id: i64, code: &str) -> Result<String, PlexError> {
    let http = reqwest::Client::new();
    let url = format!("{PLEX_TV_URL}/api/v2/pins/{pin_id}");
    let max_attempts = (POLL_TIMEOUT_SECS * 1000 / POLL_INTERVAL_MS) as usize;

    for _ in 0..max_attempts {
        tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;

        let resp = http
            .get(&url)
            .headers(plex_headers(client_id))
            .query(&[("code", code)])
            .send()
            .await?;

        if !resp.status().is_success() {
            continue;
        }

        let pin: PinResponse = resp.json().await?;
        if let Some(token) = pin.auth_token
            && !token.is_empty()
        {
            return Ok(token);
        }
    }

    Err(PlexError::Server {
        status: 408,
        message: "Authentication timed out. Please try again.".to_string(),
    })
}

/// Discover Plex servers available to the authenticated user.
pub async fn discover_servers(
    client_id: &str,
    auth_token: &str,
) -> Result<Vec<PlexResource>, PlexError> {
    let http = reqwest::Client::new();
    let mut headers = plex_headers(client_id);
    if let Ok(v) = reqwest::header::HeaderValue::from_str(auth_token) {
        headers.insert("X-Plex-Token", v);
    }

    let resp = http
        .get(format!(
            "{PLEX_TV_URL}/api/v2/resources?includeHttps=1&includeRelay=1"
        ))
        .headers(headers)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(PlexError::Server {
            status: resp.status().as_u16(),
            message: "Failed to discover servers".to_string(),
        });
    }

    let resources: Vec<PlexResource> = resp.json().await?;
    // Filter to only Plex Media Servers
    let servers: Vec<PlexResource> = resources
        .into_iter()
        .filter(|r| r.provides.contains("server"))
        .collect();

    Ok(servers)
}

/// Open a URL in the system browser.
pub fn open_browser(url: &str) {
    let _ = std::process::Command::new("xdg-open")
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Pick the best reachable connection URI for a server.
///
/// Tests all connections in parallel with a short timeout, then picks the
/// best one that responded (preferring local, non-relay).
pub async fn best_server_uri(server: &PlexResource) -> Option<String> {
    use futures::future::join_all;

    tracing::info!(
        "Testing {} connections for server '{}'",
        server.connections.len(),
        server.name
    );
    for conn in &server.connections {
        tracing::info!(
            "  candidate: {} (local={}, relay={})",
            conn.uri,
            conn.local,
            conn.relay
        );
    }

    let http = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .build()
        .ok()?;

    // Test all connections in parallel
    let results = join_all(server.connections.iter().map(|conn| {
        let http = http.clone();
        let uri = conn.uri.clone();
        async move {
            let url = format!("{uri}/");
            match http.get(&url).send().await {
                Ok(resp) => {
                    tracing::info!("  reachable: {uri} (status={})", resp.status());
                    true
                }
                Err(e) => {
                    tracing::info!("  unreachable: {uri} ({e})");
                    false
                }
            }
        }
    }))
    .await;

    // Pick the best reachable connection: prefer local non-relay
    let mut best: Option<&PlexConnection> = None;
    for (conn, reachable) in server.connections.iter().zip(results.iter()) {
        if *reachable {
            let dominated =
                best.is_some_and(|b| !conn.relay && (b.relay || (conn.local && !b.local)));
            if best.is_none() || dominated {
                best = Some(conn);
            }
        }
    }

    if let Some(conn) = best {
        tracing::info!(
            "Selected connection: {} (local={}, relay={})",
            conn.uri,
            conn.local,
            conn.relay
        );
        return Some(conn.uri.clone());
    }

    tracing::warn!("No connections responded for server '{}'", server.name);
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_identifier_is_persistent() {
        let dir = tempfile::tempdir().unwrap();
        let id1 = client_identifier(&dir.path().to_path_buf());
        let id2 = client_identifier(&dir.path().to_path_buf());
        assert_eq!(id1, id2);
        assert!(id1.starts_with("reel-"));
    }

    #[test]
    fn client_identifier_generates_uuid_format() {
        let dir = tempfile::tempdir().unwrap();
        let id = client_identifier(&dir.path().to_path_buf());
        assert!(id.starts_with("reel-"));
        assert!(id.len() > 20);
        // Should contain hyphens like a UUID
        assert!(id.matches('-').count() >= 4);
    }

    #[tokio::test]
    async fn best_server_uri_picks_reachable_server() {
        let mock = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&mock)
            .await;

        let server = PlexResource {
            name: "My Server".to_string(),
            provides: "server".to_string(),
            connections: vec![
                PlexConnection {
                    uri: "http://10.88.0.1:32400".to_string(), // unreachable
                    local: true,
                    ..Default::default()
                },
                PlexConnection {
                    uri: mock.uri(), // reachable
                    local: true,
                    ..Default::default()
                },
            ],
        };
        assert_eq!(best_server_uri(&server).await, Some(mock.uri()));
    }

    #[tokio::test]
    async fn best_server_uri_returns_none_when_none_reachable() {
        let server = PlexResource {
            name: "My Server".to_string(),
            provides: "server".to_string(),
            connections: vec![PlexConnection {
                uri: "http://192.0.2.1:32400".to_string(), // unreachable (TEST-NET)
                local: true,
                ..Default::default()
            }],
        };
        let result = best_server_uri(&server).await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn best_server_uri_returns_none_for_no_connections() {
        let server = PlexResource {
            name: "Empty".to_string(),
            provides: "server".to_string(),
            connections: vec![],
        };
        assert_eq!(best_server_uri(&server).await, None);
    }

    #[tokio::test]
    async fn request_pin_parses_response() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api/v2/pins"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_string(r#"{"id":12345,"code":"abc123","authToken":null}"#),
            )
            .mount(&server)
            .await;

        // We can't easily test request_pin directly since it hardcodes plex.tv,
        // but we can test the response parsing
        let json = r#"{"id":12345,"code":"abc123","authToken":null}"#;
        let pin: PinResponse = serde_json::from_str(json).unwrap();
        assert_eq!(pin.id, 12345);
        assert_eq!(pin.code, "abc123");
        assert!(pin.auth_token.is_none());
    }

    #[test]
    fn pin_response_with_token() {
        let json = r#"{"id":12345,"code":"abc123","authToken":"mytoken123"}"#;
        let pin: PinResponse = serde_json::from_str(json).unwrap();
        assert_eq!(pin.auth_token, Some("mytoken123".to_string()));
    }

    #[test]
    fn discover_servers_deserializes_resources() {
        let json = r#"[
            {"name":"My Server","provides":"server","connections":[
                {"uri":"http://192.168.1.100:32400","local":true},
                {"uri":"https://abc.plex.direct:32400","local":false}
            ]},
            {"name":"Plex Web","provides":"player","connections":[]}
        ]"#;
        let resources: Vec<PlexResource> = serde_json::from_str(json).unwrap();
        let servers: Vec<_> = resources
            .into_iter()
            .filter(|r| r.provides.contains("server"))
            .collect();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "My Server");
        assert_eq!(servers[0].connections.len(), 2);
    }
}
