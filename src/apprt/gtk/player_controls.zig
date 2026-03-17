const std = @import("std");
const c = @cImport({
    @cInclude("gtk/gtk.h");
    @cInclude("mpv/client.h");
});
const player_mod = @import("../../core/player.zig");

pub const Controls = struct {
    widget: ?*c.GtkWidget = null,
    player: ?*player_mod.Player = null,
    play_button: ?*c.GtkWidget = null,
    time_label: ?*c.GtkWidget = null,
    seek_bar: ?*c.GtkWidget = null,
    volume_button: ?*c.GtkWidget = null,
    hide_timeout: c.guint = 0,
    seeking: bool = false,
    poll_source: c.guint = 0,

    pub fn init(player: *player_mod.Player) Controls {
        // Main controls box
        const controls_box = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 4);
        c.gtk_widget_add_css_class(controls_box, "osd");
        c.gtk_widget_set_margin_start(controls_box, 12);
        c.gtk_widget_set_margin_end(controls_box, 12);
        c.gtk_widget_set_margin_bottom(controls_box, 12);

        // Seek bar
        const seek_bar = c.gtk_scale_new_with_range(c.GTK_ORIENTATION_HORIZONTAL, 0, 100, 1);
        c.gtk_scale_set_draw_value(@ptrCast(seek_bar), 0);
        c.gtk_widget_set_hexpand(seek_bar, 1);
        c.gtk_box_append(@ptrCast(controls_box), seek_bar);

        // Bottom row: play button, time label, volume
        const bottom_row = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 8);
        c.gtk_widget_set_halign(bottom_row, c.GTK_ALIGN_CENTER);

        // Play/pause button
        const play_button = c.gtk_button_new_from_icon_name("media-playback-start-symbolic");
        c.gtk_widget_add_css_class(play_button, "flat");
        c.gtk_box_append(@ptrCast(bottom_row), play_button);

        // Time label
        const time_label = c.gtk_label_new("0:00 / 0:00");
        c.gtk_widget_set_margin_start(time_label, 8);
        c.gtk_widget_set_margin_end(time_label, 8);
        c.gtk_box_append(@ptrCast(bottom_row), time_label);

        // Volume button
        const volume_button = c.gtk_volume_button_new();
        c.gtk_scale_button_set_value(@ptrCast(volume_button), 1.0);
        c.gtk_box_append(@ptrCast(bottom_row), volume_button);

        // Fullscreen button
        const fs_button = c.gtk_button_new_from_icon_name("view-fullscreen-symbolic");
        c.gtk_widget_add_css_class(fs_button, "flat");
        c.gtk_box_append(@ptrCast(bottom_row), fs_button);

        c.gtk_box_append(@ptrCast(controls_box), bottom_row);

        const self = Controls{
            .widget = controls_box,
            .player = player,
            .play_button = play_button,
            .time_label = time_label,
            .seek_bar = seek_bar,
            .volume_button = volume_button,
        };

        // Store self pointer for callbacks
        c.g_object_set_data(@ptrCast(controls_box), "controls", @ptrCast(player));

        // Connect signals
        _ = c.g_signal_connect_data(
            @ptrCast(play_button),
            "clicked",
            @ptrCast(&onPlayClicked),
            @ptrCast(player),
            null,
            c.G_CONNECT_DEFAULT,
        );

        _ = c.g_signal_connect_data(
            @ptrCast(fs_button),
            "clicked",
            @ptrCast(&onFullscreenClicked),
            null,
            null,
            c.G_CONNECT_DEFAULT,
        );

        _ = c.g_signal_connect_data(
            @ptrCast(volume_button),
            "value-changed",
            @ptrCast(&onVolumeChanged),
            @ptrCast(player),
            null,
            c.G_CONNECT_DEFAULT,
        );

        _ = c.g_signal_connect_data(
            @ptrCast(seek_bar),
            "change-value",
            @ptrCast(&onSeekChanged),
            @ptrCast(player),
            null,
            c.G_CONNECT_DEFAULT,
        );

        return self;
    }

    /// Start polling mpv state. Must be called after the Controls value
    /// has been stored at its final (stable) address.
    pub fn startPolling(self: *Controls) void {
        global_controls = self;
        self.poll_source = c.g_timeout_add(250, &pollPlayerState, null);
    }

    pub fn show(self: *Controls) void {
        if (self.widget) |w| {
            c.gtk_widget_set_visible(w, 1);
        }
    }

    pub fn hide(self: *Controls) void {
        if (self.widget) |w| {
            c.gtk_widget_set_visible(w, 0);
        }
    }

    pub fn scheduleHide(self: *Controls) void {
        if (self.hide_timeout != 0) {
            _ = c.g_source_remove(self.hide_timeout);
        }
        self.hide_timeout = c.g_timeout_add_seconds(3, &hideCallback, @ptrCast(self));
    }

    pub fn updatePlayButton(self: *Controls, paused: bool) void {
        if (self.play_button) |btn| {
            const icon: [*:0]const u8 = if (paused)
                "media-playback-start-symbolic"
            else
                "media-playback-pause-symbolic";
            c.gtk_button_set_icon_name(@ptrCast(btn), icon);
        }
    }

    pub fn updateTime(self: *Controls, pos: f64, dur: f64) void {
        if (self.time_label) |label| {
            var buf: [64]u8 = undefined;
            const text = std.fmt.bufPrintZ(&buf, "{s} / {s}", .{
                formatTime(pos),
                formatTime(dur),
            }) catch return;
            c.gtk_label_set_text(@ptrCast(label), text.ptr);
        }
        if (!self.seeking) {
            if (self.seek_bar) |bar| {
                if (dur > 0) {
                    c.gtk_range_set_value(@ptrCast(bar), pos / dur * 100.0);
                }
            }
        }
    }
};

var global_controls: ?*Controls = null;

fn formatTime(seconds: f64) [8]u8 {
    const total: u64 = @intFromFloat(@max(0, seconds));
    const h = total / 3600;
    const m = (total % 3600) / 60;
    const s = total % 60;
    var buf: [8]u8 = undefined;
    if (h > 0) {
        _ = std.fmt.bufPrint(&buf, "{d}:{d:0>2}:{d:0>2}", .{ h, m, s }) catch {};
    } else {
        _ = std.fmt.bufPrint(&buf, "  {d}:{d:0>2}", .{ m, s }) catch {};
    }
    return buf;
}

fn onPlayClicked(_: *c.GtkButton, user_data: ?*anyopaque) callconv(.c) void {
    const player: *player_mod.Player = @ptrCast(@alignCast(user_data orelse return));
    player.togglePause() catch {};
}

fn onFullscreenClicked(_: *c.GtkButton, _: ?*anyopaque) callconv(.c) void {
    const app = @import("app.zig");
    app.toggleFullscreen();
}

fn onVolumeChanged(_: *c.GtkScaleButton, value: f64, user_data: ?*anyopaque) callconv(.c) void {
    const player: *player_mod.Player = @ptrCast(@alignCast(user_data orelse return));
    player.setVolume(value * 100.0) catch {};
}

fn onSeekChanged(_: *c.GtkRange, _: c.GtkScrollType, value: f64, user_data: ?*anyopaque) callconv(.c) c_int {
    const player: *player_mod.Player = @ptrCast(@alignCast(user_data orelse return 0));

    // value is 0-100 percentage; we need to convert to seconds
    // For now, use mpv's percent-pos property
    var pct = value;
    _ = c.mpv_set_property(@ptrCast(player.handle), "percent-pos", c.MPV_FORMAT_DOUBLE, @ptrCast(&pct));
    return 0;
}

fn hideCallback(user_data: ?*anyopaque) callconv(.c) c_int {
    const controls: *Controls = @ptrCast(@alignCast(user_data orelse return 0));
    controls.hide_timeout = 0;
    controls.hide();
    return 0; // G_SOURCE_REMOVE
}

fn pollPlayerState(_: ?*anyopaque) callconv(.c) c_int {
    const controls = global_controls orelse return 1;
    const player = controls.player orelse return 1;

    // Process pending mpv events
    while (true) {
        const event = player.waitEvent(0);
        switch (event) {
            .property_change => |prop| {
                switch (prop) {
                    .pause => |paused| controls.updatePlayButton(paused),
                    .time_pos => |pos| {
                        if (pos) |p| {
                            // Get duration from mpv directly
                            var dur: f64 = 0;
                            _ = c.mpv_get_property(@ptrCast(player.handle), "duration", c.MPV_FORMAT_DOUBLE, @ptrCast(&dur));
                            controls.updateTime(p, dur);
                        }
                    },
                    else => {},
                }
            },
            .idle, .unknown => break,
            .shutdown => return 0, // Stop polling
            else => {},
        }
    }
    return 1; // G_SOURCE_CONTINUE
}
