const std = @import("std");
const c = @import("c.zig").c;
const app = @import("app.zig");
const types = @import("../../core/types.zig");

pub const FavoritesView = struct {
    widget: *c.GtkWidget,
    content_stack: *c.GtkWidget,
    list_box: *c.GtkWidget,

    pub fn init() FavoritesView {
        const content_stack = c.gtk_stack_new();
        c.gtk_widget_set_vexpand(@ptrCast(content_stack), 1);
        c.gtk_widget_set_hexpand(@ptrCast(content_stack), 1);

        // Empty state
        const status = c.adw_status_page_new();
        c.adw_status_page_set_icon_name(@ptrCast(status), "starred-symbolic");
        c.adw_status_page_set_title(@ptrCast(status), "No Favorites");
        c.adw_status_page_set_description(@ptrCast(status),
            "Right-click any item to add it to your favorites.",
        );
        _ = c.gtk_stack_add_named(@ptrCast(content_stack), @ptrCast(status), "empty");

        // Favorites list
        const scrolled = c.gtk_scrolled_window_new();
        c.gtk_widget_set_vexpand(@ptrCast(scrolled), 1);

        const clamp = c.adw_clamp_new();
        c.adw_clamp_set_maximum_size(@ptrCast(clamp), 800);

        const list_box = c.gtk_list_box_new();
        c.gtk_list_box_set_selection_mode(@ptrCast(list_box), c.GTK_SELECTION_NONE);
        c.gtk_widget_add_css_class(@ptrCast(list_box), "boxed-list");
        c.gtk_widget_set_margin_top(@ptrCast(list_box), 16);
        c.gtk_widget_set_margin_bottom(@ptrCast(list_box), 16);
        c.gtk_widget_set_margin_start(@ptrCast(list_box), 16);
        c.gtk_widget_set_margin_end(@ptrCast(list_box), 16);

        c.adw_clamp_set_child(@ptrCast(clamp), @ptrCast(list_box));
        c.gtk_scrolled_window_set_child(@ptrCast(scrolled), @ptrCast(clamp));
        _ = c.gtk_stack_add_named(@ptrCast(content_stack), scrolled, "list");

        c.gtk_stack_set_visible_child_name(@ptrCast(content_stack), "empty");

        return FavoritesView{
            .widget = @ptrCast(content_stack),
            .content_stack = @ptrCast(content_stack),
            .list_box = @ptrCast(list_box),
        };
    }

    pub fn refresh(self: *FavoritesView) void {
        // Clear existing rows
        while (true) {
            const child = c.gtk_widget_get_first_child(@ptrCast(self.list_box));
            if (child == null) break;
            c.gtk_list_box_remove(@ptrCast(self.list_box), child);
        }

        var lib = app.getLibrary() orelse {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.content_stack), "empty");
            return;
        };

        const favs = lib.listFavorites() catch {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.content_stack), "empty");
            return;
        };
        defer lib.freeFavorites(favs);

        if (favs.len == 0) {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.content_stack), "empty");
            return;
        }

        for (favs) |fav| {
            const row = c.adw_action_row_new();
            c.adw_preferences_row_set_title(@ptrCast(row), @ptrCast(fav.display_name.ptr));

            const icon_name: [*:0]const u8 = switch (fav.item_type) {
                .media_item => "video-display-symbolic",
                .plex_library => "network-server-symbolic",
                .scan_path => "folder-symbolic",
                .filter => "funnel-symbolic",
            };
            c.adw_action_row_add_prefix(@ptrCast(row),
                c.gtk_image_new_from_icon_name(icon_name),
            );
            c.gtk_list_box_append(@ptrCast(self.list_box), @ptrCast(row));
        }

        c.gtk_stack_set_visible_child_name(@ptrCast(self.content_stack), "list");
    }
};
