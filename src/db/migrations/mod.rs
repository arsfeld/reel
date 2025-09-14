pub use sea_orm_migration::prelude::*;

mod m20250101_000001_create_initial_schema;
mod m20250102_000001_add_episode_fields;
mod m20250103_000001_add_source_connections;
mod m20250104_000001_add_sync_total_items;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250101_000001_create_initial_schema::Migration),
            Box::new(m20250102_000001_add_episode_fields::Migration),
            Box::new(m20250103_000001_add_source_connections::Migration),
            Box::new(m20250104_000001_add_sync_total_items::Migration),
        ]
    }
}
