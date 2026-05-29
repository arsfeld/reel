use std::path::PathBuf;

use tracing::info;

use crate::models::media::SourceType;
use crate::models::source::Source;
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

/// Validate one saved source on startup, dispatching by source type. Each
/// source validates independently — a failure surfaces for *that* source only
/// (carrying its id) and never touches another source's registry entry or data.
pub async fn validate_source(source: Source, data_dir: PathBuf) -> AppCmd {
    match source.source_type {
        SourceType::Plex => validate_or_rediscover_plex(source, data_dir).await,
        // Jellyfin has no plex.tv-style broker: just probe the saved URL.
        SourceType::Jellyfin => probe_saved_url(source).await,
        SourceType::Local => AppCmd::SourceValidationFailed {
            source_id: source.id.clone(),
            message: "Local sources are not validated".into(),
        },
    }
}

/// Build a reqwest client for connectivity probing. Plex tolerates invalid
/// certs (plex.direct wildcard / self-signed); Jellyfin uses strict validation.
fn probe_client(accept_invalid_certs: bool) -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(5))
        .danger_accept_invalid_certs(accept_invalid_certs)
        .build()
        .ok()
}

/// Probe a source's saved URL across the warm-up window. `Some(())` once any
/// HTTP response comes back (even 401 — the endpoint is reachable).
async fn probe_reachable(http: &reqwest::Client, url: &str) -> bool {
    let probe = format!("{url}/");
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(SAVED_URL_PROBE_WINDOW_SECS);
    let mut attempt = 0;
    loop {
        attempt += 1;
        if http.get(&probe).send().await.is_ok() {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        info!("Saved URL probe {attempt} failed ({url}); retrying while the network warms up...");
        tokio::time::sleep(std::time::Duration::from_millis(SAVED_URL_PROBE_BACKOFF_MS)).await;
    }
}

/// Jellyfin (and any non-Plex networked source): probe the saved URL only.
async fn probe_saved_url(source: Source) -> AppCmd {
    info!(
        "Validating saved {:?} connection: {}",
        source.source_type, source.config.url
    );
    let Some(http) = probe_client(false) else {
        return AppCmd::SourceValidationFailed {
            source_id: source.id.clone(),
            message: "Failed to create HTTP client".into(),
        };
    };
    if probe_reachable(&http, &source.config.url).await {
        info!("Saved URL is reachable: {}", source.config.url);
        let original_id = source.id.clone();
        // Non-Plex source: is_remote is irrelevant (only Plex applies the cap).
        AppCmd::SourceValidated {
            source,
            original_id,
            is_remote: false,
        }
    } else {
        AppCmd::SourceValidationFailed {
            source_id: source.id.clone(),
            message: format!("Server unreachable: {}", source.config.url),
        }
    }
}

/// Plex: test the saved URL; if unreachable, re-discover the server via plex.tv.
/// A rediscovered URL produces a `Source` with the new url/id; `original_id`
/// always carries the pre-validation id so the persistence upsert can clear the
/// stale row if the URL changed.
async fn validate_or_rediscover_plex(source: Source, data_dir: PathBuf) -> AppCmd {
    let original_id = source.id.clone();
    let token = source.config.token.clone();
    let name = source.name.clone();
    info!("Validating saved Plex connection: {}", source.config.url);

    if let Some(http) = probe_client(true)
        && probe_reachable(&http, &source.config.url).await
    {
        info!("Saved URL is reachable: {}", source.config.url);
        // Known limitation (U2/KTD6): the saved-URL fast path only probes
        // reachability — no plex.tv connection metadata — so it cannot classify
        // local vs remote. Default to `is_remote = false` (no cap), preserving
        // today's behavior. A genuinely remote user reaches the cap via
        // rediscovery (below) or the manual quality override (R10).
        return AppCmd::SourceValidated {
            source,
            original_id,
            is_remote: false,
        };
    }

    info!(
        "Saved URL unreachable ({}), re-discovering server...",
        source.config.url
    );

    let client_id = auth::client_identifier(&data_dir);
    let servers = match auth::discover_servers(&client_id, &token).await {
        Ok(s) => s,
        Err(e) => {
            return AppCmd::SourceValidationFailed {
                source_id: original_id,
                message: format!("Discovery failed: {e}"),
            };
        }
    };

    let server = servers.iter().find(|s| s.name == name).or(servers.first());
    let Some(server) = server else {
        return AppCmd::SourceValidationFailed {
            source_id: original_id,
            message: "No servers found on account".to_string(),
        };
    };

    match auth::best_server_uri(server).await {
        Some(selected) => {
            info!(
                "Re-discovered server '{}' at {} (remote={})",
                server.name, selected.uri, selected.is_remote
            );
            let rediscovered = Source {
                id: Source::make_id(SourceType::Plex, &selected.uri),
                source_type: SourceType::Plex,
                name: server.name.clone(),
                config: crate::models::source::SourceConfig {
                    url: selected.uri,
                    token,
                    user_id: None,
                },
                enabled: true,
                last_synced_at: source.last_synced_at,
            };
            AppCmd::SourceValidated {
                source: rediscovered,
                original_id,
                is_remote: selected.is_remote,
            }
        }
        None => AppCmd::SourceValidationFailed {
            source_id: original_id,
            message: format!(
                "Server '{}' found but no connections reachable",
                server.name
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::source::SourceConfig;

    #[tokio::test]
    async fn validate_local_source_is_unsupported() {
        // The Local branch returns synchronously with no I/O, carrying the
        // source's own id so a failure never affects another source.
        let source = Source {
            id: "local:/media".into(),
            source_type: SourceType::Local,
            name: "Local".into(),
            config: SourceConfig {
                url: "/media".into(),
                token: String::new(),
                user_id: None,
            },
            enabled: true,
            last_synced_at: None,
        };
        let cmd = validate_source(source, std::path::PathBuf::from("/tmp")).await;
        match cmd {
            AppCmd::SourceValidationFailed { source_id, .. } => {
                assert_eq!(source_id, "local:/media");
            }
            _ => panic!("expected SourceValidationFailed for a Local source"),
        }
    }
}
