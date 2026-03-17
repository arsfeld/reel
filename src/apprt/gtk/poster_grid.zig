const std = @import("std");
const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});
const types = @import("../../core/types.zig");
const app = @import("app.zig");
const image_loader = @import("image_loader.zig");

const card_w: c_int = 154;
const card_h: c_int = 231;

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
        c.gtk_flow_box_set_max_children_per_line(@ptrCast(flow_box), 20);
        c.gtk_flow_box_set_column_spacing(@ptrCast(flow_box), 4);
        c.gtk_flow_box_set_row_spacing(@ptrCast(flow_box), 8);
        c.gtk_flow_box_set_selection_mode(@ptrCast(flow_box), c.GTK_SELECTION_NONE);
        c.gtk_widget_set_margin_top(@ptrCast(flow_box), 8);
        c.gtk_widget_set_margin_bottom(@ptrCast(flow_box), 8);
        c.gtk_widget_set_margin_start(@ptrCast(flow_box), 8);
        c.gtk_widget_set_margin_end(@ptrCast(flow_box), 8);

        c.gtk_scrolled_window_set_child(@ptrCast(scrolled), @ptrCast(flow_box));

        return .{
            .widget = @ptrCast(scrolled),
            .flow_box = @ptrCast(flow_box),
            .allocator = allocator,
        };
    }

    pub fn clear(self: *PosterGrid) void {
        while (true) {
            const child = c.gtk_widget_get_first_child(@ptrCast(self.flow_box));
            if (child == null) break;
            c.gtk_flow_box_remove(@ptrCast(self.flow_box), child);
        }
    }

    pub fn addItem(self: *PosterGrid, item: types.MediaItem) void {
        const card = createPosterCard(item);
        c.gtk_flow_box_append(@ptrCast(self.flow_box), @ptrCast(card));
    }

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
    // The entire card is a fixed-size clickable button
    const button = c.gtk_button_new();
    c.gtk_widget_add_css_class(@ptrCast(button), "flat");
    c.gtk_widget_set_size_request(@ptrCast(button), card_w, -1);

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

    // Poster: fixed size, clipped, all posters same dimensions
    const picture = c.gtk_picture_new();
    c.gtk_widget_set_size_request(@ptrCast(picture), card_w, card_h);
    c.gtk_widget_set_hexpand(@ptrCast(picture), 0);
    c.gtk_widget_set_vexpand(@ptrCast(picture), 0);
    c.gtk_widget_set_overflow(@ptrCast(picture), c.GTK_OVERFLOW_HIDDEN);
    c.gtk_picture_set_content_fit(@ptrCast(picture), c.GTK_CONTENT_FIT_COVER);
    c.gtk_box_append(@ptrCast(card), @ptrCast(picture));

    if (item.poster_path != null) {
        image_loader.loadImageFromUrl(@ptrCast(picture), item.poster_path, card_w * 2, card_h * 2);
    } else {
        // Set a placeholder icon as the picture's paintable would be empty
        // Just leave blank — the fixed size ensures consistent layout
    }

    // Title
    const title_z = std.heap.c_allocator.dupeZ(u8, item.title) catch return @ptrCast(button);
    defer std.heap.c_allocator.free(title_z);
    const title_label = c.gtk_label_new(title_z.ptr);
    c.gtk_label_set_ellipsize(@ptrCast(title_label), c.PANGO_ELLIPSIZE_END);
    c.gtk_label_set_max_width_chars(@ptrCast(title_label), 18);
    c.gtk_widget_set_halign(@ptrCast(title_label), c.GTK_ALIGN_START);
    c.gtk_widget_set_margin_top(@ptrCast(title_label), 2);
    c.gtk_box_append(@ptrCast(card), @ptrCast(title_label));

    // Year
    if (item.year) |year| {
        var year_buf: [16]u8 = undefined;
        const year_str = std.fmt.bufPrintZ(&year_buf, "{d}", .{year}) catch "?";
        const year_label = c.gtk_label_new(year_str);
        c.gtk_widget_add_css_class(@ptrCast(year_label), "dim-label");
        c.gtk_widget_add_css_class(@ptrCast(year_label), "caption");
        c.gtk_widget_set_halign(@ptrCast(year_label), c.GTK_ALIGN_START);
        c.gtk_box_append(@ptrCast(card), @ptrCast(year_label));
    }

    c.gtk_button_set_child(@ptrCast(button), @ptrCast(card));
    return @ptrCast(button);
}
