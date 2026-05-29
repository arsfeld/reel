use std::collections::HashMap;

use serde::Deserialize;

/// Jellyfin uses "ticks" as its time unit: 1 tick = 100 nanoseconds, so
/// 10,000,000 ticks = 1 second. This newtype wraps the raw tick count and
/// converts to the units the rest of the app uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Ticks(pub i64);

impl Ticks {
    /// Convert ticks to whole milliseconds.
    #[allow(clippy::wrong_self_convention)]
    pub fn to_ms(&self) -> i64 {
        self.0 / 10_000
    }

    /// Convert ticks to fractional seconds.
    #[allow(clippy::wrong_self_convention)]
    pub fn to_secs(&self) -> f64 {
        self.0 as f64 / 10_000_000.0
    }

    /// Build ticks from a millisecond count (useful for request bodies).
    pub fn from_ms(ms: i64) -> Ticks {
        Ticks(ms * 10_000)
    }
}

/// Generic Jellyfin query envelope: `{ "Items": [...], "TotalRecordCount": N }`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct QueryResult<T> {
    #[serde(default, rename = "Items")]
    pub items: Vec<T>,
    #[serde(default, rename = "TotalRecordCount")]
    pub total_record_count: i64,
}

/// The universal Jellyfin item. Movies, series, seasons, episodes, box sets and
/// library views are all returned as a `BaseItemDto` distinguished by `Type` /
/// `CollectionType`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct BaseItemDto {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(default, rename = "Name")]
    pub name: Option<String>,
    /// e.g. "Movie", "Series", "Season", "Episode", "BoxSet".
    #[serde(default, rename = "Type")]
    pub type_: Option<String>,
    #[serde(default, rename = "Overview")]
    pub overview: Option<String>,
    #[serde(default, rename = "ProductionYear")]
    pub production_year: Option<i32>,
    #[serde(default, rename = "PremiereDate")]
    pub premiere_date: Option<String>,
    #[serde(default, rename = "RunTimeTicks")]
    pub run_time_ticks: Option<i64>,
    #[serde(default, rename = "OfficialRating")]
    pub official_rating: Option<String>,
    #[serde(default, rename = "CommunityRating")]
    pub community_rating: Option<f64>,
    #[serde(default, rename = "Genres")]
    pub genres: Vec<String>,
    #[serde(default, rename = "People")]
    pub people: Vec<PersonDto>,
    #[serde(default, rename = "IndexNumber")]
    pub index_number: Option<i32>,
    #[serde(default, rename = "ParentIndexNumber")]
    pub parent_index_number: Option<i32>,
    #[serde(default, rename = "SeriesId")]
    pub series_id: Option<String>,
    #[serde(default, rename = "SeasonId")]
    pub season_id: Option<String>,
    #[serde(default, rename = "SeriesName")]
    pub series_name: Option<String>,
    #[serde(default, rename = "ParentId")]
    pub parent_id: Option<String>,
    /// e.g. `{"Primary": "tag", "Logo": "tag"}`.
    #[serde(default, rename = "ImageTags")]
    pub image_tags: Option<HashMap<String, String>>,
    #[serde(default, rename = "BackdropImageTags")]
    pub backdrop_image_tags: Vec<String>,
    #[serde(default, rename = "ParentBackdropItemId")]
    pub parent_backdrop_item_id: Option<String>,
    #[serde(default, rename = "ParentBackdropImageTags")]
    pub parent_backdrop_image_tags: Vec<String>,
    #[serde(default, rename = "SeriesPrimaryImageTag")]
    pub series_primary_image_tag: Option<String>,
    #[serde(default, rename = "UserData")]
    pub user_data: Option<UserItemData>,
    #[serde(default, rename = "MediaSources")]
    pub media_sources: Vec<MediaSourceInfo>,
    /// For library views: "movies", "tvshows", "boxsets", etc.
    #[serde(default, rename = "CollectionType")]
    pub collection_type: Option<String>,
    #[serde(default, rename = "Width")]
    pub width: Option<i32>,
    #[serde(default, rename = "Height")]
    pub height: Option<i32>,
}

/// A cast/crew member attached to an item.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PersonDto {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(default, rename = "Role")]
    pub role: Option<String>,
    #[serde(default, rename = "Type")]
    pub type_: Option<String>,
}

/// Per-user watch state for an item.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserItemData {
    #[serde(default, rename = "Played")]
    pub played: bool,
    #[serde(default, rename = "PlaybackPositionTicks")]
    pub playback_position_ticks: Option<i64>,
    #[serde(default, rename = "PlayCount")]
    pub play_count: Option<i32>,
    #[serde(default, rename = "IsFavorite")]
    pub is_favorite: bool,
}

/// A playable media source (used to obtain the `mediaSourceId` for streaming).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MediaSourceInfo {
    #[serde(default, rename = "Id")]
    pub id: Option<String>,
    #[serde(default, rename = "Path")]
    pub path: Option<String>,
    #[serde(default, rename = "Container")]
    pub container: Option<String>,
    #[serde(default, rename = "Name")]
    pub name: Option<String>,
}

/// Result of an authentication request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthenticationResult {
    #[serde(rename = "AccessToken")]
    pub access_token: String,
    #[serde(rename = "User")]
    pub user: UserDto,
    #[serde(default, rename = "ServerId")]
    pub server_id: Option<String>,
}

/// A Jellyfin user.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserDto {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(default, rename = "Name")]
    pub name: Option<String>,
}

/// State of a Quick Connect request.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct QuickConnectResult {
    #[serde(default, rename = "Code")]
    pub code: Option<String>,
    #[serde(default, rename = "Secret")]
    pub secret: Option<String>,
    #[serde(default, rename = "Authenticated")]
    pub authenticated: bool,
}

/// A media segment (intro/outro/etc) from the Media Segments API (10.10+).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MediaSegmentDto {
    #[serde(default, rename = "Id")]
    pub id: Option<String>,
    /// "Intro", "Outro", "Preview", "Recap", "Commercial".
    #[serde(default, rename = "Type")]
    pub type_: Option<String>,
    #[serde(default, rename = "StartTicks")]
    pub start_ticks: Option<i64>,
    #[serde(default, rename = "EndTicks")]
    pub end_ticks: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_movie_base_item() {
        let json = r#"{
            "Id": "abc123",
            "Name": "Dune",
            "Type": "Movie",
            "Overview": "A boy on a desert planet.",
            "ProductionYear": 2021,
            "RunTimeTicks": 95400000000,
            "OfficialRating": "PG-13",
            "CommunityRating": 8.0,
            "Genres": ["Science Fiction", "Adventure"],
            "ImageTags": {"Primary": "tag123"},
            "MediaSources": [{"Id": "src1", "Path": "/movies/dune.mkv", "Container": "mkv"}]
        }"#;
        let item: BaseItemDto = serde_json::from_str(json).unwrap();
        assert_eq!(item.id, "abc123");
        assert_eq!(item.name.as_deref(), Some("Dune"));
        assert_eq!(item.type_.as_deref(), Some("Movie"));
        assert_eq!(item.production_year, Some(2021));
        assert_eq!(item.genres.len(), 2);
        assert_eq!(
            item.image_tags.as_ref().unwrap().get("Primary").unwrap(),
            "tag123"
        );
        assert_eq!(item.media_sources.len(), 1);
        assert_eq!(item.media_sources[0].id.as_deref(), Some("src1"));
    }

    #[test]
    fn deserialize_episode_base_item() {
        let json = r#"{
            "Id": "ep1",
            "Name": "Pilot",
            "Type": "Episode",
            "IndexNumber": 1,
            "ParentIndexNumber": 2,
            "SeriesId": "series9",
            "SeasonId": "season3",
            "SeriesName": "Some Show"
        }"#;
        let item: BaseItemDto = serde_json::from_str(json).unwrap();
        assert_eq!(item.type_.as_deref(), Some("Episode"));
        assert_eq!(item.index_number, Some(1));
        assert_eq!(item.parent_index_number, Some(2));
        assert_eq!(item.series_id.as_deref(), Some("series9"));
        assert_eq!(item.season_id.as_deref(), Some("season3"));
        assert_eq!(item.series_name.as_deref(), Some("Some Show"));
    }

    #[test]
    fn deserialize_user_data() {
        let json = r#"{
            "Id": "x",
            "Type": "Movie",
            "UserData": {"Played": true, "PlaybackPositionTicks": 27000000000, "PlayCount": 3, "IsFavorite": true}
        }"#;
        let item: BaseItemDto = serde_json::from_str(json).unwrap();
        let ud = item.user_data.unwrap();
        assert!(ud.played);
        assert_eq!(ud.playback_position_ticks, Some(27000000000));
        assert_eq!(ud.play_count, Some(3));
        assert!(ud.is_favorite);
    }

    #[test]
    fn deserialize_query_result() {
        let json = r#"{
            "Items": [
                {"Id": "1", "Name": "A", "Type": "Movie"},
                {"Id": "2", "Name": "B", "Type": "Movie"}
            ],
            "TotalRecordCount": 2
        }"#;
        let qr: QueryResult<BaseItemDto> = serde_json::from_str(json).unwrap();
        assert_eq!(qr.items.len(), 2);
        assert_eq!(qr.total_record_count, 2);
        assert_eq!(qr.items[0].id, "1");
    }

    #[test]
    fn missing_optional_fields_use_defaults() {
        let json = r#"{"Id": "minimal"}"#;
        let item: BaseItemDto = serde_json::from_str(json).unwrap();
        assert_eq!(item.id, "minimal");
        assert_eq!(item.name, None);
        assert_eq!(item.type_, None);
        assert!(item.genres.is_empty());
        assert!(item.people.is_empty());
        assert!(item.media_sources.is_empty());
        assert!(item.image_tags.is_none());
        assert!(item.user_data.is_none());
    }

    #[test]
    fn query_result_missing_fields_use_defaults() {
        let json = r#"{}"#;
        let qr: QueryResult<BaseItemDto> = serde_json::from_str(json).unwrap();
        assert!(qr.items.is_empty());
        assert_eq!(qr.total_record_count, 0);
    }

    #[test]
    fn deserialize_authentication_result() {
        let json = r#"{
            "AccessToken": "token-xyz",
            "User": {"Id": "user1", "Name": "alice"},
            "ServerId": "srv1"
        }"#;
        let auth: AuthenticationResult = serde_json::from_str(json).unwrap();
        assert_eq!(auth.access_token, "token-xyz");
        assert_eq!(auth.user.id, "user1");
        assert_eq!(auth.user.name.as_deref(), Some("alice"));
        assert_eq!(auth.server_id.as_deref(), Some("srv1"));
    }

    #[test]
    fn deserialize_media_segment() {
        let json = r#"{"Id": "seg1", "Type": "Intro", "StartTicks": 0, "EndTicks": 300000000}"#;
        let seg: MediaSegmentDto = serde_json::from_str(json).unwrap();
        assert_eq!(seg.type_.as_deref(), Some("Intro"));
        assert_eq!(seg.start_ticks, Some(0));
        assert_eq!(seg.end_ticks, Some(300000000));
    }

    #[test]
    fn ticks_to_ms_and_secs() {
        let t = Ticks(10_000_000);
        assert_eq!(t.to_secs(), 1.0);
        assert_eq!(t.to_ms(), 1000);
        assert_eq!(Ticks::from_ms(1000), Ticks(10_000_000));
    }
}
