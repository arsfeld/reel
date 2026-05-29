//! A lightweight fake Jellyfin server for end-to-end testing.
//!
//! Implements the subset of the Jellyfin 10.9+ API that Reel uses, backed by
//! in-memory state. Supports seeding data via a builder and records the
//! session/progress/played calls so tests can assert ordering.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Json;
use axum::routing::{get, post};
use serde_json::{Value, json};
use tokio::net::TcpListener;

// --- State ---

#[derive(Clone)]
struct AppState {
    inner: Arc<RwLock<JellyfinData>>,
}

struct JellyfinData {
    token: String,
    user_id: String,
    views: Vec<Value>,
    /// All known items by id (bare `BaseItemDto` JSON).
    items: HashMap<String, Value>,
    /// library_id -> item ids contained directly.
    library_items: HashMap<String, Vec<String>>,
    /// parent_id -> child item ids (seasons under a series, episodes under a
    /// season).
    children: HashMap<String, Vec<String>>,
    resume: Vec<String>,
    next_up: Vec<String>,
    latest: Vec<String>,
    /// item_id -> media segment JSON values.
    segments: HashMap<String, Vec<Value>>,
    /// Recorded POST/DELETE calls: (path, parsed-body) for assertions.
    recorded: Vec<(String, Value)>,
}

// --- Public API ---

pub struct FakeJellyfinServer {
    url: String,
    token: String,
    user_id: String,
    state: AppState,
    shutdown_tx: tokio::sync::watch::Sender<()>,
}

impl FakeJellyfinServer {
    pub fn builder() -> FakeJellyfinBuilder {
        FakeJellyfinBuilder::default()
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    /// Recorded session/progress/played calls in arrival order.
    pub fn recorded_calls(&self) -> Vec<(String, Value)> {
        self.state.inner.read().unwrap().recorded.clone()
    }
}

impl Drop for FakeJellyfinServer {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(());
    }
}

// --- Builder ---

#[derive(Default)]
pub struct FakeJellyfinBuilder {
    token: String,
    user_id: String,
    views: Vec<Value>,
    items: HashMap<String, Value>,
    library_items: HashMap<String, Vec<String>>,
    children: HashMap<String, Vec<String>>,
    resume: Vec<String>,
    next_up: Vec<String>,
    latest: Vec<String>,
    segments: HashMap<String, Vec<Value>>,
}

impl FakeJellyfinBuilder {
    pub fn token(mut self, token: &str) -> Self {
        self.token = token.to_string();
        self
    }

    pub fn user(mut self, user_id: &str) -> Self {
        self.user_id = user_id.to_string();
        self
    }

    /// Add a library view (`UserViews` entry).
    pub fn library(mut self, id: &str, name: &str, collection_type: &str) -> Self {
        self.views.push(json!({
            "Id": id,
            "Name": name,
            "CollectionType": collection_type,
            "Type": "CollectionFolder",
        }));
        self.library_items.entry(id.to_string()).or_default();
        self
    }

    /// Add an item directly under a library.
    pub fn item_in(mut self, library_id: &str, item: Value) -> Self {
        let id = item["Id"].as_str().expect("item must have Id").to_string();
        self.items.insert(id.clone(), item);
        self.library_items
            .entry(library_id.to_string())
            .or_default()
            .push(id);
        self
    }

    /// Add an item as a child of a parent (seasons under a series, episodes
    /// under a season).
    pub fn child_of(mut self, parent_id: &str, item: Value) -> Self {
        let id = item["Id"].as_str().expect("item must have Id").to_string();
        self.items.insert(id.clone(), item);
        self.children
            .entry(parent_id.to_string())
            .or_default()
            .push(id);
        self
    }

    /// Add an item to the Continue Watching (resume) list.
    pub fn resume_item(mut self, item: Value) -> Self {
        let id = item["Id"].as_str().expect("item must have Id").to_string();
        self.items.insert(id.clone(), item);
        self.resume.push(id);
        self
    }

    /// Add an item to the Next Up list.
    pub fn next_up_item(mut self, item: Value) -> Self {
        let id = item["Id"].as_str().expect("item must have Id").to_string();
        self.items.insert(id.clone(), item);
        self.next_up.push(id);
        self
    }

    /// Add an item to the Latest list.
    pub fn latest_item(mut self, item: Value) -> Self {
        let id = item["Id"].as_str().expect("item must have Id").to_string();
        self.items.insert(id.clone(), item);
        self.latest.push(id);
        self
    }

    /// Add a media segment for an item.
    pub fn segment(mut self, item_id: &str, segment: Value) -> Self {
        self.segments
            .entry(item_id.to_string())
            .or_default()
            .push(segment);
        self
    }

    pub async fn start(self) -> FakeJellyfinServer {
        let state = AppState {
            inner: Arc::new(RwLock::new(JellyfinData {
                token: self.token.clone(),
                user_id: self.user_id.clone(),
                views: self.views,
                items: self.items,
                library_items: self.library_items,
                children: self.children,
                resume: self.resume,
                next_up: self.next_up,
                latest: self.latest,
                segments: self.segments,
                recorded: Vec::new(),
            })),
        };

        let app = Router::new()
            .route("/UserViews", get(user_views_handler))
            .route("/Items", get(items_handler))
            .route("/Items/Latest", get(latest_handler))
            .route("/Items/{id}", get(item_handler))
            .route("/Shows/{id}/Seasons", get(seasons_handler))
            .route("/Shows/{id}/Episodes", get(episodes_handler))
            .route("/Shows/NextUp", get(next_up_handler))
            .route("/UserItems/Resume", get(resume_handler))
            .route("/MediaSegments/{id}", get(segments_handler))
            .route("/Sessions/Playing", post(session_playing_handler))
            .route("/Sessions/Playing/Progress", post(session_progress_handler))
            .route("/Sessions/Playing/Stopped", post(session_stopped_handler))
            .route(
                "/UserPlayedItems/{id}",
                post(played_post_handler).delete(played_delete_handler),
            )
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

        FakeJellyfinServer {
            url,
            token: self.token,
            user_id: self.user_id,
            state,
            shutdown_tx,
        }
    }
}

// --- Auth ---

/// Jellyfin sends `Authorization: MediaBrowser Token="...", Client=...`.
/// Extract the `Token="..."` value and compare against the expected token.
fn check_token(headers: &HeaderMap, expected: &str) -> Result<(), StatusCode> {
    let auth = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let token = auth
        .split("Token=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or("");
    if token == expected {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn query_result(items: Vec<&Value>) -> Json<Value> {
    let count = items.len();
    Json(json!({
        "Items": items,
        "TotalRecordCount": count,
    }))
}

// --- Handlers ---

async fn user_views_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let data = state.inner.read().unwrap();
    check_token(&headers, &data.token)?;
    let views: Vec<&Value> = data.views.iter().collect();
    Ok(query_result(views))
}

async fn items_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let data = state.inner.read().unwrap();
    check_token(&headers, &data.token)?;

    // Determine the candidate item ids: scoped to a parent (library or
    // collection) if `parentId` is set, otherwise all items.
    let candidate_ids: Vec<String> = match params.get("parentId") {
        Some(parent) => {
            let mut ids = data.library_items.get(parent).cloned().unwrap_or_default();
            ids.extend(data.children.get(parent).cloned().unwrap_or_default());
            ids
        }
        None => data.items.keys().cloned().collect(),
    };

    // Filter by includeItemTypes (comma-separated) if present.
    let type_filter: Option<Vec<String>> = params
        .get("includeItemTypes")
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect());

    let items: Vec<&Value> = candidate_ids
        .iter()
        .filter_map(|id| data.items.get(id))
        .filter(|item| match &type_filter {
            Some(types) => item["Type"]
                .as_str()
                .map(|t| types.iter().any(|f| f == t))
                .unwrap_or(false),
            None => true,
        })
        .collect();

    Ok(query_result(items))
}

async fn item_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let data = state.inner.read().unwrap();
    check_token(&headers, &data.token)?;
    let item = data.items.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    // Bare object, NOT wrapped in a QueryResult.
    Ok(Json(item.clone()))
}

async fn seasons_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let data = state.inner.read().unwrap();
    check_token(&headers, &data.token)?;
    let child_ids = data.children.get(&id).cloned().unwrap_or_default();
    let items: Vec<&Value> = child_ids
        .iter()
        .filter_map(|cid| data.items.get(cid))
        .filter(|item| item["Type"].as_str() == Some("Season"))
        .collect();
    Ok(query_result(items))
}

async fn episodes_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(_series_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let data = state.inner.read().unwrap();
    check_token(&headers, &data.token)?;
    // Episodes are children of the season (seasonId), honoring the query param.
    let season_id = params.get("seasonId").cloned().unwrap_or_default();
    let child_ids = data.children.get(&season_id).cloned().unwrap_or_default();
    let items: Vec<&Value> = child_ids
        .iter()
        .filter_map(|cid| data.items.get(cid))
        .filter(|item| item["Type"].as_str() == Some("Episode"))
        .collect();
    Ok(query_result(items))
}

async fn resume_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let data = state.inner.read().unwrap();
    check_token(&headers, &data.token)?;
    let items: Vec<&Value> = data
        .resume
        .iter()
        .filter_map(|id| data.items.get(id))
        .collect();
    Ok(query_result(items))
}

async fn next_up_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let data = state.inner.read().unwrap();
    check_token(&headers, &data.token)?;
    let items: Vec<&Value> = data
        .next_up
        .iter()
        .filter_map(|id| data.items.get(id))
        .collect();
    Ok(query_result(items))
}

async fn latest_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let data = state.inner.read().unwrap();
    check_token(&headers, &data.token)?;
    // Bare array, NOT a QueryResult.
    let items: Vec<&Value> = data
        .latest
        .iter()
        .filter_map(|id| data.items.get(id))
        .collect();
    Ok(Json(json!(items)))
}

async fn segments_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let data = state.inner.read().unwrap();
    check_token(&headers, &data.token)?;
    match data.segments.get(&id) {
        Some(segs) => Ok(query_result(segs.iter().collect())),
        // Items with no segments 404 (older servers / no plugin).
        None => Err(StatusCode::NOT_FOUND),
    }
}

fn record(state: &AppState, path: &str, body: Value) {
    state
        .inner
        .write()
        .unwrap()
        .recorded
        .push((path.to_string(), body));
}

async fn session_playing_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<StatusCode, StatusCode> {
    {
        let data = state.inner.read().unwrap();
        check_token(&headers, &data.token)?;
    }
    record(&state, "/Sessions/Playing", body);
    Ok(StatusCode::NO_CONTENT)
}

async fn session_progress_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<StatusCode, StatusCode> {
    {
        let data = state.inner.read().unwrap();
        check_token(&headers, &data.token)?;
    }
    record(&state, "/Sessions/Playing/Progress", body);
    Ok(StatusCode::NO_CONTENT)
}

async fn session_stopped_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<StatusCode, StatusCode> {
    {
        let data = state.inner.read().unwrap();
        check_token(&headers, &data.token)?;
    }
    record(&state, "/Sessions/Playing/Stopped", body);
    Ok(StatusCode::NO_CONTENT)
}

async fn played_post_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    {
        let data = state.inner.read().unwrap();
        check_token(&headers, &data.token)?;
    }
    record(
        &state,
        &format!("/UserPlayedItems/{id}"),
        json!({"played": true}),
    );
    Ok(Json(json!({})))
}

async fn played_delete_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    {
        let data = state.inner.read().unwrap();
        check_token(&headers, &data.token)?;
    }
    record(
        &state,
        &format!("/UserPlayedItems/{id}"),
        json!({"played": false}),
    );
    Ok(Json(json!({})))
}

// --- Metadata helpers (PascalCase Jellyfin JSON) ---

/// Create a movie `BaseItemDto`.
pub fn movie(id: &str, name: &str, year: i32) -> Value {
    json!({
        "Id": id,
        "Name": name,
        "Type": "Movie",
        "ProductionYear": year,
        "Overview": format!("Overview of {name}"),
        "OfficialRating": "PG-13",
        "CommunityRating": 7.5,
        "RunTimeTicks": 7_200_000_i64 * 10_000,
        "Genres": ["Drama"],
        "ImageTags": {"Primary": "tag"},
        "MediaSources": [{"Id": id}],
        "Height": 1080,
        "Width": 1920,
    })
}

/// Create a series `BaseItemDto`.
pub fn series(id: &str, name: &str, year: i32) -> Value {
    json!({
        "Id": id,
        "Name": name,
        "Type": "Series",
        "ProductionYear": year,
        "Overview": format!("Overview of {name}"),
        "OfficialRating": "TV-MA",
        "CommunityRating": 8.5,
        "Genres": ["Drama"],
        "ImageTags": {"Primary": "tag"},
    })
}

/// Create a season `BaseItemDto`.
pub fn season(id: &str, name: &str, index: i32, series_id: &str) -> Value {
    json!({
        "Id": id,
        "Name": name,
        "Type": "Season",
        "IndexNumber": index,
        "SeriesId": series_id,
        "ImageTags": {"Primary": "tag"},
    })
}

/// Create an episode `BaseItemDto`.
pub fn episode(
    id: &str,
    name: &str,
    season_num: i32,
    ep_num: i32,
    season_id: &str,
    series_id: &str,
) -> Value {
    json!({
        "Id": id,
        "Name": name,
        "Type": "Episode",
        "IndexNumber": ep_num,
        "ParentIndexNumber": season_num,
        "SeasonId": season_id,
        "SeriesId": series_id,
        "RunTimeTicks": 2_520_000_i64 * 10_000,
        "Overview": format!("Overview of {name}"),
        "MediaSources": [{"Id": id}],
        "ImageTags": {"Primary": "tag"},
    })
}

/// Create a box set (collection) `BaseItemDto`.
pub fn box_set(id: &str, name: &str) -> Value {
    json!({
        "Id": id,
        "Name": name,
        "Type": "BoxSet",
        "ImageTags": {"Primary": "tag"},
    })
}
