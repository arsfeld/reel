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
