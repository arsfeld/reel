const std = @import("std");
const c = @cImport({
    @cInclude("gtk/gtk.h");
    @cInclude("gdk-pixbuf/gdk-pixbuf.h");
});
const tmdb_types = @import("../../net/tmdb/types.zig");
const image_cache = @import("../../core/image_cache.zig");
const app = @import("app.zig");

/// Try to load a TMDB image into a GtkPicture widget.
/// Returns true if image was loaded, false if placeholder should be shown.
pub fn loadTmdbImage(
    picture: *c.GtkWidget,
    tmdb_path: ?[]const u8,
    size: tmdb_types.ImageSize,
    width: c_int,
    height: c_int,
) bool {
    const path = tmdb_path orelse return false;
    if (path.len == 0) return false;

    const allocator = app.getAllocator();

    // Build full TMDB URL
    const url = tmdb_types.imageUrl(allocator, size, path) catch return false;
    defer allocator.free(url);

    // Check image cache for local file
    var cache = app.getImageCache() orelse return false;
    const local_path = cache.getLocalPath(url) catch return false orelse return false;
    defer allocator.free(local_path);

    // Load the image from local file at the requested size
    const local_path_z = allocator.dupeZ(u8, local_path) catch return false;
    defer allocator.free(local_path_z);

    const pixbuf = c.gdk_pixbuf_new_from_file_at_scale(
        local_path_z.ptr,
        width,
        height,
        1, // preserve_aspect_ratio
        null,
    );
    if (pixbuf == null) return false;
    defer c.g_object_unref(@ptrCast(pixbuf));

    const texture = c.gdk_texture_new_for_pixbuf(pixbuf);
    if (texture == null) return false;
    defer c.g_object_unref(@ptrCast(texture));

    c.gtk_picture_set_paintable(@ptrCast(picture), @ptrCast(texture));
    return true;
}

/// Create a GtkPicture widget sized for a poster (130x195 default).
/// Attempts to load from cache, falls back to a placeholder icon.
pub fn createPosterPicture(
    tmdb_path: ?[]const u8,
    width: c_int,
    height: c_int,
    media_type_icon: [*:0]const u8,
) *c.GtkWidget {
    const picture = c.gtk_picture_new();
    c.gtk_widget_set_size_request(@ptrCast(picture), width, height);
    c.gtk_picture_set_content_fit(@ptrCast(picture), c.GTK_CONTENT_FIT_COVER);

    if (!loadTmdbImage(@ptrCast(picture), tmdb_path, .w342, width, height)) {
        // Show placeholder icon instead
        const icon = c.gtk_image_new_from_icon_name(media_type_icon);
        c.gtk_image_set_pixel_size(@ptrCast(icon), 36);
        c.gtk_widget_set_opacity(@ptrCast(icon), 0.3);
        // Return the icon wrapped in a frame for consistent sizing
        const frame = c.gtk_frame_new(null);
        c.gtk_widget_set_size_request(@ptrCast(frame), width, height);
        c.gtk_widget_set_overflow(@ptrCast(frame), c.GTK_OVERFLOW_HIDDEN);
        c.gtk_widget_set_halign(@ptrCast(icon), c.GTK_ALIGN_CENTER);
        c.gtk_widget_set_valign(@ptrCast(icon), c.GTK_ALIGN_CENTER);
        c.gtk_frame_set_child(@ptrCast(frame), @ptrCast(icon));

        // Can't use picture widget, return the frame instead
        // Caller should not use the picture anymore
        return @ptrCast(frame);
    }

    return @ptrCast(picture);
}
