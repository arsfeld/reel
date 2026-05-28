use std::path::PathBuf;

use tracing::info;

use crate::services::plex::auth;

use super::AppCmd;

/// How long to keep probing the saved URL before falling back to the much
/// slower plex.tv rediscovery. A cold start (uncached *.plex.direct DNS, a
/// LAN/VPN route still coming up) can leave every endpoint unreachable for the
/// first several seconds after launch — and rediscovery typically just re-finds
/// the saved URL anyway. Probing across a generous window catches the URL the
/// moment the network warms up and skips the redundant round trip.
const SAVED_URL_PROBE_WINDOW_SECS: u64 = 12;
/// Delay between saved-URL probe attempts, to let the network warm up.
const SAVED_URL_PROBE_BACKOFF_MS: u64 = 600;

/// Test the saved URL; if unreachable, re-discover the server via plex.tv.
pub async fn validate_or_rediscover_source(
    url: String,
    token: String,
    name: String,
    data_dir: PathBuf,
) -> AppCmd {
    info!("Validating saved Plex connection: {url}");

    // Connectivity test on the saved URL. The timeout is generous enough to
    // absorb a cold start (uncached DNS for *.plex.direct, a LAN/VPN route that
    // is not up yet) so we don't fall back to slow rediscovery for a URL that is
    // actually fine. We probe a few times with a short backoff because the
    // first attempt right after launch often fails while the network warms up.
    let Ok(http) = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .build()
    else {
        return AppCmd::SourceValidationFailed("Failed to create HTTP client".into());
    };

    let probe = format!("{url}/");
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(SAVED_URL_PROBE_WINDOW_SECS);
    let mut attempt = 0;
    loop {
        attempt += 1;
        // Any HTTP response (even 401) means the endpoint is reachable.
        if http.get(&probe).send().await.is_ok() {
            info!("Saved URL is reachable: {url}");
            // Known limitation (U2/KTD6): this fast path only probes
            // reachability and has no plex.tv connection metadata, so it cannot
            // classify the saved URL as local vs remote. Default to
            // `is_remote = false` (no bitrate cap), preserving today's
            // direct-play-first behavior — not a regression. A genuinely remote
            // user reaches the cap via rediscovery (below) or the manual quality
            // override (R10).
            return AppCmd::SourceValidated {
                url,
                token,
                name,
                is_remote: false,
            };
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        info!("Saved URL probe {attempt} failed ({url}); retrying while the network warms up...");
        tokio::time::sleep(std::time::Duration::from_millis(SAVED_URL_PROBE_BACKOFF_MS)).await;
    }

    info!("Saved URL unreachable ({url}), re-discovering server...");

    let client_id = auth::client_identifier(&data_dir);
    let servers = match auth::discover_servers(&client_id, &token).await {
        Ok(s) => s,
        Err(e) => {
            return AppCmd::SourceValidationFailed(format!("Discovery failed: {e}"));
        }
    };

    // Find the server by name, or take the first one
    let server = servers.iter().find(|s| s.name == name).or(servers.first());

    let Some(server) = server else {
        return AppCmd::SourceValidationFailed("No servers found on account".to_string());
    };

    match auth::best_server_uri(server).await {
        Some(selected) => {
            info!(
                "Re-discovered server '{}' at {} (remote={})",
                server.name, selected.uri, selected.is_remote
            );
            AppCmd::SourceValidated {
                url: selected.uri,
                token,
                name: server.name.clone(),
                is_remote: selected.is_remote,
            }
        }
        None => AppCmd::SourceValidationFailed(format!(
            "Server '{}' found but no connections reachable",
            server.name
        )),
    }
}
