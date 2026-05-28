use std::collections::HashMap;
use std::time::Instant;

use rusqlite::Connection;
use tracing::{info, warn};

use crate::config;
use crate::db;
use crate::db::media_repo::MediaRepo;
use crate::db::watch_progress_repo::WatchProgressRepo;
use crate::models::media::MediaItem;
use crate::models::watch::WatchProgress;

/// Reclaim streaming-cache temp files orphaned by a previous crash or unclean
/// exit. A clean stop removes its own temp file via `downloadbuffer`'s
/// `temp-remove=true` default; this blunt sweep of the whole stream-cache dir
/// is the backstop for crashes. Failures are logged inside `reclaim`, never
/// fatal — a cache sweep must not block startup. Call before any playback
/// pipeline can begin writing a new temp file.
pub fn reclaim_stream_cache() {
    crate::services::stream_cache::reclaim(&config::stream_cache_dir());
}

pub fn init_database() -> Option<Connection> {
    let db_path = config::db_path();

    if let Some(parent) = db_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        warn!("Failed to create database directory: {e}");
        return None;
    }

    match Connection::open(&db_path) {
        Ok(conn) => {
            if let Err(e) = db::init_db(&conn) {
                warn!("Failed to initialize database: {e}");
                return None;
            }
            info!("Database initialized at {}", db_path.display());
            Some(conn)
        }
        Err(e) => {
            warn!("Failed to open database: {e}");
            None
        }
    }
}

/// Load all watch progress from DB into a HashMap for the library view.
pub fn load_watch_data(db_conn: &Option<Connection>) -> HashMap<String, (f64, bool)> {
    let start = Instant::now();
    let mut map = HashMap::new();
    if let Some(conn) = db_conn {
        let repo = WatchProgressRepo::new(conn);
        // Load all in-progress items
        if let Ok(items) = repo.list_in_progress(1000) {
            for item in &items {
                map.insert(
                    item.media_item_id.clone(),
                    (item.progress_fraction(), false),
                );
            }
        }
        // Also query watched items (where watched = 1)
        let mut stmt = conn
            .prepare(
                "SELECT media_item_id, position_seconds, duration_seconds FROM watch_progress WHERE watched = 1",
            )
            .ok();
        if let Some(ref mut stmt) = stmt
            && let Ok(rows) = stmt.query_map([], |row| {
                let id: String = row.get(0)?;
                Ok(id)
            })
        {
            for row in rows.flatten() {
                map.insert(row, (1.0, true));
            }
        }
    }
    info!(
        "load_watch_data: {} entries in {:?}",
        map.len(),
        start.elapsed()
    );
    map
}

/// Query the local database for in-progress items (Continue Watching).
pub fn load_in_progress(db_conn: &Option<Connection>) -> Vec<(MediaItem, WatchProgress)> {
    let Some(conn) = db_conn else {
        return Vec::new();
    };

    let watch_repo = WatchProgressRepo::new(conn);
    let in_progress = match watch_repo.list_in_progress(30) {
        Ok(items) => items,
        Err(e) => {
            warn!("Failed to load in-progress items: {e}");
            return Vec::new();
        }
    };

    let media_repo = MediaRepo::new(conn);
    let mut result = Vec::new();
    for wp in in_progress {
        if let Ok(Some(item)) = media_repo.find_by_id(&wp.media_item_id) {
            result.push((item, wp));
        }
    }
    result
}
