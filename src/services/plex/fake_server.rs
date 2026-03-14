//! A lightweight fake Plex Media Server for end-to-end testing.
//!
//! Implements the subset of the Plex API that Reel uses, backed by
//! in-memory state. Supports seeding data via a builder and mutating
//! state at runtime for incremental sync testing.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use axum::Router;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::routing::get;
use serde_json::{Value, json};
use tokio::net::TcpListener;

// --- State ---

#[derive(Clone)]
struct AppState {
    inner: Arc<RwLock<PlexData>>,
}

struct PlexData {
    server_name: String,
    token: String,
    libraries: Vec<Value>,
    metadata: HashMap<String, Value>,
    library_items: HashMap<String, Vec<String>>,
    children_map: HashMap<String, Vec<String>>,
}

// --- Public API ---

pub struct FakePlexServer {
    url: String,
    token: String,
    state: AppState,
    shutdown_tx: tokio::sync::watch::Sender<()>,
}

impl FakePlexServer {
    pub fn builder() -> FakePlexBuilder {
        FakePlexBuilder::default()
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    /// Add a new item to a library at runtime (for incremental sync tests).
    pub fn add_item(&self, library_key: &str, item: Value) {
        let key = item["ratingKey"]
            .as_str()
            .expect("item must have ratingKey")
            .to_string();
        let mut data = self.state.inner.write().unwrap();
        data.metadata.insert(key.clone(), item);
        data.library_items
            .entry(library_key.to_string())
            .or_default()
            .push(key);
    }

    /// Remove an item by rating key.
    pub fn remove_item(&self, rating_key: &str) {
        let mut data = self.state.inner.write().unwrap();
        data.metadata.remove(rating_key);
        for items in data.library_items.values_mut() {
            items.retain(|k| k != rating_key);
        }
        data.children_map.remove(rating_key);
        for children in data.children_map.values_mut() {
            children.retain(|k| k != rating_key);
        }
    }
}

impl Drop for FakePlexServer {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(());
    }
}

// --- Builder ---

#[derive(Default)]
pub struct FakePlexBuilder {
    server_name: String,
    token: String,
    libraries: Vec<Value>,
    metadata: HashMap<String, Value>,
    library_items: HashMap<String, Vec<String>>,
    children_map: HashMap<String, Vec<String>>,
}

impl FakePlexBuilder {
    pub fn server_name(mut self, name: &str) -> Self {
        self.server_name = name.to_string();
        self
    }

    pub fn token(mut self, token: &str) -> Self {
        self.token = token.to_string();
        self
    }

    pub fn library(mut self, key: &str, title: &str, lib_type: &str) -> Self {
        self.libraries.push(json!({
            "key": key,
            "title": title,
            "type": lib_type,
            "count": 0
        }));
        self.library_items.entry(key.to_string()).or_default();
        self
    }

    /// Add a metadata item to a library.
    pub fn item_in(mut self, library_key: &str, item: Value) -> Self {
        let key = item["ratingKey"]
            .as_str()
            .expect("item must have ratingKey")
            .to_string();
        self.metadata.insert(key.clone(), item);
        self.library_items
            .entry(library_key.to_string())
            .or_default()
            .push(key);
        self
    }

    /// Add a metadata item as a child of another item.
    pub fn child_of(mut self, parent_key: &str, item: Value) -> Self {
        let key = item["ratingKey"]
            .as_str()
            .expect("item must have ratingKey")
            .to_string();
        self.metadata.insert(key.clone(), item);
        self.children_map
            .entry(parent_key.to_string())
            .or_default()
            .push(key);
        self
    }

    pub async fn start(mut self) -> FakePlexServer {
        // Update library counts
        for lib in &mut self.libraries {
            let key = lib["key"].as_str().unwrap().to_string();
            let count = self.library_items.get(&key).map_or(0, |v| v.len());
            lib["count"] = json!(count);
        }

        let state = AppState {
            inner: Arc::new(RwLock::new(PlexData {
                server_name: self.server_name,
                token: self.token.clone(),
                libraries: self.libraries,
                metadata: self.metadata,
                library_items: self.library_items,
                children_map: self.children_map,
            })),
        };

        let app = Router::new()
            .route("/", get(root_handler))
            .route("/library/sections", get(sections_handler))
            .route("/library/sections/{key}/all", get(library_items_handler))
            .route("/library/metadata/{key}", get(metadata_handler))
            .route("/library/metadata/{key}/children", get(children_handler))
            .with_state(state.clone());

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://127.0.0.1:{}", addr.port());

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(());

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    shutdown_rx.changed().await.ok();
                })
                .await
                .unwrap();
        });

        FakePlexServer {
            url,
            token: self.token,
            state,
            shutdown_tx,
        }
    }
}

// --- Handlers ---

fn check_token(headers: &HeaderMap, expected: &str) -> Result<(), StatusCode> {
    let token = headers
        .get("X-Plex-Token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if token == expected {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn root_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let data = state.inner.read().unwrap();
    check_token(&headers, &data.token)?;
    Ok(Json(json!({
        "MediaContainer": {
            "friendlyName": data.server_name
        }
    })))
}

async fn sections_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let data = state.inner.read().unwrap();
    check_token(&headers, &data.token)?;
    Ok(Json(json!({
        "MediaContainer": {
            "size": data.libraries.len(),
            "Directory": data.libraries
        }
    })))
}

async fn library_items_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let data = state.inner.read().unwrap();
    check_token(&headers, &data.token)?;

    let keys = data.library_items.get(&key).ok_or(StatusCode::NOT_FOUND)?;
    let items: Vec<&Value> = keys.iter().filter_map(|k| data.metadata.get(k)).collect();

    Ok(Json(json!({
        "MediaContainer": {
            "size": items.len(),
            "Metadata": items
        }
    })))
}

async fn metadata_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let data = state.inner.read().unwrap();
    check_token(&headers, &data.token)?;

    let item = data.metadata.get(&key).ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(json!({
        "MediaContainer": {
            "size": 1,
            "Metadata": [item]
        }
    })))
}

async fn children_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let data = state.inner.read().unwrap();
    check_token(&headers, &data.token)?;

    let child_keys = data.children_map.get(&key).cloned().unwrap_or_default();
    let items: Vec<&Value> = child_keys
        .iter()
        .filter_map(|k| data.metadata.get(k))
        .collect();

    Ok(Json(json!({
        "MediaContainer": {
            "size": items.len(),
            "Metadata": items
        }
    })))
}

// --- Metadata helpers ---

/// Create a movie metadata entry with sensible defaults.
pub fn movie(key: &str, title: &str, year: i32) -> Value {
    json!({
        "ratingKey": key,
        "title": title,
        "type": "movie",
        "year": year,
        "summary": format!("Summary of {title}"),
        "contentRating": "PG-13",
        "rating": 7.5,
        "duration": 7_200_000_i64,
        "thumb": format!("/library/metadata/{key}/thumb/1"),
        "art": format!("/library/metadata/{key}/art/1"),
        "addedAt": 1_705_276_800_i64,
        "updatedAt": 1_705_276_800_i64,
        "Genre": [{"tag": "Drama"}],
        "Media": [{
            "Part": [{
                "key": format!("/library/parts/{key}/1/file.mkv"),
                "file": format!("/data/movies/{title}.mkv"),
                "size": 5_000_000_000_i64
            }]
        }]
    })
}

/// Create a movie with custom genres, duration, and rating.
pub fn movie_detailed(
    key: &str,
    title: &str,
    year: i32,
    genres: &[&str],
    duration_min: i32,
    rating: f64,
) -> Value {
    let mut m = movie(key, title, year);
    m["duration"] = json!(i64::from(duration_min) * 60_000);
    m["rating"] = json!(rating);
    m["Genre"] = json!(genres.iter().map(|g| json!({"tag": g})).collect::<Vec<_>>());
    m
}

/// Create a TV show metadata entry.
pub fn show(key: &str, title: &str, year: i32) -> Value {
    json!({
        "ratingKey": key,
        "title": title,
        "type": "show",
        "year": year,
        "summary": format!("Summary of {title}"),
        "contentRating": "TV-MA",
        "rating": 8.5,
        "thumb": format!("/library/metadata/{key}/thumb/1"),
        "art": format!("/library/metadata/{key}/art/1"),
        "addedAt": 1_705_276_800_i64,
        "updatedAt": 1_705_276_800_i64,
        "Genre": [{"tag": "Drama"}]
    })
}

/// Create a season metadata entry.
pub fn season(key: &str, title: &str, index: i32, parent_key: &str) -> Value {
    json!({
        "ratingKey": key,
        "title": title,
        "type": "season",
        "index": index,
        "parentRatingKey": parent_key,
        "thumb": format!("/library/metadata/{key}/thumb/1"),
        "addedAt": 1_705_276_800_i64,
        "updatedAt": 1_705_276_800_i64
    })
}

/// Create an episode metadata entry.
pub fn episode(
    key: &str,
    title: &str,
    season_num: i32,
    episode_num: i32,
    parent_key: &str,
    grandparent_key: &str,
) -> Value {
    json!({
        "ratingKey": key,
        "title": title,
        "type": "episode",
        "parentIndex": season_num,
        "index": episode_num,
        "parentRatingKey": parent_key,
        "grandparentRatingKey": grandparent_key,
        "duration": 2_520_000_i64,
        "summary": format!("Summary of {title}"),
        "thumb": format!("/library/metadata/{key}/thumb/1"),
        "originallyAvailableAt": "2024-03-01",
        "addedAt": 1_705_276_800_i64,
        "updatedAt": 1_705_276_800_i64,
        "Media": [{
            "Part": [{
                "key": format!("/library/parts/{key}/1/file.mkv"),
                "file": format!("/data/tv/S{season_num:02}E{episode_num:02}.mkv"),
                "size": 2_000_000_000_i64
            }]
        }]
    })
}
