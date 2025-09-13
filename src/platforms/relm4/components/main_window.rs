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
    // Window chrome management
    header_bar: adw::HeaderBar,
    toolbar_view: adw::ToolbarView,
    // Window state for restoration
    saved_window_size: Option<(i32, i32)>,
    was_maximized: bool,
    was_fullscreen: bool,
}

#[derive(Debug)]
pub enum MainWindowInput {
    Navigate(String),
    NavigateToSource(SourceId),
    NavigateToLibrary(LibraryId),
    NavigateToMediaItem(MediaItemId),
    NavigateToPlayer(MediaItemId),
    ToggleSidebar,
    RestoreWindowChrome,
    ResizeWindow(i32, i32),
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
        #[root]
        adw::ApplicationWindow {
            set_title: Some("Reel"),
            set_default_width: 1200,
            set_default_height: 800,

            #[wrap(Some)]
            #[name(toolbar_view)]
            set_content = &adw::ToolbarView {
                #[name(header_bar)]
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
            header_bar: adw::HeaderBar::new(),
            toolbar_view: adw::ToolbarView::new(),
            saved_window_size: None,
            was_maximized: false,
            was_fullscreen: false,
        };

        let widgets = view_output!();

        // Set the sidebar widget
        widgets.sidebar_page.set_child(Some(model.sidebar.widget()));

        // Store references to widgets for later use
        model.navigation_view.clone_from(&widgets.navigation_view);
        model.header_bar.clone_from(&widgets.header_bar);
        model.toolbar_view.clone_from(&widgets.toolbar_view);

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

                // Save current window state before entering player
                let (width, height) = root.default_size();
                self.saved_window_size = Some((width, height));
                self.was_maximized = root.is_maximized();
                self.was_fullscreen = root.is_fullscreen();

                // Hide window chrome for immersive viewing
                self.header_bar.set_visible(false);
                self.toolbar_view.set_top_bar_style(adw::ToolbarStyle::Flat);

                // Create player page if not exists
                if self.player_page.is_none() {
                    let db = std::sync::Arc::new(self.db.clone());
                    let sender_clone = sender.clone();
                    self.player_page = Some(
                        PlayerPage::builder()
                            .launch((Some(media_id.clone()), db, root.clone()))
                            .forward(sender.input_sender(), move |output| match output {
                                crate::platforms::relm4::components::pages::player::PlayerOutput::NavigateBack => {
                                    // Restore window chrome when leaving player
                                    sender_clone.input(MainWindowInput::RestoreWindowChrome);
                                    MainWindowInput::Navigate("back".to_string())
                                }
                                crate::platforms::relm4::components::pages::player::PlayerOutput::MediaLoaded => {
                                    tracing::info!("Media loaded in player");
                                    MainWindowInput::Navigate("media_loaded".to_string())
                                }
                                crate::platforms::relm4::components::pages::player::PlayerOutput::Error(msg) => {
                                    tracing::error!("Player error: {}", msg);
                                    // Restore window chrome on error
                                    sender_clone.input(MainWindowInput::RestoreWindowChrome);
                                    MainWindowInput::Navigate("player_error".to_string())
                                }
                                crate::platforms::relm4::components::pages::player::PlayerOutput::WindowStateChanged { width, height } => {
                                    // Player is requesting window size change for aspect ratio
                                    MainWindowInput::ResizeWindow(width, height)
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
            MainWindowInput::RestoreWindowChrome => {
                tracing::info!("Restoring window chrome after player");

                // Show window chrome again
                self.header_bar.set_visible(true);
                self.toolbar_view
                    .set_top_bar_style(adw::ToolbarStyle::Raised);

                // Restore window size and state
                if let Some((width, height)) = self.saved_window_size {
                    root.set_default_size(width, height);
                }

                if self.was_maximized {
                    root.maximize();
                } else if self.was_fullscreen {
                    root.fullscreen();
                } else {
                    root.unmaximize();
                    root.unfullscreen();
                }

                // Pop the player page from navigation
                self.navigation_view.pop();
            }
            MainWindowInput::ResizeWindow(width, height) => {
                tracing::info!(
                    "Resizing window to {}x{} for video aspect ratio",
                    width,
                    height
                );
                root.set_default_size(width, height);

                // Center the window on screen after resize
                // Note: GTK4 doesn't have a direct center method, but setting default size
                // and letting the window manager handle it usually works well
            }
        }
    }
}
