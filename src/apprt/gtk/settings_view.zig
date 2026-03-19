const std = @import("std");
const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});
const app = @import("app.zig");
const types = @import("../../core/types.zig");
const plex_setup = @import("plex_setup.zig");

pub const SettingsView = struct {
    widget: *c.GtkWidget,

    pub fn init() SettingsView {
        const scrolled = c.gtk_scrolled_window_new();
        c.gtk_widget_set_vexpand(@ptrCast(scrolled), 1);
        c.gtk_widget_set_hexpand(@ptrCast(scrolled), 1);

        const page = c.adw_preferences_page_new();

        // Plex group
        const plex_group = c.adw_preferences_group_new();
        c.adw_preferences_group_set_title(@ptrCast(plex_group), "Plex");
        c.adw_preferences_group_set_description(@ptrCast(plex_group),
            "Connect to a Plex Media Server to stream your library.",
        );

        const add_server_row = c.adw_action_row_new();
        c.adw_preferences_row_set_title(@ptrCast(add_server_row), "Add Server");
        c.adw_action_row_set_subtitle(@ptrCast(add_server_row), "Connect via Plex PIN");
        c.adw_action_row_add_prefix(@ptrCast(add_server_row),
            c.gtk_image_new_from_icon_name("list-add-symbolic"),
        );
        c.adw_action_row_add_suffix(@ptrCast(add_server_row),
            c.gtk_image_new_from_icon_name("go-next-symbolic"),
        );
        c.gtk_list_box_row_set_activatable(@ptrCast(add_server_row), 1);
        _ = c.g_signal_connect_data(
            @ptrCast(add_server_row),
            "activated",
            @ptrCast(&onAddServerClicked),
            null,
            null,
            c.G_CONNECT_DEFAULT,
        );
        c.adw_preferences_group_add(@ptrCast(plex_group), @ptrCast(add_server_row));

        // Show existing servers
        if (app.getLibrary()) |lib| {
            if (lib.listServers()) |servers| {
                defer lib.freeServers(servers);
                for (servers) |server| {
                    const row = c.adw_action_row_new();
                    c.adw_preferences_row_set_title(@ptrCast(row), @ptrCast(server.name.ptr));
                    if (server.connection_uri) |uri| {
                        c.adw_action_row_set_subtitle(@ptrCast(row), @ptrCast(uri.ptr));
                    }
                    c.adw_action_row_add_prefix(@ptrCast(row),
                        c.gtk_image_new_from_icon_name("network-server-symbolic"),
                    );
                    c.adw_preferences_group_add(@ptrCast(plex_group), @ptrCast(row));
                }
            } else |_| {}
        }

        c.adw_preferences_page_add(@ptrCast(page), @ptrCast(plex_group));

        // Plex Libraries group (populated dynamically)
        const plex_libs_group = c.adw_preferences_group_new();
        c.adw_preferences_group_set_title(@ptrCast(plex_libs_group), "Plex Libraries");
        c.adw_preferences_group_set_description(@ptrCast(plex_libs_group),
            "Choose which Plex libraries to sync.",
        );
        populatePlexLibraries(@ptrCast(plex_libs_group));
        c.adw_preferences_page_add(@ptrCast(page), @ptrCast(plex_libs_group));

        // Library group
        const lib_group = c.adw_preferences_group_new();
        c.adw_preferences_group_set_title(@ptrCast(lib_group), "Library");
        c.adw_preferences_group_set_description(@ptrCast(lib_group),
            "Local folders to scan for media files.",
        );

        const add_folder_row = c.adw_action_row_new();
        c.adw_preferences_row_set_title(@ptrCast(add_folder_row), "Add Folder");
        c.adw_action_row_set_subtitle(@ptrCast(add_folder_row), "Scan a local directory for media");
        c.adw_action_row_add_prefix(@ptrCast(add_folder_row),
            c.gtk_image_new_from_icon_name("list-add-symbolic"),
        );
        c.adw_action_row_add_suffix(@ptrCast(add_folder_row),
            c.gtk_image_new_from_icon_name("go-next-symbolic"),
        );
        c.adw_preferences_group_add(@ptrCast(lib_group), @ptrCast(add_folder_row));

        // Show existing scan paths
        if (app.getLibrary()) |lib| {
            if (lib.listScanPaths()) |paths| {
                defer lib.freeScanPaths(paths);
                for (paths) |path| {
                    const row = c.adw_action_row_new();
                    c.adw_preferences_row_set_title(@ptrCast(row), @ptrCast(path.path.ptr));
                    c.adw_action_row_add_prefix(@ptrCast(row),
                        c.gtk_image_new_from_icon_name("folder-symbolic"),
                    );
                    c.adw_preferences_group_add(@ptrCast(lib_group), @ptrCast(row));
                }
            } else |_| {}
        }

        c.adw_preferences_page_add(@ptrCast(page), @ptrCast(lib_group));

        // Metadata group
        const meta_group = c.adw_preferences_group_new();
        c.adw_preferences_group_set_title(@ptrCast(meta_group), "Metadata");
        c.adw_preferences_group_set_description(@ptrCast(meta_group),
            "Configure metadata sources for local media.",
        );

        const tmdb_row = c.adw_entry_row_new();
        c.adw_preferences_row_set_title(@ptrCast(tmdb_row), "TMDB API Key");
        c.adw_preferences_group_add(@ptrCast(meta_group), @ptrCast(tmdb_row));
        c.adw_preferences_page_add(@ptrCast(page), @ptrCast(meta_group));

        // Playback group
        const playback_group = c.adw_preferences_group_new();
        c.adw_preferences_group_set_title(@ptrCast(playback_group), "Playback");

        const sub_row = c.adw_entry_row_new();
        c.adw_preferences_row_set_title(@ptrCast(sub_row), "Preferred Subtitle Language");
        c.adw_preferences_group_add(@ptrCast(playback_group), @ptrCast(sub_row));

        const audio_row = c.adw_entry_row_new();
        c.adw_preferences_row_set_title(@ptrCast(audio_row), "Preferred Audio Language");
        c.adw_preferences_group_add(@ptrCast(playback_group), @ptrCast(audio_row));

        c.adw_preferences_page_add(@ptrCast(page), @ptrCast(playback_group));

        // Subtitle Appearance group
        const sub_appearance_group = c.adw_preferences_group_new();
        c.adw_preferences_group_set_title(@ptrCast(sub_appearance_group), "Subtitle Appearance");
        c.adw_preferences_group_set_description(@ptrCast(sub_appearance_group),
            "Customize how subtitles are displayed during playback.",
        );

        const sub_font_row = c.adw_entry_row_new();
        c.adw_preferences_row_set_title(@ptrCast(sub_font_row), "Font");
        c.adw_entry_row_set_show_apply_button(@ptrCast(sub_font_row), 1);
        c.adw_preferences_group_add(@ptrCast(sub_appearance_group), @ptrCast(sub_font_row));
        _ = c.g_signal_connect_data(
            @ptrCast(sub_font_row),
            "apply",
            @ptrCast(&onSubFontApply),
            null,
            null,
            c.G_CONNECT_DEFAULT,
        );

        const sub_size_row = c.adw_spin_row_new_with_range(16, 72, 1);
        c.adw_preferences_row_set_title(@ptrCast(sub_size_row), "Font Size");
        c.adw_spin_row_set_value(@ptrCast(sub_size_row), 55); // mpv default
        c.adw_preferences_group_add(@ptrCast(sub_appearance_group), @ptrCast(sub_size_row));
        _ = c.g_signal_connect_data(
            @ptrCast(sub_size_row),
            "notify::value",
            @ptrCast(&onSubSizeChanged),
            null,
            null,
            c.G_CONNECT_DEFAULT,
        );

        const sub_border_size_row = c.adw_spin_row_new_with_range(0, 10, 0.5);
        c.adw_preferences_row_set_title(@ptrCast(sub_border_size_row), "Outline Size");
        c.adw_spin_row_set_value(@ptrCast(sub_border_size_row), 3); // mpv default
        c.adw_preferences_group_add(@ptrCast(sub_appearance_group), @ptrCast(sub_border_size_row));
        _ = c.g_signal_connect_data(
            @ptrCast(sub_border_size_row),
            "notify::value",
            @ptrCast(&onSubBorderSizeChanged),
            null,
            null,
            c.G_CONNECT_DEFAULT,
        );

        const sub_pos_row = c.adw_switch_row_new();
        c.adw_preferences_row_set_title(@ptrCast(sub_pos_row), "Subtitles at Top");
        c.adw_switch_row_set_active(@ptrCast(sub_pos_row), 0); // default: bottom
        c.adw_preferences_group_add(@ptrCast(sub_appearance_group), @ptrCast(sub_pos_row));
        _ = c.g_signal_connect_data(
            @ptrCast(sub_pos_row),
            "notify::active",
            @ptrCast(&onSubPosChanged),
            null,
            null,
            c.G_CONNECT_DEFAULT,
        );

        // Load saved values
        if (app.getSettings()) |settings| {
            if (settings.getString("sub_font") catch null) |font| {
                defer settings.allocator.free(font);
                const font_z = std.heap.c_allocator.dupeZ(u8, font) catch null;
                if (font_z) |f| {
                    defer std.heap.c_allocator.free(f);
                    c.gtk_editable_set_text(@ptrCast(sub_font_row), f.ptr);
                }
            }
            if (settings.getInt("sub_font_size") catch null) |size| {
                c.adw_spin_row_set_value(@ptrCast(sub_size_row), @floatFromInt(size));
            }
            if (settings.getString("sub_border_size") catch null) |size_str| {
                defer settings.allocator.free(size_str);
                const size = std.fmt.parseFloat(f64, size_str) catch 3.0;
                c.adw_spin_row_set_value(@ptrCast(sub_border_size_row), size);
            }
            if (settings.getString("sub_pos") catch null) |pos_str| {
                defer settings.allocator.free(pos_str);
                const pos = std.fmt.parseInt(i64, pos_str, 10) catch 100;
                c.adw_switch_row_set_active(@ptrCast(sub_pos_row), if (pos < 50) 1 else 0);
            }
        }

        c.adw_preferences_page_add(@ptrCast(page), @ptrCast(sub_appearance_group));

        // Storage group
        const storage_group = c.adw_preferences_group_new();
        c.adw_preferences_group_set_title(@ptrCast(storage_group), "Storage");

        const dl_path_row = c.adw_action_row_new();
        c.adw_preferences_row_set_title(@ptrCast(dl_path_row), "Download Path");
        c.adw_action_row_set_subtitle(@ptrCast(dl_path_row), @ptrCast(app.getDownloadDir().ptr));
        c.adw_action_row_add_prefix(@ptrCast(dl_path_row),
            c.gtk_image_new_from_icon_name("folder-download-symbolic"),
        );
        c.adw_preferences_group_add(@ptrCast(storage_group), @ptrCast(dl_path_row));

        const cache_row = c.adw_action_row_new();
        c.adw_preferences_row_set_title(@ptrCast(cache_row), "Clear Image Cache");
        c.adw_action_row_set_subtitle(@ptrCast(cache_row), "Free disk space used by cached poster images");
        c.adw_action_row_add_suffix(@ptrCast(cache_row),
            c.gtk_image_new_from_icon_name("edit-clear-symbolic"),
        );
        c.adw_preferences_group_add(@ptrCast(storage_group), @ptrCast(cache_row));

        c.adw_preferences_page_add(@ptrCast(page), @ptrCast(storage_group));

        c.gtk_scrolled_window_set_child(@ptrCast(scrolled), @ptrCast(page));

        return .{ .widget = @ptrCast(scrolled) };
    }
};

fn populatePlexLibraries(group: *c.GtkWidget) void {
    const allocator = app.getAllocator();
    const lib = app.getLibrary() orelse return;
    const http_client = app.getHttpClient() orelse return;

    const servers = lib.listServers() catch return;
    defer lib.freeServers(servers);

    // Get disabled libraries setting
    const settings = app.getSettings() orelse return;
    const disabled_str = settings.getString("plex_disabled_libraries") catch null;
    defer if (disabled_str) |s| allocator.free(s);

    for (servers) |server| {
        const uri = server.connection_uri orelse continue;
        const token = server.auth_token orelse continue;
        const client_id_str = blk: {
            if (settings.getString("client_identifier") catch null) |cid| break :blk cid;
            break :blk @as(?[]const u8, null);
        };
        defer if (client_id_str) |s| allocator.free(s);
        const client_id = client_id_str orelse continue;

        const plex_client_mod = @import("../../net/plex/client.zig");
        var plex = plex_client_mod.PlexClient.init(allocator, http_client, client_id, token);
        plex.setServerUri(uri);

        const libraries = plex.getLibraries() catch continue;
        defer {
            for (libraries) |l| {
                allocator.free(l.title);
                allocator.free(l.key);
            }
            allocator.free(libraries);
        }

        for (libraries) |section| {
            const row = c.adw_switch_row_new();

            // Build title with library type
            var title_buf: [128]u8 = undefined;
            const title = std.fmt.bufPrintZ(&title_buf, "{s}", .{section.title}) catch continue;
            c.adw_preferences_row_set_title(@ptrCast(row), title.ptr);

            var sub_buf: [64]u8 = undefined;
            const sub = std.fmt.bufPrintZ(&sub_buf, "{s}", .{section.library_type}) catch "";
            c.adw_action_row_set_subtitle(@ptrCast(row), sub.ptr);

            // Check if this library is enabled (not in disabled list)
            const is_enabled = if (disabled_str) |ds|
                !isKeyInList(ds, section.key)
            else
                true;
            c.adw_switch_row_set_active(@ptrCast(row), if (is_enabled) 1 else 0);

            // Store the library key on the widget for the callback
            const key_copy = allocator.dupeZ(u8, section.key) catch continue;
            c.g_object_set_data_full(
                @ptrCast(@alignCast(row)),
                "plex-library-key",
                @ptrCast(key_copy.ptr),
                &freeGObjectData,
            );

            _ = c.g_signal_connect_data(
                @ptrCast(row),
                "notify::active",
                @ptrCast(&onPlexLibraryToggled),
                null,
                null,
                c.G_CONNECT_DEFAULT,
            );

            c.adw_preferences_group_add(@ptrCast(group), @ptrCast(row));
        }
    }
}

fn freeGObjectData(data: ?*anyopaque) callconv(.c) void {
    if (data) |ptr| {
        // The data is a Zig-allocated null-terminated string.
        // We need to find its length to free it properly.
        const slice: [*]u8 = @ptrCast(ptr);
        const len = std.mem.len(@as([*:0]const u8, @ptrCast(slice)));
        app.getAllocator().free(slice[0 .. len + 1]);
    }
}

fn isKeyInList(list: []const u8, key: []const u8) bool {
    var iter = std.mem.splitScalar(u8, list, ',');
    while (iter.next()) |entry| {
        if (std.mem.eql(u8, entry, key)) return true;
    }
    return false;
}

fn onPlexLibraryToggled(row: *c.GtkWidget, _: ?*anyopaque) callconv(.c) void {
    const key_ptr: ?[*:0]const u8 = @ptrCast(c.g_object_get_data(@ptrCast(@alignCast(row)), "plex-library-key"));
    const key = if (key_ptr) |p| std.mem.span(p) else return;

    const active = c.adw_switch_row_get_active(@ptrCast(row));
    const settings = app.getSettings() orelse return;
    const allocator = app.getAllocator();

    // Get current disabled list
    const current = settings.getString("plex_disabled_libraries") catch null;
    defer if (current) |s| allocator.free(s);

    if (active != 0) {
        // Enabling: remove key from disabled list
        if (current) |list| {
            var new_list: std.ArrayList(u8) = .empty;
            defer new_list.deinit(allocator);
            var iter = std.mem.splitScalar(u8, list, ',');
            var first = true;
            while (iter.next()) |entry| {
                if (entry.len == 0) continue;
                if (std.mem.eql(u8, entry, key)) continue;
                if (!first) new_list.append(allocator, ',') catch return;
                new_list.appendSlice(allocator, entry) catch return;
                first = false;
            }
            settings.setString("plex_disabled_libraries", new_list.items) catch {};
        }
    } else {
        // Disabling: add key to disabled list and remove synced items
        if (current) |list| {
            if (list.len > 0) {
                const new = std.fmt.allocPrint(allocator, "{s},{s}", .{ list, key }) catch return;
                defer allocator.free(new);
                settings.setString("plex_disabled_libraries", new) catch {};
            } else {
                settings.setString("plex_disabled_libraries", key) catch {};
            }
        } else {
            settings.setString("plex_disabled_libraries", key) catch {};
        }

        // Remove already-synced items from this library section
        if (app.getLibrary()) |lib| {
            const servers = lib.listServers() catch return;
            defer lib.freeServers(servers);
            for (servers) |server| {
                lib.deleteByLibrarySection(server.id, key) catch |err| {
                    std.log.err("Failed to remove items for library {s}: {}", .{ key, err });
                };
            }
        }
    }
}

fn onAddServerClicked(_: *c.GtkWidget, _: ?*anyopaque) callconv(.c) void {
    plex_setup.showSetupDialog();
}

fn onSubFontApply(row: *c.GtkWidget, _: ?*anyopaque) callconv(.c) void {
    const text = c.gtk_editable_get_text(@ptrCast(row));
    if (text == null) return;
    const font = std.mem.span(text.?);
    if (font.len == 0) return;
    const settings = app.getSettings() orelse return;
    settings.setString("sub_font", font) catch {};
}

fn onSubSizeChanged(row: *c.GtkWidget, _: ?*anyopaque) callconv(.c) void {
    const value = c.adw_spin_row_get_value(@ptrCast(row));
    const settings = app.getSettings() orelse return;
    settings.setInt("sub_font_size", @intFromFloat(value)) catch {};
}

fn onSubBorderSizeChanged(row: *c.GtkWidget, _: ?*anyopaque) callconv(.c) void {
    const value = c.adw_spin_row_get_value(@ptrCast(row));
    const settings = app.getSettings() orelse return;
    var buf: [16]u8 = undefined;
    const str = std.fmt.bufPrint(&buf, "{d:.1}", .{value}) catch return;
    settings.setString("sub_border_size", str) catch {};
}

fn onSubPosChanged(row: *c.GtkWidget, _: ?*anyopaque) callconv(.c) void {
    const active = c.adw_switch_row_get_active(@ptrCast(row));
    const settings = app.getSettings() orelse return;
    // sub-pos: 0 = top, 100 = bottom
    settings.setInt("sub_pos", if (active != 0) @as(i64, 10) else @as(i64, 100)) catch {};
}
