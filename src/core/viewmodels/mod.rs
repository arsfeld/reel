pub mod authentication_view_model;
pub mod details_view_model;
pub mod home_view_model;
pub mod library_view_model;
pub mod player_view_model;
pub mod preferences_view_model;
pub mod property;
pub mod sidebar_view_model;
pub mod sources_view_model;

pub use authentication_view_model::AuthenticationViewModel;
pub use details_view_model::DetailsViewModel;
pub use preferences_view_model::PreferencesViewModel;
pub use property::{Property, PropertySubscriber};
pub use sidebar_view_model::SidebarViewModel;

use crate::events::EventBus;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait ViewModel: Send + Sync {
    async fn initialize(&self, event_bus: Arc<EventBus>);

    fn subscribe_to_property(&self, property_name: &str) -> Option<PropertySubscriber>;

    async fn refresh(&self);

    fn dispose(&self);
}
