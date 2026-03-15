/**
 * libreel - Core library for the Reel media center
 *
 * This header defines the C ABI for libreel, consumed by
 * platform-native frontends (GTK4 on Linux, AppKit on macOS).
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
ReelState reel_player_get_state(ReelPlayer* player);

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
    ReelMediaType media_type;
    ReelMediaSource source;
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

/* Query items by type with sort and pagination */
int32_t reel_library_get_items(ReelLibrary* lib,
                               ReelMediaType type,
                               ReelSortField sort_by,
                               ReelSortOrder sort_order,
                               int32_t limit,
                               int32_t offset,
                               ReelMediaItem* out_items,
                               int32_t max_items);

int32_t reel_library_get_recently_added(ReelLibrary* lib,
                                         int32_t limit,
                                         ReelMediaItem* out_items,
                                         int32_t max_items);

int32_t reel_library_get_continue_watching(ReelLibrary* lib,
                                            int32_t limit,
                                            ReelMediaItem* out_items,
                                            int32_t max_items);

int32_t reel_library_get_item_count(ReelLibrary* lib, ReelMediaType type);

/* Search */
int32_t reel_library_search(ReelLibrary* lib,
                             const char* query,
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

/* Scan paths */
int64_t reel_library_add_scan_path(ReelLibrary* lib, const char* path);
ReelError reel_library_remove_scan_path(ReelLibrary* lib, int64_t id);

/* Settings */
const char* reel_settings_get(ReelDatabase* db, const char* key);
ReelError reel_settings_set(ReelDatabase* db, const char* key, const char* value);

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

/** Create a collection. Returns the new collection id, or -1 on error. */
int64_t reel_collection_create(ReelLibrary* lib,
                                const char* name,
                                int collection_type,
                                const char* description);

/** Delete a collection by id. Returns REEL_OK or error. */
ReelError reel_collection_delete(ReelLibrary* lib, int64_t id);

/**
 * List all collections.
 * Pass out_ptr=NULL to query just the count.
 * Caller must allocate out_ptr with enough space.
 * Returns REEL_OK or error.
 */
ReelError reel_collection_list(ReelLibrary* lib,
                                ReelCollectionC* out_ptr,
                                int32_t* out_count);

/** Add a media item to a collection. Returns REEL_OK or error. */
ReelError reel_collection_add_item(ReelLibrary* lib,
                                    int64_t collection_id,
                                    int64_t media_item_id);

/** Remove a media item from a collection. Returns REEL_OK or error. */
ReelError reel_collection_remove_item(ReelLibrary* lib,
                                       int64_t collection_id,
                                       int64_t media_item_id);

/* ── Genres ─────────────────────────────────────────────── */

/**
 * Set genres for a media item (replaces existing).
 * genre_names is an array of null-terminated strings with `count` elements.
 * Returns REEL_OK or error.
 */
ReelError reel_genre_set(ReelLibrary* lib,
                          int64_t media_item_id,
                          const char* const* genre_names,
                          int count);

/* ── Match lock ─────────────────────────────────────────── */

/** Lock or unlock metadata matching for a media item. */
ReelError reel_match_set_locked(ReelLibrary* lib,
                                 int64_t media_item_id,
                                 int locked);

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

#ifdef __cplusplus
}
#endif

#endif /* REEL_H */
