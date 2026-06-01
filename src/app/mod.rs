use std::sync::Arc;

use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;
use tracing::info;

use crate::components::connection::{ConnectionDialog, ConnectionDialogOutput};
use crate::components::detail::movie_detail::{MovieDetail, MovieDetailMsg, MovieDetailOutput};
use crate::components::detail::show_detail::{ShowDetail, ShowDetailMsg, ShowDetailOutput};
use crate::components::downloads::{
    DeleteMode, DownloadsConfig, DownloadsScope, DownloadsView, DownloadsViewMsg,
    DownloadsViewOutput,
};
use crate::components::home::{HomeView, HomeViewMsg, HomeViewOutput};
use crate::components::library::{LibraryView, LibraryViewMsg, LibraryViewOutput};

use crate::components::player::video_player::{
    VideoPlayer, VideoPlayerInit, VideoPlayerMsg, VideoPlayerOutput,
};
use crate::components::settings_dialog;
use crate::components::sidebar::{Sidebar, SidebarMsg, SidebarOutput};
use crate::db::database::Database;
use crate::db::watch_progress_repo::WatchProgressRepo;
use crate::models::library::LibrarySection;
use crate::models::media::{MediaItem, MediaType, SourceType};
use crate::models::source::Source;
use crate::navigation::CurrentView;
use crate::player::SkipMarkers;

use crate::services::artwork::ArtworkCache;
use crate::services::media_source::MediaSource;
use crate::services::mpris::{self, MprisBridge, MprisCommand};
use crate::services::screensaver::ScreensaverInhibitor;
use crate::services::session_cache::{
    CachedHome, SessionContentCache, SourceSetEvent, invalidation_for,
};
use crate::services::watch_state::WatchStateTracker;
use crate::settings::Settings;

mod db_helpers;
mod dialogs;
mod download_handlers;
mod handlers;
mod player_ui;
mod source_factory;
mod source_validation;
mod transcode_keepalive;
mod utils;
mod watch_events;
mod widget_builder;

use db_helpers::{load_in_progress, load_watch_data, reclaim_stream_cache};
use dialogs::show_file_chooser;
use download_handlers::{DownloadItemAction, DownloadManager, DownloadRunnerMsg};
use handlers::{
    handle_connection_saved, handle_play_media, handle_playback_resolve_failed,
    handle_playback_resolved, handle_video_output,
};
use player_ui::{enter_player_mode, leave_player_mode, player_title_for_item};
use source_factory::SourceRegistry;
use source_validation::validate_source;
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
    downloads_view: Controller<DownloadsView>,
    /// The per-show download drill-in, created on demand when a show card is
    /// tapped and dropped when its page is popped or its group empties.
    show_drilldown: Option<Controller<DownloadsView>>,
    /// The group id the open drill-in is scoped to (for live refresh + auto-pop).
    drilldown_group: Option<String>,
    /// Downloads whose delete is pending an undo-toast window: captured rows
    /// keyed by `media_item_id`, restored on undo or removed on finalize.
    pending_deletes: std::collections::HashMap<String, crate::models::download::Download>,
    connection_dialog: Option<Controller<ConnectionDialog>>,
    screensaver: ScreensaverInhibitor,
    toast_overlay: adw::ToastOverlay,
    stack: gtk::Stack,
    nav_view: adw::NavigationView,
    split_view: adw::NavigationSplitView,
    /// Library header title label — updated when sidebar navigation changes.
    library_title: gtk::Label,
    current_view: CurrentView,
    db: Option<Database>,
    now_playing: Option<MediaItem>,
    watch_tracker: WatchStateTracker,
    /// Cached current position for save-on-exit.
    last_position: f64,
    /// Resume position to seek to after file loads. Set from saved watch progress.
    pending_resume: Option<f64>,
    /// Live source registry, keyed by `{source_type}:{source_id}` in insertion
    /// order. Detail/play/scrobble/skip resolve the owning source per item;
    /// Home fans out across all of them.
    sources: SourceRegistry,
    /// The browsed source `(source_type, source_id)`: feeds the LibraryView
    /// *list* and the per-library visibility key. Detail/play never use this —
    /// they resolve the owning source from the item itself.
    browsed_source: Option<(SourceType, String)>,
    /// Shared artwork cache handed to every view; built once.
    artwork_cache: Arc<ArtworkCache>,
    /// True while async startup validation of a saved source is in flight.
    source_connecting: bool,
    /// Application settings.
    settings: Settings,
    /// MPRIS D-Bus bridge channels.
    mpris: MprisBridge,
    /// Windowed player chrome (back + title) overlaid on the video page.
    player_chrome_revealer: gtk::Revealer,
    player_window_title: adw::WindowTitle,
    /// Offline-download manager: pure queue + in-flight transfer tasks.
    downloads: DownloadManager,

    // --- Session-only playback-quality state (R11; never persisted) ---
    /// The current title's quality selection. Reset to Auto on each new title.
    current_quality: crate::models::playback::QualitySelection,
    /// The active transcode session id, if the current decision transcodes.
    active_transcode_session: Option<String>,
    /// Content-time offset the current transcode stream was built at. The player
    /// adds this to playbin3's 0-based position for display/seek/scrobble (U1).
    transcode_base_offset: f64,
    /// The current resolved decision, for the indicator (R16) and switching.
    current_decision: Option<crate::models::playback::PlaybackDecision>,
    /// Epoch guard for overlapping quality/seek switches (U8): a resolve result
    /// from a superseded switch is discarded and its session stopped.
    switch_state: crate::components::player::switch_state::SwitchState,
    /// Per-item render-failure fallback state (U4): once a direct-play stream
    /// fails to render, the item sticks to server transcode for the session and
    /// retries are bounded so a failing transcode can't loop.
    render_fallback: crate::components::player::switch_state::RenderFallback,
    /// Fixed-interval keepalive timer for the active transcode session (U10/R15);
    /// `None` when no transcode is active. Pings independent of playback so a
    /// paused transcode is not reaped.
    keepalive_source: Option<glib::SourceId>,
    /// Consecutive keepalive ping failures; a session is declared lost past a
    /// threshold (U10).
    keepalive_failures: u32,
    /// Selected Plex audio/subtitle stream ids for the current title (AE6),
    /// carried into every re-decision so a quality switch preserves the chosen
    /// tracks. Synced from the server-selected streams on each resolve. Reset
    /// per title.
    current_audio_stream_id: Option<i64>,
    current_subtitle_stream_id: Option<i64>,
    /// Playback capabilities (probed once at startup).
    playback_capabilities: crate::player::capabilities::PlaybackCapabilities,
    /// Session-scoped content cache for library + Home views, so navigating back
    /// to a previously-opened view renders instantly instead of re-fetching.
    content_cache: SessionContentCache,
    /// Monotonic counter for local watch-state mutations (mark watched, playback
    /// progress). Background revalidation captures this at dispatch and uses it
    /// to protect a racing local change from a stale refetch (KTD-4).
    content_mutation_seq: u64,
    /// Per-item sequence at its last local watch mutation.
    content_last_mutation: std::collections::HashMap<String, u64>,
    /// Library cache keys with a background revalidation refetch in flight, so a
    /// second revisit doesn't dispatch a duplicate (coalescing).
    revalidating: std::collections::HashSet<String>,
    /// True while a background Home revalidation refetch is in flight.
    home_revalidating: bool,
    /// Source-set epoch captured when the in-flight Home revalidation was
    /// dispatched; a mismatch on return means the source set changed and the
    /// result is dropped.
    home_revalidate_epoch: u64,
}

impl App {
    /// The source that owns the now-playing item, resolved per-item from the
    /// registry — never the browsed pointer. `None` if nothing is playing or
    /// the item's source isn't registered.
    fn now_playing_source(&self) -> Option<Arc<dyn MediaSource>> {
        self.now_playing
            .as_ref()
            .and_then(|item| self.sources.for_item(item))
    }

    /// Open the per-show download drill-in: a `DownloadsView` scoped to one
    /// group's episodes, pushed as a navigation page. Built fresh on each open
    /// and dropped on pop / when the group empties (`maybe_pop_empty_drilldown`).
    /// Show identity (page title) comes from the group row, never the episode
    /// `parent_id` (which is the season, not the show).
    fn open_show_downloads(&mut self, group_id: &str, sender: &ComponentSender<Self>) {
        let title = self
            .db
            .as_ref()
            .and_then(|db| {
                db.with(|conn| {
                    crate::db::downloads_repo::DownloadsRepo::new(conn)
                        .find_group(group_id)
                        .ok()
                        .flatten()
                })
            })
            .map(|g| g.title)
            .unwrap_or_else(|| "Downloaded Episodes".to_string());

        let controller = DownloadsView::builder()
            .launch(DownloadsConfig {
                scope: DownloadsScope::Group(group_id.to_string()),
                delete_mode: DeleteMode::Undo,
            })
            .forward(sender.input_sender(), |output| match output {
                DownloadsViewOutput::ItemAction {
                    media_item_id,
                    action,
                } => AppMsg::DownloadAction {
                    media_item_id,
                    action,
                },
                DownloadsViewOutput::RequestDelete {
                    media_item_id,
                    title,
                } => AppMsg::RequestDeleteDownload {
                    media_item_id,
                    title,
                },
                DownloadsViewOutput::PlayDownload(id) => AppMsg::PlayDownload(id),
                // A drill-in renders no group cards, so these never fire; map
                // them to their normal handlers rather than panicking.
                DownloadsViewOutput::OpenShowDownloads(gid) => AppMsg::OpenShowDownloads(gid),
                DownloadsViewOutput::OpenDetail(id) => AppMsg::OpenDownloadDetail(id),
            });

        let src = match &self.browsed_source {
            Some((t, id)) => self.sources.get(*t, id),
            None => None,
        };
        if let Some(src) = src {
            controller.emit(DownloadsViewMsg::SetSource(src, self.artwork_cache.clone()));
        }

        let page = adw::NavigationPage::builder()
            .title(&title)
            .tag("drilldown")
            .child(controller.widget())
            .build();
        self.nav_view.push(&page);

        self.show_drilldown = Some(controller);
        self.drilldown_group = Some(group_id.to_string());
        // Populate the freshly-built drill-in with the current snapshot.
        self.refresh_downloads_view();
    }

    /// The full set of connected sources as `(source_type, source_id, source)`,
    /// for Home's cross-source merged Continue Watching row — Home resolves each
    /// merged item's owning source by matching type + id.
    fn home_sources(&self) -> Vec<(SourceType, String, Arc<dyn MediaSource>)> {
        self.sources
            .iter()
            .map(|entry| {
                (
                    entry.source_type,
                    entry.source_id.clone(),
                    entry.source.clone(),
                )
            })
            .collect()
    }

    /// Session-cache key for the current Home, derived from the connected source
    /// set. Changes whenever a source is added or removed, so the cached Home is
    /// a natural miss after the set changes.
    fn home_source_set_key(&self) -> String {
        let sources: Vec<(SourceType, String)> = self
            .sources
            .iter()
            .map(|e| (e.source_type, e.source_id.clone()))
            .collect();
        SessionContentCache::source_set_key(&sources)
    }

    /// Record a local watch-state change (R8): stamp it with the next mutation
    /// sequence and patch every cached entry so an immediate revisit shows it,
    /// without waiting on background revalidation. The sequence lets a later
    /// revalidation tell a racing local change from a stale server read (KTD-4).
    fn note_local_watch_mutation(&mut self, id: &str, watched: bool, position_ms: Option<i64>) {
        self.content_mutation_seq += 1;
        self.content_last_mutation
            .insert(id.to_string(), self.content_mutation_seq);
        self.content_cache
            .patch_watch_state(id, watched, position_ms);
    }

    /// Dispatch a background revalidation refetch for a cached library after an
    /// instant render (U5). Coalesces duplicates per key, and stamps the request
    /// with the entry epoch, source-set epoch, and mutation sequence so a result
    /// superseded by eviction, a source-set change, or a racing local mutation is
    /// dropped or merged correctly on return.
    fn revalidate_library(
        &mut self,
        cache_key: String,
        section_key: String,
        source: Arc<dyn MediaSource>,
        sender: &ComponentSender<Self>,
    ) {
        if !self.revalidating.insert(cache_key.clone()) {
            return; // a refetch for this key is already in flight
        }
        let entry_epoch = self.content_cache.library_epoch(&cache_key).unwrap_or(0);
        let source_set_epoch = self.content_cache.source_set_epoch();
        let dispatch_seq = self.content_mutation_seq;
        sender.oneshot_command(async move {
            let result = source
                .library_items(&section_key)
                .await
                .map_err(|e| e.to_string());
            AppCmd::LibraryRevalidated {
                cache_key,
                entry_epoch,
                source_set_epoch,
                dispatch_seq,
                result,
            }
        });
    }

    /// Register a freshly-built source and add it to the sidebar as its own
    /// group: register in the live registry (idempotent), push its identity +
    /// visibility into the sidebar, and spawn an async libraries fetch. Called
    /// for EVERY validated source so all servers appear in the sidebar — this
    /// does NOT change the browsed source.
    fn feed_sidebar_source(
        &mut self,
        source_type: SourceType,
        source_id: &str,
        name: &str,
        source: Arc<dyn MediaSource>,
        sender: &ComponentSender<Self>,
    ) {
        // Normalize to the same trailing-slash-trimmed form the API clients use
        // for base_url(), so the registry key and sidebar source_id match the
        // source_id every MediaItem carries (else per-item resolution misses).
        let source_id = source_id.trim_end_matches('/');
        self.sources
            .register(source_type, source_id.to_string(), source.clone());
        // Home merges Continue Watching across all sources, so refresh its view
        // of the registry whenever a source is added.
        self.home_view
            .emit(HomeViewMsg::SetSources(self.home_sources()));
        // The source set changed: the cached Home spans the whole set, so drop it
        // (library entries are per-source and stay valid — KTD-2).
        self.content_cache
            .apply_invalidation(invalidation_for(SourceSetEvent::SourceAdded), None);
        let hidden = self.settings.library_visibility.hidden.clone();

        self.sidebar.emit(SidebarMsg::SetSource {
            name: name.to_string(),
            source_type: source_type.as_str().to_string(),
            source_id: source_id.to_string(),
        });
        self.sidebar.emit(SidebarMsg::SetVisibility(hidden));

        // The client absorbs cold-start retries, so a single fetch suffices.
        let src = source;
        let id = source_id.to_string();
        sender.oneshot_command(async move {
            AppCmd::LibrariesLoaded {
                source_id: id,
                libraries: src.libraries().await.unwrap_or_default(),
            }
        });
    }

    /// Make `source` the browsed source and point every browse-oriented view at
    /// it (Home, LibraryView, detail views). Per-item paths still resolve their
    /// own source; this only drives the *browsed* list + Home shelves.
    fn set_browsed_views(
        &mut self,
        source_type: SourceType,
        source_id: &str,
        source: Arc<dyn MediaSource>,
    ) {
        // Match the trimmed form the clients/items use (see feed_sidebar_source).
        let source_id = source_id.trim_end_matches('/');
        self.browsed_source = Some((source_type, source_id.to_string()));
        let cache = self.artwork_cache.clone();
        let hidden = self.settings.library_visibility.hidden.clone();

        self.home_view
            .emit(HomeViewMsg::SetSource(source.clone(), cache.clone()));
        self.home_view.emit(HomeViewMsg::SetVisibility(hidden));
        // NavigateHome may have fired before the source was ready, so re-trigger.
        let in_progress = load_in_progress(&self.db);
        self.home_view.emit(HomeViewMsg::LoadHome { in_progress });
        self.library_view
            .emit(LibraryViewMsg::SetSource(source.clone(), cache.clone()));
        self.library_view.emit(LibraryViewMsg::SetSavedUiState(
            self.settings.library.clone(),
        ));
        self.movie_detail
            .emit(MovieDetailMsg::SetSource(source.clone(), cache.clone()));
        self.show_detail
            .emit(ShowDetailMsg::SetSource(source, cache));

        let watch_data = load_watch_data(&self.db);
        self.library_view
            .emit(LibraryViewMsg::SetWatchData(watch_data));
    }

    /// Feed a source into the sidebar AND make it the browsed source. Used by
    /// the connection-saved path, where a newly-added server should become the
    /// active one.
    fn wire_active_source(
        &mut self,
        source_type: SourceType,
        source_id: String,
        source: Arc<dyn MediaSource>,
        sender: &ComponentSender<Self>,
    ) {
        self.feed_sidebar_source(
            source_type,
            &source_id,
            source.name(),
            source.clone(),
            sender,
        );
        self.set_browsed_views(source_type, &source_id, source);
    }

    /// Remove a connected source and evict everything it owns. This is the ONLY
    /// media/watch eviction path (R5 / DATA-PROTECTION) — it runs solely from
    /// the explicit user-initiated remove-source action.
    fn remove_source(
        &mut self,
        source_type: &str,
        source_id: &str,
        sender: &ComponentSender<Self>,
    ) {
        let Some(st) = SourceType::from_str(source_type) else {
            return;
        };

        // Capture the display name before dropping the live source.
        let name = self
            .sources
            .get(st, source_id)
            .map(|s| s.name().to_string())
            .unwrap_or_else(|| source_id.to_string());

        // Drop from the live registry (the caller drops the sidebar group via
        // SidebarMsg::DropSource after this confirmed removal).
        self.sources.remove(st, source_id);
        // Refresh Home's source set so the removed source leaves the merged
        // Continue Watching row on the subsequent LoadHome.
        self.home_view
            .emit(HomeViewMsg::SetSources(self.home_sources()));
        // Drop the removed source's cached library entries + the cached Home.
        let prefix = SessionContentCache::library_key_prefix(st, source_id.trim_end_matches('/'));
        self.content_cache.apply_invalidation(
            invalidation_for(SourceSetEvent::SourceRemoved),
            Some(&prefix),
        );

        // Persisted eviction: the saved source row, then its media items, then
        // any now-orphaned watch_progress rows (keyed by media_item_id, which
        // embeds the source). Done in this order so orphan cleanup is accurate.
        if let Some(ref db) = self.db {
            let source_key = Source::make_id(st, source_id);
            db.with(|conn| {
                let _ = crate::db::source_repo::SourceRepo::new(conn).delete(&source_key);
                if let Err(e) =
                    crate::db::media_repo::MediaRepo::new(conn).delete_by_source(&st, source_id)
                {
                    tracing::warn!("Failed to delete media for removed source: {e}");
                }
                if let Err(e) = WatchProgressRepo::new(conn).cleanup_orphans() {
                    tracing::warn!("Failed to clean up watch progress for removed source: {e}");
                }
            });
        }

        // If the removed source was the browsed one, pick another registered
        // source as browsed (or none) and navigate Home.
        if self.browsed_source.as_ref().map(|(t, i)| (*t, i.as_str())) == Some((st, source_id)) {
            self.browsed_source = None;
            let next = self
                .sources
                .iter()
                .next()
                .map(|n| (n.source_type, n.source_id.clone(), n.source.clone()));
            if let Some((next_type, next_id, next_src)) = next {
                self.set_browsed_views(next_type, &next_id, next_src);
            }
            sender.input(AppMsg::NavigateHome);
        } else {
            // Removing a non-browsed source still changes Home (it fans out over
            // all sources) and the library watch data.
            let in_progress = load_in_progress(&self.db);
            self.home_view.emit(HomeViewMsg::LoadHome { in_progress });
            let watch_data = load_watch_data(&self.db);
            self.library_view
                .emit(LibraryViewMsg::SetWatchData(watch_data));
        }

        sender.input(AppMsg::ShowToast(format!("Removed {name}")));
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum AppMsg {
    NavigateHome,
    Navigate {
        section: LibrarySection,
        /// The owning source's type/id, so the App can scope the LibraryView
        /// list to the right source (sidebar carries this per row).
        source_type: String,
        source_id: String,
    },
    /// Store freshly-fetched library items in the session content cache,
    /// composing the cache key from the browsed source + the section key.
    CacheLibraryItems {
        section_key: String,
        items: Vec<MediaItem>,
    },
    /// Store the assembled Home payload in the session content cache, keyed by
    /// the current source set.
    CacheHome(Box<CachedHome>),
    /// A background Home revalidation finished; clear the in-flight flag.
    HomeRevalidationDone,
    /// Persist a library/Collections visibility change (composite key + visible).
    SetLibraryVisible {
        key: String,
        visible: bool,
    },
    /// The sidebar left edit mode; re-evaluate the current view and refresh Home.
    SidebarEditModeExited,
    ShowMovieDetail(crate::models::media::MediaItem),
    ShowShowDetail(crate::models::media::MediaItem),
    /// Open the detail page for a downloaded item, resolving the stored
    /// `media_item_id` (a movie's, or an episode's) to a `MediaItem` from the
    /// local DB — episodes route to their parent show.
    OpenDownloadDetail(String),
    /// Play a completed download straight from its local file, reconstructing a
    /// `MediaItem` from the download's own metadata snapshot so it works with no
    /// source reachable. Carries the download's `media_item_id`.
    PlayDownload(String),
    /// The library poster density changed; mirror it onto the Downloads grid.
    SetGridDensity(crate::services::library_filter::GridDensity),
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
        source_type: String,
        user_id: Option<String>,
        /// Whether the selected connection is remote/relay (R5/U2). `false` for
        /// non-Plex sources.
        is_remote: bool,
    },
    ShowToast(String),
    /// Keepalive timer tick — ping the active transcode session (U10/R15).
    TranscodeKeepalive,
    FocusSearch,
    /// Show the Collections of a specific source (per-source, from the sidebar).
    ShowCollections {
        source_type: String,
        source_id: String,
    },
    ShowDownloads,
    /// A source's remove button was clicked: show a destructive confirmation
    /// dialog before evicting anything.
    RemoveSource {
        source_type: String,
        source_id: String,
    },
    /// The user confirmed removal: evict the source's media/watch rows + saved
    /// config and drop its sidebar group.
    ConfirmRemoveSource {
        source_type: String,
        source_id: String,
    },
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
    /// Enqueue a single library item (movie or episode) for download.
    EnqueueDownload(MediaItem),
    /// Enqueue a show/season download: snapshot its current episodes as a group.
    EnqueueDownloadGroup {
        parent: MediaItem,
        episodes: Vec<MediaItem>,
        scope: crate::models::download::GroupScope,
    },
    /// Download (or retry) a single episode, nesting it under its show's group.
    EnqueueDownloadEpisode {
        show: Box<MediaItem>,
        episode: Box<MediaItem>,
    },
    /// Open the per-show download drill-in for a group id.
    OpenShowDownloads(String),
    /// The drill-in page was popped (back/swipe/escape): drop its controller and
    /// scope state.
    DrilldownClosed,
    /// Begin an undoable delete of a download (shows an undo toast).
    RequestDeleteDownload {
        media_item_id: String,
        title: String,
    },
    /// Undo button on the delete toast was pressed: restore the captured row.
    UndoDeleteDownload(String),
    /// The delete toast was dismissed: finalize the delete unless it was undone.
    FinalizeDeleteDownload(String),
    /// A per-item download action from the Downloads UI.
    DownloadAction {
        media_item_id: String,
        action: DownloadItemAction,
    },
    /// Reorder a queued download to a new position.
    ReorderDownload {
        media_item_id: String,
        new_index: usize,
    },
    /// Progress / completion streamed from a background transfer task.
    DownloadRunner(DownloadRunnerMsg),
}

#[derive(Debug)]
pub enum AppCmd {
    /// A saved source validated (or was re-discovered) on startup. `source`
    /// carries the final, possibly-rediscovered config; `original_id` is the
    /// pre-validation id so the persistence upsert can clear a stale row when a
    /// Plex URL changed. `is_remote` is the connection-type classification (U2)
    /// applied to the Plex client for the default bitrate cap (R6).
    SourceValidated {
        source: Source,
        original_id: String,
        is_remote: bool,
    },
    /// Validation failed for a single source — identified by id so it never
    /// affects another source's data.
    SourceValidationFailed { source_id: String, message: String },
    /// A source's libraries, fetched after validation, routed to its sidebar
    /// group by `source_id`.
    LibrariesLoaded {
        source_id: String,
        libraries: Vec<LibrarySection>,
    },
    /// No-op for fire-and-forget async commands (scrobble, timeline).
    Noop,
    /// A background library revalidation refetch returned. Guarded on apply by
    /// the stamped epochs; merged with any racing local mutation (U5/U6/U7).
    LibraryRevalidated {
        cache_key: String,
        entry_epoch: u64,
        source_set_epoch: u64,
        dispatch_seq: u64,
        result: Result<Vec<MediaItem>, String>,
    },
    /// Skip markers fetched from the media source after playback starts.
    SkipMarkersLoaded(SkipMarkers),
    /// A playback decision resolved (U7): hand the URL to the player using the
    /// decision-kind-specific resume policy. `epoch` tags the switch this result
    /// belongs to; a superseded epoch is discarded (U8).
    PlaybackResolved {
        decision: Box<crate::models::playback::PlaybackDecision>,
        resume_secs: Option<f64>,
        epoch: u64,
    },
    /// The playback decision failed to resolve. Surfaces a notice and falls back
    /// to a last-resort direct-play of `fallback_url` (not silent — R-guarded).
    PlaybackResolveFailed {
        message: String,
        fallback_url: String,
        resume_secs: Option<f64>,
        epoch: u64,
    },
    /// A timeline/scrobble report failed offline — queue it for later sync.
    QueueOfflineSync(crate::models::download::PendingSync),
    /// Pending-sync rows successfully flushed to the source on reconnect.
    FlushedPending(Vec<i64>),
    /// Result of a transcode keepalive ping (U10): `true` = alive, `false` =
    /// failed. A run of failures declares the session lost.
    KeepalivePinged(bool),
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
        // Reclaim streaming-cache temp files orphaned by a previous crash
        // before any playback pipeline can begin writing a new one.
        reclaim_stream_cache();

        // One-time capability probe: GStreamer capabilities and mpv availability.
        let playback_capabilities = crate::player::capabilities::PlaybackCapabilities::probe();

        let settings = Settings::load();
        let (downloads, download_rx) =
            DownloadManager::new(settings.downloads.effective_concurrency());
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
                SidebarOutput::AddSource => AppMsg::ShowConnectionDialog,
                SidebarOutput::Navigate {
                    section,
                    source_type,
                    source_id,
                } => AppMsg::Navigate {
                    section,
                    source_type,
                    source_id,
                },
                SidebarOutput::ShowCollections {
                    source_type,
                    source_id,
                } => AppMsg::ShowCollections {
                    source_type,
                    source_id,
                },
                SidebarOutput::ShowDownloads => AppMsg::ShowDownloads,
                SidebarOutput::SetLibraryVisible { key, visible } => {
                    AppMsg::SetLibraryVisible { key, visible }
                }
                SidebarOutput::EditModeExited => AppMsg::SidebarEditModeExited,
                SidebarOutput::RemoveSource {
                    source_type,
                    source_id,
                } => AppMsg::RemoveSource {
                    source_type,
                    source_id,
                },
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
                HomeViewOutput::HomeAssembled(home) => AppMsg::CacheHome(home),
                HomeViewOutput::RevalidationDone => AppMsg::HomeRevalidationDone,
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
                LibraryViewOutput::DensityChanged(d) => AppMsg::SetGridDensity(d),
                LibraryViewOutput::ItemsFetched { section_key, items } => {
                    AppMsg::CacheLibraryItems { section_key, items }
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
                MovieDetailOutput::DownloadMedia(item) => AppMsg::EnqueueDownload(*item),
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
                ShowDetailOutput::DownloadGroup {
                    parent,
                    episodes,
                    scope,
                } => AppMsg::EnqueueDownloadGroup {
                    parent: *parent,
                    episodes,
                    scope,
                },
                ShowDetailOutput::DownloadEpisode { show, episode } => {
                    AppMsg::EnqueueDownloadEpisode { show, episode }
                }
                ShowDetailOutput::DeleteEpisode {
                    media_item_id,
                    title,
                } => AppMsg::RequestDeleteDownload {
                    media_item_id,
                    title,
                },
                ShowDetailOutput::Error(msg) => AppMsg::ShowToast(msg),
            },
        );

        let downloads_view = DownloadsView::builder()
            .launch(DownloadsConfig::default())
            .forward(sender.input_sender(), |output| match output {
                DownloadsViewOutput::ItemAction {
                    media_item_id,
                    action,
                } => AppMsg::DownloadAction {
                    media_item_id,
                    action,
                },
                DownloadsViewOutput::OpenShowDownloads(group_id) => {
                    AppMsg::OpenShowDownloads(group_id)
                }
                DownloadsViewOutput::RequestDelete {
                    media_item_id,
                    title,
                } => AppMsg::RequestDeleteDownload {
                    media_item_id,
                    title,
                },
                DownloadsViewOutput::OpenDetail(media_item_id) => {
                    AppMsg::OpenDownloadDetail(media_item_id)
                }
                DownloadsViewOutput::PlayDownload(media_item_id) => {
                    AppMsg::PlayDownload(media_item_id)
                }
            });

        let widgets = view_output!();

        let built = build_widgets(
            &root,
            &sender,
            &sidebar,
            &home_view,
            &library_view,
            &video_player,
            &downloads_view,
        );
        let toast_overlay = built.toast_overlay;
        let stack = built.stack;
        let nav_view = built.nav_view;
        // Clear the drill-in's controller/state when its page is popped (back
        // button, swipe, or Escape) so a manually-dismissed drill-in doesn't keep
        // receiving refreshes or leave stale auto-pop state.
        {
            let s = sender.input_sender().clone();
            nav_view.connect_popped(move |_nav, page| {
                if page.tag().as_deref() == Some("drilldown") {
                    let _ = s.send(AppMsg::DrilldownClosed);
                }
            });
        }
        let split_view = built.split_view;
        let library_title = built.library_title;
        let player_chrome_revealer = built.player_chrome_revealer;
        let player_window_title = built.player_window_title;

        // Initialize database (shared, single connection for the whole app)
        let db = Database::open();
        if let Some(db) = &db {
            show_detail.emit(ShowDetailMsg::SetDb(db.clone()));
        }

        // Load and validate *all* enabled saved sources (async — each validates
        // independently so one unreachable server never blocks or evicts another).
        let mut has_sources = false;
        if let Some(db) = &db {
            let sources = db.with(|conn| {
                crate::db::source_repo::SourceRepo::new(conn)
                    .list()
                    .unwrap_or_default()
            });
            for source in sources.into_iter().filter(|s| s.enabled) {
                has_sources = true;
                info!(
                    "Loaded saved {:?} source: {} (url={})",
                    source.source_type, source.name, source.config.url
                );
                let data_dir = crate::config::data_dir();
                sender.oneshot_command(async move { validate_source(source, data_dir).await });
            }
        }

        let mut model = Self {
            home_view,
            video_player,
            sidebar,
            library_view,
            movie_detail,
            show_detail,
            downloads_view,
            show_drilldown: None,
            drilldown_group: None,
            pending_deletes: std::collections::HashMap::new(),
            connection_dialog: None,
            screensaver: ScreensaverInhibitor::new(),
            toast_overlay,
            stack,
            nav_view,
            split_view,
            library_title,
            current_view: CurrentView::default(),
            db,
            now_playing: None,
            sources: SourceRegistry::default(),
            browsed_source: None,
            artwork_cache: Arc::new(ArtworkCache::new(crate::config::artwork_dir())),
            watch_tracker: WatchStateTracker::new(),
            last_position: 0.0,
            pending_resume: None,
            source_connecting: false,
            settings,
            mpris: mpris::spawn_mpris_server(),
            player_chrome_revealer,
            player_window_title,
            downloads,
            current_quality: crate::models::playback::QualitySelection::Auto,
            active_transcode_session: None,
            transcode_base_offset: 0.0,
            current_decision: None,
            switch_state: crate::components::player::switch_state::SwitchState::new(),
            render_fallback: crate::components::player::switch_state::RenderFallback::new(),
            keepalive_source: None,
            keepalive_failures: 0,
            current_audio_stream_id: None,
            current_subtitle_stream_id: None,
            playback_capabilities,
            content_cache: SessionContentCache::new(),
            content_mutation_seq: 0,
            content_last_mutation: std::collections::HashMap::new(),
            revalidating: std::collections::HashSet::new(),
            home_revalidating: false,
            home_revalidate_epoch: 0,
        };

        // Reconcile downloads against disk and rebuild the queue (no transfers
        // start until the source validates — see download_handlers::start_pending).
        download_handlers::recover_on_startup(&mut model);

        // Relay transfer-task progress from tokio to the GTK main loop.
        let sender_dl = sender.input_sender().clone();
        let mut download_rx = download_rx;
        glib::spawn_future_local(async move {
            while let Some(msg) = download_rx.recv().await {
                let _ = sender_dl.send(AppMsg::DownloadRunner(msg));
            }
        });

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
                handlers::handle_navigate_home(self, root);
            }
            AppMsg::Navigate {
                section,
                source_type,
                source_id,
            } => {
                handlers::handle_navigate(self, section, source_type, source_id, &sender, root);
            }
            AppMsg::CacheLibraryItems { section_key, items } => {
                handlers::handle_cache_library_items(self, section_key, items);
            }
            AppMsg::CacheHome(home) => {
                handlers::handle_cache_home(self, *home);
            }
            AppMsg::HomeRevalidationDone => {
                self.home_revalidating = false;
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
                handlers::handle_sidebar_edit_mode_exited(self, &sender);
            }
            AppMsg::ShowMovieDetail(item) => {
                self.current_view = CurrentView::MovieDetail(item.id.clone());
                // Resolve the item's OWNING source (not the browsed one) so a
                // merged-Home item from another server opens correctly.
                if let Some(src) = self.sources.for_item(&item) {
                    self.movie_detail
                        .emit(MovieDetailMsg::SetSource(src, self.artwork_cache.clone()));
                }
                let downloaded = self.is_downloaded(&item.id);
                self.movie_detail
                    .emit(MovieDetailMsg::LoadMovie(item.clone()));
                self.movie_detail
                    .emit(MovieDetailMsg::SetDownloaded(downloaded));
                let page = adw::NavigationPage::builder()
                    .title(&item.title)
                    .child(self.movie_detail.widget())
                    .build();
                self.nav_view.push(&page);
            }
            AppMsg::ShowShowDetail(item) => {
                self.current_view = CurrentView::ShowDetail(item.id.clone());
                if let Some(src) = self.sources.for_item(&item) {
                    self.show_detail
                        .emit(ShowDetailMsg::SetSource(src, self.artwork_cache.clone()));
                }
                self.show_detail.emit(ShowDetailMsg::LoadShow(item.clone()));
                // Seed the per-episode download controls with current state so
                // they're correct as soon as the episode list renders.
                self.refresh_downloads_view();
                let page = adw::NavigationPage::builder()
                    .title(&item.title)
                    .child(self.show_detail.widget())
                    .build();
                self.nav_view.push(&page);
            }
            AppMsg::OpenDownloadDetail(media_item_id) => {
                handlers::handle_open_download_detail(self, media_item_id, &sender);
            }
            AppMsg::PlayDownload(media_item_id) => {
                match download_handlers::resolve_play_download(self, &media_item_id) {
                    download_handlers::PlayDownloadOutcome::Play { url, item } => {
                        sender.input(AppMsg::PlayMedia {
                            url,
                            media_item: Some(*item),
                        });
                    }
                    download_handlers::PlayDownloadOutcome::FileMissing => {
                        sender.input(AppMsg::ShowToast(
                            "This download's file is missing.".to_string(),
                        ));
                    }
                    // Still transferring (or failed): the card only plays once
                    // complete, so there's nothing to do here.
                    download_handlers::PlayDownloadOutcome::NotReady => {}
                }
            }
            AppMsg::SetGridDensity(density) => {
                self.downloads_view
                    .emit(DownloadsViewMsg::SetDensity(density));
            }
            AppMsg::GoBack => {
                if self.stack.visible_child_name().as_deref() == Some("player") {
                    // Stop the transcode session + keepalive on navigate-away (R14).
                    handlers::stop_active_session(self, &sender);
                    // Stop watch tracking when leaving player
                    let events = self.watch_tracker.stop(self.last_position);
                    let src = self.now_playing_source();
                    dispatch_watch_events(
                        &self.db,
                        events,
                        &src,
                        self.now_playing.as_ref().map(|i| i.id.as_str()),
                        &sender,
                    );
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
                let dialog = ConnectionDialog::builder()
                    .transient_for(root)
                    .launch(crate::config::data_dir())
                    .forward(sender.input_sender(), |output| match output {
                        ConnectionDialogOutput::ConnectionSaved {
                            url,
                            token,
                            name,
                            source_type,
                            user_id,
                            is_remote,
                        } => AppMsg::ConnectionSaved {
                            url,
                            token,
                            name,
                            source_type,
                            user_id,
                            is_remote,
                        },
                        ConnectionDialogOutput::Cancelled => {
                            AppMsg::ShowToast("Connection cancelled".to_string())
                        }
                    });
                dialog.widget().present();
                self.connection_dialog = Some(dialog);
            }
            AppMsg::ConnectionSaved {
                url,
                token,
                name,
                source_type,
                user_id,
                is_remote,
            } => {
                // Don't silently fall back to Plex on an unrecognized type — a
                // Plex client built from Jellyfin credentials would fail opaquely.
                let Some(st) = SourceType::from_str(&source_type) else {
                    tracing::warn!("Ignoring connection with unknown source_type: {source_type}");
                    sender.input(AppMsg::ShowToast(format!(
                        "Unknown source type \u{201c}{source_type}\u{201d}"
                    )));
                    return;
                };
                handle_connection_saved(self, url, token, name, st, user_id, is_remote, &sender);
            }
            AppMsg::ShowToast(message) => {
                if !message.is_empty() {
                    let toast = adw::Toast::new(&message);
                    toast.set_timeout(3);
                    self.toast_overlay.add_toast(toast);
                }
            }
            AppMsg::TranscodeKeepalive => {
                transcode_keepalive::tick(self, &sender);
            }
            AppMsg::ShowFileChooser => {
                show_file_chooser(root, sender.input_sender().clone());
            }
            AppMsg::FocusSearch => {
                self.library_view.emit(LibraryViewMsg::FocusSearch);
            }
            AppMsg::ShowCollections {
                source_type,
                source_id,
            } => {
                self.current_view = CurrentView::Collections;
                self.stack.set_visible_child_name("shell");
                self.nav_view.replace_with_tags(&["library"]);
                root.set_fullscreened(false);
                root.set_title(Some("Reel"));
                self.library_title.set_label("Collections");
                // Collections are per-source: point the browsed source + the
                // LibraryView list at the owning source before loading.
                if let Some(st) = SourceType::from_str(&source_type) {
                    self.browsed_source = Some((st, source_id.clone()));
                    if let Some(src) = self.sources.get(st, &source_id) {
                        self.library_view
                            .emit(LibraryViewMsg::SetSource(src, self.artwork_cache.clone()));
                    }
                }
                self.library_view.emit(LibraryViewMsg::LoadCollections);
            }
            AppMsg::RemoveSource {
                source_type,
                source_id,
            } => {
                // Destructive: confirm before evicting media + watch history
                // (DATA-PROTECTION — never delete user data without explicit
                // confirmation).
                let name = SourceType::from_str(&source_type)
                    .and_then(|st| self.sources.get(st, &source_id))
                    .map_or_else(|| source_id.clone(), |s| s.name().to_string());
                let dialog = adw::AlertDialog::new(
                    Some("Remove server?"),
                    Some(&format!(
                        "Removing \u{201c}{name}\u{201d} deletes its synced media and watch \
                         history from this device. This can't be undone."
                    )),
                );
                dialog.add_response("cancel", "Cancel");
                dialog.add_response("remove", "Remove");
                dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
                dialog.set_default_response(Some("cancel"));
                dialog.set_close_response("cancel");
                let input = sender.input_sender().clone();
                dialog.connect_response(None, move |_, response| {
                    if response == "remove" {
                        let _ = input.send(AppMsg::ConfirmRemoveSource {
                            source_type: source_type.clone(),
                            source_id: source_id.clone(),
                        });
                    }
                });
                dialog.present(Some(root));
            }
            AppMsg::ConfirmRemoveSource {
                source_type,
                source_id,
            } => {
                self.remove_source(&source_type, &source_id, &sender);
                // Drop the now-evicted source's sidebar group.
                self.sidebar.emit(SidebarMsg::DropSource {
                    source_type,
                    source_id,
                });
            }
            AppMsg::ShowDownloads => {
                self.current_view = CurrentView::Downloads;
                self.stack.set_visible_child_name("shell");
                self.nav_view.replace_with_tags(&["downloads"]);
                root.set_fullscreened(false);
                root.set_title(Some("Reel"));
                self.refresh_downloads_view();
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
                handlers::handle_mark_watched(self, item, &sender);
            }
            AppMsg::MarkUnwatched(item) => {
                handlers::handle_mark_unwatched(self, item, &sender);
            }
            AppMsg::SaveLibraryUiState { library_id, state } => {
                self.settings.library.set(&library_id, state);
                if let Err(e) = self.settings.save() {
                    tracing::warn!("Failed to persist library filter/sort state: {e}");
                }
            }
            AppMsg::OpenPreferences => {
                let usage = self
                    .db
                    .as_ref()
                    .and_then(|db| {
                        db.with(|conn| {
                            crate::db::downloads_repo::DownloadsRepo::new(conn)
                                .total_completed_bytes()
                                .ok()
                        })
                    })
                    .unwrap_or(0);
                self.settings = settings_dialog::show_preferences(root, &self.settings, usage);
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
            AppMsg::EnqueueDownload(item) => {
                download_handlers::enqueue_download(self, &item);
            }
            AppMsg::EnqueueDownloadGroup {
                parent,
                episodes,
                scope,
            } => {
                download_handlers::enqueue_group(self, &parent, &episodes, scope);
            }
            AppMsg::EnqueueDownloadEpisode { show, episode } => {
                download_handlers::enqueue_download_episode(self, &show, &episode);
            }
            AppMsg::OpenShowDownloads(group_id) => {
                self.open_show_downloads(&group_id, &sender);
            }
            AppMsg::DrilldownClosed => {
                self.show_drilldown = None;
                self.drilldown_group = None;
            }
            AppMsg::RequestDeleteDownload {
                media_item_id,
                title,
            } => {
                download_handlers::request_delete_with_undo(self, &media_item_id, &title, &sender);
            }
            AppMsg::UndoDeleteDownload(media_item_id) => {
                download_handlers::undo_delete(self, &media_item_id);
            }
            AppMsg::FinalizeDeleteDownload(media_item_id) => {
                download_handlers::finalize_delete(self, &media_item_id);
            }
            AppMsg::DownloadAction {
                media_item_id,
                action,
            } => {
                download_handlers::item_action(self, &media_item_id, action);
            }
            AppMsg::ReorderDownload {
                media_item_id,
                new_index,
            } => {
                download_handlers::reorder_download(self, &media_item_id, new_index);
            }
            AppMsg::DownloadRunner(msg) => {
                download_handlers::handle_runner_msg(self, msg);
            }
        }
    }

    // Command dispatcher; grows one arm per async command type (see init /
    // handle_video_output for the same allow).
    #[allow(clippy::too_many_lines)]
    fn update_cmd(
        &mut self,
        cmd: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match cmd {
            AppCmd::SourceValidated {
                source,
                original_id,
                is_remote,
            } => {
                handlers::handle_source_validated(self, source, original_id, is_remote, &sender);
            }
            AppCmd::LibrariesLoaded {
                source_id,
                libraries,
            } => {
                self.sidebar.emit(SidebarMsg::SetLibraries {
                    source_id,
                    libraries,
                });
            }
            AppCmd::SourceValidationFailed { source_id, message } => {
                // One source failing must not disturb others: log + toast, leave
                // its saved row and all media/watch data intact.
                tracing::warn!("Saved source {source_id} not reachable: {message}");
                self.source_connecting = false;
                self.home_view.emit(HomeViewMsg::SetConnecting(false));
                sender.input(AppMsg::ShowToast(format!("Server unreachable: {message}")));
            }
            AppCmd::SkipMarkersLoaded(markers) => {
                self.video_player
                    .emit(VideoPlayerMsg::SetSkipMarkers(markers));
            }
            AppCmd::PlaybackResolved {
                decision,
                resume_secs,
                epoch,
            } => handle_playback_resolved(self, decision, resume_secs, epoch, &sender),
            AppCmd::PlaybackResolveFailed {
                message,
                fallback_url,
                resume_secs,
                epoch,
            } => handle_playback_resolve_failed(
                self,
                message,
                fallback_url,
                resume_secs,
                epoch,
                &sender,
            ),
            AppCmd::QueueOfflineSync(pending) => {
                watch_events::queue_offline_sync(&self.db, &pending)
            }
            AppCmd::FlushedPending(ids) => watch_events::delete_flushed_pending(&self.db, &ids),
            AppCmd::KeepalivePinged(ok) => {
                transcode_keepalive::record_result(self, ok, &sender);
            }
            AppCmd::LibraryRevalidated {
                cache_key,
                entry_epoch,
                source_set_epoch,
                dispatch_seq,
                result,
            } => {
                handlers::handle_library_revalidated(
                    self,
                    cache_key,
                    entry_epoch,
                    source_set_epoch,
                    dispatch_seq,
                    result,
                );
            }
            AppCmd::Noop => {}
        }
    }
}
