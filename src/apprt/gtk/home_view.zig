const std = @import("std");
const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});
const app = @import("app.zig");
const types = @import("../../core/types.zig");

pub const HomeView = struct {
    widget: *c.GtkWidget,
    content_stack: *c.GtkWidget,
    rows_box: *c.GtkWidget,

    pub fn init() HomeView {
        const content_stack = c.gtk_stack_new();
        c.gtk_widget_set_vexpand(@ptrCast(content_stack), 1);
        c.gtk_widget_set_hexpand(@ptrCast(content_stack), 1);

        // Empty state
        const status = c.adw_status_page_new();
        c.adw_status_page_set_icon_name(@ptrCast(status), "video-display-symbolic");
        c.adw_status_page_set_title(@ptrCast(status), "Welcome to Reel");
        c.adw_status_page_set_description(@ptrCast(status),
            "Connect to Plex or add local folders in Settings to get started.",
        );
        _ = c.gtk_stack_add_named(@ptrCast(content_stack), @ptrCast(status), "empty");

        // Content with rows
        const scrolled = c.gtk_scrolled_window_new();
        c.gtk_widget_set_vexpand(@ptrCast(scrolled), 1);

        const clamp = c.adw_clamp_new();
        c.adw_clamp_set_maximum_size(@ptrCast(clamp), 1400);

        const rows_box = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 24);
        c.gtk_widget_set_margin_top(@ptrCast(rows_box), 16);
        c.gtk_widget_set_margin_bottom(@ptrCast(rows_box), 16);
        c.gtk_widget_set_margin_start(@ptrCast(rows_box), 16);
        c.gtk_widget_set_margin_end(@ptrCast(rows_box), 16);

        c.adw_clamp_set_child(@ptrCast(clamp), @ptrCast(rows_box));
        c.gtk_scrolled_window_set_child(@ptrCast(scrolled), @ptrCast(clamp));
        _ = c.gtk_stack_add_named(@ptrCast(content_stack), scrolled, "content");

        c.gtk_stack_set_visible_child_name(@ptrCast(content_stack), "empty");

        return HomeView{
            .widget = @ptrCast(content_stack),
            .content_stack = @ptrCast(content_stack),
            .rows_box = @ptrCast(rows_box),
        };
    }

    pub fn refresh(self: *HomeView) void {
        // Clear existing rows
        while (true) {
            const child = c.gtk_widget_get_first_child(@ptrCast(self.rows_box));
            if (child == null) break;
            c.gtk_box_remove(@ptrCast(self.rows_box), child);
        }

        var lib = app.getLibrary() orelse {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.content_stack), "empty");
            return;
        };

        var has_content = false;

        // Continue Watching row
        if (lib.getContinueWatching(20)) |items| {
            defer lib.freeMediaItems(items);
            if (items.len > 0) {
                addRow(self.rows_box, "Continue Watching", items);
                has_content = true;
            }
        } else |_| {}

        // Recently Added row
        if (lib.getRecentlyAdded(20)) |items| {
            defer lib.freeMediaItems(items);
            if (items.len > 0) {
                addRow(self.rows_box, "Recently Added", items);
                has_content = true;
            }
        } else |_| {}

        if (has_content) {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.content_stack), "content");
        } else {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.content_stack), "empty");
        }
    }
};

fn addRow(container: *c.GtkWidget, title: [*:0]const u8, items: []const types.MediaItem) void {
    const section = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 8);

    // Section title
    const label = c.gtk_label_new(title);
    c.gtk_widget_add_css_class(@ptrCast(label), "title-3");
    c.gtk_widget_set_halign(@ptrCast(label), c.GTK_ALIGN_START);
    c.gtk_box_append(@ptrCast(section), @ptrCast(label));

    // Horizontal scrollable row of posters
    const scroll = c.gtk_scrolled_window_new();
    c.gtk_scrolled_window_set_policy(@ptrCast(scroll), c.GTK_POLICY_AUTOMATIC, c.GTK_POLICY_NEVER);
    c.gtk_widget_set_size_request(@ptrCast(scroll), -1, 250);

    const hbox = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 12);

    for (items) |item| {
        const card = createSmallPosterCard(item);
        c.gtk_box_append(@ptrCast(hbox), @ptrCast(card));
    }

    c.gtk_scrolled_window_set_child(@ptrCast(scroll), @ptrCast(hbox));
    c.gtk_box_append(@ptrCast(section), scroll);

    c.gtk_box_append(@ptrCast(container), @ptrCast(section));
}

fn createSmallPosterCard(item: types.MediaItem) *c.GtkWidget {
    const card = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 4);
    c.gtk_widget_set_size_request(@ptrCast(card), 130, -1);

    // Poster frame
    const frame = c.gtk_frame_new(null);
    c.gtk_widget_set_size_request(@ptrCast(frame), 130, 195);
    c.gtk_widget_set_overflow(@ptrCast(frame), c.GTK_OVERFLOW_HIDDEN);

    const icon_name: [*:0]const u8 = switch (item.media_type) {
        .movie => "camera-video-symbolic",
        .show => "tv-symbolic",
        .episode => "media-playback-start-symbolic",
        else => "folder-videos-symbolic",
    };
    const icon = c.gtk_image_new_from_icon_name(icon_name);
    c.gtk_image_set_pixel_size(@ptrCast(icon), 36);
    c.gtk_widget_set_opacity(@ptrCast(icon), 0.3);
    c.gtk_widget_set_halign(@ptrCast(icon), c.GTK_ALIGN_CENTER);
    c.gtk_widget_set_valign(@ptrCast(icon), c.GTK_ALIGN_CENTER);
    c.gtk_frame_set_child(@ptrCast(frame), @ptrCast(icon));

    c.gtk_box_append(@ptrCast(card), @ptrCast(frame));

    // Title
    const title_label = c.gtk_label_new(item.title.ptr);
    c.gtk_label_set_ellipsize(@ptrCast(title_label), c.PANGO_ELLIPSIZE_END);
    c.gtk_label_set_max_width_chars(@ptrCast(title_label), 18);
    c.gtk_widget_set_halign(@ptrCast(title_label), c.GTK_ALIGN_START);
    c.gtk_widget_set_margin_top(@ptrCast(title_label), 2);
    c.gtk_box_append(@ptrCast(card), @ptrCast(title_label));

    return @ptrCast(card);
}
