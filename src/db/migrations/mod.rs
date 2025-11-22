pub use sea_orm_migration::prelude::*;

mod m20250101_000001_create_initial_schema;
mod m20250102_000001_add_episode_fields;
mod m20250103_000001_add_source_connections;
mod m20250104_000001_add_sync_total_items;
mod m20250105_000001_add_connection_tracking;
mod m20250106_000001_add_playqueue_fields;
mod m20250107_000001_add_home_sections;
mod m20250928_000001_add_auth_tokens;
mod m20250929_000001_add_cache_tracking;
mod m20251003_000001_add_people_tables;
mod m20251003_000002_add_marker_fields;
mod m20251004_000001_add_filter_indexes;
mod m20251120_000001_add_auth_status;
mod m20251122_000001_add_playback_sync_queue;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250101_000001_create_initial_schema::Migration),
            Box::new(m20250102_000001_add_episode_fields::Migration),
            Box::new(m20250103_000001_add_source_connections::Migration),
            Box::new(m20250104_000001_add_sync_total_items::Migration),
            Box::new(m20250105_000001_add_connection_tracking::Migration),
            Box::new(m20250106_000001_add_playqueue_fields::Migration),
            Box::new(m20250107_000001_add_home_sections::Migration),
            Box::new(m20250928_000001_add_auth_tokens::Migration),
            Box::new(m20250929_000001_add_cache_tracking::Migration),
            Box::new(m20251003_000001_add_people_tables::Migration),
            Box::new(m20251003_000002_add_marker_fields::Migration),
            Box::new(m20251004_000001_add_filter_indexes::Migration),
            Box::new(m20251120_000001_add_auth_status::Migration),
            Box::new(m20251122_000001_add_playback_sync_queue::Migration),
        ]
    }
}
