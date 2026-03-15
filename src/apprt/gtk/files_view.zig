const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});

pub const FilesView = struct {
    widget: *c.GtkWidget,

    pub fn init() FilesView {
        const status = c.adw_status_page_new();
        c.adw_status_page_set_icon_name(@ptrCast(status), "network-server-symbolic");
        c.adw_status_page_set_title(@ptrCast(status), "Files");
        c.adw_status_page_set_description(@ptrCast(status),
            "Connected sources will appear here. Add servers or folders in Settings.",
        );
        c.gtk_widget_set_vexpand(@ptrCast(status), 1);
        c.gtk_widget_set_hexpand(@ptrCast(status), 1);

        return .{ .widget = @ptrCast(status) };
    }
};
