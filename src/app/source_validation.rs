use std::path::PathBuf;

use tracing::info;

use crate::services::plex::auth;

use super::AppCmd;

/// Number of times to probe the saved URL before falling back to rediscovery.
const SAVED_URL_PROBE_ATTEMPTS: u32 = 3;
/// Delay between saved-URL probe attempts, to let the network warm up.
const SAVED_URL_PROBE_BACKOFF_MS: u64 = 400;

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
        .connect_timeout(std::time::Duration::from_secs(2))
        .timeout(std::time::Duration::from_secs(4))
        .danger_accept_invalid_certs(true)
        .build()
    else {
        return AppCmd::SourceValidationFailed("Failed to create HTTP client".into());
    };

    let probe = format!("{url}/");
    for attempt in 1..=SAVED_URL_PROBE_ATTEMPTS {
        if http.get(&probe).send().await.is_ok() {
            info!("Saved URL is reachable: {url}");
            return AppCmd::SourceValidated { url, token, name };
        }
        if attempt < SAVED_URL_PROBE_ATTEMPTS {
            info!(
                "Saved URL probe {attempt}/{SAVED_URL_PROBE_ATTEMPTS} failed ({url}); retrying..."
            );
            tokio::time::sleep(std::time::Duration::from_millis(SAVED_URL_PROBE_BACKOFF_MS)).await;
        }
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
        Some(new_url) => {
            info!("Re-discovered server '{}' at {new_url}", server.name);
            AppCmd::SourceValidated {
                url: new_url,
                token,
                name: server.name.clone(),
            }
        }
        None => AppCmd::SourceValidationFailed(format!(
            "Server '{}' found but no connections reachable",
            server.name
        )),
    }
}
