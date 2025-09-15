pub mod builders;
pub mod fixtures;
pub mod mocks;

use reel::db::{connection::DatabaseConnection, entities::*};
use sea_orm::{Database, DatabaseConnection as DbConn, EntityTrait, Set};
use std::sync::Arc;
use tempfile::TempDir;

pub struct TestContext {
    pub db: Arc<DatabaseConnection>,
    _temp_dir: TempDir,
}

impl TestContext {
    pub async fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

        let db_conn = Database::connect(&db_url)
            .await
            .expect("Failed to connect to test database");

        let db = Arc::new(DatabaseConnection::new(db_conn));

        // Run migrations
        db.run_migrations().await.expect("Failed to run migrations");

        Self {
            db,
            _temp_dir: temp_dir,
        }
    }

    pub fn db(&self) -> Arc<DatabaseConnection> {
        self.db.clone()
    }
}

pub async fn create_test_database() -> Arc<DatabaseConnection> {
    let context = TestContext::new().await;
    context.db()
}

pub async fn seed_test_data(db: &Arc<DatabaseConnection>) -> TestData {
    use chrono::Utc;
    use reel::models::{MediaType, ServerType};

    // Create test source
    let source = sources::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        name: Set("Test Plex Server".to_string()),
        server_type: Set(ServerType::Plex.to_string()),
        address: Set("http://localhost:32400".to_string()),
        machine_identifier: Set(Some("test_machine_123".to_string())),
        access_token: Set(Some("test_token".to_string())),
        is_active: Set(true),
        created_at: Set(Utc::now().timestamp()),
        updated_at: Set(Utc::now().timestamp()),
        ..Default::default()
    };
    let source = source.insert(db.connection()).await.unwrap();

    // Create test library
    let library = libraries::ActiveModel {
        id: Set(uuid::Uuid::new_v4().to_string()),
        source_id: Set(source.id.clone()),
        name: Set("Test Movies".to_string()),
        library_type: Set(MediaType::Movie.to_string()),
        item_count: Set(10),
        created_at: Set(Utc::now().timestamp()),
        updated_at: Set(Utc::now().timestamp()),
        ..Default::default()
    };
    let library = library.insert(db.connection()).await.unwrap();

    // Create test media items
    let mut media_items = Vec::new();
    for i in 1..=5 {
        let movie = media_items::ActiveModel {
            id: Set(format!("movie_{}", i)),
            source_id: Set(source.id.clone()),
            library_id: Set(library.id.clone()),
            media_type: Set(MediaType::Movie.to_string()),
            title: Set(format!("Test Movie {}", i)),
            sort_title: Set(format!("Test Movie {}", i)),
            year: Set(Some(2020 + i)),
            duration_ms: Set(Some(120 * 60 * 1000)), // 2 hours
            rating: Set(Some(7.5 + (i as f32) * 0.2)),
            added_at: Set(Utc::now().timestamp()),
            updated_at: Set(Utc::now().timestamp()),
            ..Default::default()
        };
        let movie = movie.insert(db.connection()).await.unwrap();
        media_items.push(movie);
    }

    TestData {
        source,
        library,
        media_items,
    }
}

pub struct TestData {
    pub source: sources::Model,
    pub library: libraries::Model,
    pub media_items: Vec<media_items::Model>,
}

pub struct TestComponentBuilder;

impl TestComponentBuilder {
    pub fn new() -> Self {
        Self
    }
}

pub struct TestApp {
    pub db: Arc<DatabaseConnection>,
}

impl TestApp {
    pub async fn new() -> Self {
        let db = create_test_database().await;
        Self { db }
    }

    pub async fn with_test_data() -> (Self, TestData) {
        let app = Self::new().await;
        let test_data = seed_test_data(&app.db).await;
        (app, test_data)
    }
}
