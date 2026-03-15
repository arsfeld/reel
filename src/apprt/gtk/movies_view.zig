const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});

pub const MoviesView = struct {
    widget: *c.GtkWidget,

    pub fn init() MoviesView {
        const status = c.adw_status_page_new();
        c.adw_status_page_set_icon_name(@ptrCast(status), "camera-video-symbolic");
        c.adw_status_page_set_title(@ptrCast(status), "Movies");
        c.adw_status_page_set_description(@ptrCast(status),
            "Your movie library will appear here.",
        );
        c.gtk_widget_set_vexpand(@ptrCast(status), 1);
        c.gtk_widget_set_hexpand(@ptrCast(status), 1);

        return .{ .widget = @ptrCast(status) };
    }
};
