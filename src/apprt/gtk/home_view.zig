const std = @import("std");
const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
    @cInclude("gdk-pixbuf/gdk-pixbuf.h");
});
const app = @import("app.zig");
const image_loader = @import("image_loader.zig");
const types = @import("../../core/types.zig");
const tmdb_types = @import("../../net/tmdb/types.zig");

pub const HomeView = struct {
    widget: *c.GtkWidget,
    content_stack: *c.GtkWidget,
    rows_box: *c.GtkWidget,
    backdrop_picture: *c.GtkWidget,

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

        // Content with backdrop overlay
        const overlay = c.gtk_overlay_new();
        c.gtk_widget_set_vexpand(@ptrCast(overlay), 1);

        // Backdrop image (full-bleed, dimmed)
        const backdrop_picture = c.gtk_picture_new();
        c.gtk_picture_set_content_fit(@ptrCast(backdrop_picture), c.GTK_CONTENT_FIT_COVER);
        c.gtk_widget_set_opacity(@ptrCast(backdrop_picture), 0.2);
        c.gtk_widget_set_vexpand(@ptrCast(backdrop_picture), 1);
        c.gtk_widget_set_hexpand(@ptrCast(backdrop_picture), 1);
        c.gtk_overlay_set_child(@ptrCast(overlay), @ptrCast(backdrop_picture));

        // Scrollable content on top of backdrop
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
        c.gtk_overlay_add_overlay(@ptrCast(overlay), scrolled);

        _ = c.gtk_stack_add_named(@ptrCast(content_stack), @ptrCast(overlay), "content");

        c.gtk_stack_set_visible_child_name(@ptrCast(content_stack), "empty");

        return HomeView{
            .widget = @ptrCast(content_stack),
            .content_stack = @ptrCast(content_stack),
            .rows_box = @ptrCast(rows_box),
            .backdrop_picture = @ptrCast(backdrop_picture),
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
        var first_backdrop_set = false;

        // Continue Watching row
        if (lib.getContinueWatching(20)) |items| {
            defer lib.freeMediaItems(items);
            if (items.len > 0) {
                addRow(self.rows_box, "Continue Watching", items);
                has_content = true;
                if (!first_backdrop_set) {
                    self.setBackdrop(items[0].backdrop_path);
                    first_backdrop_set = true;
                }
            }
        } else |_| {}

        // Recently Added row
        if (lib.getRecentlyAdded(20)) |items| {
            defer lib.freeMediaItems(items);
            if (items.len > 0) {
                addRow(self.rows_box, "Recently Added", items);
                has_content = true;
                if (!first_backdrop_set) {
                    self.setBackdrop(items[0].backdrop_path);
                    first_backdrop_set = true;
                }
            }
        } else |_| {}

        // Favorites row (media_item type only)
        if (lib.listFavorites()) |favs| {
            defer lib.freeFavorites(favs);
            if (favs.len > 0) {
                // Resolve favorites to media items
                var fav_items: std.ArrayList(types.MediaItem) = .empty;
                defer {
                    for (fav_items.items) |item| lib.freeMediaItem(item);
                    fav_items.deinit(app.getAllocator());
                }

                for (favs) |fav| {
                    if (fav.item_type != .media_item) continue;
                    const fav_id = std.fmt.parseInt(i64, fav.item_id, 10) catch continue;
                    if (lib.getMediaItem(fav_id) catch null) |item| {
                        fav_items.append(app.getAllocator(), item) catch continue;
                    }
                }

                if (fav_items.items.len > 0) {
                    addRow(self.rows_box, "Favorites", fav_items.items);
                    has_content = true;
                }
            }
        } else |_| {}

        // Genre rows (top 10 by item count)
        if (lib.getDistinctGenres()) |genres| {
            defer lib.freeGenres(genres);
            const max_genre_rows: usize = 10;
            const genre_count = @min(genres.len, max_genre_rows);

            for (genres[0..genre_count]) |genre| {
                if (lib.getItemsByGenre(genre.name, 20)) |items| {
                    defer lib.freeMediaItems(items);
                    if (items.len > 0) {
                        // Need null-terminated string for GTK label
                        const allocator = app.getAllocator();
                        const label = allocator.dupeZ(u8, genre.name) catch continue;
                        defer allocator.free(label);
                        addRow(self.rows_box, label, items);
                        has_content = true;
                    }
                } else |_| {}
            }
        } else |_| {}

        // Collection rows (show_on_home = true)
        if (lib.listCollections()) |collections| {
            defer lib.freeCollections(collections);
            for (collections) |col| {
                if (!col.show_on_home) continue;

                const col_items = if (col.collection_type == .smart)
                    lib.evaluateSmartCollection(col.id) catch continue
                else
                    lib.getCollectionItems(col.id) catch continue;
                defer lib.freeMediaItems(col_items);

                if (col_items.len > 0) {
                    const allocator = app.getAllocator();
                    const label = allocator.dupeZ(u8, col.name) catch continue;
                    defer allocator.free(label);
                    addRow(self.rows_box, label, col_items);
                    has_content = true;
                }
            }
        } else |_| {}

        if (has_content) {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.content_stack), "content");
        } else {
            c.gtk_stack_set_visible_child_name(@ptrCast(self.content_stack), "empty");
        }
    }

    fn setBackdrop(self: *HomeView, backdrop_path: ?[]const u8) void {
        _ = image_loader.loadTmdbImage(@ptrCast(self.backdrop_picture), backdrop_path, .w780, -1, -1);
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
    const allocator = app.getAllocator();

    // Clickable button wrapping the card
    const button = c.gtk_button_new();
    c.gtk_widget_add_css_class(@ptrCast(button), "flat");
    c.gtk_widget_set_size_request(@ptrCast(button), 130, -1);

    const card = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 4);

    // Poster image (from cache) or placeholder
    const icon_name: [*:0]const u8 = switch (item.media_type) {
        .movie => "camera-video-symbolic",
        .show => "tv-symbolic",
        .episode => "media-playback-start-symbolic",
        else => "folder-videos-symbolic",
    };
    const poster_widget = image_loader.createPosterPicture(item.poster_path, 130, 195, icon_name);
    c.gtk_box_append(@ptrCast(card), @ptrCast(poster_widget));

    // Title — need to copy since item may be freed before GTK renders
    const title_z = allocator.dupeZ(u8, item.title) catch {
        const title_label = c.gtk_label_new("Unknown");
        c.gtk_box_append(@ptrCast(card), @ptrCast(title_label));
        c.gtk_button_set_child(@ptrCast(button), @ptrCast(card));
        return @ptrCast(button);
    };
    defer allocator.free(title_z);
    const title_label = c.gtk_label_new(title_z.ptr);
    c.gtk_label_set_ellipsize(@ptrCast(title_label), c.PANGO_ELLIPSIZE_END);
    c.gtk_label_set_max_width_chars(@ptrCast(title_label), 18);
    c.gtk_widget_set_halign(@ptrCast(title_label), c.GTK_ALIGN_START);
    c.gtk_widget_set_margin_top(@ptrCast(title_label), 2);
    c.gtk_box_append(@ptrCast(card), @ptrCast(title_label));

    c.gtk_button_set_child(@ptrCast(button), @ptrCast(card));

    // Connect click to show detail
    const id_ptr = allocator.create(i64) catch return @ptrCast(button);
    id_ptr.* = item.id;
    _ = c.g_signal_connect_data(
        @ptrCast(button),
        "clicked",
        @ptrCast(&onPosterClicked),
        @ptrCast(id_ptr),
        @ptrCast(&onPosterDestroy),
        c.G_CONNECT_DEFAULT,
    );

    return @ptrCast(button);
}

fn onPosterClicked(_: *c.GtkButton, user_data: ?*anyopaque) callconv(.c) void {
    const id_ptr: *i64 = @ptrCast(@alignCast(user_data orelse return));
    app.showDetail(id_ptr.*);
}

fn onPosterDestroy(user_data: ?*anyopaque, _: ?*anyopaque) callconv(.c) void {
    const id_ptr: *i64 = @ptrCast(@alignCast(user_data orelse return));
    app.getAllocator().destroy(id_ptr);
}
