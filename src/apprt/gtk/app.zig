const std = @import("std");
const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});
const video_area = @import("video_area.zig");
const player_controls = @import("player_controls.zig");
const keys = @import("keys.zig");
const player_mod = @import("../../core/player.zig");
const database = @import("../../core/database.zig");
const library_mod = @import("../../core/library.zig");
const downloader_mod = @import("../../core/downloader.zig");
const http_mod = @import("../../net/http.zig");
const types = @import("../../core/types.zig");

// View modules
const home_view = @import("home_view.zig");
const movies_view = @import("movies_view.zig");
const tv_shows_view = @import("tv_shows_view.zig");
const other_view = @import("other_view.zig");
const favorites_view = @import("favorites_view.zig");
const files_view = @import("files_view.zig");
const downloads_view = @import("downloads_view.zig");
const settings_view = @import("settings_view.zig");
const detail_view = @import("detail_view.zig");

pub const ViewId = enum(u8) {
    home = 0,
    movies = 1,
    tv_shows = 2,
    other = 3,
    favorites = 4,
    files = 5,
    downloads = 6,
    settings = 7,
    player = 8,
};

const SidebarItem = struct {
    label: [*:0]const u8,
    icon: [*:0]const u8,
    view_id: ViewId,
};

const sidebar_items = [_]SidebarItem{
    .{ .label = "Home", .icon = "user-home-symbolic", .view_id = .home },
    .{ .label = "Movies", .icon = "camera-video-symbolic", .view_id = .movies },
    .{ .label = "TV Shows", .icon = "tv-symbolic", .view_id = .tv_shows },
    .{ .label = "Other", .icon = "folder-videos-symbolic", .view_id = .other },
    .{ .label = "Favorites", .icon = "starred-symbolic", .view_id = .favorites },
    .{ .label = "Files", .icon = "network-server-symbolic", .view_id = .files },
    .{ .label = "Downloads", .icon = "folder-download-symbolic", .view_id = .downloads },
    .{ .label = "Settings", .icon = "emblem-system-symbolic", .view_id = .settings },
};

const AppState = struct {
    player: player_mod.Player,
    window: ?*c.GtkWidget = null,
    video: video_area.VideoArea = .{},
    controls: player_controls.Controls = .{},
    fullscreen: bool = false,
    file_path: ?[]const u8 = null,
    hide_cursor_timeout: c.guint = 0,
    // Sidebar navigation
    content_stack: ?*c.GtkWidget = null,
    sidebar_list: ?*c.GtkWidget = null,
    split_view: ?*c.GtkWidget = null,
    active_view: ViewId = .home,
    direct_play: bool = false,
    detail: ?detail_view.DetailView = null,
    // Data layer
    db: ?*database.Database = null,
    library: ?library_mod.Library = null,
    downloader: ?downloader_mod.Downloader = null,
    http_client: ?http_mod.HttpClient = null,
    allocator: std.mem.Allocator = std.heap.c_allocator,
};

var app_state: AppState = undefined;

var db_path_buf: [512]u8 = undefined;
var db_path_val: ?[*:0]const u8 = null;

fn getDbPath() [*:0]const u8 {
    if (db_path_val) |p| return p;

    const data_dir = std.posix.getenv("XDG_DATA_HOME") orelse blk: {
        const home = std.posix.getenv("HOME") orelse "/tmp";
        break :blk std.fmt.bufPrint(db_path_buf[0..256], "{s}/.local/share", .{home}) catch "/tmp";
    };
    const full = std.fmt.bufPrintZ(&db_path_buf, "{s}/reel/reel.db", .{data_dir}) catch {
        return "/tmp/reel.db";
    };

    // Ensure directory exists
    const dir_end = std.mem.lastIndexOfScalar(u8, full[0..full.len], '/') orelse return full;
    std.fs.cwd().makePath(full[0..dir_end]) catch {};

    db_path_val = full;
    return full;
}

pub fn run(file_path: ?[]const u8) !void {
    app_state = .{
        .player = try player_mod.Player.init(),
        .file_path = file_path,
        .direct_play = file_path != null,
    };

    // Initialize database
    var db = database.Database.open(getDbPath()) catch |err| {
        std.log.err("Failed to open database: {}", .{err});
        // Continue without database - views will show empty state
        app_state.db = null;
        app_state.library = null;
        return run_app();
    };
    app_state.db = &db;
    app_state.library = library_mod.Library.init(app_state.allocator, &db);
    app_state.downloader = downloader_mod.Downloader.init(app_state.allocator, &db);
    app_state.http_client = http_mod.HttpClient.init(app_state.allocator);

    // Start download worker thread
    if (app_state.downloader != null and app_state.http_client != null) {
        app_state.downloader.?.startWorker(&app_state.http_client.?) catch |err| {
            std.log.err("Failed to start download worker: {}", .{err});
        };
    }

    defer {
        if (app_state.downloader != null) app_state.downloader.?.stopWorker();
        if (app_state.http_client != null) app_state.http_client.?.deinit();
        db.close();
    }

    return run_app();
}

fn run_app() !void {
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

    if (app_state.direct_play) {
        // Direct playback mode: skip sidebar, go straight to player
        buildPlayerOnlyLayout(window);
    } else {
        // Full media center mode: sidebar + content
        buildSidebarLayout(window);
    }

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

    // Load file if provided (direct play mode)
    if (app_state.file_path) |path| {
        app_state.player.loadFile(path) catch |err| {
            std.log.err("Failed to load file: {}", .{err});
        };
    }
}

fn buildPlayerOnlyLayout(window: *c.GtkWidget) void {
    const overlay = c.gtk_overlay_new();

    app_state.video = video_area.VideoArea.init(&app_state.player);
    c.gtk_overlay_set_child(@ptrCast(overlay), @ptrCast(app_state.video.widget));

    app_state.controls = player_controls.Controls.init(&app_state.player);
    c.gtk_overlay_add_overlay(@ptrCast(overlay), @ptrCast(app_state.controls.widget));
    c.gtk_widget_set_valign(@ptrCast(app_state.controls.widget), c.GTK_ALIGN_END);

    const vbox = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 0);
    const header = c.adw_header_bar_new();
    c.gtk_box_append(@ptrCast(vbox), @ptrCast(header));
    c.gtk_box_append(@ptrCast(vbox), overlay);
    c.gtk_widget_set_vexpand(overlay, 1);

    c.adw_application_window_set_content(@ptrCast(window), vbox);
}

fn buildSidebarLayout(window: *c.GtkWidget) void {
    // --- Sidebar ---
    const sidebar_box = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 0);

    // Sidebar header
    const sidebar_header = c.adw_header_bar_new();
    c.adw_header_bar_set_show_title(@ptrCast(sidebar_header), 0);
    c.gtk_box_append(@ptrCast(sidebar_box), @ptrCast(sidebar_header));

    // Sidebar list
    const list_box = c.gtk_list_box_new();
    c.gtk_list_box_set_selection_mode(@ptrCast(list_box), c.GTK_SELECTION_SINGLE);
    c.gtk_widget_add_css_class(@ptrCast(list_box), "navigation-sidebar");
    c.gtk_widget_set_vexpand(@ptrCast(list_box), 1);
    app_state.sidebar_list = @ptrCast(list_box);

    // Add sidebar items
    for (sidebar_items, 0..) |item, i| {
        // Add separator before Favorites and Downloads groups
        if (i == 4 or i == 6) {
            const sep = c.gtk_separator_new(c.GTK_ORIENTATION_HORIZONTAL);
            c.gtk_widget_set_margin_top(@ptrCast(sep), 6);
            c.gtk_widget_set_margin_bottom(@ptrCast(sep), 6);
            c.gtk_list_box_append(@ptrCast(list_box), @ptrCast(sep));
        }

        const row_box = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 8);
        c.gtk_widget_set_margin_top(@ptrCast(row_box), 4);
        c.gtk_widget_set_margin_bottom(@ptrCast(row_box), 4);
        c.gtk_widget_set_margin_start(@ptrCast(row_box), 4);
        c.gtk_widget_set_margin_end(@ptrCast(row_box), 4);

        const icon = c.gtk_image_new_from_icon_name(item.icon);
        c.gtk_box_append(@ptrCast(row_box), @ptrCast(icon));

        const label = c.gtk_label_new(item.label);
        c.gtk_box_append(@ptrCast(row_box), @ptrCast(label));

        c.gtk_list_box_append(@ptrCast(list_box), @ptrCast(row_box));
    }

    const scrolled = c.gtk_scrolled_window_new();
    c.gtk_scrolled_window_set_policy(@ptrCast(scrolled), c.GTK_POLICY_NEVER, c.GTK_POLICY_AUTOMATIC);
    c.gtk_scrolled_window_set_child(@ptrCast(scrolled), @ptrCast(list_box));
    c.gtk_box_append(@ptrCast(sidebar_box), scrolled);

    // --- Content area ---
    const content_box = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 0);

    const content_header = c.adw_header_bar_new();
    c.gtk_box_append(@ptrCast(content_box), @ptrCast(content_header));

    const stack = c.gtk_stack_new();
    c.gtk_stack_set_transition_type(@ptrCast(stack), c.GTK_STACK_TRANSITION_TYPE_CROSSFADE);
    c.gtk_stack_set_transition_duration(@ptrCast(stack), 150);
    c.gtk_widget_set_vexpand(@ptrCast(stack), 1);
    c.gtk_widget_set_hexpand(@ptrCast(stack), 1);
    app_state.content_stack = @ptrCast(stack);

    // Add view pages to stack
    const hv = home_view.HomeView.init();
    _ = c.gtk_stack_add_named(@ptrCast(stack), @ptrCast(@alignCast(hv.widget)), "home");

    const mv = movies_view.MoviesView.init();
    _ = c.gtk_stack_add_named(@ptrCast(stack), @ptrCast(@alignCast(mv.widget)), "movies");

    const tv = tv_shows_view.TVShowsView.init();
    _ = c.gtk_stack_add_named(@ptrCast(stack), @ptrCast(@alignCast(tv.widget)), "tv_shows");

    const ov = other_view.OtherView.init();
    _ = c.gtk_stack_add_named(@ptrCast(stack), @ptrCast(@alignCast(ov.widget)), "other");

    const fav = favorites_view.FavoritesView.init();
    _ = c.gtk_stack_add_named(@ptrCast(stack), @ptrCast(@alignCast(fav.widget)), "favorites");

    const fv = files_view.FilesView.init();
    _ = c.gtk_stack_add_named(@ptrCast(stack), @ptrCast(@alignCast(fv.widget)), "files");

    const dv = downloads_view.DownloadsView.init();
    _ = c.gtk_stack_add_named(@ptrCast(stack), @ptrCast(@alignCast(dv.widget)), "downloads");

    const sv = settings_view.SettingsView.init();
    _ = c.gtk_stack_add_named(@ptrCast(stack), @ptrCast(@alignCast(sv.widget)), "settings");

    // Detail page (shown when clicking a poster)
    app_state.detail = detail_view.DetailView.init();
    _ = c.gtk_stack_add_named(@ptrCast(stack), @ptrCast(@alignCast(app_state.detail.?.widget)), "detail");
    detail_view.setGlobalDetail(&app_state.detail.?);

    // Player page (hidden by default, shown during playback)
    const player_overlay = c.gtk_overlay_new();
    app_state.video = video_area.VideoArea.init(&app_state.player);
    c.gtk_overlay_set_child(@ptrCast(player_overlay), @ptrCast(app_state.video.widget));
    app_state.controls = player_controls.Controls.init(&app_state.player);
    c.gtk_overlay_add_overlay(@ptrCast(player_overlay), @ptrCast(app_state.controls.widget));
    c.gtk_widget_set_valign(@ptrCast(app_state.controls.widget), c.GTK_ALIGN_END);
    _ = c.gtk_stack_add_named(@ptrCast(stack), @ptrCast(player_overlay), "player");

    c.gtk_box_append(@ptrCast(content_box), @ptrCast(stack));

    // Set initial visible page
    c.gtk_stack_set_visible_child_name(@ptrCast(stack), "home");

    // --- Split view ---
    const split_view = c.adw_overlay_split_view_new();
    c.adw_overlay_split_view_set_sidebar(@ptrCast(split_view), @ptrCast(sidebar_box));
    c.adw_overlay_split_view_set_content(@ptrCast(split_view), @ptrCast(content_box));
    c.adw_overlay_split_view_set_min_sidebar_width(@ptrCast(split_view), 200);
    c.adw_overlay_split_view_set_max_sidebar_width(@ptrCast(split_view), 260);
    c.adw_overlay_split_view_set_collapsed(@ptrCast(split_view), 0);
    app_state.split_view = @ptrCast(split_view);

    c.adw_application_window_set_content(@ptrCast(window), @ptrCast(split_view));

    // Connect sidebar selection signal
    _ = c.g_signal_connect_data(
        @ptrCast(list_box),
        "row-selected",
        @ptrCast(&onSidebarRowSelected),
        null,
        null,
        c.G_CONNECT_DEFAULT,
    );

    // Select the first actual item row (skip separators)
    const first_row = c.gtk_list_box_get_row_at_index(@ptrCast(list_box), 0);
    if (first_row) |row| {
        c.gtk_list_box_select_row(@ptrCast(list_box), row);
    }
}

fn onSidebarRowSelected(_: *c.GtkListBox, row: ?*c.GtkListBoxRow, _: ?*anyopaque) callconv(.c) void {
    const r = row orelse return;
    const stack: *c.GtkStack = @ptrCast(app_state.content_stack orelse return);

    const index = c.gtk_list_box_row_get_index(r);
    if (index < 0) return;

    // Map row index to view, accounting for separator rows.
    // Rows: 0=Home, 1=Movies, 2=TV Shows, 3=Other, 4=sep, 5=Favorites, 6=Files, 7=sep, 8=Downloads, 9=Settings
    // GtkListBox separator rows are not selectable, so this callback
    // won't fire for them. But their indices still count.
    const view_name: [*:0]const u8 = switch (index) {
        0 => "home",
        1 => "movies",
        2 => "tv_shows",
        3 => "other",
        // 4 = separator
        5 => "favorites",
        6 => "files",
        // 7 = separator
        8 => "downloads",
        9 => "settings",
        else => return,
    };

    c.gtk_stack_set_visible_child_name(stack, view_name);
}

fn onMotion(_: *c.GtkEventControllerMotion, _: f64, _: f64, _: ?*anyopaque) callconv(.c) void {
    if (app_state.active_view == .player or app_state.direct_play) {
        app_state.controls.show();
        app_state.controls.scheduleHide();
    }
}

pub fn toggleFullscreen() void {
    const window: *c.GtkWindow = @ptrCast(app_state.window orelse return);
    app_state.fullscreen = !app_state.fullscreen;
    if (app_state.fullscreen) {
        c.gtk_window_fullscreen(window);
        // Hide sidebar in fullscreen
        if (app_state.split_view) |sv| {
            c.adw_overlay_split_view_set_collapsed(@ptrCast(sv), 1);
        }
    } else {
        c.gtk_window_unfullscreen(window);
        if (app_state.split_view) |sv| {
            c.adw_overlay_split_view_set_collapsed(@ptrCast(sv), 0);
        }
    }
}

pub fn isFullscreen() bool {
    return app_state.fullscreen;
}

pub fn getLibrary() ?*library_mod.Library {
    if (app_state.library != null) {
        return &app_state.library.?;
    }
    return null;
}

pub fn getAllocator() std.mem.Allocator {
    return app_state.allocator;
}

pub fn getDownloader() ?*downloader_mod.Downloader {
    if (app_state.downloader != null) {
        return &app_state.downloader.?;
    }
    return null;
}

var download_dir_buf: [512]u8 = undefined;
var download_dir_val: ?[]const u8 = null;

pub fn getDownloadDir() []const u8 {
    if (download_dir_val) |d| return d;

    const data_dir = std.posix.getenv("XDG_DATA_HOME") orelse blk: {
        const home = std.posix.getenv("HOME") orelse "/tmp";
        break :blk std.fmt.bufPrint(download_dir_buf[0..256], "{s}/.local/share", .{home}) catch "/tmp";
    };
    const dir = std.fmt.bufPrint(&download_dir_buf, "{s}/reel/downloads", .{data_dir}) catch "/tmp/reel/downloads";
    std.fs.cwd().makePath(dir) catch {};
    download_dir_val = dir;
    return dir;
}

/// Enqueue a download for a Plex media item.
pub fn enqueueDownload(media_item_id: i64) void {
    var dl = getDownloader() orelse return;
    var lib = getLibrary() orelse return;

    const item = lib.getMediaItem(media_item_id) catch return orelse return;
    defer lib.freeMediaItem(item);

    if (item.source != .plex) return;

    // Build filename: {id}_{sanitized_title}.{ext}
    var filename_buf: [256]u8 = undefined;
    const ext = if (item.file_path) |fp| blk: {
        if (std.mem.lastIndexOfScalar(u8, fp, '.')) |dot| break :blk fp[dot..];
        break :blk ".mkv";
    } else ".mkv";

    const filename = std.fmt.bufPrint(&filename_buf, "{d}_{s}{s}", .{
        item.id,
        if (item.title.len > 80) item.title[0..80] else item.title,
        ext,
    }) catch return;

    _ = dl.enqueue(.{
        .media_item_id = media_item_id,
        .server_id = item.server_id orelse "",
        .source_url = item.file_path orelse "", // Plex part key used as URL base
        .download_dir = getDownloadDir(),
        .filename = filename,
        .part_key = item.file_path, // Store the Plex part key
    }) catch |err| {
        switch (err) {
            error.AlreadyExists => std.log.info("Download already exists for item {d}", .{media_item_id}),
            else => std.log.err("Failed to enqueue download: {}", .{err}),
        }
    };
}

pub fn switchToPlayer(file_path: []const u8) void {
    const stack: *c.GtkStack = @ptrCast(app_state.content_stack orelse return);
    app_state.active_view = .player;
    c.gtk_stack_set_visible_child_name(stack, "player");

    app_state.player.loadFile(file_path) catch |err| {
        std.log.err("Failed to load file: {}", .{err});
    };
    app_state.controls.show();
    app_state.controls.scheduleHide();
}

/// Play a media item, preferring a completed local download over streaming.
pub fn playMediaItem(item_id: i64, streaming_path: []const u8) void {
    // Check for completed local download first
    if (getDownloader()) |dl| {
        if (dl.getCompletedLocalPath(item_id) catch null) |local_path| {
            defer app_state.allocator.free(local_path);
            // Verify the file actually exists on disk
            std.fs.cwd().access(local_path, .{}) catch {
                // File deleted externally, mark as failed
                if (dl.getByMediaItemId(item_id) catch null) |download| {
                    defer dl.freeDownload(download);
                    dl.setFailed(download.id, "File not found") catch {};
                }
                // Fall through to streaming
                switchToPlayer(streaming_path);
                return;
            };
            switchToPlayer(local_path);
            return;
        }
    }
    switchToPlayer(streaming_path);
}

pub fn switchToView(view_name: [*:0]const u8) void {
    const stack: *c.GtkStack = @ptrCast(app_state.content_stack orelse return);
    c.gtk_stack_set_visible_child_name(stack, view_name);
}

pub fn showDetail(item_id: i64) void {
    var lib = getLibrary() orelse return;
    const item = lib.getMediaItem(item_id) catch return orelse return;
    defer lib.freeMediaItem(item);

    if (app_state.detail) |*detail| {
        detail.showItem(item);
    }

    const stack: *c.GtkStack = @ptrCast(app_state.content_stack orelse return);
    app_state.active_view = .home; // track previous
    c.gtk_stack_set_visible_child_name(stack, "detail");
}
