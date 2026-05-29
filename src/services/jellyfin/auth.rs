use std::path::PathBuf;

use super::error::JellyfinError;
use super::models::{AuthenticationResult, QuickConnectResult};

const APP_VERSION: &str = "0.1.0";
const POLL_INTERVAL_MS: u64 = 200;
const POLL_TIMEOUT_SECS: u64 = 300; // 5 minutes

/// Generate or load a persistent Jellyfin device identifier (UUID-ish).
///
/// Persisted to `jellyfin_device_id` in the data dir so it is independent of
/// the Plex `client_id`. The server permits one token per device id, so this
/// must be stable across launches.
pub fn device_id(data_dir: &PathBuf) -> String {
    let id_path = data_dir.join("jellyfin_device_id");
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

/// Build the device-scoped `MediaBrowser` Authorization header used for auth
/// requests. Unlike the authenticated client header, this carries NO `Token=`
/// field — there is no token yet.
fn device_auth_header(device_id: &str) -> String {
    format!(
        "MediaBrowser Client=\"Reel\", Device=\"Reel\", DeviceId=\"{device_id}\", Version=\"{APP_VERSION}\""
    )
}

/// Build a plain reqwest client carrying the device Authorization header.
/// Strict TLS (no `danger_accept_invalid_certs`), short connect timeout.
fn auth_client(device_id: &str) -> Result<reqwest::Client, JellyfinError> {
    use reqwest::header::{HeaderMap, HeaderValue};
    let mut headers = HeaderMap::new();
    headers.insert("Accept", HeaderValue::from_static("application/json"));
    if let Ok(value) = HeaderValue::from_str(&device_auth_header(device_id)) {
        headers.insert("Authorization", value);
    }
    Ok(reqwest::Client::builder()
        .default_headers(headers)
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(15))
        .build()?)
}

/// Map a non-success response to the appropriate `JellyfinError`.
fn map_status(status: reqwest::StatusCode) -> JellyfinError {
    match status.as_u16() {
        401 => JellyfinError::Unauthorized,
        404 => JellyfinError::NotFound("Resource not found".to_string()),
        s => JellyfinError::Server {
            status: s,
            message: status
                .canonical_reason()
                .unwrap_or("Unknown error")
                .to_string(),
        },
    }
}

/// Authenticate by username + password.
///
/// `POST {base}/Users/AuthenticateByName` with body `{ "Username", "Pw" }`.
/// Returns `(access_token, user_id)`.
pub async fn authenticate_by_name(
    base_url: &str,
    device_id: &str,
    username: &str,
    password: &str,
) -> Result<(String, String), JellyfinError> {
    let base = base_url.trim_end_matches('/');
    let http = auth_client(device_id)?;
    let body = serde_json::json!({ "Username": username, "Pw": password });

    let resp = http
        .post(format!("{base}/Users/AuthenticateByName"))
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(map_status(resp.status()));
    }

    let auth: AuthenticationResult = resp.json().await?;
    Ok((auth.access_token, auth.user.id))
}

/// Check whether the server has Quick Connect enabled.
///
/// `GET {base}/QuickConnect/Enabled` → bool.
pub async fn quick_connect_enabled(base_url: &str, device_id: &str) -> Result<bool, JellyfinError> {
    let base = base_url.trim_end_matches('/');
    let http = auth_client(device_id)?;

    let resp = http
        .get(format!("{base}/QuickConnect/Enabled"))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(map_status(resp.status()));
    }

    let enabled: bool = resp.json().await?;
    Ok(enabled)
}

/// Initiate a Quick Connect request.
///
/// `POST {base}/QuickConnect/Initiate` → `(code, secret)`.
pub async fn quick_connect_initiate(
    base_url: &str,
    device_id: &str,
) -> Result<(String, String), JellyfinError> {
    let base = base_url.trim_end_matches('/');
    let http = auth_client(device_id)?;

    let resp = http
        .post(format!("{base}/QuickConnect/Initiate"))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(map_status(resp.status()));
    }

    let result: QuickConnectResult = resp.json().await?;
    match (result.code, result.secret) {
        (Some(code), Some(secret)) => Ok((code, secret)),
        _ => Err(JellyfinError::Server {
            status: 0,
            message: "Quick Connect initiate returned no code/secret".to_string(),
        }),
    }
}

/// Poll the server until the Quick Connect request is authenticated, using the
/// production interval and timeout.
pub async fn quick_connect_poll(
    base_url: &str,
    device_id: &str,
    secret: &str,
) -> Result<(), JellyfinError> {
    let max_attempts = (POLL_TIMEOUT_SECS * 1000 / POLL_INTERVAL_MS) as usize;
    quick_connect_poll_with(base_url, device_id, secret, POLL_INTERVAL_MS, max_attempts).await
}

/// Poll loop with explicit interval and max attempts (so tests can short-circuit
/// the 5-minute production timeout). Loops `GET {base}/QuickConnect/Connect?
/// secret=` until `Authenticated == true`. Transient non-success responses are
/// retried. Times out with a 408 error after `max_attempts`.
pub(crate) async fn quick_connect_poll_with(
    base_url: &str,
    device_id: &str,
    secret: &str,
    interval_ms: u64,
    max_attempts: usize,
) -> Result<(), JellyfinError> {
    let base = base_url.trim_end_matches('/');
    let http = auth_client(device_id)?;
    let url = format!("{base}/QuickConnect/Connect");

    for _ in 0..max_attempts {
        tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;

        let resp = match http.get(&url).query(&[("secret", secret)]).send().await {
            Ok(r) => r,
            // Transient: keep polling.
            Err(e) if e.is_connect() || e.is_timeout() => continue,
            Err(e) => return Err(e.into()),
        };

        if !resp.status().is_success() {
            // Transient server hiccup; keep polling.
            continue;
        }

        let result: QuickConnectResult = resp.json().await?;
        if result.authenticated {
            return Ok(());
        }
    }

    Err(JellyfinError::Server {
        status: 408,
        message: "Quick Connect timed out. Please try again.".to_string(),
    })
}

/// Exchange an approved Quick Connect secret for a token.
///
/// `POST {base}/Users/AuthenticateWithQuickConnect` with body `{ "Secret" }`.
/// Returns `(access_token, user_id)`.
pub async fn authenticate_with_quick_connect(
    base_url: &str,
    device_id: &str,
    secret: &str,
) -> Result<(String, String), JellyfinError> {
    let base = base_url.trim_end_matches('/');
    let http = auth_client(device_id)?;
    let body = serde_json::json!({ "Secret": secret });

    let resp = http
        .post(format!("{base}/Users/AuthenticateWithQuickConnect"))
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(map_status(resp.status()));
    }

    let auth: AuthenticationResult = resp.json().await?;
    Ok((auth.access_token, auth.user.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_string_contains, header_exists, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn device_id_is_persistent() {
        let dir = tempfile::tempdir().unwrap();
        let id1 = device_id(&dir.path().to_path_buf());
        let id2 = device_id(&dir.path().to_path_buf());
        assert_eq!(id1, id2);
        assert!(id1.starts_with("reel-"));
    }

    #[test]
    fn device_id_generates_uuid_format() {
        let dir = tempfile::tempdir().unwrap();
        let id = device_id(&dir.path().to_path_buf());
        assert!(id.starts_with("reel-"));
        assert!(id.len() > 20);
        assert!(id.matches('-').count() >= 4);
    }

    #[test]
    fn device_id_persists_to_jellyfin_file() {
        let dir = tempfile::tempdir().unwrap();
        let id = device_id(&dir.path().to_path_buf());
        let stored = std::fs::read_to_string(dir.path().join("jellyfin_device_id")).unwrap();
        assert_eq!(stored.trim(), id);
    }

    #[test]
    fn device_auth_header_has_no_token() {
        let h = device_auth_header("dev123");
        assert!(h.starts_with("MediaBrowser"));
        assert!(h.contains("DeviceId=\"dev123\""));
        assert!(h.contains("Client=\"Reel\""));
        assert!(!h.contains("Token="));
    }

    #[tokio::test]
    async fn authenticate_by_name_returns_token_and_user() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/Users/AuthenticateByName"))
            .and(header_exists("Authorization"))
            // The field MUST be "Pw", not "Password".
            .and(body_string_contains("\"Pw\""))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"AccessToken":"tok","User":{"Id":"u1","Name":"alice"}}"#),
            )
            .mount(&server)
            .await;

        let (token, user_id) = authenticate_by_name(&server.uri(), "dev1", "alice", "secret")
            .await
            .unwrap();
        assert_eq!(token, "tok");
        assert_eq!(user_id, "u1");
    }

    #[tokio::test]
    async fn authenticate_by_name_401_surfaces_auth_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/Users/AuthenticateByName"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let result = authenticate_by_name(&server.uri(), "dev1", "alice", "bad").await;
        assert!(matches!(result, Err(JellyfinError::Unauthorized)));
    }

    #[tokio::test]
    async fn quick_connect_enabled_false_short_circuits() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/QuickConnect/Enabled"))
            .respond_with(ResponseTemplate::new(200).set_body_string("false"))
            .mount(&server)
            .await;

        let enabled = quick_connect_enabled(&server.uri(), "dev1").await.unwrap();
        assert!(!enabled);
    }

    #[tokio::test]
    async fn quick_connect_enabled_true() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/QuickConnect/Enabled"))
            .respond_with(ResponseTemplate::new(200).set_body_string("true"))
            .mount(&server)
            .await;

        let enabled = quick_connect_enabled(&server.uri(), "dev1").await.unwrap();
        assert!(enabled);
    }

    #[tokio::test]
    async fn quick_connect_initiate_parses_code_and_secret() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/QuickConnect/Initiate"))
            .and(header_exists("Authorization"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"Code":"123456","Secret":"sec","Authenticated":false}"#),
            )
            .mount(&server)
            .await;

        let (code, secret) = quick_connect_initiate(&server.uri(), "dev1").await.unwrap();
        assert_eq!(code, "123456");
        assert_eq!(secret, "sec");
    }

    #[tokio::test]
    async fn quick_connect_poll_succeeds_when_authenticated() {
        let server = MockServer::start().await;
        // First poll: not authenticated yet.
        Mock::given(method("GET"))
            .and(path("/QuickConnect/Connect"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"Code":"123456","Secret":"sec","Authenticated":false}"#),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;
        // Subsequent polls: authenticated.
        Mock::given(method("GET"))
            .and(path("/QuickConnect/Connect"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"Code":"123456","Secret":"sec","Authenticated":true}"#),
            )
            .mount(&server)
            .await;

        let result = quick_connect_poll_with(&server.uri(), "dev1", "sec", 50, 20).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn quick_connect_poll_times_out() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/QuickConnect/Connect"))
            .and(query_param("secret", "sec"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"Code":"123456","Secret":"sec","Authenticated":false}"#),
            )
            .mount(&server)
            .await;

        let result = quick_connect_poll_with(&server.uri(), "dev1", "sec", 10, 2).await;
        assert!(matches!(
            result,
            Err(JellyfinError::Server { status: 408, .. })
        ));
    }

    #[tokio::test]
    async fn authenticate_with_quick_connect_returns_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/Users/AuthenticateWithQuickConnect"))
            .and(header_exists("Authorization"))
            .and(body_string_contains("\"Secret\""))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"AccessToken":"tok2","User":{"Id":"u9","Name":"bob"}}"#),
            )
            .mount(&server)
            .await;

        let (token, user_id) = authenticate_with_quick_connect(&server.uri(), "dev1", "sec")
            .await
            .unwrap();
        assert_eq!(token, "tok2");
        assert_eq!(user_id, "u9");
    }
}
