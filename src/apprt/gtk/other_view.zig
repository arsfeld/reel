const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});

pub const OtherView = struct {
    widget: *c.GtkWidget,

    pub fn init() OtherView {
        const status = c.adw_status_page_new();
        c.adw_status_page_set_icon_name(@ptrCast(status), "folder-videos-symbolic");
        c.adw_status_page_set_title(@ptrCast(status), "Other");
        c.adw_status_page_set_description(@ptrCast(status),
            "Other media files will appear here.",
        );
        c.gtk_widget_set_vexpand(@ptrCast(status), 1);
        c.gtk_widget_set_hexpand(@ptrCast(status), 1);

        return .{ .widget = @ptrCast(status) };
    }
};
