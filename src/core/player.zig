const std = @import("std");
const c = @cImport({
    @cInclude("mpv/client.h");
    @cInclude("mpv/render.h");
    @cInclude("mpv/render_gl.h");
    @cInclude("epoxy/gl.h");
});

pub const Event = union(enum) {
    file_loaded,
    end_file: EndFileReason,
    property_change: PropertyChange,
    log_message: LogMessage,
    idle,
    shutdown,
    unknown,
};

pub const LogMessage = struct {
    prefix: []const u8,
    text: []const u8,
    level: []const u8,
};

pub const EndFileReason = enum {
    eof,
    stop,
    quit,
    @"error",
    redirect,
    unknown,
};

pub const PropertyChange = union(enum) {
    time_pos: ?f64,
    duration: ?f64,
    pause: bool,
    volume: f64,
    mute: bool,
    eof_reached: bool,
    paused_for_cache: bool,
    track_list: void,
};

pub const Player = struct {
    handle: *c.mpv_handle,
    render_ctx: ?*c.mpv_render_context = null,
    update_callback: ?*const fn (?*anyopaque) callconv(.c) void = null,
    update_callback_data: ?*anyopaque = null,

    pub fn init() !Player {
        const handle = c.mpv_create() orelse return error.MpvCreateFailed;

        // Set default options before initialize
        try setOptionString(handle, "hwdec", "auto-safe");
        try setOptionString(handle, "vo", "libmpv");
        try setOptionString(handle, "keep-open", "yes");
        try setOptionString(handle, "idle", "yes");

        if (c.mpv_initialize(handle) < 0) {
            c.mpv_destroy(handle);
            return error.MpvInitFailed;
        }

        // Enable mpv log messages at "error" level for diagnostics
        _ = c.mpv_request_log_messages(handle, "warn");

        var self = Player{ .handle = handle };

        // Observe properties we care about
        try self.observeProperty("time-pos", c.MPV_FORMAT_DOUBLE);
        try self.observeProperty("duration", c.MPV_FORMAT_DOUBLE);
        try self.observeProperty("pause", c.MPV_FORMAT_FLAG);
        try self.observeProperty("volume", c.MPV_FORMAT_DOUBLE);
        try self.observeProperty("mute", c.MPV_FORMAT_FLAG);
        try self.observeProperty("eof-reached", c.MPV_FORMAT_FLAG);
        try self.observeProperty("paused-for-cache", c.MPV_FORMAT_FLAG);

        return self;
    }

    pub fn deinit(self: *Player) void {
        if (self.render_ctx) |ctx| {
            c.mpv_render_context_free(ctx);
            self.render_ctx = null;
        }
        c.mpv_terminate_destroy(self.handle);
    }

    pub fn initRender(self: *Player, get_proc_address: *const fn (?*anyopaque, [*c]const u8) callconv(.c) ?*anyopaque) !void {
        var gl_init_params = c.mpv_opengl_init_params{
            .get_proc_address = get_proc_address,
            .get_proc_address_ctx = null,
        };
        var params = [_]c.mpv_render_param{
            .{ .type = c.MPV_RENDER_PARAM_API_TYPE, .data = @constCast(@ptrCast(c.MPV_RENDER_API_TYPE_OPENGL)) },
            .{ .type = c.MPV_RENDER_PARAM_OPENGL_INIT_PARAMS, .data = @ptrCast(&gl_init_params) },
            .{ .type = c.MPV_RENDER_PARAM_INVALID, .data = null },
        };

        var render_ctx: ?*c.mpv_render_context = null;
        const err = c.mpv_render_context_create(&render_ctx, self.handle, &params);
        if (err < 0) return error.RenderContextCreateFailed;
        self.render_ctx = render_ctx;
    }

    pub fn setRenderUpdateCallback(self: *Player, callback: *const fn (?*anyopaque) callconv(.c) void, data: ?*anyopaque) void {
        if (self.render_ctx) |ctx| {
            self.update_callback = callback;
            self.update_callback_data = data;
            c.mpv_render_context_set_update_callback(ctx, callback, data);
        }
    }

    pub fn render(self: *Player, fbo_id: c_int, width: c_int, height: c_int) void {
        const ctx = self.render_ctx orelse return;
        var fbo = c.mpv_opengl_fbo{
            .fbo = fbo_id,
            .w = width,
            .h = height,
            .internal_format = 0,
        };
        var flip: c_int = 1;
        var params = [_]c.mpv_render_param{
            .{ .type = c.MPV_RENDER_PARAM_OPENGL_FBO, .data = @ptrCast(&fbo) },
            .{ .type = c.MPV_RENDER_PARAM_FLIP_Y, .data = @ptrCast(&flip) },
            .{ .type = c.MPV_RENDER_PARAM_INVALID, .data = null },
        };
        _ = c.mpv_render_context_render(ctx, &params);
    }

    pub fn loadFile(self: *Player, path: []const u8) !void {
        const path_z = try std.heap.c_allocator.dupeZ(u8, path);
        defer std.heap.c_allocator.free(path_z);
        const cmd = [_:null]?[*:0]const u8{ "loadfile", path_z, null };
        const err = c.mpv_command(self.handle, @constCast(@ptrCast(&cmd)));
        if (err < 0) return error.CommandFailed;
    }

    pub fn stop(self: *Player) !void {
        const cmd = [_:null]?[*:0]const u8{ "stop", null };
        const err = c.mpv_command(self.handle, @constCast(@ptrCast(&cmd)));
        if (err < 0) return error.CommandFailed;
    }

    pub fn togglePause(self: *Player) !void {
        const cmd = [_:null]?[*:0]const u8{ "cycle", "pause", null };
        const err = c.mpv_command(self.handle, @constCast(@ptrCast(&cmd)));
        if (err < 0) return error.CommandFailed;
    }

    pub fn seek(self: *Player, seconds: f64) !void {
        var buf: [32]u8 = undefined;
        const str = std.fmt.bufPrintZ(&buf, "{d:.1}", .{seconds}) catch return error.FormatFailed;
        const cmd = [_:null]?[*:0]const u8{ "seek", str, "relative", null };
        const err = c.mpv_command(self.handle, @constCast(@ptrCast(&cmd)));
        if (err < 0) return error.CommandFailed;
    }

    pub fn seekAbsolute(self: *Player, seconds: f64) !void {
        var buf: [32]u8 = undefined;
        const str = std.fmt.bufPrintZ(&buf, "{d:.1}", .{seconds}) catch return error.FormatFailed;
        const cmd = [_:null]?[*:0]const u8{ "seek", str, "absolute", null };
        const err = c.mpv_command(self.handle, @constCast(@ptrCast(&cmd)));
        if (err < 0) return error.CommandFailed;
    }

    pub fn setVolume(self: *Player, volume: f64) !void {
        var vol = volume;
        const err = c.mpv_set_property(self.handle, "volume", c.MPV_FORMAT_DOUBLE, @ptrCast(&vol));
        if (err < 0) return error.SetPropertyFailed;
    }

    pub fn toggleMute(self: *Player) !void {
        const cmd = [_:null]?[*:0]const u8{ "cycle", "mute", null };
        const err = c.mpv_command(self.handle, @constCast(@ptrCast(&cmd)));
        if (err < 0) return error.CommandFailed;
    }

    pub fn cycleSub(self: *Player) !void {
        const cmd = [_:null]?[*:0]const u8{ "cycle", "sub", null };
        const err = c.mpv_command(self.handle, @constCast(@ptrCast(&cmd)));
        if (err < 0) return error.CommandFailed;
    }

    pub fn cycleAudio(self: *Player) !void {
        const cmd = [_:null]?[*:0]const u8{ "cycle", "audio", null };
        const err = c.mpv_command(self.handle, @constCast(@ptrCast(&cmd)));
        if (err < 0) return error.CommandFailed;
    }

    // ── Chapter Navigation ──────────────────────────────────

    pub fn getChapterCount(self: *Player) i32 {
        var count: i64 = 0;
        _ = c.mpv_get_property(self.handle, "chapter-list/count", c.MPV_FORMAT_INT64, @ptrCast(&count));
        return @intCast(count);
    }

    pub fn nextChapter(self: *Player) !void {
        const cmd = [_:null]?[*:0]const u8{ "add", "chapter", "1", null };
        const err = c.mpv_command(self.handle, @constCast(@ptrCast(&cmd)));
        if (err < 0) return error.CommandFailed;
    }

    pub fn prevChapter(self: *Player) !void {
        const cmd = [_:null]?[*:0]const u8{ "add", "chapter", "-1", null };
        const err = c.mpv_command(self.handle, @constCast(@ptrCast(&cmd)));
        if (err < 0) return error.CommandFailed;
    }

    // ── Playback Speed ──────────────────────────────────────

    pub fn setSpeed(self: *Player, speed: f64) !void {
        var s = speed;
        const err = c.mpv_set_property(self.handle, "speed", c.MPV_FORMAT_DOUBLE, @ptrCast(&s));
        if (err < 0) return error.SetPropertyFailed;
    }

    pub fn getSpeed(self: *Player) f64 {
        var speed: f64 = 1.0;
        _ = c.mpv_get_property(self.handle, "speed", c.MPV_FORMAT_DOUBLE, @ptrCast(&speed));
        return speed;
    }

    // ── Subtitle Loading ────────────────────────────────────

    pub fn loadSubtitleFile(self: *Player, path: []const u8) !void {
        const path_z = try std.heap.c_allocator.dupeZ(u8, path);
        defer std.heap.c_allocator.free(path_z);
        const cmd = [_:null]?[*:0]const u8{ "sub-add", path_z, null };
        const err = c.mpv_command(self.handle, @constCast(@ptrCast(&cmd)));
        if (err < 0) return error.CommandFailed;
    }

    pub fn getSubtitleTrackCount(self: *Player) i32 {
        var count: i64 = 0;
        _ = c.mpv_get_property(self.handle, "track-list/count", c.MPV_FORMAT_INT64, @ptrCast(&count));
        // Count only subtitle tracks
        var sub_count: i32 = 0;
        var i: i64 = 0;
        while (i < count) : (i += 1) {
            var key_buf: [64]u8 = undefined;
            const key = std.fmt.bufPrintZ(&key_buf, "track-list/{d}/type", .{i}) catch continue;
            var type_val: [*c]const u8 = null;
            _ = c.mpv_get_property(self.handle, key.ptr, c.MPV_FORMAT_STRING, @ptrCast(&type_val));
            if (type_val) |tv| {
                if (std.mem.eql(u8, std.mem.span(tv), "sub")) {
                    sub_count += 1;
                }
                c.mpv_free(@constCast(tv));
            }
        }
        return sub_count;
    }

    pub fn setSubtitleTrack(self: *Player, track_id: i64) !void {
        var id = track_id;
        const err = c.mpv_set_property(self.handle, "sid", c.MPV_FORMAT_INT64, @ptrCast(&id));
        if (err < 0) return error.SetPropertyFailed;
    }

    pub fn disableSubtitles(self: *Player) !void {
        const err = c.mpv_set_property_string(self.handle, "sid", "no");
        if (err < 0) return error.SetPropertyFailed;
    }

    /// Scan for external subtitle files in the same directory as the video.
    /// Only works for local file paths (not HTTP URLs).
    pub fn scanAndLoadExternalSubs(self: *Player, video_path: []const u8) void {
        // Skip HTTP streams
        if (std.mem.startsWith(u8, video_path, "http://") or
            std.mem.startsWith(u8, video_path, "https://"))
            return;

        // Extract directory and filename stem
        const dir_end = std.mem.lastIndexOfScalar(u8, video_path, '/') orelse return;
        const dir_path = video_path[0 .. dir_end + 1];
        const filename = video_path[dir_end + 1 ..];
        const dot_idx = std.mem.lastIndexOfScalar(u8, filename, '.') orelse return;
        const stem = filename[0..dot_idx];

        const sub_exts = [_][]const u8{ ".srt", ".ass", ".ssa", ".sub", ".idx", ".vtt" };

        // Open directory and scan
        const sio = std.Io.Threaded.global_single_threaded.io();
        var dir = std.Io.Dir.cwd().openDir(sio, dir_path, .{ .iterate = true }) catch return;
        defer dir.close(sio);

        var iter = dir.iterate();
        while (iter.next(sio) catch null) |entry| {
            if (entry.kind != .file) continue;
            const name = entry.name;

            // Check if extension matches a subtitle format
            var is_sub = false;
            for (sub_exts) |ext| {
                if (name.len > ext.len and
                    std.ascii.eqlIgnoreCase(name[name.len - ext.len ..], ext))
                {
                    is_sub = true;
                    break;
                }
            }
            if (!is_sub) continue;

            // Check if filename starts with the video stem (case-insensitive)
            if (name.len < stem.len) continue;
            if (!std.ascii.eqlIgnoreCase(name[0..stem.len], stem)) continue;

            // The character after the stem must be '.' (e.g., movie.en.srt, movie.srt)
            // or the stem IS the full name before extension
            if (name.len > stem.len and name[stem.len] != '.') continue;

            // Build full path and load
            var path_buf: [1024]u8 = undefined;
            const full_path = std.fmt.bufPrint(&path_buf, "{s}{s}", .{ dir_path, name }) catch continue;
            self.loadSubtitleFile(full_path) catch {};
        }
    }

    // ── Subtitle Appearance ─────────────────────────────────

    pub fn setSubFont(self: *Player, font: [*:0]const u8) void {
        _ = c.mpv_set_property_string(self.handle, "sub-font", font);
    }

    pub fn setSubFontSize(self: *Player, size: i64) void {
        var s = size;
        _ = c.mpv_set_property(self.handle, "sub-font-size", c.MPV_FORMAT_INT64, @ptrCast(&s));
    }

    pub fn setSubColor(self: *Player, color: [*:0]const u8) void {
        _ = c.mpv_set_property_string(self.handle, "sub-color", color);
    }

    pub fn setSubBorderColor(self: *Player, color: [*:0]const u8) void {
        _ = c.mpv_set_property_string(self.handle, "sub-border-color", color);
    }

    pub fn setSubBorderSize(self: *Player, size: f64) void {
        var s = size;
        _ = c.mpv_set_property(self.handle, "sub-border-size", c.MPV_FORMAT_DOUBLE, @ptrCast(&s));
    }

    pub fn setSubBackColor(self: *Player, color: [*:0]const u8) void {
        _ = c.mpv_set_property_string(self.handle, "sub-back-color", color);
    }

    pub fn setSubPos(self: *Player, pos: i64) void {
        var p = pos;
        _ = c.mpv_set_property(self.handle, "sub-pos", c.MPV_FORMAT_INT64, @ptrCast(&p));
    }

    pub fn waitEvent(self: *Player, timeout: f64) Event {
        const ev = c.mpv_wait_event(self.handle, timeout);
        return switch (ev.*.event_id) {
            c.MPV_EVENT_FILE_LOADED => .file_loaded,
            c.MPV_EVENT_END_FILE => blk: {
                if (ev.*.data) |data| {
                    const end: *c.mpv_event_end_file = @ptrCast(@alignCast(data));
                    break :blk .{ .end_file = switch (end.reason) {
                        c.MPV_END_FILE_REASON_EOF => .eof,
                        c.MPV_END_FILE_REASON_STOP => .stop,
                        c.MPV_END_FILE_REASON_QUIT => .quit,
                        c.MPV_END_FILE_REASON_ERROR => .@"error",
                        c.MPV_END_FILE_REASON_REDIRECT => .redirect,
                        else => .unknown,
                    } };
                }
                break :blk .{ .end_file = .unknown };
            },
            c.MPV_EVENT_PROPERTY_CHANGE => blk: {
                if (ev.*.data) |data| {
                    const prop: *c.mpv_event_property = @ptrCast(@alignCast(data));
                    const name = std.mem.span(prop.name);
                    break :blk .{ .property_change = parsePropertyChange(name, prop) };
                }
                break :blk .unknown;
            },
            c.MPV_EVENT_LOG_MESSAGE => blk: {
                if (ev.*.data) |data| {
                    const msg: *c.mpv_event_log_message = @ptrCast(@alignCast(data));
                    break :blk .{ .log_message = .{
                        .prefix = if (msg.prefix) |p| std.mem.span(p) else "",
                        .text = if (msg.text) |t| std.mem.span(t) else "",
                        .level = if (msg.level) |l| std.mem.span(l) else "",
                    } };
                }
                break :blk .unknown;
            },
            c.MPV_EVENT_IDLE => .idle,
            c.MPV_EVENT_SHUTDOWN => .shutdown,
            else => .unknown,
        };
    }

    fn parsePropertyChange(name: []const u8, prop: *c.mpv_event_property) PropertyChange {
        if (std.mem.eql(u8, name, "time-pos")) {
            return .{ .time_pos = getDouble(prop) };
        } else if (std.mem.eql(u8, name, "duration")) {
            return .{ .duration = getDouble(prop) };
        } else if (std.mem.eql(u8, name, "pause")) {
            return .{ .pause = getFlag(prop) };
        } else if (std.mem.eql(u8, name, "volume")) {
            return .{ .volume = getDouble(prop) orelse 100.0 };
        } else if (std.mem.eql(u8, name, "mute")) {
            return .{ .mute = getFlag(prop) };
        } else if (std.mem.eql(u8, name, "eof-reached")) {
            return .{ .eof_reached = getFlag(prop) };
        } else if (std.mem.eql(u8, name, "paused-for-cache")) {
            return .{ .paused_for_cache = getFlag(prop) };
        }
        return .{ .time_pos = null };
    }

    fn getDouble(prop: *c.mpv_event_property) ?f64 {
        if (prop.format == c.MPV_FORMAT_DOUBLE) {
            if (prop.data) |data| {
                const val: *f64 = @ptrCast(@alignCast(data));
                return val.*;
            }
        }
        return null;
    }

    fn getFlag(prop: *c.mpv_event_property) bool {
        if (prop.format == c.MPV_FORMAT_FLAG) {
            if (prop.data) |data| {
                const val: *c_int = @ptrCast(@alignCast(data));
                return val.* != 0;
            }
        }
        return false;
    }

    pub fn getVideoSize(self: *Player) struct { width: i64, height: i64 } {
        var w: i64 = 0;
        var h: i64 = 0;
        _ = c.mpv_get_property(self.handle, "dwidth", c.MPV_FORMAT_INT64, @ptrCast(&w));
        _ = c.mpv_get_property(self.handle, "dheight", c.MPV_FORMAT_INT64, @ptrCast(&h));
        return .{ .width = w, .height = h };
    }

    fn observeProperty(self: *Player, name: [*:0]const u8, format: c_int) !void {
        const err = c.mpv_observe_property(self.handle, 0, name, @intCast(format));
        if (err < 0) return error.ObservePropertyFailed;
    }

    fn setOptionString(handle: *c.mpv_handle, name: [*:0]const u8, value: [*:0]const u8) !void {
        const err = c.mpv_set_option_string(handle, name, value);
        if (err < 0) return error.SetOptionFailed;
    }
};

test "Player types compile" {
    // Verify our types are properly defined
    const e: Event = .idle;
    _ = e;
    const p: PropertyChange = .{ .pause = false };
    _ = p;
}
