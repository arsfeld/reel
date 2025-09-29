pub use sea_orm_migration::prelude::*;

mod m20250101_000001_create_initial_schema;
mod m20250102_000001_add_episode_fields;
mod m20250103_000001_add_source_connections;
mod m20250104_000001_add_sync_total_items;
mod m20250105_000001_add_connection_tracking;
mod m20250106_000001_add_playqueue_fields;
mod m20250107_000001_add_home_sections;
mod m20250928_000001_add_auth_tokens;

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
        ]
    }
}
