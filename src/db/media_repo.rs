use diesel::prelude::*;

use crate::db::rows::{MediaItemChangeset, MediaItemRow, NewMediaItemRow};
use crate::models::media::{MediaItem, MediaType, SourceType};
use crate::schema::media_items;

use super::DbError;

/// Repository for media item CRUD operations on SQLite (diesel).
#[allow(dead_code)]
pub struct MediaRepo<'a> {
    conn: &'a mut SqliteConnection,
}

#[allow(dead_code)]
impl<'a> MediaRepo<'a> {
    pub fn new(conn: &'a mut SqliteConnection) -> Self {
        Self { conn }
    }

    /// Insert or update a media item. On conflict (same id) the identity
    /// columns and `added_at` are preserved; everything else is overwritten.
    pub fn upsert(&mut self, item: &MediaItem) -> Result<(), DbError> {
        let new = NewMediaItemRow::try_from(item)?;
        let changes = MediaItemChangeset::try_from(item)?;

        diesel::insert_into(media_items::table)
            .values(&new)
            .on_conflict(media_items::id)
            .do_update()
            .set(&changes)
            .execute(self.conn)?;
        Ok(())
    }

    /// Find a media item by its composite ID.
    pub fn find_by_id(&mut self, id: &str) -> Result<Option<MediaItem>, DbError> {
        let row = media_items::table
            .find(id)
            .select(MediaItemRow::as_select())
            .first::<MediaItemRow>(self.conn)
            .optional()?;
        row.map(MediaItem::try_from).transpose()
    }

    /// List media items by type, ordered by title, with limit/offset pagination.
    pub fn list_by_type(
        &mut self,
        media_type: MediaType,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<MediaItem>, DbError> {
        let rows = media_items::table
            .filter(media_items::media_type.eq(media_type.as_str()))
            .order(media_items::title.asc())
            .limit(limit as i64)
            .offset(offset as i64)
            .select(MediaItemRow::as_select())
            .load::<MediaItemRow>(self.conn)?;
        rows.into_iter().map(MediaItem::try_from).collect()
    }

    /// List children of a parent (e.g., seasons of a show, episodes of a season).
    pub fn list_by_parent(&mut self, parent_id: &str) -> Result<Vec<MediaItem>, DbError> {
        let rows = media_items::table
            .filter(media_items::parent_id.eq(parent_id))
            .order((
                media_items::season_number.asc(),
                media_items::episode_number.asc(),
                media_items::title.asc(),
            ))
            .select(MediaItemRow::as_select())
            .load::<MediaItemRow>(self.conn)?;
        rows.into_iter().map(MediaItem::try_from).collect()
    }

    /// Delete all media items from a specific source.
    pub fn delete_by_source(
        &mut self,
        source_type: &SourceType,
        source_id: &str,
    ) -> Result<usize, DbError> {
        let count = diesel::delete(
            media_items::table.filter(
                media_items::source_type
                    .eq(source_type.as_str())
                    .and(media_items::source_id.eq(source_id)),
            ),
        )
        .execute(self.conn)?;
        Ok(count)
    }

    /// Count media items of a given type.
    pub fn count_by_type(&mut self, media_type: MediaType) -> Result<usize, DbError> {
        let count: i64 = media_items::table
            .filter(media_items::media_type.eq(media_type.as_str()))
            .count()
            .get_result(self.conn)?;
        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init::init_db;
    use crate::models::media::HdrFormat;

    fn test_db() -> SqliteConnection {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        init_db(&mut conn).unwrap();
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
            watched: false,
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
            watched: false,
            library_section_id: None,
        }
    }

    #[test]
    fn upsert_and_find_by_id_roundtrip() {
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);
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
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);

        let mut movie = test_movie("m1", "Dune");
        repo.upsert(&movie).unwrap();

        movie.title = "Dune: Part Two".to_string();
        movie.year = Some(2024);
        repo.upsert(&movie).unwrap();

        let found = repo.find_by_id("m1").unwrap().unwrap();
        assert_eq!(found.title, "Dune: Part Two");
        assert_eq!(found.year, Some(2024));
        assert_eq!(repo.count_by_type(MediaType::Movie).unwrap(), 1);
    }

    #[test]
    fn upsert_preserves_added_at_on_update() {
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);

        let mut movie = test_movie("m1", "Dune");
        movie.added_at = "2020-01-01".to_string();
        repo.upsert(&movie).unwrap();

        movie.added_at = "2099-12-31".to_string(); // a re-sync reporting a different value
        movie.title = "Dune (updated)".to_string();
        repo.upsert(&movie).unwrap();

        let found = repo.find_by_id("m1").unwrap().unwrap();
        assert_eq!(found.added_at, "2020-01-01", "added_at preserved on update");
        assert_eq!(found.title, "Dune (updated)");
    }

    #[test]
    fn find_by_id_returns_none_for_missing() {
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);
        assert!(repo.find_by_id("nonexistent").unwrap().is_none());
    }

    #[test]
    fn list_by_type_returns_only_matching_type() {
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);

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
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);

        for i in 0..5 {
            repo.upsert(&test_movie(&format!("m{i}"), &format!("Movie {i}")))
                .unwrap();
        }

        assert_eq!(repo.list_by_type(MediaType::Movie, 2, 0).unwrap().len(), 2);
        assert_eq!(repo.list_by_type(MediaType::Movie, 2, 2).unwrap().len(), 2);
        assert_eq!(repo.list_by_type(MediaType::Movie, 2, 4).unwrap().len(), 1);
    }

    #[test]
    fn list_by_type_orders_by_title() {
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);

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
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);

        repo.upsert(&test_episode("e1", "show1", 1, 1)).unwrap();
        repo.upsert(&test_episode("e2", "show1", 1, 2)).unwrap();
        repo.upsert(&test_episode("e3", "show2", 1, 1)).unwrap();

        assert_eq!(repo.list_by_parent("show1").unwrap().len(), 2);
    }

    #[test]
    fn list_by_parent_orders_by_season_then_episode() {
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);

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
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);

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
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);

        assert_eq!(repo.count_by_type(MediaType::Movie).unwrap(), 0);
        repo.upsert(&test_movie("m1", "Dune")).unwrap();
        repo.upsert(&test_movie("m2", "Arrival")).unwrap();
        assert_eq!(repo.count_by_type(MediaType::Movie).unwrap(), 2);
    }

    #[test]
    fn genres_stored_as_json_array() {
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);

        repo.upsert(&test_movie("m1", "Dune")).unwrap();

        use diesel::sql_types::Text;
        #[derive(QueryableByName)]
        struct G {
            #[diesel(sql_type = Text)]
            genres: String,
        }
        let raw = diesel::sql_query("SELECT genres FROM media_items WHERE id = 'm1'")
            .get_result::<G>(&mut conn)
            .unwrap();
        assert_eq!(raw.genres, r#"["Action","Sci-Fi"]"#);

        let mut repo = MediaRepo::new(&mut conn);
        let found = repo.find_by_id("m1").unwrap().unwrap();
        assert_eq!(found.genres, vec!["Action", "Sci-Fi"]);
    }

    #[test]
    fn empty_genres_stored_correctly() {
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);

        let mut movie = test_movie("m1", "Dune");
        movie.genres = vec![];
        repo.upsert(&movie).unwrap();

        let found = repo.find_by_id("m1").unwrap().unwrap();
        assert!(found.genres.is_empty());
    }

    #[test]
    fn hdr_field_roundtrip_hdr() {
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);
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
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);
        let mut movie = test_movie("m1", "Dune");
        movie.hdr = Some(HdrFormat::DolbyVision);
        repo.upsert(&movie).unwrap();
        let found = repo.find_by_id("m1").unwrap().unwrap();
        assert_eq!(found.hdr, Some(HdrFormat::DolbyVision));
    }

    #[test]
    fn hdr_field_roundtrip_none() {
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);
        let movie = test_movie("m1", "Dune"); // hdr: None
        repo.upsert(&movie).unwrap();
        let found = repo.find_by_id("m1").unwrap().unwrap();
        assert_eq!(found.hdr, None);
    }

    #[test]
    fn upsert_updates_hdr_value() {
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);
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
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);

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

    #[test]
    fn upsert_clears_field_to_null_on_update() {
        let mut conn = test_db();
        let mut repo = MediaRepo::new(&mut conn);

        let mut movie = test_movie("m1", "Dune");
        movie.overview = Some("present".to_string());
        repo.upsert(&movie).unwrap();

        movie.overview = None; // treat_none_as_null must write NULL, not skip
        repo.upsert(&movie).unwrap();

        let found = repo.find_by_id("m1").unwrap().unwrap();
        assert_eq!(found.overview, None);
    }
}
