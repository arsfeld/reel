const std = @import("std");
const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});
const app = @import("app.zig");
const types = @import("../../core/types.zig");
const tmdb_client_mod = @import("../../net/tmdb/client.zig");
const tmdb_types = @import("../../net/tmdb/types.zig");
const detail_view = @import("detail_view.zig");

/// State for the fix match dialog, stored in a global so callbacks can reach it.
const DialogState = struct {
    window: *c.GtkWidget,
    search_entry: *c.GtkWidget,
    list_box: *c.GtkWidget,
    select_button: *c.GtkWidget,
    status_label: *c.GtkWidget,
    movie_toggle: *c.GtkWidget,
    tv_toggle: *c.GtkWidget,
    media_item_id: i64,
    media_type: types.MediaType,
    // Currently stored search results
    results: []tmdb_types.SearchResult = &.{},
    selected_index: ?usize = null,
};

var global_state: ?DialogState = null;

/// Open the Fix Match dialog for the given media item.
pub fn open(item_id: i64, item_title: []const u8, media_type: types.MediaType) void {
    // Don't open a second dialog
    if (global_state != null) return;

    const parent_window = app.getWindow() orelse return;

    // Create the dialog window
    const window = c.gtk_window_new();
    c.gtk_window_set_title(@ptrCast(window), "Fix Match");
    c.gtk_window_set_default_size(@ptrCast(window), 600, 500);
    c.gtk_window_set_modal(@ptrCast(window), 1);
    c.gtk_window_set_transient_for(@ptrCast(window), @ptrCast(parent_window));

    // Main vertical layout
    const vbox = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 0);

    // Header bar with Cancel and Select
    const header = c.adw_header_bar_new();
    c.adw_header_bar_set_show_title(@ptrCast(header), 1);

    const cancel_button = c.gtk_button_new_with_label("Cancel");
    c.adw_header_bar_pack_start(@ptrCast(header), @ptrCast(cancel_button));

    const select_button = c.gtk_button_new_with_label("Select");
    c.gtk_widget_add_css_class(@ptrCast(select_button), "suggested-action");
    c.gtk_widget_set_sensitive(@ptrCast(select_button), 0);
    c.adw_header_bar_pack_end(@ptrCast(header), @ptrCast(select_button));

    c.gtk_box_append(@ptrCast(vbox), @ptrCast(header));

    // Content area with margins
    const content = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 12);
    c.gtk_widget_set_margin_top(@ptrCast(content), 12);
    c.gtk_widget_set_margin_bottom(@ptrCast(content), 12);
    c.gtk_widget_set_margin_start(@ptrCast(content), 12);
    c.gtk_widget_set_margin_end(@ptrCast(content), 12);

    // Type toggle (Movie / TV Show)
    const toggle_box = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 0);
    c.gtk_widget_add_css_class(@ptrCast(toggle_box), "linked");
    c.gtk_widget_set_halign(@ptrCast(toggle_box), c.GTK_ALIGN_CENTER);

    const movie_toggle = c.gtk_toggle_button_new_with_label("Movie");
    const tv_toggle = c.gtk_toggle_button_new_with_label("TV Show");
    c.gtk_toggle_button_set_group(@ptrCast(tv_toggle), @ptrCast(movie_toggle));

    // Set initial toggle based on media type
    if (media_type == .show) {
        c.gtk_toggle_button_set_active(@ptrCast(tv_toggle), 1);
    } else {
        c.gtk_toggle_button_set_active(@ptrCast(movie_toggle), 1);
    }

    c.gtk_box_append(@ptrCast(toggle_box), @ptrCast(movie_toggle));
    c.gtk_box_append(@ptrCast(toggle_box), @ptrCast(tv_toggle));
    c.gtk_box_append(@ptrCast(content), @ptrCast(toggle_box));

    // Search entry
    const search_entry = c.gtk_search_entry_new();
    c.gtk_widget_set_hexpand(@ptrCast(search_entry), 1);

    // Pre-populate with the item title
    const allocator = app.getAllocator();
    const title_z = allocator.dupeZ(u8, item_title) catch null;
    if (title_z) |tz| {
        c.gtk_editable_set_text(@ptrCast(search_entry), tz.ptr);
        allocator.free(tz);
    }

    c.gtk_box_append(@ptrCast(content), @ptrCast(search_entry));

    // Status label (for "Searching..." / "No results" / result count)
    const status_label = c.gtk_label_new("");
    c.gtk_widget_add_css_class(@ptrCast(status_label), "dim-label");
    c.gtk_widget_set_halign(@ptrCast(status_label), c.GTK_ALIGN_START);
    c.gtk_widget_set_visible(@ptrCast(status_label), 0);
    c.gtk_box_append(@ptrCast(content), @ptrCast(status_label));

    // Scrolled window for results
    const scrolled = c.gtk_scrolled_window_new();
    c.gtk_scrolled_window_set_policy(@ptrCast(scrolled), c.GTK_POLICY_NEVER, c.GTK_POLICY_AUTOMATIC);
    c.gtk_widget_set_vexpand(@ptrCast(scrolled), 1);

    const list_box = c.gtk_list_box_new();
    c.gtk_list_box_set_selection_mode(@ptrCast(list_box), c.GTK_SELECTION_SINGLE);
    c.gtk_widget_add_css_class(@ptrCast(list_box), "boxed-list");
    c.gtk_scrolled_window_set_child(@ptrCast(scrolled), @ptrCast(list_box));

    c.gtk_box_append(@ptrCast(content), @ptrCast(scrolled));
    c.gtk_box_append(@ptrCast(vbox), @ptrCast(content));

    c.gtk_window_set_child(@ptrCast(window), @ptrCast(vbox));

    // Store state
    global_state = .{
        .window = @ptrCast(window),
        .search_entry = @ptrCast(search_entry),
        .list_box = @ptrCast(list_box),
        .select_button = @ptrCast(select_button),
        .status_label = @ptrCast(status_label),
        .movie_toggle = @ptrCast(movie_toggle),
        .tv_toggle = @ptrCast(tv_toggle),
        .media_item_id = item_id,
        .media_type = media_type,
    };

    // Connect signals
    _ = c.g_signal_connect_data(
        @ptrCast(cancel_button),
        "clicked",
        @ptrCast(&onCancelClicked),
        null,
        null,
        c.G_CONNECT_DEFAULT,
    );

    _ = c.g_signal_connect_data(
        @ptrCast(select_button),
        "clicked",
        @ptrCast(&onSelectClicked),
        null,
        null,
        c.G_CONNECT_DEFAULT,
    );

    _ = c.g_signal_connect_data(
        @ptrCast(search_entry),
        "activate",
        @ptrCast(&onSearchActivated),
        null,
        null,
        c.G_CONNECT_DEFAULT,
    );

    _ = c.g_signal_connect_data(
        @ptrCast(list_box),
        "row-selected",
        @ptrCast(&onRowSelected),
        null,
        null,
        c.G_CONNECT_DEFAULT,
    );

    _ = c.g_signal_connect_data(
        @ptrCast(window),
        "close-request",
        @ptrCast(&onCloseRequest),
        null,
        null,
        c.G_CONNECT_DEFAULT,
    );

    c.gtk_window_present(@ptrCast(window));
}

fn onCancelClicked(_: *c.GtkButton, _: ?*anyopaque) callconv(.c) void {
    closeDialog();
}

fn onCloseRequest(_: *c.GtkWindow, _: ?*anyopaque) callconv(.c) c.gboolean {
    cleanupState();
    return 0; // allow close
}

fn onSearchActivated(_: *c.GtkSearchEntry, _: ?*anyopaque) callconv(.c) void {
    var state = &(global_state orelse return);

    const query_raw = c.gtk_editable_get_text(@ptrCast(state.search_entry));
    if (query_raw == null) return;
    const query = std.mem.span(query_raw.?);
    if (query.len == 0) return;

    // Show searching status
    c.gtk_label_set_text(@ptrCast(state.status_label), "Searching...");
    c.gtk_widget_set_visible(@ptrCast(state.status_label), 1);

    // Clear previous results
    freeResults(state);
    clearListBox(state.list_box);
    c.gtk_widget_set_sensitive(@ptrCast(state.select_button), 0);
    state.selected_index = null;

    // Determine search type
    const is_tv = c.gtk_toggle_button_get_active(@ptrCast(state.tv_toggle)) != 0;

    // Get TMDB client
    var tmdb = app.getTmdbClient() orelse {
        c.gtk_label_set_text(@ptrCast(state.status_label), "TMDB API key not configured. Set it in Settings.");
        c.gtk_widget_set_visible(@ptrCast(state.status_label), 1);
        return;
    };

    // Perform search
    const results = if (is_tv)
        tmdb.searchTV(query) catch {
            c.gtk_label_set_text(@ptrCast(state.status_label), "Search failed. Check your network connection.");
            return;
        }
    else
        tmdb.searchMovies(query) catch {
            c.gtk_label_set_text(@ptrCast(state.status_label), "Search failed. Check your network connection.");
            return;
        };

    state.results = results;

    if (results.len == 0) {
        c.gtk_label_set_text(@ptrCast(state.status_label), "No results found.");
        c.gtk_widget_set_visible(@ptrCast(state.status_label), 1);
        return;
    }

    // Update status with count
    var count_buf: [64]u8 = undefined;
    const count_str = std.fmt.bufPrintZ(&count_buf, "{d} result{s} found", .{
        results.len,
        if (results.len == 1) "" else "s",
    }) catch "Results found";
    c.gtk_label_set_text(@ptrCast(state.status_label), count_str);
    c.gtk_widget_set_visible(@ptrCast(state.status_label), 1);

    // Populate list box
    for (results) |result| {
        const row = buildResultRow(result);
        c.gtk_list_box_append(@ptrCast(state.list_box), @ptrCast(row));
    }
}

fn buildResultRow(result: tmdb_types.SearchResult) *c.GtkWidget {
    const row_box = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 4);
    c.gtk_widget_set_margin_top(@ptrCast(row_box), 8);
    c.gtk_widget_set_margin_bottom(@ptrCast(row_box), 8);
    c.gtk_widget_set_margin_start(@ptrCast(row_box), 8);
    c.gtk_widget_set_margin_end(@ptrCast(row_box), 8);

    // Title + year row
    const title_box = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 8);

    // Title label
    const allocator = app.getAllocator();
    const title_z = allocator.dupeZ(u8, result.title) catch null;
    const title_label = c.gtk_label_new(if (title_z) |tz| tz.ptr else "Unknown");
    if (title_z) |tz| allocator.free(tz);
    c.gtk_widget_add_css_class(@ptrCast(title_label), "heading");
    c.gtk_widget_set_halign(@ptrCast(title_label), c.GTK_ALIGN_START);
    c.gtk_label_set_ellipsize(@ptrCast(title_label), c.PANGO_ELLIPSIZE_END);
    c.gtk_box_append(@ptrCast(title_box), @ptrCast(title_label));

    // Year from release_date (extract first 4 chars)
    if (result.release_date) |date| {
        if (date.len >= 4) {
            var year_buf: [16]u8 = undefined;
            const year_len = @min(date.len, 4);
            @memcpy(year_buf[0..year_len], date[0..year_len]);
            year_buf[year_len] = 0;
            const year_label = c.gtk_label_new(&year_buf);
            c.gtk_widget_add_css_class(@ptrCast(year_label), "dim-label");
            c.gtk_box_append(@ptrCast(title_box), @ptrCast(year_label));
        }
    }

    // Rating
    if (result.vote_average) |vote| {
        if (vote > 0) {
            var rating_buf: [16]u8 = undefined;
            const rating_str = std.fmt.bufPrintZ(&rating_buf, "{d:.1}/10", .{vote}) catch null;
            if (rating_str) |rs| {
                const rating_label = c.gtk_label_new(rs);
                c.gtk_widget_add_css_class(@ptrCast(rating_label), "dim-label");
                c.gtk_widget_set_hexpand(@ptrCast(rating_label), 1);
                c.gtk_widget_set_halign(@ptrCast(rating_label), c.GTK_ALIGN_END);
                c.gtk_box_append(@ptrCast(title_box), @ptrCast(rating_label));
            }
        }
    }

    c.gtk_box_append(@ptrCast(row_box), @ptrCast(title_box));

    // Overview snippet (first ~150 chars)
    if (result.overview) |overview| {
        if (overview.len > 0) {
            const max_len: usize = 150;
            const snippet_len = @min(overview.len, max_len);
            // Find a safe truncation point (don't cut in the middle of a multi-byte char)
            var safe_len = snippet_len;
            while (safe_len > 0 and (overview[safe_len - 1] & 0xC0) == 0x80) {
                safe_len -= 1;
            }
            // If we cut at the start of a multi-byte sequence, step back one more
            if (safe_len > 0 and safe_len < overview.len and (overview[safe_len - 1] & 0x80) != 0 and (overview[safe_len - 1] & 0xC0) != 0x80) {
                // This byte is the start of a multi-byte sequence that we'd be cutting
                safe_len -= 1;
            }

            var snippet_buf: [256]u8 = undefined;
            const suffix: []const u8 = if (overview.len > max_len) "..." else "";
            const snippet = std.fmt.bufPrintZ(&snippet_buf, "{s}{s}", .{
                overview[0..safe_len],
                suffix,
            }) catch null;
            if (snippet) |s| {
                const overview_label = c.gtk_label_new(s);
                c.gtk_widget_add_css_class(@ptrCast(overview_label), "dim-label");
                c.gtk_widget_set_halign(@ptrCast(overview_label), c.GTK_ALIGN_START);
                c.gtk_label_set_wrap(@ptrCast(overview_label), 1);
                c.gtk_label_set_xalign(@ptrCast(overview_label), 0);
                c.gtk_label_set_max_width_chars(@ptrCast(overview_label), 80);
                c.gtk_box_append(@ptrCast(row_box), @ptrCast(overview_label));
            }
        }
    }

    return @ptrCast(row_box);
}

fn onRowSelected(_: *c.GtkListBox, row: ?*c.GtkListBoxRow, _: ?*anyopaque) callconv(.c) void {
    var state = &(global_state orelse return);

    if (row) |r| {
        const index = c.gtk_list_box_row_get_index(r);
        if (index >= 0) {
            state.selected_index = @intCast(index);
            c.gtk_widget_set_sensitive(@ptrCast(state.select_button), 1);
            return;
        }
    }
    state.selected_index = null;
    c.gtk_widget_set_sensitive(@ptrCast(state.select_button), 0);
}

fn onSelectClicked(_: *c.GtkButton, _: ?*anyopaque) callconv(.c) void {
    const state = global_state orelse return;
    const idx = state.selected_index orelse return;
    if (idx >= state.results.len) return;

    const selected = state.results[idx];
    const is_tv = c.gtk_toggle_button_get_active(@ptrCast(state.tv_toggle)) != 0;

    const tmdb = app.getTmdbClient() orelse return;
    const lib = app.getLibrary() orelse return;

    if (is_tv) {
        applyTVMatch(tmdb, lib, state.media_item_id, selected.id);
    } else {
        applyMovieMatch(tmdb, lib, state.media_item_id, selected.id);
    }

    closeDialog();

    // Refresh the detail view
    refreshDetailView(state.media_item_id);
}

fn applyMovieMatch(tmdb: *tmdb_client_mod.TmdbClient, lib: *@import("../../core/library.zig").Library, item_id: i64, tmdb_id: i32) void {
    const detail = tmdb.getMovieDetail(tmdb_id) catch {
        std.log.err("Failed to fetch movie detail for TMDB id {d}", .{tmdb_id});
        return;
    } orelse {
        std.log.err("No movie detail returned for TMDB id {d}", .{tmdb_id});
        return;
    };
    defer tmdb.freeMovieDetail(detail);

    // Parse year from release_date
    var year: ?i32 = null;
    if (detail.release_date) |date| {
        if (date.len >= 4) {
            year = std.fmt.parseInt(i32, date[0..4], 10) catch null;
        }
    }

    // Convert runtime minutes to milliseconds
    var duration_ms: ?i64 = null;
    if (detail.runtime) |rt| {
        duration_ms = @as(i64, rt) * 60000;
    }

    // Update metadata
    lib.updateMediaItemMetadata(item_id, .{
        .source = .local, // not used by update, but needed for struct
        .media_type = .movie,
        .title = detail.title,
        .sort_title = null,
        .year = year,
        .summary = detail.overview,
        .rating = detail.vote_average,
        .duration_ms = duration_ms,
        .poster_path = detail.poster_path,
        .backdrop_path = detail.backdrop_path,
        .tmdb_id = detail.id,
        .match_locked = true,
    }) catch {
        std.log.err("Failed to update metadata for item {d}", .{item_id});
        return;
    };

    // Update genres
    const allocator = app.getAllocator();
    var genre_names: std.ArrayList([]const u8) = .empty;
    defer genre_names.deinit(allocator);

    for (detail.genres) |genre| {
        genre_names.append(allocator, genre.name) catch continue;
    }
    const names_slice = genre_names.toOwnedSlice(allocator) catch return;
    defer allocator.free(names_slice);

    lib.setMediaItemGenres(item_id, names_slice) catch {
        std.log.err("Failed to update genres for item {d}", .{item_id});
    };

    std.log.info("Fixed match: item {d} -> TMDB movie {d} ({s})", .{ item_id, detail.id, detail.title });
}

fn applyTVMatch(tmdb: *tmdb_client_mod.TmdbClient, lib: *@import("../../core/library.zig").Library, item_id: i64, tmdb_id: i32) void {
    const detail = tmdb.getTVDetail(tmdb_id) catch {
        std.log.err("Failed to fetch TV detail for TMDB id {d}", .{tmdb_id});
        return;
    } orelse {
        std.log.err("No TV detail returned for TMDB id {d}", .{tmdb_id});
        return;
    };
    defer tmdb.freeTVDetail(detail);

    // Parse year from first_air_date
    var year: ?i32 = null;
    if (detail.first_air_date) |date| {
        if (date.len >= 4) {
            year = std.fmt.parseInt(i32, date[0..4], 10) catch null;
        }
    }

    // Update metadata
    lib.updateMediaItemMetadata(item_id, .{
        .source = .local,
        .media_type = .show,
        .title = detail.name,
        .sort_title = null,
        .year = year,
        .summary = detail.overview,
        .rating = detail.vote_average,
        .duration_ms = null,
        .poster_path = detail.poster_path,
        .backdrop_path = detail.backdrop_path,
        .tmdb_id = detail.id,
        .match_locked = true,
    }) catch {
        std.log.err("Failed to update metadata for item {d}", .{item_id});
        return;
    };

    // Update genres
    const allocator = app.getAllocator();
    var genre_names: std.ArrayList([]const u8) = .empty;
    defer genre_names.deinit(allocator);

    for (detail.genres) |genre| {
        genre_names.append(allocator, genre.name) catch continue;
    }
    const names_slice = genre_names.toOwnedSlice(allocator) catch return;
    defer allocator.free(names_slice);

    lib.setMediaItemGenres(item_id, names_slice) catch {
        std.log.err("Failed to update genres for item {d}", .{item_id});
    };

    std.log.info("Fixed match: item {d} -> TMDB TV {d} ({s})", .{ item_id, detail.id, detail.name });
}

fn refreshDetailView(item_id: i64) void {
    var lib = app.getLibrary() orelse return;
    const item = lib.getMediaItem(item_id) catch return orelse return;
    defer lib.freeMediaItem(item);

    if (detail_view.global_detail) |gd| {
        gd.showItem(item);
    }
}

fn clearListBox(list_box: *c.GtkWidget) void {
    // Remove all children from the list box
    while (true) {
        const row = c.gtk_list_box_get_row_at_index(@ptrCast(list_box), 0);
        if (row == null) break;
        c.gtk_list_box_remove(@ptrCast(list_box), @ptrCast(row.?));
    }
}

fn freeResults(state: *DialogState) void {
    if (state.results.len > 0) {
        var tmdb = app.getTmdbClient() orelse return;
        tmdb.freeSearchResults(state.results);
        state.results = &.{};
    }
}

fn cleanupState() void {
    if (global_state) |*state| {
        freeResults(state);
    }
    global_state = null;
}

fn closeDialog() void {
    if (global_state) |state| {
        const win = state.window;
        cleanupState();
        c.gtk_window_destroy(@ptrCast(win));
    }
}
