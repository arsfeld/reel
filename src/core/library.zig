const std = @import("std");
const database = @import("database.zig");
const types = @import("types.zig");
const sqlite = database.sqlite;

pub const Library = struct {
    db: *database.Database,
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator, db: *database.Database) Library {
        return .{ .db = db, .allocator = allocator };
    }

    pub fn insertMediaItem(self: *Library, item: types.MediaItem) !i64 {
        try self.db.db.exec(
            \\INSERT INTO media_items (source, source_id, server_id, media_type,
            \\  title, sort_title, year, summary, rating, duration_ms,
            \\  poster_path, backdrop_path, tmdb_id, parent_id,
            \\  season_number, episode_number, file_path, library_section,
            \\  added_at, updated_at)
            \\VALUES (?{[]const u8}, ?{?[]const u8}, ?{?[]const u8}, ?{[]const u8},
            \\  ?{[]const u8}, ?{?[]const u8}, ?{?i32}, ?{?[]const u8}, ?{?f64}, ?{?i64},
            \\  ?{?[]const u8}, ?{?[]const u8}, ?{?i32}, ?{?i64},
            \\  ?{?i32}, ?{?i32}, ?{?[]const u8}, ?{?[]const u8},
            \\  ?{?i64}, ?{?i64})
        , .{}, .{
            item.source,       item.source_id,      item.server_id,      item.media_type,
            item.title,        item.sort_title,      item.year,           item.summary,
            item.rating,       item.duration_ms,     item.poster_path,    item.backdrop_path,
            item.tmdb_id,      item.parent_id,       item.season_number,  item.episode_number,
            item.file_path,    item.library_section, item.added_at,       item.updated_at,
        });
        return self.db.db.getLastInsertRowID();
    }

    pub fn getMediaItem(self: *Library, id: i64) !?types.MediaItem {
        return try self.db.db.oneAlloc(types.MediaItem, self.allocator,
            \\SELECT id, source, source_id, server_id, media_type, title,
            \\       sort_title, year, summary, rating, duration_ms,
            \\       poster_path, backdrop_path, tmdb_id, parent_id,
            \\       season_number, episode_number, file_path, library_section,
            \\       added_at, updated_at, match_locked
            \\FROM media_items WHERE id = ?{i64}
        , .{}, .{id});
    }

    pub fn getBySourceId(self: *Library, source: types.MediaSource, source_id: []const u8) !?types.MediaItem {
        return try self.db.db.oneAlloc(types.MediaItem, self.allocator,
            \\SELECT id, source, source_id, server_id, media_type, title,
            \\       sort_title, year, summary, rating, duration_ms,
            \\       poster_path, backdrop_path, tmdb_id, parent_id,
            \\       season_number, episode_number, file_path, library_section,
            \\       added_at, updated_at, match_locked
            \\FROM media_items WHERE source = ?{[]const u8} AND source_id = ?{[]const u8}
        , .{}, .{ source, source_id });
    }

    pub fn updateWatchProgress(self: *Library, progress: types.WatchProgress) !void {
        try self.db.db.exec(
            \\INSERT OR REPLACE INTO watch_progress
            \\  (media_item_id, position_ms, duration_ms, watched, last_watched_at)
            \\VALUES (?{i64}, ?{i64}, ?{?i64}, ?{bool}, ?{?i64})
        , .{}, .{
            progress.media_item_id,
            progress.position_ms,
            progress.duration_ms,
            progress.watched,
            progress.last_watched_at,
        });
    }

    pub fn getWatchProgress(self: *Library, media_item_id: i64) !?types.WatchProgress {
        return try self.db.db.one(types.WatchProgress,
            \\SELECT media_item_id, position_ms, duration_ms, watched, last_watched_at
            \\FROM watch_progress WHERE media_item_id = ?{i64}
        , .{}, .{media_item_id});
    }

    pub fn setLibrarySection(self: *Library, item_id: i64, section: []const u8) !void {
        try self.db.db.exec(
            "UPDATE media_items SET library_section = ?{[]const u8} WHERE id = ?{i64}",
            .{},
            .{ section, item_id },
        );
    }

    pub fn deleteByLibrarySection(self: *Library, server_id: []const u8, section: []const u8) !void {
        // Delete watch progress for affected items
        try self.db.db.exec(
            \\DELETE FROM watch_progress WHERE media_item_id IN
            \\  (SELECT id FROM media_items WHERE server_id = ?{[]const u8} AND library_section = ?{[]const u8})
        , .{}, .{ server_id, section });

        // Delete the items themselves
        try self.db.db.exec(
            "DELETE FROM media_items WHERE server_id = ?{[]const u8} AND library_section = ?{[]const u8}",
            .{},
            .{ server_id, section },
        );
    }

    pub fn deleteMediaItem(self: *Library, id: i64) !void {
        try self.db.db.exec(
            "DELETE FROM watch_progress WHERE media_item_id = ?{i64}",
            .{},
            .{id},
        );
        try self.db.db.exec(
            "DELETE FROM media_items WHERE id = ?{i64}",
            .{},
            .{id},
        );
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

        var sql_buf: [1024]u8 = undefined;
        const sql = std.fmt.bufPrint(&sql_buf,
            \\SELECT id, source, source_id, server_id, media_type, title,
            \\       sort_title, year, summary, rating, duration_ms,
            \\       poster_path, backdrop_path, tmdb_id, parent_id,
            \\       season_number, episode_number, file_path, library_section,
            \\       added_at, updated_at, match_locked
            \\FROM media_items WHERE media_type = ?
            \\ORDER BY {s} {s} LIMIT ? OFFSET ?
        , .{ order_clause, direction }) catch return error.SqlFormatFailed;

        var stmt = try self.db.db.prepareDynamic(sql);
        defer stmt.deinit();
        return try stmt.all(types.MediaItem, self.allocator, .{}, .{
            media_type.toString(),
            @as(i64, @intCast(limit)),
            @as(i64, @intCast(offset)),
        });
    }

    pub fn getRecentlyAdded(self: *Library, limit: u32) ![]types.MediaItem {
        var stmt = try self.db.db.prepare(
            \\SELECT id, source, source_id, server_id, media_type, title,
            \\       sort_title, year, summary, rating, duration_ms,
            \\       poster_path, backdrop_path, tmdb_id, parent_id,
            \\       season_number, episode_number, file_path, library_section,
            \\       added_at, updated_at, match_locked
            \\FROM media_items
            \\WHERE media_type IN ('movie', 'show')
            \\ORDER BY added_at DESC LIMIT ?{i32}
        );
        defer stmt.deinit();
        return try stmt.all(types.MediaItem, self.allocator, .{}, .{@as(i32, @intCast(limit))});
    }

    pub fn getContinueWatching(self: *Library, limit: u32) ![]types.MediaItem {
        var stmt = try self.db.db.prepare(
            \\SELECT m.id, m.source, m.source_id, m.server_id, m.media_type, m.title,
            \\       m.sort_title, m.year, m.summary, m.rating, m.duration_ms,
            \\       m.poster_path, m.backdrop_path, m.tmdb_id, m.parent_id,
            \\       m.season_number, m.episode_number, m.file_path, m.library_section,
            \\       m.added_at, m.updated_at, m.match_locked
            \\FROM media_items m
            \\JOIN watch_progress wp ON m.id = wp.media_item_id
            \\WHERE wp.watched = 0 AND wp.position_ms > 0
            \\ORDER BY wp.last_watched_at DESC LIMIT ?{i32}
        );
        defer stmt.deinit();
        return try stmt.all(types.MediaItem, self.allocator, .{}, .{@as(i32, @intCast(limit))});
    }

    pub fn searchItems(self: *Library, query: []const u8, media_type_filter: ?types.MediaType) ![]types.MediaItem {
        var pattern_buf: [256]u8 = undefined;
        const pattern = std.fmt.bufPrint(&pattern_buf, "%{s}%", .{query}) catch return error.SqlFormatFailed;

        if (media_type_filter) |mt| {
            var stmt = try self.db.db.prepare(
                \\SELECT id, source, source_id, server_id, media_type, title,
                \\       sort_title, year, summary, rating, duration_ms,
                \\       poster_path, backdrop_path, tmdb_id, parent_id,
                \\       season_number, episode_number, file_path, library_section,
                \\       added_at, updated_at, match_locked
                \\FROM media_items WHERE title LIKE ?{[]const u8} AND media_type = ?{[]const u8}
                \\ORDER BY title LIMIT 50
            );
            defer stmt.deinit();
            return try stmt.all(types.MediaItem, self.allocator, .{}, .{ pattern, mt });
        } else {
            var stmt = try self.db.db.prepare(
                \\SELECT id, source, source_id, server_id, media_type, title,
                \\       sort_title, year, summary, rating, duration_ms,
                \\       poster_path, backdrop_path, tmdb_id, parent_id,
                \\       season_number, episode_number, file_path, library_section,
                \\       added_at, updated_at, match_locked
                \\FROM media_items WHERE title LIKE ?{[]const u8}
                \\ORDER BY title LIMIT 50
            );
            defer stmt.deinit();
            return try stmt.all(types.MediaItem, self.allocator, .{}, .{pattern});
        }
    }

    pub fn getItemsByParent(self: *Library, parent_id: i64) ![]types.MediaItem {
        var stmt = try self.db.db.prepare(
            \\SELECT id, source, source_id, server_id, media_type, title,
            \\       sort_title, year, summary, rating, duration_ms,
            \\       poster_path, backdrop_path, tmdb_id, parent_id,
            \\       season_number, episode_number, file_path, library_section,
            \\       added_at, updated_at, match_locked
            \\FROM media_items WHERE parent_id = ?{i64}
            \\ORDER BY season_number, episode_number, title
        );
        defer stmt.deinit();
        return try stmt.all(types.MediaItem, self.allocator, .{}, .{parent_id});
    }

    /// Get a single media item by ID.
    pub fn getItemById(self: *Library, id: i64) !?types.MediaItem {
        return try self.db.db.oneAlloc(types.MediaItem, self.allocator,
            \\SELECT id, source, source_id, server_id, media_type, title,
            \\       sort_title, year, summary, rating, duration_ms,
            \\       poster_path, backdrop_path, tmdb_id, parent_id,
            \\       season_number, episode_number, file_path, library_section,
            \\       added_at, updated_at, match_locked
            \\FROM media_items WHERE id = ?{i64}
        , .{}, .{id});
    }

    /// Find the next episode after the given media item.
    /// Looks for the next episode in the same season, then the first episode of the next season.
    /// Returns null if the item is not an episode or no next episode exists.
    pub fn getNextEpisode(self: *Library, current_item_id: i64) !?types.MediaItem {
        // First, get the current item to find its parent (season), episode number, etc.
        const CurrentInfo = struct {
            parent_id: ?i64,
            season_number: ?i32,
            episode_number: ?i32,
            media_type: types.MediaType,
        };
        const info = try self.db.db.oneAlloc(CurrentInfo, self.allocator,
            \\SELECT parent_id, season_number, episode_number, media_type
            \\FROM media_items WHERE id = ?{i64}
        , .{}, .{current_item_id}) orelse return null;

        if (info.media_type != .episode) return null;
        const season_id = info.parent_id orelse return null;
        const current_episode = info.episode_number orelse return null;

        // Try next episode in the same season
        const next_in_season = try self.db.db.oneAlloc(types.MediaItem, self.allocator,
            \\SELECT id, source, source_id, server_id, media_type, title,
            \\       sort_title, year, summary, rating, duration_ms,
            \\       poster_path, backdrop_path, tmdb_id, parent_id,
            \\       season_number, episode_number, file_path, library_section,
            \\       added_at, updated_at, match_locked
            \\FROM media_items
            \\WHERE parent_id = ?{i64} AND episode_number > ?{i32}
            \\ORDER BY episode_number ASC LIMIT 1
        , .{}, .{ season_id, current_episode });

        if (next_in_season) |item| return item;

        // No more episodes in this season -- try the next season
        const current_season = info.season_number orelse return null;

        // Get the show ID (parent of the season)
        const ShowId = struct { parent_id: ?i64 };
        const show_row = try self.db.db.one(ShowId,
            "SELECT parent_id FROM media_items WHERE id = ?{i64}",
            .{},
            .{season_id},
        ) orelse return null;
        const show_id = show_row.parent_id orelse return null;

        // Find the next season
        const NextSeason = struct { id: i64 };
        const next_season = try self.db.db.one(NextSeason,
            "SELECT id FROM media_items WHERE parent_id = ?{i64} AND season_number > ?{i32} ORDER BY season_number ASC LIMIT 1",
            .{},
            .{ show_id, current_season },
        ) orelse return null;

        // Get first episode of that season
        return try self.db.db.oneAlloc(types.MediaItem, self.allocator,
            \\SELECT id, source, source_id, server_id, media_type, title,
            \\       sort_title, year, summary, rating, duration_ms,
            \\       poster_path, backdrop_path, tmdb_id, parent_id,
            \\       season_number, episode_number, file_path, library_section,
            \\       added_at, updated_at, match_locked
            \\FROM media_items
            \\WHERE parent_id = ?{i64} AND media_type = 'episode'
            \\ORDER BY episode_number ASC LIMIT 1
        , .{}, .{next_season.id});
    }

    pub fn getItemCount(self: *Library, media_type: types.MediaType) !u32 {
        const CountResult = struct { count: i64 };
        const result = try self.db.db.one(CountResult,
            "SELECT COUNT(*) FROM media_items WHERE media_type = ?{[]const u8}",
            .{},
            .{media_type},
        );
        if (result) |r| return @intCast(r.count);
        return 0;
    }

    pub fn listServers(self: *Library) ![]types.Server {
        var stmt = try self.db.db.prepare(
            \\SELECT id, name, client_identifier, auth_token, connection_uri, last_connected_at
            \\FROM servers ORDER BY name
        );
        defer stmt.deinit();
        return try stmt.all(types.Server, self.allocator, .{}, .{});
    }

    pub fn freeServers(self: *Library, servers: []types.Server) void {
        for (servers) |s| self.freeServer(s);
        self.allocator.free(servers);
    }

    pub fn listScanPaths(self: *Library) ![]types.ScanPath {
        var stmt = try self.db.db.prepare(
            "SELECT id, path, last_scanned_at FROM scan_paths ORDER BY path",
        );
        defer stmt.deinit();
        return try stmt.all(types.ScanPath, self.allocator, .{}, .{});
    }

    pub fn freeScanPaths(self: *Library, paths: []types.ScanPath) void {
        for (paths) |p| self.allocator.free(p.path);
        self.allocator.free(paths);
    }

    pub fn insertScanPath(self: *Library, path: []const u8) !i64 {
        try self.db.db.exec(
            "INSERT OR IGNORE INTO scan_paths (path) VALUES (?{[]const u8})",
            .{},
            .{path},
        );
        return self.db.db.getLastInsertRowID();
    }

    pub fn deleteScanPath(self: *Library, id: i64) !void {
        try self.db.db.exec(
            "DELETE FROM scan_paths WHERE id = ?{i64}",
            .{},
            .{id},
        );
    }

    // Favorites operations

    pub fn addFavorite(self: *Library, fav: types.Favorite) !i64 {
        // Get max sort_order
        const MaxResult = struct { max_order: i32 };
        const max_row = try self.db.db.one(MaxResult,
            "SELECT COALESCE(MAX(sort_order), -1) FROM favorites",
            .{},
            .{},
        );
        const next_order: i32 = if (max_row) |r| r.max_order + 1 else 0;

        try self.db.db.exec(
            \\INSERT INTO favorites (item_type, item_id, display_name, sort_order, created_at)
            \\VALUES (?{[]const u8}, ?{[]const u8}, ?{[]const u8}, ?{i32}, ?{i64})
        , .{}, .{
            fav.item_type,
            fav.item_id,
            fav.display_name,
            if (fav.sort_order != 0) fav.sort_order else next_order,
            unixTimestamp(),
        });
        return self.db.db.getLastInsertRowID();
    }

    pub fn removeFavorite(self: *Library, id: i64) !void {
        try self.db.db.exec(
            "DELETE FROM favorites WHERE id = ?{i64}",
            .{},
            .{id},
        );
    }

    pub fn listFavorites(self: *Library) ![]types.Favorite {
        var stmt = try self.db.db.prepare(
            \\SELECT id, item_type, item_id, display_name, sort_order, created_at
            \\FROM favorites ORDER BY sort_order ASC
        );
        defer stmt.deinit();
        return try stmt.all(types.Favorite, self.allocator, .{}, .{});
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
        return self.insertGenreInner(name);
    }

    /// Insert a genre (or find existing).
    fn insertGenreInner(self: *Library, name: []const u8) !i64 {
        try self.db.db.exec(
            "INSERT OR IGNORE INTO genres (name) VALUES (?{[]const u8})",
            .{},
            .{name},
        );

        // Always look up the id -- lastInsertRowId is unreliable with INSERT OR IGNORE
        const IdResult = struct { id: i64 };
        const row = try self.db.db.one(IdResult,
            "SELECT id FROM genres WHERE name = ?{[]const u8}",
            .{},
            .{name},
        ) orelse return error.SqlExecFailed;
        return row.id;
    }

    pub fn setMediaItemGenres(self: *Library, media_item_id: i64, genre_names: []const []const u8) !void {
        // Delete existing associations
        try self.db.db.exec(
            "DELETE FROM media_item_genres WHERE media_item_id = ?{i64}",
            .{},
            .{media_item_id},
        );

        // Insert new associations
        for (genre_names) |name| {
            const genre_id = try self.insertGenreInner(name);
            try self.db.db.exec(
                "INSERT OR IGNORE INTO media_item_genres (media_item_id, genre_id) VALUES (?{i64}, ?{i64})",
                .{},
                .{ media_item_id, genre_id },
            );
        }
    }

    pub fn getDistinctGenres(self: *Library) ![]types.Genre {
        var stmt = try self.db.db.prepare(
            \\SELECT g.id, g.name FROM genres g
            \\INNER JOIN media_item_genres mig ON g.id = mig.genre_id
            \\GROUP BY g.id, g.name
            \\ORDER BY COUNT(mig.media_item_id) DESC
        );
        defer stmt.deinit();
        return try stmt.all(types.Genre, self.allocator, .{}, .{});
    }

    pub fn freeGenres(self: *Library, genres: []types.Genre) void {
        for (genres) |g| self.allocator.free(g.name);
        self.allocator.free(genres);
    }

    pub fn getItemsByGenre(self: *Library, genre_name: []const u8, limit: u32) ![]types.MediaItem {
        var stmt = try self.db.db.prepare(
            \\SELECT m.id, m.source, m.source_id, m.server_id, m.media_type, m.title,
            \\       m.sort_title, m.year, m.summary, m.rating, m.duration_ms,
            \\       m.poster_path, m.backdrop_path, m.tmdb_id, m.parent_id,
            \\       m.season_number, m.episode_number, m.file_path, m.library_section,
            \\       m.added_at, m.updated_at, m.match_locked
            \\FROM media_items m
            \\INNER JOIN media_item_genres mig ON m.id = mig.media_item_id
            \\INNER JOIN genres g ON mig.genre_id = g.id
            \\WHERE g.name = ?{[]const u8} AND m.media_type IN ('movie', 'show')
            \\ORDER BY m.added_at DESC LIMIT ?{i32}
        );
        defer stmt.deinit();
        return try stmt.all(types.MediaItem, self.allocator, .{}, .{ genre_name, @as(i32, @intCast(limit)) });
    }

    // Match lock operations

    pub fn setMatchLocked(self: *Library, media_item_id: i64, locked: bool) !void {
        try self.db.db.exec(
            "UPDATE media_items SET match_locked = ?{bool} WHERE id = ?{i64}",
            .{},
            .{ locked, media_item_id },
        );
    }

    pub fn updateMediaItemMetadata(self: *Library, id: i64, item: types.MediaItem) !void {
        try self.db.db.exec(
            \\UPDATE media_items SET
            \\  title = ?{[]const u8}, sort_title = ?{?[]const u8}, year = ?{?i32},
            \\  summary = ?{?[]const u8}, rating = ?{?f64},
            \\  duration_ms = ?{?i64}, poster_path = ?{?[]const u8},
            \\  backdrop_path = ?{?[]const u8}, tmdb_id = ?{?i32},
            \\  match_locked = ?{bool}, updated_at = ?{i64}
            \\WHERE id = ?{i64}
        , .{}, .{
            item.title,       item.sort_title,   item.year,
            item.summary,     item.rating,
            item.duration_ms, item.poster_path,
            item.backdrop_path, item.tmdb_id,
            item.match_locked, unixTimestamp(),
            id,
        });
    }

    // Collection operations

    pub fn createCollection(self: *Library, name: []const u8, collection_type: types.CollectionType, description: ?[]const u8) !i64 {
        // Get max sort_order
        const MaxResult = struct { max_order: i32 };
        const max_row = try self.db.db.one(MaxResult,
            "SELECT COALESCE(MAX(sort_order), -1) FROM collections",
            .{},
            .{},
        );
        const next_order: i32 = if (max_row) |r| r.max_order + 1 else 0;

        const now = unixTimestamp();
        try self.db.db.exec(
            \\INSERT INTO collections (name, collection_type, description, sort_order, created_at, updated_at)
            \\VALUES (?{[]const u8}, ?{[]const u8}, ?{?[]const u8}, ?{i32}, ?{i64}, ?{i64})
        , .{}, .{ name, collection_type, description, next_order, now, now });
        return self.db.db.getLastInsertRowID();
    }

    pub fn getCollection(self: *Library, id: i64) !?types.Collection {
        return try self.db.db.oneAlloc(types.Collection, self.allocator,
            \\SELECT id, name, collection_type, description, poster_path,
            \\       show_on_home, sort_order, created_at, updated_at
            \\FROM collections WHERE id = ?{i64}
        , .{}, .{id});
    }

    pub fn listCollections(self: *Library) ![]types.Collection {
        var stmt = try self.db.db.prepare(
            \\SELECT id, name, collection_type, description, poster_path,
            \\       show_on_home, sort_order, created_at, updated_at
            \\FROM collections ORDER BY sort_order ASC, name ASC
        );
        defer stmt.deinit();
        return try stmt.all(types.Collection, self.allocator, .{}, .{});
    }

    pub fn deleteCollection(self: *Library, id: i64) !void {
        try self.db.db.exec(
            "DELETE FROM collections WHERE id = ?{i64}",
            .{},
            .{id},
        );
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
        try self.db.db.exec(
            \\INSERT OR IGNORE INTO collection_items (collection_id, media_item_id, added_at)
            \\VALUES (?{i64}, ?{i64}, ?{i64})
        , .{}, .{ collection_id, media_item_id, unixTimestamp() });
    }

    pub fn removeFromCollection(self: *Library, collection_id: i64, media_item_id: i64) !void {
        try self.db.db.exec(
            "DELETE FROM collection_items WHERE collection_id = ?{i64} AND media_item_id = ?{i64}",
            .{},
            .{ collection_id, media_item_id },
        );
    }

    pub fn getCollectionItems(self: *Library, collection_id: i64) ![]types.MediaItem {
        var stmt = try self.db.db.prepare(
            \\SELECT m.id, m.source, m.source_id, m.server_id, m.media_type, m.title,
            \\       m.sort_title, m.year, m.summary, m.rating, m.duration_ms,
            \\       m.poster_path, m.backdrop_path, m.tmdb_id, m.parent_id,
            \\       m.season_number, m.episode_number, m.file_path, m.library_section,
            \\       m.added_at, m.updated_at, m.match_locked
            \\FROM media_items m
            \\INNER JOIN collection_items ci ON m.id = ci.media_item_id
            \\WHERE ci.collection_id = ?{i64}
            \\ORDER BY ci.sort_order ASC, ci.added_at DESC
        );
        defer stmt.deinit();
        return try stmt.all(types.MediaItem, self.allocator, .{}, .{collection_id});
    }

    // Collection rules

    pub fn addCollectionRule(self: *Library, collection_id: i64, field: []const u8, operator: []const u8, value: []const u8) !i64 {
        try self.db.db.exec(
            \\INSERT INTO collection_rules (collection_id, field, operator, value)
            \\VALUES (?{i64}, ?{[]const u8}, ?{[]const u8}, ?{[]const u8})
        , .{}, .{ collection_id, field, operator, value });
        return self.db.db.getLastInsertRowID();
    }

    pub fn getCollectionRules(self: *Library, collection_id: i64) ![]types.CollectionRule {
        var stmt = try self.db.db.prepare(
            \\SELECT id, collection_id, field, operator, value
            \\FROM collection_rules WHERE collection_id = ?{i64}
        );
        defer stmt.deinit();
        return try stmt.all(types.CollectionRule, self.allocator, .{}, .{collection_id});
    }

    pub fn removeCollectionRule(self: *Library, rule_id: i64) !void {
        try self.db.db.exec(
            "DELETE FROM collection_rules WHERE id = ?{i64}",
            .{},
            .{rule_id},
        );
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

        var where_parts: std.ArrayList([]const u8) = .empty;
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
        var full_sql: std.ArrayList(u8) = .empty;
        try full_sql.appendSlice(self.allocator,
            \\SELECT m.id, m.source, m.source_id, m.server_id, m.media_type, m.title,
            \\       m.sort_title, m.year, m.summary, m.rating, m.duration_ms,
            \\       m.poster_path, m.backdrop_path, m.tmdb_id, m.parent_id,
            \\       m.season_number, m.episode_number, m.file_path, m.library_section,
            \\       m.added_at, m.updated_at, m.match_locked
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
        defer full_sql.deinit(self.allocator);

        var stmt = try self.db.db.prepareDynamic(full_sql.items);
        defer stmt.deinit();

        // Bind rule values positionally using raw sqlite3 API
        var bind_idx: c_int = 1;
        for (rules) |rule| {
            if (std.mem.eql(u8, rule.field, "genre") or
                std.mem.eql(u8, rule.field, "media_type") or
                std.mem.eql(u8, rule.field, "source"))
            {
                const rc = sqlite.c.sqlite3_bind_text(
                    stmt.stmt,
                    bind_idx,
                    rule.value.ptr,
                    @intCast(rule.value.len),
                    sqlite.c.SQLITE_STATIC,
                );
                if (rc != sqlite.c.SQLITE_OK) return error.SqlExecFailed;
                bind_idx += 1;
            } else if (std.mem.eql(u8, rule.field, "year") or
                std.mem.eql(u8, rule.field, "watched"))
            {
                const int_val = std.fmt.parseInt(i64, rule.value, 10) catch 0;
                const rc = sqlite.c.sqlite3_bind_int64(
                    stmt.stmt,
                    bind_idx,
                    @intCast(int_val),
                );
                if (rc != sqlite.c.SQLITE_OK) return error.SqlExecFailed;
                bind_idx += 1;
            }
        }

        // Read results using the iterator
        var iter = try stmt.iteratorAlloc(types.MediaItem, self.allocator, .{});
        var results: std.ArrayList(types.MediaItem) = .empty;
        while (try iter.nextAlloc(self.allocator, .{})) |row| {
            try results.append(self.allocator, row);
        }
        return results.toOwnedSlice(self.allocator);
    }

    pub fn freeMediaItems(self: *Library, items: []types.MediaItem) void {
        for (items) |item| self.freeMediaItem(item);
        self.allocator.free(items);
    }

    // Server operations
    pub fn upsertServer(self: *Library, server: types.Server) !void {
        try self.db.db.exec(
            \\INSERT OR REPLACE INTO servers
            \\  (id, name, client_identifier, auth_token, connection_uri, last_connected_at)
            \\VALUES (?{[]const u8}, ?{[]const u8}, ?{[]const u8}, ?{?[]const u8}, ?{?[]const u8}, ?{?i64})
        , .{}, .{
            server.id,
            server.name,
            server.client_identifier,
            server.auth_token,
            server.connection_uri,
            server.last_connected_at,
        });
    }

    pub fn getServer(self: *Library, id: []const u8) !?types.Server {
        return try self.db.db.oneAlloc(types.Server, self.allocator,
            \\SELECT id, name, client_identifier, auth_token, connection_uri, last_connected_at
            \\FROM servers WHERE id = ?{[]const u8}
        , .{}, .{id});
    }

    pub fn deleteServer(self: *Library, id: []const u8) !void {
        try self.db.db.exec(
            "DELETE FROM servers WHERE id = ?{[]const u8}",
            .{},
            .{id},
        );
    }

    pub fn freeServer(self: *Library, server: types.Server) void {
        self.allocator.free(server.id);
        self.allocator.free(server.name);
        self.allocator.free(server.client_identifier);
        if (server.auth_token) |t| self.allocator.free(t);
        if (server.connection_uri) |u| self.allocator.free(u);
    }

    // -- Server Connections --

    pub fn upsertServerConnections(self: *Library, server_id: []const u8, connections: []const types.ServerConnection) !void {
        // Delete existing connections for this server
        try self.db.db.exec(
            "DELETE FROM server_connections WHERE server_id = ?{[]const u8}",
            .{},
            .{server_id},
        );

        // Insert all new connections
        for (connections) |conn| {
            try self.db.db.exec(
                \\INSERT INTO server_connections (server_id, uri, is_local, is_relay, protocol)
                \\VALUES (?{[]const u8}, ?{[]const u8}, ?{bool}, ?{bool}, ?{[]const u8})
            , .{}, .{ server_id, conn.uri, conn.is_local, conn.is_relay, conn.protocol });
        }
    }

    pub fn getServerConnections(self: *Library, server_id: []const u8) ![]types.ServerConnection {
        var stmt = try self.db.db.prepare(
            \\SELECT id, server_id, uri, is_local, is_relay, protocol, latency_ms
            \\FROM server_connections WHERE server_id = ?{[]const u8}
            \\ORDER BY is_local DESC, is_relay ASC
        );
        defer stmt.deinit();
        return try stmt.all(types.ServerConnection, self.allocator, .{}, .{server_id});
    }

    pub fn updateServerBestUri(self: *Library, server_id: []const u8, uri: []const u8) !void {
        try self.db.db.exec(
            "UPDATE servers SET connection_uri = ?{[]const u8} WHERE id = ?{[]const u8}",
            .{},
            .{ uri, server_id },
        );
    }

    pub fn freeServerConnections(self: *Library, conns: []types.ServerConnection) void {
        for (conns) |conn| {
            self.allocator.free(conn.server_id);
            self.allocator.free(conn.uri);
            self.allocator.free(conn.protocol);
        }
        self.allocator.free(conns);
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
        if (item.library_section) |s| self.allocator.free(s);
    }
};

/// Get current unix timestamp in seconds.
fn unixTimestamp() i64 {
    var ts: std.c.timespec = undefined;
    _ = std.c.clock_gettime(.REALTIME, &ts);
    return @intCast(ts.sec);
}

fn sqlOperator(op: []const u8) []const u8 {
    if (std.mem.eql(u8, op, "eq")) return "=";
    if (std.mem.eql(u8, op, "neq")) return "!=";
    if (std.mem.eql(u8, op, "gt")) return ">";
    if (std.mem.eql(u8, op, "gte")) return ">=";
    if (std.mem.eql(u8, op, "lt")) return "<";
    if (std.mem.eql(u8, op, "lte")) return "<=";
    return "=";
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

    // Get distinct genres -- Action should come first (2 items) then Sci-Fi (1 item)
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
