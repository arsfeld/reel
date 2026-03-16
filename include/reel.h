/**
 * libreel - Core library for the Reel media center
 *
 * This header defines the C ABI for libreel, consumed by
 * platform-native frontends (GTK4 on Linux, SwiftUI on macOS).
 */

#ifndef REEL_H
#define REEL_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ── Opaque handles ──────────────────────────────────────── */

typedef struct ReelPlayer ReelPlayer;
typedef struct ReelDatabase ReelDatabase;
typedef struct ReelLibrary ReelLibrary;

/* ── Error codes ─────────────────────────────────────────── */

typedef enum {
    REEL_OK = 0,
    REEL_ERR_INIT = -1,
    REEL_ERR_LOAD = -2,
    REEL_ERR_COMMAND = -3,
    REEL_ERR_RENDER = -4,
    REEL_ERR_DB = -5,
    REEL_ERR_QUERY = -6,
    REEL_ERR_NOT_FOUND = -7,
} ReelError;

/* ── Player ──────────────────────────────────────────────── */

typedef enum {
    REEL_STATE_IDLE,
    REEL_STATE_PLAYING,
    REEL_STATE_PAUSED,
    REEL_STATE_STOPPED,
} ReelState;

ReelPlayer* reel_player_create(void);
void reel_player_destroy(ReelPlayer* player);
ReelError reel_player_load_file(ReelPlayer* player, const char* path);
ReelError reel_player_toggle_pause(ReelPlayer* player);
ReelError reel_player_seek(ReelPlayer* player, double seconds);
ReelError reel_player_seek_absolute(ReelPlayer* player, double seconds);
ReelError reel_player_set_volume(ReelPlayer* player, double volume);
ReelError reel_player_toggle_mute(ReelPlayer* player);
ReelError reel_player_cycle_sub(ReelPlayer* player);
ReelError reel_player_cycle_audio(ReelPlayer* player);
double reel_player_get_position(ReelPlayer* player);
double reel_player_get_duration(ReelPlayer* player);
double reel_player_get_volume(ReelPlayer* player);
ReelState reel_player_get_state(ReelPlayer* player);
ReelError reel_player_stop(ReelPlayer* player);

/** Drain pending mpv events, updating cached position/duration/state. */
void reel_player_poll_events(ReelPlayer* player);

/* ── Player Rendering ───────────────────────────────────── */

typedef void* (*ReelGetProcAddressFn)(void* ctx, const char* name);
typedef void (*ReelRenderUpdateFn)(void* ctx);

/** Initialize OpenGL render context. Call after GL context is current. */
ReelError reel_player_init_render(ReelPlayer* player,
                                   ReelGetProcAddressFn get_proc_address);

/** Set callback invoked when mpv has a new frame. Fires on mpv's thread. */
void reel_player_set_render_update_callback(ReelPlayer* player,
                                             ReelRenderUpdateFn callback,
                                             void* ctx);

/** Render current frame into the given OpenGL FBO. */
void reel_player_render(ReelPlayer* player,
                         int fbo, int width, int height);

/* ── Media types ─────────────────────────────────────────── */

typedef enum {
    REEL_MEDIA_MOVIE = 0,
    REEL_MEDIA_SHOW = 1,
    REEL_MEDIA_SEASON = 2,
    REEL_MEDIA_EPISODE = 3,
    REEL_MEDIA_OTHER = 4,
} ReelMediaType;

typedef enum {
    REEL_SOURCE_PLEX = 0,
    REEL_SOURCE_LOCAL = 1,
} ReelMediaSource;

typedef enum {
    REEL_SORT_TITLE = 0,
    REEL_SORT_YEAR = 1,
    REEL_SORT_RATING = 2,
    REEL_SORT_ADDED = 3,
} ReelSortField;

typedef enum {
    REEL_SORT_ASC = 0,
    REEL_SORT_DESC = 1,
} ReelSortOrder;

/* ── Media item (C-compatible flat struct) ───────────────── */

typedef struct {
    int64_t id;
    int media_type;             /* ReelMediaType */
    int source;                 /* ReelMediaSource */
    const char* title;
    const char* sort_title;     /* nullable */
    int32_t year;               /* 0 if unknown */
    const char* summary;        /* nullable */
    double rating;              /* 0 if unknown */
    int64_t duration_ms;        /* 0 if unknown */
    const char* poster_path;    /* nullable */
    const char* backdrop_path;  /* nullable */
    int64_t parent_id;          /* 0 if none */
    int32_t season_number;      /* 0 if none */
    int32_t episode_number;     /* 0 if none */
    const char* file_path;      /* nullable */
} ReelMediaItem;

/* ── Database & Library ──────────────────────────────────── */

ReelDatabase* reel_db_open(const char* path);
void reel_db_close(ReelDatabase* db);

ReelLibrary* reel_library_create(ReelDatabase* db);
void reel_library_destroy(ReelLibrary* lib);

/** Query items by type with sort and pagination. Returns count written, or -1 on error. */
int32_t reel_library_get_items(ReelLibrary* lib,
                               ReelMediaType type,
                               ReelSortField sort_by,
                               ReelSortOrder sort_order,
                               int32_t limit,
                               int32_t offset,
                               ReelMediaItem* out_items,
                               int32_t max_items);

/** Get recently added movies/shows. Returns count written, or -1 on error. */
int32_t reel_library_get_recently_added(ReelLibrary* lib,
                                         int32_t limit,
                                         ReelMediaItem* out_items,
                                         int32_t max_items);

/** Get items with in-progress watch state. Returns count written, or -1 on error. */
int32_t reel_library_get_continue_watching(ReelLibrary* lib,
                                            int32_t limit,
                                            ReelMediaItem* out_items,
                                            int32_t max_items);

int32_t reel_library_get_item_count(ReelLibrary* lib, ReelMediaType type);

/** Full-text search. Returns count written, or -1 on error. */
int32_t reel_library_search(ReelLibrary* lib,
                             const char* query,
                             ReelMediaItem* out_items,
                             int32_t max_items);

/** Get a single media item by ID. Returns REEL_OK or REEL_ERR_NOT_FOUND. */
ReelError reel_library_get_item(ReelLibrary* lib,
                                 int64_t id,
                                 ReelMediaItem* out_item);

/** Get children of a parent item (seasons of show, episodes of season). */
int32_t reel_library_get_items_by_parent(ReelLibrary* lib,
                                          int64_t parent_id,
                                          ReelMediaItem* out_items,
                                          int32_t max_items);

/* Server management */
int32_t reel_library_server_count(ReelLibrary* lib);

/* Favorites */
int64_t reel_library_add_favorite(ReelLibrary* lib,
                                   const char* item_type,
                                   const char* item_id,
                                   const char* display_name);
ReelError reel_library_remove_favorite(ReelLibrary* lib, int64_t id);

typedef struct {
    int64_t id;
    const char* item_type;
    const char* item_id;
    const char* display_name;
    int32_t sort_order;
} ReelFavoriteC;

/** List all favorites. Pass out_ptr=NULL to query just the count. */
ReelError reel_library_list_favorites(ReelLibrary* lib,
                                       ReelFavoriteC* out_ptr,
                                       int32_t* out_count);

/* Scan paths */
int64_t reel_library_add_scan_path(ReelLibrary* lib, const char* path);
ReelError reel_library_remove_scan_path(ReelLibrary* lib, int64_t id);

typedef struct {
    int64_t id;
    const char* path;
} ReelScanPathC;

/** List all scan paths. Pass out_ptr=NULL to query just the count. */
ReelError reel_library_list_scan_paths(ReelLibrary* lib,
                                        ReelScanPathC* out_ptr,
                                        int32_t* out_count);

/* Settings */
const char* reel_settings_get(ReelDatabase* db, const char* key);
ReelError reel_settings_set(ReelDatabase* db, const char* key, const char* value);

/* ── Watch Progress ─────────────────────────────────────── */

typedef struct {
    int64_t media_item_id;
    int64_t position_ms;
    int64_t duration_ms;    /* 0 if unknown */
    int watched;            /* 0 or 1 */
} ReelWatchProgressC;

/** Get watch progress for a media item. Returns REEL_OK or REEL_ERR_NOT_FOUND. */
ReelError reel_library_get_watch_progress(ReelLibrary* lib,
                                            int64_t media_item_id,
                                            ReelWatchProgressC* out);

/** Update watch progress for a media item. */
ReelError reel_library_update_watch_progress(ReelLibrary* lib,
                                               int64_t media_item_id,
                                               int64_t position_ms,
                                               int64_t duration_ms,
                                               int watched);

/* ── Collections ────────────────────────────────────────── */

typedef enum {
    REEL_COLLECTION_MANUAL = 0,
    REEL_COLLECTION_SMART = 1,
} ReelCollectionType;

typedef struct {
    int64_t id;
    const char* name;
    int collection_type;    /* 0=manual, 1=smart */
    const char* description; /* nullable */
} ReelCollectionC;

int64_t reel_collection_create(ReelLibrary* lib,
                                const char* name,
                                int collection_type,
                                const char* description);
ReelError reel_collection_delete(ReelLibrary* lib, int64_t id);

/** List all collections. Pass out_ptr=NULL to query just the count. */
ReelError reel_collection_list(ReelLibrary* lib,
                                ReelCollectionC* out_ptr,
                                int32_t* out_count);

ReelError reel_collection_add_item(ReelLibrary* lib,
                                    int64_t collection_id,
                                    int64_t media_item_id);
ReelError reel_collection_remove_item(ReelLibrary* lib,
                                       int64_t collection_id,
                                       int64_t media_item_id);

/** Get media items in a collection. Returns count written, or -1 on error. */
int32_t reel_collection_get_items(ReelLibrary* lib,
                                   int64_t collection_id,
                                   ReelMediaItem* out_items,
                                   int32_t max_items);

/* ── Genres ─────────────────────────────────────────────── */

ReelError reel_genre_set(ReelLibrary* lib,
                          int64_t media_item_id,
                          const char* const* genre_names,
                          int count);

typedef struct {
    int64_t id;
    const char* name;
} ReelGenreC;

/** List distinct genres. Pass out_ptr=NULL to query just the count. */
ReelError reel_library_get_genres(ReelLibrary* lib,
                                   ReelGenreC* out_ptr,
                                   int32_t* out_count);

/** Get media items matching a genre. Returns count written, or -1 on error. */
int32_t reel_library_get_items_by_genre(ReelLibrary* lib,
                                         const char* genre_name,
                                         int32_t limit,
                                         ReelMediaItem* out_items,
                                         int32_t max_items);

/* ── Match lock ─────────────────────────────────────────── */

ReelError reel_match_set_locked(ReelLibrary* lib,
                                 int64_t media_item_id,
                                 int locked);

/* ── Plex Auth ──────────────────────────────────────────── */

typedef struct ReelPlexAuth ReelPlexAuth;

typedef struct {
    const char* id;
    const char* name;
    const char* uri;
    const char* access_token;
} ReelPlexServerC;

typedef struct {
    const char* id;
    const char* name;
    const char* connection_uri;
} ReelServerC;

ReelPlexAuth* reel_plex_create(ReelDatabase* db);
void reel_plex_destroy(ReelPlexAuth* plex);
const char* reel_plex_request_pin(ReelPlexAuth* plex);
const char* reel_plex_get_auth_url(ReelPlexAuth* plex);
const char* reel_plex_poll_pin(ReelPlexAuth* plex);
int32_t reel_plex_discover_servers(ReelPlexAuth* plex,
                                    ReelPlexServerC* out_ptr,
                                    int32_t max);

/** Discover servers and store all connection URIs in the database. */
int32_t reel_plex_discover_servers_with_lib(ReelPlexAuth* plex,
                                             ReelPlexServerC* out_ptr,
                                             int32_t max,
                                             ReelLibrary* lib);

/**
 * Resolve the best connection URI for a server by testing reachability.
 * Connections must already be stored (via discover_servers_with_lib or refresh_connections).
 * Updates servers.connection_uri with the winner.
 * Returns the best URI (caller frees with reel_free), or NULL if none reachable.
 */
const char* reel_server_resolve_connection(ReelLibrary* lib, const char* server_id);

/**
 * Re-discover connections for all stored servers from Plex API and store them.
 * Loads auth tokens from the servers table automatically.
 * Returns number of servers refreshed, or -1 on error.
 */
int32_t reel_server_refresh_connections(ReelLibrary* lib);

/**
 * Sync a Plex server's libraries into the local media_items table.
 * Fetches all library sections, items, and children (seasons/episodes).
 * Returns the number of items synced, or -1 on error.
 */
int32_t reel_plex_sync(ReelLibrary* lib,
                        const char* server_id,
                        const char* server_uri,
                        const char* auth_token);

/** Sync all stored Plex servers (reads auth tokens from DB). */
int32_t reel_plex_sync_all(ReelLibrary* lib);

ReelError reel_server_upsert(ReelLibrary* lib,
                              const char* id,
                              const char* name,
                              const char* connection_uri,
                              const char* auth_token);
ReelError reel_server_delete(ReelLibrary* lib, const char* id);

/** List stored servers. Pass out_ptr=NULL to query just the count. */
ReelError reel_server_list(ReelLibrary* lib,
                            ReelServerC* out_ptr,
                            int32_t* out_count);

/* ── Downloads ──────────────────────────────────────────── */

typedef struct ReelDownloader ReelDownloader;

ReelDownloader* reel_download_create(ReelDatabase* db);
void reel_download_destroy(ReelDownloader* dl);

int64_t reel_download_enqueue(ReelDownloader* dl,
                               int64_t media_item_id,
                               const char* server_id,
                               const char* source_url,
                               const char* download_dir,
                               const char* filename);

ReelError reel_download_pause(ReelDownloader* dl, int64_t id);
ReelError reel_download_resume(ReelDownloader* dl, int64_t id);
ReelError reel_download_remove(ReelDownloader* dl, int64_t id, int delete_file);
const char* reel_download_get_local_path(ReelDownloader* dl, int64_t media_item_id);

typedef struct {
    int64_t id;
    int64_t media_item_id;
    const char* status;         /* "queued", "downloading", "paused", "complete", "failed" */
    int64_t downloaded_bytes;
    int64_t total_bytes;        /* 0 if unknown */
    const char* local_path;     /* nullable */
    const char* error_message;  /* nullable */
} ReelDownloadC;

/** List all downloads. Pass out_ptr=NULL to query just the count. */
ReelError reel_download_list(ReelDownloader* dl,
                              ReelDownloadC* out_ptr,
                              int32_t* out_count);

/* ── Memory Management ──────────────────────────────────── */

/** Free a string or buffer allocated by libreel. */
void reel_free(void* ptr);

#ifdef __cplusplus
}
#endif

#endif /* REEL_H */
