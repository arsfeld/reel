use diesel::prelude::*;

use crate::db::rows::{NewSourceRow, SourceRow};
use crate::models::source::Source;
use crate::schema::sources;

use super::DbError;

/// Repository for media source CRUD operations (diesel).
#[allow(dead_code)]
pub struct SourceRepo<'a> {
    conn: &'a mut SqliteConnection,
}

#[allow(dead_code)]
impl<'a> SourceRepo<'a> {
    pub fn new(conn: &'a mut SqliteConnection) -> Self {
        Self { conn }
    }

    pub fn insert(&mut self, source: &Source) -> Result<(), DbError> {
        let new = NewSourceRow::try_from(source)?;
        diesel::insert_into(sources::table)
            .values(&new)
            .execute(self.conn)?;
        Ok(())
    }

    pub fn find_by_id(&mut self, id: &str) -> Result<Option<Source>, DbError> {
        let row = sources::table
            .find(id)
            .select(SourceRow::as_select())
            .first::<SourceRow>(self.conn)
            .optional()?;
        row.map(Source::try_from).transpose()
    }

    pub fn list(&mut self) -> Result<Vec<Source>, DbError> {
        let rows = sources::table
            .order(sources::name.asc())
            .select(SourceRow::as_select())
            .load::<SourceRow>(self.conn)?;
        rows.into_iter().map(Source::try_from).collect()
    }

    pub fn update(&mut self, source: &Source) -> Result<(), DbError> {
        let changes = NewSourceRow::try_from(source)?;
        diesel::update(sources::table.find(&source.id))
            .set(&changes)
            .execute(self.conn)?;
        Ok(())
    }

    pub fn delete(&mut self, id: &str) -> Result<(), DbError> {
        diesel::delete(sources::table.find(id)).execute(self.conn)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init::init_db;
    use crate::models::media::SourceType;
    use crate::models::source::SourceConfig;

    fn test_db() -> SqliteConnection {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        init_db(&mut conn).unwrap();
        conn
    }

    fn test_source() -> Source {
        Source {
            id: "plex:http://localhost:32400".to_string(),
            source_type: SourceType::Plex,
            name: "My Plex Server".to_string(),
            config: SourceConfig {
                url: "http://localhost:32400".to_string(),
                token: "test-token-123".to_string(),
                user_id: None,
            },
            enabled: true,
            last_synced_at: None,
        }
    }

    #[test]
    fn insert_and_find_roundtrip() {
        let mut conn = test_db();
        let mut repo = SourceRepo::new(&mut conn);
        let source = test_source();

        repo.insert(&source).unwrap();
        let found = repo.find_by_id(&source.id).unwrap().unwrap();

        assert_eq!(found.name, "My Plex Server");
        assert_eq!(found.config.url, "http://localhost:32400");
        assert_eq!(found.config.token, "test-token-123");
        assert!(found.enabled);
        assert_eq!(found.last_synced_at, None);
    }

    #[test]
    fn find_by_id_returns_none_for_missing() {
        let mut conn = test_db();
        let mut repo = SourceRepo::new(&mut conn);
        assert!(repo.find_by_id("nonexistent").unwrap().is_none());
    }

    #[test]
    fn list_returns_all_sources() {
        let mut conn = test_db();
        let mut repo = SourceRepo::new(&mut conn);

        let mut s1 = test_source();
        s1.id = "plex:server1".to_string();
        s1.name = "Server A".to_string();
        let mut s2 = test_source();
        s2.id = "plex:server2".to_string();
        s2.name = "Server B".to_string();

        repo.insert(&s1).unwrap();
        repo.insert(&s2).unwrap();

        let all = repo.list().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].name, "Server A");
        assert_eq!(all[1].name, "Server B");
    }

    #[test]
    fn update_modifies_source() {
        let mut conn = test_db();
        let mut repo = SourceRepo::new(&mut conn);

        let mut source = test_source();
        repo.insert(&source).unwrap();

        source.name = "Updated Server".to_string();
        source.enabled = false;
        source.last_synced_at = Some("2024-03-14T10:00:00Z".to_string());
        repo.update(&source).unwrap();

        let found = repo.find_by_id(&source.id).unwrap().unwrap();
        assert_eq!(found.name, "Updated Server");
        assert!(!found.enabled);
        assert_eq!(
            found.last_synced_at,
            Some("2024-03-14T10:00:00Z".to_string())
        );
    }

    #[test]
    fn delete_removes_source() {
        let mut conn = test_db();
        let mut repo = SourceRepo::new(&mut conn);

        let source = test_source();
        repo.insert(&source).unwrap();
        assert!(repo.find_by_id(&source.id).unwrap().is_some());

        repo.delete(&source.id).unwrap();
        assert!(repo.find_by_id(&source.id).unwrap().is_none());
    }

    #[test]
    fn config_json_stored_correctly() {
        let mut conn = test_db();
        let mut repo = SourceRepo::new(&mut conn);

        let source = test_source();
        repo.insert(&source).unwrap();

        let found = repo.find_by_id(&source.id).unwrap().unwrap();
        assert_eq!(found.config.url, "http://localhost:32400");
        assert_eq!(found.config.token, "test-token-123");
    }
}
