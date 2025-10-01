use super::{BaseRepository, Repository};
use crate::db::entities::{
    HomeSectionModel, MediaItemModel, home_section_items, home_sections, media_items,
};
use anyhow::Result;
use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel,
    PaginatorTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use std::sync::Arc;

/// Repository trait for HomeSection entities
#[async_trait]
pub trait HomeSectionRepository: Repository<HomeSectionModel> {
    /// Save or update sections from API, replacing existing sections for the source
    async fn save_sections(
        &self,
        source_id: &str,
        sections: Vec<HomeSectionModel>,
        section_items: Vec<(i32, Vec<String>)>,
    ) -> Result<()>;

    /// Find all sections for a source, ordered by position
    async fn find_by_source(&self, source_id: &str) -> Result<Vec<HomeSectionModel>>;

    /// Find all sections for a source with their associated media items
    async fn find_by_source_with_items(
        &self,
        source_id: &str,
    ) -> Result<Vec<(HomeSectionModel, Vec<MediaItemModel>)>>;

    /// Clear all sections for a source (used during refresh operations)
    async fn clear_sections_for_source(&self, source_id: &str) -> Result<()>;

    /// Get media items for a specific section
    async fn get_section_items(&self, section_id: i32) -> Result<Vec<MediaItemModel>>;

    /// Add media items to a section
    async fn add_section_items(&self, section_id: i32, media_item_ids: Vec<String>) -> Result<()>;

    /// Remove all items from a section
    async fn clear_section_items(&self, section_id: i32) -> Result<()>;

    /// Mark sections as stale (need refresh)
    async fn mark_sections_stale(&self, source_id: &str) -> Result<()>;

    /// Check if sections exist for a source
    async fn has_sections(&self, source_id: &str) -> Result<bool>;

    /// Find sections by hub identifier
    async fn find_by_hub_identifier(
        &self,
        source_id: &str,
        hub_identifier: &str,
    ) -> Result<Option<HomeSectionModel>>;
}

/// Implementation of HomeSectionRepository
pub struct HomeSectionRepositoryImpl {
    base: BaseRepository,
}

impl HomeSectionRepositoryImpl {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            base: BaseRepository::new(db),
        }
    }
}

#[async_trait]
impl Repository<HomeSectionModel> for HomeSectionRepositoryImpl {
    type Entity = home_sections::Entity;

    async fn find_by_id(&self, id: &str) -> Result<Option<HomeSectionModel>> {
        let id_parsed = id.parse::<i32>()?;
        let result = home_sections::Entity::find_by_id(id_parsed)
            .one(self.base.db.as_ref())
            .await?;
        Ok(result)
    }

    async fn find_all(&self) -> Result<Vec<HomeSectionModel>> {
        let results = home_sections::Entity::find()
            .all(self.base.db.as_ref())
            .await?;
        Ok(results)
    }

    async fn insert(&self, entity: HomeSectionModel) -> Result<HomeSectionModel> {
        let active_model = entity.into_active_model();
        let result = active_model.insert(self.base.db.as_ref()).await?;
        Ok(result)
    }

    async fn update(&self, entity: HomeSectionModel) -> Result<HomeSectionModel> {
        let active_model = entity.into_active_model();
        let result = active_model.update(self.base.db.as_ref()).await?;
        Ok(result)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let id_parsed = id.parse::<i32>()?;
        home_sections::Entity::delete_by_id(id_parsed)
            .exec(self.base.db.as_ref())
            .await?;
        Ok(())
    }

    async fn count(&self) -> Result<u64> {
        let count = home_sections::Entity::find()
            .count(self.base.db.as_ref())
            .await?;
        Ok(count)
    }
}

#[async_trait]
impl HomeSectionRepository for HomeSectionRepositoryImpl {
    async fn save_sections(
        &self,
        source_id: &str,
        sections: Vec<HomeSectionModel>,
        section_items: Vec<(i32, Vec<String>)>,
    ) -> Result<()> {
        // Use transaction for atomic updates
        let txn = self.base.db.begin().await?;

        // First, clear existing sections for this source
        home_sections::Entity::delete_many()
            .filter(home_sections::Column::SourceId.eq(source_id))
            .exec(&txn)
            .await?;

        // Insert new sections
        let mut inserted_sections = Vec::new();
        for section in sections {
            // Create ActiveModel with NotSet for id so it auto-increments
            let mut active_model = section.into_active_model();
            active_model.id = ActiveValue::NotSet;
            let inserted = active_model.insert(&txn).await?;
            inserted_sections.push(inserted);
        }

        // Insert section items
        for (section_idx, (_, media_ids)) in section_items.iter().enumerate() {
            if section_idx < inserted_sections.len() {
                let section_id = inserted_sections[section_idx].id;

                // Insert items for this section
                for (position, media_id) in media_ids.iter().enumerate() {
                    let item = home_section_items::ActiveModel {
                        id: ActiveValue::NotSet, // Auto-increment
                        section_id: Set(section_id),
                        media_item_id: Set(media_id.clone()),
                        position: Set(position as i32),
                        created_at: Set(chrono::Utc::now().naive_utc()),
                    };
                    item.insert(&txn).await?;
                }
            }
        }

        txn.commit().await?;
        Ok(())
    }

    async fn find_by_source(&self, source_id: &str) -> Result<Vec<HomeSectionModel>> {
        let results = home_sections::Entity::find()
            .filter(home_sections::Column::SourceId.eq(source_id))
            .order_by_asc(home_sections::Column::Position)
            .all(self.base.db.as_ref())
            .await?;
        Ok(results)
    }

    async fn find_by_source_with_items(
        &self,
        source_id: &str,
    ) -> Result<Vec<(HomeSectionModel, Vec<MediaItemModel>)>> {
        // Get all sections for the source
        let sections = self.find_by_source(source_id).await?;
        let mut results = Vec::new();

        for section in sections {
            // Get items for each section
            let items = self.get_section_items(section.id).await?;
            results.push((section, items));
        }

        Ok(results)
    }

    async fn clear_sections_for_source(&self, source_id: &str) -> Result<()> {
        // Delete all sections for the source (cascade will handle section items)
        home_sections::Entity::delete_many()
            .filter(home_sections::Column::SourceId.eq(source_id))
            .exec(self.base.db.as_ref())
            .await?;
        Ok(())
    }

    async fn get_section_items(&self, section_id: i32) -> Result<Vec<MediaItemModel>> {
        // Get all items for a section, joined with media_items, ordered by position
        let items = home_section_items::Entity::find()
            .filter(home_section_items::Column::SectionId.eq(section_id))
            .find_also_related(media_items::Entity)
            .order_by_asc(home_section_items::Column::Position)
            .all(self.base.db.as_ref())
            .await?;

        // Extract media items that were found
        let media_items: Vec<MediaItemModel> =
            items.into_iter().filter_map(|(_, media)| media).collect();

        Ok(media_items)
    }

    async fn add_section_items(&self, section_id: i32, media_item_ids: Vec<String>) -> Result<()> {
        // Get current max position
        let max_position = home_section_items::Entity::find()
            .filter(home_section_items::Column::SectionId.eq(section_id))
            .order_by_desc(home_section_items::Column::Position)
            .one(self.base.db.as_ref())
            .await?
            .map(|item| item.position)
            .unwrap_or(-1);

        // Insert new items
        for (idx, media_id) in media_item_ids.iter().enumerate() {
            let item = home_section_items::ActiveModel {
                id: ActiveValue::NotSet, // Auto-increment
                section_id: Set(section_id),
                media_item_id: Set(media_id.clone()),
                position: Set(max_position + 1 + idx as i32),
                created_at: Set(chrono::Utc::now().naive_utc()),
            };
            item.insert(self.base.db.as_ref()).await?;
        }

        Ok(())
    }

    async fn clear_section_items(&self, section_id: i32) -> Result<()> {
        home_section_items::Entity::delete_many()
            .filter(home_section_items::Column::SectionId.eq(section_id))
            .exec(self.base.db.as_ref())
            .await?;
        Ok(())
    }

    async fn mark_sections_stale(&self, source_id: &str) -> Result<()> {
        // Update all sections for the source to be stale
        let sections = home_sections::Entity::find()
            .filter(home_sections::Column::SourceId.eq(source_id))
            .all(self.base.db.as_ref())
            .await?;

        for section in sections {
            let mut active_model = section.into_active_model();
            active_model.is_stale = Set(true);
            active_model.update(self.base.db.as_ref()).await?;
        }

        Ok(())
    }

    async fn has_sections(&self, source_id: &str) -> Result<bool> {
        let count = home_sections::Entity::find()
            .filter(home_sections::Column::SourceId.eq(source_id))
            .count(self.base.db.as_ref())
            .await?;
        Ok(count > 0)
    }

    async fn find_by_hub_identifier(
        &self,
        source_id: &str,
        hub_identifier: &str,
    ) -> Result<Option<HomeSectionModel>> {
        let result = home_sections::Entity::find()
            .filter(home_sections::Column::SourceId.eq(source_id))
            .filter(home_sections::Column::HubIdentifier.eq(hub_identifier))
            .one(self.base.db.as_ref())
            .await?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::get_test_db_connection;
    use crate::db::entities::sources;
    use chrono::Utc;

    async fn setup_test_db() -> Arc<DatabaseConnection> {
        let db = get_test_db_connection().await.unwrap();

        // Create a test source for foreign key constraints
        let test_source = sources::ActiveModel {
            id: Set("test_source".to_string()),
            name: Set("Test Source".to_string()),
            source_type: Set("plex".to_string()),
            auth_provider_id: Set(None),
            connection_url: Set(Some("http://test.local".to_string())),
            connections: Set(None),
            machine_id: Set(None),
            is_owned: Set(false),
            is_online: Set(true),
            last_sync: Set(None),
            last_connection_test: Set(None),
            connection_failure_count: Set(0),
            connection_quality: Set(None),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Utc::now().naive_utc()),
        };
        test_source.insert(&db).await.unwrap();

        Arc::new(db)
    }

    fn create_test_section(source_id: &str, position: i32) -> HomeSectionModel {
        HomeSectionModel {
            id: 0, // Will be auto-generated
            source_id: source_id.to_string(),
            hub_identifier: format!("hub_{}", position),
            title: format!("Section {}", position),
            section_type: "movie".to_string(),
            position,
            context: None,
            style: Some("shelf".to_string()),
            hub_type: Some("video".to_string()),
            size: Some(10),
            last_updated: Utc::now().naive_utc(),
            is_stale: false,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        }
    }

    #[tokio::test]
    async fn test_save_and_retrieve_sections() {
        let db = setup_test_db().await;
        let repo = HomeSectionRepositoryImpl::new(db);

        let source_id = "test_source";
        let sections = vec![
            create_test_section(source_id, 0),
            create_test_section(source_id, 1),
        ];

        // Save sections without items for simplicity
        repo.save_sections(source_id, sections.clone(), vec![])
            .await
            .unwrap();

        // Retrieve and verify
        let retrieved = repo.find_by_source(source_id).await.unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].position, 0);
        assert_eq!(retrieved[1].position, 1);
    }

    #[tokio::test]
    async fn test_clear_sections_for_source() {
        let db = setup_test_db().await;
        let repo = HomeSectionRepositoryImpl::new(db);

        let source_id = "test_source";
        let sections = vec![create_test_section(source_id, 0)];

        // Save and then clear
        repo.save_sections(source_id, sections, vec![])
            .await
            .unwrap();
        repo.clear_sections_for_source(source_id).await.unwrap();

        // Verify cleared
        let retrieved = repo.find_by_source(source_id).await.unwrap();
        assert_eq!(retrieved.len(), 0);
    }

    #[tokio::test]
    async fn test_mark_sections_stale() {
        let db = setup_test_db().await;
        let repo = HomeSectionRepositoryImpl::new(db);

        let source_id = "test_source";
        let sections = vec![create_test_section(source_id, 0)];

        // Save and mark stale
        repo.save_sections(source_id, sections, vec![])
            .await
            .unwrap();
        repo.mark_sections_stale(source_id).await.unwrap();

        // Verify stale flag
        let retrieved = repo.find_by_source(source_id).await.unwrap();
        assert!(retrieved[0].is_stale);
    }

    #[tokio::test]
    async fn test_has_sections() {
        let db = setup_test_db().await;
        let repo = HomeSectionRepositoryImpl::new(db);

        let source_id = "test_source";

        // Check empty
        assert!(!repo.has_sections(source_id).await.unwrap());

        // Save and check again
        let sections = vec![create_test_section(source_id, 0)];
        repo.save_sections(source_id, sections, vec![])
            .await
            .unwrap();
        assert!(repo.has_sections(source_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_find_by_hub_identifier() {
        let db = setup_test_db().await;
        let repo = HomeSectionRepositoryImpl::new(db);

        let source_id = "test_source";
        let sections = vec![create_test_section(source_id, 0)];

        repo.save_sections(source_id, sections.clone(), vec![])
            .await
            .unwrap();

        // Find by hub identifier
        let found = repo
            .find_by_hub_identifier(source_id, "hub_0")
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().hub_identifier, "hub_0");

        // Not found
        let not_found = repo
            .find_by_hub_identifier(source_id, "nonexistent")
            .await
            .unwrap();
        assert!(not_found.is_none());
    }
}
