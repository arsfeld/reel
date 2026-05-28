use serde::Deserialize;

/// Deserialize Plex's `librarySectionID`, which arrives as a JSON number on most
/// endpoints but occasionally as a string. Normalizes to `Option<String>` so it
/// matches the string section keys returned by `/library/sections`.
fn de_opt_section_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrInt {
        Str(String),
        Int(i64),
    }
    let opt = Option::<StringOrInt>::deserialize(deserializer)?;
    Ok(opt.map(|v| match v {
        StringOrInt::Str(s) => s,
        StringOrInt::Int(i) => i.to_string(),
    }))
}

/// Deserialize an optional i64 that Plex may send as a JSON number or a
/// stringified number. The transcode decision response returns `Media.id` /
/// `Part.id` / `Stream.id` as strings while metadata endpoints send numbers.
fn de_opt_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum IntOrStr {
        Int(i64),
        Str(String),
    }
    let opt = Option::<IntOrStr>::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(IntOrStr::Int(i)) => Ok(Some(i)),
        Some(IntOrStr::Str(s)) => s.parse::<i64>().map(Some).map_err(serde::de::Error::custom),
    }
}

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

    // Parent artwork (season art on episodes, show art on seasons)
    #[serde(rename = "parentThumb")]
    pub parent_thumb: Option<String>,
    // Grandparent artwork: the series poster on an episode. Plex sends this on
    // On Deck / recently-added episode payloads.
    #[serde(rename = "grandparentThumb")]
    pub grandparent_thumb: Option<String>,

    /// The library section this item belongs to. Plex sends `librarySectionID`
    /// as a JSON number on On Deck / recently-added payloads; absent on some
    /// endpoints (e.g. items fetched via a `/library/sections/{key}/all` URL,
    /// where the section is implied). Normalized to a string section key.
    #[serde(
        rename = "librarySectionID",
        default,
        deserialize_with = "de_opt_section_id"
    )]
    pub library_section_id: Option<String>,

    // Watch state fields
    /// Resume position in milliseconds. Present only if partially watched.
    #[serde(rename = "viewOffset")]
    pub view_offset: Option<i64>,
    /// Number of times fully watched. >= 1 means watched.
    #[serde(rename = "viewCount")]
    pub view_count: Option<i32>,
    /// Unix timestamp of when last fully watched.
    #[serde(rename = "lastViewedAt")]
    pub last_viewed_at: Option<i64>,

    // Nested structures
    #[serde(default, rename = "Genre")]
    pub genres: Vec<PlexTag>,
    #[serde(default, rename = "Role")]
    pub roles: Vec<PlexRole>,
    #[serde(default, rename = "Director")]
    pub directors: Vec<PlexTag>,
    #[serde(default, rename = "Writer")]
    pub writers: Vec<PlexTag>,
    #[serde(default, rename = "Collection")]
    pub collections: Vec<PlexTag>,
    #[serde(default, rename = "Media")]
    pub media: Vec<PlexMedia>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlexTag {
    pub tag: String,
}

/// Cast member with character name and photo.
#[derive(Debug, Clone, Deserialize)]
pub struct PlexRole {
    pub tag: String,
    pub role: Option<String>,
    pub thumb: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlexMedia {
    /// Plex's internal Media id. Dropped before U3; kept now for transcode
    /// correlation. Positional `mediaIndex` (not this id) drives the transcode
    /// URL — see [`crate::services::plex`] transcode flow. Plex sends this as a
    /// string on decision responses, a number on metadata.
    #[serde(default, deserialize_with = "de_opt_i64")]
    pub id: Option<i64>,
    #[serde(rename = "videoResolution")]
    pub video_resolution: Option<String>,
    #[serde(rename = "videoCodec")]
    pub video_codec: Option<String>,
    #[serde(rename = "audioCodec")]
    pub audio_codec: Option<String>,
    #[serde(rename = "audioChannels")]
    pub audio_channels: Option<i32>,
    pub bitrate: Option<i64>,
    pub container: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    /// Plex's `videoDynamicRange` field. Typical values: "SDR", "HDR", "HDR10",
    /// "HDR10+", "HLG", "Dolby Vision", "DV". Absent on older Plex servers.
    #[serde(rename = "videoDynamicRange")]
    pub video_dynamic_range: Option<String>,
    /// Output protocol on a decision response (e.g. "hls" for a transcode).
    pub protocol: Option<String>,
    /// Whether the transcode decision selected this Media variant. Decision
    /// responses only; absent on plain metadata.
    pub selected: Option<bool>,
    #[serde(default, rename = "Part")]
    pub parts: Vec<PlexPart>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlexPart {
    /// Plex's internal Part id. Dropped before U3. String on decision
    /// responses, number on metadata.
    #[serde(default, deserialize_with = "de_opt_i64")]
    pub id: Option<i64>,
    /// Present on direct-play; absent (server-generated) on transcode parts.
    #[serde(default)]
    pub key: Option<String>,
    pub file: Option<String>,
    pub size: Option<i64>,
    pub container: Option<String>,
    /// Transcode decision for this part on a `/decision` response:
    /// `directplay` | `copy` | `transcode`. Absent on plain metadata.
    pub decision: Option<String>,
    #[serde(default, rename = "Stream")]
    pub streams: Vec<PlexStream>,
}

/// A single media stream (video / audio / subtitle) as returned on a transcode
/// decision response. Only the fields the transcode flow needs are modeled.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlexStream {
    #[serde(default, deserialize_with = "de_opt_i64")]
    pub id: Option<i64>,
    /// 1 = video, 2 = audio, 3 = subtitle.
    #[serde(rename = "streamType")]
    pub stream_type: Option<i32>,
    pub codec: Option<String>,
    /// Per-stream transcode decision: `directplay` | `copy` | `transcode`.
    pub decision: Option<String>,
    /// Where the stream is sourced from on a decision (e.g. "segments-av",
    /// "direct").
    pub location: Option<String>,
    pub selected: Option<bool>,
}

/// A live server-side transcode session, as listed by `/transcode/sessions` or
/// (on some Plex versions) embedded in a decision response. On the target
/// server (PMS 1.43) it is **absent** from the decision response — the
/// indicator's resolution/bitrate therefore come from the selected `PlexMedia`,
/// not from here (see U1 findings).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlexTranscodeSession {
    /// The session key — equals the client-supplied `session` id (U1: the
    /// server honors a client-chosen id).
    pub key: Option<String>,
    #[serde(rename = "videoDecision")]
    pub video_decision: Option<String>,
    #[serde(rename = "audioDecision")]
    pub audio_decision: Option<String>,
    pub container: Option<String>,
    pub protocol: Option<String>,
    /// Whether the transcoder is throttled (caught up to the playhead). The
    /// buffering watchdog (U8) reads this.
    pub throttled: Option<bool>,
    pub complete: Option<bool>,
    pub progress: Option<f64>,
    /// Total media duration in milliseconds.
    pub duration: Option<i64>,
    #[serde(rename = "transcodeHwEncoding")]
    pub transcode_hw_encoding: Option<String>,
}

// --- Transcode decision response (`/video/:/transcode/universal/decision`) ---

/// Response from the universal transcode `/decision` endpoint. Reuses
/// [`PlexMetadata`] for the item body (the decision echoes `ratingKey`/`title`/
/// `type`), adding the container-level decision codes and an optional
/// [`PlexTranscodeSession`].
#[derive(Debug, Clone, Deserialize)]
pub struct PlexDecisionResponse {
    #[serde(rename = "MediaContainer")]
    pub media_container: PlexDecisionContainer,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlexDecisionContainer {
    #[serde(rename = "generalDecisionCode")]
    pub general_decision_code: Option<i64>,
    #[serde(rename = "generalDecisionText")]
    pub general_decision_text: Option<String>,
    #[serde(rename = "directPlayDecisionCode")]
    pub direct_play_decision_code: Option<i64>,
    #[serde(rename = "directPlayDecisionText")]
    pub direct_play_decision_text: Option<String>,
    #[serde(rename = "transcodeDecisionCode")]
    pub transcode_decision_code: Option<i64>,
    #[serde(rename = "transcodeDecisionText")]
    pub transcode_decision_text: Option<String>,
    #[serde(default, rename = "Metadata")]
    pub metadata: Vec<PlexMetadata>,
    /// Absent on the target server's decision response (U1); present on
    /// `/transcode/sessions`. Modeled here for Plex versions that include it.
    #[serde(rename = "TranscodeSession")]
    pub transcode_session: Option<PlexTranscodeSession>,
}

/// Response from `/transcode/sessions` (list of active transcode sessions).
/// Used to verify a session started/stopped and to read `throttled` state.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlexTranscodeSessionsResponse {
    #[serde(rename = "MediaContainer")]
    pub media_container: PlexTranscodeSessionsContainer,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlexTranscodeSessionsContainer {
    pub size: Option<i32>,
    #[serde(default, rename = "TranscodeSession")]
    pub sessions: Vec<PlexTranscodeSession>,
}

// --- Chapter response (intro/credits skip markers) ---

#[derive(Debug, Deserialize)]
pub struct PlexChapterResponse {
    #[serde(rename = "MediaContainer")]
    pub media_container: PlexChapterContainer,
}

#[derive(Debug, Deserialize)]
pub struct PlexChapterContainer {
    pub size: Option<i32>,
    #[serde(default, rename = "Chapter")]
    pub chapters: Vec<PlexChapter>,
}

/// A single chapter marker from the Plex chapters API.
/// Times are in milliseconds from the start of the media.
#[derive(Debug, Clone, Deserialize)]
pub struct PlexChapter {
    pub id: Option<i64>,
    /// Human-readable chapter title (e.g. "Chapter 1", "Intro", "Credits").
    #[serde(default)]
    pub tag: Option<String>,
    /// Start time in milliseconds.
    #[serde(rename = "startTimeOffset", default)]
    pub start_time_offset: i64,
    /// End time in milliseconds.
    #[serde(rename = "endTimeOffset", default)]
    pub end_time_offset: i64,
}

// --- Hubs response (home-screen curated rows) ---

#[derive(Debug, Deserialize)]
pub struct PlexHubResponse {
    #[serde(rename = "MediaContainer")]
    pub media_container: PlexHubContainer,
}

#[derive(Debug, Deserialize)]
pub struct PlexHubContainer {
    pub size: Option<i32>,
    #[serde(default, rename = "Hub")]
    pub hubs: Vec<PlexHub>,
}

/// A single hub (curated home-screen row) from the Plex `/hubs` API:
/// Continue Watching, On Deck, Recently Added per library, Recommended,
/// "Because you watched", genre rows, etc. `hub_identifier` distinguishes the
/// kind of hub (e.g. `home.continue`, `home.ondeck`, `home.television.recent`)
/// and is what the home view uses to drop hubs Reel already renders itself.
#[derive(Debug, Clone, Deserialize)]
pub struct PlexHub {
    pub title: String,
    #[serde(rename = "hubIdentifier")]
    pub hub_identifier: Option<String>,
    #[serde(rename = "type")]
    pub hub_type: Option<String>,
    #[serde(default, rename = "Metadata")]
    pub metadata: Vec<PlexMetadata>,
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
        assert_eq!(
            m.media[0].parts[0].key.as_deref(),
            Some("/library/parts/456/1234/file.mkv")
        );
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
    fn deserialize_metadata_with_roles_directors_writers_collections() {
        let json = r#"{
            "MediaContainer": {
                "size": 1,
                "Metadata": [{
                    "ratingKey": "123",
                    "title": "Dune",
                    "type": "movie",
                    "Role": [
                        {"tag": "Timothée Chalamet", "role": "Paul Atreides", "thumb": "/photo/1"},
                        {"tag": "Zendaya", "role": "Chani"}
                    ],
                    "Director": [{"tag": "Denis Villeneuve"}],
                    "Writer": [{"tag": "Jon Spaihts"}, {"tag": "Denis Villeneuve"}],
                    "Collection": [{"tag": "Dune Collection"}]
                }]
            }
        }"#;

        let resp: PlexMetadataResponse = serde_json::from_str(json).unwrap();
        let m = &resp.media_container.metadata[0];
        assert_eq!(m.roles.len(), 2);
        assert_eq!(m.roles[0].tag, "Timothée Chalamet");
        assert_eq!(m.roles[0].role, Some("Paul Atreides".to_string()));
        assert_eq!(m.roles[0].thumb, Some("/photo/1".to_string()));
        assert_eq!(m.roles[1].tag, "Zendaya");
        assert_eq!(m.roles[1].thumb, None);
        assert_eq!(m.directors.len(), 1);
        assert_eq!(m.directors[0].tag, "Denis Villeneuve");
        assert_eq!(m.writers.len(), 2);
        assert_eq!(m.collections.len(), 1);
        assert_eq!(m.collections[0].tag, "Dune Collection");
    }

    #[test]
    fn deserialize_media_with_technical_fields() {
        let json = r#"{
            "MediaContainer": {
                "size": 1,
                "Metadata": [{
                    "ratingKey": "123",
                    "title": "Dune",
                    "type": "movie",
                    "Media": [{
                        "videoResolution": "1080",
                        "videoCodec": "h264",
                        "audioCodec": "aac",
                        "audioChannels": 6,
                        "bitrate": 12000,
                        "container": "mkv",
                        "width": 1920,
                        "height": 1080,
                        "Part": [{
                            "key": "/library/parts/456/file.mkv",
                            "file": "/data/Dune.mkv",
                            "size": 15000000000
                        }]
                    }]
                }]
            }
        }"#;

        let resp: PlexMetadataResponse = serde_json::from_str(json).unwrap();
        let media = &resp.media_container.metadata[0].media[0];
        assert_eq!(media.video_resolution, Some("1080".to_string()));
        assert_eq!(media.video_codec, Some("h264".to_string()));
        assert_eq!(media.audio_codec, Some("aac".to_string()));
        assert_eq!(media.audio_channels, Some(6));
        assert_eq!(media.bitrate, Some(12000));
        assert_eq!(media.container, Some("mkv".to_string()));
        assert_eq!(media.width, Some(1920));
        assert_eq!(media.height, Some(1080));
    }

    #[test]
    fn deserialize_metadata_with_parent_thumb() {
        let json = r#"{
            "MediaContainer": {
                "size": 1,
                "Metadata": [{
                    "ratingKey": "500",
                    "title": "Pilot",
                    "type": "episode",
                    "parentThumb": "/library/metadata/400/thumb/1"
                }]
            }
        }"#;

        let resp: PlexMetadataResponse = serde_json::from_str(json).unwrap();
        let m = &resp.media_container.metadata[0];
        assert_eq!(
            m.parent_thumb,
            Some("/library/metadata/400/thumb/1".to_string())
        );
    }

    #[test]
    fn deserialize_missing_new_fields_uses_defaults() {
        // Existing JSON without Role, Director, Writer, Collection, parentThumb
        // should still deserialize successfully with empty defaults
        let json = r#"{
            "MediaContainer": {
                "size": 1,
                "Metadata": [{
                    "ratingKey": "1",
                    "title": "Old Movie",
                    "type": "movie"
                }]
            }
        }"#;

        let resp: PlexMetadataResponse = serde_json::from_str(json).unwrap();
        let m = &resp.media_container.metadata[0];
        assert!(m.roles.is_empty());
        assert!(m.directors.is_empty());
        assert!(m.writers.is_empty());
        assert!(m.collections.is_empty());
        assert_eq!(m.parent_thumb, None);
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

    #[test]
    fn deserialize_watch_state_fields() {
        let json = r#"{
            "MediaContainer": {
                "size": 1,
                "Metadata": [{
                    "ratingKey": "123",
                    "title": "Dune",
                    "type": "movie",
                    "viewOffset": 2700000,
                    "viewCount": 3,
                    "lastViewedAt": 1710000000
                }]
            }
        }"#;

        let resp: PlexMetadataResponse = serde_json::from_str(json).unwrap();
        let m = &resp.media_container.metadata[0];
        assert_eq!(m.view_offset, Some(2700000));
        assert_eq!(m.view_count, Some(3));
        assert_eq!(m.last_viewed_at, Some(1710000000));
    }

    #[test]
    fn deserialize_library_section_id_as_number() {
        // Plex sends librarySectionID as a JSON number on On Deck / recently-added.
        let json = r#"{
            "MediaContainer": {
                "size": 1,
                "Metadata": [{
                    "ratingKey": "123",
                    "title": "Dune",
                    "type": "movie",
                    "librarySectionID": 4
                }]
            }
        }"#;

        let resp: PlexMetadataResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            resp.media_container.metadata[0].library_section_id,
            Some("4".to_string())
        );
    }

    #[test]
    fn deserialize_library_section_id_as_string() {
        let json = r#"{
            "MediaContainer": {
                "size": 1,
                "Metadata": [{
                    "ratingKey": "123",
                    "title": "Dune",
                    "type": "movie",
                    "librarySectionID": "4"
                }]
            }
        }"#;

        let resp: PlexMetadataResponse = serde_json::from_str(json).unwrap();
        assert_eq!(
            resp.media_container.metadata[0].library_section_id,
            Some("4".to_string())
        );
    }

    #[test]
    fn deserialize_library_section_id_absent_is_none() {
        let json = r#"{
            "MediaContainer": {
                "size": 1,
                "Metadata": [{"ratingKey": "1", "title": "X", "type": "movie"}]
            }
        }"#;

        let resp: PlexMetadataResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.media_container.metadata[0].library_section_id, None);
    }

    #[test]
    fn deserialize_watch_state_fields_absent() {
        let json = r#"{
            "MediaContainer": {
                "size": 1,
                "Metadata": [{
                    "ratingKey": "456",
                    "title": "Unwatched",
                    "type": "movie"
                }]
            }
        }"#;

        let resp: PlexMetadataResponse = serde_json::from_str(json).unwrap();
        let m = &resp.media_container.metadata[0];
        assert_eq!(m.view_offset, None);
        assert_eq!(m.view_count, None);
        assert_eq!(m.last_viewed_at, None);
    }

    #[test]
    fn deserialize_hub_response() {
        let json = r#"{
            "MediaContainer": {
                "size": 2,
                "Hub": [
                    {
                        "title": "Recommended",
                        "hubIdentifier": "home.movies.recommended",
                        "type": "movie",
                        "Metadata": [
                            {"ratingKey": "10", "title": "Dune", "type": "movie", "year": 2021},
                            {"ratingKey": "11", "title": "Arrival", "type": "movie", "year": 2016}
                        ]
                    },
                    {
                        "title": "Because you watched Breaking Bad",
                        "hubIdentifier": "home.television.becauseYouWatched",
                        "type": "show",
                        "Metadata": [
                            {"ratingKey": "20", "title": "Better Call Saul", "type": "show"}
                        ]
                    }
                ]
            }
        }"#;

        let resp: PlexHubResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.media_container.hubs.len(), 2);
        let first = &resp.media_container.hubs[0];
        assert_eq!(first.title, "Recommended");
        assert_eq!(
            first.hub_identifier,
            Some("home.movies.recommended".to_string())
        );
        assert_eq!(first.hub_type, Some("movie".to_string()));
        assert_eq!(first.metadata.len(), 2);
        assert_eq!(first.metadata[0].title, "Dune");
        assert_eq!(resp.media_container.hubs[1].metadata.len(), 1);
    }

    #[test]
    fn deserialize_hub_response_empty() {
        let json = r#"{"MediaContainer": {"size": 0}}"#;
        let resp: PlexHubResponse = serde_json::from_str(json).unwrap();
        assert!(resp.media_container.hubs.is_empty());
    }

    #[test]
    fn deserialize_hub_missing_optional_fields() {
        let json = r#"{
            "MediaContainer": {
                "size": 1,
                "Hub": [
                    {"title": "Mixed Row"}
                ]
            }
        }"#;

        let resp: PlexHubResponse = serde_json::from_str(json).unwrap();
        let hub = &resp.media_container.hubs[0];
        assert_eq!(hub.title, "Mixed Row");
        assert_eq!(hub.hub_identifier, None);
        assert_eq!(hub.hub_type, None);
        assert!(hub.metadata.is_empty());
    }

    #[test]
    fn deserialize_hub_ignores_unknown_fields() {
        let json = r#"{
            "MediaContainer": {
                "size": 1,
                "librarySectionID": 1,
                "Hub": [
                    {
                        "title": "Recently Added",
                        "hubIdentifier": "home.movies.recent",
                        "context": "hub.home.recentlyAdded",
                        "more": true,
                        "Metadata": [
                            {"ratingKey": "30", "title": "New Film", "type": "movie"}
                        ]
                    }
                ]
            }
        }"#;

        let resp: PlexHubResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.media_container.hubs[0].title, "Recently Added");
        assert_eq!(resp.media_container.hubs[0].metadata.len(), 1);
    }

    // --- U3: transcode decision request/response models ---

    /// First Part of the first Media of the first Metadata in a decision.
    fn first_part(resp: &PlexDecisionResponse) -> &PlexPart {
        resp.media_container.metadata[0].media[0]
            .parts
            .first()
            .expect("decision has a Part")
    }

    #[test]
    fn parse_transcode_decision_fixture_yields_transcode_part() {
        // Covers R1/R3: a recorded transcode decision parses with
        // Part.decision == "transcode" and per-stream transcode decisions.
        let json = include_str!("../../../tests/fixtures/plex/decision_transcode.json");
        let resp: PlexDecisionResponse = serde_json::from_str(json).unwrap();
        let mc = &resp.media_container;
        assert_eq!(mc.transcode_decision_code, Some(1001));
        assert_eq!(mc.direct_play_decision_code, Some(3000));
        let part = first_part(&resp);
        assert_eq!(part.decision.as_deref(), Some("transcode"));
        let video = part
            .streams
            .iter()
            .find(|s| s.stream_type == Some(1))
            .unwrap();
        assert_eq!(video.decision.as_deref(), Some("transcode"));
    }

    #[test]
    fn transcode_decision_sources_output_from_selected_media_not_session() {
        // U1 finding: TranscodeSession is absent from the decision response on
        // the target server, so the indicator's resolution/bitrate come from the
        // selected Media element, not TranscodeSession.
        let json = include_str!("../../../tests/fixtures/plex/decision_transcode.json");
        let resp: PlexDecisionResponse = serde_json::from_str(json).unwrap();
        assert!(resp.media_container.transcode_session.is_none());
        let media = &resp.media_container.metadata[0].media[0];
        assert_eq!(media.selected, Some(true));
        assert_eq!(media.video_codec.as_deref(), Some("h264")); // output codec
        assert_eq!(media.video_resolution.as_deref(), Some("720p"));
        assert_eq!(media.bitrate, Some(1877));
    }

    #[test]
    fn parse_directplay_decision_fixture() {
        // Covers R4: a direct-play decision parses with Part.decision ==
        // "directplay" and a usable part key.
        let json = include_str!("../../../tests/fixtures/plex/decision_directplay.json");
        let resp: PlexDecisionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.media_container.direct_play_decision_code, Some(1000));
        let part = first_part(&resp);
        assert_eq!(part.decision.as_deref(), Some("directplay"));
        assert!(part.key.as_deref().unwrap().contains("/library/parts/"));
    }

    #[test]
    fn parse_copy_directstream_decision_fixture() {
        // A direct-stream (remux) decision: video copied, audio transcoded.
        let json = include_str!("../../../tests/fixtures/plex/decision_copy.json");
        let resp: PlexDecisionResponse = serde_json::from_str(json).unwrap();
        let part = first_part(&resp);
        assert_eq!(part.decision.as_deref(), Some("copy"));
        let video = part
            .streams
            .iter()
            .find(|s| s.stream_type == Some(1))
            .unwrap();
        let audio = part
            .streams
            .iter()
            .find(|s| s.stream_type == Some(2))
            .unwrap();
        assert_eq!(video.decision.as_deref(), Some("copy"));
        assert_eq!(audio.decision.as_deref(), Some("transcode"));
    }

    #[test]
    fn decision_populates_media_and_part_ids() {
        // Covers: Media.id / Part.id (previously dropped) are populated.
        let json = include_str!("../../../tests/fixtures/plex/decision_transcode.json");
        let resp: PlexDecisionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.media_container.metadata[0].media[0].id, Some(282965));
        assert_eq!(first_part(&resp).id, Some(950874));
    }

    #[test]
    fn plain_metadata_without_decision_fields_still_parses() {
        // Back-compat: a normal metadata payload (no decision/id/Stream fields)
        // still deserializes; the new fields default to None/empty.
        let json = r#"{
            "MediaContainer": {"size": 1, "Metadata": [{
                "ratingKey": "1", "title": "Plain", "type": "movie",
                "Media": [{"videoResolution": "1080", "Part": [{"key": "/p/1/file.mkv"}]}]
            }]}
        }"#;
        let resp: PlexMetadataResponse = serde_json::from_str(json).unwrap();
        let media = &resp.media_container.metadata[0].media[0];
        assert_eq!(media.id, None);
        assert_eq!(media.selected, None);
        let part = &media.parts[0];
        assert_eq!(part.id, None);
        assert_eq!(part.decision, None);
        assert_eq!(part.key.as_deref(), Some("/p/1/file.mkv"));
        assert!(part.streams.is_empty());
    }

    #[test]
    fn parse_transcode_sessions_list() {
        // /transcode/sessions parses, exposing throttled + decisions (U10/U1).
        let json = include_str!("../../../tests/fixtures/plex/transcode_session.json");
        let resp: PlexTranscodeSessionsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.media_container.sessions.len(), 1);
        let s = &resp.media_container.sessions[0];
        assert_eq!(s.key.as_deref(), Some("reel-session-abcdef"));
        assert_eq!(s.video_decision.as_deref(), Some("transcode"));
        assert_eq!(s.throttled, Some(false));
        assert_eq!(s.transcode_hw_encoding.as_deref(), Some("vaapi"));
    }
}
