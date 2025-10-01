use serde::{Deserialize, Deserializer};
use serde_json::Value;

// Helper function to deserialize fields that can be either string or number
fn deserialize_string_or_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(s) => Ok(s),
        Value::Number(n) => Ok(n.to_string()),
        _ => Err(serde::de::Error::custom("expected string or number")),
    }
}

// Plex Identity response for getting server machine ID
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlexIdentityResponse {
    pub media_container: PlexIdentityContainer,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexIdentityContainer {
    pub machine_identifier: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlexMetadataResponse {
    #[serde(rename = "MediaContainer")]
    _media_container: PlexMetadataContainer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexMetadataContainer {
    #[serde(rename = "Metadata", default)]
    _metadata: Vec<PlexMetadataWithMarkers>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexMetadataWithMarkers {
    #[serde(rename = "Marker", default)]
    _marker: Option<Vec<PlexMarker>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexMarker {
    #[serde(rename = "type")]
    _type: String,
    _start_time_offset: i64,
    _end_time_offset: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlexLibrariesResponse {
    pub media_container: PlexLibrariesContainer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexLibrariesContainer {
    #[serde(rename = "Directory", default)]
    pub directory: Vec<PlexLibraryDirectory>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexLibraryDirectory {
    pub key: String,
    pub title: String,
    #[serde(rename = "type")]
    pub library_type: String,
    _uuid: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlexMoviesResponse {
    pub media_container: PlexMoviesContainer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexMoviesContainer {
    #[serde(rename = "Metadata", default)]
    pub metadata: Vec<PlexMovieMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexMovieMetadata {
    pub rating_key: String,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub year: Option<i32>,
    #[serde(default)]
    pub rating: Option<f64>,
    #[serde(default)]
    pub thumb: Option<String>,
    #[serde(default)]
    pub art: Option<String>,
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(default)]
    pub view_count: Option<u32>,
    #[serde(default)]
    pub view_offset: Option<i64>,
    #[serde(default)]
    pub last_viewed_at: Option<i64>,
    #[serde(default)]
    pub added_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(rename = "Genre", default)]
    pub genres: Vec<PlexTag>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexTag {
    pub tag: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlexShowsResponse {
    pub media_container: PlexShowsContainer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexShowsContainer {
    #[serde(rename = "Metadata", default)]
    pub metadata: Vec<PlexShowMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexShowMetadata {
    pub rating_key: String,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub year: Option<i32>,
    #[serde(default)]
    pub rating: Option<f64>,
    #[serde(default)]
    pub thumb: Option<String>,
    #[serde(default)]
    pub art: Option<String>,
    #[serde(default)]
    pub added_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(rename = "Genre", default)]
    pub genres: Vec<PlexTag>,
    #[serde(default)]
    pub leaf_count: Option<i32>,
    #[serde(default)]
    pub viewed_leaf_count: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlexSeasonsResponse {
    pub media_container: PlexSeasonsContainer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexSeasonsContainer {
    #[serde(rename = "Metadata", default)]
    pub metadata: Vec<PlexSeasonMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexSeasonMetadata {
    pub rating_key: String,
    _title: String,
    pub index: i32,
    #[serde(default)]
    pub thumb: Option<String>,
    #[serde(default)]
    pub leaf_count: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlexEpisodesResponse {
    pub media_container: PlexEpisodesContainer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexEpisodesContainer {
    #[serde(rename = "Metadata", default)]
    pub metadata: Vec<PlexEpisodeMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexEpisodeMetadata {
    pub rating_key: String,
    pub title: String,
    pub index: i32,
    #[serde(default)]
    pub parent_index: Option<i32>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub thumb: Option<String>,
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(default)]
    pub aired_at: Option<String>,
    #[serde(default)]
    pub view_offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlexMediaResponse {
    pub media_container: PlexMediaContainer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlexMediaContainer {
    #[serde(default)]
    pub metadata: Vec<PlexMediaMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlexMediaMetadata {
    #[serde(default)]
    pub media: Vec<PlexMedia>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexMedia {
    #[serde(default)]
    pub bitrate: Option<u64>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub video_codec: Option<String>,
    #[serde(default)]
    pub audio_codec: Option<String>,
    #[serde(rename = "Part", default)]
    pub parts: Option<Vec<PlexPart>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexPart {
    pub key: String,
    #[serde(default)]
    pub container: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexGenericMetadata {
    pub rating_key: String,
    pub title: String,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    #[serde(default)]
    pub thumb: Option<String>,
    #[serde(default)]
    pub art: Option<String>,
    #[serde(default)]
    pub year: Option<i32>,
    #[serde(default)]
    pub rating: Option<f64>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(default)]
    pub view_offset: Option<i64>,
    #[serde(default)]
    pub view_count: Option<u32>,
    #[serde(default)]
    pub added_at: Option<i64>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub last_viewed_at: Option<i64>,
    #[serde(default)]
    pub leaf_count: Option<i32>,
    #[serde(default)]
    pub viewed_leaf_count: Option<i32>,
    #[serde(default)]
    pub parent_index: Option<i32>,
    #[serde(default)]
    pub index: Option<i32>,
    #[serde(default)]
    pub grandparent_rating_key: String,
    #[serde(default)]
    pub grandparent_title: Option<String>,
    #[serde(default)]
    pub grandparent_thumb: Option<String>,
    #[serde(rename = "Genre", default)]
    pub genre: Option<Vec<PlexTag>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlexOnDeckResponse {
    #[serde(rename = "MediaContainer")]
    _media_container: PlexOnDeckContainer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexOnDeckContainer {
    #[serde(rename = "Metadata", default)]
    _metadata: Vec<PlexGenericMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlexRecentlyAddedResponse {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexRecentlyAddedContainer {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlexHubsResponse {
    pub media_container: PlexHubsContainer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexHubsContainer {
    #[serde(rename = "Hub", default)]
    pub hubs: Vec<PlexHub>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlexHub {
    pub key: String,
    pub title: String,
    #[serde(rename = "hubIdentifier")]
    pub hub_identifier: Option<String>,
    #[serde(rename = "Metadata", default)]
    pub metadata: Vec<PlexGenericMetadata>,
}
