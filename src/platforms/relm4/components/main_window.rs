use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use relm4::gtk;
use relm4::prelude::*;

use super::pages::{HomePage, LibraryPage, MovieDetailsPage, PlayerPage, ShowDetailsPage};
use super::sidebar::{Sidebar, SidebarOutput};
use crate::db::connection::DatabaseConnection;
use crate::models::{LibraryId, MediaItemId, SourceId};

#[derive(Debug)]
pub struct MainWindow {
    db: DatabaseConnection,
    sidebar: Controller<Sidebar>,
    home_page: AsyncController<HomePage>,
    library_page: Option<AsyncController<LibraryPage>>,
    movie_details_page: Option<AsyncController<MovieDetailsPage>>,
    show_details_page: Option<AsyncController<ShowDetailsPage>>,
    player_page: Option<AsyncController<PlayerPage>>,
    navigation_view: adw::NavigationView,
}

#[derive(Debug)]
pub enum MainWindowInput {
    Navigate(String),
    NavigateToSource(SourceId),
    NavigateToLibrary(LibraryId),
    NavigateToMediaItem(MediaItemId),
    NavigateToPlayer(MediaItemId),
    ToggleSidebar,
}

#[derive(Debug)]
pub enum MainWindowOutput {
    Quit,
}

#[relm4::component(pub async)]
impl AsyncComponent for MainWindow {
    type Init = DatabaseConnection;
    type Input = MainWindowInput;
    type Output = MainWindowOutput;
    type CommandOutput = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("Reel"),
            set_default_width: 1200,
            set_default_height: 800,

            #[wrap(Some)]
            set_content = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    pack_start = &gtk::Button {
                        set_icon_name: "sidebar-show-symbolic",
                        connect_clicked => MainWindowInput::ToggleSidebar,
                    },

                    set_title_widget = Some(&adw::WindowTitle::new("Reel", "Media Player")),

                    pack_end = &gtk::MenuButton {
                        set_icon_name: "open-menu-symbolic",
                    },
                },

                #[wrap(Some)]
                set_content = &adw::NavigationSplitView {
                    set_sidebar_width_fraction: 0.2,
                    set_min_sidebar_width: 200.0,
                    set_max_sidebar_width: 300.0,

                    #[wrap(Some)]
                    #[name(sidebar_page)]
                    set_sidebar = &adw::NavigationPage {
                        set_title: "Navigation",
                    },

                    #[wrap(Some)]
                    set_content = &adw::NavigationPage {
                        set_title: "Content",

                        #[wrap(Some)]
                        #[name(navigation_view)]
                        set_child = &adw::NavigationView {},
                    },
                },
            },
        }
    }

    async fn init(
        db: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        // Initialize the sidebar
        let sidebar =
            Sidebar::builder()
                .launch(db.clone())
                .forward(sender.input_sender(), |output| match output {
                    SidebarOutput::NavigateToSource(id) => MainWindowInput::NavigateToSource(id),
                    SidebarOutput::NavigateToLibrary(id) => MainWindowInput::NavigateToLibrary(id),
                });

        // Initialize the home page
        let home_page = HomePage::builder()
            .launch(db.clone())
            .forward(sender.input_sender(), |output| match output {
                crate::platforms::relm4::components::pages::home::HomePageOutput::NavigateToMediaItem(id) => {
                    MainWindowInput::NavigateToMediaItem(id)
                }
            });

        let mut model = Self {
            db,
            sidebar,
            home_page,
            library_page: None,
            movie_details_page: None,
            show_details_page: None,
            player_page: None,
            navigation_view: adw::NavigationView::new(),
        };

        let widgets = view_output!();

        // Set the sidebar widget
        widgets.sidebar_page.set_child(Some(model.sidebar.widget()));

        // Store reference to navigation view for later use
        model.navigation_view.clone_from(&widgets.navigation_view);

        // Add the home page as the initial page
        let home_nav_page = adw::NavigationPage::builder()
            .title("Home")
            .child(model.home_page.widget())
            .build();
        model.navigation_view.add(&home_nav_page);

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root,
    ) {
        match msg {
            MainWindowInput::Navigate(page) => {
                tracing::info!("Navigating to: {}", page);
                // TODO: Implement generic navigation
            }
            MainWindowInput::NavigateToSource(source_id) => {
                tracing::info!("Navigating to source: {}", source_id);
                // TODO: Create and push source page
                // For now, just create a placeholder page
                let page = adw::NavigationPage::builder()
                    .title(&format!("Source: {}", source_id))
                    .child(&gtk::Label::new(Some(&format!(
                        "Source {} content",
                        source_id
                    ))))
                    .build();
                self.navigation_view.push(&page);
            }
            MainWindowInput::NavigateToLibrary(library_id) => {
                tracing::info!("Navigating to library: {}", library_id);

                // Create library page if it doesn't exist
                if self.library_page.is_none() {
                    let library_controller = LibraryPage::builder()
                        .launch(self.db.clone())
                        .forward(sender.input_sender(), |output| match output {
                            crate::platforms::relm4::components::pages::library::LibraryPageOutput::NavigateToMediaItem(id) => {
                                MainWindowInput::NavigateToMediaItem(id)
                            }
                        });
                    self.library_page = Some(library_controller);
                }

                // Set the library on the page
                if let Some(ref library_controller) = self.library_page {
                    library_controller.emit(crate::platforms::relm4::components::pages::library::LibraryPageInput::SetLibrary(library_id.clone()));

                    // Create navigation page and push it
                    let page = adw::NavigationPage::builder()
                        .title(&format!("Library"))
                        .child(library_controller.widget())
                        .build();
                    self.navigation_view.push(&page);
                }
            }
            MainWindowInput::NavigateToMediaItem(item_id) => {
                tracing::info!("Navigating to media item: {}", item_id);

                // Create movie details page if not exists
                if self.movie_details_page.is_none() {
                    let db = std::sync::Arc::new(self.db.clone());
                    self.movie_details_page = Some(
                        MovieDetailsPage::builder()
                            .launch((item_id.clone(), db))
                            .forward(sender.input_sender(), |output| match output {
                                crate::platforms::relm4::components::pages::movie_details::MovieDetailsOutput::PlayMedia(id) => {
                                    tracing::info!("Playing media: {}", id);
                                    MainWindowInput::NavigateToPlayer(id)
                                }
                                crate::platforms::relm4::components::pages::movie_details::MovieDetailsOutput::NavigateBack => {
                                    MainWindowInput::Navigate("back".to_string())
                                }
                            }),
                    );
                } else if let Some(ref movie_page) = self.movie_details_page {
                    // Update existing page with new item
                    movie_page.sender().send(crate::platforms::relm4::components::pages::movie_details::MovieDetailsInput::LoadMovie(item_id.clone())).unwrap();
                }

                // Push the page to navigation
                if let Some(ref movie_page) = self.movie_details_page {
                    let page = adw::NavigationPage::builder()
                        .title("Movie Details")
                        .child(movie_page.widget())
                        .build();
                    self.navigation_view.push(&page);
                }
            }
            MainWindowInput::NavigateToPlayer(media_id) => {
                tracing::info!("Navigating to player for media: {}", media_id);

                // Create player page if not exists
                if self.player_page.is_none() {
                    let db = std::sync::Arc::new(self.db.clone());
                    self.player_page = Some(
                        PlayerPage::builder()
                            .launch((Some(media_id.clone()), db))
                            .forward(sender.input_sender(), |output| match output {
                                crate::platforms::relm4::components::pages::player::PlayerOutput::NavigateBack => {
                                    MainWindowInput::Navigate("back".to_string())
                                }
                                crate::platforms::relm4::components::pages::player::PlayerOutput::MediaLoaded => {
                                    tracing::info!("Media loaded in player");
                                    MainWindowInput::Navigate("media_loaded".to_string())
                                }
                                crate::platforms::relm4::components::pages::player::PlayerOutput::Error(msg) => {
                                    tracing::error!("Player error: {}", msg);
                                    MainWindowInput::Navigate("player_error".to_string())
                                }
                            }),
                    );
                } else if let Some(ref player_page) = self.player_page {
                    // Update existing page with new media
                    player_page.sender().send(crate::platforms::relm4::components::pages::player::PlayerInput::LoadMedia(media_id.clone())).unwrap();
                }

                // Push the player page to navigation
                if let Some(ref player_page) = self.player_page {
                    let page = adw::NavigationPage::builder()
                        .title("Player")
                        .child(player_page.widget())
                        .build();
                    self.navigation_view.push(&page);
                }
            }
            MainWindowInput::ToggleSidebar => {
                tracing::info!("Toggling sidebar");
                // TODO: Implement sidebar toggle
            }
        }
    }
}
