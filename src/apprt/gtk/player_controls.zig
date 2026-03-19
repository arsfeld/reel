const std = @import("std");
const c = @cImport({
    @cInclude("gtk/gtk.h");
    @cInclude("mpv/client.h");
});
const player_mod = @import("../../core/player.zig");
const library_mod = @import("../../core/library.zig");
const types = @import("../../core/types.zig");

fn unixTimestamp() i64 {
    var ts: std.c.timespec = undefined;
    _ = std.c.clock_gettime(.REALTIME, &ts);
    return @intCast(ts.sec);
}

pub const AutoPlayState = enum {
    idle, // No auto-play (movie, no next episode, direct play)
    monitoring, // Watching position, waiting for trigger
    countdown_active, // Overlay visible, counting down
    transitioning, // Loading next episode
};

pub const Controls = struct {
    widget: ?*c.GtkWidget = null,
    player: ?*player_mod.Player = null,
    play_button: ?*c.GtkWidget = null,
    time_label: ?*c.GtkWidget = null,
    seek_bar: ?*c.GtkWidget = null,
    volume_button: ?*c.GtkWidget = null,
    speed_button: ?*c.GtkWidget = null,
    sub_button: ?*c.GtkWidget = null,
    hide_timeout: c.guint = 0,
    seeking: bool = false,
    poll_source: c.guint = 0,
    // Auto-play state
    auto_play_state: AutoPlayState = .idle,
    countdown_seconds: i32 = 0,
    countdown_source: c.guint = 0,
    countdown_overlay: ?*c.GtkWidget = null,
    countdown_label: ?*c.GtkWidget = null,
    next_episode_id: ?i64 = null,
    next_episode_path: ?[]const u8 = null,
    next_episode_title: ?[]const u8 = null,
    trigger_position: f64 = 0, // position (seconds) at which to trigger countdown
    last_position: f64 = 0,
    last_duration: f64 = 0,
    is_paused: bool = false,
    // Chapter marks
    chapter_marks: [64]?*c.GtkWidget = .{null} ** 64,
    chapter_count: i32 = 0,
    seek_bar_overlay: ?*c.GtkWidget = null, // GtkOverlay wrapping the seek bar

    pub fn init(player: *player_mod.Player) Controls {
        // Main controls box
        const controls_box = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 4);
        c.gtk_widget_add_css_class(controls_box, "osd");
        c.gtk_widget_set_margin_start(controls_box, 12);
        c.gtk_widget_set_margin_end(controls_box, 12);
        c.gtk_widget_set_margin_bottom(controls_box, 12);

        // Seek bar wrapped in overlay for chapter marks
        const seek_overlay = c.gtk_overlay_new();
        const seek_bar = c.gtk_scale_new_with_range(c.GTK_ORIENTATION_HORIZONTAL, 0, 100, 1);
        c.gtk_scale_set_draw_value(@ptrCast(seek_bar), 0);
        c.gtk_widget_set_hexpand(seek_bar, 1);
        c.gtk_overlay_set_child(@ptrCast(seek_overlay), seek_bar);
        c.gtk_widget_set_hexpand(seek_overlay, 1);
        c.gtk_box_append(@ptrCast(controls_box), seek_overlay);

        // Bottom row: play button, time label, subtitle, speed, volume, fullscreen
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

        // Subtitle button
        const sub_button = c.gtk_button_new_from_icon_name("media-view-subtitles-symbolic");
        c.gtk_widget_add_css_class(sub_button, "flat");
        c.gtk_widget_set_tooltip_text(sub_button, "Subtitles");
        c.gtk_box_append(@ptrCast(bottom_row), sub_button);

        // Speed button
        const speed_button = c.gtk_button_new_with_label("1x");
        c.gtk_widget_add_css_class(speed_button, "flat");
        c.gtk_widget_set_tooltip_text(speed_button, "Playback speed");
        c.gtk_box_append(@ptrCast(bottom_row), speed_button);

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
            .speed_button = speed_button,
            .sub_button = sub_button,
            .seek_bar_overlay = seek_overlay,
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

        _ = c.g_signal_connect_data(
            @ptrCast(speed_button),
            "clicked",
            @ptrCast(&onSpeedClicked),
            @ptrCast(player),
            null,
            c.G_CONNECT_DEFAULT,
        );

        _ = c.g_signal_connect_data(
            @ptrCast(sub_button),
            "clicked",
            @ptrCast(&onSubClicked),
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

    pub fn updateSpeedLabel(self: *Controls) void {
        const player = self.player orelse return;
        const btn = self.speed_button orelse return;
        const speed = player.getSpeed();
        var buf: [8]u8 = undefined;
        const text = std.fmt.bufPrintZ(&buf, "{d:.2}x", .{speed}) catch return;
        // Trim trailing zeros: "1.00x" -> "1x", "1.50x" -> "1.5x"
        var label_buf: [8]u8 = undefined;
        var len: usize = 0;
        var saw_dot = false;
        var trailing_zeros: usize = 0;
        for (text) |ch| {
            if (ch == 'x') break;
            if (ch == '.') saw_dot = true;
            if (saw_dot and ch == '0') {
                trailing_zeros += 1;
            } else {
                trailing_zeros = 0;
            }
            label_buf[len] = ch;
            len += 1;
        }
        // Remove trailing zeros after decimal (keep at least one digit after dot)
        if (saw_dot) {
            while (trailing_zeros > 0 and len > 0 and label_buf[len - 1] == '0') {
                len -= 1;
                trailing_zeros -= 1;
            }
            // Remove trailing dot
            if (len > 0 and label_buf[len - 1] == '.') {
                len -= 1;
            }
        }
        label_buf[len] = 'x';
        len += 1;
        label_buf[len] = 0;
        c.gtk_button_set_label(@ptrCast(btn), @ptrCast(&label_buf));
    }

    // ── Auto-play ───────────────────────────────────────

    /// Called when a new file starts playing. Checks if there's a next episode.
    pub fn setupAutoPlay(self: *Controls) void {
        self.cancelAutoPlay();

        const app = @import("app.zig");
        if (app.isDirectPlay()) {
            self.auto_play_state = .idle;
            return;
        }

        const item_id = app.getCurrentMediaItemId() orelse {
            self.auto_play_state = .idle;
            return;
        };

        const lib = app.getLibrary() orelse {
            self.auto_play_state = .idle;
            return;
        };

        const next = lib.getNextEpisode(item_id) catch null orelse {
            self.auto_play_state = .idle;
            return;
        };

        // Store next episode info
        self.next_episode_id = next.id;
        self.next_episode_title = next.title;
        self.next_episode_path = next.file_path;
        self.auto_play_state = .monitoring;

        // Try to get Plex credits marker for trigger position
        if (app.getCreditsStartSeconds(item_id)) |credits_start| {
            self.trigger_position = credits_start;
        } else {
            self.trigger_position = 0; // Will fall back to duration-30 once known
        }
    }

    /// Update chapter tick marks on the seek bar. Called after file_loaded.
    pub fn updateChapterMarks(self: *Controls) void {
        // Remove existing marks
        self.clearChapterMarks();

        const player = self.player orelse return;
        const count = player.getChapterCount();
        if (count <= 0) return;

        const seek_overlay = self.seek_bar_overlay orelse return;

        self.chapter_count = @min(count, 64);

        var i: i32 = 0;
        while (i < self.chapter_count) : (i += 1) {
            // Get chapter time position
            var key_buf: [64]u8 = undefined;
            const key = std.fmt.bufPrintZ(&key_buf, "chapter-list/{d}/time", .{i}) catch continue;
            var time_pos: f64 = 0;
            _ = c.mpv_get_property(@ptrCast(player.handle), key.ptr, c.MPV_FORMAT_DOUBLE, @ptrCast(&time_pos));

            // Create a thin mark widget
            const mark = c.gtk_drawing_area_new();
            c.gtk_widget_set_size_request(mark, 2, -1); // 2px wide, full height
            c.gtk_widget_set_can_target(mark, 0); // Don't intercept clicks
            c.gtk_widget_set_opacity(mark, 0.6);
            c.gtk_widget_add_css_class(mark, "chapter-mark");
            c.gtk_widget_set_valign(mark, c.GTK_ALIGN_FILL);
            c.gtk_widget_set_halign(mark, c.GTK_ALIGN_START);

            // Store time_pos as widget data for repositioning
            c.g_object_set_data(@ptrCast(@alignCast(mark)), "time_pos",
                @ptrFromInt(@as(usize, @bitCast(@as(i64, @intFromFloat(time_pos * 1000.0))))));

            c.gtk_overlay_add_overlay(@ptrCast(seek_overlay), mark);
            self.chapter_marks[@intCast(i)] = mark;
        }
    }

    fn clearChapterMarks(self: *Controls) void {
        const seek_overlay = self.seek_bar_overlay orelse return;
        for (&self.chapter_marks) |*mark_slot| {
            if (mark_slot.*) |mark| {
                c.gtk_overlay_remove_overlay(@ptrCast(seek_overlay), mark);
                mark_slot.* = null;
            }
        }
        self.chapter_count = 0;
    }

    /// Reposition chapter marks based on current seek bar width and duration.
    pub fn repositionChapterMarks(self: *Controls, dur: f64) void {
        if (dur <= 0) return;
        const seek_bar = self.seek_bar orelse return;
        const bar_width = c.gtk_widget_get_width(seek_bar);
        if (bar_width <= 0) return;

        // GtkScale has internal padding (~12px each side for the trough)
        const padding: i32 = 12;
        const usable_width = bar_width - (padding * 2);
        if (usable_width <= 0) return;

        var i: usize = 0;
        while (i < @as(usize, @intCast(self.chapter_count))) : (i += 1) {
            const mark = self.chapter_marks[i] orelse continue;

            // Retrieve time_pos from widget data
            const raw = @intFromPtr(c.g_object_get_data(@ptrCast(@alignCast(mark)), "time_pos"));
            const time_ms: i64 = @bitCast(@as(usize, raw));
            const time_pos: f64 = @as(f64, @floatFromInt(time_ms)) / 1000.0;

            const frac = time_pos / dur;
            const margin: i32 = padding + @as(i32, @intFromFloat(frac * @as(f64, @floatFromInt(usable_width))));
            c.gtk_widget_set_margin_start(mark, margin);
        }
    }

    /// Check if we should trigger the countdown based on current position.
    pub fn checkAutoPlayTrigger(self: *Controls, pos: f64, dur: f64) void {
        self.last_position = pos;
        self.last_duration = dur;

        if (dur <= 0) return;

        // Handle seek backward past trigger — cancel active countdown
        if (self.auto_play_state == .countdown_active) {
            if (pos < self.trigger_position) {
                // User seeked backward past trigger — cancel and return to monitoring
                if (self.countdown_source != 0) {
                    _ = c.g_source_remove(self.countdown_source);
                    self.countdown_source = 0;
                }
                self.hideCountdownOverlay();
                self.auto_play_state = .monitoring;
            }
            return;
        }

        if (self.auto_play_state != .monitoring) return;

        // Set trigger position if not yet set (30 seconds before end, or 90% for short content)
        if (self.trigger_position == 0) {
            self.trigger_position = @max(dur - 30.0, dur * 0.9);
        }

        if (pos >= self.trigger_position) {
            self.startCountdown();
        }
    }

    fn startCountdown(self: *Controls) void {
        self.auto_play_state = .countdown_active;
        self.countdown_seconds = 15;

        // Create countdown overlay
        self.createCountdownOverlay();

        // Start real-time countdown timer
        self.countdown_source = c.g_timeout_add_seconds(1, &countdownTick, null);
    }

    pub fn cancelAutoPlay(self: *Controls) void {
        if (self.countdown_source != 0) {
            _ = c.g_source_remove(self.countdown_source);
            self.countdown_source = 0;
        }
        self.hideCountdownOverlay();
        self.auto_play_state = .idle;
        self.next_episode_id = null;
        self.next_episode_path = null;
        self.next_episode_title = null;
        self.trigger_position = 0;
    }

    fn createCountdownOverlay(self: *Controls) void {
        if (self.countdown_overlay != null) return;

        const overlay_box = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 12);
        c.gtk_widget_add_css_class(overlay_box, "osd");
        c.gtk_widget_set_halign(overlay_box, c.GTK_ALIGN_END);
        c.gtk_widget_set_valign(overlay_box, c.GTK_ALIGN_END);
        c.gtk_widget_set_margin_end(overlay_box, 24);
        c.gtk_widget_set_margin_bottom(overlay_box, 80); // Clear controls bar

        // Info box: title + countdown
        const info_box = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 4);
        c.gtk_widget_set_margin_top(info_box, 12);
        c.gtk_widget_set_margin_bottom(info_box, 12);
        c.gtk_widget_set_margin_start(info_box, 12);

        const up_next_label = c.gtk_label_new("Up Next");
        c.gtk_widget_add_css_class(up_next_label, "dim-label");
        c.gtk_widget_set_halign(up_next_label, c.GTK_ALIGN_START);
        c.gtk_box_append(@ptrCast(info_box), up_next_label);

        // Episode title
        if (self.next_episode_title) |title| {
            var title_buf: [128]u8 = undefined;
            const title_z = std.fmt.bufPrintZ(&title_buf, "{s}", .{title}) catch "Next Episode";
            const title_label = c.gtk_label_new(title_z.ptr);
            c.gtk_widget_set_halign(title_label, c.GTK_ALIGN_START);
            c.gtk_label_set_ellipsize(@ptrCast(title_label), 3); // PANGO_ELLIPSIZE_END
            c.gtk_label_set_max_width_chars(@ptrCast(title_label), 30);
            c.gtk_box_append(@ptrCast(info_box), title_label);
        }

        // Countdown label
        const cd_label = c.gtk_label_new("Playing in 15s...");
        c.gtk_widget_add_css_class(cd_label, "dim-label");
        c.gtk_widget_set_halign(cd_label, c.GTK_ALIGN_START);
        c.gtk_box_append(@ptrCast(info_box), cd_label);
        self.countdown_label = cd_label;

        c.gtk_box_append(@ptrCast(overlay_box), info_box);

        // Buttons box
        const btn_box = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 4);
        c.gtk_widget_set_margin_top(btn_box, 8);
        c.gtk_widget_set_margin_bottom(btn_box, 8);
        c.gtk_widget_set_margin_end(btn_box, 12);
        c.gtk_widget_set_valign(btn_box, c.GTK_ALIGN_CENTER);

        const play_now_btn = c.gtk_button_new_with_label("Play Now");
        c.gtk_widget_add_css_class(play_now_btn, "suggested-action");
        _ = c.g_signal_connect_data(
            @ptrCast(play_now_btn),
            "clicked",
            @ptrCast(&onPlayNowClicked),
            null,
            null,
            c.G_CONNECT_DEFAULT,
        );
        c.gtk_box_append(@ptrCast(btn_box), play_now_btn);

        const cancel_btn = c.gtk_button_new_with_label("Cancel");
        c.gtk_widget_add_css_class(cancel_btn, "flat");
        _ = c.g_signal_connect_data(
            @ptrCast(cancel_btn),
            "clicked",
            @ptrCast(&onCancelAutoPlayClicked),
            null,
            null,
            c.G_CONNECT_DEFAULT,
        );
        c.gtk_box_append(@ptrCast(btn_box), cancel_btn);

        c.gtk_box_append(@ptrCast(overlay_box), btn_box);

        self.countdown_overlay = overlay_box;

        // Add overlay to the player's GtkOverlay widget
        const app = @import("app.zig");
        if (app.getPlayerOverlay()) |player_overlay| {
            c.gtk_overlay_add_overlay(@ptrCast(player_overlay), overlay_box);
        }
    }

    fn hideCountdownOverlay(self: *Controls) void {
        if (self.countdown_overlay) |overlay| {
            const parent = c.gtk_widget_get_parent(overlay);
            if (parent != null) {
                c.gtk_overlay_remove_overlay(@ptrCast(parent), overlay);
            }
            self.countdown_overlay = null;
            self.countdown_label = null;
        }
    }

    fn transitionToNextEpisode(self: *Controls) void {
        self.auto_play_state = .transitioning;

        // Preserve current speed
        const current_speed = if (self.player) |p| p.getSpeed() else 1.0;

        const next_id = self.next_episode_id orelse {
            self.cancelAutoPlay();
            return;
        };

        const app = @import("app.zig");

        // Mark current episode as watched
        if (app.getCurrentMediaItemId()) |current_id| {
            if (app.getLibrary()) |lib| {
                const dur_ms: i64 = @intFromFloat(self.last_duration * 1000.0);
                lib.updateWatchProgress(.{
                    .media_item_id = current_id,
                    .position_ms = dur_ms, // At the end
                    .duration_ms = dur_ms,
                    .watched = true,
                    .last_watched_at = unixTimestamp(),
                }) catch {};
            }
        }

        // Check for downloaded version first
        const path = blk: {
            if (app.getDownloader()) |dl| {
                if (dl.getCompletedLocalPath(next_id) catch null) |local_path| {
                    break :blk local_path;
                }
            }
            if (self.next_episode_path) |p| break :blk p;
            self.cancelAutoPlay();
            return;
        };

        self.hideCountdownOverlay();

        // Play next episode
        app.playNextEpisode(next_id, path);

        // Restore speed
        if (current_speed != 1.0) {
            if (self.player) |p| p.setSpeed(current_speed) catch {};
        }

        // Setup auto-play for the episode after that
        self.setupAutoPlay();
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

// ── Speed popover ───────────────────────────────────────

const speed_presets = [_]f64{ 0.5, 0.75, 1.0, 1.25, 1.5, 2.0 };
const speed_labels = [_][*:0]const u8{ "0.5x", "0.75x", "1x", "1.25x", "1.5x", "2x" };

fn onSpeedClicked(button: *c.GtkButton, user_data: ?*anyopaque) callconv(.c) void {
    const player: *player_mod.Player = @ptrCast(@alignCast(user_data orelse return));
    const current_speed = player.getSpeed();

    // Create popover
    const popover = c.gtk_popover_new();
    c.gtk_widget_set_parent(popover, @ptrCast(button));

    const list_box = c.gtk_list_box_new();
    c.gtk_list_box_set_selection_mode(@ptrCast(list_box), c.GTK_SELECTION_NONE);
    c.gtk_widget_set_size_request(list_box, 100, -1);

    for (speed_labels, 0..) |label, i| {
        const row = c.gtk_label_new(label);
        c.gtk_widget_set_margin_top(row, 4);
        c.gtk_widget_set_margin_bottom(row, 4);
        c.gtk_widget_set_margin_start(row, 8);
        c.gtk_widget_set_margin_end(row, 8);

        // Bold the current speed
        const speed = speed_presets[i];
        if (@abs(speed - current_speed) < 0.01) {
            var markup_buf: [32]u8 = undefined;
            const markup = std.fmt.bufPrintZ(&markup_buf, "<b>{s}</b>", .{label}) catch continue;
            c.gtk_label_set_markup(@ptrCast(row), markup.ptr);
        }

        c.gtk_list_box_append(@ptrCast(list_box), row);
    }

    // Store player pointer and popover on the list_box for the callback
    c.g_object_set_data(@ptrCast(@alignCast(list_box)), "player", @ptrCast(player));
    c.g_object_set_data(@ptrCast(@alignCast(list_box)), "popover", @ptrCast(popover));

    _ = c.g_signal_connect_data(
        @ptrCast(list_box),
        "row-activated",
        @ptrCast(&onSpeedRowActivated),
        null,
        null,
        c.G_CONNECT_DEFAULT,
    );

    c.gtk_popover_set_child(@ptrCast(popover), list_box);
    c.gtk_popover_popup(@ptrCast(popover));
}

fn onSpeedRowActivated(list_box: *c.GtkListBox, row: *c.GtkListBoxRow, _: ?*anyopaque) callconv(.c) void {
    const index: usize = @intCast(c.gtk_list_box_row_get_index(row));
    if (index >= speed_presets.len) return;

    const player: *player_mod.Player = @ptrCast(@alignCast(
        c.g_object_get_data(@ptrCast(@alignCast(list_box)), "player") orelse return,
    ));
    const popover: *c.GtkWidget = @ptrCast(@alignCast(
        c.g_object_get_data(@ptrCast(@alignCast(list_box)), "popover") orelse return,
    ));

    player.setSpeed(speed_presets[index]) catch {};

    // Update the speed button label
    if (global_controls) |controls| {
        controls.updateSpeedLabel();
    }

    c.gtk_popover_popdown(@ptrCast(popover));
}

// ── Subtitle popover ────────────────────────────────────

fn onSubClicked(button: *c.GtkButton, user_data: ?*anyopaque) callconv(.c) void {
    const player: *player_mod.Player = @ptrCast(@alignCast(user_data orelse return));

    // Create popover
    const popover = c.gtk_popover_new();
    c.gtk_widget_set_parent(popover, @ptrCast(button));

    const list_box = c.gtk_list_box_new();
    c.gtk_list_box_set_selection_mode(@ptrCast(list_box), c.GTK_SELECTION_NONE);
    c.gtk_widget_set_size_request(list_box, 200, -1);

    // Get current subtitle track
    var current_sid: i64 = 0;
    _ = c.mpv_get_property(@ptrCast(player.handle), "sid", c.MPV_FORMAT_INT64, @ptrCast(&current_sid));

    // "None" option
    {
        const none_label = c.gtk_label_new(if (current_sid == 0) "✓ None" else "  None");
        c.gtk_widget_set_halign(none_label, c.GTK_ALIGN_START);
        c.gtk_widget_set_margin_top(none_label, 4);
        c.gtk_widget_set_margin_bottom(none_label, 4);
        c.gtk_widget_set_margin_start(none_label, 8);
        c.gtk_widget_set_margin_end(none_label, 8);
        c.gtk_list_box_append(@ptrCast(list_box), none_label);
    }

    // Enumerate subtitle tracks from mpv's track-list
    var track_count: i64 = 0;
    _ = c.mpv_get_property(@ptrCast(player.handle), "track-list/count", c.MPV_FORMAT_INT64, @ptrCast(&track_count));

    var i: i64 = 0;
    while (i < track_count) : (i += 1) {
        // Check if this is a subtitle track
        var type_key_buf: [64]u8 = undefined;
        const type_key = std.fmt.bufPrintZ(&type_key_buf, "track-list/{d}/type", .{i}) catch continue;
        var type_val: [*c]const u8 = null;
        _ = c.mpv_get_property(@ptrCast(player.handle), type_key.ptr, c.MPV_FORMAT_STRING, @ptrCast(&type_val));
        if (type_val == null) continue;
        const is_sub = std.mem.eql(u8, std.mem.span(type_val.?), "sub");
        c.mpv_free(@constCast(type_val));
        if (!is_sub) continue;

        // Get track ID
        var id_key_buf: [64]u8 = undefined;
        const id_key = std.fmt.bufPrintZ(&id_key_buf, "track-list/{d}/id", .{i}) catch continue;
        var track_id: i64 = 0;
        _ = c.mpv_get_property(@ptrCast(player.handle), id_key.ptr, c.MPV_FORMAT_INT64, @ptrCast(&track_id));

        // Get track title
        var title_key_buf: [64]u8 = undefined;
        const title_key = std.fmt.bufPrintZ(&title_key_buf, "track-list/{d}/title", .{i}) catch continue;
        var title_val: [*c]const u8 = null;
        _ = c.mpv_get_property(@ptrCast(player.handle), title_key.ptr, c.MPV_FORMAT_STRING, @ptrCast(&title_val));

        // Get track language
        var lang_key_buf: [64]u8 = undefined;
        const lang_key = std.fmt.bufPrintZ(&lang_key_buf, "track-list/{d}/lang", .{i}) catch continue;
        var lang_val: [*c]const u8 = null;
        _ = c.mpv_get_property(@ptrCast(player.handle), lang_key.ptr, c.MPV_FORMAT_STRING, @ptrCast(&lang_val));

        // Check if external
        var ext_key_buf: [64]u8 = undefined;
        const ext_key = std.fmt.bufPrintZ(&ext_key_buf, "track-list/{d}/external", .{i}) catch continue;
        var is_external: c_int = 0;
        _ = c.mpv_get_property(@ptrCast(player.handle), ext_key.ptr, c.MPV_FORMAT_FLAG, @ptrCast(&is_external));

        // Build display label
        var label_buf: [128]u8 = undefined;
        const prefix: []const u8 = if (track_id == current_sid) "✓ " else "  ";
        const ext_suffix: []const u8 = if (is_external != 0) " (ext)" else "";

        const display = blk: {
            if (title_val) |t| {
                if (lang_val) |l| {
                    break :blk std.fmt.bufPrintZ(&label_buf, "{s}{s} [{s}]{s}", .{ prefix, std.mem.span(t), std.mem.span(l), ext_suffix }) catch continue;
                }
                break :blk std.fmt.bufPrintZ(&label_buf, "{s}{s}{s}", .{ prefix, std.mem.span(t), ext_suffix }) catch continue;
            } else if (lang_val) |l| {
                break :blk std.fmt.bufPrintZ(&label_buf, "{s}Track {d} [{s}]{s}", .{ prefix, track_id, std.mem.span(l), ext_suffix }) catch continue;
            } else {
                break :blk std.fmt.bufPrintZ(&label_buf, "{s}Track {d}{s}", .{ prefix, track_id, ext_suffix }) catch continue;
            }
        };

        if (title_val) |t| c.mpv_free(@constCast(t));
        if (lang_val) |l| c.mpv_free(@constCast(l));

        const row_label = c.gtk_label_new(display.ptr);
        c.gtk_widget_set_halign(row_label, c.GTK_ALIGN_START);
        c.gtk_widget_set_margin_top(row_label, 4);
        c.gtk_widget_set_margin_bottom(row_label, 4);
        c.gtk_widget_set_margin_start(row_label, 8);
        c.gtk_widget_set_margin_end(row_label, 8);
        c.gtk_list_box_append(@ptrCast(list_box), row_label);
    }

    // Separator + "Add subtitle file..." option
    {
        const sep = c.gtk_separator_new(c.GTK_ORIENTATION_HORIZONTAL);
        c.gtk_list_box_append(@ptrCast(list_box), sep);

        const add_label = c.gtk_label_new("  Add subtitle file...");
        c.gtk_widget_set_halign(add_label, c.GTK_ALIGN_START);
        c.gtk_widget_set_margin_top(add_label, 4);
        c.gtk_widget_set_margin_bottom(add_label, 4);
        c.gtk_widget_set_margin_start(add_label, 8);
        c.gtk_widget_set_margin_end(add_label, 8);
        c.gtk_list_box_append(@ptrCast(list_box), add_label);
    }

    // Store player pointer and popover
    c.g_object_set_data(@ptrCast(@alignCast(list_box)), "player", @ptrCast(player));
    c.g_object_set_data(@ptrCast(@alignCast(list_box)), "popover", @ptrCast(popover));

    _ = c.g_signal_connect_data(
        @ptrCast(list_box),
        "row-activated",
        @ptrCast(&onSubRowActivated),
        null,
        null,
        c.G_CONNECT_DEFAULT,
    );

    c.gtk_popover_set_child(@ptrCast(popover), list_box);
    c.gtk_popover_popup(@ptrCast(popover));
}

fn onSubRowActivated(list_box: *c.GtkListBox, row: *c.GtkListBoxRow, _: ?*anyopaque) callconv(.c) void {
    const player: *player_mod.Player = @ptrCast(@alignCast(
        c.g_object_get_data(@ptrCast(@alignCast(list_box)), "player") orelse return,
    ));
    const popover: *c.GtkWidget = @ptrCast(@alignCast(
        c.g_object_get_data(@ptrCast(@alignCast(list_box)), "popover") orelse return,
    ));

    const index = c.gtk_list_box_row_get_index(row);

    // Count subtitle tracks to determine where "Add subtitle file..." row is
    var sub_track_count: i32 = 0;
    {
        var tc: i64 = 0;
        _ = c.mpv_get_property(@ptrCast(player.handle), "track-list/count", c.MPV_FORMAT_INT64, @ptrCast(&tc));
        var k: i64 = 0;
        while (k < tc) : (k += 1) {
            var tkb: [64]u8 = undefined;
            const tk = std.fmt.bufPrintZ(&tkb, "track-list/{d}/type", .{k}) catch continue;
            var tv: [*c]const u8 = null;
            _ = c.mpv_get_property(@ptrCast(player.handle), tk.ptr, c.MPV_FORMAT_STRING, @ptrCast(&tv));
            if (tv) |t| {
                if (std.mem.eql(u8, std.mem.span(t), "sub")) sub_track_count += 1;
                c.mpv_free(@constCast(t));
            }
        }
    }
    // Rows: 0=None, 1..N=sub tracks, N+1=separator, N+2="Add subtitle file..."
    if (index == 1 + sub_track_count + 1) {
        // "Add subtitle file..." row
        c.gtk_popover_popdown(@ptrCast(popover));
        openSubtitleFilePicker(player);
        return;
    }

    if (index == 0) {
        // "None" selected - disable subtitles
        player.disableSubtitles() catch {};
    } else {
        // Find the Nth subtitle track (index-1 because row 0 is "None")
        var track_count: i64 = 0;
        _ = c.mpv_get_property(@ptrCast(player.handle), "track-list/count", c.MPV_FORMAT_INT64, @ptrCast(&track_count));

        var sub_idx: i32 = 0;
        var j: i64 = 0;
        while (j < track_count) : (j += 1) {
            var type_key_buf: [64]u8 = undefined;
            const type_key = std.fmt.bufPrintZ(&type_key_buf, "track-list/{d}/type", .{j}) catch continue;
            var type_val: [*c]const u8 = null;
            _ = c.mpv_get_property(@ptrCast(player.handle), type_key.ptr, c.MPV_FORMAT_STRING, @ptrCast(&type_val));
            if (type_val == null) continue;
            const is_sub = std.mem.eql(u8, std.mem.span(type_val.?), "sub");
            c.mpv_free(@constCast(type_val));
            if (!is_sub) continue;

            sub_idx += 1;
            if (sub_idx == index) {
                // This is the selected subtitle track
                var id_key_buf: [64]u8 = undefined;
                const id_key = std.fmt.bufPrintZ(&id_key_buf, "track-list/{d}/id", .{j}) catch break;
                var track_id: i64 = 0;
                _ = c.mpv_get_property(@ptrCast(player.handle), id_key.ptr, c.MPV_FORMAT_INT64, @ptrCast(&track_id));
                player.setSubtitleTrack(track_id) catch {};
                break;
            }
        }
    }

    c.gtk_popover_popdown(@ptrCast(popover));
}

fn openSubtitleFilePicker(player: *player_mod.Player) void {
    const app = @import("app.zig");
    const window = app.getWindow() orelse return;

    const dialog = c.gtk_file_dialog_new();
    c.gtk_file_dialog_set_title(@ptrCast(dialog), "Select Subtitle File");

    // Set file filter for subtitle formats
    const filter = c.gtk_file_filter_new();
    c.gtk_file_filter_set_name(filter, "Subtitle files");
    c.gtk_file_filter_add_pattern(filter, "*.srt");
    c.gtk_file_filter_add_pattern(filter, "*.ass");
    c.gtk_file_filter_add_pattern(filter, "*.ssa");
    c.gtk_file_filter_add_pattern(filter, "*.sub");
    c.gtk_file_filter_add_pattern(filter, "*.idx");
    c.gtk_file_filter_add_pattern(filter, "*.vtt");

    const filters = c.g_list_store_new(c.gtk_file_filter_get_type());
    c.g_list_store_append(filters, @ptrCast(filter));
    c.gtk_file_dialog_set_filters(@ptrCast(dialog), @ptrCast(filters));

    // Store player pointer for the callback
    c.g_object_set_data(@ptrCast(@alignCast(dialog)), "player", @ptrCast(player));

    c.gtk_file_dialog_open(
        @ptrCast(dialog),
        @ptrCast(window),
        null, // cancellable
        &onSubFileSelected,
        null,
    );
}

fn onSubFileSelected(source: ?*c.GObject, result: ?*c.GAsyncResult, _: ?*anyopaque) callconv(.c) void {
    const dialog = source orelse return;
    const file = c.gtk_file_dialog_open_finish(@ptrCast(dialog), result, null) orelse return;
    defer c.g_object_unref(file);

    const path = c.g_file_get_path(file) orelse return;
    defer c.g_free(path);

    const player: *player_mod.Player = @ptrCast(@alignCast(
        c.g_object_get_data(dialog, "player") orelse return,
    ));
    player.loadSubtitleFile(std.mem.span(path)) catch {};
}

// ── Polling ─────────────────────────────────────────────

fn pollPlayerState(_: ?*anyopaque) callconv(.c) c_int {
    const controls = global_controls orelse return 1;
    const player = controls.player orelse return 1;

    // Process pending mpv events
    while (true) {
        const event = player.waitEvent(0);
        switch (event) {
            .property_change => |prop| {
                switch (prop) {
                    .pause => |paused| {
                        controls.updatePlayButton(paused);
                        controls.is_paused = paused;
                    },
                    .time_pos => |pos| {
                        if (pos) |p| {
                            // Get duration from mpv directly
                            var dur: f64 = 0;
                            _ = c.mpv_get_property(@ptrCast(player.handle), "duration", c.MPV_FORMAT_DOUBLE, @ptrCast(&dur));
                            controls.updateTime(p, dur);
                            controls.repositionChapterMarks(dur);
                            // Check auto-play trigger
                            controls.checkAutoPlayTrigger(p, dur);
                        }
                    },
                    else => {},
                }
            },
            .file_loaded => {
                // New file loaded — setup auto-play and chapter marks
                controls.setupAutoPlay();
                controls.updateChapterMarks();
            },
            .end_file => |reason| {
                switch (reason) {
                    .eof => {
                        if (controls.auto_play_state == .monitoring) {
                            controls.startCountdown();
                        }
                    },
                    .@"error" => std.log.err("mpv: file playback ended with error", .{}),
                    .stop => std.log.info("mpv: playback stopped", .{}),
                    else => {},
                }
            },
            .log_message => |msg| {
                std.log.warn("mpv [{s}] {s}: {s}", .{ msg.level, msg.prefix, msg.text });
            },
            .idle, .unknown => break,
            .shutdown => return 0, // Stop polling
        }
    }
    return 1; // G_SOURCE_CONTINUE
}

// ── Auto-play callbacks ─────────────────────────────────

fn countdownTick(_: ?*anyopaque) callconv(.c) c_int {
    const controls = global_controls orelse return 0;

    if (controls.auto_play_state != .countdown_active) return 0; // G_SOURCE_REMOVE

    // Pause countdown when playback is paused
    if (controls.is_paused) return 1; // G_SOURCE_CONTINUE (but don't decrement)

    controls.countdown_seconds -= 1;

    if (controls.countdown_seconds <= 0) {
        controls.countdown_source = 0;
        controls.transitionToNextEpisode();
        return 0; // G_SOURCE_REMOVE
    }

    // Update countdown label
    if (controls.countdown_label) |label| {
        var buf: [32]u8 = undefined;
        const text = std.fmt.bufPrintZ(&buf, "Playing in {d}s...", .{controls.countdown_seconds}) catch return 1;
        c.gtk_label_set_text(@ptrCast(label), text.ptr);
    }

    return 1; // G_SOURCE_CONTINUE
}

fn onPlayNowClicked(_: *c.GtkButton, _: ?*anyopaque) callconv(.c) void {
    const controls = global_controls orelse return;
    if (controls.countdown_source != 0) {
        _ = c.g_source_remove(controls.countdown_source);
        controls.countdown_source = 0;
    }
    controls.transitionToNextEpisode();
}

fn onCancelAutoPlayClicked(_: *c.GtkButton, _: ?*anyopaque) callconv(.c) void {
    const controls = global_controls orelse return;
    controls.cancelAutoPlay();
}
