use adw::prelude::*;
use gtk4::{self, glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use std::cell::RefCell;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::models::{Episode, Show};
use crate::platforms::gtk::ui::reactive::bindings::{
    bind_css_class_to_property, bind_dropdown_to_property, bind_image_icon_to_property,
    bind_image_to_property, bind_label_to_property, bind_text_to_computed_property,
    bind_text_to_property, bind_visibility_to_computed_property, bind_visibility_to_property,
};
use crate::platforms::gtk::ui::viewmodels::{DetailsViewModel, ViewModel};
use crate::state::AppState;
use crate::utils::{ImageLoader, ImageSize};

// Global image loader instance
use once_cell::sync::Lazy;
static IMAGE_LOADER: Lazy<ImageLoader> =
    Lazy::new(|| ImageLoader::new().expect("Failed to create ImageLoader"));

mod imp {
    use super::*;
    use gtk4::CompositeTemplate;

    #[derive(CompositeTemplate)]
    #[template(resource = "/dev/arsfeld/Reel/show_details.ui")]
    pub struct ShowDetailsPage {
        #[template_child]
        pub show_poster: TemplateChild<gtk4::Picture>,
        #[template_child]
        pub show_backdrop: TemplateChild<gtk4::Picture>,
        #[template_child]
        pub poster_placeholder: TemplateChild<gtk4::Box>,
        #[template_child]
        pub show_title: TemplateChild<gtk4::Label>,
        #[template_child]
        pub year_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub rating_box: TemplateChild<gtk4::Box>,
        #[template_child]
        pub rating_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub seasons_box: TemplateChild<gtk4::Box>,
        #[template_child]
        pub seasons_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub synopsis_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub genres_box: TemplateChild<gtk4::FlowBox>,
        #[template_child]
        pub season_dropdown: TemplateChild<gtk4::DropDown>,
        #[template_child]
        pub mark_watched_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub watched_icon: TemplateChild<gtk4::Image>,
        #[template_child]
        pub watched_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub episodes_box: TemplateChild<gtk4::Box>,
        #[template_child]
        pub episodes_carousel: TemplateChild<gtk4::ScrolledWindow>,
        #[template_child]
        pub episodes_count_label: TemplateChild<gtk4::Label>,

        // Optional show info fields
        #[template_child]
        pub show_info_section: TemplateChild<gtk4::Box>,
        #[template_child]
        pub network_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub network_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub status_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub status_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub content_rating_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub content_rating_label: TemplateChild<gtk4::Label>,

        pub state: RefCell<Option<Arc<AppState>>>,
        pub viewmodel: RefCell<Option<Arc<DetailsViewModel>>>,
        pub on_episode_selected: RefCell<Option<Box<dyn Fn(&Episode)>>>,
        pub load_generation: RefCell<u64>,
        pub property_subscriptions: RefCell<Vec<tokio::sync::broadcast::Receiver<()>>>,
        pub binding_handles:
            RefCell<Vec<crate::platforms::gtk::ui::reactive::bindings::BindingHandle>>,
    }

    impl Default for ShowDetailsPage {
        fn default() -> Self {
            Self {
                show_poster: Default::default(),
                show_backdrop: Default::default(),
                poster_placeholder: Default::default(),
                show_title: Default::default(),
                year_label: Default::default(),
                rating_box: Default::default(),
                rating_label: Default::default(),
                seasons_box: Default::default(),
                seasons_label: Default::default(),
                synopsis_label: Default::default(),
                genres_box: Default::default(),
                season_dropdown: Default::default(),
                mark_watched_button: Default::default(),
                watched_icon: Default::default(),
                watched_label: Default::default(),
                episodes_box: Default::default(),
                episodes_carousel: Default::default(),
                episodes_count_label: Default::default(),
                show_info_section: Default::default(),
                network_row: Default::default(),
                network_label: Default::default(),
                status_row: Default::default(),
                status_label: Default::default(),
                content_rating_row: Default::default(),
                content_rating_label: Default::default(),
                state: RefCell::new(None),
                viewmodel: RefCell::new(None),
                on_episode_selected: RefCell::new(None),
                load_generation: RefCell::new(0),
                property_subscriptions: RefCell::new(Vec::new()),
                binding_handles: RefCell::new(Vec::new()),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ShowDetailsPage {
        const NAME: &'static str = "ShowDetailsPage";
        type Type = super::ShowDetailsPage;
        type ParentType = gtk4::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ShowDetailsPage {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            // Connect season dropdown
            self.season_dropdown.connect_selected_notify(glib::clone!(
                #[weak]
                obj,
                move |dropdown| {
                    let selected = dropdown.selected();
                    glib::spawn_future_local(glib::clone!(
                        #[weak]
                        obj,
                        async move {
                            obj.on_season_changed(selected).await;
                        }
                    ));
                }
            ));

            // Connect mark watched button
            self.mark_watched_button.connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |_| {
                    obj.on_mark_watched_clicked();
                }
            ));
        }
    }

    impl WidgetImpl for ShowDetailsPage {}
    impl BoxImpl for ShowDetailsPage {}
}

glib::wrapper! {
    pub struct ShowDetailsPage(ObjectSubclass<imp::ShowDetailsPage>)
        @extends gtk4::Widget, gtk4::Box,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Orientable;
}

impl ShowDetailsPage {
    pub fn new(state: Arc<AppState>) -> Self {
        let page: Self = glib::Object::new();
        let data_service = state.data_service.clone();
        let viewmodel = Arc::new(DetailsViewModel::new(data_service));

        // Initialize ViewModel with EventBus
        glib::spawn_future_local({
            let vm = viewmodel.clone();
            let event_bus = state.event_bus.clone();
            async move {
                use crate::platforms::gtk::ui::viewmodels::ViewModel;
                vm.initialize(event_bus).await;
            }
        });

        page.imp().state.replace(Some(state));
        page.imp().viewmodel.replace(Some(viewmodel));

        // Set up property subscriptions
        page.setup_property_bindings();

        page
    }

    fn setup_property_bindings(&self) {
        let imp = self.imp();
        let page_weak = self.downgrade();

        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            // Subscribe to current_item property
            if let Some(mut subscriber) = viewmodel.subscribe_to_property("current_item") {
                let page_weak_clone = page_weak.clone();
                glib::spawn_future_local(async move {
                    while subscriber.wait_for_change().await {
                        if let Some(page) = page_weak_clone.upgrade() {
                            page.on_current_item_changed().await;
                        }
                    }
                });
            }

            // Subscribe to current_item property for genre chips
            if let Some(mut subscriber) = viewmodel.subscribe_to_property("current_item") {
                let page_weak_clone = page_weak.clone();
                glib::spawn_future_local(async move {
                    while subscriber.wait_for_change().await {
                        if let Some(page) = page_weak_clone.upgrade() {
                            page.on_genres_changed().await;
                        }
                    }
                });
            }

            // Subscribe to is_loading property
            if let Some(mut subscriber) = viewmodel.subscribe_to_property("is_loading") {
                let page_weak_clone = page_weak.clone();
                glib::spawn_future_local(async move {
                    while subscriber.wait_for_change().await {
                        if let Some(page) = page_weak_clone.upgrade() {
                            page.on_loading_changed().await;
                        }
                    }
                });
            }

            // Subscribe to episodes property for reactive episode list
            if let Some(mut subscriber) = viewmodel.subscribe_to_property("episodes") {
                let page_weak_clone = page_weak.clone();
                glib::spawn_future_local(async move {
                    while subscriber.wait_for_change().await {
                        if let Some(page) = page_weak_clone.upgrade() {
                            page.on_episodes_list_changed().await;
                        }
                    }
                });
            }

            // Set up reactive bindings for show details using current_item property
            self.setup_reactive_show_details_bindings();
        }
    }

    fn setup_reactive_show_details_bindings(&self) {
        let imp = self.imp();

        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            // Reactive binding for title
            bind_label_to_property(
                &imp.show_title,
                viewmodel.current_item().clone(),
                |detailed_info| {
                    if let Some(info) = detailed_info {
                        if let crate::models::MediaItem::Show(show) = &info.media {
                            return show.title.clone();
                        }
                    }
                    String::new()
                },
            );

            // Reactive binding for year
            bind_text_to_property(
                &imp.year_label,
                viewmodel.current_item().clone(),
                |detailed_info| {
                    if let Some(info) = detailed_info {
                        if let crate::models::MediaItem::Show(show) = &info.media {
                            if let Some(year) = show.year {
                                return format!("{}", year);
                            }
                        }
                    }
                    String::new()
                },
            );

            // Reactive binding for year visibility
            bind_visibility_to_property(
                &*imp.year_label,
                viewmodel.current_item().clone(),
                |detailed_info| {
                    if let Some(info) = detailed_info {
                        if let crate::models::MediaItem::Show(show) = &info.media {
                            return show.year.is_some();
                        }
                    }
                    false
                },
            );

            // Reactive binding for rating
            bind_text_to_property(
                &imp.rating_label,
                viewmodel.current_item().clone(),
                |detailed_info| {
                    if let Some(info) = detailed_info {
                        if let crate::models::MediaItem::Show(show) = &info.media {
                            if let Some(rating) = show.rating {
                                return format!("{:.1}", rating);
                            }
                        }
                    }
                    String::new()
                },
            );

            // Reactive binding for rating visibility
            bind_visibility_to_property(
                &*imp.rating_box,
                viewmodel.current_item().clone(),
                |detailed_info| {
                    if let Some(info) = detailed_info {
                        if let crate::models::MediaItem::Show(show) = &info.media {
                            return show.rating.is_some();
                        }
                    }
                    false
                },
            );

            // Reactive binding for synopsis
            bind_text_to_property(
                &imp.synopsis_label,
                viewmodel.current_item().clone(),
                |detailed_info| {
                    if let Some(info) = detailed_info {
                        if let crate::models::MediaItem::Show(show) = &info.media {
                            if let Some(overview) = &show.overview {
                                return overview.clone();
                            }
                        }
                    }
                    String::new()
                },
            );

            // Reactive binding for synopsis visibility
            bind_visibility_to_property(
                &*imp.synopsis_label,
                viewmodel.current_item().clone(),
                |detailed_info| {
                    if let Some(info) = detailed_info {
                        if let crate::models::MediaItem::Show(show) = &info.media {
                            return show.overview.is_some()
                                && !show.overview.as_ref().unwrap().trim().is_empty();
                        }
                    }
                    false
                },
            );

            // Reactive binding for poster image
            bind_image_to_property(
                &imp.show_poster,
                viewmodel.current_item().clone(),
                |detailed_info| {
                    if let Some(info) = detailed_info {
                        if let crate::models::MediaItem::Show(show) = &info.media {
                            return show.poster_url.clone();
                        }
                    }
                    None
                },
            );

            // Reactive binding for backdrop image
            bind_image_to_property(
                &imp.show_backdrop,
                viewmodel.current_item().clone(),
                |detailed_info| {
                    if let Some(info) = detailed_info {
                        if let crate::models::MediaItem::Show(show) = &info.media {
                            return show.backdrop_url.clone();
                        }
                    }
                    None
                },
            );

            // Reactive binding for poster placeholder visibility based on loading state
            bind_visibility_to_property(
                &*imp.poster_placeholder,
                viewmodel.is_loading().clone(),
                |is_loading| *is_loading, // Show placeholder when loading
            );

            // Reactive binding for episodes collection
            // For now, we'll skip the reactive episodes binding and keep using the manual approach
            // until we can resolve the thread safety issue with creating episode cards
            // TODO: Implement reactive episodes binding once we can make episode card creation thread-safe

            // Reactive binding for episode count
            let episodes_count_handle = bind_text_to_property(
                &*imp.episodes_count_label,
                viewmodel.episodes().clone(),
                |episodes| {
                    let episode_count = episodes
                        .iter()
                        .filter(|item| matches!(item, crate::models::MediaItem::Episode(_)))
                        .count();
                    format!("{} episodes", episode_count)
                },
            );
            imp.binding_handles.borrow_mut().push(episodes_count_handle);

            // Reactive binding for episodes box visibility
            let episodes_visibility_handle = bind_visibility_to_property(
                &*imp.episodes_box,
                viewmodel.episodes().clone(),
                |episodes| !episodes.is_empty(),
            );
            imp.binding_handles
                .borrow_mut()
                .push(episodes_visibility_handle);

            // Reactive binding for season dropdown
            let season_dropdown_handle = bind_dropdown_to_property(
                &*imp.season_dropdown,
                viewmodel.seasons().clone(),
                |season| format!("Season {}", season),
            );
            imp.binding_handles
                .borrow_mut()
                .push(season_dropdown_handle);

            // Reactive binding for seasons count and visibility
            let seasons_count_handle = bind_text_to_property(
                &*imp.seasons_label,
                viewmodel.seasons().clone(),
                |seasons| format!("{} Seasons", seasons.len()),
            );
            imp.binding_handles.borrow_mut().push(seasons_count_handle);

            let seasons_visibility_handle = bind_visibility_to_property(
                &*imp.seasons_box,
                viewmodel.seasons().clone(),
                |seasons| !seasons.is_empty(),
            );
            imp.binding_handles
                .borrow_mut()
                .push(seasons_visibility_handle);

            // Reactive binding for watched icon
            let watched_icon_handle = bind_image_icon_to_property(
                &*imp.watched_icon,
                viewmodel.is_watched().clone(),
                |is_watched| {
                    if *is_watched {
                        "checkbox-checked-symbolic".to_string()
                    } else {
                        "object-select-symbolic".to_string()
                    }
                },
            );
            imp.binding_handles.borrow_mut().push(watched_icon_handle);

            // Reactive binding for watched label
            let watched_label_handle = bind_text_to_property(
                &*imp.watched_label,
                viewmodel.is_watched().clone(),
                |is_watched| {
                    if *is_watched {
                        "Season Watched".to_string()
                    } else {
                        "Mark Season as Watched".to_string()
                    }
                },
            );
            imp.binding_handles.borrow_mut().push(watched_label_handle);

            // Reactive binding for CSS class on watched button
            let watched_css_handle = bind_css_class_to_property(
                &*imp.mark_watched_button,
                viewmodel.is_watched().clone(),
                "suggested-action",
                |is_watched| !*is_watched, // Add suggested-action when NOT watched
            );
            imp.binding_handles.borrow_mut().push(watched_css_handle);

            // Reactive bindings for show info fields
            let network_handle = bind_text_to_computed_property(
                &*imp.network_label,
                viewmodel.show_network(),
                |network| network.clone().unwrap_or_else(|| "Unknown".to_string()),
            );
            imp.binding_handles.borrow_mut().push(network_handle);

            let network_visibility_handle = bind_visibility_to_computed_property(
                &*imp.network_row,
                viewmodel.show_network(),
                |network| network.is_some(),
            );
            imp.binding_handles
                .borrow_mut()
                .push(network_visibility_handle);

            let status_handle = bind_text_to_computed_property(
                &*imp.status_label,
                viewmodel.show_status(),
                |status| status.clone().unwrap_or_else(|| "Unknown".to_string()),
            );
            imp.binding_handles.borrow_mut().push(status_handle);

            let status_visibility_handle = bind_visibility_to_computed_property(
                &*imp.status_row,
                viewmodel.show_status(),
                |status| status.is_some(),
            );
            imp.binding_handles
                .borrow_mut()
                .push(status_visibility_handle);

            let content_rating_handle = bind_text_to_computed_property(
                &*imp.content_rating_label,
                viewmodel.show_content_rating(),
                |rating| rating.clone().unwrap_or_else(|| "Not Rated".to_string()),
            );
            imp.binding_handles.borrow_mut().push(content_rating_handle);

            let content_rating_visibility_handle = bind_visibility_to_computed_property(
                &*imp.content_rating_row,
                viewmodel.show_content_rating(),
                |rating| rating.is_some(),
            );
            imp.binding_handles
                .borrow_mut()
                .push(content_rating_visibility_handle);

            // Show info section visibility (show if any field has data)
            let show_info_visibility_handle = bind_visibility_to_computed_property(
                &*imp.show_info_section,
                viewmodel.show_content_rating(), // Use content rating as a proxy since it's the only field with data currently
                |rating| rating.is_some(),
            );
            imp.binding_handles
                .borrow_mut()
                .push(show_info_visibility_handle);
        }
    }

    pub fn load_show(&self, show: Show) {
        info!("Loading show details: {}", show.title);

        let imp = self.imp();

        // Use ViewModel to load the media details
        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            let viewmodel = viewmodel.clone();
            let show_id = show.id.clone();

            glib::spawn_future_local(async move {
                if let Err(e) = viewmodel.load_media(show_id).await {
                    error!("Failed to load show through ViewModel: {}", e);
                }
            });
        }
    }

    async fn on_current_item_changed(&self) {
        let imp = self.imp();

        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            if let Some(detailed_info) = viewmodel.current_item().get().await {
                self.display_media_info(&detailed_info).await;
            } else {
                // Clear UI when no item is loaded
                imp.show_poster.set_paintable(gtk4::gdk::Paintable::NONE);
                imp.show_backdrop.set_paintable(gtk4::gdk::Paintable::NONE);
                imp.poster_placeholder.set_visible(true);
                self.clear_episodes();
            }
        }
    }

    async fn on_loading_changed(&self) {
        // Loading states are now handled by reactive bindings
        // Images and placeholder visibility are managed automatically

        let imp = self.imp();
        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            let is_loading = viewmodel.is_loading().get().await;
            if is_loading {
                // Clear episodes when starting to load
                self.clear_episodes();
            }
        }
    }

    async fn on_episodes_changed(&self) {
        // Episodes have been updated, refresh the display
        // This is called when episodes are loaded from the database
        // The actual display update happens in load_episodes which reads from the ViewModel
        debug!("Episodes changed in ViewModel");
    }

    async fn on_episodes_list_changed(&self) {
        let imp = self.imp();

        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            let episodes_models = viewmodel.episodes().get().await;
            self.bind_episodes_to_box(&episodes_models).await;
        }
    }

    async fn bind_episodes_to_box(&self, episodes: &[crate::models::MediaItem]) {
        let imp = self.imp();

        // Clear existing episodes
        self.clear_episodes();

        // Update episode count
        imp.episodes_count_label
            .set_text(&format!("{} episodes", episodes.len()));

        // Convert and add episode cards
        for episode_item in episodes {
            if let crate::models::MediaItem::Episode(episode) = episode_item {
                self.add_episode_card(episode.clone(), false);
            }
        }
    }

    async fn display_media_info(
        &self,
        detailed_info: &crate::platforms::gtk::ui::viewmodels::details_view_model::DetailedMediaInfo,
    ) {
        let media = &detailed_info.media;
        let metadata = &detailed_info.metadata;
        let imp = self.imp();

        // Extract show from MediaItem enum
        let show = match media {
            crate::models::MediaItem::Show(show) => show,
            _ => {
                error!("ShowDetailsPage received non-show MediaItem");
                return;
            }
        };

        // Image loading is now handled by reactive bindings
        // Add CSS classes for styling
        imp.show_backdrop.add_css_class("show-backdrop");
        imp.show_poster.add_css_class("show-poster");
        imp.poster_placeholder
            .add_css_class("show-poster-placeholder");

        // Title, year, rating, synopsis are now handled by reactive bindings
        // Set seasons count - for shows, we'll need to extract this from metadata or use a default
        // Since we don't have season info in the database entity, we'll hide this for now
        // TODO: Extract season information from metadata or add to database schema
        imp.seasons_box.set_visible(false);

        // Genre chips are now handled reactively by on_genres_changed()

        // Load episodes for the first season
        // For now, try to load season 1 episodes
        // TODO: Properly handle season selection from show metadata
        let show_id = show.id.clone();
        let page_weak = self.downgrade();
        glib::spawn_future_local(async move {
            if let Some(page) = page_weak.upgrade() {
                page.load_episodes_for_show(&show_id, 1).await;
            }
        });
    }

    async fn load_episodes(&self, season_number: u32) {
        info!("Loading episodes for season {}", season_number);

        let imp = self.imp();

        // Use ViewModel to load episodes
        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            let viewmodel = viewmodel.clone();

            match viewmodel.select_season(season_number as i32).await {
                Ok(_) => {
                    // Episodes will be automatically updated via reactive binding to episodes property
                    debug!(
                        "Episodes loaded for season {}, reactive binding will update UI",
                        season_number
                    );
                }
                Err(e) => {
                    error!("Failed to load episodes: {}", e);
                    // Clear episodes and show error
                    self.clear_episodes();
                    let error_label = gtk4::Label::builder()
                        .label(format!("Failed to load episodes: {}", e))
                        .css_classes(vec!["error"])
                        .build();
                    imp.episodes_box.append(&error_label);
                }
            }
        }
    }

    fn create_episode_card_widget(&self, episode: Episode) -> gtk4::Button {
        self.create_episode_card_internal(episode, false)
    }

    fn create_episode_card_internal(
        &self,
        episode: Episode,
        should_highlight: bool,
    ) -> gtk4::Button {
        // Create episode card with enhanced styling (no default card background)
        let mut card_classes = vec!["episode-card", "flat"];
        if should_highlight {
            card_classes.push("next-unwatched");
        }
        let card = gtk4::Button::builder()
            .css_classes(card_classes)
            .width_request(320)
            .build();

        let card_content = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(0)
            .build();

        // Episode thumbnail with overlay
        let overlay = gtk4::Overlay::new();
        overlay.add_css_class("episode-thumbnail-overlay");

        let thumbnail_frame = gtk4::Frame::builder()
            .height_request(180) // 16:9 aspect ratio for 320px width
            .css_classes(vec!["episode-thumbnail"])
            .build();

        let thumbnail = gtk4::Picture::builder()
            .content_fit(gtk4::ContentFit::Cover)
            .css_classes(vec!["episode-picture"])
            .build();

        // Load episode thumbnail if available
        if let Some(thumb_url) = &episode.thumbnail_url {
            let picture = thumbnail.clone();
            let url = thumb_url.clone();

            glib::spawn_future_local(async move {
                match IMAGE_LOADER.load_image(&url, ImageSize::Medium).await {
                    Ok(texture) => {
                        picture.set_paintable(Some(&texture));
                    }
                    Err(e) => {
                        debug!("Failed to load episode thumbnail: {}", e);
                    }
                }
            });
        }

        thumbnail_frame.set_child(Some(&thumbnail));
        overlay.set_child(Some(&thumbnail_frame));

        // Episode number badge
        let episode_badge = gtk4::Label::builder()
            .label(format!("E{:02}", episode.episode_number))
            .css_classes(vec!["episode-badge", "osd"])
            .halign(gtk4::Align::Start)
            .valign(gtk4::Align::Start)
            .margin_top(8)
            .margin_start(8)
            .build();
        overlay.add_overlay(&episode_badge);

        // Watched indicator - checkmark with background
        if episode.view_count > 0 {
            let watched_container = gtk4::Box::builder()
                .css_classes(vec!["episode-watched-container"])
                .halign(gtk4::Align::End)
                .valign(gtk4::Align::Start)
                .margin_top(10)
                .margin_end(10)
                .build();

            let watched_icon = gtk4::Image::builder()
                .icon_name("object-select-symbolic")
                .css_classes(vec!["episode-watched-icon"])
                .pixel_size(16)
                .build();

            watched_container.append(&watched_icon);
            overlay.add_overlay(&watched_container);
        }

        // Progress bar if partially watched
        if let Some(position) = episode.playback_position
            && position.as_secs() > 0
            && position < episode.duration
        {
            let progress = position.as_secs_f64() / episode.duration.as_secs_f64();
            let progress_bar = gtk4::ProgressBar::builder()
                .fraction(progress)
                .css_classes(vec!["episode-progress", "osd"])
                .valign(gtk4::Align::End)
                .margin_bottom(0)
                .build();
            overlay.add_overlay(&progress_bar);
        }

        // Play overlay on hover (CSS handles visibility)
        let play_overlay = gtk4::Box::builder()
            .css_classes(vec!["episode-play-overlay"])
            .valign(gtk4::Align::Center)
            .halign(gtk4::Align::Center)
            .build();

        let play_icon = gtk4::Image::builder()
            .icon_name("media-playback-start-symbolic")
            .pixel_size(48)
            .css_classes(vec!["osd", "play-icon"])
            .build();
        play_overlay.append(&play_icon);
        overlay.add_overlay(&play_overlay);

        card_content.append(&overlay);

        // Episode info
        let info_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(4)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(16)
            .margin_end(16)
            .build();

        // Episode title (fallback to SxE if missing)
        let title_text = if episode.title.trim().is_empty() {
            format!("S{}E{}", episode.season_number, episode.episode_number)
        } else {
            episode.title.clone()
        };
        let title_label = gtk4::Label::builder()
            .label(&title_text)
            .css_classes(vec!["heading"])
            .xalign(0.0)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .single_line_mode(true)
            .build();
        info_box.append(&title_label);

        // Episode description (overview), if available
        if let Some(overview) = &episode.overview
            && !overview.trim().is_empty()
        {
            let desc_label = gtk4::Label::builder()
                .label(overview)
                .wrap(true)
                .xalign(0.0)
                .css_classes(vec!["dim-label"])
                .build();
            info_box.append(&desc_label);
        }

        // Episode duration and air date
        let metadata_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .build();

        let duration_mins = episode.duration.as_secs() / 60;
        let duration_label = gtk4::Label::builder()
            .label(format!("{} min", duration_mins))
            .css_classes(vec!["dim-label", "caption"])
            .xalign(0.0)
            .build();
        metadata_box.append(&duration_label);

        info_box.append(&metadata_box);

        card_content.append(&info_box);
        card.set_child(Some(&card_content));

        // Connect click handler
        let self_weak = glib::clone::Downgrade::downgrade(self);
        let episode_clone = episode.clone();
        card.connect_clicked(move |_| {
            if let Some(page) = self_weak.upgrade() {
                let episode = episode_clone.clone();
                glib::spawn_future_local(async move {
                    if let Some(callback) = page.imp().on_episode_selected.borrow().as_ref() {
                        callback(&episode);
                    }
                });
            }
        });

        card
    }

    fn add_episode_card(&self, episode: Episode, should_highlight: bool) {
        let card = self.create_episode_card_internal(episode, should_highlight);
        self.imp().episodes_box.append(&card);
    }

    fn clear_episodes(&self) {
        let imp = self.imp();
        while let Some(child) = imp.episodes_box.first_child() {
            imp.episodes_box.remove(&child);
        }
        imp.episodes_count_label.set_text("");
    }

    // Removed - now handled by on_watched_changed via property binding

    async fn on_season_changed(&self, index: u32) {
        let imp = self.imp();
        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            let seasons = viewmodel.seasons().get().await;
            tracing::info!(
                "ShowDetailsPage: on_season_changed index={} seasons={:?}",
                index,
                seasons
            );
            if let Some(season_num) = seasons.get(index as usize) {
                tracing::info!(
                    "ShowDetailsPage: loading episodes for season {} (from ViewModel)",
                    season_num
                );
                self.load_episodes(*season_num as u32).await;
            } else {
                // Try to get season info from current media item metadata as fallback
                if let Some(detailed_info) = viewmodel.current_item().get().await {
                    if let crate::models::MediaItem::Show(show) = &detailed_info.media {
                        if let Some(season) = show.seasons.get(index as usize) {
                            tracing::info!(
                                "ShowDetailsPage: loading episodes for season {} (from metadata fallback)",
                                season.season_number
                            );
                            self.load_episodes(season.season_number).await;
                            return;
                        }
                    }
                }

                tracing::warn!(
                    "ShowDetailsPage: on_season_changed: no season found for index {} (vm len: {})",
                    index,
                    seasons.len()
                );
            }
        }
    }

    async fn load_episodes_for_show(&self, _show_id: &str, season_number: u32) {
        // Use ViewModel to load episodes
        if let Some(viewmodel) = self.imp().viewmodel.borrow().as_ref() {
            let viewmodel = viewmodel.clone();

            match viewmodel.select_season(season_number as i32).await {
                Ok(_) => {
                    // Episodes will be automatically updated via reactive binding to episodes property
                    debug!("Episodes loaded for show, reactive binding will update UI");
                }
                Err(e) => {
                    error!("Failed to load episodes: {}", e);
                    // Clear episodes and show error
                    self.clear_episodes();
                    let error_label = gtk4::Label::builder()
                        .label(format!("Failed to load episodes: {}", e))
                        .css_classes(vec!["error"])
                        .build();
                    self.imp().episodes_box.append(&error_label);
                }
            }
        }
    }

    fn on_mark_watched_clicked(&self) {
        let imp = self.imp();

        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            let viewmodel = viewmodel.clone();

            glib::spawn_future_local(async move {
                let is_watched = viewmodel.is_watched().get().await;

                // If there's a current season (showing episodes), mark the season
                // Otherwise mark the show itself
                if viewmodel.current_season().get().await.is_some() {
                    if is_watched {
                        viewmodel.mark_season_as_unwatched().await;
                    } else {
                        viewmodel.mark_season_as_watched().await;
                    }
                } else if is_watched {
                    viewmodel.mark_as_unwatched().await;
                } else {
                    viewmodel.mark_as_watched().await;
                }
            });
        }
    }

    async fn on_genres_changed(&self) {
        let imp = self.imp();

        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            if let Some(detailed_info) = viewmodel.current_item().get().await {
                self.bind_genres_to_flowbox(&detailed_info.metadata.genres)
                    .await;
            } else {
                // Clear genres when no item is loaded
                self.clear_genres();
            }
        }
    }

    async fn bind_genres_to_flowbox(&self, genres: &[String]) {
        let imp = self.imp();

        // Clear existing genre chips
        self.clear_genres();

        // Create new genre chips
        for genre in genres {
            let genre_chip = adw::Bin::builder()
                .css_classes(vec!["card", "compact", "genre-chip"])
                .build();

            let genre_label = gtk4::Label::builder()
                .label(genre)
                .css_classes(vec!["caption", "genre-label"])
                .margin_top(6)
                .margin_bottom(6)
                .margin_start(12)
                .margin_end(12)
                .build();

            genre_chip.set_child(Some(&genre_label));
            imp.genres_box.insert(&genre_chip, -1);
        }

        // Set visibility based on whether genres exist
        imp.genres_box.set_visible(!genres.is_empty());
    }

    fn clear_genres(&self) {
        let imp = self.imp();
        while let Some(child) = imp.genres_box.first_child() {
            imp.genres_box.remove(&child);
        }
        imp.genres_box.set_visible(false);
    }

    pub fn set_on_episode_selected<F>(&self, callback: F)
    where
        F: Fn(&Episode) + 'static,
    {
        self.imp()
            .on_episode_selected
            .replace(Some(Box::new(callback)));
    }

    async fn convert_media_item_to_episode(
        &self,
        media_item: &crate::db::entities::MediaItemModel,
    ) -> Option<Episode> {
        use std::time::Duration;

        // Only convert if this is actually an episode
        if media_item.media_type != "episode" {
            return None;
        }

        let imp = self.imp();

        // Get playback information
        let (watched, view_count, last_watched_at, playback_position) =
            if let Some(state) = imp.state.borrow().as_ref() {
                match state
                    .data_service
                    .get_playback_progress(&media_item.id)
                    .await
                {
                    Ok(Some((position_ms, duration_ms))) => {
                        let watched = position_ms as f64 / duration_ms as f64 > 0.9;
                        let position = Duration::from_millis(position_ms);
                        (watched, if watched { 1 } else { 0 }, None, Some(position))
                    }
                    _ => (false, 0, None, None),
                }
            } else {
                (false, 0, None, None)
            };

        // Extract episode-specific metadata
        let (air_date, show_title, show_poster_url) = if let Some(metadata) = &media_item.metadata {
            let metadata_json: serde_json::Value = metadata.clone();
            let air_date = metadata_json
                .get("air_date")
                .and_then(|v| v.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc));
            let show_title = metadata_json
                .get("show_title")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let show_poster_url = metadata_json
                .get("show_poster_url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            (air_date, show_title, show_poster_url)
        } else {
            (None, None, None)
        };

        Some(Episode {
            id: media_item.id.clone(),
            backend_id: media_item.source_id.clone(),
            show_id: media_item.parent_id.clone(),
            title: media_item.title.clone(),
            season_number: media_item.season_number.unwrap_or(0) as u32,
            episode_number: media_item.episode_number.unwrap_or(0) as u32,
            duration: Duration::from_millis(media_item.duration_ms.unwrap_or(0) as u64),
            thumbnail_url: media_item.poster_url.clone(),
            overview: media_item.overview.clone(),
            air_date,
            watched,
            view_count,
            last_watched_at,
            playback_position,
            show_title,
            show_poster_url,
            intro_marker: None,
            credits_marker: None,
        })
    }

    pub fn widget(&self) -> &gtk4::Box {
        self.upcast_ref()
    }
}
