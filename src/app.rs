use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

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
use crate::db::watch_progress_repo::WatchProgressRepo;
use crate::models::library::LibraryType;
use crate::models::media::{MediaItem, MediaType, SourceType};
use crate::models::source::{Source, SourceConfig};
use crate::models::watch::WatchProgress;
use crate::navigation::CurrentView;
use crate::player::backend::{self, PlayState};
use crate::services::artwork::ArtworkCache;
use crate::services::media_source::MediaSource;
use crate::services::plex::api::PlexClient;
use crate::services::plex::source::PlexSource;
use crate::services::screensaver::ScreensaverInhibitor;
use crate::services::watch_state::{PlaybackState, WatchStateEvent, WatchStateTracker};
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
    now_playing: Option<MediaItem>,
    watch_tracker: WatchStateTracker,
    /// Cached current position for save-on-exit.
    last_position: f64,
    /// Resume position to seek to after file loads. Set from saved watch progress.
    pending_resume: Option<f64>,
    /// Active media source for scrobble/timeline reporting.
    active_source: Option<Arc<PlexSource>>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum AppMsg {
    Navigate(LibraryType),
    ShowMovieDetail(crate::models::media::MediaItem),
    ShowShowDetail(crate::models::media::MediaItem),
    GoBack,
    VideoOutput(VideoAreaOutput),
    PlayMedia {
        url: String,
        media_item: Option<MediaItem>,
    },
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
    MarkWatched(MediaItem),
    MarkUnwatched(MediaItem),
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
    /// No-op for fire-and-forget async commands (scrobble, timeline).
    Noop,
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
                LibraryViewOutput::MarkWatched(item) => AppMsg::MarkWatched(item),
                LibraryViewOutput::MarkUnwatched(item) => AppMsg::MarkUnwatched(item),
                LibraryViewOutput::Error(msg) => AppMsg::ShowToast(msg),
            },
        );

        let movie_detail = MovieDetail::builder().launch(()).forward(
            sender.input_sender(),
            |output| match output {
                MovieDetailOutput::PlayMedia { url, media_item } => AppMsg::PlayMedia {
                    url,
                    media_item: *media_item,
                },
                MovieDetailOutput::Error(msg) => AppMsg::ShowToast(msg),
            },
        );

        let show_detail = ShowDetail::builder().launch(()).forward(
            sender.input_sender(),
            |output| match output {
                ShowDetailOutput::PlayMedia { url, media_item } => AppMsg::PlayMedia {
                    url,
                    media_item: *media_item,
                },
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
                ..window_state::load()
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
            now_playing: None,
            watch_tracker: WatchStateTracker::new(),
            last_position: 0.0,
            pending_resume: None,
            active_source: None,
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
                    // Stop watch tracking when leaving player
                    let events = self.watch_tracker.stop(self.last_position);
                    dispatch_watch_events(&self.db_conn, events, &self.active_source, &sender);
                    self.now_playing = None;
                    self.stack.set_visible_child_name("shell");
                    root.set_fullscreened(false);
                    root.set_title(Some("Reel"));
                } else {
                    self.nav_view.pop();
                }
            }
            AppMsg::PlayMedia { url, media_item } => {
                info!("Playing media: {}...", &url[..url.len().min(80)]);
                // Check for saved watch progress to auto-resume
                self.pending_resume = None;
                if let Some(ref item) = media_item
                    && let Some(ref conn) = self.db_conn
                {
                    let repo = WatchProgressRepo::new(conn);
                    if let Ok(Some(progress)) = repo.find_by_media_id(&item.id)
                        && progress.should_show_resume()
                    {
                        self.pending_resume = Some(progress.resume_position());
                    }
                }
                self.now_playing = media_item;
                self.last_position = 0.0;
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
                    // Auto-resume from saved position
                    if let Some(position) = self.pending_resume.take() {
                        let formatted = backend::format_position(position);
                        info!("Resuming at {formatted}");
                        self.video_area.emit(VideoAreaMsg::SeekAbsolute(position));
                        let toast = adw::Toast::new(&format!("Resumed at {formatted}"));
                        toast.set_timeout(3);
                        self.toast_overlay.add_toast(toast);
                    }
                    // Start watch state tracking
                    if let Some(ref item) = self.now_playing {
                        let rating_key = if item.source_type == SourceType::Plex {
                            Some(item.external_id.as_str())
                        } else {
                            None
                        };
                        let duration = item.runtime_minutes.map(|m| m as f64 * 60.0).unwrap_or(0.0);
                        self.watch_tracker
                            .start(&item.id, rating_key, duration, Instant::now());
                    }
                }
                VideoAreaOutput::PositionChanged { position, duration } => {
                    self.last_position = position;
                    // Update tracker duration from actual mpv value if we have it
                    if let Some(ref item) = self.now_playing
                        && !self.watch_tracker.is_active()
                    {
                        let rating_key = if item.source_type == SourceType::Plex {
                            Some(item.external_id.as_str())
                        } else {
                            None
                        };
                        self.watch_tracker
                            .start(&item.id, rating_key, duration, Instant::now());
                    }
                    let events = self
                        .watch_tracker
                        .process_position(position, Instant::now());
                    dispatch_watch_events(&self.db_conn, events, &self.active_source, &sender);
                }
                VideoAreaOutput::StateChanged(state) => {
                    if self.current_view == CurrentView::Player {
                        root.set_title(Some(backend::window_title_for_state(state)));
                    }
                    match state {
                        PlayState::Playing => {
                            self.screensaver.inhibit(root);
                            let events = self.watch_tracker.process_state_change(
                                PlaybackState::Playing,
                                self.last_position,
                                Instant::now(),
                            );
                            dispatch_watch_events(&self.db_conn, events, &self.active_source, &sender);
                        }
                        PlayState::Paused | PlayState::Stopped => {
                            self.screensaver.uninhibit(root);
                            let events = self.watch_tracker.process_state_change(
                                PlaybackState::Paused,
                                self.last_position,
                                Instant::now(),
                            );
                            dispatch_watch_events(&self.db_conn, events, &self.active_source, &sender);
                        }
                    }
                }
                VideoAreaOutput::EndOfFile(reason) => {
                    info!("Playback ended: {:?}", reason);
                    self.screensaver.uninhibit(root);
                    // Stop watch tracking with final persist + scrobble check
                    let events = self.watch_tracker.stop(self.last_position);
                    dispatch_watch_events(&self.db_conn, events, &self.active_source, &sender);
                    self.now_playing = None;
                    // Refresh watch data on library cards
                    let watch_data = load_watch_data(&self.db_conn);
                    self.library_view
                        .emit(LibraryViewMsg::SetWatchData(watch_data));
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

                self.active_source = Some(source.clone());

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

                // Send watch data to library view
                let watch_data = load_watch_data(&self.db_conn);
                self.library_view
                    .emit(LibraryViewMsg::SetWatchData(watch_data));

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
            AppMsg::MarkWatched(item) => {
                info!("Marking as watched: {}", item.title);
                if let Some(ref conn) = self.db_conn {
                    let repo = WatchProgressRepo::new(conn);
                    let progress = WatchProgress {
                        media_item_id: item.id.clone(),
                        position_seconds: 0.0,
                        duration_seconds: item
                            .runtime_minutes
                            .map(|m| m as f64 * 60.0)
                            .unwrap_or(0.0),
                        watched: true,
                        last_watched_at: iso_now(),
                    };
                    let _ = repo.upsert(&progress);
                }
                // Fire-and-forget Plex scrobble
                if item.source_type == SourceType::Plex {
                    if let Some(source) = self.active_source.clone() {
                        let key = item.external_id.clone();
                        sender.oneshot_command(async move {
                            if let Err(e) = source.scrobble(&key).await {
                                tracing::warn!("Plex scrobble failed: {e}");
                            }
                            AppCmd::Noop
                        });
                    }
                }
                // Refresh watch data
                let watch_data = load_watch_data(&self.db_conn);
                self.library_view
                    .emit(LibraryViewMsg::SetWatchData(watch_data));
                sender.input(AppMsg::ShowToast(format!(
                    "Marked \"{}\" as watched",
                    item.title
                )));
            }
            AppMsg::MarkUnwatched(item) => {
                info!("Marking as unwatched: {}", item.title);
                if let Some(ref conn) = self.db_conn {
                    let repo = WatchProgressRepo::new(conn);
                    let _ = repo.mark_unwatched(&item.id);
                }
                // Fire-and-forget Plex unscrobble
                if item.source_type == SourceType::Plex {
                    if let Some(source) = self.active_source.clone() {
                        let key = item.external_id.clone();
                        sender.oneshot_command(async move {
                            if let Err(e) = source.unscrobble(&key).await {
                                tracing::warn!("Plex unscrobble failed: {e}");
                            }
                            AppCmd::Noop
                        });
                    }
                }
                // Refresh watch data
                let watch_data = load_watch_data(&self.db_conn);
                self.library_view
                    .emit(LibraryViewMsg::SetWatchData(watch_data));
                sender.input(AppMsg::ShowToast(format!(
                    "Marked \"{}\" as unwatched",
                    item.title
                )));
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
                let source_start = Instant::now();
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

                self.active_source = Some(plex_source.clone());

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

                // Send watch data to library view
                let watch_data = load_watch_data(&self.db_conn);
                self.library_view
                    .emit(LibraryViewMsg::SetWatchData(watch_data));

                info!("Source setup took {:?} (DB save + watch data)", source_start.elapsed());

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
            AppCmd::Noop => {}
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

/// Dispatch watch state events to persistence and Plex API.
/// All operations are fire-and-forget to avoid blocking the UI.
fn dispatch_watch_events(
    db_conn: &Option<Connection>,
    events: Vec<WatchStateEvent>,
    source: &Option<Arc<PlexSource>>,
    sender: &ComponentSender<App>,
) {
    for event in events {
        match event {
            WatchStateEvent::PersistProgress {
                media_id,
                position,
                duration,
            } => {
                if let Some(conn) = db_conn {
                    let repo = WatchProgressRepo::new(conn);
                    let progress = WatchProgress {
                        media_item_id: media_id,
                        position_seconds: position,
                        duration_seconds: duration,
                        watched: false,
                        last_watched_at: iso_now(),
                    };
                    if let Err(e) = repo.upsert(&progress) {
                        tracing::warn!("Failed to persist watch progress: {e}");
                    }
                }
            }
            WatchStateEvent::Scrobble {
                media_id,
                rating_key,
            } => {
                // Mark as watched locally
                if let Some(conn) = db_conn {
                    let repo = WatchProgressRepo::new(conn);
                    let timestamp = iso_now();
                    if let Err(e) = repo.mark_watched(&media_id, &timestamp) {
                        tracing::warn!("Failed to mark as watched: {e}");
                    }
                }
                // Fire-and-forget Plex scrobble
                if !rating_key.is_empty() {
                    if let Some(source) = source.clone() {
                        tracing::info!("Scrobble: rating_key={rating_key}");
                        sender.oneshot_command(async move {
                            if let Err(e) = source.scrobble(&rating_key).await {
                                tracing::warn!("Plex scrobble failed: {e}");
                            }
                            AppCmd::Noop
                        });
                    }
                }
            }
            WatchStateEvent::ReportTimeline {
                rating_key,
                state,
                time_ms,
                duration_ms,
            } => {
                if !rating_key.is_empty() {
                    if let Some(source) = source.clone() {
                        tracing::debug!(
                            "Timeline: key={rating_key} state={state} time={time_ms}ms"
                        );
                        sender.oneshot_command(async move {
                            if let Err(e) = source
                                .report_progress(&rating_key, &state, time_ms, duration_ms)
                                .await
                            {
                                tracing::warn!("Plex timeline report failed: {e}");
                            }
                            AppCmd::Noop
                        });
                    }
                }
            }
        }
    }
}

/// Generate a UTC ISO 8601 timestamp string.
fn iso_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple ISO format sufficient for sort ordering
    let secs_per_day = 86400;
    let days_since_epoch = now / secs_per_day;
    let time_of_day = now % secs_per_day;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    // Approximate date calculation (sufficient for timestamping)
    let (year, month, day) = days_to_ymd(days_since_epoch);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Load all watch progress from DB into a HashMap for the library view.
fn load_watch_data(db_conn: &Option<Connection>) -> HashMap<String, (f64, bool)> {
    let start = Instant::now();
    let mut map = HashMap::new();
    if let Some(conn) = db_conn {
        let repo = WatchProgressRepo::new(conn);
        // Load all in-progress items
        if let Ok(items) = repo.list_in_progress(1000) {
            for item in &items {
                map.insert(
                    item.media_item_id.clone(),
                    (item.progress_fraction(), false),
                );
            }
        }
        // Also query watched items (where watched = 1)
        let mut stmt = conn
            .prepare(
                "SELECT media_item_id, position_seconds, duration_seconds FROM watch_progress WHERE watched = 1",
            )
            .ok();
        if let Some(ref mut stmt) = stmt
            && let Ok(rows) = stmt.query_map([], |row| {
                let id: String = row.get(0)?;
                Ok(id)
            })
        {
            for row in rows.flatten() {
                map.insert(row, (1.0, true));
            }
        }
    }
    info!("load_watch_data: {} entries in {:?}", map.len(), start.elapsed());
    map
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
