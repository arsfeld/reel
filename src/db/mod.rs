pub mod connection;
pub mod entities;
pub mod migrations;
pub mod repository;

pub use connection::{Database, DatabaseConnection};
pub use migrations::Migrator;

use sea_orm::DatabaseConnection as SeaOrmConnection;
use std::sync::Arc;

/// Re-export commonly used SeaORM types
pub use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, QueryOrder,
    TransactionTrait,
};
