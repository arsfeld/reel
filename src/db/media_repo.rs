use rusqlite::{Connection, params};

use crate::models::media::{HdrFormat, MediaItem, MediaType, SourceType};

use super::DbError;

/// Repository for media item CRUD operations on SQLite.
#[allow(dead_code)]
pub struct MediaRepo<'a> {
    conn: &'a Connection,
}

#[allow(dead_code)]
impl<'a> MediaRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Insert or update a media item. Uses SQLite UPSERT on primary key.
    pub fn upsert(&self, item: &MediaItem) -> Result<(), DbError> {
        let genres_json = serde_json::to_string(&item.genres)
            .map_err(|e| DbError::Data(format!("Failed to serialize genres: {e}")))?;

        self.conn.execute(
            "INSERT INTO media_items (
                id, source_type, source_id, external_id, media_type,
                title, year, overview, content_rating, rating,
                runtime_minutes, poster_path, backdrop_path, genres,
                parent_id, season_number, episode_number, air_date,
                file_path, video_resolution, hdr, added_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14,
                ?15, ?16, ?17, ?18,
                ?19, ?20, ?21, ?22, ?23
            )
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                year = excluded.year,
                overview = excluded.overview,
                content_rating = excluded.content_rating,
                rating = excluded.rating,
                runtime_minutes = excluded.runtime_minutes,
                poster_path = excluded.poster_path,
                backdrop_path = excluded.backdrop_path,
                genres = excluded.genres,
                parent_id = excluded.parent_id,
                season_number = excluded.season_number,
                episode_number = excluded.episode_number,
                air_date = excluded.air_date,
                file_path = excluded.file_path,
                video_resolution = excluded.video_resolution,
                hdr = excluded.hdr,
                updated_at = excluded.updated_at",
            params![
                item.id,
                item.source_type.as_str(),
                item.source_id,
                item.external_id,
                item.media_type.as_str(),
                item.title,
                item.year,
                item.overview,
                item.content_rating,
                item.rating,
                item.runtime_minutes,
                item.poster_path,
                item.backdrop_path,
                genres_json,
                item.parent_id,
                item.season_number,
                item.episode_number,
                item.air_date,
                item.file_path,
                item.video_resolution,
                item.hdr.map(|h| h.as_str()),
                item.added_at,
                item.updated_at,
            ],
        )?;
        Ok(())
    }

    /// Find a media item by its composite ID.
    pub fn find_by_id(&self, id: &str) -> Result<Option<MediaItem>, DbError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM media_items WHERE id = ?1")?;

        let mut rows = stmt.query(params![id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_media_item(row)?)),
            None => Ok(None),
        }
    }

    /// List media items by type, ordered by title, with limit/offset pagination.
    pub fn list_by_type(
        &self,
        media_type: MediaType,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MediaItem>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM media_items WHERE media_type = ?1 ORDER BY title ASC LIMIT ?2 OFFSET ?3",
        )?;

        let rows = stmt.query_map(
            params![media_type.as_str(), limit as i64, offset as i64],
            |row| {
                row_to_media_item(row).map_err(|e| match e {
                    DbError::Sqlite(e) => e,
                    other => rusqlite::Error::ToSqlConversionFailure(Box::new(other)),
                })
            },
        )?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        Ok(items)
    }

    /// List children of a parent (e.g., seasons of a show, episodes of a season).
    pub fn list_by_parent(&self, parent_id: &str) -> Result<Vec<MediaItem>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM media_items WHERE parent_id = ?1 ORDER BY season_number ASC, episode_number ASC, title ASC",
        )?;

        let rows = stmt.query_map(params![parent_id], |row| {
            row_to_media_item(row).map_err(|e| match e {
                DbError::Sqlite(e) => e,
                other => rusqlite::Error::ToSqlConversionFailure(Box::new(other)),
            })
        })?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        Ok(items)
    }

    /// Delete all media items from a specific source.
    pub fn delete_by_source(
        &self,
        source_type: &SourceType,
        source_id: &str,
    ) -> Result<usize, DbError> {
        let count = self.conn.execute(
            "DELETE FROM media_items WHERE source_type = ?1 AND source_id = ?2",
            params![source_type.as_str(), source_id],
        )?;
        Ok(count)
    }

    /// Count media items of a given type.
    pub fn count_by_type(&self, media_type: MediaType) -> Result<usize, DbError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM media_items WHERE media_type = ?1",
            params![media_type.as_str()],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }
}

#[allow(dead_code)]
fn row_to_media_item(row: &rusqlite::Row) -> Result<MediaItem, DbError> {
    let source_type_str: String = row.get("source_type")?;
    let media_type_str: String = row.get("media_type")?;
    let genres_json: String = row.get("genres")?;

    let source_type = SourceType::from_str(&source_type_str)
        .ok_or_else(|| DbError::Data(format!("Unknown source type: {source_type_str}")))?;
    let media_type = MediaType::from_str(&media_type_str)
        .ok_or_else(|| DbError::Data(format!("Unknown media type: {media_type_str}")))?;
    let genres: Vec<String> = serde_json::from_str(&genres_json)
        .map_err(|e| DbError::Data(format!("Failed to parse genres: {e}")))?;

    Ok(MediaItem {
        id: row.get("id")?,
        source_type,
        source_id: row.get("source_id")?,
        external_id: row.get("external_id")?,
        media_type,
        title: row.get("title")?,
        year: row.get("year")?,
        overview: row.get("overview")?,
        content_rating: row.get("content_rating")?,
        rating: row.get("rating")?,
        runtime_minutes: row.get("runtime_minutes")?,
        poster_path: row.get("poster_path")?,
        // Transient field (series poster), not stored in media_items.
        series_poster_path: None,
        backdrop_path: row.get("backdrop_path")?,
        genres,
        parent_id: row.get("parent_id")?,
        season_number: row.get("season_number")?,
        episode_number: row.get("episode_number")?,
        air_date: row.get("air_date")?,
        file_path: row.get("file_path")?,
        video_resolution: row.get("video_resolution").ok().flatten(),
        hdr: row
            .get::<_, Option<String>>("hdr")
            .ok()
            .flatten()
            .and_then(|s| HdrFormat::from_db_str(&s)),
        added_at: row.get("added_at")?,
        updated_at: row.get("updated_at")?,
        playback_position_ms: None,
        // Transient field, not stored in media_items.
        library_section_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        conn
    }

    fn test_movie(id: &str, title: &str) -> MediaItem {
        MediaItem {
            id: id.to_string(),
            source_type: SourceType::Plex,
            source_id: "http://localhost:32400".to_string(),
            external_id: id.to_string(),
            media_type: MediaType::Movie,
            title: title.to_string(),
            year: Some(2021),
            overview: Some("A great movie".to_string()),
            content_rating: Some("PG-13".to_string()),
            rating: Some(8.0),
            runtime_minutes: Some(155),
            poster_path: Some("/thumb/123".to_string()),
            series_poster_path: None,
            backdrop_path: Some("/art/123".to_string()),
            genres: vec!["Action".to_string(), "Sci-Fi".to_string()],
            parent_id: None,
            season_number: None,
            episode_number: None,
            air_date: None,
            file_path: Some("/library/parts/456/file.mkv".to_string()),
            video_resolution: None,
            hdr: None,
            added_at: "2024-01-15".to_string(),
            updated_at: "2024-01-15".to_string(),
            playback_position_ms: None,
            library_section_id: None,
        }
    }

    fn test_episode(id: &str, parent_id: &str, season: i32, episode: i32) -> MediaItem {
        MediaItem {
            id: id.to_string(),
            source_type: SourceType::Plex,
            source_id: "http://localhost:32400".to_string(),
            external_id: id.to_string(),
            media_type: MediaType::Episode,
            title: format!("Episode {episode}"),
            year: None,
            overview: None,
            content_rating: None,
            rating: None,
            runtime_minutes: Some(42),
            poster_path: None,
            series_poster_path: None,
            backdrop_path: None,
            genres: vec![],
            parent_id: Some(parent_id.to_string()),
            season_number: Some(season),
            episode_number: Some(episode),
            air_date: Some("2024-03-01".to_string()),
            file_path: Some(format!("/library/parts/{id}/file.mkv")),
            video_resolution: None,
            hdr: None,
            added_at: "2024-01-15".to_string(),
            updated_at: "2024-01-15".to_string(),
            playback_position_ms: None,
            library_section_id: None,
        }
    }

    #[test]
    fn upsert_and_find_by_id_roundtrip() {
        let conn = test_db();
        let repo = MediaRepo::new(&conn);
        let movie = test_movie("m1", "Dune");

        repo.upsert(&movie).unwrap();
        let found = repo.find_by_id("m1").unwrap().unwrap();

        assert_eq!(found.title, "Dune");
        assert_eq!(found.year, Some(2021));
        assert_eq!(found.genres, vec!["Action", "Sci-Fi"]);
        assert_eq!(found.media_type, MediaType::Movie);
        assert_eq!(found.source_type, SourceType::Plex);
    }

    #[test]
    fn upsert_updates_existing_item() {
        let conn = test_db();
        let repo = MediaRepo::new(&conn);

        let mut movie = test_movie("m1", "Dune");
        repo.upsert(&movie).unwrap();

        movie.title = "Dune: Part Two".to_string();
        movie.year = Some(2024);
        repo.upsert(&movie).unwrap();

        let found = repo.find_by_id("m1").unwrap().unwrap();
        assert_eq!(found.title, "Dune: Part Two");
        assert_eq!(found.year, Some(2024));

        // Verify no duplicate
        assert_eq!(repo.count_by_type(MediaType::Movie).unwrap(), 1);
    }

    #[test]
    fn find_by_id_returns_none_for_missing() {
        let conn = test_db();
        let repo = MediaRepo::new(&conn);
        assert!(repo.find_by_id("nonexistent").unwrap().is_none());
    }

    #[test]
    fn list_by_type_returns_only_matching_type() {
        let conn = test_db();
        let repo = MediaRepo::new(&conn);

        repo.upsert(&test_movie("m1", "Dune")).unwrap();
        repo.upsert(&test_movie("m2", "Arrival")).unwrap();
        repo.upsert(&test_episode("e1", "s1", 1, 1)).unwrap();

        let movies = repo.list_by_type(MediaType::Movie, 100, 0).unwrap();
        assert_eq!(movies.len(), 2);
        assert!(movies.iter().all(|m| m.media_type == MediaType::Movie));

        let episodes = repo.list_by_type(MediaType::Episode, 100, 0).unwrap();
        assert_eq!(episodes.len(), 1);
    }

    #[test]
    fn list_by_type_respects_limit_and_offset() {
        let conn = test_db();
        let repo = MediaRepo::new(&conn);

        for i in 0..5 {
            repo.upsert(&test_movie(&format!("m{i}"), &format!("Movie {i}")))
                .unwrap();
        }

        let page1 = repo.list_by_type(MediaType::Movie, 2, 0).unwrap();
        assert_eq!(page1.len(), 2);

        let page2 = repo.list_by_type(MediaType::Movie, 2, 2).unwrap();
        assert_eq!(page2.len(), 2);

        let page3 = repo.list_by_type(MediaType::Movie, 2, 4).unwrap();
        assert_eq!(page3.len(), 1);
    }

    #[test]
    fn list_by_type_orders_by_title() {
        let conn = test_db();
        let repo = MediaRepo::new(&conn);

        repo.upsert(&test_movie("m1", "Zoolander")).unwrap();
        repo.upsert(&test_movie("m2", "Arrival")).unwrap();
        repo.upsert(&test_movie("m3", "Dune")).unwrap();

        let movies = repo.list_by_type(MediaType::Movie, 100, 0).unwrap();
        assert_eq!(movies[0].title, "Arrival");
        assert_eq!(movies[1].title, "Dune");
        assert_eq!(movies[2].title, "Zoolander");
    }

    #[test]
    fn list_by_parent_returns_children() {
        let conn = test_db();
        let repo = MediaRepo::new(&conn);

        repo.upsert(&test_episode("e1", "show1", 1, 1)).unwrap();
        repo.upsert(&test_episode("e2", "show1", 1, 2)).unwrap();
        repo.upsert(&test_episode("e3", "show2", 1, 1)).unwrap();

        let children = repo.list_by_parent("show1").unwrap();
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn list_by_parent_orders_by_season_then_episode() {
        let conn = test_db();
        let repo = MediaRepo::new(&conn);

        repo.upsert(&test_episode("e3", "show1", 2, 1)).unwrap();
        repo.upsert(&test_episode("e1", "show1", 1, 1)).unwrap();
        repo.upsert(&test_episode("e2", "show1", 1, 2)).unwrap();

        let children = repo.list_by_parent("show1").unwrap();
        assert_eq!(children[0].season_number, Some(1));
        assert_eq!(children[0].episode_number, Some(1));
        assert_eq!(children[1].season_number, Some(1));
        assert_eq!(children[1].episode_number, Some(2));
        assert_eq!(children[2].season_number, Some(2));
    }

    #[test]
    fn delete_by_source_removes_correct_items() {
        let conn = test_db();
        let repo = MediaRepo::new(&conn);

        repo.upsert(&test_movie("m1", "Dune")).unwrap();

        let mut other = test_movie("m2", "Other");
        other.source_id = "http://other:32400".to_string();
        repo.upsert(&other).unwrap();

        let deleted = repo
            .delete_by_source(&SourceType::Plex, "http://localhost:32400")
            .unwrap();
        assert_eq!(deleted, 1);

        assert!(repo.find_by_id("m1").unwrap().is_none());
        assert!(repo.find_by_id("m2").unwrap().is_some());
    }

    #[test]
    fn count_by_type_correct() {
        let conn = test_db();
        let repo = MediaRepo::new(&conn);

        assert_eq!(repo.count_by_type(MediaType::Movie).unwrap(), 0);

        repo.upsert(&test_movie("m1", "Dune")).unwrap();
        repo.upsert(&test_movie("m2", "Arrival")).unwrap();
        assert_eq!(repo.count_by_type(MediaType::Movie).unwrap(), 2);
    }

    #[test]
    fn genres_stored_as_json_array() {
        let conn = test_db();
        let repo = MediaRepo::new(&conn);

        let movie = test_movie("m1", "Dune");
        repo.upsert(&movie).unwrap();

        // Verify raw JSON in database
        let raw: String = conn
            .query_row(
                "SELECT genres FROM media_items WHERE id = 'm1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(raw, r#"["Action","Sci-Fi"]"#);

        // Verify deserialization
        let found = repo.find_by_id("m1").unwrap().unwrap();
        assert_eq!(found.genres, vec!["Action", "Sci-Fi"]);
    }

    #[test]
    fn empty_genres_stored_correctly() {
        let conn = test_db();
        let repo = MediaRepo::new(&conn);

        let mut movie = test_movie("m1", "Dune");
        movie.genres = vec![];
        repo.upsert(&movie).unwrap();

        let found = repo.find_by_id("m1").unwrap().unwrap();
        assert!(found.genres.is_empty());
    }

    #[test]
    fn hdr_field_roundtrip_hdr() {
        let conn = test_db();
        let repo = MediaRepo::new(&conn);
        let mut movie = test_movie("m1", "Dune");
        movie.hdr = Some(HdrFormat::Hdr);
        movie.video_resolution = Some("4k".to_string());
        repo.upsert(&movie).unwrap();
        let found = repo.find_by_id("m1").unwrap().unwrap();
        assert_eq!(found.hdr, Some(HdrFormat::Hdr));
        assert_eq!(found.video_resolution, Some("4k".to_string()));
    }

    #[test]
    fn hdr_field_roundtrip_dolby_vision() {
        let conn = test_db();
        let repo = MediaRepo::new(&conn);
        let mut movie = test_movie("m1", "Dune");
        movie.hdr = Some(HdrFormat::DolbyVision);
        repo.upsert(&movie).unwrap();
        let found = repo.find_by_id("m1").unwrap().unwrap();
        assert_eq!(found.hdr, Some(HdrFormat::DolbyVision));
    }

    #[test]
    fn hdr_field_roundtrip_none() {
        let conn = test_db();
        let repo = MediaRepo::new(&conn);
        let movie = test_movie("m1", "Dune"); // hdr: None
        repo.upsert(&movie).unwrap();
        let found = repo.find_by_id("m1").unwrap().unwrap();
        assert_eq!(found.hdr, None);
    }

    #[test]
    fn upsert_updates_hdr_value() {
        let conn = test_db();
        let repo = MediaRepo::new(&conn);
        let mut movie = test_movie("m1", "Dune");
        movie.hdr = Some(HdrFormat::Hdr);
        repo.upsert(&movie).unwrap();
        movie.hdr = Some(HdrFormat::DolbyVision);
        repo.upsert(&movie).unwrap();
        let found = repo.find_by_id("m1").unwrap().unwrap();
        assert_eq!(found.hdr, Some(HdrFormat::DolbyVision));
    }

    #[test]
    fn optional_fields_null_roundtrip() {
        let conn = test_db();
        let repo = MediaRepo::new(&conn);

        let mut movie = test_movie("m1", "Dune");
        movie.year = None;
        movie.overview = None;
        movie.rating = None;
        movie.runtime_minutes = None;
        movie.poster_path = None;
        movie.backdrop_path = None;
        movie.file_path = None;
        repo.upsert(&movie).unwrap();

        let found = repo.find_by_id("m1").unwrap().unwrap();
        assert_eq!(found.year, None);
        assert_eq!(found.overview, None);
        assert_eq!(found.rating, None);
        assert_eq!(found.runtime_minutes, None);
        assert_eq!(found.poster_path, None);
        assert_eq!(found.backdrop_path, None);
        assert_eq!(found.file_path, None);
    }
}
