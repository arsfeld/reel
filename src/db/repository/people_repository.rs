use super::{BaseRepository, Repository};
use crate::db::entities::{
    media_people::{
        self, ActiveModel as MediaPeopleActiveModel, Entity as MediaPeopleEntity,
        Model as MediaPeopleModel,
    },
    people::{ActiveModel as PeopleActiveModel, Entity as PeopleEntity, Model as PeopleModel},
};
use anyhow::Result;
use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};
use std::sync::Arc;

/// Repository trait for People entities
#[async_trait]
pub trait PeopleRepository: Repository<PeopleModel> {
    /// Upsert a person (insert or update if exists)
    async fn upsert(&self, person: PeopleModel) -> Result<PeopleModel>;

    /// Upsert multiple people at once
    async fn upsert_batch(&self, people: Vec<PeopleModel>) -> Result<Vec<PeopleModel>>;

    /// Find all people associated with a media item
    async fn find_by_media_item(
        &self,
        media_item_id: &str,
    ) -> Result<Vec<(PeopleModel, MediaPeopleModel)>>;

    /// Save people relationships for a media item (replaces existing)
    async fn save_media_people(
        &self,
        media_item_id: &str,
        people: Vec<MediaPeopleModel>,
    ) -> Result<()>;

    /// Delete all people relationships for a media item
    async fn delete_media_people(&self, media_item_id: &str) -> Result<()>;
}

#[derive(Debug)]
pub struct PeopleRepositoryImpl {
    base: BaseRepository,
}

impl PeopleRepositoryImpl {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            base: BaseRepository::new(db),
        }
    }
}

#[async_trait]
impl Repository<PeopleModel> for PeopleRepositoryImpl {
    type Entity = PeopleEntity;

    async fn find_by_id(&self, id: &str) -> Result<Option<PeopleModel>> {
        Ok(PeopleEntity::find_by_id(id)
            .one(self.base.db.as_ref())
            .await?)
    }

    async fn find_all(&self) -> Result<Vec<PeopleModel>> {
        Ok(PeopleEntity::find().all(self.base.db.as_ref()).await?)
    }

    async fn insert(&self, entity: PeopleModel) -> Result<PeopleModel> {
        let active_model = PeopleActiveModel {
            id: Set(entity.id),
            name: Set(entity.name),
            image_url: Set(entity.image_url),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(chrono::Utc::now().naive_utc()),
        };

        Ok(active_model.insert(self.base.db.as_ref()).await?)
    }

    async fn update(&self, entity: PeopleModel) -> Result<PeopleModel> {
        let active_model = PeopleActiveModel {
            id: Set(entity.id),
            name: Set(entity.name),
            image_url: Set(entity.image_url),
            created_at: Set(entity.created_at),
            updated_at: Set(chrono::Utc::now().naive_utc()),
        };

        Ok(active_model.update(self.base.db.as_ref()).await?)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        PeopleEntity::delete_by_id(id)
            .exec(self.base.db.as_ref())
            .await?;
        Ok(())
    }

    async fn count(&self) -> Result<u64> {
        Ok(PeopleEntity::find().count(self.base.db.as_ref()).await?)
    }
}

#[async_trait]
impl PeopleRepository for PeopleRepositoryImpl {
    async fn upsert(&self, person: PeopleModel) -> Result<PeopleModel> {
        // Check if person exists
        let existing = self.find_by_id(&person.id).await?;

        if existing.is_some() {
            // Update existing person
            self.update(person).await
        } else {
            // Insert new person
            self.insert(person).await
        }
    }

    async fn upsert_batch(&self, people: Vec<PeopleModel>) -> Result<Vec<PeopleModel>> {
        let mut results = Vec::with_capacity(people.len());
        for person in people {
            results.push(self.upsert(person).await?);
        }
        Ok(results)
    }

    async fn find_by_media_item(
        &self,
        media_item_id: &str,
    ) -> Result<Vec<(PeopleModel, MediaPeopleModel)>> {
        use sea_orm::QueryOrder;

        // First get media_people records for this media item
        let media_people_records = MediaPeopleEntity::find()
            .filter(media_people::Column::MediaItemId.eq(media_item_id))
            .order_by_asc(media_people::Column::SortOrder)
            .all(self.base.db.as_ref())
            .await?;

        // Then get the corresponding people records
        let mut results = Vec::new();
        for media_person in media_people_records {
            if let Some(person) = PeopleEntity::find_by_id(&media_person.person_id)
                .one(self.base.db.as_ref())
                .await?
            {
                results.push((person, media_person));
            }
        }

        Ok(results)
    }

    async fn save_media_people(
        &self,
        media_item_id: &str,
        people: Vec<MediaPeopleModel>,
    ) -> Result<()> {
        // First, delete existing relationships
        self.delete_media_people(media_item_id).await?;

        // Then insert new relationships
        if !people.is_empty() {
            use sea_orm::ActiveValue::NotSet;

            let active_models: Vec<MediaPeopleActiveModel> = people
                .into_iter()
                .map(|p| MediaPeopleActiveModel {
                    id: NotSet, // Let database auto-generate the ID
                    media_item_id: Set(p.media_item_id),
                    person_id: Set(p.person_id),
                    person_type: Set(p.person_type),
                    role: Set(p.role),
                    sort_order: Set(p.sort_order),
                })
                .collect();

            MediaPeopleEntity::insert_many(active_models)
                .exec(self.base.db.as_ref())
                .await?;
        }

        Ok(())
    }

    async fn delete_media_people(&self, media_item_id: &str) -> Result<()> {
        MediaPeopleEntity::delete_many()
            .filter(media_people::Column::MediaItemId.eq(media_item_id))
            .exec(self.base.db.as_ref())
            .await?;
        Ok(())
    }
}
