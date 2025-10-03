use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "media_people")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub media_item_id: String,
    pub person_id: String,
    pub person_type: String,     // 'actor', 'director', 'writer', 'producer'
    pub role: Option<String>,    // Character name for actors
    pub sort_order: Option<i32>, // Display order
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::media_items::Entity",
        from = "Column::MediaItemId",
        to = "super::media_items::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    MediaItem,
    #[sea_orm(
        belongs_to = "super::people::Entity",
        from = "Column::PersonId",
        to = "super::people::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Person,
}

impl Related<super::media_items::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MediaItem.def()
    }
}

impl Related<super::people::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Person.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Person type enum for type safety
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PersonType {
    Actor,
    Director,
    Writer,
    Producer,
    Other(String),
}

impl PersonType {
    pub fn as_str(&self) -> &str {
        match self {
            PersonType::Actor => "actor",
            PersonType::Director => "director",
            PersonType::Writer => "writer",
            PersonType::Producer => "producer",
            PersonType::Other(s) => s.as_str(),
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "actor" => PersonType::Actor,
            "director" => PersonType::Director,
            "writer" => PersonType::Writer,
            "producer" => PersonType::Producer,
            _ => PersonType::Other(s.to_string()),
        }
    }
}
