const std = @import("std");
const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});
const app = @import("app.zig");
const types = @import("../../core/types.zig");

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

        // Storage group
        const storage_group = c.adw_preferences_group_new();
        c.adw_preferences_group_set_title(@ptrCast(storage_group), "Storage");

        const dl_path_row = c.adw_entry_row_new();
        c.adw_preferences_row_set_title(@ptrCast(dl_path_row), "Download Path");
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
