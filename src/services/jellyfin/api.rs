use reqwest::header::{HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;

use super::error::JellyfinError;
use super::models::*;

const APP_VERSION: &str = "0.1.0";

/// HTTP client for the Jellyfin server API.
///
/// Targets Jellyfin 10.9+ `userId`-as-query routes exclusively (the
/// `/Users/{userId}/...` family is removed in 10.11).
pub struct JellyfinClient {
    http: reqwest::Client,
    base_url: String,
    token: String,
    user_id: String,
    device_id: String,
    /// Whether the active connection is remote. Session-only (never persisted);
    /// playback reads this to apply the Auto default bitrate cap on remote
    /// connections (R3). Defaults to `false`; set via [`Self::with_remote`].
    is_remote: bool,
}

impl JellyfinClient {
    pub fn new(base_url: &str, token: &str, user_id: &str, device_id: &str) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();

        let mut default_headers = HeaderMap::new();
        default_headers.insert("Accept", HeaderValue::from_static("application/json"));

        // Jellyfin auth header: a single MediaBrowser value carrying the token
        // and client identity.
        let authorization = format!(
            "MediaBrowser Token=\"{token}\", Client=\"Reel\", Device=\"Reel\", DeviceId=\"{device_id}\", Version=\"{APP_VERSION}\""
        );
        if let Ok(value) = HeaderValue::from_str(&authorization) {
            default_headers.insert("Authorization", value);
        }

        // Unlike Plex, Jellyfin uses strict certificate validation (reqwest
        // defaults). Do NOT accept invalid certs here.
        let http = reqwest::Client::builder()
            .default_headers(default_headers)
            .connect_timeout(std::time::Duration::from_secs(3))
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("TLS backend initialization failed");

        Self {
            http,
            base_url,
            token: token.to_string(),
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            is_remote: false,
        }
    }

    /// Record whether the selected server connection is remote. Builder style so
    /// existing `JellyfinClient::new` call sites are unaffected (mirrors
    /// `PlexClient::with_remote`; session-only, set at construction).
    pub fn with_remote(mut self, is_remote: bool) -> Self {
        self.is_remote = is_remote;
        self
    }

    /// Whether the active connection is remote (R3). Playback reads this to
    /// decide whether to apply the Auto default transcode bitrate cap.
    pub fn is_remote(&self) -> bool {
        self.is_remote
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// The shared HTTP client (carries the default Jellyfin auth header). Used by
    /// the transcode module for the single-attempt decision call and the
    /// session lifecycle requests, which bypass the retrying `request_factory`.
    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// GET a URL, retrying transient connection/timeout errors with a short
    /// backoff. HTTP status errors (401/404/5xx) are returned immediately —
    /// they are not transient.
    async fn get(&self, url: &str) -> Result<reqwest::Response, JellyfinError> {
        self.request(self.http.get(url)).await
    }

    /// POST a JSON body, applying the same retry/status handling as `get`.
    async fn post_json<B: serde::Serialize>(
        &self,
        url: &str,
        body: &B,
    ) -> Result<reqwest::Response, JellyfinError> {
        let payload = serde_json::to_vec(body)?;
        self.request_factory(|| {
            self.http
                .post(url)
                .header("Content-Type", "application/json")
                .body(payload.clone())
        })
        .await
    }

    /// POST with no body (used for endpoints that take only query params).
    async fn post_empty(&self, url: &str) -> Result<reqwest::Response, JellyfinError> {
        self.request_factory(|| self.http.post(url).body(Vec::<u8>::new()))
            .await
    }

    /// DELETE a URL, applying the same retry/status handling as `get`.
    async fn delete(&self, url: &str) -> Result<reqwest::Response, JellyfinError> {
        self.request_factory(|| self.http.delete(url)).await
    }

    /// Send a prebuilt request builder, retrying transient errors. Used for
    /// GET where the builder is cheap to construct once.
    async fn request(
        &self,
        builder: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, JellyfinError> {
        // A RequestBuilder can only be sent once; clone for retries.
        self.request_factory(|| {
            builder
                .try_clone()
                .expect("request body is not a stream and can be cloned")
        })
        .await
    }

    /// Core retry loop. `make` produces a fresh request builder per attempt.
    async fn request_factory<F>(&self, make: F) -> Result<reqwest::Response, JellyfinError>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        const ATTEMPTS: u32 = 3;
        const BACKOFF_MS: u64 = 600;
        let mut last_err: Option<reqwest::Error> = None;
        for attempt in 1..=ATTEMPTS {
            match make().send().await {
                Ok(resp) => {
                    Self::check_status(&resp)?;
                    return Ok(resp);
                }
                Err(e) if e.is_connect() || e.is_timeout() => {
                    tracing::debug!(
                        "Jellyfin request transient error (attempt {attempt}/{ATTEMPTS}): {e}"
                    );
                    last_err = Some(e);
                    if attempt < ATTEMPTS {
                        tokio::time::sleep(std::time::Duration::from_millis(BACKOFF_MS)).await;
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }
        Err(last_err
            .map(JellyfinError::from)
            .unwrap_or_else(|| JellyfinError::Server {
                status: 0,
                message: "request failed".into(),
            }))
    }

    fn check_status(resp: &reqwest::Response) -> Result<(), JellyfinError> {
        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        match status.as_u16() {
            401 => Err(JellyfinError::Unauthorized),
            404 => Err(JellyfinError::NotFound("Resource not found".to_string())),
            s => Err(JellyfinError::Server {
                status: s,
                message: status
                    .canonical_reason()
                    .unwrap_or("Unknown error")
                    .to_string(),
            }),
        }
    }

    /// Standard `fields` list so Jellyfin returns the metadata we map. Jellyfin
    /// omits many fields unless explicitly requested.
    fn item_fields() -> &'static str {
        "Overview,Genres,People,MediaSources,ProductionYear,PremiereDate,ParentId"
    }

    // --- Library / item browsing ---

    /// Library views (the user's libraries). `GET /UserViews?userId=`.
    pub async fn user_views(&self) -> Result<Vec<BaseItemDto>, JellyfinError> {
        let url = format!("{}/UserViews?userId={}", self.base_url, self.user_id);
        let resp = self.get(&url).await?;
        let body: QueryResult<BaseItemDto> = resp.json().await?;
        Ok(body.items)
    }

    /// List items, optionally scoped to a parent / item types / recursive.
    /// `GET /Items?userId=`.
    pub async fn items(
        &self,
        parent_id: Option<&str>,
        include_item_types: Option<&str>,
        recursive: bool,
    ) -> Result<Vec<BaseItemDto>, JellyfinError> {
        let mut url = format!(
            "{}/Items?userId={}&fields={}",
            self.base_url,
            self.user_id,
            Self::item_fields()
        );
        if let Some(pid) = parent_id {
            url.push_str(&format!("&parentId={pid}"));
        }
        if let Some(types) = include_item_types {
            url.push_str(&format!("&includeItemTypes={types}"));
        }
        if recursive {
            url.push_str("&recursive=true");
        }
        let resp = self.get(&url).await?;
        let body: QueryResult<BaseItemDto> = resp.json().await?;
        Ok(body.items)
    }

    /// Get a single item. `GET /Items/{id}?userId=` returns a bare
    /// `BaseItemDto` (not wrapped in a `QueryResult`).
    pub async fn item(&self, item_id: &str) -> Result<BaseItemDto, JellyfinError> {
        let url = format!(
            "{}/Items/{}?userId={}&fields={}",
            self.base_url,
            item_id,
            self.user_id,
            Self::item_fields()
        );
        let resp = self.get(&url).await?;
        let body: BaseItemDto = resp.json().await?;
        Ok(body)
    }

    /// Seasons of a series. `GET /Shows/{seriesId}/Seasons?userId=`.
    pub async fn seasons(&self, series_id: &str) -> Result<Vec<BaseItemDto>, JellyfinError> {
        let url = format!(
            "{}/Shows/{}/Seasons?userId={}",
            self.base_url, series_id, self.user_id
        );
        let resp = self.get(&url).await?;
        let body: QueryResult<BaseItemDto> = resp.json().await?;
        Ok(body.items)
    }

    /// Episodes of a season within a series.
    /// `GET /Shows/{seriesId}/Episodes?userId=&seasonId=`.
    pub async fn episodes(
        &self,
        series_id: &str,
        season_id: &str,
    ) -> Result<Vec<BaseItemDto>, JellyfinError> {
        let url = format!(
            "{}/Shows/{}/Episodes?userId={}&seasonId={}",
            self.base_url, series_id, self.user_id, season_id
        );
        let resp = self.get(&url).await?;
        let body: QueryResult<BaseItemDto> = resp.json().await?;
        Ok(body.items)
    }

    /// Continue Watching. `GET /UserItems/Resume?userId=`.
    pub async fn resume_items(&self) -> Result<Vec<BaseItemDto>, JellyfinError> {
        let url = format!("{}/UserItems/Resume?userId={}", self.base_url, self.user_id);
        let resp = self.get(&url).await?;
        let body: QueryResult<BaseItemDto> = resp.json().await?;
        Ok(body.items)
    }

    /// Next Up across the user's shows. `GET /Shows/NextUp?userId=`.
    pub async fn next_up(&self) -> Result<Vec<BaseItemDto>, JellyfinError> {
        let url = format!("{}/Shows/NextUp?userId={}", self.base_url, self.user_id);
        let resp = self.get(&url).await?;
        let body: QueryResult<BaseItemDto> = resp.json().await?;
        Ok(body.items)
    }

    /// Latest items, optionally within a parent library. `GET /Items/Latest`
    /// returns a bare JSON array (NOT a `QueryResult`).
    pub async fn latest(&self, parent_id: Option<&str>) -> Result<Vec<BaseItemDto>, JellyfinError> {
        let mut url = format!("{}/Items/Latest?userId={}", self.base_url, self.user_id);
        if let Some(pid) = parent_id {
            url.push_str(&format!("&parentId={pid}"));
        }
        let resp = self.get(&url).await?;
        let body: Vec<BaseItemDto> = resp.json().await?;
        Ok(body)
    }

    /// Media segments (intro/outro/etc) for an item.
    /// `GET /MediaSegments/{itemId}`. On 404 (older servers without the Media
    /// Segments API) this degrades gracefully to an empty list.
    pub async fn media_segments(
        &self,
        item_id: &str,
    ) -> Result<Vec<MediaSegmentDto>, JellyfinError> {
        let url = format!("{}/MediaSegments/{}", self.base_url, item_id);
        match self.get(&url).await {
            Ok(resp) => {
                let body: QueryResult<MediaSegmentDto> = resp.json().await?;
                Ok(body.items)
            }
            Err(JellyfinError::NotFound(_)) => Ok(vec![]),
            Err(e) => Err(e),
        }
    }

    // --- Session / progress reporting (POST) ---

    /// Report that playback has started. `POST /Sessions/Playing`.
    pub async fn report_playing(
        &self,
        item_id: &str,
        play_session_id: &str,
        position_ticks: i64,
        play_method: &str,
    ) -> Result<(), JellyfinError> {
        let url = format!("{}/Sessions/Playing", self.base_url);
        let body = serde_json::json!({
            "ItemId": item_id,
            "PlaySessionId": play_session_id,
            "PositionTicks": position_ticks,
            "PlayMethod": play_method,
            "CanSeek": true,
        });
        self.post_json(&url, &body).await?;
        Ok(())
    }

    /// Report ongoing playback progress. `POST /Sessions/Playing/Progress`. Also
    /// keeps a transcode session alive server-side (Jellyfin pings the encoder on
    /// progress), so `play_method` must be `"Transcode"` for transcoded playback
    /// or the server rejects the report.
    pub async fn report_progress(
        &self,
        item_id: &str,
        play_session_id: &str,
        position_ticks: i64,
        is_paused: bool,
        play_method: &str,
    ) -> Result<(), JellyfinError> {
        let url = format!("{}/Sessions/Playing/Progress", self.base_url);
        let body = serde_json::json!({
            "ItemId": item_id,
            "PlaySessionId": play_session_id,
            "PositionTicks": position_ticks,
            "IsPaused": is_paused,
            "PlayMethod": play_method,
            "EventName": "timeupdate",
        });
        self.post_json(&url, &body).await?;
        Ok(())
    }

    /// Report that playback has stopped. `POST /Sessions/Playing/Stopped`.
    pub async fn report_stopped(
        &self,
        item_id: &str,
        play_session_id: &str,
        position_ticks: i64,
    ) -> Result<(), JellyfinError> {
        let url = format!("{}/Sessions/Playing/Stopped", self.base_url);
        let body = serde_json::json!({
            "ItemId": item_id,
            "PlaySessionId": play_session_id,
            "PositionTicks": position_ticks,
        });
        self.post_json(&url, &body).await?;
        Ok(())
    }

    /// Keep a transcode session alive (R5). `POST /Sessions/Playing/Ping`.
    /// Single short attempt — a missed ping is recovered by the next keepalive
    /// tick. Bypasses the retrying `request_factory` (lifecycle, not hot-path).
    pub async fn ping_session(&self, play_session_id: &str) -> Result<(), JellyfinError> {
        let url = format!(
            "{}/Sessions/Playing/Ping?playSessionId={}",
            self.base_url, play_session_id
        );
        let resp = self
            .http
            .post(&url)
            .timeout(std::time::Duration::from_secs(3))
            .body(Vec::<u8>::new())
            .send()
            .await?;
        Self::check_status(&resp)
    }

    /// Kill the active transcode encoder for a session (R5).
    /// `DELETE /Videos/ActiveEncodings?deviceId=&playSessionId=`. Both params are
    /// required (jellyfin-kodi#557). A 404/204 means the session already ended —
    /// treated as success.
    pub async fn stop_active_encoding(&self, play_session_id: &str) -> Result<(), JellyfinError> {
        let url = format!(
            "{}/Videos/ActiveEncodings?deviceId={}&playSessionId={}",
            self.base_url, self.device_id, play_session_id
        );
        const ATTEMPTS: u32 = 3;
        let mut last: Option<JellyfinError> = None;
        for _ in 0..ATTEMPTS {
            match self
                .http
                .delete(&url)
                .timeout(std::time::Duration::from_secs(3))
                .send()
                .await
            {
                Ok(resp) => {
                    let s = resp.status();
                    if s.is_success() || s.as_u16() == 404 {
                        return Ok(());
                    }
                    last = Some(JellyfinError::Server {
                        status: s.as_u16(),
                        message: "stop encoding failed".into(),
                    });
                }
                Err(e) => last = Some(e.into()),
            }
        }
        Err(last.unwrap_or(JellyfinError::Server {
            status: 0,
            message: "stop encoding failed".into(),
        }))
    }

    /// Mark an item played. `POST /UserPlayedItems/{itemId}?userId=`.
    pub async fn mark_played(&self, item_id: &str) -> Result<(), JellyfinError> {
        let url = format!(
            "{}/UserPlayedItems/{}?userId={}",
            self.base_url, item_id, self.user_id
        );
        self.post_empty(&url).await?;
        Ok(())
    }

    /// Mark an item unplayed. `DELETE /UserPlayedItems/{itemId}?userId=`.
    pub async fn mark_unplayed(&self, item_id: &str) -> Result<(), JellyfinError> {
        let url = format!(
            "{}/UserPlayedItems/{}?userId={}",
            self.base_url, item_id, self.user_id
        );
        self.delete(&url).await?;
        Ok(())
    }

    // --- URL builders (pure) ---

    /// Direct-play stream URL for a media source.
    pub fn stream_url(&self, item_id: &str, media_source_id: &str) -> String {
        format!(
            "{}/Videos/{}/stream?static=true&mediaSourceId={}&api_key={}",
            self.base_url, item_id, media_source_id, self.token
        )
    }

    /// Image URL for an item. `image_type` is e.g. "Primary", "Backdrop",
    /// "Logo". Appends `tag`, `fillWidth`/`fillHeight`, and `api_key` when set.
    pub fn image_url(
        &self,
        item_id: &str,
        image_type: &str,
        tag: Option<&str>,
        max_width: Option<u32>,
        max_height: Option<u32>,
    ) -> String {
        let mut url = format!("{}/Items/{}/Images/{}", self.base_url, item_id, image_type);
        let mut params: Vec<String> = Vec::new();
        if let Some(t) = tag {
            params.push(format!("tag={t}"));
        }
        if let Some(w) = max_width {
            params.push(format!("fillWidth={w}"));
        }
        if let Some(h) = max_height {
            params.push(format!("fillHeight={h}"));
        }
        params.push(format!("api_key={}", self.token));
        url.push('?');
        url.push_str(&params.join("&"));
        url
    }

    // Suppress unused warning for the generic JSON helper if not yet wired.
    #[allow(dead_code)]
    async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, JellyfinError> {
        let resp = self.get(url).await?;
        Ok(resp.json().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header_exists, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(uri: &str) -> JellyfinClient {
        JellyfinClient::new(uri, "test-token", "user1", "device1")
    }

    #[test]
    fn base_url_strips_trailing_slash() {
        let c = JellyfinClient::new("http://localhost:8096/", "t", "u", "d");
        assert_eq!(c.base_url(), "http://localhost:8096");
    }

    #[test]
    fn stream_url_appends_api_key_and_static() {
        let c = client("http://localhost:8096");
        let url = c.stream_url("item9", "src5");
        assert!(url.contains("/Videos/item9/stream"));
        assert!(url.contains("static=true"));
        assert!(url.contains("mediaSourceId=src5"));
        assert!(url.contains("api_key=test-token"));
    }

    #[test]
    fn image_url_includes_tag_and_fill() {
        let c = client("http://localhost:8096");
        let url = c.image_url("item9", "Primary", Some("tagX"), Some(300), Some(450));
        assert!(url.contains("/Items/item9/Images/Primary"));
        assert!(url.contains("tag=tagX"));
        assert!(url.contains("fillWidth=300"));
        assert!(url.contains("fillHeight=450"));
        assert!(url.contains("api_key=test-token"));
    }

    #[test]
    fn ticks_to_ms_and_secs() {
        assert_eq!(Ticks(10_000_000).to_secs(), 1.0);
        assert_eq!(Ticks(10_000_000).to_ms(), 1000);
    }

    #[tokio::test]
    async fn client_sends_authorization_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/UserViews"))
            .and(header_exists("Authorization"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"Items":[],"TotalRecordCount":0}"#),
            )
            .mount(&server)
            .await;

        let c = client(&server.uri());
        let result = c.user_views().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn user_views_parses_query_result() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/UserViews"))
            .and(query_param("userId", "user1"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"Items":[
                    {"Id":"v1","Name":"Movies","CollectionType":"movies"},
                    {"Id":"v2","Name":"Shows","CollectionType":"tvshows"}
                ],"TotalRecordCount":2}"#,
            ))
            .mount(&server)
            .await;

        let c = client(&server.uri());
        let views = c.user_views().await.unwrap();
        assert_eq!(views.len(), 2);
        assert_eq!(views[0].name.as_deref(), Some("Movies"));
        assert_eq!(views[0].collection_type.as_deref(), Some("movies"));
    }

    #[tokio::test]
    async fn items_parses_movies() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/Items"))
            .and(query_param("includeItemTypes", "Movie"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"Items":[
                    {"Id":"m1","Name":"Dune","Type":"Movie","ProductionYear":2021}
                ],"TotalRecordCount":1}"#,
            ))
            .mount(&server)
            .await;

        let c = client(&server.uri());
        let items = c.items(Some("v1"), Some("Movie"), true).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name.as_deref(), Some("Dune"));
        assert_eq!(items[0].production_year, Some(2021));
    }

    #[tokio::test]
    async fn items_parses_episodes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/Items"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"Items":[
                    {"Id":"e1","Name":"Pilot","Type":"Episode","IndexNumber":1,"ParentIndexNumber":2,"SeriesId":"s1"}
                ],"TotalRecordCount":1}"#,
            ))
            .mount(&server)
            .await;

        let c = client(&server.uri());
        let items = c.items(None, Some("Episode"), true).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].type_.as_deref(), Some("Episode"));
        assert_eq!(items[0].index_number, Some(1));
        assert_eq!(items[0].parent_index_number, Some(2));
        assert_eq!(items[0].series_id.as_deref(), Some("s1"));
    }

    #[tokio::test]
    async fn item_parses_bare_object() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/Items/m1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"Id":"m1","Name":"Dune","Type":"Movie"}"#),
            )
            .mount(&server)
            .await;

        let c = client(&server.uri());
        let item = c.item("m1").await.unwrap();
        assert_eq!(item.id, "m1");
        assert_eq!(item.name.as_deref(), Some("Dune"));
    }

    #[tokio::test]
    async fn latest_parses_bare_array() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/Items/Latest"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"[{"Id":"a","Name":"A","Type":"Movie"},{"Id":"b","Name":"B","Type":"Movie"}]"#,
            ))
            .mount(&server)
            .await;

        let c = client(&server.uri());
        let items = c.latest(Some("v1")).await.unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[1].id, "b");
    }

    #[tokio::test]
    async fn unauthorized_maps_to_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/UserViews"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let c = client(&server.uri());
        let result = c.user_views().await;
        assert!(matches!(result, Err(JellyfinError::Unauthorized)));
    }

    #[tokio::test]
    async fn not_found_maps_to_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/Items/missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let c = client(&server.uri());
        let result = c.item("missing").await;
        assert!(matches!(result, Err(JellyfinError::NotFound(_))));
    }

    #[tokio::test]
    async fn server_error_maps() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/UserViews"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let c = client(&server.uri());
        let result = c.user_views().await;
        assert!(matches!(
            result,
            Err(JellyfinError::Server { status: 500, .. })
        ));
    }

    #[tokio::test]
    async fn malformed_json_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/UserViews"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let c = client(&server.uri());
        let result = c.user_views().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn media_segments_404_degrades_to_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/MediaSegments/item9"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let c = client(&server.uri());
        let segments = c.media_segments("item9").await.unwrap();
        assert!(segments.is_empty());
    }

    #[tokio::test]
    async fn media_segments_parses_query_result() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/MediaSegments/item9"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"Items":[{"Id":"seg1","Type":"Intro","StartTicks":0,"EndTicks":3000000}],"TotalRecordCount":1}"#,
            ))
            .mount(&server)
            .await;

        let c = client(&server.uri());
        let segments = c.media_segments("item9").await.unwrap();
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].type_.as_deref(), Some("Intro"));
    }

    #[tokio::test]
    async fn report_progress_posts_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/Sessions/Playing/Progress"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let c = client(&server.uri());
        let result = c
            .report_progress("item9", "session1", 10_000_000, false, "DirectPlay")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn report_playing_posts_to_sessions_playing() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/Sessions/Playing"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let c = client(&server.uri());
        let result = c.report_playing("item9", "session1", 0, "DirectPlay").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn report_stopped_posts_to_sessions_playing_stopped() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/Sessions/Playing/Stopped"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let c = client(&server.uri());
        let result = c.report_stopped("item9", "session1", 20_000_000).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mark_played_posts() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/UserPlayedItems/item9"))
            .and(query_param("userId", "user1"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .mount(&server)
            .await;

        let c = client(&server.uri());
        let result = c.mark_played("item9").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mark_unplayed_deletes() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/UserPlayedItems/item9"))
            .and(query_param("userId", "user1"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .mount(&server)
            .await;

        let c = client(&server.uri());
        let result = c.mark_unplayed("item9").await;
        assert!(result.is_ok());
    }
}
