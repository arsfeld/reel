pub mod details_view_model;
pub mod home_view_model;
pub mod library_view_model;
pub mod player_view_model;
pub mod property;
pub mod sidebar_view_model;
pub mod sources_view_model;

pub use details_view_model::DetailsViewModel;
pub use home_view_model::HomeViewModel;
pub use library_view_model::LibraryViewModel;
pub use player_view_model::PlayerViewModel;
pub use property::{Property, PropertySubscriber};
pub use sidebar_view_model::SidebarViewModel;
pub use sources_view_model::SourcesViewModel;

use crate::events::EventBus;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait ViewModel: Send + Sync {
    async fn initialize(&self, event_bus: Arc<EventBus>);

    fn subscribe_to_property(&self, property_name: &str) -> Option<PropertySubscriber>;

    async fn refresh(&self);

    fn dispose(&self);
}
