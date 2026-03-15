const std = @import("std");
const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});
const video_area = @import("video_area.zig");
const player_controls = @import("player_controls.zig");
const keys = @import("keys.zig");
const player_mod = @import("../../core/player.zig");

const AppState = struct {
    player: player_mod.Player,
    window: ?*c.GtkWidget = null,
    video: video_area.VideoArea = .{},
    controls: player_controls.Controls = .{},
    fullscreen: bool = false,
    file_path: ?[]const u8 = null,
    hide_cursor_timeout: c.guint = 0,
};

var app_state: AppState = undefined;

pub fn run(file_path: ?[]const u8) !void {
    app_state.player = try player_mod.Player.init();
    app_state.file_path = file_path;

    const app = c.adw_application_new("dev.reel.player", c.G_APPLICATION_DEFAULT_FLAGS) orelse
        return error.AppCreateFailed;
    defer c.g_object_unref(@ptrCast(app));

    _ = c.g_signal_connect_data(
        @ptrCast(app),
        "activate",
        @ptrCast(&onActivate),
        null,
        null,
        c.G_CONNECT_DEFAULT,
    );

    const status = c.g_application_run(@ptrCast(app), 0, null);
    app_state.player.deinit();

    if (status != 0) return error.AppRunFailed;
}

fn onActivate(app: *c.GtkApplication, _: ?*anyopaque) callconv(.c) void {
    const window = c.adw_application_window_new(app);
    app_state.window = window;
    c.gtk_window_set_title(@ptrCast(window), "Reel");
    c.gtk_window_set_default_size(@ptrCast(window), 1280, 720);

    // Create the main layout: overlay with video + controls
    const overlay = c.gtk_overlay_new();

    // Video area
    app_state.video = video_area.VideoArea.init(&app_state.player);
    c.gtk_overlay_set_child(@ptrCast(overlay), @ptrCast(app_state.video.widget));

    // Player controls overlay
    app_state.controls = player_controls.Controls.init(&app_state.player);
    c.gtk_overlay_add_overlay(@ptrCast(overlay), @ptrCast(app_state.controls.widget));
    c.gtk_widget_set_valign(@ptrCast(app_state.controls.widget), c.GTK_ALIGN_END);

    // Main box with header bar
    const vbox = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 0);
    const header = c.adw_header_bar_new();
    c.gtk_box_append(@ptrCast(vbox), @ptrCast(header));
    c.gtk_box_append(@ptrCast(vbox), overlay);
    c.gtk_widget_set_vexpand(overlay, 1);

    c.adw_application_window_set_content(@ptrCast(window), vbox);

    // Keyboard handler
    keys.setup(@ptrCast(window), &app_state.player);

    // Motion handler for cursor hiding
    const motion = c.gtk_event_controller_motion_new();
    _ = c.g_signal_connect_data(
        @ptrCast(motion),
        "motion",
        @ptrCast(&onMotion),
        null,
        null,
        c.G_CONNECT_DEFAULT,
    );
    c.gtk_widget_add_controller(window, @ptrCast(motion));

    c.gtk_window_present(@ptrCast(window));

    // Load file if provided
    if (app_state.file_path) |path| {
        app_state.player.loadFile(path) catch |err| {
            std.log.err("Failed to load file: {}", .{err});
        };
    }
}

fn onMotion(_: *c.GtkEventControllerMotion, _: f64, _: f64, _: ?*anyopaque) callconv(.c) void {
    // Show controls on mouse movement
    app_state.controls.show();
    app_state.controls.scheduleHide();
}

pub fn toggleFullscreen() void {
    const window: *c.GtkWindow = @ptrCast(app_state.window orelse return);
    app_state.fullscreen = !app_state.fullscreen;
    if (app_state.fullscreen) {
        c.gtk_window_fullscreen(window);
    } else {
        c.gtk_window_unfullscreen(window);
    }
}

pub fn isFullscreen() bool {
    return app_state.fullscreen;
}
