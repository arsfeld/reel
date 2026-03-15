const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});

pub const SettingsView = struct {
    widget: *c.GtkWidget,

    pub fn init() SettingsView {
        const page = c.adw_preferences_page_new();

        // Plex group
        const plex_group = c.adw_preferences_group_new();
        c.adw_preferences_group_set_title(@ptrCast(plex_group), "Plex");
        c.adw_preferences_group_set_description(@ptrCast(plex_group),
            "Connect to a Plex server to stream your media library.",
        );

        const add_server_row = c.adw_action_row_new();
        c.adw_preferences_row_set_title(@ptrCast(add_server_row), "Add Server");
        c.adw_action_row_set_subtitle(@ptrCast(add_server_row), "Connect to a Plex Media Server");
        c.adw_action_row_add_suffix(@ptrCast(add_server_row),
            c.gtk_image_new_from_icon_name("go-next-symbolic"),
        );
        c.adw_action_row_set_activatable_widget(@ptrCast(add_server_row), null);
        c.adw_preferences_group_add(@ptrCast(plex_group), @ptrCast(add_server_row));
        c.adw_preferences_page_add(@ptrCast(page), @ptrCast(plex_group));

        // Library group
        const lib_group = c.adw_preferences_group_new();
        c.adw_preferences_group_set_title(@ptrCast(lib_group), "Library");
        c.adw_preferences_group_set_description(@ptrCast(lib_group),
            "Local folders to scan for media files.",
        );

        const add_folder_row = c.adw_action_row_new();
        c.adw_preferences_row_set_title(@ptrCast(add_folder_row), "Add Folder");
        c.adw_action_row_set_subtitle(@ptrCast(add_folder_row), "Add a local folder to scan for media");
        c.adw_action_row_add_suffix(@ptrCast(add_folder_row),
            c.gtk_image_new_from_icon_name("go-next-symbolic"),
        );
        c.adw_preferences_group_add(@ptrCast(lib_group), @ptrCast(add_folder_row));
        c.adw_preferences_page_add(@ptrCast(page), @ptrCast(lib_group));

        // Metadata group
        const meta_group = c.adw_preferences_group_new();
        c.adw_preferences_group_set_title(@ptrCast(meta_group), "Metadata");

        const tmdb_row = c.adw_entry_row_new();
        c.adw_preferences_row_set_title(@ptrCast(tmdb_row), "TMDB API Key");
        c.adw_preferences_group_add(@ptrCast(meta_group), @ptrCast(tmdb_row));
        c.adw_preferences_page_add(@ptrCast(page), @ptrCast(meta_group));

        return .{ .widget = @ptrCast(page) };
    }
};
