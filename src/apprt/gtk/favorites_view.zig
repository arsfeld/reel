const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});

pub const FavoritesView = struct {
    widget: *c.GtkWidget,

    pub fn init() FavoritesView {
        const status = c.adw_status_page_new();
        c.adw_status_page_set_icon_name(@ptrCast(status), "starred-symbolic");
        c.adw_status_page_set_title(@ptrCast(status), "Favorites");
        c.adw_status_page_set_description(@ptrCast(status),
            "No favorites yet \u{2014} right-click any item to add it.",
        );
        c.gtk_widget_set_vexpand(@ptrCast(status), 1);
        c.gtk_widget_set_hexpand(@ptrCast(status), 1);

        return .{ .widget = @ptrCast(status) };
    }
};
