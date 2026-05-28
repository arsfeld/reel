use std::collections::HashMap;
use std::time::Instant;

use diesel::{Connection, SqliteConnection};
use tracing::{info, warn};

use crate::config;
use crate::db::init::init_db;
use crate::db::media_repo::MediaRepo;
use crate::db::watch_progress_repo::WatchProgressRepo;
use crate::models::media::MediaItem;
use crate::models::watch::WatchProgress;

pub fn init_database() -> Option<SqliteConnection> {
    let db_path = config::db_path();

    if let Some(parent) = db_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        warn!("Failed to create database directory: {e}");
        return None;
    }

    let path = db_path.to_string_lossy();
    match SqliteConnection::establish(&path) {
        Ok(mut conn) => {
            if let Err(e) = init_db(&mut conn) {
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
pub fn load_watch_data(db_conn: &mut Option<SqliteConnection>) -> HashMap<String, (f64, bool)> {
    let start = Instant::now();
    let mut map = HashMap::new();
    if let Some(conn) = db_conn.as_mut() {
        let mut repo = WatchProgressRepo::new(conn);
        // Load all in-progress items.
        if let Ok(items) = repo.list_in_progress(1000) {
            for item in &items {
                map.insert(
                    item.media_item_id.clone(),
                    (item.progress_fraction(), false),
                );
            }
        }
        // Watched items override in-progress with a full (1.0, true).
        if let Ok(watched) = repo.list_watched() {
            for wp in &watched {
                map.insert(wp.media_item_id.clone(), (1.0, true));
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
pub fn load_in_progress(db_conn: &mut Option<SqliteConnection>) -> Vec<(MediaItem, WatchProgress)> {
    let Some(conn) = db_conn.as_mut() else {
        return Vec::new();
    };

    let in_progress = match WatchProgressRepo::new(conn).list_in_progress(30) {
        Ok(items) => items,
        Err(e) => {
            warn!("Failed to load in-progress items: {e}");
            return Vec::new();
        }
    };

    let mut media_repo = MediaRepo::new(conn);
    let mut result = Vec::new();
    for wp in in_progress {
        if let Ok(Some(item)) = media_repo.find_by_id(&wp.media_item_id) {
            result.push((item, wp));
        }
    }
    result
}
