use adw::prelude::*;
use gtk4::{self, glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use std::cell::RefCell;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::backends::traits::MediaBackend;
use crate::models::{Episode, Show};
use crate::services::DataService;
use crate::state::AppState;
use crate::ui::viewmodels::{DetailsViewModel, ViewModel};
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
        pub current_show: RefCell<Option<Show>>,
        pub current_season: RefCell<Option<u32>>,
        pub on_episode_selected: RefCell<Option<Box<dyn Fn(&Episode)>>>,
        pub load_generation: RefCell<u64>,
        pub property_subscriptions: RefCell<Vec<tokio::sync::broadcast::Receiver<()>>>,
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
                current_show: RefCell::new(None),
                current_season: RefCell::new(None),
                on_episode_selected: RefCell::new(None),
                load_generation: RefCell::new(0),
                property_subscriptions: RefCell::new(Vec::new()),
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
                use crate::ui::viewmodels::ViewModel;
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

            // Subscribe to is_watched property
            if let Some(mut subscriber) = viewmodel.subscribe_to_property("is_watched") {
                let page_weak_clone = page_weak.clone();
                glib::spawn_future_local(async move {
                    while subscriber.wait_for_change().await {
                        if let Some(page) = page_weak_clone.upgrade() {
                            page.on_watched_changed().await;
                        }
                    }
                });
            }

            // Subscribe to episodes property
            if let Some(mut subscriber) = viewmodel.subscribe_to_property("episodes") {
                let page_weak_clone = page_weak.clone();
                glib::spawn_future_local(async move {
                    while subscriber.wait_for_change().await {
                        if let Some(page) = page_weak_clone.upgrade() {
                            page.on_episodes_changed().await;
                        }
                    }
                });
            }

            // Subscribe to seasons property
            if let Some(mut subscriber) = viewmodel.subscribe_to_property("seasons") {
                let page_weak_clone = page_weak.clone();
                glib::spawn_future_local(async move {
                    while subscriber.wait_for_change().await {
                        if let Some(page) = page_weak_clone.upgrade() {
                            page.on_seasons_changed().await;
                        }
                    }
                });
            }
        }
    }

    pub fn load_show(&self, show: Show) {
        info!("Loading show details: {}", show.title);

        let imp = self.imp();

        // Store current show for compatibility
        imp.current_show.replace(Some(show.clone()));

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
        let imp = self.imp();

        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            let is_loading = viewmodel.is_loading().get().await;

            if is_loading {
                // Show loading state
                imp.show_poster.set_paintable(gtk4::gdk::Paintable::NONE);
                imp.show_backdrop.set_paintable(gtk4::gdk::Paintable::NONE);
                imp.poster_placeholder.set_visible(true);
                self.clear_episodes();
            }
        }
    }

    async fn on_watched_changed(&self) {
        let imp = self.imp();

        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            let is_watched = viewmodel.is_watched().get().await;

            if is_watched {
                imp.watched_icon
                    .set_icon_name(Some("checkbox-checked-symbolic"));
                imp.watched_label.set_text("Season Watched");
                imp.mark_watched_button.remove_css_class("suggested-action");
            } else {
                imp.watched_icon
                    .set_icon_name(Some("object-select-symbolic"));
                imp.watched_label.set_text("Mark Season as Watched");
                imp.mark_watched_button.add_css_class("suggested-action");
            }
        }
    }

    async fn on_episodes_changed(&self) {
        // Episodes have been updated, refresh the display
        // This is called when episodes are loaded from the database
        // The actual display update happens in load_episodes which reads from the ViewModel
        debug!("Episodes changed in ViewModel");
    }

    async fn on_seasons_changed(&self) {
        let imp = self.imp();

        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            let seasons = viewmodel.seasons().get().await;
            tracing::info!("ShowDetailsPage: seasons updated: {:?}", seasons);

            if !seasons.is_empty() {
                // Update season dropdown with available seasons
                let season_strings: Vec<String> =
                    seasons.iter().map(|s| format!("Season {}", s)).collect();

                // Build the StringList incrementally to avoid any transient lifetime issues
                let string_list = gtk4::StringList::new(&[]);
                for s in &season_strings {
                    string_list.append(s);
                }
                imp.season_dropdown.set_model(Some(&string_list));
                // Expression is optional for StringList; DropDown can display strings directly
                // Clear any previous expression to use default string display
                imp.season_dropdown.set_expression(gtk4::Expression::NONE);
                // Ensure it has a reasonable width so text is visible
                if imp.season_dropdown.width_request() <= 0 {
                    imp.season_dropdown.set_width_request(140);
                }

                // Show the seasons box
                imp.seasons_box.set_visible(true);
                imp.seasons_label
                    .set_text(&format!("{} Seasons", seasons.len()));

                // Select the first season by default
                imp.season_dropdown.set_selected(0);
                tracing::info!(
                    "ShowDetailsPage: season dropdown model set ({} items), selected index 0",
                    season_strings.len()
                );

                // Log current selection and model size just after updates
                let selected_now = imp.season_dropdown.selected();
                let model_items = imp
                    .season_dropdown
                    .model()
                    .map(|m| m.n_items())
                    .unwrap_or(0);
                tracing::info!(
                    "ShowDetailsPage: dropdown immediate state -> selected={} items={}",
                    selected_now,
                    model_items
                );

                // Some GTK versions briefly reset selection after setting model/expression.
                // Apply a short delayed re-select to ensure we don't end up with (None).
                let dropdown = imp.season_dropdown.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(20), move || {
                    if dropdown.selected() == u32::MAX {
                        dropdown.set_selected(0);
                        tracing::info!("ShowDetailsPage: dropdown re-selected index 0 after delay");
                    } else {
                        tracing::info!(
                            "ShowDetailsPage: dropdown selection persisted at {}",
                            dropdown.selected()
                        );
                    }
                });
            } else {
                // Hide season selector if no seasons
                imp.seasons_box.set_visible(false);
                tracing::info!("ShowDetailsPage: seasons empty; hiding selector");
            }
        }
    }

    async fn display_media_info(
        &self,
        detailed_info: &crate::ui::viewmodels::details_view_model::DetailedMediaInfo,
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

        // Load backdrop image with enhanced styling
        if let Some(backdrop_url) = &show.backdrop_url {
            let backdrop_picture = imp.show_backdrop.clone();
            backdrop_picture.add_css_class("show-backdrop");
            let url = backdrop_url.clone();

            glib::spawn_future_local(async move {
                match IMAGE_LOADER.load_image(&url, ImageSize::Large).await {
                    Ok(texture) => {
                        backdrop_picture.set_paintable(Some(&texture));
                    }
                    Err(e) => {
                        error!("Failed to load show backdrop: {}", e);
                    }
                }
            });
        }

        // Load poster image with enhanced 3D effect
        if let Some(poster_url) = &show.poster_url {
            let picture = imp.show_poster.clone();
            picture.add_css_class("show-poster");
            let placeholder = imp.poster_placeholder.clone();
            placeholder.add_css_class("show-poster-placeholder");
            let url = poster_url.clone();

            glib::spawn_future_local(async move {
                match IMAGE_LOADER.load_image(&url, ImageSize::Large).await {
                    Ok(texture) => {
                        picture.set_paintable(Some(&texture));
                        placeholder.set_visible(false);
                    }
                    Err(e) => {
                        error!("Failed to load show poster: {}", e);
                        // Keep placeholder visible on error
                    }
                }
            });
        }

        // Set title
        imp.show_title.set_label(&show.title);

        // Set year
        if let Some(year) = show.year {
            imp.year_label.set_text(&format!("{}", year));
            imp.year_label.set_visible(true);
        } else {
            imp.year_label.set_visible(false);
        }

        // Set rating
        if let Some(rating) = show.rating {
            imp.rating_label.set_text(&format!("{:.1}", rating));
            imp.rating_box.set_visible(true);
        } else {
            imp.rating_box.set_visible(false);
        }

        // Set seasons count - for shows, we'll need to extract this from metadata or use a default
        // Since we don't have season info in the database entity, we'll hide this for now
        // TODO: Extract season information from metadata or add to database schema
        imp.seasons_box.set_visible(false);

        // Set synopsis
        if let Some(overview) = &show.overview {
            imp.synopsis_label.set_text(overview);
            imp.synopsis_label.set_visible(true);
        } else {
            imp.synopsis_label.set_visible(false);
        }

        // Clear and populate genres
        while let Some(child) = imp.genres_box.first_child() {
            imp.genres_box.remove(&child);
        }

        for genre in &metadata.genres {
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

        imp.genres_box.set_visible(!metadata.genres.is_empty());

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

        // Clear existing episodes
        self.clear_episodes();

        // Store current season for compatibility
        imp.current_season.replace(Some(season_number));

        // Use ViewModel to load episodes
        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            let viewmodel = viewmodel.clone();

            match viewmodel.select_season(season_number as i32).await {
                Ok(_) => {
                    // Get episodes from ViewModel
                    let episodes_models = viewmodel.episodes().get().await;

                    // Update episode count
                    imp.episodes_count_label
                        .set_text(&format!("{} episodes", episodes_models.len()));

                    // Convert MediaItem enums to Episode structs for display
                    for episode_item in episodes_models {
                        if let crate::models::MediaItem::Episode(episode) = episode_item {
                            self.add_episode_card(episode, false);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to load episodes: {}", e);
                    // Show error message
                    let error_label = gtk4::Label::builder()
                        .label(format!("Failed to load episodes: {}", e))
                        .css_classes(vec!["error"])
                        .build();
                    imp.episodes_box.append(&error_label);
                }
            }
        }
    }

    async fn find_next_unwatched_season(&self, show: &Show) -> Option<(usize, u32)> {
        let imp = self.imp();

        if let Some(state) = imp.state.borrow().as_ref() {
            let backend_id = &show.backend_id;
            if let Some(backend) = state.source_coordinator.get_backend(backend_id).await {
                // Check each season for unwatched episodes
                for (index, season) in show.seasons.iter().enumerate() {
                    match backend.get_episodes(&show.id, season.season_number).await {
                        Ok(episodes) => {
                            // Check if this season has any unwatched episodes
                            if episodes.iter().any(|ep| ep.view_count == 0) {
                                info!(
                                    "Found unwatched episodes in season {}",
                                    season.season_number
                                );
                                return Some((index, season.season_number));
                            }
                        }
                        Err(e) => {
                            error!(
                                "Failed to get episodes for season {}: {}",
                                season.season_number, e
                            );
                        }
                    }
                }
            }
        }

        None
    }

    async fn load_episodes_with_highlight(&self, season_number: u32) {
        info!(
            "Loading episodes for season {} with highlight",
            season_number
        );

        let imp = self.imp();

        // Clear existing episodes
        self.clear_episodes();

        // Store current season
        imp.current_season.replace(Some(season_number));

        // Get the show
        if let Some(show) = imp.current_show.borrow().as_ref() {
            // Get backend and fetch episodes
            if let Some(state) = imp.state.borrow().as_ref() {
                let backend_id = &show.backend_id;
                if let Some(backend) = state.source_coordinator.get_backend(backend_id).await {
                    match backend.get_episodes(&show.id, season_number).await {
                        Ok(episodes) => {
                            // Update episode count
                            imp.episodes_count_label
                                .set_text(&format!("{} episodes", episodes.len()));

                            // Find the first unwatched episode
                            let first_unwatched_index =
                                episodes.iter().position(|ep| ep.view_count == 0);

                            // Add episode cards with highlight flag
                            for (index, episode) in episodes.into_iter().enumerate() {
                                let should_highlight = first_unwatched_index == Some(index);
                                self.add_episode_card(episode, should_highlight);
                            }

                            // Scroll to the highlighted episode after a brief delay to ensure layout
                            if first_unwatched_index.is_some() {
                                let episodes_carousel = imp.episodes_carousel.clone();
                                glib::timeout_add_local_once(
                                    std::time::Duration::from_millis(100),
                                    move || {
                                        // Scroll to show the highlighted episode
                                        let adjustment = episodes_carousel.hadjustment();
                                        // Calculate approximate position (320px card width + spacing)
                                        let card_width = 330.0; // 320px + spacing
                                        let target_position =
                                            first_unwatched_index.unwrap() as f64 * card_width;

                                        // Center the card if possible
                                        let viewport_width = adjustment.page_size();
                                        let centered_position = (target_position
                                            - viewport_width / 2.0
                                            + card_width / 2.0)
                                            .max(0.0);

                                        adjustment.set_value(centered_position);
                                    },
                                );
                            }
                        }
                        Err(e) => {
                            error!("Failed to load episodes: {}", e);
                            // Show error message
                            let error_label = gtk4::Label::builder()
                                .label(format!("Failed to load episodes: {}", e))
                                .css_classes(vec!["error"])
                                .build();
                            imp.episodes_box.append(&error_label);
                        }
                    }
                }
            }
        }
    }

    fn add_episode_card(&self, episode: Episode, should_highlight: bool) {
        let imp = self.imp();

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
        if let Some(overview) = &episode.overview {
            if !overview.trim().is_empty() {
                let desc_label = gtk4::Label::builder()
                    .label(overview)
                    .wrap(true)
                    .xalign(0.0)
                    .css_classes(vec!["dim-label"])
                    .build();
                info_box.append(&desc_label);
            }
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

        imp.episodes_box.append(&card);
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
            } else if let Some(show) = imp.current_show.borrow().as_ref()
                && let Some(season) = show.seasons.get(index as usize)
            {
                tracing::info!(
                    "ShowDetailsPage: loading episodes for season {} (from metadata fallback)",
                    season.season_number
                );
                // Fallback to metadata seasons if DB-derived seasons are unavailable
                self.load_episodes(season.season_number).await;
            } else {
                tracing::warn!(
                    "ShowDetailsPage: on_season_changed: no season found for index {} (vm len: {}, metadata len: {})",
                    index,
                    seasons.len(),
                    imp.current_show
                        .borrow()
                        .as_ref()
                        .map(|s| s.seasons.len())
                        .unwrap_or(0)
                );
            }
        }
    }

    async fn load_episodes_for_show(&self, show_id: &str, season_number: u32) {
        let imp = self.imp();

        // Clear existing episodes
        self.clear_episodes();

        // Store current season for compatibility
        imp.current_season.replace(Some(season_number));

        // Use ViewModel to load episodes
        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            let viewmodel = viewmodel.clone();

            match viewmodel.select_season(season_number as i32).await {
                Ok(_) => {
                    // Get episodes from ViewModel
                    let episodes_models = viewmodel.episodes().get().await;

                    // Update episode count
                    imp.episodes_count_label
                        .set_text(&format!("{} episodes", episodes_models.len()));

                    // Convert and add episode cards
                    for episode_item in episodes_models {
                        if let crate::models::MediaItem::Episode(episode) = episode_item {
                            self.add_episode_card(episode, false);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to load episodes: {}", e);
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
                } else {
                    if is_watched {
                        viewmodel.mark_as_unwatched().await;
                    } else {
                        viewmodel.mark_as_watched().await;
                    }
                }
            });
        }
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
            let metadata_json: serde_json::Value = metadata.clone().into();
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
