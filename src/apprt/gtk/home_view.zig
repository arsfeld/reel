const std = @import("std");
const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});

pub const HomeView = struct {
    widget: *c.GtkWidget,

    pub fn init() HomeView {
        const scrolled = c.gtk_scrolled_window_new();
        c.gtk_widget_set_vexpand(@ptrCast(scrolled), 1);
        c.gtk_widget_set_hexpand(@ptrCast(scrolled), 1);

        const clamp = c.adw_clamp_new();
        c.adw_clamp_set_maximum_size(@ptrCast(clamp), 1200);

        const vbox = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 24);
        c.gtk_widget_set_margin_top(@ptrCast(vbox), 24);
        c.gtk_widget_set_margin_bottom(@ptrCast(vbox), 24);
        c.gtk_widget_set_margin_start(@ptrCast(vbox), 16);
        c.gtk_widget_set_margin_end(@ptrCast(vbox), 16);

        // Welcome / empty state
        const status = c.adw_status_page_new();
        c.adw_status_page_set_icon_name(@ptrCast(status), "video-display-symbolic");
        c.adw_status_page_set_title(@ptrCast(status), "Welcome to Reel");
        c.adw_status_page_set_description(@ptrCast(status),
            "Connect to Plex or add local folders in Settings to get started.",
        );
        c.gtk_box_append(@ptrCast(vbox), @ptrCast(status));

        c.adw_clamp_set_child(@ptrCast(clamp), @ptrCast(vbox));
        c.gtk_scrolled_window_set_child(@ptrCast(scrolled), @ptrCast(clamp));

        return .{ .widget = @ptrCast(scrolled) };
    }
};
