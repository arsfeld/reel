const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});

pub const DownloadsView = struct {
    widget: *c.GtkWidget,

    pub fn init() DownloadsView {
        const status = c.adw_status_page_new();
        c.adw_status_page_set_icon_name(@ptrCast(status), "folder-download-symbolic");
        c.adw_status_page_set_title(@ptrCast(status), "Downloads");
        c.adw_status_page_set_description(@ptrCast(status),
            "No downloads \u{2014} download Plex items for offline viewing.",
        );
        c.gtk_widget_set_vexpand(@ptrCast(status), 1);
        c.gtk_widget_set_hexpand(@ptrCast(status), 1);

        return .{ .widget = @ptrCast(status) };
    }
};
