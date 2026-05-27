use std::path::PathBuf;

use tracing::info;

use crate::services::plex::auth;

use super::AppCmd;

/// Test the saved URL; if unreachable, re-discover the server via plex.tv.
pub async fn validate_or_rediscover_source(
    url: String,
    token: String,
    name: String,
    data_dir: PathBuf,
) -> AppCmd {
    info!("Validating saved Plex connection: {url}");

    // Quick connectivity test on the saved URL.
    // Use a short timeout — a reachable Plex server responds in <500ms.
    // If it takes longer, the URL is likely stale and we should re-discover.
    let Ok(http) = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(1))
        .timeout(std::time::Duration::from_secs(2))
        .danger_accept_invalid_certs(true)
        .build()
    else {
        return AppCmd::SourceValidationFailed("Failed to create HTTP client".into());
    };

    if http.get(format!("{url}/")).send().await.is_ok() {
        info!("Saved URL is reachable: {url}");
        return AppCmd::SourceValidated { url, token, name };
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
