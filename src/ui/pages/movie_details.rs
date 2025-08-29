use adw::prelude::*;
use gtk4::{self, glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use std::cell::RefCell;
use std::sync::Arc;
use tracing::{error, info};

use crate::backends::traits::MediaBackend;
use crate::models::{Movie, StreamInfo};
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
    #[template(resource = "/dev/arsfeld/Reel/movie_details.ui")]
    pub struct MovieDetailsPage {
        #[template_child]
        pub movie_poster: TemplateChild<gtk4::Picture>,
        #[template_child]
        pub movie_backdrop: TemplateChild<gtk4::Picture>,
        #[template_child]
        pub poster_placeholder: TemplateChild<gtk4::Box>,
        #[template_child]
        pub movie_title: TemplateChild<gtk4::Label>,
        #[template_child]
        pub year_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub rating_box: TemplateChild<gtk4::Box>,
        #[template_child]
        pub rating_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub duration_box: TemplateChild<gtk4::Box>,
        #[template_child]
        pub duration_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub synopsis_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub genres_box: TemplateChild<gtk4::FlowBox>,
        #[template_child]
        pub play_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub mark_watched_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub watched_icon: TemplateChild<gtk4::Image>,
        #[template_child]
        pub watched_label: TemplateChild<gtk4::Label>,

        // Stream info fields
        #[template_child]
        pub stream_info_list: TemplateChild<gtk4::ListBox>,
        #[template_child]
        pub video_codec_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub video_codec_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub audio_codec_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub audio_codec_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub resolution_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub resolution_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub bitrate_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub bitrate_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub container_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub container_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub file_size_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub file_size_label: TemplateChild<gtk4::Label>,

        pub state: RefCell<Option<Arc<AppState>>>,
        pub viewmodel: RefCell<Option<Arc<DetailsViewModel>>>,
        pub current_movie: RefCell<Option<Movie>>,
        pub on_play_clicked: RefCell<Option<Box<dyn Fn(&Movie)>>>,
        pub load_generation: RefCell<u64>,
        pub property_subscriptions: RefCell<Vec<tokio::sync::broadcast::Receiver<()>>>,
    }

    impl Default for MovieDetailsPage {
        fn default() -> Self {
            Self {
                movie_poster: Default::default(),
                movie_backdrop: Default::default(),
                poster_placeholder: Default::default(),
                movie_title: Default::default(),
                year_label: Default::default(),
                rating_box: Default::default(),
                rating_label: Default::default(),
                duration_box: Default::default(),
                duration_label: Default::default(),
                synopsis_label: Default::default(),
                genres_box: Default::default(),
                play_button: Default::default(),
                mark_watched_button: Default::default(),
                watched_icon: Default::default(),
                watched_label: Default::default(),
                stream_info_list: Default::default(),
                video_codec_row: Default::default(),
                video_codec_label: Default::default(),
                audio_codec_row: Default::default(),
                audio_codec_label: Default::default(),
                resolution_row: Default::default(),
                resolution_label: Default::default(),
                bitrate_row: Default::default(),
                bitrate_label: Default::default(),
                container_row: Default::default(),
                container_label: Default::default(),
                file_size_row: Default::default(),
                file_size_label: Default::default(),
                state: RefCell::new(None),
                viewmodel: RefCell::new(None),
                current_movie: RefCell::new(None),
                on_play_clicked: RefCell::new(None),
                load_generation: RefCell::new(0),
                property_subscriptions: RefCell::new(Vec::new()),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MovieDetailsPage {
        const NAME: &'static str = "MovieDetailsPage";
        type Type = super::MovieDetailsPage;
        type ParentType = gtk4::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MovieDetailsPage {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            // Connect play button
            self.play_button.connect_clicked(glib::clone!(
                #[weak]
                obj,
                move |_| {
                    obj.on_play_clicked();
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

    impl WidgetImpl for MovieDetailsPage {}
    impl BoxImpl for MovieDetailsPage {}
}

glib::wrapper! {
    pub struct MovieDetailsPage(ObjectSubclass<imp::MovieDetailsPage>)
        @extends gtk4::Widget, gtk4::Box,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Orientable;
}

impl MovieDetailsPage {
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
        }
    }

    pub fn load_movie(&self, movie: Movie) {
        info!("Loading movie details: {}", movie.title);

        let imp = self.imp();

        // Store current movie for compatibility
        imp.current_movie.replace(Some(movie.clone()));

        // Use ViewModel to load the media details
        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            let viewmodel = viewmodel.clone();
            let movie_id = movie.id.clone();

            glib::spawn_future_local(async move {
                if let Err(e) = viewmodel.load_media(movie_id).await {
                    error!("Failed to load movie through ViewModel: {}", e);
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
                imp.movie_poster.set_paintable(gtk4::gdk::Paintable::NONE);
                imp.movie_backdrop.set_paintable(gtk4::gdk::Paintable::NONE);
                imp.poster_placeholder.set_visible(true);
            }
        }
    }

    async fn on_loading_changed(&self) {
        let imp = self.imp();

        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            let is_loading = viewmodel.is_loading().get().await;

            if is_loading {
                // Show loading state
                imp.movie_poster.set_paintable(gtk4::gdk::Paintable::NONE);
                imp.movie_backdrop.set_paintable(gtk4::gdk::Paintable::NONE);
                imp.poster_placeholder.set_visible(true);
                imp.stream_info_list.set_visible(false);
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
                imp.watched_label.set_text("Watched");
                imp.mark_watched_button.remove_css_class("suggested-action");
            } else {
                imp.watched_icon.set_icon_name(Some("checkbox-symbolic"));
                imp.watched_label.set_text("Mark as Watched");
                imp.mark_watched_button.add_css_class("suggested-action");
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

        // Extract movie from MediaItem enum
        let movie = match media {
            crate::models::MediaItem::Movie(movie) => movie,
            _ => {
                error!("MovieDetailsPage received non-movie MediaItem");
                return;
            }
        };

        // Load backdrop image
        if let Some(backdrop_url) = &movie.backdrop_url {
            let backdrop_picture = imp.movie_backdrop.clone();
            let url = backdrop_url.clone();

            glib::spawn_future_local(async move {
                match IMAGE_LOADER.load_image(&url, ImageSize::Large).await {
                    Ok(texture) => {
                        backdrop_picture.set_paintable(Some(&texture));
                    }
                    Err(e) => {
                        error!("Failed to load movie backdrop: {}", e);
                    }
                }
            });
        }

        // Load poster image
        if let Some(poster_url) = &movie.poster_url {
            let picture = imp.movie_poster.clone();
            let placeholder = imp.poster_placeholder.clone();
            let url = poster_url.clone();

            glib::spawn_future_local(async move {
                match IMAGE_LOADER.load_image(&url, ImageSize::Large).await {
                    Ok(texture) => {
                        picture.set_paintable(Some(&texture));
                        placeholder.set_visible(false);
                    }
                    Err(e) => {
                        error!("Failed to load movie poster: {}", e);
                        // Keep placeholder visible on error
                    }
                }
            });
        }

        // Set title
        imp.movie_title.set_label(&movie.title);

        // Set year
        if let Some(year) = movie.year {
            imp.year_label.set_text(&format!("{}", year));
            imp.year_label.set_visible(true);
        } else {
            imp.year_label.set_visible(false);
        }

        // Set rating
        if let Some(rating) = movie.rating {
            imp.rating_label.set_text(&format!("{:.1}", rating));
            imp.rating_box.set_visible(true);
        } else {
            imp.rating_box.set_visible(false);
        }

        // Set duration
        let duration_ms = movie.duration.as_millis() as i64;
        if duration_ms > 0 {
            let duration_secs = duration_ms / 1000;
            let duration_mins = duration_secs / 60;
            let duration_hours = duration_mins / 60;
            let remaining_mins = duration_mins % 60;

            if duration_hours > 0 {
                imp.duration_label
                    .set_text(&format!("{}h {}m", duration_hours, remaining_mins));
            } else {
                imp.duration_label
                    .set_text(&format!("{} min", duration_mins));
            }
            imp.duration_box.set_visible(duration_mins > 0);
        } else {
            imp.duration_box.set_visible(false);
        }

        // Set synopsis
        if let Some(overview) = &movie.overview {
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
                .css_classes(vec!["card", "compact"])
                .build();

            let genre_label = gtk4::Label::builder()
                .label(genre)
                .css_classes(vec!["caption"])
                .margin_top(6)
                .margin_bottom(6)
                .margin_start(12)
                .margin_end(12)
                .build();

            genre_chip.set_child(Some(&genre_label));
            imp.genres_box.insert(&genre_chip, -1);
        }

        imp.genres_box.set_visible(!metadata.genres.is_empty());

        // Load stream info asynchronously
        let movie_clone = movie.clone();
        let page_weak = self.downgrade();
        glib::spawn_future_local(async move {
            if let Some(page) = page_weak.upgrade() {
                page.load_stream_info(&movie_clone).await;
            }
        });
    }

    async fn load_stream_info(&self, movie: &crate::models::Movie) {
        let imp = self.imp();

        // Get backend and fetch stream info
        if let Some(state) = imp.state.borrow().as_ref() {
            let backend_id = &movie.backend_id;
            if let Some(backend) = state.source_coordinator.get_backend(backend_id).await {
                match backend.get_stream_url(&movie.id).await {
                    Ok(stream_info) => {
                        self.display_stream_info(&stream_info);
                    }
                    Err(e) => {
                        error!("Failed to load stream info: {}", e);
                        // Hide stream info section on error
                        imp.stream_info_list.set_visible(false);
                    }
                }
            }
        }
    }

    fn display_stream_info(&self, stream_info: &StreamInfo) {
        let imp = self.imp();

        // Video codec
        imp.video_codec_label.set_text(&stream_info.video_codec);

        // Audio codec
        imp.audio_codec_label.set_text(&stream_info.audio_codec);

        // Resolution
        imp.resolution_label.set_text(&format!(
            "{}x{}",
            stream_info.resolution.width, stream_info.resolution.height
        ));

        // Add quality badge if 4K or higher
        if stream_info.resolution.width >= 3840 {
            imp.resolution_label.set_text(&format!(
                "{}x{} (4K)",
                stream_info.resolution.width, stream_info.resolution.height
            ));
        } else if stream_info.resolution.width >= 1920 {
            imp.resolution_label.set_text(&format!(
                "{}x{} (HD)",
                stream_info.resolution.width, stream_info.resolution.height
            ));
        }

        // Bitrate (convert to Mbps for readability)
        let bitrate_mbps = stream_info.bitrate as f64 / 1_000_000.0;
        imp.bitrate_label
            .set_text(&format!("{:.1} Mbps", bitrate_mbps));

        // Container
        imp.container_label.set_text(&stream_info.container);

        // Direct play indicator
        if stream_info.direct_play {
            imp.container_label
                .set_text(&format!("{} (Direct Play)", stream_info.container));
        } else {
            imp.container_label
                .set_text(&format!("{} (Transcode)", stream_info.container));
        }

        imp.stream_info_list.set_visible(true);
    }

    // Removed - now handled by on_watched_changed via property binding

    fn on_play_clicked(&self) {
        if let Some(movie) = self.imp().current_movie.borrow().as_ref()
            && let Some(callback) = self.imp().on_play_clicked.borrow().as_ref()
        {
            callback(movie);
        }
    }

    fn on_mark_watched_clicked(&self) {
        let imp = self.imp();

        if let Some(viewmodel) = imp.viewmodel.borrow().as_ref() {
            let viewmodel = viewmodel.clone();

            glib::spawn_future_local(async move {
                let is_watched = viewmodel.is_watched().get().await;

                if is_watched {
                    viewmodel.mark_as_unwatched().await;
                } else {
                    viewmodel.mark_as_watched().await;
                }
            });
        }
    }

    pub fn set_on_play_clicked<F>(&self, callback: F)
    where
        F: Fn(&Movie) + 'static,
    {
        self.imp().on_play_clicked.replace(Some(Box::new(callback)));
    }
}
