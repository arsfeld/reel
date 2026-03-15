const std = @import("std");
const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});
const poster_grid = @import("poster_grid.zig");
const app = @import("app.zig");
const library_mod = @import("../../core/library.zig");
const types = @import("../../core/types.zig");

pub const MoviesView = struct {
    widget: *c.GtkWidget,
    grid: poster_grid.PosterGrid,
    status_page: *c.GtkWidget,
    grid_container: *c.GtkWidget,

    pub fn init() MoviesView {
        const vbox = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 0);
        c.gtk_widget_set_vexpand(@ptrCast(vbox), 1);
        c.gtk_widget_set_hexpand(@ptrCast(vbox), 1);

        // Toolbar with sort and search
        const toolbar = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 8);
        c.gtk_widget_set_margin_start(@ptrCast(toolbar), 16);
        c.gtk_widget_set_margin_end(@ptrCast(toolbar), 16);
        c.gtk_widget_set_margin_top(@ptrCast(toolbar), 8);
        c.gtk_widget_set_margin_bottom(@ptrCast(toolbar), 4);

        // Sort dropdown
        const sort_strings: [5:null]?[*:0]const u8 = .{ "Title", "Year", "Rating", "Date Added", null };
        const sort_drop = c.gtk_drop_down_new_from_strings(&sort_strings);
        c.gtk_widget_set_tooltip_text(@ptrCast(sort_drop), "Sort by");
        c.gtk_box_append(@ptrCast(toolbar), @ptrCast(sort_drop));

        // Spacer
        const spacer = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 0);
        c.gtk_widget_set_hexpand(@ptrCast(spacer), 1);
        c.gtk_box_append(@ptrCast(toolbar), spacer);

        // Search entry
        const search = c.gtk_search_entry_new();
        c.gtk_widget_set_size_request(@ptrCast(search), 200, -1);
        c.gtk_box_append(@ptrCast(toolbar), @ptrCast(search));

        c.gtk_box_append(@ptrCast(vbox), @ptrCast(toolbar));

        // Content area - either poster grid or empty state
        const content_stack = c.gtk_stack_new();
        c.gtk_widget_set_vexpand(@ptrCast(content_stack), 1);

        // Empty state
        const status = c.adw_status_page_new();
        c.adw_status_page_set_icon_name(@ptrCast(status), "camera-video-symbolic");
        c.adw_status_page_set_title(@ptrCast(status), "No Movies");
        c.adw_status_page_set_description(@ptrCast(status),
            "Add local folders or connect to Plex in Settings to see your movies.",
        );
        _ = c.gtk_stack_add_named(@ptrCast(content_stack), @ptrCast(status), "empty");

        // Poster grid
        const grid = poster_grid.PosterGrid.init(app.getAllocator());
        _ = c.gtk_stack_add_named(@ptrCast(content_stack), @ptrCast(@alignCast(grid.widget)), "grid");

        c.gtk_box_append(@ptrCast(vbox), @ptrCast(content_stack));

        // Start with empty state; data loaded when view becomes visible
        c.gtk_stack_set_visible_child_name(@ptrCast(content_stack), "empty");

        return MoviesView{
            .widget = @ptrCast(vbox),
            .grid = grid,
            .status_page = @ptrCast(status),
            .grid_container = @ptrCast(content_stack),
        };
    }

    pub fn refresh(self: *MoviesView) void {
        var lib = app.getLibrary() orelse {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.grid_container), "empty");
            return;
        };

        const items = lib.getItemsByType(.movie, .title, .asc, 200, 0) catch {
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
