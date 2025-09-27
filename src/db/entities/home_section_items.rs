use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "home_section_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub section_id: i32,
    pub media_item_id: String,
    pub position: i32,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::home_sections::Entity",
        from = "Column::SectionId",
        to = "super::home_sections::Column::Id"
    )]
    HomeSection,
    #[sea_orm(
        belongs_to = "super::media_items::Entity",
        from = "Column::MediaItemId",
        to = "super::media_items::Column::Id"
    )]
    MediaItem,
}

impl Related<super::home_sections::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::HomeSection.def()
    }
}

impl Related<super::media_items::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MediaItem.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
