use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "home_sections")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub source_id: String,
    pub hub_identifier: String,
    pub title: String,
    pub section_type: String,
    pub position: i32,
    pub context: Option<String>,
    pub style: Option<String>,
    pub hub_type: Option<String>,
    pub size: Option<i32>,
    pub last_updated: DateTime,
    pub is_stale: bool,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::sources::Entity",
        from = "Column::SourceId",
        to = "super::sources::Column::Id"
    )]
    Source,
    #[sea_orm(has_many = "super::home_section_items::Entity")]
    HomeSectionItems,
}

impl Related<super::sources::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Source.def()
    }
}

impl Related<super::home_section_items::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::HomeSectionItems.def()
    }
}

// Many-to-many relation with media_items through home_section_items junction table
impl Related<super::media_items::Entity> for Entity {
    fn to() -> RelationDef {
        super::home_section_items::Relation::MediaItem.def()
    }

    fn via() -> Option<RelationDef> {
        Some(super::home_section_items::Relation::HomeSection.def().rev())
    }
}

impl ActiveModelBehavior for ActiveModel {}
