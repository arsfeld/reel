use std::sync::Arc;
use std::time::Instant;

use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;
use rusqlite::Connection;
use tracing::info;

use crate::components::connection::{ConnectionDialog, ConnectionDialogOutput};
use crate::components::detail::movie_detail::{MovieDetail, MovieDetailMsg, MovieDetailOutput};
use crate::components::detail::show_detail::{ShowDetail, ShowDetailMsg, ShowDetailOutput};
use crate::components::home::{HomeView, HomeViewMsg, HomeViewOutput};
use crate::components::library::{LibraryView, LibraryViewMsg, LibraryViewOutput};

use crate::components::player::video_player::{
    VideoPlayer, VideoPlayerInit, VideoPlayerMsg, VideoPlayerOutput,
};
use crate::components::settings_dialog;
use crate::components::sidebar::{Sidebar, SidebarMsg, SidebarOutput};
use crate::db::watch_progress_repo::WatchProgressRepo;
use crate::models::library::LibrarySection;
use crate::models::media::{MediaItem, MediaType, SourceType};
use crate::models::source::{Source, SourceConfig};
use crate::models::watch::WatchProgress;
use crate::navigation::CurrentView;
use crate::player::SkipMarkers;

use crate::services::artwork::ArtworkCache;
use crate::services::media_source::MediaSource;
use crate::services::mpris::{self, MprisBridge, MprisCommand};
use crate::services::plex::api::PlexClient;
use crate::services::plex::source::PlexSource;
use crate::services::screensaver::ScreensaverInhibitor;
use crate::services::watch_state::WatchStateTracker;
use crate::settings::Settings;

mod db_helpers;
mod dialogs;
mod handlers;
mod player_ui;
mod source_validation;
mod utils;
mod watch_events;
mod widget_builder;

use db_helpers::{init_database, load_in_progress, load_watch_data};
use dialogs::show_file_chooser;
use handlers::{handle_connection_saved, handle_play_media, handle_video_output};
use player_ui::{enter_player_mode, leave_player_mode, player_title_for_item};
use source_validation::validate_or_rediscover_source;
use utils::iso_now;
use watch_events::dispatch_watch_events;
use widget_builder::build_widgets;

#[allow(dead_code)]
pub struct App {
    home_view: Controller<HomeView>,
    video_player: Controller<VideoPlayer>,
    sidebar: Controller<Sidebar>,
    library_view: Controller<LibraryView>,
    movie_detail: Controller<MovieDetail>,
    show_detail: Controller<ShowDetail>,
    connection_dialog: Option<Controller<ConnectionDialog>>,
    screensaver: ScreensaverInhibitor,
    toast_overlay: adw::ToastOverlay,
    stack: gtk::Stack,
    nav_view: adw::NavigationView,
    split_view: adw::NavigationSplitView,
    /// Library header title label — updated when sidebar navigation changes.
    library_title: gtk::Label,
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
    /// Base URL of the active source, used as the `source_id` when building
    /// per-library visibility keys.
    source_url: Option<String>,
    /// True while async startup validation of a saved source is in flight.
    source_connecting: bool,
    /// Application settings.
    settings: Settings,
    /// MPRIS D-Bus bridge channels.
    mpris: MprisBridge,
    /// Windowed player chrome (back + title) overlaid on the video page.
    player_chrome_revealer: gtk::Revealer,
    player_window_title: adw::WindowTitle,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum AppMsg {
    NavigateHome,
    Navigate(LibrarySection),
    /// Persist a library/Collections visibility change (composite key + visible).
    SetLibraryVisible {
        key: String,
        visible: bool,
    },
    /// The sidebar left edit mode; re-evaluate the current view and refresh Home.
    SidebarEditModeExited,
    ShowMovieDetail(crate::models::media::MediaItem),
    ShowShowDetail(crate::models::media::MediaItem),
    GoBack,
    VideoOutput(VideoPlayerOutput),
    PlayMedia {
        url: String,
        media_item: Option<MediaItem>,
    },
    PlayerAction(VideoPlayerMsg),
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
    SaveLibraryUiState {
        library_id: String,
        state: crate::settings::LibraryUiState,
    },
    OpenPreferences,
    OpenAbout,
    MprisInput(MprisCommand),
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
    /// The active source's libraries, fetched after validation, for the sidebar.
    LibrariesLoaded(Vec<LibrarySection>),
    /// No-op for fire-and-forget async commands (scrobble, timeline).
    Noop,
    /// Skip markers fetched from the media source after playback starts.
    SkipMarkersLoaded(SkipMarkers),
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
        let settings = Settings::load();
        let video_player = VideoPlayer::builder()
            .launch(VideoPlayerInit {
                preferred_subtitle_lang: settings.subtitles.preferred_language.clone(),
                ..VideoPlayerInit::default()
            })
            .forward(sender.input_sender(), AppMsg::VideoOutput);

        let sidebar = Sidebar::builder()
            .launch(())
            .forward(sender.input_sender(), |output| match output {
                SidebarOutput::NavigateHome => AppMsg::NavigateHome,
                SidebarOutput::Navigate(section) => AppMsg::Navigate(section),
                SidebarOutput::ShowCollections => AppMsg::ShowCollections,
                SidebarOutput::SetLibraryVisible { key, visible } => {
                    AppMsg::SetLibraryVisible { key, visible }
                }
                SidebarOutput::EditModeExited => AppMsg::SidebarEditModeExited,
            });

        let home_view = HomeView::builder()
            .launch(())
            .forward(sender.input_sender(), |output| match output {
                HomeViewOutput::ShowDetail(item) => match item.media_type {
                    MediaType::Movie => AppMsg::ShowMovieDetail(item),
                    MediaType::Show => AppMsg::ShowShowDetail(item),
                    MediaType::Collection => AppMsg::ShowCollectionDetail(item),
                    _ => AppMsg::ShowToast("Unsupported media type".to_string()),
                },
                HomeViewOutput::PlayMedia { url, media_item } => AppMsg::PlayMedia {
                    url,
                    media_item: Some(media_item),
                },
                HomeViewOutput::ShowConnectionDialog => AppMsg::ShowConnectionDialog,
                HomeViewOutput::Error(msg) => AppMsg::ShowToast(msg),
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
                LibraryViewOutput::SaveLibraryUiState { library_id, state } => {
                    AppMsg::SaveLibraryUiState { library_id, state }
                }
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

        let built = build_widgets(
            &root,
            &sender,
            &sidebar,
            &home_view,
            &library_view,
            &video_player,
        );
        let toast_overlay = built.toast_overlay;
        let stack = built.stack;
        let nav_view = built.nav_view;
        let split_view = built.split_view;
        let library_title = built.library_title;
        let player_chrome_revealer = built.player_chrome_revealer;
        let player_window_title = built.player_window_title;

        // Initialize database
        let db_conn = init_database();

        // Load and validate saved source (async — tests connection, re-discovers if stale)
        let mut has_sources = false;
        if let Some(ref conn) = db_conn {
            let repo = crate::db::source_repo::SourceRepo::new(conn);
            if let Ok(sources) = repo.list()
                && let Some(source) = sources.into_iter().find(|s| s.enabled)
            {
                has_sources = true;
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
            home_view,
            video_player,
            sidebar,
            library_view,
            movie_detail,
            show_detail,
            connection_dialog: None,
            screensaver: ScreensaverInhibitor::new(),
            toast_overlay,
            stack,
            nav_view,
            split_view,
            library_title,
            current_view: CurrentView::default(),
            db_conn,
            now_playing: None,
            source_url: None,
            watch_tracker: WatchStateTracker::new(),
            last_position: 0.0,
            pending_resume: None,
            active_source: None,
            source_connecting: false,
            settings,
            mpris: mpris::spawn_mpris_server(),
            player_chrome_revealer,
            player_window_title,
        };

        // Show a loading page on the home view while async validation runs.
        if has_sources {
            model.source_connecting = true;
            model.home_view.emit(HomeViewMsg::SetConnecting(true));
        }

        // Relay MPRIS commands from tokio to the GTK main loop
        let sender_mpris = sender.input_sender().clone();
        if let Some(mut command_rx) = model.mpris.command_rx.take() {
            glib::spawn_future_local(async move {
                while let Some(cmd) = command_rx.recv().await {
                    let _ = sender_mpris.send(AppMsg::MprisInput(cmd));
                }
            });
        }

        // Handle CLI file arg or start with library
        let sender_init = sender.clone();
        let has_file_arg = file_arg.is_some();
        glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
            if let Some(path) = file_arg {
                sender_init.input(AppMsg::OpenFile(path));
            } else if !has_sources {
                // First run: auto-prompt Plex connection
                sender_init.input(AppMsg::ShowConnectionDialog);
            } else {
                sender_init.input(AppMsg::NavigateHome);
            }
        });

        let _ = has_file_arg;
        let _ = &mut model;

        ComponentParts { model, widgets }
    }

    #[allow(clippy::too_many_lines)]
    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            AppMsg::NavigateHome => {
                self.current_view = CurrentView::Home;
                self.stack.set_visible_child_name("shell");
                self.nav_view.replace_with_tags(&["home"]);
                root.set_fullscreened(false);
                root.set_title(Some("Reel"));
                // Load home data: in-progress from local DB + recently_added from source
                self.home_view.emit(HomeViewMsg::SetVisibility(
                    self.settings.library_visibility.hidden.clone(),
                ));
                let in_progress = load_in_progress(&self.db_conn);
                self.home_view.emit(HomeViewMsg::LoadHome { in_progress });
            }
            AppMsg::Navigate(section) => {
                self.current_view = CurrentView::Library(section.key.clone());
                self.stack.set_visible_child_name("shell");
                self.nav_view.replace_with_tags(&["library"]);
                root.set_fullscreened(false);
                root.set_title(Some("Reel"));
                self.library_title.set_label(&section.title);
                self.library_view.emit(LibraryViewMsg::LoadLibrary(section));
            }
            AppMsg::SetLibraryVisible { key, visible } => {
                // `key` is the composite visibility key built by the sidebar.
                if visible {
                    self.settings.library_visibility.hidden.remove(&key);
                } else {
                    self.settings.library_visibility.hidden.insert(key);
                }
                if let Err(e) = self.settings.save() {
                    tracing::warn!("Failed to persist library visibility: {e}");
                }
            }
            AppMsg::SidebarEditModeExited => {
                let hidden = self.settings.library_visibility.hidden.clone();
                self.home_view
                    .emit(HomeViewMsg::SetVisibility(hidden.clone()));
                // If the library being viewed was just hidden, drop to Home.
                let redirect = match (&self.current_view, &self.source_url) {
                    (CurrentView::Library(key), Some(url)) => {
                        hidden.contains(&LibrarySection::visibility_key_for("plex", url, key))
                    }
                    _ => false,
                };
                if redirect {
                    sender.input(AppMsg::NavigateHome);
                } else if matches!(self.current_view, CurrentView::Home) {
                    // Refresh Home in place so it reflects the new visibility.
                    let in_progress = load_in_progress(&self.db_conn);
                    self.home_view.emit(HomeViewMsg::LoadHome { in_progress });
                }
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
                    self.video_player.emit(VideoPlayerMsg::Clear);
                    leave_player_mode(root, &mut self.player_chrome_revealer);
                    self.stack.set_visible_child_name("shell");
                    root.set_fullscreened(false);
                } else {
                    self.nav_view.pop();
                }
            }
            AppMsg::PlayMedia { url, media_item } => {
                handle_play_media(self, url, media_item, &sender, root);
            }
            AppMsg::PlayerAction(video_msg) => {
                self.video_player.emit(video_msg);
            }
            AppMsg::OpenFile(path) => {
                info!("Opening file: {}", path);
                self.current_view = CurrentView::Player;
                let title = player_title_for_item(None, &path);
                enter_player_mode(
                    root,
                    &mut self.player_chrome_revealer,
                    &self.player_window_title,
                    &title,
                );
                self.video_player
                    .emit(VideoPlayerMsg::SetTitle(Some(title)));
                self.stack.set_visible_child_name("player");
                self.video_player.emit(VideoPlayerMsg::SetAutoplay(true));
                self.video_player.emit(VideoPlayerMsg::LoadFile(path));
            }
            AppMsg::ToggleFullscreen => {
                self.video_player.emit(VideoPlayerMsg::ToggleFullscreen);
            }
            AppMsg::ExitFullscreen => {
                self.video_player.emit(VideoPlayerMsg::ExitFullscreen);
            }
            AppMsg::FilesDropped(uri) => {
                self.current_view = CurrentView::Player;
                if self.stack.visible_child_name().as_deref() != Some("player") {
                    let title = player_title_for_item(self.now_playing.as_ref(), &uri);
                    enter_player_mode(
                        root,
                        &mut self.player_chrome_revealer,
                        &self.player_window_title,
                        &title,
                    );
                    self.video_player
                        .emit(VideoPlayerMsg::SetTitle(Some(title)));
                }
                self.stack.set_visible_child_name("player");
                self.video_player.emit(VideoPlayerMsg::FilesDropped(uri));
            }
            AppMsg::VideoOutput(output) => {
                handle_video_output(self, output, &sender, root);
            }
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
                handle_connection_saved(self, url, token, name, &sender);
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
                self.nav_view.replace_with_tags(&["library"]);
                root.set_fullscreened(false);
                root.set_title(Some("Reel"));
                self.library_title.set_label("Collections");
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
                if item.source_type == SourceType::Plex
                    && let Some(source) = self.active_source.clone()
                {
                    let key = item.external_id.clone();
                    sender.oneshot_command(async move {
                        if let Err(e) = source.scrobble(&key).await {
                            tracing::warn!("Plex scrobble failed: {e}");
                        }
                        AppCmd::Noop
                    });
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
                if item.source_type == SourceType::Plex
                    && let Some(source) = self.active_source.clone()
                {
                    let key = item.external_id.clone();
                    sender.oneshot_command(async move {
                        if let Err(e) = source.unscrobble(&key).await {
                            tracing::warn!("Plex unscrobble failed: {e}");
                        }
                        AppCmd::Noop
                    });
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
            AppMsg::SaveLibraryUiState { library_id, state } => {
                self.settings.library.set(&library_id, state);
                if let Err(e) = self.settings.save() {
                    tracing::warn!("Failed to persist library filter/sort state: {e}");
                }
            }
            AppMsg::OpenPreferences => {
                self.settings = settings_dialog::show_preferences(root, &self.settings);
            }
            AppMsg::OpenAbout => {
                settings_dialog::show_about(root);
            }
            AppMsg::MprisInput(cmd) => match cmd {
                MprisCommand::Play | MprisCommand::PlayPause | MprisCommand::Pause => {
                    self.video_player.emit(VideoPlayerMsg::TogglePlay);
                }
                MprisCommand::Stop => {
                    sender.input(AppMsg::GoBack);
                }
                MprisCommand::Seek(offset_us) => {
                    let offset_secs = mpris::micros_to_seconds(offset_us) as i64;
                    self.video_player
                        .emit(VideoPlayerMsg::SeekRelative(offset_secs));
                }
                MprisCommand::SetPosition(_pos_us) => {
                    // The VideoPlayer doesn't support absolute seek by position yet.
                    // Skipped: would need SeekFraction with known duration.
                }
                MprisCommand::SetVolume(vol) => {
                    self.video_player.emit(VideoPlayerMsg::SetVolume(vol));
                }
                MprisCommand::OpenUri(uri) => {
                    sender.input(AppMsg::OpenFile(uri));
                }
                MprisCommand::Raise => {
                    root.present();
                }
                MprisCommand::Quit => {
                    root.close();
                }
            },
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

                self.source_connecting = false;

                // Update saved URL in DB (clear old entries — URL may have changed)
                if let Some(ref conn) = self.db_conn {
                    let repo = crate::db::source_repo::SourceRepo::new(conn);
                    if let Ok(old_sources) = repo.list() {
                        for s in &old_sources {
                            let _ = repo.delete(&s.id);
                        }
                    }
                    let source = Source {
                        id: Source::make_id(SourceType::Plex, &url),
                        source_type: SourceType::Plex,
                        name: name.clone(),
                        config: SourceConfig {
                            url: url.clone(),
                            token: token.clone(),
                            user_id: None,
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
                self.source_url = Some(url.clone());

                // Feed the sidebar tree: source identity, current visibility, and
                // (async) the source's libraries.
                self.sidebar.emit(SidebarMsg::SetSource {
                    name: plex_source.name().to_string(),
                    source_type: "plex".to_string(),
                    source_id: url.clone(),
                });
                self.sidebar.emit(SidebarMsg::SetVisibility(
                    self.settings.library_visibility.hidden.clone(),
                ));
                {
                    // The PlexClient absorbs cold-start connection retries, so a
                    // single fetch here is enough once the source is validated.
                    let src = plex_source.clone();
                    sender.oneshot_command(async move {
                        AppCmd::LibrariesLoaded(src.libraries().await.unwrap_or_default())
                    });
                }

                self.home_view.emit(HomeViewMsg::SetSource(
                    plex_source.clone(),
                    artwork_cache.clone(),
                ));
                self.home_view.emit(HomeViewMsg::SetVisibility(
                    self.settings.library_visibility.hidden.clone(),
                ));
                // Re-trigger home data load now that the source is available.
                // NavigateHome fires before validation completes, so LoadHome
                // was skipped — retry it here.
                let in_progress = load_in_progress(&self.db_conn);
                self.home_view.emit(HomeViewMsg::LoadHome { in_progress });
                self.library_view.emit(LibraryViewMsg::SetSource(
                    plex_source.clone(),
                    artwork_cache.clone(),
                ));
                self.library_view.emit(LibraryViewMsg::SetSavedUiState(
                    self.settings.library.clone(),
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

                info!(
                    "Source setup took {:?} (DB save + watch data)",
                    source_start.elapsed()
                );

                // Switch home view from connecting page to shelves.
                self.home_view.emit(HomeViewMsg::SetConnecting(false));
                // A specific library loads when the user picks it from the
                // sidebar; the default view is Home, so no eager load here.
            }
            AppCmd::LibrariesLoaded(libraries) => {
                self.sidebar.emit(SidebarMsg::SetLibraries(libraries));
            }
            AppCmd::SourceValidationFailed(msg) => {
                tracing::warn!("Saved Plex source not reachable: {msg}");
                self.source_connecting = false;
                self.home_view.emit(HomeViewMsg::SetConnecting(false));
                sender.input(AppMsg::ShowToast(format!("Plex server unreachable: {msg}")));
            }
            AppCmd::SkipMarkersLoaded(markers) => {
                self.video_player
                    .emit(VideoPlayerMsg::SetSkipMarkers(markers));
            }
            AppCmd::Noop => {}
        }
    }
}
