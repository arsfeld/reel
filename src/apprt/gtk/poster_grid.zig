const std = @import("std");
const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});
const types = @import("../../core/types.zig");
const app = @import("app.zig");

/// A reusable poster grid widget that displays media items as a grid of
/// poster cards (image + title + year). Used by Movies, TV Shows, Other, etc.
pub const PosterGrid = struct {
    widget: *c.GtkWidget,
    flow_box: *c.GtkWidget,
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator) PosterGrid {
        const scrolled = c.gtk_scrolled_window_new();
        c.gtk_widget_set_vexpand(@ptrCast(scrolled), 1);
        c.gtk_widget_set_hexpand(@ptrCast(scrolled), 1);

        const flow_box = c.gtk_flow_box_new();
        c.gtk_flow_box_set_homogeneous(@ptrCast(flow_box), 1);
        c.gtk_flow_box_set_min_children_per_line(@ptrCast(flow_box), 2);
        c.gtk_flow_box_set_max_children_per_line(@ptrCast(flow_box), 8);
        c.gtk_flow_box_set_column_spacing(@ptrCast(flow_box), 12);
        c.gtk_flow_box_set_row_spacing(@ptrCast(flow_box), 16);
        c.gtk_flow_box_set_selection_mode(@ptrCast(flow_box), c.GTK_SELECTION_NONE);
        c.gtk_widget_set_margin_top(@ptrCast(flow_box), 16);
        c.gtk_widget_set_margin_bottom(@ptrCast(flow_box), 16);
        c.gtk_widget_set_margin_start(@ptrCast(flow_box), 16);
        c.gtk_widget_set_margin_end(@ptrCast(flow_box), 16);

        c.gtk_scrolled_window_set_child(@ptrCast(scrolled), @ptrCast(flow_box));

        return .{
            .widget = @ptrCast(scrolled),
            .flow_box = @ptrCast(flow_box),
            .allocator = allocator,
        };
    }

    /// Remove all children from the grid.
    pub fn clear(self: *PosterGrid) void {
        // Remove all children from flow box
        while (true) {
            const child = c.gtk_widget_get_first_child(@ptrCast(self.flow_box));
            if (child == null) break;
            c.gtk_flow_box_remove(@ptrCast(self.flow_box), child);
        }
    }

    /// Add a media item as a poster card to the grid.
    pub fn addItem(self: *PosterGrid, item: types.MediaItem) void {
        const card = createPosterCard(item);
        c.gtk_flow_box_append(@ptrCast(self.flow_box), @ptrCast(card));
    }

    /// Populate the grid with a slice of media items.
    pub fn populate(self: *PosterGrid, items: []const types.MediaItem) void {
        self.clear();
        for (items) |item| {
            self.addItem(item);
        }
    }
};

fn onPosterClicked(_: *c.GtkButton, user_data: ?*anyopaque) callconv(.c) void {
    const id_ptr: *i64 = @ptrCast(@alignCast(user_data orelse return));
    app.showDetail(id_ptr.*);
}

fn createPosterCard(item: types.MediaItem) *c.GtkWidget {
    const button = c.gtk_button_new();
    c.gtk_widget_add_css_class(@ptrCast(button), "flat");
    c.gtk_widget_set_size_request(@ptrCast(button), 150, -1);

    // Store item ID - allocate a stable i64 for the callback
    const id_ptr = std.heap.c_allocator.create(i64) catch return @ptrCast(button);
    id_ptr.* = item.id;
    _ = c.g_signal_connect_data(
        @ptrCast(button),
        "clicked",
        @ptrCast(&onPosterClicked),
        @ptrCast(id_ptr),
        null,
        c.G_CONNECT_DEFAULT,
    );

    const card = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 4);
    c.gtk_widget_set_size_request(@ptrCast(card), 150, -1);

    // Poster image placeholder
    const frame = c.gtk_frame_new(null);
    c.gtk_widget_set_size_request(@ptrCast(frame), 150, 225);

    // If poster_path is available, try to load it; otherwise show a placeholder
    const overlay = c.gtk_overlay_new();

    // Placeholder background with icon
    const placeholder = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 4);
    c.gtk_widget_set_size_request(@ptrCast(placeholder), 150, 225);
    c.gtk_widget_set_halign(@ptrCast(placeholder), c.GTK_ALIGN_CENTER);
    c.gtk_widget_set_valign(@ptrCast(placeholder), c.GTK_ALIGN_CENTER);
    c.gtk_widget_add_css_class(@ptrCast(placeholder), "dim-label");

    const icon_name: [*:0]const u8 = switch (item.media_type) {
        .movie => "camera-video-symbolic",
        .show => "tv-symbolic",
        .episode => "media-playback-start-symbolic",
        .season => "view-list-symbolic",
        .other => "folder-videos-symbolic",
    };
    const icon = c.gtk_image_new_from_icon_name(icon_name);
    c.gtk_image_set_pixel_size(@ptrCast(icon), 48);
    c.gtk_widget_set_opacity(@ptrCast(icon), 0.3);
    c.gtk_widget_set_halign(@ptrCast(icon), c.GTK_ALIGN_CENTER);
    c.gtk_widget_set_valign(@ptrCast(icon), c.GTK_ALIGN_CENTER);
    c.gtk_widget_set_vexpand(@ptrCast(icon), 1);
    c.gtk_box_append(@ptrCast(placeholder), @ptrCast(icon));

    c.gtk_overlay_set_child(@ptrCast(overlay), @ptrCast(placeholder));
    c.gtk_frame_set_child(@ptrCast(frame), @ptrCast(overlay));
    c.gtk_widget_set_overflow(@ptrCast(frame), c.GTK_OVERFLOW_HIDDEN);
    c.gtk_box_append(@ptrCast(card), @ptrCast(frame));

    // Title label
    const title_label = c.gtk_label_new(item.title.ptr);
    c.gtk_label_set_ellipsize(@ptrCast(title_label), c.PANGO_ELLIPSIZE_END);
    c.gtk_label_set_max_width_chars(@ptrCast(title_label), 20);
    c.gtk_label_set_lines(@ptrCast(title_label), 2);
    c.gtk_label_set_wrap(@ptrCast(title_label), 1);
    c.gtk_widget_set_halign(@ptrCast(title_label), c.GTK_ALIGN_START);
    c.gtk_widget_set_margin_top(@ptrCast(title_label), 4);
    c.gtk_widget_set_margin_start(@ptrCast(title_label), 4);
    c.gtk_widget_set_margin_end(@ptrCast(title_label), 4);
    c.gtk_box_append(@ptrCast(card), @ptrCast(title_label));

    // Year label (if present)
    if (item.year) |year| {
        var year_buf: [16]u8 = undefined;
        const year_str = std.fmt.bufPrintZ(&year_buf, "{d}", .{year}) catch "?";
        const year_label = c.gtk_label_new(year_str);
        c.gtk_widget_add_css_class(@ptrCast(year_label), "dim-label");
        c.gtk_widget_add_css_class(@ptrCast(year_label), "caption");
        c.gtk_widget_set_halign(@ptrCast(year_label), c.GTK_ALIGN_START);
        c.gtk_widget_set_margin_start(@ptrCast(year_label), 4);
        c.gtk_widget_set_margin_bottom(@ptrCast(year_label), 4);
        c.gtk_box_append(@ptrCast(card), @ptrCast(year_label));
    }

    c.gtk_button_set_child(@ptrCast(button), @ptrCast(card));
    return @ptrCast(button);
}
