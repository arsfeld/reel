use reqwest::header::{HeaderMap, HeaderValue};

use super::error::PlexError;
use super::models::*;

/// HTTP client for the Plex Media Server API.
pub struct PlexClient {
    http: reqwest::Client,
    base_url: String,
    auth_token: String,
}

impl PlexClient {
    pub fn new(base_url: &str, auth_token: &str) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();

        let mut default_headers = HeaderMap::new();
        default_headers.insert("Accept", HeaderValue::from_static("application/json"));
        default_headers.insert("X-Plex-Product", HeaderValue::from_static("Reel"));
        default_headers.insert("X-Plex-Version", HeaderValue::from_static("0.1.0"));
        default_headers.insert(
            "X-Plex-Client-Identifier",
            HeaderValue::from_static("reel-media-player"),
        );
        if let Ok(token) = HeaderValue::from_str(auth_token) {
            default_headers.insert("X-Plex-Token", token);
        }

        // reqwest Client::builder().build() only fails if the TLS backend
        // cannot initialize, which indicates a broken system configuration.
        //
        // Accept invalid TLS certs — Plex servers use self-signed or
        // plex.direct wildcard certificates whose OCSP/CRL validation
        // can take seconds. This is standard practice for Plex clients.
        let http = reqwest::Client::builder()
            .default_headers(default_headers)
            .danger_accept_invalid_certs(true)
            .connect_timeout(std::time::Duration::from_secs(3))
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("TLS backend initialization failed");

        Self {
            http,
            base_url,
            auth_token: auth_token.to_string(),
        }
    }

    /// Test the connection to the Plex server.
    pub async fn test_connection(&self) -> Result<String, PlexError> {
        let url = format!("{}/", self.base_url);
        let resp = self.http.get(&url).send().await?;
        Self::check_status(&resp)?;
        // Return the server name from the response
        let body: serde_json::Value = resp.json().await?;
        let name = body["MediaContainer"]["friendlyName"]
            .as_str()
            .unwrap_or("Plex Server")
            .to_string();
        Ok(name)
    }

    /// Get all library sections.
    pub async fn libraries(&self) -> Result<Vec<PlexLibrary>, PlexError> {
        let start = std::time::Instant::now();
        let url = format!("{}/library/sections", self.base_url);
        let resp = self.http.get(&url).send().await?;
        Self::check_status(&resp)?;
        let body: PlexLibraryResponse = resp.json().await?;
        tracing::info!(
            "PlexClient::libraries() -> {} sections in {:?}",
            body.media_container.directories.len(),
            start.elapsed()
        );
        Ok(body.media_container.directories)
    }

    /// Get all items in a library section.
    pub async fn library_items(&self, library_key: &str) -> Result<Vec<PlexMetadata>, PlexError> {
        let start = std::time::Instant::now();
        let url = format!("{}/library/sections/{}/all", self.base_url, library_key);
        let resp = self.http.get(&url).send().await?;
        let http_elapsed = start.elapsed();
        Self::check_status(&resp)?;
        let body: PlexMetadataResponse = resp.json().await?;
        tracing::info!(
            "PlexClient::library_items(key={}) -> {} items (http: {:?}, total: {:?})",
            library_key,
            body.media_container.metadata.len(),
            http_elapsed,
            start.elapsed()
        );
        Ok(body.media_container.metadata)
    }

    /// Get metadata for a specific item.
    pub async fn metadata(&self, rating_key: &str) -> Result<PlexMetadata, PlexError> {
        let url = format!("{}/library/metadata/{}", self.base_url, rating_key);
        let resp = self.http.get(&url).send().await?;
        Self::check_status(&resp)?;
        let body: PlexMetadataResponse = resp.json().await?;
        body.media_container
            .metadata
            .into_iter()
            .next()
            .ok_or_else(|| PlexError::NotFound(format!("Item {rating_key} not found")))
    }

    /// Get children of an item (seasons of a show, episodes of a season).
    pub async fn children(&self, rating_key: &str) -> Result<Vec<PlexMetadata>, PlexError> {
        let url = format!("{}/library/metadata/{}/children", self.base_url, rating_key);
        let resp = self.http.get(&url).send().await?;
        Self::check_status(&resp)?;
        let body: PlexMetadataResponse = resp.json().await?;
        Ok(body.media_container.metadata)
    }

    /// Get chapter markers for a media item.
    pub async fn chapters(&self, rating_key: &str) -> Result<Vec<PlexChapter>, PlexError> {
        let url = format!("{}/library/metadata/{}/chapters", self.base_url, rating_key);
        let resp = self.http.get(&url).send().await?;
        Self::check_status(&resp)?;
        let body: PlexChapterResponse = resp.json().await?;
        Ok(body.media_container.chapters)
    }

    /// Get all collections in a library section.
    pub async fn collections(&self, library_key: &str) -> Result<Vec<PlexMetadata>, PlexError> {
        let url = format!(
            "{}/library/sections/{}/collections",
            self.base_url, library_key
        );
        let resp = self.http.get(&url).send().await?;
        Self::check_status(&resp)?;
        let body: PlexMetadataResponse = resp.json().await?;
        Ok(body.media_container.metadata)
    }

    /// Get items in a collection.
    pub async fn collection_items(
        &self,
        collection_key: &str,
    ) -> Result<Vec<PlexMetadata>, PlexError> {
        let url = format!(
            "{}/library/collections/{}/children",
            self.base_url, collection_key
        );
        let resp = self.http.get(&url).send().await?;
        Self::check_status(&resp)?;
        let body: PlexMetadataResponse = resp.json().await?;
        Ok(body.media_container.metadata)
    }

    /// Build a direct play URL for a media part.
    ///
    /// `download=1` is required: without it the `/library/parts/.../file.ext`
    /// endpoint returns HTTP 500. With it, Plex serves the raw file with
    /// `Accept-Ranges: bytes` so the player can seek.
    pub fn playback_url(&self, part_key: &str) -> String {
        format!(
            "{}{}?download=1&X-Plex-Token={}",
            self.base_url, part_key, self.auth_token
        )
    }

    /// Build a transcoded image URL.
    pub fn transcode_image_url(&self, path: &str, width: u32, height: u32) -> String {
        format!(
            "{}/photo/:/transcode?width={}&height={}&minSize=1&url={}&X-Plex-Token={}",
            self.base_url, width, height, path, self.auth_token
        )
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Report playback timeline to the Plex server.
    /// Call every ~10 seconds during playback, and on state changes.
    pub async fn report_timeline(
        &self,
        rating_key: &str,
        state: &str,
        time_ms: i64,
        duration_ms: i64,
    ) -> Result<(), PlexError> {
        let url = format!(
            "{}/:/timeline?ratingKey={}&key=/library/metadata/{}&state={}&time={}&duration={}&identifier=com.plexapp.plugins.library",
            self.base_url, rating_key, rating_key, state, time_ms, duration_ms
        );
        let resp = self.http.get(&url).send().await?;
        Self::check_status(&resp)?;
        Ok(())
    }

    /// Mark an item as watched.
    pub async fn scrobble(&self, rating_key: &str) -> Result<(), PlexError> {
        let url = format!(
            "{}/:/scrobble?key={}&identifier=com.plexapp.plugins.library",
            self.base_url, rating_key
        );
        let resp = self.http.get(&url).send().await?;
        Self::check_status(&resp)?;
        Ok(())
    }

    /// Mark an item as unwatched.
    pub async fn unscrobble(&self, rating_key: &str) -> Result<(), PlexError> {
        let url = format!(
            "{}/:/unscrobble?key={}&identifier=com.plexapp.plugins.library",
            self.base_url, rating_key
        );
        let resp = self.http.get(&url).send().await?;
        Self::check_status(&resp)?;
        Ok(())
    }

    /// Get on-deck (continue watching) items.
    pub async fn on_deck(&self) -> Result<Vec<PlexMetadata>, PlexError> {
        let url = format!("{}/library/onDeck", self.base_url);
        let resp = self.http.get(&url).send().await?;
        Self::check_status(&resp)?;
        let body: PlexMetadataResponse = resp.json().await?;
        Ok(body.media_container.metadata)
    }

    /// Get recently added items across all libraries.
    pub async fn recently_added(&self) -> Result<Vec<PlexMetadata>, PlexError> {
        let url = format!("{}/library/recentlyAdded", self.base_url);
        let resp = self.http.get(&url).send().await?;
        Self::check_status(&resp)?;
        let body: PlexMetadataResponse = resp.json().await?;
        Ok(body.media_container.metadata)
    }

    /// Get the server's home hubs — curated rows (Continue Watching, On Deck,
    /// Recently Added per library, Recommended, "Because you watched", genre
    /// rows) computed server-side.
    pub async fn hubs(&self) -> Result<Vec<PlexHub>, PlexError> {
        let url = format!("{}/hubs", self.base_url);
        let resp = self.http.get(&url).send().await?;
        Self::check_status(&resp)?;
        let body: PlexHubResponse = resp.json().await?;
        Ok(body.media_container.hubs)
    }

    fn check_status(resp: &reqwest::Response) -> Result<(), PlexError> {
        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        match status.as_u16() {
            401 => Err(PlexError::Unauthorized),
            404 => Err(PlexError::NotFound("Resource not found".to_string())),
            s => Err(PlexError::Server {
                status: s,
                message: status
                    .canonical_reason()
                    .unwrap_or("Unknown error")
                    .to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_url_format() {
        let client = PlexClient::new("http://192.168.1.100:32400", "my-token");
        let url = client.playback_url("/library/parts/456/file.mkv");
        assert_eq!(
            url,
            "http://192.168.1.100:32400/library/parts/456/file.mkv?download=1&X-Plex-Token=my-token"
        );
    }

    #[test]
    fn playback_url_strips_trailing_slash_from_base() {
        let client = PlexClient::new("http://localhost:32400/", "token");
        let url = client.playback_url("/library/parts/1/file.mkv");
        assert!(url.starts_with("http://localhost:32400/library/parts/"));
        assert!(!url.contains("//library"));
    }

    #[test]
    fn transcode_image_url_format() {
        let client = PlexClient::new("http://localhost:32400", "my-token");
        let url = client.transcode_image_url("/library/metadata/123/thumb/1234", 300, 450);
        assert!(url.contains("width=300"));
        assert!(url.contains("height=450"));
        assert!(url.contains("X-Plex-Token=my-token"));
        assert!(url.contains("/photo/:/transcode"));
    }

    #[test]
    fn base_url_getter() {
        let client = PlexClient::new("http://localhost:32400/", "token");
        assert_eq!(client.base_url(), "http://localhost:32400");
    }

    // --- Async tests with wiremock ---

    #[tokio::test]
    async fn test_connection_success() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_string(r#"{"MediaContainer":{"friendlyName":"Test Server"}}"#),
            )
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "test-token");
        let name = client.test_connection().await.unwrap();
        assert_eq!(name, "Test Server");
    }

    #[tokio::test]
    async fn test_connection_unauthorized() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/"))
            .respond_with(wiremock::ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "bad-token");
        let result = client.test_connection().await;
        assert!(matches!(result, Err(PlexError::Unauthorized)));
    }

    #[tokio::test]
    async fn libraries_returns_parsed_data() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/library/sections"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"MediaContainer":{"size":2,"Directory":[
                    {"key":"1","title":"Movies","type":"movie","count":500},
                    {"key":"2","title":"TV Shows","type":"show","count":100}
                ]}}"#,
            ))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let libs = client.libraries().await.unwrap();
        assert_eq!(libs.len(), 2);
        assert_eq!(libs[0].title, "Movies");
        assert_eq!(libs[1].title, "TV Shows");
    }

    #[tokio::test]
    async fn library_items_returns_metadata() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/library/sections/1/all"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"MediaContainer":{"size":1,"Metadata":[
                    {"ratingKey":"123","title":"Dune","type":"movie","year":2021}
                ]}}"#,
            ))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let items = client.library_items("1").await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Dune");
    }

    #[tokio::test]
    async fn metadata_returns_single_item() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/library/metadata/123"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"MediaContainer":{"size":1,"Metadata":[
                    {"ratingKey":"123","title":"Dune","type":"movie"}
                ]}}"#,
            ))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let item = client.metadata("123").await.unwrap();
        assert_eq!(item.title, "Dune");
    }

    #[tokio::test]
    async fn metadata_not_found_returns_error() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/library/metadata/999"))
            .respond_with(wiremock::ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let result = client.metadata("999").await;
        assert!(matches!(result, Err(PlexError::NotFound(_))));
    }

    #[tokio::test]
    async fn children_returns_list() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/library/metadata/300/children"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"MediaContainer":{"size":2,"Metadata":[
                    {"ratingKey":"400","title":"Season 1","type":"season","index":1,"parentRatingKey":"300"},
                    {"ratingKey":"401","title":"Season 2","type":"season","index":2,"parentRatingKey":"300"}
                ]}}"#,
            ))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let children = client.children("300").await.unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].title, "Season 1");
    }

    #[tokio::test]
    async fn server_error_returns_error() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/library/sections"))
            .respond_with(wiremock::ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let result = client.libraries().await;
        assert!(matches!(result, Err(PlexError::Server { status: 500, .. })));
    }

    #[tokio::test]
    async fn plex_headers_included_in_requests() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/"))
            .and(wiremock::matchers::header("X-Plex-Token", "my-token"))
            .and(wiremock::matchers::header("X-Plex-Product", "Reel"))
            .and(wiremock::matchers::header("Accept", "application/json"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_string(r#"{"MediaContainer":{"friendlyName":"Test"}}"#),
            )
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "my-token");
        let result = client.test_connection().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn malformed_json_returns_error() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/library/sections"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("not json at all"))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let result = client.libraries().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn collections_returns_collection_metadata() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/library/sections/1/collections"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"MediaContainer":{"size":2,"Metadata":[
                    {"ratingKey":"50","title":"Dune Collection","type":"collection","thumb":"/thumb/50"},
                    {"ratingKey":"51","title":"Marvel","type":"collection"}
                ]}}"#,
            ))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let cols = client.collections("1").await.unwrap();
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[0].title, "Dune Collection");
        assert_eq!(cols[0].metadata_type, "collection");
        assert_eq!(cols[1].title, "Marvel");
    }

    #[tokio::test]
    async fn collection_items_returns_children() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/library/collections/50/children"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"MediaContainer":{"size":2,"Metadata":[
                    {"ratingKey":"123","title":"Dune","type":"movie","year":2021},
                    {"ratingKey":"124","title":"Dune: Part Two","type":"movie","year":2024}
                ]}}"#,
            ))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let items = client.collection_items("50").await.unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Dune");
        assert_eq!(items[1].title, "Dune: Part Two");
    }

    #[tokio::test]
    async fn collections_empty_returns_empty_vec() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/library/sections/1/collections"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_string(r#"{"MediaContainer":{"size":0}}"#),
            )
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let cols = client.collections("1").await.unwrap();
        assert!(cols.is_empty());
    }

    #[tokio::test]
    async fn report_timeline_sends_correct_params() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path_regex("/:/timeline.*"))
            .and(wiremock::matchers::query_param("ratingKey", "123"))
            .and(wiremock::matchers::query_param("state", "playing"))
            .and(wiremock::matchers::query_param("time", "45000"))
            .and(wiremock::matchers::query_param("duration", "7200000"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let result = client
            .report_timeline("123", "playing", 45000, 7200000)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn scrobble_calls_correct_endpoint() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path_regex("/:/scrobble.*"))
            .and(wiremock::matchers::query_param("key", "123"))
            .and(wiremock::matchers::query_param(
                "identifier",
                "com.plexapp.plugins.library",
            ))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let result = client.scrobble("123").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn unscrobble_calls_correct_endpoint() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path_regex("/:/unscrobble.*"))
            .and(wiremock::matchers::query_param("key", "123"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let result = client.unscrobble("123").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn on_deck_returns_items_with_view_offset() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/library/onDeck"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"MediaContainer":{"size":1,"Metadata":[
                    {"ratingKey":"123","title":"Dune","type":"movie","viewOffset":2700000,"duration":7200000}
                ]}}"#,
            ))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let items = client.on_deck().await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Dune");
        assert_eq!(items[0].view_offset, Some(2700000));
    }

    #[tokio::test]
    async fn scrobble_unauthorized_returns_error() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path_regex("/:/scrobble.*"))
            .respond_with(wiremock::ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "bad-token");
        let result = client.scrobble("123").await;
        assert!(matches!(result, Err(PlexError::Unauthorized)));
    }
}
