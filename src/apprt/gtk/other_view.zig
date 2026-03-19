const std = @import("std");
const c = @import("c.zig").c;
const poster_grid = @import("poster_grid.zig");
const app = @import("app.zig");
const types = @import("../../core/types.zig");

pub const OtherView = struct {
    widget: *c.GtkWidget,
    grid: poster_grid.PosterGrid,
    grid_container: *c.GtkWidget,

    pub fn init() OtherView {
        const vbox = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 0);
        c.gtk_widget_set_vexpand(@ptrCast(vbox), 1);
        c.gtk_widget_set_hexpand(@ptrCast(vbox), 1);

        const content_stack = c.gtk_stack_new();
        c.gtk_widget_set_vexpand(@ptrCast(content_stack), 1);

        const status = c.adw_status_page_new();
        c.adw_status_page_set_icon_name(@ptrCast(status), "folder-videos-symbolic");
        c.adw_status_page_set_title(@ptrCast(status), "No Other Media");
        c.adw_status_page_set_description(@ptrCast(status),
            "Media files that don't match movie or TV show patterns will appear here.",
        );
        _ = c.gtk_stack_add_named(@ptrCast(content_stack), @ptrCast(status), "empty");

        const grid = poster_grid.PosterGrid.init(app.getAllocator());
        _ = c.gtk_stack_add_named(@ptrCast(content_stack), @ptrCast(@alignCast(grid.widget)), "grid");

        c.gtk_box_append(@ptrCast(vbox), @ptrCast(content_stack));

        c.gtk_stack_set_visible_child_name(@ptrCast(content_stack), "empty");

        return OtherView{
            .widget = @ptrCast(vbox),
            .grid = grid,
            .grid_container = @ptrCast(content_stack),
        };
    }

    pub fn refresh(self: *OtherView) void {
        var lib = app.getLibrary() orelse {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.grid_container), "empty");
            return;
        };

        const items = lib.getItemsByType(.other, .title, .asc, 200, 0) catch {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.grid_container), "empty");
            return;
        };
        defer lib.freeMediaItems(items);

        if (items.len == 0) {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.grid_container), "empty");
            return;
        }

        self.grid.populate(items);
        c.gtk_stack_set_visible_child_name(@ptrCast(self.grid_container), "grid");
    }
};
