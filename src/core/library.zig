const std = @import("std");
const database = @import("database.zig");
const types = @import("types.zig");

pub const Library = struct {
    db: *database.Database,
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator, db: *database.Database) Library {
        return .{ .db = db, .allocator = allocator };
    }

    pub fn insertMediaItem(self: *Library, item: types.MediaItem) !i64 {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare(
            \\INSERT INTO media_items
            \\  (source, source_id, server_id, media_type, title, sort_title,
            \\   year, summary, rating, duration_ms, poster_path, backdrop_path,
            \\   tmdb_id, parent_id, season_number, episode_number, file_path,
            \\   added_at, updated_at)
            \\VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        );
        defer stmt.finalize();

        stmt.bindText(1, item.source.toString());
        stmt.bindOptionalText(2, item.source_id);
        stmt.bindOptionalText(3, item.server_id);
        stmt.bindText(4, item.media_type.toString());
        stmt.bindText(5, item.title);
        stmt.bindOptionalText(6, item.sort_title);
        stmt.bindOptionalInt(7, item.year);
        stmt.bindOptionalText(8, item.summary);
        stmt.bindOptionalDouble(9, item.rating);
        stmt.bindOptionalInt64(10, item.duration_ms);
        stmt.bindOptionalText(11, item.poster_path);
        stmt.bindOptionalText(12, item.backdrop_path);
        stmt.bindOptionalInt(13, item.tmdb_id);
        stmt.bindOptionalInt64(14, item.parent_id);
        stmt.bindOptionalInt(15, item.season_number);
        stmt.bindOptionalInt(16, item.episode_number);
        stmt.bindOptionalText(17, item.file_path);
        stmt.bindOptionalInt64(18, item.added_at);
        stmt.bindOptionalInt64(19, item.updated_at);

        try stmt.exec();
        return stmt.lastInsertRowId();
    }

    pub fn getMediaItem(self: *Library, id: i64) !?types.MediaItem {
        var stmt = try self.db.prepare(
            \\SELECT id, source, source_id, server_id, media_type, title,
            \\       sort_title, year, summary, rating, duration_ms,
            \\       poster_path, backdrop_path, tmdb_id, parent_id,
            \\       season_number, episode_number, file_path, added_at, updated_at
            \\FROM media_items WHERE id = ?
        );
        defer stmt.finalize();
        stmt.bindInt64(1, id);

        if (stmt.step()) {
            return try readMediaItem(self.allocator, &stmt);
        }
        return null;
    }

    pub fn getBySourceId(self: *Library, source: types.MediaSource, source_id: []const u8) !?types.MediaItem {
        var stmt = try self.db.prepare(
            \\SELECT id, source, source_id, server_id, media_type, title,
            \\       sort_title, year, summary, rating, duration_ms,
            \\       poster_path, backdrop_path, tmdb_id, parent_id,
            \\       season_number, episode_number, file_path, added_at, updated_at
            \\FROM media_items WHERE source = ? AND source_id = ?
        );
        defer stmt.finalize();
        stmt.bindText(1, source.toString());
        stmt.bindText(2, source_id);

        if (stmt.step()) {
            return try readMediaItem(self.allocator, &stmt);
        }
        return null;
    }

    pub fn updateWatchProgress(self: *Library, progress: types.WatchProgress) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare(
            \\INSERT OR REPLACE INTO watch_progress
            \\  (media_item_id, position_ms, duration_ms, watched, last_watched_at)
            \\VALUES (?, ?, ?, ?, ?)
        );
        defer stmt.finalize();

        stmt.bindInt64(1, progress.media_item_id);
        stmt.bindInt64(2, progress.position_ms);
        stmt.bindOptionalInt64(3, progress.duration_ms);
        stmt.bindInt(4, if (progress.watched) 1 else 0);
        stmt.bindOptionalInt64(5, progress.last_watched_at);

        try stmt.exec();
    }

    pub fn getWatchProgress(self: *Library, media_item_id: i64) !?types.WatchProgress {
        var stmt = try self.db.prepare(
            \\SELECT media_item_id, position_ms, duration_ms, watched, last_watched_at
            \\FROM watch_progress WHERE media_item_id = ?
        );
        defer stmt.finalize();
        stmt.bindInt64(1, media_item_id);

        if (stmt.step()) {
            return types.WatchProgress{
                .media_item_id = stmt.columnInt64(0),
                .position_ms = stmt.columnInt64(1),
                .duration_ms = stmt.columnOptionalInt64(2),
                .watched = stmt.columnBool(3),
                .last_watched_at = stmt.columnOptionalInt64(4),
            };
        }
        return null;
    }

    pub fn deleteMediaItem(self: *Library, id: i64) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var wp_stmt = try self.db.prepare("DELETE FROM watch_progress WHERE media_item_id = ?");
        defer wp_stmt.finalize();
        wp_stmt.bindInt64(1, id);
        try wp_stmt.exec();

        var stmt = try self.db.prepare("DELETE FROM media_items WHERE id = ?");
        defer stmt.finalize();
        stmt.bindInt64(1, id);
        try stmt.exec();
    }

    // Query operations

    pub const SortField = enum {
        title,
        year,
        rating,
        added_at,
    };

    pub const SortOrder = enum {
        asc,
        desc,
    };

    pub fn getItemsByType(
        self: *Library,
        media_type: types.MediaType,
        sort_by: SortField,
        sort_order: SortOrder,
        limit: u32,
        offset: u32,
    ) ![]types.MediaItem {
        // Build SQL with sort clause. We use comptime-known sort strings
        // but must pick at runtime, so use a buffer approach.
        const order_clause = switch (sort_by) {
            .title => "COALESCE(sort_title, title)",
            .year => "year",
            .rating => "rating",
            .added_at => "added_at",
        };
        const direction = switch (sort_order) {
            .asc => "ASC",
            .desc => "DESC",
        };

        var sql_buf: [512]u8 = undefined;
        const sql = std.fmt.bufPrintZ(&sql_buf,
            \\SELECT id, source, source_id, server_id, media_type, title,
            \\       sort_title, year, summary, rating, duration_ms,
            \\       poster_path, backdrop_path, tmdb_id, parent_id,
            \\       season_number, episode_number, file_path, added_at, updated_at
            \\FROM media_items WHERE media_type = ?
            \\ORDER BY {s} {s} LIMIT ? OFFSET ?
        , .{ order_clause, direction }) catch return error.SqlFormatFailed;

        var stmt = try self.db.prepare(sql);
        defer stmt.finalize();
        stmt.bindText(1, media_type.toString());
        stmt.bindInt(2, @intCast(limit));
        stmt.bindInt(3, @intCast(offset));

        return self.collectMediaItems(&stmt);
    }

    pub fn getRecentlyAdded(self: *Library, limit: u32) ![]types.MediaItem {
        var stmt = try self.db.prepare(
            \\SELECT id, source, source_id, server_id, media_type, title,
            \\       sort_title, year, summary, rating, duration_ms,
            \\       poster_path, backdrop_path, tmdb_id, parent_id,
            \\       season_number, episode_number, file_path, added_at, updated_at
            \\FROM media_items
            \\WHERE media_type IN ('movie', 'show')
            \\ORDER BY added_at DESC LIMIT ?
        );
        defer stmt.finalize();
        stmt.bindInt(1, @intCast(limit));

        return self.collectMediaItems(&stmt);
    }

    pub fn getContinueWatching(self: *Library, limit: u32) ![]types.MediaItem {
        var stmt = try self.db.prepare(
            \\SELECT m.id, m.source, m.source_id, m.server_id, m.media_type, m.title,
            \\       m.sort_title, m.year, m.summary, m.rating, m.duration_ms,
            \\       m.poster_path, m.backdrop_path, m.tmdb_id, m.parent_id,
            \\       m.season_number, m.episode_number, m.file_path, m.added_at, m.updated_at
            \\FROM media_items m
            \\JOIN watch_progress wp ON m.id = wp.media_item_id
            \\WHERE wp.watched = 0 AND wp.position_ms > 0
            \\ORDER BY wp.last_watched_at DESC LIMIT ?
        );
        defer stmt.finalize();
        stmt.bindInt(1, @intCast(limit));

        return self.collectMediaItems(&stmt);
    }

    pub fn searchItems(self: *Library, query: []const u8, media_type_filter: ?types.MediaType) ![]types.MediaItem {
        if (media_type_filter) |mt| {
            var stmt = try self.db.prepare(
                \\SELECT id, source, source_id, server_id, media_type, title,
                \\       sort_title, year, summary, rating, duration_ms,
                \\       poster_path, backdrop_path, tmdb_id, parent_id,
                \\       season_number, episode_number, file_path, added_at, updated_at
                \\FROM media_items WHERE title LIKE ? AND media_type = ?
                \\ORDER BY title LIMIT 50
            );
            defer stmt.finalize();

            var pattern_buf: [256]u8 = undefined;
            const pattern = std.fmt.bufPrint(&pattern_buf, "%{s}%", .{query}) catch return error.SqlFormatFailed;
            stmt.bindText(1, pattern);
            stmt.bindText(2, mt.toString());
            return self.collectMediaItems(&stmt);
        } else {
            var stmt = try self.db.prepare(
                \\SELECT id, source, source_id, server_id, media_type, title,
                \\       sort_title, year, summary, rating, duration_ms,
                \\       poster_path, backdrop_path, tmdb_id, parent_id,
                \\       season_number, episode_number, file_path, added_at, updated_at
                \\FROM media_items WHERE title LIKE ?
                \\ORDER BY title LIMIT 50
            );
            defer stmt.finalize();

            var pattern_buf: [256]u8 = undefined;
            const pattern = std.fmt.bufPrint(&pattern_buf, "%{s}%", .{query}) catch return error.SqlFormatFailed;
            stmt.bindText(1, pattern);
            return self.collectMediaItems(&stmt);
        }
    }

    pub fn getItemsByParent(self: *Library, parent_id: i64) ![]types.MediaItem {
        var stmt = try self.db.prepare(
            \\SELECT id, source, source_id, server_id, media_type, title,
            \\       sort_title, year, summary, rating, duration_ms,
            \\       poster_path, backdrop_path, tmdb_id, parent_id,
            \\       season_number, episode_number, file_path, added_at, updated_at
            \\FROM media_items WHERE parent_id = ?
            \\ORDER BY season_number, episode_number, title
        );
        defer stmt.finalize();
        stmt.bindInt64(1, parent_id);

        return self.collectMediaItems(&stmt);
    }

    pub fn getItemCount(self: *Library, media_type: types.MediaType) !u32 {
        var stmt = try self.db.prepare(
            "SELECT COUNT(*) FROM media_items WHERE media_type = ?"
        );
        defer stmt.finalize();
        stmt.bindText(1, media_type.toString());

        if (stmt.step()) {
            return @intCast(stmt.columnInt(0));
        }
        return 0;
    }

    pub fn listServers(self: *Library) ![]types.Server {
        var stmt = try self.db.prepare(
            \\SELECT id, name, client_identifier, auth_token, connection_uri, last_connected_at
            \\FROM servers ORDER BY name
        );
        defer stmt.finalize();

        var results: std.ArrayList(types.Server) = .{};
        while (stmt.step()) {
            try results.append(self.allocator, types.Server{
                .id = try dupeText(self.allocator, stmt.columnText(0) orelse continue),
                .name = try dupeText(self.allocator, stmt.columnText(1) orelse ""),
                .client_identifier = try dupeText(self.allocator, stmt.columnText(2) orelse ""),
                .auth_token = try dupeOptionalText(self.allocator, stmt.columnText(3)),
                .connection_uri = try dupeOptionalText(self.allocator, stmt.columnText(4)),
                .last_connected_at = stmt.columnOptionalInt64(5),
            });
        }
        return results.toOwnedSlice(self.allocator);
    }

    pub fn freeServers(self: *Library, servers: []types.Server) void {
        for (servers) |s| self.freeServer(s);
        self.allocator.free(servers);
    }

    pub fn listScanPaths(self: *Library) ![]types.ScanPath {
        var stmt = try self.db.prepare(
            "SELECT id, path, last_scanned_at FROM scan_paths ORDER BY path"
        );
        defer stmt.finalize();

        var results: std.ArrayList(types.ScanPath) = .{};
        while (stmt.step()) {
            try results.append(self.allocator, types.ScanPath{
                .id = stmt.columnInt64(0),
                .path = try dupeText(self.allocator, stmt.columnText(1) orelse continue),
                .last_scanned_at = stmt.columnOptionalInt64(2),
            });
        }
        return results.toOwnedSlice(self.allocator);
    }

    pub fn freeScanPaths(self: *Library, paths: []types.ScanPath) void {
        for (paths) |p| self.allocator.free(p.path);
        self.allocator.free(paths);
    }

    pub fn insertScanPath(self: *Library, path: []const u8) !i64 {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare(
            "INSERT OR IGNORE INTO scan_paths (path) VALUES (?)"
        );
        defer stmt.finalize();
        stmt.bindText(1, path);
        try stmt.exec();
        return stmt.lastInsertRowId();
    }

    pub fn deleteScanPath(self: *Library, id: i64) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare("DELETE FROM scan_paths WHERE id = ?");
        defer stmt.finalize();
        stmt.bindInt64(1, id);
        try stmt.exec();
    }

    // Favorites operations

    pub fn addFavorite(self: *Library, fav: types.Favorite) !i64 {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        // Get max sort_order
        var max_stmt = try self.db.prepare("SELECT COALESCE(MAX(sort_order), -1) FROM favorites");
        defer max_stmt.finalize();
        var next_order: i32 = 0;
        if (max_stmt.step()) {
            next_order = max_stmt.columnInt(0) + 1;
        }

        var stmt = try self.db.prepare(
            \\INSERT INTO favorites (item_type, item_id, display_name, sort_order, created_at)
            \\VALUES (?, ?, ?, ?, ?)
        );
        defer stmt.finalize();
        stmt.bindText(1, fav.item_type.toString());
        stmt.bindText(2, fav.item_id);
        stmt.bindText(3, fav.display_name);
        stmt.bindInt(4, if (fav.sort_order != 0) fav.sort_order else next_order);
        stmt.bindInt64(5, std.time.timestamp());

        try stmt.exec();
        return stmt.lastInsertRowId();
    }

    pub fn removeFavorite(self: *Library, id: i64) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare("DELETE FROM favorites WHERE id = ?");
        defer stmt.finalize();
        stmt.bindInt64(1, id);
        try stmt.exec();
    }

    pub fn listFavorites(self: *Library) ![]types.Favorite {
        var stmt = try self.db.prepare(
            \\SELECT id, item_type, item_id, display_name, sort_order, created_at
            \\FROM favorites ORDER BY sort_order ASC
        );
        defer stmt.finalize();

        var results: std.ArrayList(types.Favorite) = .{};
        while (stmt.step()) {
            try results.append(self.allocator, types.Favorite{
                .id = stmt.columnInt64(0),
                .item_type = types.FavoriteType.fromString(stmt.columnText(1) orelse "media_item") orelse .media_item,
                .item_id = try dupeText(self.allocator, stmt.columnText(2) orelse ""),
                .display_name = try dupeText(self.allocator, stmt.columnText(3) orelse ""),
                .sort_order = stmt.columnInt(4),
                .created_at = stmt.columnOptionalInt64(5),
            });
        }
        return results.toOwnedSlice(self.allocator);
    }

    pub fn freeFavorites(self: *Library, favs: []types.Favorite) void {
        for (favs) |f| {
            self.allocator.free(f.item_id);
            self.allocator.free(f.display_name);
        }
        self.allocator.free(favs);
    }

    // Helpers

    fn collectMediaItems(self: *Library, stmt: *database.Statement) ![]types.MediaItem {
        var results: std.ArrayList(types.MediaItem) = .{};
        while (stmt.step()) {
            try results.append(self.allocator, try readMediaItem(self.allocator, stmt));
        }
        return results.toOwnedSlice(self.allocator);
    }

    pub fn freeMediaItems(self: *Library, items: []types.MediaItem) void {
        for (items) |item| self.freeMediaItem(item);
        self.allocator.free(items);
    }

    // Server operations
    pub fn upsertServer(self: *Library, server: types.Server) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare(
            \\INSERT OR REPLACE INTO servers
            \\  (id, name, client_identifier, auth_token, connection_uri, last_connected_at)
            \\VALUES (?, ?, ?, ?, ?, ?)
        );
        defer stmt.finalize();

        stmt.bindText(1, server.id);
        stmt.bindText(2, server.name);
        stmt.bindText(3, server.client_identifier);
        stmt.bindOptionalText(4, server.auth_token);
        stmt.bindOptionalText(5, server.connection_uri);
        stmt.bindOptionalInt64(6, server.last_connected_at);

        try stmt.exec();
    }

    pub fn getServer(self: *Library, id: []const u8) !?types.Server {
        var stmt = try self.db.prepare(
            \\SELECT id, name, client_identifier, auth_token, connection_uri, last_connected_at
            \\FROM servers WHERE id = ?
        );
        defer stmt.finalize();
        stmt.bindText(1, id);

        if (stmt.step()) {
            return types.Server{
                .id = try dupeText(self.allocator, stmt.columnText(0) orelse return null),
                .name = try dupeText(self.allocator, stmt.columnText(1) orelse return null),
                .client_identifier = try dupeText(self.allocator, stmt.columnText(2) orelse return null),
                .auth_token = try dupeOptionalText(self.allocator, stmt.columnText(3)),
                .connection_uri = try dupeOptionalText(self.allocator, stmt.columnText(4)),
                .last_connected_at = stmt.columnOptionalInt64(5),
            };
        }
        return null;
    }

    pub fn freeServer(self: *Library, server: types.Server) void {
        self.allocator.free(server.id);
        self.allocator.free(server.name);
        self.allocator.free(server.client_identifier);
        if (server.auth_token) |t| self.allocator.free(t);
        if (server.connection_uri) |u| self.allocator.free(u);
    }

    pub fn freeMediaItem(self: *Library, item: types.MediaItem) void {
        self.allocator.free(item.title);
        if (item.source_id) |s| self.allocator.free(s);
        if (item.server_id) |s| self.allocator.free(s);
        if (item.sort_title) |s| self.allocator.free(s);
        if (item.summary) |s| self.allocator.free(s);
        if (item.poster_path) |s| self.allocator.free(s);
        if (item.backdrop_path) |s| self.allocator.free(s);
        if (item.file_path) |s| self.allocator.free(s);
    }

    fn readMediaItem(allocator: std.mem.Allocator, stmt: *database.Statement) !types.MediaItem {
        return types.MediaItem{
            .id = stmt.columnInt64(0),
            .source = types.MediaSource.fromString(stmt.columnText(1) orelse "local") orelse .local,
            .source_id = try dupeOptionalText(allocator, stmt.columnText(2)),
            .server_id = try dupeOptionalText(allocator, stmt.columnText(3)),
            .media_type = types.MediaType.fromString(stmt.columnText(4) orelse "movie") orelse .movie,
            .title = try dupeText(allocator, stmt.columnText(5) orelse ""),
            .sort_title = try dupeOptionalText(allocator, stmt.columnText(6)),
            .year = stmt.columnOptionalInt(7),
            .summary = try dupeOptionalText(allocator, stmt.columnText(8)),
            .rating = stmt.columnOptionalDouble(9),
            .duration_ms = stmt.columnOptionalInt64(10),
            .poster_path = try dupeOptionalText(allocator, stmt.columnText(11)),
            .backdrop_path = try dupeOptionalText(allocator, stmt.columnText(12)),
            .tmdb_id = stmt.columnOptionalInt(13),
            .parent_id = stmt.columnOptionalInt64(14),
            .season_number = stmt.columnOptionalInt(15),
            .episode_number = stmt.columnOptionalInt(16),
            .file_path = try dupeOptionalText(allocator, stmt.columnText(17)),
            .added_at = stmt.columnOptionalInt64(18),
            .updated_at = stmt.columnOptionalInt64(19),
        };
    }
};

fn dupeText(allocator: std.mem.Allocator, text: []const u8) ![]const u8 {
    return allocator.dupe(u8, text);
}

fn dupeOptionalText(allocator: std.mem.Allocator, text: ?[]const u8) !?[]const u8 {
    if (text) |t| return try allocator.dupe(u8, t);
    return null;
}

test "library insert and retrieve media item" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var lib = Library.init(std.testing.allocator, &db);

    const id = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .movie,
        .title = "Test Movie",
        .year = 2024,
        .file_path = "/movies/test.mkv",
    });

    const item = (try lib.getMediaItem(id)).?;
    defer lib.freeMediaItem(item);

    try std.testing.expectEqualStrings("Test Movie", item.title);
    try std.testing.expectEqual(@as(?i32, 2024), item.year);
    try std.testing.expectEqual(types.MediaSource.local, item.source);
}

test "library watch progress" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var lib = Library.init(std.testing.allocator, &db);

    const id = try lib.insertMediaItem(.{
        .source = .plex,
        .source_id = "12345",
        .media_type = .movie,
        .title = "Plex Movie",
    });

    try lib.updateWatchProgress(.{
        .media_item_id = id,
        .position_ms = 60000,
        .duration_ms = 7200000,
    });

    const progress = (try lib.getWatchProgress(id)).?;
    try std.testing.expectEqual(@as(i64, 60000), progress.position_ms);
    try std.testing.expectEqual(false, progress.watched);
}

test "library getItemsByType" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var lib = Library.init(std.testing.allocator, &db);

    _ = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .movie,
        .title = "Alpha Movie",
        .year = 2020,
        .added_at = 100,
    });
    _ = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .movie,
        .title = "Beta Movie",
        .year = 2022,
        .added_at = 200,
    });
    _ = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .show,
        .title = "Some Show",
    });

    const movies = try lib.getItemsByType(.movie, .title, .asc, 50, 0);
    defer lib.freeMediaItems(movies);

    try std.testing.expectEqual(@as(usize, 2), movies.len);
    try std.testing.expectEqualStrings("Alpha Movie", movies[0].title);
    try std.testing.expectEqualStrings("Beta Movie", movies[1].title);
}

test "library getRecentlyAdded" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var lib = Library.init(std.testing.allocator, &db);

    _ = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .movie,
        .title = "Old Movie",
        .added_at = 100,
    });
    _ = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .movie,
        .title = "New Movie",
        .added_at = 200,
    });

    const recent = try lib.getRecentlyAdded(10);
    defer lib.freeMediaItems(recent);

    try std.testing.expectEqual(@as(usize, 2), recent.len);
    try std.testing.expectEqualStrings("New Movie", recent[0].title);
}

test "library getContinueWatching" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var lib = Library.init(std.testing.allocator, &db);

    const id1 = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .movie,
        .title = "Partially Watched",
    });
    const id2 = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .movie,
        .title = "Unwatched",
    });
    _ = id2;

    try lib.updateWatchProgress(.{
        .media_item_id = id1,
        .position_ms = 30000,
        .duration_ms = 120000,
        .watched = false,
        .last_watched_at = 500,
    });

    const watching = try lib.getContinueWatching(10);
    defer lib.freeMediaItems(watching);

    try std.testing.expectEqual(@as(usize, 1), watching.len);
    try std.testing.expectEqualStrings("Partially Watched", watching[0].title);
}

test "library searchItems" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var lib = Library.init(std.testing.allocator, &db);

    _ = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .movie,
        .title = "The Matrix",
    });
    _ = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .movie,
        .title = "Interstellar",
    });

    const results = try lib.searchItems("Matrix", null);
    defer lib.freeMediaItems(results);

    try std.testing.expectEqual(@as(usize, 1), results.len);
    try std.testing.expectEqualStrings("The Matrix", results[0].title);
}

test "library getItemsByParent" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var lib = Library.init(std.testing.allocator, &db);

    const show_id = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .show,
        .title = "Breaking Bad",
    });
    _ = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .season,
        .title = "Season 1",
        .parent_id = show_id,
        .season_number = 1,
    });
    _ = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .season,
        .title = "Season 2",
        .parent_id = show_id,
        .season_number = 2,
    });

    const seasons = try lib.getItemsByParent(show_id);
    defer lib.freeMediaItems(seasons);

    try std.testing.expectEqual(@as(usize, 2), seasons.len);
    try std.testing.expectEqualStrings("Season 1", seasons[0].title);
}

test "library getItemCount" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var lib = Library.init(std.testing.allocator, &db);

    _ = try lib.insertMediaItem(.{ .source = .local, .media_type = .movie, .title = "M1" });
    _ = try lib.insertMediaItem(.{ .source = .local, .media_type = .movie, .title = "M2" });
    _ = try lib.insertMediaItem(.{ .source = .local, .media_type = .show, .title = "S1" });

    try std.testing.expectEqual(@as(u32, 2), try lib.getItemCount(.movie));
    try std.testing.expectEqual(@as(u32, 1), try lib.getItemCount(.show));
    try std.testing.expectEqual(@as(u32, 0), try lib.getItemCount(.other));
}

test "library favorites CRUD" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var lib = Library.init(std.testing.allocator, &db);

    const fav_id = try lib.addFavorite(.{
        .item_type = .media_item,
        .item_id = "42",
        .display_name = "Favorite Movie",
    });
    try std.testing.expect(fav_id > 0);

    _ = try lib.addFavorite(.{
        .item_type = .scan_path,
        .item_id = "/movies",
        .display_name = "Movies Folder",
    });

    const favs = try lib.listFavorites();
    defer lib.freeFavorites(favs);

    try std.testing.expectEqual(@as(usize, 2), favs.len);
    try std.testing.expectEqualStrings("Favorite Movie", favs[0].display_name);

    try lib.removeFavorite(fav_id);

    const after = try lib.listFavorites();
    defer lib.freeFavorites(after);
    try std.testing.expectEqual(@as(usize, 1), after.len);
}

test "library listServers" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var lib = Library.init(std.testing.allocator, &db);

    try lib.upsertServer(.{ .id = "s1", .name = "Server A", .client_identifier = "uuid1" });
    try lib.upsertServer(.{ .id = "s2", .name = "Server B", .client_identifier = "uuid2" });

    const servers = try lib.listServers();
    defer lib.freeServers(servers);

    try std.testing.expectEqual(@as(usize, 2), servers.len);
}

test "library scan paths" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var lib = Library.init(std.testing.allocator, &db);

    const id = try lib.insertScanPath("/home/user/movies");
    try std.testing.expect(id > 0);

    const paths = try lib.listScanPaths();
    defer lib.freeScanPaths(paths);

    try std.testing.expectEqual(@as(usize, 1), paths.len);
    try std.testing.expectEqualStrings("/home/user/movies", paths[0].path);

    try lib.deleteScanPath(paths[0].id);

    const after = try lib.listScanPaths();
    defer lib.freeScanPaths(after);
    try std.testing.expectEqual(@as(usize, 0), after.len);
}

test "library server upsert" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var lib = Library.init(std.testing.allocator, &db);

    try lib.upsertServer(.{
        .id = "server1",
        .name = "My Plex",
        .client_identifier = "uuid-1234",
        .auth_token = "token-abc",
        .connection_uri = "http://192.168.1.10:32400",
    });

    const server = (try lib.getServer("server1")).?;
    defer lib.freeServer(server);

    try std.testing.expectEqualStrings("My Plex", server.name);
    try std.testing.expectEqualStrings("token-abc", server.auth_token.?);
}
