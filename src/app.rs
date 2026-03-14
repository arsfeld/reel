use std::sync::Arc;

use adw::prelude::*;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use relm4::prelude::*;
use rusqlite::Connection;
use tracing::info;

use crate::components::connection::{ConnectionDialog, ConnectionDialogOutput};
use crate::components::detail::movie_detail::{MovieDetail, MovieDetailMsg, MovieDetailOutput};
use crate::components::detail::show_detail::{ShowDetail, ShowDetailMsg, ShowDetailOutput};
use crate::components::library::{LibraryView, LibraryViewMsg, LibraryViewOutput};
use crate::components::player::drop_target;
use crate::components::player::shortcuts::{self, PlayerAction};
use crate::components::player::video_area::{VideoArea, VideoAreaMsg, VideoAreaOutput};
use crate::components::sidebar::{Sidebar, SidebarOutput};
use crate::db;
use crate::models::library::LibraryType;
use crate::models::media::{MediaItem, MediaType, SourceType};
use crate::models::source::{Source, SourceConfig};
use crate::navigation::CurrentView;
use crate::player::backend::{self, PlayState};
use crate::services::artwork::ArtworkCache;
use crate::services::plex::api::PlexClient;
use crate::services::plex::source::PlexSource;
use crate::services::screensaver::ScreensaverInhibitor;
use crate::services::window_state::{self, WindowState};

#[allow(dead_code)]
pub struct App {
    video_area: Controller<VideoArea>,
    sidebar: Controller<Sidebar>,
    library_view: Controller<LibraryView>,
    movie_detail: Controller<MovieDetail>,
    show_detail: Controller<ShowDetail>,
    connection_dialog: Option<Controller<ConnectionDialog>>,
    screensaver: ScreensaverInhibitor,
    toast_overlay: adw::ToastOverlay,
    stack: gtk4::Stack,
    nav_view: adw::NavigationView,
    current_view: CurrentView,
    db_conn: Option<Connection>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum AppMsg {
    Navigate(LibraryType),
    ShowMovieDetail(crate::models::media::MediaItem),
    ShowShowDetail(crate::models::media::MediaItem),
    GoBack,
    VideoOutput(VideoAreaOutput),
    PlayMedia(String),
    PlayerAction(VideoAreaMsg),
    OpenFile(String),
    ShowFileChooser,
    ToggleFullscreen,
    ExitFullscreen,
    FilesDropped(String),
    ShowConnectionDialog,
    ConnectionSaved {
        url: String,
        token: String,
        name: String,
    },
    ShowToast(String),
    FocusSearch,
    ShowCollections,
    ShowCollectionDetail(MediaItem),
}

#[derive(Debug)]
pub enum AppCmd {
    /// Validated (or re-discovered) server URL on startup.
    SourceValidated {
        url: String,
        token: String,
        name: String,
    },
    SourceValidationFailed(String),
}

#[relm4::component(pub)]
impl Component for App {
    type Init = Option<String>;
    type Input = AppMsg;
    type Output = ();
    type CommandOutput = AppCmd;

    view! {
        #[root]
        adw::ApplicationWindow {
            set_title: Some("Reel"),
            set_default_width: 1280,
            set_default_height: 720,
        }
    }

    #[allow(clippy::too_many_lines)]
    fn init(
        file_arg: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let video_area = VideoArea::builder()
            .launch(())
            .forward(sender.input_sender(), AppMsg::VideoOutput);

        let sidebar = Sidebar::builder()
            .launch(())
            .forward(sender.input_sender(), |output| match output {
                SidebarOutput::Navigate(target) => AppMsg::Navigate(target),
                SidebarOutput::ShowCollections => AppMsg::ShowCollections,
            });

        let library_view = LibraryView::builder().launch(()).forward(
            sender.input_sender(),
            |output| match output {
                LibraryViewOutput::ShowDetail(item) => match item.media_type {
                    MediaType::Movie => AppMsg::ShowMovieDetail(item),
                    MediaType::Show => AppMsg::ShowShowDetail(item),
                    MediaType::Collection => AppMsg::ShowCollectionDetail(item),
                    _ => AppMsg::ShowToast("Unsupported media type".to_string()),
                },
                LibraryViewOutput::Error(msg) => AppMsg::ShowToast(msg),
            },
        );

        let movie_detail = MovieDetail::builder().launch(()).forward(
            sender.input_sender(),
            |output| match output {
                MovieDetailOutput::PlayMedia(url) => AppMsg::PlayMedia(url),
                MovieDetailOutput::Error(msg) => AppMsg::ShowToast(msg),
            },
        );

        let show_detail = ShowDetail::builder().launch(()).forward(
            sender.input_sender(),
            |output| match output {
                ShowDetailOutput::PlayMedia(url) => AppMsg::PlayMedia(url),
                ShowDetailOutput::Error(msg) => AppMsg::ShowToast(msg),
            },
        );

        let widgets = view_output!();

        // --- Build widget hierarchy manually ---

        let toast_overlay = adw::ToastOverlay::new();
        let stack = gtk4::Stack::builder()
            .transition_type(gtk4::StackTransitionType::Crossfade)
            .build();

        // Shell page: sidebar + navigation content
        let split_view = adw::NavigationSplitView::new();

        let sidebar_nav_page = adw::NavigationPage::builder()
            .title("Library")
            .child(sidebar.widget())
            .build();
        split_view.set_sidebar(Some(&sidebar_nav_page));

        let nav_view = adw::NavigationView::new();

        // Library root page (with header bar + settings button)
        let library_toolbar = adw::ToolbarView::new();
        let library_header = adw::HeaderBar::new();

        let settings_button = gtk4::Button::builder()
            .icon_name("emblem-system-symbolic")
            .tooltip_text("Plex Settings")
            .build();
        let sender_settings = sender.input_sender().clone();
        settings_button.connect_clicked(move |_| {
            let _ = sender_settings.send(AppMsg::ShowConnectionDialog);
        });
        library_header.pack_end(&settings_button);
        library_toolbar.add_top_bar(&library_header);
        library_toolbar.set_content(Some(library_view.widget()));

        let library_nav_page = adw::NavigationPage::builder()
            .title("Library")
            .tag("library")
            .child(&library_toolbar)
            .build();
        nav_view.add(&library_nav_page);

        let content_nav_page = adw::NavigationPage::builder()
            .title("Content")
            .child(&nav_view)
            .build();
        split_view.set_content(Some(&content_nav_page));

        // Player page
        let player_page = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .build();
        player_page.append(video_area.widget());
        video_area.widget().set_vexpand(true);

        stack.add_named(&split_view, Some("shell"));
        stack.add_named(&player_page, Some("player"));

        toast_overlay.set_child(Some(&stack));
        root.set_content(Some(&toast_overlay));

        // Keyboard shortcuts
        let key_controller = gtk4::EventControllerKey::new();
        let sender_key = sender.input_sender().clone();
        let stack_key = stack.clone();
        let root_key = root.clone();
        key_controller.connect_key_pressed(move |_controller, key, _code, mods| {
            let in_player = stack_key.visible_child_name().as_deref() == Some("player");

            // Detect if a text input widget has focus
            let is_text_focused = gtk4::prelude::GtkWindowExt::focus(&root_key).is_some_and(|w| {
                w.is::<gtk4::SearchEntry>()
                    || w.is::<gtk4::Entry>()
                    || w.is::<gtk4::Text>()
                    || w.is::<gtk4::TextView>()
            });

            if let Some(action) = shortcuts::map_key_to_action(key, mods, is_text_focused) {
                if in_player {
                    dispatch_player_action_sender(&sender_key, action);
                    glib::Propagation::Stop
                } else {
                    match action {
                        PlayerAction::ExitFullscreen => {
                            let _ = sender_key.send(AppMsg::GoBack);
                            glib::Propagation::Stop
                        }
                        _ => glib::Propagation::Proceed,
                    }
                }
            } else if !in_player && !is_text_focused {
                // Library-level shortcuts
                let ctrl = mods.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
                match key {
                    gtk4::gdk::Key::f if ctrl => {
                        let _ = sender_key.send(AppMsg::FocusSearch);
                        glib::Propagation::Stop
                    }
                    gtk4::gdk::Key::slash if mods.is_empty() => {
                        let _ = sender_key.send(AppMsg::FocusSearch);
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                }
            } else {
                glib::Propagation::Proceed
            }
        });
        root.add_controller(key_controller);

        // Drag-and-drop
        let drop_target = gtk4::DropTarget::builder()
            .actions(gtk4::gdk::DragAction::COPY)
            .build();
        drop_target.set_types(&[gtk4::glib::types::Type::STRING]);
        let sender_drop = sender.input_sender().clone();
        drop_target.connect_drop(move |_target, value, _x, _y| {
            if let Ok(text) = value.get::<String>() {
                for uri in drop_target::parse_uri_list(&text) {
                    let _ = sender_drop.send(AppMsg::FilesDropped(uri));
                }
                return true;
            }
            false
        });
        root.add_controller(drop_target);

        // Load window state
        let ws = window_state::load();
        root.set_default_size(ws.width, ws.height);
        if ws.maximized {
            root.maximize();
        }

        // Save window state on close
        let root_close = root.clone();
        root.connect_close_request(move |_window| {
            let (width, height) = root_close.default_size();
            let state = WindowState {
                width,
                height,
                maximized: root_close.is_maximized(),
                volume: 100.0,
            };
            if let Err(e) = window_state::save(&state) {
                tracing::warn!("Failed to save window state: {}", e);
            }
            glib::Propagation::Proceed
        });

        // Initialize database
        let db_conn = init_database();

        // Load and validate saved source (async — tests connection, re-discovers if stale)
        if let Some(ref conn) = db_conn {
            let repo = crate::db::source_repo::SourceRepo::new(conn);
            if let Ok(sources) = repo.list()
                && let Some(source) = sources.into_iter().find(|s| s.enabled)
            {
                info!(
                    "Loaded saved Plex source: {} (url={})",
                    source.name, source.config.url
                );
                let url = source.config.url.clone();
                let token = source.config.token.clone();
                let name = source.name.clone();
                let data_dir = crate::config::data_dir();
                sender.oneshot_command(async move {
                    validate_or_rediscover_source(url, token, name, data_dir).await
                });
            }
        }

        let mut model = Self {
            video_area,
            sidebar,
            library_view,
            movie_detail,
            show_detail,
            connection_dialog: None,
            screensaver: ScreensaverInhibitor::new(),
            toast_overlay,
            stack,
            nav_view,
            current_view: CurrentView::default(),
            db_conn,
        };

        // Handle CLI file arg or start with library
        let sender_init = sender.clone();
        let has_file_arg = file_arg.is_some();
        glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
            if let Some(path) = file_arg {
                sender_init.input(AppMsg::OpenFile(path));
            } else {
                sender_init.input(AppMsg::Navigate(LibraryType::Movie));
            }
        });

        let _ = has_file_arg;
        let _ = &mut model;

        ComponentParts { model, widgets }
    }

    #[allow(clippy::too_many_lines)]
    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            AppMsg::Navigate(library_type) => {
                self.current_view = CurrentView::Library(library_type);
                self.stack.set_visible_child_name("shell");
                root.set_fullscreened(false);
                root.set_title(Some("Reel"));
                self.library_view
                    .emit(LibraryViewMsg::LoadLibrary(library_type));
            }
            AppMsg::ShowMovieDetail(item) => {
                self.current_view = CurrentView::MovieDetail(item.id.clone());
                self.movie_detail
                    .emit(MovieDetailMsg::LoadMovie(item.clone()));
                let page = adw::NavigationPage::builder()
                    .title(&item.title)
                    .child(self.movie_detail.widget())
                    .build();
                self.nav_view.push(&page);
            }
            AppMsg::ShowShowDetail(item) => {
                self.current_view = CurrentView::ShowDetail(item.id.clone());
                self.show_detail.emit(ShowDetailMsg::LoadShow(item.clone()));
                let page = adw::NavigationPage::builder()
                    .title(&item.title)
                    .child(self.show_detail.widget())
                    .build();
                self.nav_view.push(&page);
            }
            AppMsg::GoBack => {
                if self.stack.visible_child_name().as_deref() == Some("player") {
                    self.stack.set_visible_child_name("shell");
                    root.set_fullscreened(false);
                    root.set_title(Some("Reel"));
                } else {
                    self.nav_view.pop();
                }
            }
            AppMsg::PlayMedia(url) => {
                info!("Playing media: {}...", &url[..url.len().min(80)]);
                self.current_view = CurrentView::Player;
                self.stack.set_visible_child_name("player");
                self.video_area.emit(VideoAreaMsg::LoadFile(url));
            }
            AppMsg::PlayerAction(video_msg) => {
                self.video_area.emit(video_msg);
            }
            AppMsg::OpenFile(path) => {
                info!("Opening file: {}", path);
                self.current_view = CurrentView::Player;
                self.stack.set_visible_child_name("player");
                self.video_area.emit(VideoAreaMsg::LoadFile(path));
            }
            AppMsg::ToggleFullscreen => {
                let new_fs = !root.is_fullscreen();
                root.set_fullscreened(new_fs);
                self.video_area
                    .emit(VideoAreaMsg::FullscreenChanged(new_fs));
            }
            AppMsg::ExitFullscreen => {
                if root.is_fullscreen() {
                    root.set_fullscreened(false);
                    self.video_area.emit(VideoAreaMsg::FullscreenChanged(false));
                }
            }
            AppMsg::FilesDropped(uri) => {
                self.current_view = CurrentView::Player;
                self.stack.set_visible_child_name("player");
                self.video_area.emit(VideoAreaMsg::FilesDropped(uri));
            }
            AppMsg::VideoOutput(output) => match output {
                VideoAreaOutput::FileLoaded => {
                    self.screensaver.inhibit(root);
                }
                VideoAreaOutput::PositionChanged { .. } => {}
                VideoAreaOutput::StateChanged(state) => {
                    if self.current_view == CurrentView::Player {
                        root.set_title(Some(backend::window_title_for_state(state)));
                    }
                    match state {
                        PlayState::Playing => self.screensaver.inhibit(root),
                        PlayState::Paused | PlayState::Stopped => {
                            self.screensaver.uninhibit(root);
                        }
                    }
                }
                VideoAreaOutput::EndOfFile(reason) => {
                    info!("Playback ended: {:?}", reason);
                    self.screensaver.uninhibit(root);
                    if self.current_view == CurrentView::Player {
                        self.stack.set_visible_child_name("shell");
                        root.set_fullscreened(false);
                        root.set_title(Some("Reel"));
                    }
                }
                VideoAreaOutput::VolumeChanged { .. } | VideoAreaOutput::SpeedChanged(_) => {}
                VideoAreaOutput::ToggleFullscreen => {
                    let new_fs = !root.is_fullscreen();
                    root.set_fullscreened(new_fs);
                    self.video_area
                        .emit(VideoAreaMsg::FullscreenChanged(new_fs));
                }
                VideoAreaOutput::LoadSubtitleFile => {
                    show_subtitle_chooser(root, &self.video_area);
                }
                VideoAreaOutput::Error(msg) => {
                    sender.input(AppMsg::ShowToast(msg));
                }
            },
            AppMsg::ShowConnectionDialog => {
                let client_id =
                    crate::services::plex::auth::client_identifier(&crate::config::data_dir());
                let dialog = ConnectionDialog::builder()
                    .transient_for(root)
                    .launch(client_id)
                    .forward(sender.input_sender(), |output| match output {
                        ConnectionDialogOutput::ConnectionSaved { url, token, name } => {
                            AppMsg::ConnectionSaved { url, token, name }
                        }
                        ConnectionDialogOutput::Cancelled => {
                            AppMsg::ShowToast("Connection cancelled".to_string())
                        }
                    });
                dialog.widget().present();
                self.connection_dialog = Some(dialog);
            }
            AppMsg::ConnectionSaved { url, token, name } => {
                info!("Plex connection saved: {} ({})", name, url);
                self.connection_dialog = None;

                // Save to database
                if let Some(ref conn) = self.db_conn {
                    let source = Source {
                        id: Source::make_id(&url),
                        source_type: SourceType::Plex,
                        name: name.clone(),
                        config: SourceConfig {
                            url: url.clone(),
                            token: token.clone(),
                        },
                        enabled: true,
                        last_synced_at: None,
                    };

                    let repo = crate::db::source_repo::SourceRepo::new(conn);
                    let _ = repo.delete(&source.id);
                    if let Err(e) = repo.insert(&source) {
                        tracing::warn!("Failed to save source: {e}");
                    }
                }

                let client = PlexClient::new(&url, &token);
                let source = Arc::new(PlexSource::new(client, name.clone()));
                let artwork_cache = Arc::new(ArtworkCache::new(crate::config::artwork_dir()));

                self.library_view.emit(LibraryViewMsg::SetSource(
                    source.clone(),
                    artwork_cache.clone(),
                ));
                self.movie_detail.emit(MovieDetailMsg::SetSource(
                    source.clone(),
                    artwork_cache.clone(),
                ));
                self.show_detail
                    .emit(ShowDetailMsg::SetSource(source, artwork_cache));

                if let CurrentView::Library(lt) = self.current_view {
                    self.library_view.emit(LibraryViewMsg::LoadLibrary(lt));
                } else {
                    self.library_view
                        .emit(LibraryViewMsg::LoadLibrary(LibraryType::Movie));
                }

                sender.input(AppMsg::ShowToast(format!("Connected to {name}")));
            }
            AppMsg::ShowToast(message) => {
                if !message.is_empty() {
                    let toast = adw::Toast::new(&message);
                    toast.set_timeout(3);
                    self.toast_overlay.add_toast(toast);
                }
            }
            AppMsg::ShowFileChooser => {
                show_file_chooser(root, sender.input_sender().clone());
            }
            AppMsg::FocusSearch => {
                self.library_view.emit(LibraryViewMsg::FocusSearch);
            }
            AppMsg::ShowCollections => {
                self.current_view = CurrentView::Collections;
                self.stack.set_visible_child_name("shell");
                root.set_fullscreened(false);
                root.set_title(Some("Reel"));
                self.library_view.emit(LibraryViewMsg::LoadCollections);
            }
            AppMsg::ShowCollectionDetail(item) => {
                self.current_view = CurrentView::CollectionDetail(item.id.clone());
                self.library_view.emit(LibraryViewMsg::LoadCollectionItems(
                    item.external_id.clone(),
                ));
                let page = adw::NavigationPage::builder()
                    .title(&item.title)
                    .child(self.library_view.widget())
                    .build();
                self.nav_view.push(&page);
            }
        }
    }

    fn update_cmd(
        &mut self,
        cmd: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match cmd {
            AppCmd::SourceValidated { url, token, name } => {
                info!("Plex source validated: {} (url={})", name, url);

                // Update saved URL in DB (clear old entries — URL may have changed)
                if let Some(ref conn) = self.db_conn {
                    let repo = crate::db::source_repo::SourceRepo::new(conn);
                    if let Ok(old_sources) = repo.list() {
                        for s in &old_sources {
                            let _ = repo.delete(&s.id);
                        }
                    }
                    let source = Source {
                        id: Source::make_id(&url),
                        source_type: SourceType::Plex,
                        name: name.clone(),
                        config: SourceConfig {
                            url: url.clone(),
                            token: token.clone(),
                        },
                        enabled: true,
                        last_synced_at: None,
                    };
                    if let Err(e) = repo.insert(&source) {
                        tracing::warn!("Failed to update source: {e}");
                    }
                }

                let client = PlexClient::new(&url, &token);
                let plex_source = Arc::new(PlexSource::new(client, name));
                let artwork_cache = Arc::new(ArtworkCache::new(crate::config::artwork_dir()));

                self.library_view.emit(LibraryViewMsg::SetSource(
                    plex_source.clone(),
                    artwork_cache.clone(),
                ));
                self.movie_detail.emit(MovieDetailMsg::SetSource(
                    plex_source.clone(),
                    artwork_cache.clone(),
                ));
                self.show_detail
                    .emit(ShowDetailMsg::SetSource(plex_source, artwork_cache));

                if let CurrentView::Library(lt) = self.current_view {
                    self.library_view.emit(LibraryViewMsg::LoadLibrary(lt));
                } else {
                    self.library_view
                        .emit(LibraryViewMsg::LoadLibrary(LibraryType::Movie));
                }
            }
            AppCmd::SourceValidationFailed(msg) => {
                tracing::warn!("Saved Plex source not reachable: {msg}");
                sender.input(AppMsg::ShowToast(format!("Plex server unreachable: {msg}")));
            }
        }
    }
}

fn dispatch_player_action_sender(sender: &relm4::Sender<AppMsg>, action: PlayerAction) {
    match action {
        PlayerAction::TogglePause => {
            let _ = sender.send(AppMsg::PlayerAction(VideoAreaMsg::TogglePause));
        }
        PlayerAction::SeekForward(s) => {
            let _ = sender.send(AppMsg::PlayerAction(VideoAreaMsg::SeekRelative(s)));
        }
        PlayerAction::SeekBackward(s) => {
            let _ = sender.send(AppMsg::PlayerAction(VideoAreaMsg::SeekRelative(-s)));
        }
        PlayerAction::VolumeUp(v) => {
            let _ = sender.send(AppMsg::PlayerAction(VideoAreaMsg::VolumeStep(v)));
        }
        PlayerAction::VolumeDown(v) => {
            let _ = sender.send(AppMsg::PlayerAction(VideoAreaMsg::VolumeStep(-v)));
        }
        PlayerAction::ToggleMute => {
            let _ = sender.send(AppMsg::PlayerAction(VideoAreaMsg::ToggleMute));
        }
        PlayerAction::SpeedUp => {
            let _ = sender.send(AppMsg::PlayerAction(VideoAreaMsg::SpeedUp));
        }
        PlayerAction::SpeedDown => {
            let _ = sender.send(AppMsg::PlayerAction(VideoAreaMsg::SpeedDown));
        }
        PlayerAction::SpeedReset => {
            let _ = sender.send(AppMsg::PlayerAction(VideoAreaMsg::SpeedReset));
        }
        PlayerAction::ToggleFullscreen => {
            let _ = sender.send(AppMsg::ToggleFullscreen);
        }
        PlayerAction::ExitFullscreen => {
            let _ = sender.send(AppMsg::ExitFullscreen);
        }
    }
}

fn init_database() -> Option<Connection> {
    let db_path = crate::config::db_path();

    if let Some(parent) = db_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        tracing::warn!("Failed to create database directory: {e}");
        return None;
    }

    match Connection::open(&db_path) {
        Ok(conn) => {
            if let Err(e) = db::init_db(&conn) {
                tracing::warn!("Failed to initialize database: {e}");
                return None;
            }
            info!("Database initialized at {}", db_path.display());
            Some(conn)
        }
        Err(e) => {
            tracing::warn!("Failed to open database: {e}");
            None
        }
    }
}

fn show_subtitle_chooser(window: &adw::ApplicationWindow, video_area: &Controller<VideoArea>) {
    let dialog = gtk4::FileDialog::builder()
        .title("Load Subtitle File")
        .modal(true)
        .build();

    let filter = gtk4::FileFilter::new();
    filter.set_name(Some("Subtitle Files"));
    for ext in &["srt", "ass", "ssa", "vtt", "sub", "idx"] {
        filter.add_suffix(ext);
    }

    let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&filter);
    dialog.set_filters(Some(&filters));

    let window_clone = window.clone();
    let video_sender = video_area.sender().clone();
    dialog.open(
        Some(&window_clone),
        gtk4::gio::Cancellable::NONE,
        move |result| {
            if let Ok(file) = result
                && let Some(path) = file.path()
            {
                let _ = video_sender.send(VideoAreaMsg::AddSubtitleFile(
                    path.to_string_lossy().to_string(),
                ));
            }
        },
    );
}

fn show_file_chooser(window: &adw::ApplicationWindow, sender: relm4::Sender<AppMsg>) {
    let dialog = gtk4::FileDialog::builder()
        .title("Open Video File")
        .modal(true)
        .build();

    let filter = gtk4::FileFilter::new();
    filter.set_name(Some("Video Files"));
    filter.add_mime_type("video/*");

    let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&filter);
    dialog.set_filters(Some(&filters));

    let window_clone = window.clone();
    dialog.open(
        Some(&window_clone),
        gtk4::gio::Cancellable::NONE,
        move |result| {
            if let Ok(file) = result
                && let Some(path) = file.path()
            {
                let _ = sender.send(AppMsg::OpenFile(path.to_string_lossy().to_string()));
            }
        },
    );
}

/// Test the saved URL; if unreachable, re-discover the server via plex.tv.
async fn validate_or_rediscover_source(
    url: String,
    token: String,
    name: String,
    data_dir: std::path::PathBuf,
) -> AppCmd {
    use crate::services::plex::auth;

    info!("Validating saved Plex connection: {url}");

    // Quick connectivity test on the saved URL
    let http = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();

    if http.get(format!("{url}/")).send().await.is_ok() {
        info!("Saved URL is reachable: {url}");
        return AppCmd::SourceValidated { url, token, name };
    }

    info!("Saved URL unreachable ({url}), re-discovering server...");

    let client_id = auth::client_identifier(&data_dir);
    let servers = match auth::discover_servers(&client_id, &token).await {
        Ok(s) => s,
        Err(e) => {
            return AppCmd::SourceValidationFailed(format!("Discovery failed: {e}"));
        }
    };

    // Find the server by name, or take the first one
    let server = servers.iter().find(|s| s.name == name).or(servers.first());

    let Some(server) = server else {
        return AppCmd::SourceValidationFailed("No servers found on account".to_string());
    };

    match auth::best_server_uri(server).await {
        Some(new_url) => {
            info!("Re-discovered server '{}' at {new_url}", server.name);
            AppCmd::SourceValidated {
                url: new_url,
                token,
                name: server.name.clone(),
            }
        }
        None => AppCmd::SourceValidationFailed(format!(
            "Server '{}' found but no connections reachable",
            server.name
        )),
    }
}
