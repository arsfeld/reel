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
            \\       season_number, episode_number, file_path, added_at, updated_at, match_locked
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
            \\       season_number, episode_number, file_path, added_at, updated_at, match_locked
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
            \\       season_number, episode_number, file_path, added_at, updated_at, match_locked
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
            \\       season_number, episode_number, file_path, added_at, updated_at, match_locked
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
            \\       m.season_number, m.episode_number, m.file_path, m.added_at, m.updated_at, m.match_locked
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
                \\       season_number, episode_number, file_path, added_at, updated_at, match_locked
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
                \\       season_number, episode_number, file_path, added_at, updated_at, match_locked
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
            \\       season_number, episode_number, file_path, added_at, updated_at, match_locked
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

    // Genre operations

    pub fn insertGenre(self: *Library, name: []const u8) !i64 {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();
        return self.insertGenreLocked(name);
    }

    /// Insert a genre without locking (caller must hold mutex).
    fn insertGenreLocked(self: *Library, name: []const u8) !i64 {
        var stmt = try self.db.prepare("INSERT OR IGNORE INTO genres (name) VALUES (?)");
        defer stmt.finalize();
        stmt.bindText(1, name);
        try stmt.exec();

        // Always look up the id — lastInsertRowId is unreliable with INSERT OR IGNORE
        var lookup = try self.db.prepare("SELECT id FROM genres WHERE name = ?");
        defer lookup.finalize();
        lookup.bindText(1, name);
        if (lookup.step()) {
            return lookup.columnInt64(0);
        }
        return error.SqlExecFailed;
    }

    pub fn setMediaItemGenres(self: *Library, media_item_id: i64, genre_names: []const []const u8) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        // Delete existing associations
        var del = try self.db.prepare("DELETE FROM media_item_genres WHERE media_item_id = ?");
        defer del.finalize();
        del.bindInt64(1, media_item_id);
        try del.exec();

        // Insert new associations
        for (genre_names) |name| {
            const genre_id = try self.insertGenreLocked(name);

            var ins = try self.db.prepare(
                "INSERT OR IGNORE INTO media_item_genres (media_item_id, genre_id) VALUES (?, ?)",
            );
            defer ins.finalize();
            ins.bindInt64(1, media_item_id);
            ins.bindInt64(2, genre_id);
            try ins.exec();
        }
    }

    pub fn getDistinctGenres(self: *Library) ![]types.Genre {
        var stmt = try self.db.prepare(
            \\SELECT g.id, g.name FROM genres g
            \\INNER JOIN media_item_genres mig ON g.id = mig.genre_id
            \\GROUP BY g.id, g.name
            \\ORDER BY COUNT(mig.media_item_id) DESC
        );
        defer stmt.finalize();

        var results: std.ArrayList(types.Genre) = .{};
        while (stmt.step()) {
            try results.append(self.allocator, types.Genre{
                .id = stmt.columnInt64(0),
                .name = try dupeText(self.allocator, stmt.columnText(1) orelse continue),
            });
        }
        return results.toOwnedSlice(self.allocator);
    }

    pub fn freeGenres(self: *Library, genres: []types.Genre) void {
        for (genres) |g| self.allocator.free(g.name);
        self.allocator.free(genres);
    }

    pub fn getItemsByGenre(self: *Library, genre_name: []const u8, limit: u32) ![]types.MediaItem {
        var stmt = try self.db.prepare(
            \\SELECT m.id, m.source, m.source_id, m.server_id, m.media_type, m.title,
            \\       m.sort_title, m.year, m.summary, m.rating, m.duration_ms,
            \\       m.poster_path, m.backdrop_path, m.tmdb_id, m.parent_id,
            \\       m.season_number, m.episode_number, m.file_path, m.added_at, m.updated_at, m.match_locked
            \\FROM media_items m
            \\INNER JOIN media_item_genres mig ON m.id = mig.media_item_id
            \\INNER JOIN genres g ON mig.genre_id = g.id
            \\WHERE g.name = ? AND m.media_type IN ('movie', 'show')
            \\ORDER BY m.added_at DESC LIMIT ?
        );
        defer stmt.finalize();
        stmt.bindText(1, genre_name);
        stmt.bindInt(2, @intCast(limit));

        return self.collectMediaItems(&stmt);
    }

    // Match lock operations

    pub fn setMatchLocked(self: *Library, media_item_id: i64, locked: bool) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare("UPDATE media_items SET match_locked = ? WHERE id = ?");
        defer stmt.finalize();
        stmt.bindInt(1, if (locked) 1 else 0);
        stmt.bindInt64(2, media_item_id);
        try stmt.exec();
    }

    pub fn updateMediaItemMetadata(self: *Library, id: i64, item: types.MediaItem) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare(
            \\UPDATE media_items SET
            \\  title = ?, sort_title = ?, year = ?, summary = ?, rating = ?,
            \\  duration_ms = ?, poster_path = ?, backdrop_path = ?, tmdb_id = ?,
            \\  match_locked = ?, updated_at = ?
            \\WHERE id = ?
        );
        defer stmt.finalize();

        stmt.bindText(1, item.title);
        stmt.bindOptionalText(2, item.sort_title);
        stmt.bindOptionalInt(3, item.year);
        stmt.bindOptionalText(4, item.summary);
        stmt.bindOptionalDouble(5, item.rating);
        stmt.bindOptionalInt64(6, item.duration_ms);
        stmt.bindOptionalText(7, item.poster_path);
        stmt.bindOptionalText(8, item.backdrop_path);
        stmt.bindOptionalInt(9, item.tmdb_id);
        stmt.bindInt(10, if (item.match_locked) 1 else 0);
        stmt.bindInt64(11, std.time.timestamp());
        stmt.bindInt64(12, id);

        try stmt.exec();
    }

    // Collection operations

    pub fn createCollection(self: *Library, name: []const u8, collection_type: types.CollectionType, description: ?[]const u8) !i64 {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        // Get max sort_order
        var max_stmt = try self.db.prepare("SELECT COALESCE(MAX(sort_order), -1) FROM collections");
        defer max_stmt.finalize();
        var next_order: i32 = 0;
        if (max_stmt.step()) {
            next_order = max_stmt.columnInt(0) + 1;
        }

        var stmt = try self.db.prepare(
            \\INSERT INTO collections (name, collection_type, description, sort_order, created_at, updated_at)
            \\VALUES (?, ?, ?, ?, ?, ?)
        );
        defer stmt.finalize();
        stmt.bindText(1, name);
        stmt.bindText(2, collection_type.toString());
        stmt.bindOptionalText(3, description);
        stmt.bindInt(4, next_order);
        const now = std.time.timestamp();
        stmt.bindInt64(5, now);
        stmt.bindInt64(6, now);

        try stmt.exec();
        return stmt.lastInsertRowId();
    }

    pub fn getCollection(self: *Library, id: i64) !?types.Collection {
        var stmt = try self.db.prepare(
            \\SELECT id, name, collection_type, description, poster_path,
            \\       show_on_home, sort_order, created_at, updated_at
            \\FROM collections WHERE id = ?
        );
        defer stmt.finalize();
        stmt.bindInt64(1, id);

        if (stmt.step()) {
            return try readCollection(self.allocator, &stmt);
        }
        return null;
    }

    pub fn listCollections(self: *Library) ![]types.Collection {
        var stmt = try self.db.prepare(
            \\SELECT id, name, collection_type, description, poster_path,
            \\       show_on_home, sort_order, created_at, updated_at
            \\FROM collections ORDER BY sort_order ASC, name ASC
        );
        defer stmt.finalize();

        var results: std.ArrayList(types.Collection) = .{};
        while (stmt.step()) {
            try results.append(self.allocator, try readCollection(self.allocator, &stmt));
        }
        return results.toOwnedSlice(self.allocator);
    }

    pub fn deleteCollection(self: *Library, id: i64) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare("DELETE FROM collections WHERE id = ?");
        defer stmt.finalize();
        stmt.bindInt64(1, id);
        try stmt.exec();
    }

    pub fn freeCollection(self: *Library, col: types.Collection) void {
        self.allocator.free(col.name);
        if (col.description) |s| self.allocator.free(s);
        if (col.poster_path) |s| self.allocator.free(s);
    }

    pub fn freeCollections(self: *Library, cols: []types.Collection) void {
        for (cols) |col| self.freeCollection(col);
        self.allocator.free(cols);
    }

    // Collection items

    pub fn addToCollection(self: *Library, collection_id: i64, media_item_id: i64) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare(
            \\INSERT OR IGNORE INTO collection_items (collection_id, media_item_id, added_at)
            \\VALUES (?, ?, ?)
        );
        defer stmt.finalize();
        stmt.bindInt64(1, collection_id);
        stmt.bindInt64(2, media_item_id);
        stmt.bindInt64(3, std.time.timestamp());
        try stmt.exec();
    }

    pub fn removeFromCollection(self: *Library, collection_id: i64, media_item_id: i64) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare(
            "DELETE FROM collection_items WHERE collection_id = ? AND media_item_id = ?",
        );
        defer stmt.finalize();
        stmt.bindInt64(1, collection_id);
        stmt.bindInt64(2, media_item_id);
        try stmt.exec();
    }

    pub fn getCollectionItems(self: *Library, collection_id: i64) ![]types.MediaItem {
        var stmt = try self.db.prepare(
            \\SELECT m.id, m.source, m.source_id, m.server_id, m.media_type, m.title,
            \\       m.sort_title, m.year, m.summary, m.rating, m.duration_ms,
            \\       m.poster_path, m.backdrop_path, m.tmdb_id, m.parent_id,
            \\       m.season_number, m.episode_number, m.file_path, m.added_at, m.updated_at, m.match_locked
            \\FROM media_items m
            \\INNER JOIN collection_items ci ON m.id = ci.media_item_id
            \\WHERE ci.collection_id = ?
            \\ORDER BY ci.sort_order ASC, ci.added_at DESC
        );
        defer stmt.finalize();
        stmt.bindInt64(1, collection_id);

        return self.collectMediaItems(&stmt);
    }

    // Collection rules

    pub fn addCollectionRule(self: *Library, collection_id: i64, field: []const u8, operator: []const u8, value: []const u8) !i64 {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare(
            \\INSERT INTO collection_rules (collection_id, field, operator, value)
            \\VALUES (?, ?, ?, ?)
        );
        defer stmt.finalize();
        stmt.bindInt64(1, collection_id);
        stmt.bindText(2, field);
        stmt.bindText(3, operator);
        stmt.bindText(4, value);
        try stmt.exec();
        return stmt.lastInsertRowId();
    }

    pub fn getCollectionRules(self: *Library, collection_id: i64) ![]types.CollectionRule {
        var stmt = try self.db.prepare(
            \\SELECT id, collection_id, field, operator, value
            \\FROM collection_rules WHERE collection_id = ?
        );
        defer stmt.finalize();
        stmt.bindInt64(1, collection_id);

        var results: std.ArrayList(types.CollectionRule) = .{};
        while (stmt.step()) {
            try results.append(self.allocator, types.CollectionRule{
                .id = stmt.columnInt64(0),
                .collection_id = stmt.columnInt64(1),
                .field = try dupeText(self.allocator, stmt.columnText(2) orelse continue),
                .operator = try dupeText(self.allocator, stmt.columnText(3) orelse continue),
                .value = try dupeText(self.allocator, stmt.columnText(4) orelse continue),
            });
        }
        return results.toOwnedSlice(self.allocator);
    }

    pub fn removeCollectionRule(self: *Library, rule_id: i64) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare("DELETE FROM collection_rules WHERE id = ?");
        defer stmt.finalize();
        stmt.bindInt64(1, rule_id);
        try stmt.exec();
    }

    pub fn freeCollectionRules(self: *Library, rules: []types.CollectionRule) void {
        for (rules) |r| {
            self.allocator.free(r.field);
            self.allocator.free(r.operator);
            self.allocator.free(r.value);
        }
        self.allocator.free(rules);
    }

    /// Evaluate a smart collection's rules and return matching media items.
    pub fn evaluateSmartCollection(self: *Library, collection_id: i64) ![]types.MediaItem {
        const rules = try self.getCollectionRules(collection_id);
        defer self.freeCollectionRules(rules);

        if (rules.len == 0) return self.allocator.alloc(types.MediaItem, 0);

        // Build dynamic WHERE clause
        var sql_buf: [2048]u8 = undefined;
        var fba = std.heap.FixedBufferAllocator.init(&sql_buf);
        const fba_alloc = fba.allocator();

        var where_parts: std.ArrayList([]const u8) = .{};
        var needs_genre_join = false;
        var needs_watch_join = false;

        for (rules) |rule| {
            if (std.mem.eql(u8, rule.field, "genre")) {
                needs_genre_join = true;
                try where_parts.append(fba_alloc, "g.name = ?");
            } else if (std.mem.eql(u8, rule.field, "year")) {
                const op_str = sqlOperator(rule.operator);
                const part = try std.fmt.allocPrint(fba_alloc, "m.year {s} ?", .{op_str});
                try where_parts.append(fba_alloc, part);
            } else if (std.mem.eql(u8, rule.field, "media_type")) {
                try where_parts.append(fba_alloc, "m.media_type = ?");
            } else if (std.mem.eql(u8, rule.field, "source")) {
                try where_parts.append(fba_alloc, "m.source = ?");
            } else if (std.mem.eql(u8, rule.field, "watched")) {
                needs_watch_join = true;
                try where_parts.append(fba_alloc, "COALESCE(wp.watched, 0) = ?");
            }
        }

        if (where_parts.items.len == 0) return self.allocator.alloc(types.MediaItem, 0);

        // Build full SQL
        var full_sql: std.ArrayList(u8) = .{};
        try full_sql.appendSlice(self.allocator,
            \\SELECT m.id, m.source, m.source_id, m.server_id, m.media_type, m.title,
            \\       m.sort_title, m.year, m.summary, m.rating, m.duration_ms,
            \\       m.poster_path, m.backdrop_path, m.tmdb_id, m.parent_id,
            \\       m.season_number, m.episode_number, m.file_path, m.added_at, m.updated_at, m.match_locked
            \\FROM media_items m
        );

        if (needs_genre_join) {
            try full_sql.appendSlice(self.allocator,
                \\ INNER JOIN media_item_genres mig ON m.id = mig.media_item_id
                \\ INNER JOIN genres g ON mig.genre_id = g.id
            );
        }
        if (needs_watch_join) {
            try full_sql.appendSlice(self.allocator,
                \\ LEFT JOIN watch_progress wp ON m.id = wp.media_item_id
            );
        }

        try full_sql.appendSlice(self.allocator, " WHERE ");

        for (where_parts.items, 0..) |part, i| {
            if (i > 0) try full_sql.appendSlice(self.allocator, " AND ");
            try full_sql.appendSlice(self.allocator, part);
        }

        try full_sql.appendSlice(self.allocator, " ORDER BY m.added_at DESC LIMIT 100");
        try full_sql.append(self.allocator, 0); // null terminator

        const sql_z: [*:0]const u8 = @ptrCast(full_sql.items.ptr);
        defer full_sql.deinit(self.allocator);

        var stmt = try self.db.prepare(sql_z);
        defer stmt.finalize();

        // Bind rule values
        var bind_idx: c_int = 1;
        for (rules) |rule| {
            if (std.mem.eql(u8, rule.field, "genre") or
                std.mem.eql(u8, rule.field, "media_type") or
                std.mem.eql(u8, rule.field, "source"))
            {
                stmt.bindText(bind_idx, rule.value);
                bind_idx += 1;
            } else if (std.mem.eql(u8, rule.field, "year") or
                std.mem.eql(u8, rule.field, "watched"))
            {
                const int_val = std.fmt.parseInt(i32, rule.value, 10) catch 0;
                stmt.bindInt(bind_idx, int_val);
                bind_idx += 1;
            }
        }

        return self.collectMediaItems(&stmt);
    }

    fn readCollection(allocator: std.mem.Allocator, stmt: *database.Statement) !types.Collection {
        return types.Collection{
            .id = stmt.columnInt64(0),
            .name = try dupeText(allocator, stmt.columnText(1) orelse ""),
            .collection_type = types.CollectionType.fromString(stmt.columnText(2) orelse "manual") orelse .manual,
            .description = try dupeOptionalText(allocator, stmt.columnText(3)),
            .poster_path = try dupeOptionalText(allocator, stmt.columnText(4)),
            .show_on_home = stmt.columnBool(5),
            .sort_order = stmt.columnInt(6),
            .created_at = stmt.columnOptionalInt64(7),
            .updated_at = stmt.columnOptionalInt64(8),
        };
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
            .match_locked = stmt.columnBool(20),
        };
    }
};

fn sqlOperator(op: []const u8) []const u8 {
    if (std.mem.eql(u8, op, "eq")) return "=";
    if (std.mem.eql(u8, op, "neq")) return "!=";
    if (std.mem.eql(u8, op, "gt")) return ">";
    if (std.mem.eql(u8, op, "gte")) return ">=";
    if (std.mem.eql(u8, op, "lt")) return "<";
    if (std.mem.eql(u8, op, "lte")) return "<=";
    return "=";
}

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

test "library genre insert and query" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var lib = Library.init(std.testing.allocator, &db);

    const id1 = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .movie,
        .title = "Sci-Fi Movie",
        .year = 2024,
    });
    const id2 = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .movie,
        .title = "Action Movie",
        .year = 2023,
    });

    try lib.setMediaItemGenres(id1, &.{ "Science Fiction", "Action" });
    try lib.setMediaItemGenres(id2, &.{"Action"});

    // Get distinct genres — Action should come first (2 items) then Sci-Fi (1 item)
    const genres = try lib.getDistinctGenres();
    defer lib.freeGenres(genres);

    try std.testing.expectEqual(@as(usize, 2), genres.len);
    try std.testing.expectEqualStrings("Action", genres[0].name);
    try std.testing.expectEqualStrings("Science Fiction", genres[1].name);

    // Get items by genre
    const action_items = try lib.getItemsByGenre("Action", 20);
    defer lib.freeMediaItems(action_items);
    try std.testing.expectEqual(@as(usize, 2), action_items.len);

    const scifi_items = try lib.getItemsByGenre("Science Fiction", 20);
    defer lib.freeMediaItems(scifi_items);
    try std.testing.expectEqual(@as(usize, 1), scifi_items.len);
}

test "library collection CRUD" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var lib = Library.init(std.testing.allocator, &db);

    const col_id = try lib.createCollection("My Collection", .manual, "Test collection");
    try std.testing.expect(col_id > 0);

    const col = (try lib.getCollection(col_id)).?;
    defer lib.freeCollection(col);
    try std.testing.expectEqualStrings("My Collection", col.name);
    try std.testing.expectEqual(types.CollectionType.manual, col.collection_type);
    try std.testing.expectEqualStrings("Test collection", col.description.?);
    try std.testing.expectEqual(true, col.show_on_home);

    _ = try lib.createCollection("Second Collection", .smart, null);

    const all = try lib.listCollections();
    defer lib.freeCollections(all);
    try std.testing.expectEqual(@as(usize, 2), all.len);

    try lib.deleteCollection(col_id);

    const after = try lib.listCollections();
    defer lib.freeCollections(after);
    try std.testing.expectEqual(@as(usize, 1), after.len);
}

test "library collection items" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var lib = Library.init(std.testing.allocator, &db);

    const col_id = try lib.createCollection("Favorites 2", .manual, null);
    const item_id1 = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .movie,
        .title = "Movie A",
    });
    const item_id2 = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .movie,
        .title = "Movie B",
    });

    try lib.addToCollection(col_id, item_id1);
    try lib.addToCollection(col_id, item_id2);

    const items = try lib.getCollectionItems(col_id);
    defer lib.freeMediaItems(items);
    try std.testing.expectEqual(@as(usize, 2), items.len);

    try lib.removeFromCollection(col_id, item_id1);

    const after = try lib.getCollectionItems(col_id);
    defer lib.freeMediaItems(after);
    try std.testing.expectEqual(@as(usize, 1), after.len);
    try std.testing.expectEqualStrings("Movie B", after[0].title);
}

test "library smart collection evaluation" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var lib = Library.init(std.testing.allocator, &db);

    // Create movies with genres
    const id1 = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .movie,
        .title = "Blade Runner 2049",
        .year = 2017,
    });
    const id2 = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .movie,
        .title = "The Matrix",
        .year = 1999,
    });
    const id3 = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .show,
        .title = "Westworld",
        .year = 2016,
    });

    try lib.setMediaItemGenres(id1, &.{ "Science Fiction", "Drama" });
    try lib.setMediaItemGenres(id2, &.{ "Science Fiction", "Action" });
    try lib.setMediaItemGenres(id3, &.{"Science Fiction"});

    // Smart collection: Sci-Fi movies
    const col_id = try lib.createCollection("Sci-Fi Movies", .smart, null);
    _ = try lib.addCollectionRule(col_id, "genre", "eq", "Science Fiction");
    _ = try lib.addCollectionRule(col_id, "media_type", "eq", "movie");

    const results = try lib.evaluateSmartCollection(col_id);
    defer lib.freeMediaItems(results);

    // Should only include the two movies, not the show
    try std.testing.expectEqual(@as(usize, 2), results.len);

    // Smart collection: year >= 2010
    const col2 = try lib.createCollection("2010s+", .smart, null);
    _ = try lib.addCollectionRule(col2, "year", "gte", "2010");

    const results2 = try lib.evaluateSmartCollection(col2);
    defer lib.freeMediaItems(results2);
    try std.testing.expectEqual(@as(usize, 2), results2.len); // Blade Runner 2049 + Westworld
}

test "library match locked flag" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var lib = Library.init(std.testing.allocator, &db);

    const id = try lib.insertMediaItem(.{
        .source = .local,
        .media_type = .movie,
        .title = "Test Movie",
        .tmdb_id = 12345,
    });

    // Default should be unlocked
    const item1 = (try lib.getMediaItem(id)).?;
    defer lib.freeMediaItem(item1);
    try std.testing.expectEqual(false, item1.match_locked);

    // Lock it
    try lib.setMatchLocked(id, true);

    const item2 = (try lib.getMediaItem(id)).?;
    defer lib.freeMediaItem(item2);
    try std.testing.expectEqual(true, item2.match_locked);

    // Unlock it
    try lib.setMatchLocked(id, false);

    const item3 = (try lib.getMediaItem(id)).?;
    defer lib.freeMediaItem(item3);
    try std.testing.expectEqual(false, item3.match_locked);
}
