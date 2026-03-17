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
const image_cache_mod = @import("../../core/image_cache.zig");
const types = @import("../../core/types.zig");
const tmdb_client_mod = @import("../../net/tmdb/client.zig");
const settings_mod = @import("../../core/settings.zig");

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
const collections_view = @import("collections_view.zig");
const plex_setup = @import("plex_setup.zig");

pub const ViewId = enum(u8) {
    home = 0,
    movies = 1,
    tv_shows = 2,
    other = 3,
    favorites = 4,
    collections = 5,
    files = 6,
    downloads = 7,
    settings = 8,
    player = 9,
};

const SidebarItem = struct {
    label: [*:0]const u8,
    icon: [*:0]const u8,
    tag: [*:0]const u8,
    view_id: ViewId,
};

// Page indices for nav_pages array (detail and player are beyond sidebar_items)
const detail_page_idx: usize = sidebar_items.len;
const player_page_idx: usize = sidebar_items.len + 1;

const sidebar_items = [_]SidebarItem{
    .{ .label = "Home", .icon = "user-home-symbolic", .tag = "home", .view_id = .home },
    .{ .label = "Movies", .icon = "camera-video-symbolic", .tag = "movies", .view_id = .movies },
    .{ .label = "TV Shows", .icon = "tv-symbolic", .tag = "tv_shows", .view_id = .tv_shows },
    .{ .label = "Other", .icon = "folder-videos-symbolic", .tag = "other", .view_id = .other },
    .{ .label = "Favorites", .icon = "starred-symbolic", .tag = "favorites", .view_id = .favorites },
    .{ .label = "Collections", .icon = "view-grid-symbolic", .tag = "collections", .view_id = .collections },
    .{ .label = "Files", .icon = "network-server-symbolic", .tag = "files", .view_id = .files },
    .{ .label = "Downloads", .icon = "folder-download-symbolic", .tag = "downloads", .view_id = .downloads },
    .{ .label = "Settings", .icon = "emblem-system-symbolic", .tag = "settings", .view_id = .settings },
};

const AppState = struct {
    player: player_mod.Player,
    window: ?*c.GtkWidget = null,
    video: video_area.VideoArea = .{},
    controls: player_controls.Controls = .{},
    fullscreen: bool = false,
    file_path: ?[]const u8 = null,
    hide_cursor_timeout: c.guint = 0,
    // Navigation
    nav_view: ?*c.GtkWidget = null,
    sidebar_list: ?*c.GtkWidget = null,
    split_view: ?*c.GtkWidget = null,
    active_view: ViewId = .home,
    // Per-view navigation pages (singletons for replace())
    nav_pages: [sidebar_items.len + 2]?*c.GtkWidget = .{null} ** (sidebar_items.len + 2), // +2 for detail, player
    direct_play: bool = false,
    // View instances (singletons, created once)
    home: ?home_view.HomeView = null,
    movies: ?movies_view.MoviesView = null,
    tv_shows: ?tv_shows_view.TVShowsView = null,
    other: ?other_view.OtherView = null,
    favorites: ?favorites_view.FavoritesView = null,
    collections: ?collections_view.CollectionsView = null,
    files: ?files_view.FilesView = null,
    downloads: ?downloads_view.DownloadsView = null,
    detail: ?detail_view.DetailView = null,
    // Data layer
    db: ?*database.Database = null,
    library: ?library_mod.Library = null,
    downloader: ?downloader_mod.Downloader = null,
    http_client: ?http_mod.HttpClient = null,
    image_cache: ?image_cache_mod.ImageCache = null,
    tmdb_client: ?tmdb_client_mod.TmdbClient = null,
    settings: ?settings_mod.Settings = null,
    allocator: std.mem.Allocator = std.heap.c_allocator,
};

var app_state: AppState = undefined;

var detail_title_buf: [256]u8 = undefined;
var db_path_buf: [512]u8 = undefined;
var data_dir_buf: [256]u8 = undefined;
var db_path_val: ?[*:0]const u8 = null;

fn getDbPath() [*:0]const u8 {
    if (db_path_val) |p| return p;

    const data_dir = std.posix.getenv("XDG_DATA_HOME") orelse blk: {
        const home = std.posix.getenv("HOME") orelse "/tmp";
        break :blk std.fmt.bufPrint(&data_dir_buf, "{s}/.local/share", .{home}) catch "/tmp";
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
    app_state.image_cache = image_cache_mod.ImageCache.init(app_state.allocator, &db, getImageCacheDir());
    app_state.settings = settings_mod.Settings.init(app_state.allocator, &db);

    // Initialize TMDB client if API key is configured
    if (app_state.settings) |*settings| {
        if (settings.getString(settings_mod.keys.tmdb_api_key) catch null) |api_key| {
            if (api_key.len > 0) {
                app_state.tmdb_client = tmdb_client_mod.TmdbClient.init(
                    app_state.allocator,
                    &app_state.http_client.?,
                    api_key,
                );
            } else {
                app_state.allocator.free(api_key);
            }
        }
    }

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

    // Refresh server connections and sync libraries in background
    if (!app_state.direct_play) {
        plex_setup.refreshServerConnections();
        plex_setup.syncAllInBackground();
    }

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
    app_state.controls.startPolling();
    c.gtk_overlay_add_overlay(@ptrCast(overlay), @ptrCast(app_state.controls.widget));
    c.gtk_widget_set_valign(@ptrCast(app_state.controls.widget), c.GTK_ALIGN_END);

    const vbox = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 0);
    const header = c.adw_header_bar_new();
    c.gtk_box_append(@ptrCast(vbox), @ptrCast(header));
    c.gtk_box_append(@ptrCast(vbox), overlay);
    c.gtk_widget_set_vexpand(overlay, 1);

    c.adw_application_window_set_content(@ptrCast(window), vbox);
}

/// Wrap a view widget in an AdwNavigationPage with its own AdwToolbarView + AdwHeaderBar.
fn makeNavPage(view_widget: *c.GtkWidget, title: [*:0]const u8, tag: [*:0]const u8) *c.GtkWidget {
    const toolbar = c.adw_toolbar_view_new();
    const header = c.adw_header_bar_new();
    c.adw_toolbar_view_add_top_bar(@ptrCast(toolbar), @ptrCast(header));
    c.adw_toolbar_view_set_content(@ptrCast(toolbar), view_widget);
    const page = c.adw_navigation_page_new_with_tag(@ptrCast(toolbar), title, tag);
    return @ptrCast(page);
}

fn buildSidebarLayout(window: *c.GtkWidget) void {
    // --- Sidebar (wrapped in AdwToolbarView for modern header styling) ---
    const sidebar_toolbar = c.adw_toolbar_view_new();

    const sidebar_header = c.adw_header_bar_new();
    c.adw_header_bar_set_show_title(@ptrCast(sidebar_header), 0);
    c.adw_header_bar_set_show_end_title_buttons(@ptrCast(sidebar_header), 0);
    c.adw_toolbar_view_add_top_bar(@ptrCast(sidebar_toolbar), @ptrCast(sidebar_header));

    // Sidebar: single GtkListBox with section headers
    const list_box = c.gtk_list_box_new();
    c.gtk_list_box_set_selection_mode(@ptrCast(list_box), c.GTK_SELECTION_SINGLE);
    c.gtk_widget_add_css_class(@ptrCast(list_box), "navigation-sidebar");
    app_state.sidebar_list = @ptrCast(list_box);

    // Section headers and items — headers are non-selectable
    const sections = [_]struct { label: [*:0]const u8, start: usize, end: usize }{
        .{ .label = "Library", .start = 0, .end = 4 },
        .{ .label = "Personal", .start = 4, .end = 7 },
        .{ .label = "System", .start = 7, .end = 9 },
    };

    // Use header function to mark section headers as non-activatable
    c.gtk_list_box_set_header_func(@ptrCast(list_box), null, null, null);

    for (sections) |section| {
        // Section header (non-selectable label)
        const header_label = c.gtk_label_new(section.label);
        c.gtk_widget_set_halign(@ptrCast(header_label), c.GTK_ALIGN_START);
        c.gtk_widget_add_css_class(@ptrCast(header_label), "dim-label");
        c.gtk_widget_add_css_class(@ptrCast(header_label), "caption");
        c.gtk_widget_set_margin_start(@ptrCast(header_label), 12);
        c.gtk_widget_set_margin_top(@ptrCast(header_label), 12);
        c.gtk_widget_set_margin_bottom(@ptrCast(header_label), 4);
        c.gtk_list_box_append(@ptrCast(list_box), @ptrCast(header_label));

        // Mark header row as non-activatable/non-selectable
        const header_row = c.gtk_widget_get_parent(@ptrCast(header_label));
        if (header_row) |hr| {
            c.gtk_list_box_row_set_activatable(@ptrCast(@alignCast(hr)), 0);
            c.gtk_list_box_row_set_selectable(@ptrCast(@alignCast(hr)), 0);
        }

        // Sidebar item rows with icons
        for (sidebar_items[section.start..section.end]) |item| {
            const row_box = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 8);
            const icon = c.gtk_image_new_from_icon_name(item.icon);
            c.gtk_box_append(@ptrCast(row_box), @ptrCast(icon));
            const label = c.gtk_label_new(item.label);
            c.gtk_box_append(@ptrCast(row_box), @ptrCast(label));
            c.gtk_list_box_append(@ptrCast(list_box), @ptrCast(row_box));
        }
    }

    // Connect row selection
    _ = c.g_signal_connect_data(
        @ptrCast(list_box),
        "row-selected",
        @ptrCast(&onSidebarRowSelected),
        null,
        null,
        c.G_CONNECT_DEFAULT,
    );

    const scrolled = c.gtk_scrolled_window_new();
    c.gtk_scrolled_window_set_policy(@ptrCast(scrolled), c.GTK_POLICY_NEVER, c.GTK_POLICY_AUTOMATIC);
    c.gtk_scrolled_window_set_child(@ptrCast(scrolled), @ptrCast(list_box));
    c.adw_toolbar_view_set_content(@ptrCast(sidebar_toolbar), scrolled);

    // --- Content area: AdwNavigationView ---
    const nav_view = c.adw_navigation_view_new();
    c.gtk_widget_set_vexpand(@ptrCast(nav_view), 1);
    c.gtk_widget_set_hexpand(@ptrCast(nav_view), 1);
    app_state.nav_view = @ptrCast(nav_view);

    // Create all view widgets (stored in AppState as singletons)
    app_state.home = home_view.HomeView.init();
    app_state.movies = movies_view.MoviesView.init();
    app_state.tv_shows = tv_shows_view.TVShowsView.init();
    app_state.other = other_view.OtherView.init();
    app_state.favorites = favorites_view.FavoritesView.init();

    app_state.collections = collections_view.CollectionsView.init();
    collections_view.setGlobalCollectionsView(&app_state.collections.?);

    app_state.files = files_view.FilesView.init();

    app_state.downloads = downloads_view.DownloadsView.init();
    downloads_view.setGlobalDownloadsView(&app_state.downloads.?);

    const sv = settings_view.SettingsView.init();

    app_state.detail = detail_view.DetailView.init();
    detail_view.setGlobalDetail(&app_state.detail.?);

    // Player overlay
    const player_overlay = c.gtk_overlay_new();
    app_state.video = video_area.VideoArea.init(&app_state.player);
    c.gtk_overlay_set_child(@ptrCast(player_overlay), @ptrCast(app_state.video.widget));
    app_state.controls = player_controls.Controls.init(&app_state.player);
    app_state.controls.startPolling();
    c.gtk_overlay_add_overlay(@ptrCast(player_overlay), @ptrCast(app_state.controls.widget));
    c.gtk_widget_set_valign(@ptrCast(app_state.controls.widget), c.GTK_ALIGN_END);

    // Wrap each view in an AdwNavigationPage with its own AdwToolbarView + AdwHeaderBar
    const view_widgets = [sidebar_items.len]*c.GtkWidget{
        @ptrCast(@alignCast(app_state.home.?.widget)),
        @ptrCast(@alignCast(app_state.movies.?.widget)),
        @ptrCast(@alignCast(app_state.tv_shows.?.widget)),
        @ptrCast(@alignCast(app_state.other.?.widget)),
        @ptrCast(@alignCast(app_state.favorites.?.widget)),
        @ptrCast(@alignCast(app_state.collections.?.widget)),
        @ptrCast(@alignCast(app_state.files.?.widget)),
        @ptrCast(@alignCast(app_state.downloads.?.widget)),
        @ptrCast(@alignCast(sv.widget)),
    };

    for (sidebar_items, 0..) |item, i| {
        app_state.nav_pages[i] = makeNavPage(view_widgets[i], item.label, item.tag);
    }

    // Detail page
    app_state.nav_pages[detail_page_idx] = makeNavPage(@ptrCast(@alignCast(app_state.detail.?.widget)), "Detail", "detail");

    // Player page
    app_state.nav_pages[player_page_idx] = makeNavPage(@ptrCast(player_overlay), "Now Playing", "player");

    // Add ALL pages as static to the navigation view.
    // Static pages persist when popped/replaced (dynamic pages are destroyed).
    // Since we reuse pages as singletons, they must be static.
    for (app_state.nav_pages) |maybe_page| {
        if (maybe_page) |page| {
            c.adw_navigation_view_add(@ptrCast(nav_view), @ptrCast(page));
        }
    }

    // Connect `showing` signal on each page to trigger view refresh
    for (0..sidebar_items.len) |i| {
        if (app_state.nav_pages[i]) |page| {
            _ = c.g_signal_connect_data(
                @ptrCast(page),
                "showing",
                @ptrCast(&onPageShowing),
                @ptrFromInt(i),
                null,
                c.G_CONNECT_DEFAULT,
            );
        }
    }

    // Connect popped signal to handle player cleanup
    _ = c.g_signal_connect_data(
        @ptrCast(nav_view),
        "popped",
        @ptrCast(&onNavPagePopped),
        null,
        null,
        c.G_CONNECT_DEFAULT,
    );

    // Note: don't push home page here — the sidebar selection below triggers it

    // --- Split view ---
    const split_view = c.adw_navigation_split_view_new();

    // Wrap sidebar in AdwNavigationPage
    const sidebar_page = c.adw_navigation_page_new(@ptrCast(sidebar_toolbar), "Reel");
    c.adw_navigation_split_view_set_sidebar(@ptrCast(split_view), @ptrCast(sidebar_page));

    // Wrap NavigationView in AdwNavigationPage for content pane
    const content_page = c.adw_navigation_page_new(@ptrCast(nav_view), "Content");
    c.adw_navigation_split_view_set_content(@ptrCast(split_view), @ptrCast(content_page));

    c.adw_navigation_split_view_set_min_sidebar_width(@ptrCast(split_view), 200);
    c.adw_navigation_split_view_set_max_sidebar_width(@ptrCast(split_view), 260);
    app_state.split_view = @ptrCast(split_view);

    c.adw_application_window_set_content(@ptrCast(window), @ptrCast(split_view));

    // Select the first selectable row (index 1 = Home, after section header)
    if (app_state.sidebar_list) |sl| {
        const first_row = c.gtk_list_box_get_row_at_index(@ptrCast(sl), 1);
        if (first_row) |row| {
            c.gtk_list_box_select_row(@ptrCast(sl), row);
        }
    }
}

// Map from GtkListBox row index (including section headers) to sidebar_items index.
// Layout: header(0), items(1-4), header(5), items(6-8), header(9), items(10-11)
const row_to_item = [_]?usize{
    null, 0, 1, 2, 3, // header + Library group
    null, 4, 5, 6, // header + Personal group
    null, 7, 8, // header + System group
};

fn onSidebarRowSelected(_: *c.GtkListBox, row: ?*c.GtkListBoxRow, _: ?*anyopaque) callconv(.c) void {
    const r = row orelse return;
    const nav: *c.GtkWidget = app_state.nav_view orelse return;

    const row_index = c.gtk_list_box_row_get_index(r);
    if (row_index < 0) return;
    const idx: usize = @intCast(row_index);

    if (idx >= row_to_item.len) return;
    const item_index = row_to_item[idx] orelse return; // Skip section headers

    // Replace navigation stack with the selected top-level page (by tag)
    const tag = sidebar_items[item_index].tag;
    var tags = [_:null]?[*:0]const u8{tag};
    c.adw_navigation_view_replace_with_tags(@ptrCast(nav), @ptrCast(&tags), 1);
    app_state.active_view = sidebar_items[item_index].view_id;
}

fn onPageShowing(_: *c.GtkWidget, user_data: ?*anyopaque) callconv(.c) void {
    const idx: usize = @intFromPtr(user_data);
    if (idx >= sidebar_items.len) return;

    switch (sidebar_items[idx].view_id) {
        .home => if (app_state.home) |*v| v.refresh(),
        .movies => if (app_state.movies) |*v| v.refresh(),
        .tv_shows => if (app_state.tv_shows) |*v| v.refresh(),
        .other => if (app_state.other) |*v| v.refresh(),
        .favorites => if (app_state.favorites) |*v| v.refresh(),
        .collections => if (app_state.collections) |*v| v.refresh(),
        .files => if (app_state.files) |*v| v.refresh(),
        .downloads => {}, // Downloads uses its own polling timer
        .settings => {}, // Settings doesn't need refresh
        .player => {},
    }
}

fn onNavPagePopped(_: *c.GtkWidget, page: *c.GtkWidget, _: ?*anyopaque) callconv(.c) void {
    // If the player page was popped, stop playback
    const tag = c.adw_navigation_page_get_tag(@ptrCast(page));
    if (tag != null) {
        if (std.mem.eql(u8, std.mem.span(tag.?), "player")) {
            app_state.player.stop() catch {};
            app_state.active_view = .home;
        }
    }
}

fn onMotion(_: *c.GtkEventControllerMotion, _: f64, _: f64, _: ?*anyopaque) callconv(.c) void {
    if (isPlayerVisible() or app_state.direct_play) {
        app_state.controls.show();
        app_state.controls.scheduleHide();
    }
}

pub fn isPlayerVisible() bool {
    const nav = app_state.nav_view orelse return false;
    const tag = c.adw_navigation_view_get_visible_page_tag(@ptrCast(nav));
    if (tag == null) return false;
    return std.mem.eql(u8, std.mem.span(tag.?), "player");
}

pub fn toggleFullscreen() void {
    const window: *c.GtkWindow = @ptrCast(app_state.window orelse return);
    app_state.fullscreen = !app_state.fullscreen;
    if (app_state.fullscreen) {
        c.gtk_window_fullscreen(window);
        // Hide sidebar in fullscreen by showing only content
        if (app_state.split_view) |sv| {
            c.adw_navigation_split_view_set_collapsed(@ptrCast(sv), 1);
            c.adw_navigation_split_view_set_show_content(@ptrCast(sv), 1);
        }
    } else {
        c.gtk_window_unfullscreen(window);
        if (app_state.split_view) |sv| {
            c.adw_navigation_split_view_set_collapsed(@ptrCast(sv), 0);
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

pub fn getSettings() ?*settings_mod.Settings {
    if (app_state.settings != null) {
        return &app_state.settings.?;
    }
    return null;
}

pub fn getHttpClient() ?*http_mod.HttpClient {
    if (app_state.http_client != null) {
        return &app_state.http_client.?;
    }
    return null;
}

pub fn getImageCache() ?*image_cache_mod.ImageCache {
    if (app_state.image_cache != null) {
        return &app_state.image_cache.?;
    }
    return null;
}

pub fn getTmdbClient() ?*tmdb_client_mod.TmdbClient {
    if (app_state.tmdb_client != null) {
        return &app_state.tmdb_client.?;
    }
    return null;
}

var image_cache_dir_buf: [512]u8 = undefined;
var image_cache_tmp_buf: [256]u8 = undefined;
var image_cache_dir_val: ?[]const u8 = null;

fn getImageCacheDir() []const u8 {
    if (image_cache_dir_val) |d| return d;

    const data_dir = std.posix.getenv("XDG_CACHE_HOME") orelse blk: {
        const home = std.posix.getenv("HOME") orelse "/tmp";
        break :blk std.fmt.bufPrint(&image_cache_tmp_buf, "{s}/.cache", .{home}) catch "/tmp";
    };
    const dir = std.fmt.bufPrintZ(&image_cache_dir_buf, "{s}/reel/images", .{data_dir}) catch {
        image_cache_dir_val = "/tmp/reel/images";
        return "/tmp/reel/images";
    };
    std.fs.cwd().makePath(dir) catch {};
    image_cache_dir_val = dir;
    return dir;
}

pub fn getWindow() ?*c.GtkWindow {
    if (app_state.window) |w| {
        return @ptrCast(w);
    }
    return null;
}

pub fn getDownloader() ?*downloader_mod.Downloader {
    if (app_state.downloader != null) {
        return &app_state.downloader.?;
    }
    return null;
}

var download_dir_buf: [512]u8 = undefined;
var download_tmp_buf: [256]u8 = undefined;
var download_dir_val: ?[]const u8 = null;

pub fn getDownloadDir() []const u8 {
    if (download_dir_val) |d| return d;

    const data_dir = std.posix.getenv("XDG_DATA_HOME") orelse blk: {
        const home = std.posix.getenv("HOME") orelse "/tmp";
        break :blk std.fmt.bufPrint(&download_tmp_buf, "{s}/.local/share", .{home}) catch "/tmp";
    };
    const dir = std.fmt.bufPrintZ(&download_dir_buf, "{s}/reel/downloads", .{data_dir}) catch {
        download_dir_val = "/tmp/reel/downloads";
        return "/tmp/reel/downloads";
    };
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
    const nav: *c.GtkWidget = app_state.nav_view orelse return;
    app_state.active_view = .player;

    // Push player page onto navigation stack (by tag — page is static)
    c.adw_navigation_view_push_by_tag(@ptrCast(nav), "player");

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
    const nav: *c.GtkWidget = app_state.nav_view orelse return;

    // Find the sidebar item matching this view name and replace the nav stack
    const name_slice = std.mem.span(view_name);
    for (sidebar_items, 0..) |item, i| {
        if (std.mem.eql(u8, name_slice, std.mem.span(item.tag))) {
            var tags = [_:null]?[*:0]const u8{item.tag};
            c.adw_navigation_view_replace_with_tags(@ptrCast(nav), @ptrCast(&tags), 1);
            app_state.active_view = item.view_id;

            // Sync sidebar selection
            selectSidebarItem(i);
            return;
        }
    }
}

fn selectSidebarItem(item_index: usize) void {
    const sl: *c.GtkListBox = @ptrCast(@alignCast(app_state.sidebar_list orelse return));
    // Find the GtkListBox row index corresponding to this sidebar item
    for (row_to_item, 0..) |mapping, row_idx| {
        if (mapping) |idx| {
            if (idx == item_index) {
                const row = c.gtk_list_box_get_row_at_index(sl, @intCast(row_idx));
                if (row) |r| {
                    c.gtk_list_box_select_row(sl, r);
                }
                return;
            }
        }
    }
}

pub fn popNavigation() void {
    const nav: *c.GtkWidget = app_state.nav_view orelse return;
    _ = c.adw_navigation_view_pop(@ptrCast(nav));
}

pub fn showDetail(item_id: i64) void {
    var lib = getLibrary() orelse return;
    const item = lib.getMediaItem(item_id) catch return orelse return;
    defer lib.freeMediaItem(item);

    if (app_state.detail) |*detail| {
        detail.showItem(item);
    }

    const nav: *c.GtkWidget = app_state.nav_view orelse return;

    // Update detail page title to the media item name
    if (app_state.nav_pages[detail_page_idx]) |page| {
        const title_z = std.fmt.bufPrintZ(&detail_title_buf, "{s}", .{item.title}) catch "Detail";
        c.adw_navigation_page_set_title(@ptrCast(page), title_z);
    }

    // Push detail page onto navigation stack (by tag — page is static)
    c.adw_navigation_view_push_by_tag(@ptrCast(nav), "detail");
}
