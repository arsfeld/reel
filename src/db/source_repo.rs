use rusqlite::{Connection, params};

use crate::models::{
    media::SourceType,
    source::{Source, SourceConfig},
};

use super::DbError;

/// Repository for media source CRUD operations.
pub struct SourceRepo<'a> {
    conn: &'a Connection,
}

#[allow(dead_code)]
impl<'a> SourceRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn insert(&self, source: &Source) -> Result<(), DbError> {
        let config_json = serde_json::to_string(&source.config)
            .map_err(|e| DbError::Data(format!("Failed to serialize config: {e}")))?;

        self.conn.execute(
            "INSERT INTO sources (id, source_type, name, config, enabled, last_synced_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                source.id,
                source.source_type.as_str(),
                source.name,
                config_json,
                source.enabled,
                source.last_synced_at,
            ],
        )?;
        Ok(())
    }

    pub fn find_by_id(&self, id: &str) -> Result<Option<Source>, DbError> {
        let mut stmt = self.conn.prepare("SELECT * FROM sources WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_source(row)?)),
            None => Ok(None),
        }
    }

    pub fn list(&self) -> Result<Vec<Source>, DbError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM sources ORDER BY name ASC")?;
        let rows = stmt.query_map([], |row| {
            row_to_source(row).map_err(|e| match e {
                DbError::Sqlite(e) => e,
                other => rusqlite::Error::ToSqlConversionFailure(Box::new(other)),
            })
        })?;

        let mut sources = Vec::new();
        for row in rows {
            sources.push(row?);
        }
        Ok(sources)
    }

    pub fn update(&self, source: &Source) -> Result<(), DbError> {
        let config_json = serde_json::to_string(&source.config)
            .map_err(|e| DbError::Data(format!("Failed to serialize config: {e}")))?;

        self.conn.execute(
            "UPDATE sources SET source_type = ?2, name = ?3, config = ?4, enabled = ?5, last_synced_at = ?6
             WHERE id = ?1",
            params![
                source.id,
                source.source_type.as_str(),
                source.name,
                config_json,
                source.enabled,
                source.last_synced_at,
            ],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), DbError> {
        self.conn
            .execute("DELETE FROM sources WHERE id = ?1", params![id])?;
        Ok(())
    }
}

fn row_to_source(row: &rusqlite::Row) -> Result<Source, DbError> {
    let source_type_str: String = row.get("source_type")?;
    let config_json: String = row.get("config")?;

    let source_type = SourceType::from_str(&source_type_str)
        .ok_or_else(|| DbError::Data(format!("Unknown source type: {source_type_str}")))?;
    let config: SourceConfig = serde_json::from_str(&config_json)
        .map_err(|e| DbError::Data(format!("Failed to parse config: {e}")))?;

    Ok(Source {
        id: row.get("id")?,
        source_type,
        name: row.get("name")?,
        config,
        enabled: row.get("enabled")?,
        last_synced_at: row.get("last_synced_at")?,
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
        let conn = test_db();
        let repo = SourceRepo::new(&conn);
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
        let conn = test_db();
        let repo = SourceRepo::new(&conn);
        assert!(repo.find_by_id("nonexistent").unwrap().is_none());
    }

    #[test]
    fn list_returns_all_sources() {
        let conn = test_db();
        let repo = SourceRepo::new(&conn);

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
        // Ordered by name
        assert_eq!(all[0].name, "Server A");
        assert_eq!(all[1].name, "Server B");
    }

    #[test]
    fn update_modifies_source() {
        let conn = test_db();
        let repo = SourceRepo::new(&conn);

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
        let conn = test_db();
        let repo = SourceRepo::new(&conn);

        let source = test_source();
        repo.insert(&source).unwrap();
        assert!(repo.find_by_id(&source.id).unwrap().is_some());

        repo.delete(&source.id).unwrap();
        assert!(repo.find_by_id(&source.id).unwrap().is_none());
    }

    #[test]
    fn config_json_stored_correctly() {
        let conn = test_db();
        let repo = SourceRepo::new(&conn);

        let source = test_source();
        repo.insert(&source).unwrap();

        // Verify raw JSON
        let raw: String = conn
            .query_row(
                "SELECT config FROM sources WHERE id = ?1",
                params![source.id],
                |row| row.get(0),
            )
            .unwrap();

        let parsed: SourceConfig = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.url, "http://localhost:32400");
        assert_eq!(parsed.token, "test-token-123");
    }
}
