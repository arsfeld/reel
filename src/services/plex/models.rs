use serde::Deserialize;

// --- Library listing response ---

#[derive(Debug, Deserialize)]
pub struct PlexLibraryResponse {
    #[serde(rename = "MediaContainer")]
    pub media_container: PlexLibraryContainer,
}

#[derive(Debug, Deserialize)]
pub struct PlexLibraryContainer {
    pub size: Option<i32>,
    #[serde(default, rename = "Directory")]
    pub directories: Vec<PlexLibrary>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlexLibrary {
    pub key: String,
    pub title: String,
    #[serde(rename = "type")]
    pub library_type: String,
    #[serde(rename = "count", default)]
    pub item_count: Option<i32>,
}

// --- Metadata response (movies, shows, seasons, episodes) ---

#[derive(Debug, Deserialize)]
pub struct PlexMetadataResponse {
    #[serde(rename = "MediaContainer")]
    pub media_container: PlexMetadataContainer,
}

#[derive(Debug, Deserialize)]
pub struct PlexMetadataContainer {
    pub size: Option<i32>,
    #[serde(default, rename = "Metadata")]
    pub metadata: Vec<PlexMetadata>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlexMetadata {
    #[serde(rename = "ratingKey")]
    pub rating_key: String,
    pub title: String,
    #[serde(rename = "type")]
    pub metadata_type: String,
    pub year: Option<i32>,
    pub summary: Option<String>,
    #[serde(rename = "contentRating")]
    pub content_rating: Option<String>,
    pub rating: Option<f64>,
    /// Duration in milliseconds
    pub duration: Option<i64>,
    pub thumb: Option<String>,
    pub art: Option<String>,
    #[serde(rename = "addedAt")]
    pub added_at: Option<i64>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<i64>,

    // TV specific
    #[serde(rename = "parentRatingKey")]
    pub parent_rating_key: Option<String>,
    #[serde(rename = "grandparentRatingKey")]
    pub grandparent_rating_key: Option<String>,
    #[serde(rename = "parentIndex")]
    pub parent_index: Option<i32>,
    pub index: Option<i32>,
    #[serde(rename = "originallyAvailableAt")]
    pub originally_available_at: Option<String>,

    // Nested structures
    #[serde(default, rename = "Genre")]
    pub genres: Vec<PlexTag>,
    #[serde(default, rename = "Media")]
    pub media: Vec<PlexMedia>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlexTag {
    pub tag: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlexMedia {
    #[serde(default, rename = "Part")]
    pub parts: Vec<PlexPart>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlexPart {
    pub key: String,
    pub file: Option<String>,
    pub size: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_library_response() {
        let json = r#"{
            "MediaContainer": {
                "size": 2,
                "Directory": [
                    {"key": "1", "title": "Movies", "type": "movie", "count": 500},
                    {"key": "2", "title": "TV Shows", "type": "show", "count": 100}
                ]
            }
        }"#;

        let resp: PlexLibraryResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.media_container.directories.len(), 2);
        assert_eq!(resp.media_container.directories[0].title, "Movies");
        assert_eq!(resp.media_container.directories[0].library_type, "movie");
        assert_eq!(resp.media_container.directories[0].item_count, Some(500));
    }

    #[test]
    fn deserialize_metadata_response() {
        let json = r#"{
            "MediaContainer": {
                "size": 1,
                "Metadata": [{
                    "ratingKey": "123",
                    "title": "Dune",
                    "type": "movie",
                    "year": 2021,
                    "summary": "A noble family.",
                    "contentRating": "PG-13",
                    "rating": 8.0,
                    "duration": 9300000,
                    "thumb": "/library/metadata/123/thumb/1234",
                    "art": "/library/metadata/123/art/1234",
                    "addedAt": 1705276800,
                    "updatedAt": 1705276800,
                    "Genre": [{"tag": "Science Fiction"}, {"tag": "Adventure"}],
                    "Media": [{
                        "Part": [{
                            "key": "/library/parts/456/1234/file.mkv",
                            "file": "/data/movies/Dune (2021)/Dune.mkv",
                            "size": 15000000000
                        }]
                    }]
                }]
            }
        }"#;

        let resp: PlexMetadataResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.media_container.metadata.len(), 1);

        let m = &resp.media_container.metadata[0];
        assert_eq!(m.rating_key, "123");
        assert_eq!(m.title, "Dune");
        assert_eq!(m.year, Some(2021));
        assert_eq!(m.duration, Some(9300000));
        assert_eq!(m.genres.len(), 2);
        assert_eq!(m.genres[0].tag, "Science Fiction");
        assert_eq!(m.media[0].parts[0].key, "/library/parts/456/1234/file.mkv");
    }

    #[test]
    fn deserialize_missing_optional_fields() {
        let json = r#"{
            "MediaContainer": {
                "size": 1,
                "Metadata": [{
                    "ratingKey": "999",
                    "title": "Minimal",
                    "type": "movie"
                }]
            }
        }"#;

        let resp: PlexMetadataResponse = serde_json::from_str(json).unwrap();
        let m = &resp.media_container.metadata[0];
        assert_eq!(m.title, "Minimal");
        assert_eq!(m.year, None);
        assert_eq!(m.summary, None);
        assert_eq!(m.rating, None);
        assert_eq!(m.duration, None);
        assert!(m.genres.is_empty());
        assert!(m.media.is_empty());
    }

    #[test]
    fn deserialize_ignores_unknown_fields() {
        let json = r#"{
            "MediaContainer": {
                "size": 1,
                "unknownField": "ignored",
                "Metadata": [{
                    "ratingKey": "1",
                    "title": "Test",
                    "type": "movie",
                    "futureField": true
                }]
            }
        }"#;

        let resp: PlexMetadataResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.media_container.metadata[0].title, "Test");
    }

    #[test]
    fn deserialize_episode_metadata() {
        let json = r#"{
            "MediaContainer": {
                "size": 1,
                "Metadata": [{
                    "ratingKey": "500",
                    "title": "Pilot",
                    "type": "episode",
                    "parentRatingKey": "400",
                    "grandparentRatingKey": "300",
                    "parentIndex": 1,
                    "index": 1,
                    "originallyAvailableAt": "2024-03-01",
                    "duration": 2520000,
                    "Media": [{
                        "Part": [{
                            "key": "/library/parts/501/file.mkv",
                            "file": "/data/tv/Show/S01E01.mkv",
                            "size": 2000000000
                        }]
                    }]
                }]
            }
        }"#;

        let resp: PlexMetadataResponse = serde_json::from_str(json).unwrap();
        let ep = &resp.media_container.metadata[0];
        assert_eq!(ep.parent_rating_key, Some("400".to_string()));
        assert_eq!(ep.grandparent_rating_key, Some("300".to_string()));
        assert_eq!(ep.parent_index, Some(1));
        assert_eq!(ep.index, Some(1));
        assert_eq!(ep.originally_available_at, Some("2024-03-01".to_string()));
    }

    #[test]
    fn deserialize_empty_library() {
        let json = r#"{
            "MediaContainer": {
                "size": 0
            }
        }"#;

        let resp: PlexLibraryResponse = serde_json::from_str(json).unwrap();
        assert!(resp.media_container.directories.is_empty());
    }

    #[test]
    fn deserialize_empty_metadata() {
        let json = r#"{
            "MediaContainer": {
                "size": 0
            }
        }"#;

        let resp: PlexMetadataResponse = serde_json::from_str(json).unwrap();
        assert!(resp.media_container.metadata.is_empty());
    }

    #[test]
    fn deserialize_show_metadata() {
        let json = r#"{
            "MediaContainer": {
                "size": 1,
                "Metadata": [{
                    "ratingKey": "300",
                    "title": "Breaking Bad",
                    "type": "show",
                    "year": 2008,
                    "summary": "A chemistry teacher.",
                    "contentRating": "TV-MA",
                    "rating": 9.5,
                    "thumb": "/library/metadata/300/thumb/1",
                    "art": "/library/metadata/300/art/1",
                    "Genre": [{"tag": "Drama"}, {"tag": "Crime"}]
                }]
            }
        }"#;

        let resp: PlexMetadataResponse = serde_json::from_str(json).unwrap();
        let show = &resp.media_container.metadata[0];
        assert_eq!(show.metadata_type, "show");
        assert_eq!(show.year, Some(2008));
        assert_eq!(show.genres.len(), 2);
    }

    #[test]
    fn deserialize_season_metadata() {
        let json = r#"{
            "MediaContainer": {
                "size": 1,
                "Metadata": [{
                    "ratingKey": "400",
                    "title": "Season 1",
                    "type": "season",
                    "parentRatingKey": "300",
                    "index": 1,
                    "thumb": "/library/metadata/400/thumb/1"
                }]
            }
        }"#;

        let resp: PlexMetadataResponse = serde_json::from_str(json).unwrap();
        let season = &resp.media_container.metadata[0];
        assert_eq!(season.metadata_type, "season");
        assert_eq!(season.parent_rating_key, Some("300".to_string()));
        assert_eq!(season.index, Some(1));
    }
}
