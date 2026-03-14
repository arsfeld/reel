use async_trait::async_trait;

use crate::models::{
    library::LibrarySection,
    media::{MediaItem, SourceType},
};
use crate::services::media_source::{MediaSource, SourceError};

use super::api::PlexClient;
use super::convert::{plex_library_to_section, plex_metadata_to_media_item};
use super::error::PlexError;

/// MediaSource implementation backed by a Plex Media Server.
pub struct PlexSource {
    client: PlexClient,
    name: String,
}

impl PlexSource {
    pub fn new(client: PlexClient, name: String) -> Self {
        Self { client, name }
    }
}

impl From<PlexError> for SourceError {
    fn from(e: PlexError) -> Self {
        match e {
            PlexError::Unauthorized => SourceError::Auth("Invalid or expired Plex token".into()),
            PlexError::NotFound(msg) => SourceError::NotFound(msg),
            PlexError::Http(e) => SourceError::Connection(e.to_string()),
            PlexError::Deserialize(e) => SourceError::Other(format!("Parse error: {e}")),
            PlexError::Server { status, message } => {
                SourceError::Other(format!("Server error {status}: {message}"))
            }
        }
    }
}

#[async_trait]
impl MediaSource for PlexSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn source_type(&self) -> SourceType {
        SourceType::Plex
    }

    async fn test_connection(&self) -> Result<String, SourceError> {
        Ok(self.client.test_connection().await?)
    }

    async fn libraries(&self) -> Result<Vec<LibrarySection>, SourceError> {
        let plex_libs = self.client.libraries().await?;
        Ok(plex_libs
            .iter()
            .filter_map(plex_library_to_section)
            .collect())
    }

    async fn library_items(&self, library_key: &str) -> Result<Vec<MediaItem>, SourceError> {
        let metadata = self.client.library_items(library_key).await?;
        let base_url = self.client.base_url();
        Ok(metadata
            .iter()
            .filter_map(|m| plex_metadata_to_media_item(m, base_url))
            .collect())
    }

    async fn metadata(&self, rating_key: &str) -> Result<MediaItem, SourceError> {
        let plex_meta = self.client.metadata(rating_key).await?;
        let base_url = self.client.base_url();
        plex_metadata_to_media_item(&plex_meta, base_url)
            .ok_or_else(|| SourceError::Other("Failed to convert metadata".into()))
    }

    async fn children(&self, rating_key: &str) -> Result<Vec<MediaItem>, SourceError> {
        let plex_children = self.client.children(rating_key).await?;
        let base_url = self.client.base_url();
        Ok(plex_children
            .iter()
            .filter_map(|m| plex_metadata_to_media_item(m, base_url))
            .collect())
    }

    fn playback_url(&self, part_key: &str) -> String {
        self.client.playback_url(part_key)
    }

    fn artwork_url(&self, path: &str, width: u32, height: u32) -> String {
        self.client.transcode_image_url(path, width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn plex_source_test_connection() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_string(r#"{"MediaContainer":{"friendlyName":"My Server"}}"#),
            )
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let source = PlexSource::new(client, "Test".into());

        let name = source.test_connection().await.unwrap();
        assert_eq!(name, "My Server");
    }

    #[tokio::test]
    async fn plex_source_libraries_filters_unsupported_types() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/library/sections"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"MediaContainer":{"size":3,"Directory":[
                    {"key":"1","title":"Movies","type":"movie","count":500},
                    {"key":"2","title":"TV Shows","type":"show","count":100},
                    {"key":"3","title":"Music","type":"artist","count":200}
                ]}}"#,
            ))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "token");
        let source = PlexSource::new(client, "Test".into());

        let libs = source.libraries().await.unwrap();
        assert_eq!(libs.len(), 2); // Music filtered out
    }

    #[tokio::test]
    async fn plex_source_library_items_converts_to_media_items() {
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
        let source = PlexSource::new(client, "Test".into());

        let items = source.library_items("1").await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Dune");
        assert_eq!(items[0].media_type, crate::models::media::MediaType::Movie);
    }

    #[tokio::test]
    async fn plex_source_unauthorized_maps_to_auth_error() {
        let server = wiremock::MockServer::start().await;

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/"))
            .respond_with(wiremock::ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let client = PlexClient::new(&server.uri(), "bad-token");
        let source = PlexSource::new(client, "Test".into());

        let result = source.test_connection().await;
        assert!(matches!(result, Err(SourceError::Auth(_))));
    }

    #[test]
    fn plex_source_name_and_type() {
        let client = PlexClient::new("http://localhost:32400", "token");
        let source = PlexSource::new(client, "My Plex".into());

        assert_eq!(source.name(), "My Plex");
        assert_eq!(source.source_type(), SourceType::Plex);
    }

    #[test]
    fn plex_source_playback_url() {
        let client = PlexClient::new("http://localhost:32400", "token");
        let source = PlexSource::new(client, "Test".into());

        let url = source.playback_url("/library/parts/456/file.mkv");
        assert!(url.contains("/library/parts/456/file.mkv"));
        assert!(url.contains("X-Plex-Token=token"));
    }

    #[test]
    fn plex_source_artwork_url() {
        let client = PlexClient::new("http://localhost:32400", "token");
        let source = PlexSource::new(client, "Test".into());

        let url = source.artwork_url("/library/metadata/123/thumb/1", 300, 450);
        assert!(url.contains("width=300"));
        assert!(url.contains("height=450"));
    }

    #[test]
    fn plex_source_is_trait_object_compatible() {
        let client = PlexClient::new("http://localhost:32400", "token");
        let source = PlexSource::new(client, "Test".into());
        let _boxed: Box<dyn MediaSource> = Box::new(source);
    }
}
