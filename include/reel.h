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

/** Opaque handle to a Reel player instance */
typedef struct ReelPlayer ReelPlayer;

/** Player state */
typedef enum {
    REEL_STATE_IDLE,
    REEL_STATE_PLAYING,
    REEL_STATE_PAUSED,
    REEL_STATE_STOPPED,
} ReelState;

/** Error codes */
typedef enum {
    REEL_OK = 0,
    REEL_ERR_INIT = -1,
    REEL_ERR_LOAD = -2,
    REEL_ERR_COMMAND = -3,
    REEL_ERR_RENDER = -4,
} ReelError;

/** Create a new player instance */
ReelPlayer* reel_player_create(void);

/** Destroy a player instance */
void reel_player_destroy(ReelPlayer* player);

/** Load and play a media file */
ReelError reel_player_load_file(ReelPlayer* player, const char* path);

/** Toggle play/pause */
ReelError reel_player_toggle_pause(ReelPlayer* player);

/** Seek relative to current position (seconds) */
ReelError reel_player_seek(ReelPlayer* player, double seconds);

/** Seek to absolute position (seconds) */
ReelError reel_player_seek_absolute(ReelPlayer* player, double seconds);

/** Set volume (0-100) */
ReelError reel_player_set_volume(ReelPlayer* player, double volume);

/** Toggle mute */
ReelError reel_player_toggle_mute(ReelPlayer* player);

/** Cycle subtitle track */
ReelError reel_player_cycle_sub(ReelPlayer* player);

/** Cycle audio track */
ReelError reel_player_cycle_audio(ReelPlayer* player);

/** Get current playback position in seconds */
double reel_player_get_position(ReelPlayer* player);

/** Get total duration in seconds */
double reel_player_get_duration(ReelPlayer* player);

/** Get current player state */
ReelState reel_player_get_state(ReelPlayer* player);

#ifdef __cplusplus
}
#endif

#endif /* REEL_H */
