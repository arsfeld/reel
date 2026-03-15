const c = @cImport({
    @cInclude("gtk/gtk.h");
    @cInclude("mpv/client.h");
});
const player_mod = @import("../../core/player.zig");
const app = @import("app.zig");

pub fn setup(window: *c.GtkWidget, player: *player_mod.Player) void {
    const controller = c.gtk_event_controller_key_new();
    _ = c.g_signal_connect_data(
        @ptrCast(controller),
        "key-pressed",
        @ptrCast(&onKeyPressed),
        @ptrCast(player),
        null,
        c.G_CONNECT_DEFAULT,
    );
    c.gtk_widget_add_controller(window, @ptrCast(controller));
}

fn onKeyPressed(
    _: *c.GtkEventControllerKey,
    keyval: c.guint,
    _: c.guint,
    _: c.GdkModifierType,
    user_data: ?*anyopaque,
) callconv(.c) c_int {
    const player: *player_mod.Player = @ptrCast(@alignCast(user_data orelse return 0));

    switch (keyval) {
        c.GDK_KEY_space => {
            player.togglePause() catch {};
            return 1;
        },
        c.GDK_KEY_Left => {
            player.seek(-10) catch {};
            return 1;
        },
        c.GDK_KEY_Right => {
            player.seek(10) catch {};
            return 1;
        },
        c.GDK_KEY_Up => {
            adjustVolume(player, 5);
            return 1;
        },
        c.GDK_KEY_Down => {
            adjustVolume(player, -5);
            return 1;
        },
        c.GDK_KEY_f, c.GDK_KEY_F11 => {
            app.toggleFullscreen();
            return 1;
        },
        c.GDK_KEY_Escape => {
            if (app.isFullscreen()) {
                app.toggleFullscreen();
            }
            return 1;
        },
        c.GDK_KEY_m => {
            player.toggleMute() catch {};
            return 1;
        },
        c.GDK_KEY_s => {
            player.cycleSub() catch {};
            return 1;
        },
        c.GDK_KEY_a => {
            player.cycleAudio() catch {};
            return 1;
        },
        else => return 0,
    }
}

fn adjustVolume(player: *player_mod.Player, delta: f64) void {
    var vol: f64 = 100;
    _ = c.mpv_get_property(@ptrCast(player.handle), "volume", c.MPV_FORMAT_DOUBLE, @ptrCast(&vol));
    const new_vol = @max(0, @min(150, vol + delta));
    player.setVolume(new_vol) catch {};
}
