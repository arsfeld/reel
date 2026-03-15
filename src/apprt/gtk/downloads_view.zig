const std = @import("std");
const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});

pub const DownloadsView = struct {
    widget: *c.GtkWidget,

    pub fn init() DownloadsView {
        const content_stack = c.gtk_stack_new();
        c.gtk_widget_set_vexpand(@ptrCast(content_stack), 1);
        c.gtk_widget_set_hexpand(@ptrCast(content_stack), 1);

        // Empty state
        const status = c.adw_status_page_new();
        c.adw_status_page_set_icon_name(@ptrCast(status), "folder-download-symbolic");
        c.adw_status_page_set_title(@ptrCast(status), "No Downloads");
        c.adw_status_page_set_description(@ptrCast(status),
            "Download Plex items for offline viewing. Downloads will appear here.",
        );
        _ = c.gtk_stack_add_named(@ptrCast(content_stack), @ptrCast(status), "empty");

        // Downloads list (for future use)
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

        return .{ .widget = @ptrCast(content_stack) };
    }
};
