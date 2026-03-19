const std = @import("std");
const c = @import("c.zig").c;
const app = @import("app.zig");
const types = @import("../../core/types.zig");

pub const FilesView = struct {
    widget: *c.GtkWidget,
    content_stack: *c.GtkWidget,
    sources_box: *c.GtkWidget,

    pub fn init() FilesView {
        const content_stack = c.gtk_stack_new();
        c.gtk_widget_set_vexpand(@ptrCast(content_stack), 1);
        c.gtk_widget_set_hexpand(@ptrCast(content_stack), 1);

        // Empty state
        const status = c.adw_status_page_new();
        c.adw_status_page_set_icon_name(@ptrCast(status), "network-server-symbolic");
        c.adw_status_page_set_title(@ptrCast(status), "No Sources");
        c.adw_status_page_set_description(@ptrCast(status),
            "Add Plex servers or local folders in Settings to browse your files.",
        );
        _ = c.gtk_stack_add_named(@ptrCast(content_stack), @ptrCast(status), "empty");

        // Sources list
        const scrolled = c.gtk_scrolled_window_new();
        c.gtk_widget_set_vexpand(@ptrCast(scrolled), 1);

        const clamp = c.adw_clamp_new();
        c.adw_clamp_set_maximum_size(@ptrCast(clamp), 800);

        const sources_box = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 16);
        c.gtk_widget_set_margin_top(@ptrCast(sources_box), 16);
        c.gtk_widget_set_margin_bottom(@ptrCast(sources_box), 16);
        c.gtk_widget_set_margin_start(@ptrCast(sources_box), 16);
        c.gtk_widget_set_margin_end(@ptrCast(sources_box), 16);

        c.adw_clamp_set_child(@ptrCast(clamp), @ptrCast(sources_box));
        c.gtk_scrolled_window_set_child(@ptrCast(scrolled), @ptrCast(clamp));
        _ = c.gtk_stack_add_named(@ptrCast(content_stack), scrolled, "list");

        c.gtk_stack_set_visible_child_name(@ptrCast(content_stack), "empty");

        return FilesView{
            .widget = @ptrCast(content_stack),
            .content_stack = @ptrCast(content_stack),
            .sources_box = @ptrCast(sources_box),
        };
    }

    pub fn refresh(self: *FilesView) void {
        // Clear
        while (true) {
            const child = c.gtk_widget_get_first_child(@ptrCast(self.sources_box));
            if (child == null) break;
            c.gtk_box_remove(@ptrCast(self.sources_box), child);
        }

        var lib = app.getLibrary() orelse {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.content_stack), "empty");
            return;
        };

        var has_content = false;

        // Plex servers group
        if (lib.listServers()) |servers| {
            defer lib.freeServers(servers);
            if (servers.len > 0) {
                has_content = true;
                const group = c.adw_preferences_group_new();
                c.adw_preferences_group_set_title(@ptrCast(group), "Plex Servers");

                for (servers) |server| {
                    const row = c.adw_action_row_new();
                    c.adw_preferences_row_set_title(@ptrCast(row), @ptrCast(server.name.ptr));
                    if (server.connection_uri) |uri| {
                        c.adw_action_row_set_subtitle(@ptrCast(row), @ptrCast(uri.ptr));
                    }
                    c.adw_action_row_add_prefix(@ptrCast(row),
                        c.gtk_image_new_from_icon_name("network-server-symbolic"),
                    );
                    c.adw_action_row_add_suffix(@ptrCast(row),
                        c.gtk_image_new_from_icon_name("go-next-symbolic"),
                    );
                    c.adw_preferences_group_add(@ptrCast(group), @ptrCast(row));
                }
                c.gtk_box_append(@ptrCast(self.sources_box), @ptrCast(group));
            }
        } else |_| {}

        // Local folders group
        if (lib.listScanPaths()) |paths| {
            defer lib.freeScanPaths(paths);
            if (paths.len > 0) {
                has_content = true;
                const group = c.adw_preferences_group_new();
                c.adw_preferences_group_set_title(@ptrCast(group), "Local Folders");

                for (paths) |path| {
                    const row = c.adw_action_row_new();
                    c.adw_preferences_row_set_title(@ptrCast(row), @ptrCast(path.path.ptr));
                    c.adw_action_row_add_prefix(@ptrCast(row),
                        c.gtk_image_new_from_icon_name("folder-symbolic"),
                    );
                    c.adw_action_row_add_suffix(@ptrCast(row),
                        c.gtk_image_new_from_icon_name("go-next-symbolic"),
                    );
                    c.adw_preferences_group_add(@ptrCast(group), @ptrCast(row));
                }
                c.gtk_box_append(@ptrCast(self.sources_box), @ptrCast(group));
            }
        } else |_| {}

        if (has_content) {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.content_stack), "list");
        } else {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.content_stack), "empty");
        }
    }
};
