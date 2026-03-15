const std = @import("std");

// Core modules
pub const player = @import("core/player.zig");
pub const database = @import("core/database.zig");
pub const types = @import("core/types.zig");
pub const settings = @import("core/settings.zig");
pub const library = @import("core/library.zig");
pub const scanner = @import("core/scanner.zig");
pub const downloader = @import("core/downloader.zig");
pub const image_cache = @import("core/image_cache.zig");

// Network modules
pub const http = @import("net/http.zig");
pub const media_server = @import("net/media_server.zig");
pub const plex_types = @import("net/plex/types.zig");
pub const plex_xml = @import("net/plex/xml.zig");
pub const plex_auth = @import("net/plex/auth.zig");
pub const plex_client = @import("net/plex/client.zig");
pub const tmdb_types = @import("net/tmdb/types.zig");
pub const tmdb_client = @import("net/tmdb/client.zig");

// ── C ABI Exports ──────────────────────────────────────────

const allocator = std.heap.c_allocator;

// Database

export fn reel_db_open(path: [*:0]const u8) ?*database.Database {
    const db = allocator.create(database.Database) catch return null;
    db.* = database.Database.open(path) catch {
        allocator.destroy(db);
        return null;
    };
    return db;
}

export fn reel_db_close(db: ?*database.Database) void {
    const d = db orelse return;
    d.close();
    allocator.destroy(d);
}

// Library

export fn reel_library_create(db: ?*database.Database) ?*library.Library {
    const d = db orelse return null;
    const lib = allocator.create(library.Library) catch return null;
    lib.* = library.Library.init(allocator, d);
    return lib;
}

export fn reel_library_destroy(lib: ?*library.Library) void {
    const l = lib orelse return;
    allocator.destroy(l);
}

export fn reel_library_get_item_count(lib: ?*library.Library, media_type: c_int) i32 {
    const l = lib orelse return 0;
    const mt = intToMediaType(media_type) orelse return 0;
    return @intCast(l.getItemCount(mt) catch return 0);
}

export fn reel_library_server_count(lib: ?*library.Library) i32 {
    const l = lib orelse return 0;
    const servers = l.listServers() catch return 0;
    defer l.freeServers(servers);
    return @intCast(servers.len);
}

export fn reel_library_add_favorite(
    lib: ?*library.Library,
    item_type: [*:0]const u8,
    item_id: [*:0]const u8,
    display_name: [*:0]const u8,
) i64 {
    const l = lib orelse return -1;
    const ft = types.FavoriteType.fromString(std.mem.span(item_type)) orelse return -1;
    return l.addFavorite(.{
        .item_type = ft,
        .item_id = std.mem.span(item_id),
        .display_name = std.mem.span(display_name),
    }) catch return -1;
}

export fn reel_library_remove_favorite(lib: ?*library.Library, id: i64) c_int {
    const l = lib orelse return -1;
    l.removeFavorite(id) catch return -1;
    return 0;
}

export fn reel_library_add_scan_path(lib: ?*library.Library, path: [*:0]const u8) i64 {
    const l = lib orelse return -1;
    return l.insertScanPath(std.mem.span(path)) catch return -1;
}

export fn reel_library_remove_scan_path(lib: ?*library.Library, id: i64) c_int {
    const l = lib orelse return -1;
    l.deleteScanPath(id) catch return -1;
    return 0;
}

// Settings

export fn reel_settings_get(db: ?*database.Database, key: [*:0]const u8) ?[*:0]const u8 {
    const d = db orelse return null;
    var s = settings.Settings.init(allocator, d);
    const val = (s.getString(std.mem.span(key)) catch return null) orelse return null;
    // Caller must free this string (or we leak — acceptable for simple bridging)
    return @ptrCast(val.ptr);
}

export fn reel_settings_set(db: ?*database.Database, key: [*:0]const u8, value: [*:0]const u8) c_int {
    const d = db orelse return -1;
    var s = settings.Settings.init(allocator, d);
    s.setString(std.mem.span(key), std.mem.span(value)) catch return -1;
    return 0;
}

// Downloads

export fn reel_download_create(db: ?*database.Database) ?*downloader.Downloader {
    const d = db orelse return null;
    const dl = allocator.create(downloader.Downloader) catch return null;
    dl.* = downloader.Downloader.init(allocator, d);
    return dl;
}

export fn reel_download_destroy(dl: ?*downloader.Downloader) void {
    const d = dl orelse return;
    allocator.destroy(d);
}

export fn reel_download_enqueue(
    dl: ?*downloader.Downloader,
    media_item_id: i64,
    server_id: [*:0]const u8,
    source_url: [*:0]const u8,
    download_dir: [*:0]const u8,
    filename: [*:0]const u8,
) i64 {
    const d = dl orelse return -1;
    return d.enqueue(.{
        .media_item_id = media_item_id,
        .server_id = std.mem.span(server_id),
        .source_url = std.mem.span(source_url),
        .download_dir = std.mem.span(download_dir),
        .filename = std.mem.span(filename),
    }) catch return -1;
}

export fn reel_download_pause(dl: ?*downloader.Downloader, id: i64) c_int {
    const d = dl orelse return -1;
    d.pause(id) catch return -1;
    return 0;
}

export fn reel_download_resume(dl: ?*downloader.Downloader, id: i64) c_int {
    const d = dl orelse return -1;
    d.resumeDownload(id) catch return -1;
    return 0;
}

export fn reel_download_remove(dl: ?*downloader.Downloader, id: i64, delete_file: bool) c_int {
    const d = dl orelse return -1;
    d.remove(id, delete_file) catch return -1;
    return 0;
}

export fn reel_download_get_local_path(dl: ?*downloader.Downloader, media_item_id: i64) ?[*:0]const u8 {
    const d = dl orelse return null;
    const path = (d.getCompletedLocalPath(media_item_id) catch return null) orelse return null;
    // Caller responsible for memory (acceptable for bridging)
    return @ptrCast(path.ptr);
}

// Collections

export fn reel_collection_create(
    lib: ?*library.Library,
    name: [*:0]const u8,
    collection_type: c_int,
    description: ?[*:0]const u8,
) i64 {
    const l = lib orelse return -1;
    const ct: types.CollectionType = switch (collection_type) {
        0 => .manual,
        1 => .smart,
        else => return -1,
    };
    const desc: ?[]const u8 = if (description) |d| std.mem.span(d) else null;
    return l.createCollection(std.mem.span(name), ct, desc) catch return -1;
}

export fn reel_collection_delete(lib: ?*library.Library, id: i64) c_int {
    const l = lib orelse return -1;
    l.deleteCollection(id) catch return -1;
    return 0;
}

/// Writes a flat array of ReelCollectionC structs into caller-provided output.
/// Returns the number of collections written, or -1 on error.
const ReelCollectionC = extern struct {
    id: i64,
    name: [*:0]const u8,
    collection_type: c_int, // 0=manual, 1=smart
    description: ?[*:0]const u8,
};

export fn reel_collection_list(
    lib: ?*library.Library,
    out_ptr: ?[*]ReelCollectionC,
    out_count: ?*i32,
) c_int {
    const l = lib orelse return -1;
    const count_ptr = out_count orelse return -1;

    const cols = l.listCollections() catch return -1;

    // If out_ptr is null, just return the count
    if (out_ptr == null) {
        count_ptr.* = @intCast(cols.len);
        l.freeCollections(cols);
        return 0;
    }

    const out = out_ptr.?;
    for (cols, 0..) |col, i| {
        out[i] = .{
            .id = col.id,
            .name = @ptrCast(allocator.dupeZ(u8, col.name) catch {
                l.freeCollections(cols);
                return -1;
            }),
            .collection_type = switch (col.collection_type) {
                .manual => 0,
                .smart => 1,
            },
            .description = if (col.description) |d|
                @ptrCast(allocator.dupeZ(u8, d) catch {
                    l.freeCollections(cols);
                    return -1;
                })
            else
                null,
        };
    }
    count_ptr.* = @intCast(cols.len);
    l.freeCollections(cols);
    return 0;
}

export fn reel_collection_add_item(lib: ?*library.Library, collection_id: i64, media_item_id: i64) c_int {
    const l = lib orelse return -1;
    l.addToCollection(collection_id, media_item_id) catch return -1;
    return 0;
}

export fn reel_collection_remove_item(lib: ?*library.Library, collection_id: i64, media_item_id: i64) c_int {
    const l = lib orelse return -1;
    l.removeFromCollection(collection_id, media_item_id) catch return -1;
    return 0;
}

// Genres

export fn reel_genre_set(
    lib: ?*library.Library,
    media_item_id: i64,
    genre_names: ?[*]const [*:0]const u8,
    count: c_int,
) c_int {
    const l = lib orelse return -1;
    if (count < 0) return -1;
    const cnt: usize = @intCast(count);
    if (cnt == 0) {
        // Clear all genres for this item
        l.setMediaItemGenres(media_item_id, &.{}) catch return -1;
        return 0;
    }
    const names_ptr = genre_names orelse return -1;

    // Convert C string array to Zig slices
    var name_slices = allocator.alloc([]const u8, cnt) catch return -1;
    defer allocator.free(name_slices);
    for (0..cnt) |i| {
        name_slices[i] = std.mem.span(names_ptr[i]);
    }
    l.setMediaItemGenres(media_item_id, name_slices) catch return -1;
    return 0;
}

// Match lock

export fn reel_match_set_locked(lib: ?*library.Library, media_item_id: i64, locked: bool) c_int {
    const l = lib orelse return -1;
    l.setMatchLocked(media_item_id, locked) catch return -1;
    return 0;
}

// Helpers

fn intToMediaType(val: c_int) ?types.MediaType {
    return switch (val) {
        0 => .movie,
        1 => .show,
        2 => .season,
        3 => .episode,
        4 => .other,
        else => null,
    };
}

test {
    std.testing.refAllDecls(@This());
}
