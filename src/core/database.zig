const std = @import("std");
const c = @cImport({
    @cInclude("sqlite3.h");
});

pub const Database = struct {
    db: *c.sqlite3,
    mutex: std.Thread.Mutex = .{},

    pub fn open(path: [*:0]const u8) !Database {
        var db: ?*c.sqlite3 = null;
        const rc = c.sqlite3_open(path, &db);
        if (rc != c.SQLITE_OK) {
            if (db) |d| _ = c.sqlite3_close(d);
            return error.DatabaseOpenFailed;
        }

        var self = Database{ .db = db.? };

        // Enable WAL mode for concurrent reads
        try self.exec("PRAGMA journal_mode=WAL");
        // Enable foreign keys
        try self.exec("PRAGMA foreign_keys=ON");

        try self.migrate();
        return self;
    }

    pub fn close(self: *Database) void {
        // Passive WAL checkpoint on clean shutdown
        _ = c.sqlite3_wal_checkpoint_v2(self.db, null, c.SQLITE_CHECKPOINT_PASSIVE, null, null);
        _ = c.sqlite3_close(self.db);
    }

    pub fn exec(self: *Database, sql: [*:0]const u8) !void {
        var err_msg: [*c]u8 = null;
        const rc = c.sqlite3_exec(self.db, sql, null, null, &err_msg);
        if (rc != c.SQLITE_OK) {
            if (err_msg) |msg| {
                std.log.err("SQL error: {s}", .{std.mem.span(msg)});
                c.sqlite3_free(msg);
            }
            return error.SqlExecFailed;
        }
    }

    pub fn prepare(self: *Database, sql: [*:0]const u8) !Statement {
        var stmt: ?*c.sqlite3_stmt = null;
        const rc = c.sqlite3_prepare_v2(self.db, sql, -1, &stmt, null);
        if (rc != c.SQLITE_OK) {
            std.log.err("SQL prepare error: {s}", .{std.mem.span(c.sqlite3_errmsg(self.db))});
            return error.SqlPrepareFailed;
        }
        return Statement{ .stmt = stmt.? };
    }

    /// Like prepare but does not log on failure (for expected-failure queries like schema checks).
    fn prepareSilent(self: *Database, sql: [*:0]const u8) !Statement {
        var stmt: ?*c.sqlite3_stmt = null;
        const rc = c.sqlite3_prepare_v2(self.db, sql, -1, &stmt, null);
        if (rc != c.SQLITE_OK) {
            return error.SqlPrepareFailed;
        }
        return Statement{ .stmt = stmt.? };
    }

    fn getSchemaVersion(self: *Database) !i32 {
        var stmt = self.prepareSilent("SELECT version FROM schema_version ORDER BY version DESC LIMIT 1") catch {
            return 0;
        };
        defer stmt.finalize();

        if (stmt.step()) {
            return stmt.columnInt(0);
        }
        return 0;
    }

    fn setSchemaVersion(self: *Database, version: i32) !void {
        var stmt = try self.prepare("INSERT OR REPLACE INTO schema_version (version) VALUES (?)");
        defer stmt.finalize();
        stmt.bindInt(1, version);
        _ = stmt.step();
    }

    fn migrate(self: *Database) !void {
        const version = self.getSchemaVersion() catch 0;

        if (version < 1) {
            try self.exec(
                \\CREATE TABLE IF NOT EXISTS schema_version (
                \\    version INTEGER PRIMARY KEY
                \\);
                \\
                \\CREATE TABLE IF NOT EXISTS servers (
                \\    id TEXT PRIMARY KEY,
                \\    name TEXT NOT NULL,
                \\    client_identifier TEXT NOT NULL,
                \\    auth_token TEXT,
                \\    connection_uri TEXT,
                \\    last_connected_at INTEGER
                \\);
                \\
                \\CREATE TABLE IF NOT EXISTS media_items (
                \\    id INTEGER PRIMARY KEY AUTOINCREMENT,
                \\    source TEXT NOT NULL,
                \\    source_id TEXT,
                \\    server_id TEXT REFERENCES servers(id),
                \\    media_type TEXT NOT NULL,
                \\    title TEXT NOT NULL,
                \\    sort_title TEXT,
                \\    year INTEGER,
                \\    summary TEXT,
                \\    rating REAL,
                \\    duration_ms INTEGER,
                \\    poster_path TEXT,
                \\    backdrop_path TEXT,
                \\    tmdb_id INTEGER,
                \\    parent_id INTEGER REFERENCES media_items(id),
                \\    season_number INTEGER,
                \\    episode_number INTEGER,
                \\    file_path TEXT,
                \\    added_at INTEGER,
                \\    updated_at INTEGER
                \\);
                \\
                \\CREATE TABLE IF NOT EXISTS watch_progress (
                \\    media_item_id INTEGER PRIMARY KEY REFERENCES media_items(id),
                \\    position_ms INTEGER NOT NULL DEFAULT 0,
                \\    duration_ms INTEGER,
                \\    watched INTEGER NOT NULL DEFAULT 0,
                \\    last_watched_at INTEGER
                \\);
                \\
                \\CREATE TABLE IF NOT EXISTS downloads (
                \\    id INTEGER PRIMARY KEY AUTOINCREMENT,
                \\    media_item_id INTEGER REFERENCES media_items(id),
                \\    server_id TEXT REFERENCES servers(id),
                \\    source_url TEXT NOT NULL,
                \\    local_path TEXT,
                \\    total_bytes INTEGER,
                \\    downloaded_bytes INTEGER DEFAULT 0,
                \\    status TEXT NOT NULL DEFAULT 'queued',
                \\    created_at INTEGER,
                \\    completed_at INTEGER
                \\);
                \\
                \\CREATE TABLE IF NOT EXISTS scan_paths (
                \\    id INTEGER PRIMARY KEY AUTOINCREMENT,
                \\    path TEXT NOT NULL UNIQUE,
                \\    last_scanned_at INTEGER
                \\);
                \\
                \\CREATE TABLE IF NOT EXISTS image_cache (
                \\    url TEXT PRIMARY KEY,
                \\    local_path TEXT NOT NULL,
                \\    size_bytes INTEGER,
                \\    cached_at INTEGER
                \\);
                \\
                \\CREATE TABLE IF NOT EXISTS settings (
                \\    key TEXT PRIMARY KEY,
                \\    value TEXT
                \\);
                \\
                \\CREATE INDEX IF NOT EXISTS idx_media_items_source ON media_items(source, source_id);
                \\CREATE INDEX IF NOT EXISTS idx_media_items_type ON media_items(media_type);
                \\CREATE INDEX IF NOT EXISTS idx_media_items_parent ON media_items(parent_id);
                \\CREATE INDEX IF NOT EXISTS idx_media_items_tmdb ON media_items(tmdb_id);
            );
            try self.setSchemaVersion(1);
        }
    }
};

pub const Statement = struct {
    stmt: *c.sqlite3_stmt,

    pub fn finalize(self: *Statement) void {
        _ = c.sqlite3_finalize(self.stmt);
    }

    pub fn reset(self: *Statement) void {
        _ = c.sqlite3_reset(self.stmt);
        _ = c.sqlite3_clear_bindings(self.stmt);
    }

    pub fn step(self: *Statement) bool {
        const rc = c.sqlite3_step(self.stmt);
        return rc == c.SQLITE_ROW;
    }

    pub fn exec(self: *Statement) !void {
        const rc = c.sqlite3_step(self.stmt);
        if (rc != c.SQLITE_DONE and rc != c.SQLITE_ROW) {
            return error.SqlStepFailed;
        }
    }

    pub fn lastInsertRowId(self: *Statement) i64 {
        return c.sqlite3_last_insert_rowid(c.sqlite3_db_handle(self.stmt));
    }

    // Bind helpers
    pub fn bindInt(self: *Statement, col: c_int, val: i32) void {
        _ = c.sqlite3_bind_int(self.stmt, col, val);
    }

    pub fn bindInt64(self: *Statement, col: c_int, val: i64) void {
        _ = c.sqlite3_bind_int64(self.stmt, col, val);
    }

    pub fn bindDouble(self: *Statement, col: c_int, val: f64) void {
        _ = c.sqlite3_bind_double(self.stmt, col, val);
    }

    pub fn bindText(self: *Statement, col: c_int, val: []const u8) void {
        _ = c.sqlite3_bind_text(self.stmt, col, @ptrCast(val.ptr), @intCast(val.len), c.SQLITE_TRANSIENT);
    }

    pub fn bindNull(self: *Statement, col: c_int) void {
        _ = c.sqlite3_bind_null(self.stmt, col);
    }

    pub fn bindOptionalText(self: *Statement, col: c_int, val: ?[]const u8) void {
        if (val) |v| {
            self.bindText(col, v);
        } else {
            self.bindNull(col);
        }
    }

    pub fn bindOptionalInt(self: *Statement, col: c_int, val: ?i32) void {
        if (val) |v| {
            self.bindInt(col, v);
        } else {
            self.bindNull(col);
        }
    }

    pub fn bindOptionalInt64(self: *Statement, col: c_int, val: ?i64) void {
        if (val) |v| {
            self.bindInt64(col, v);
        } else {
            self.bindNull(col);
        }
    }

    pub fn bindOptionalDouble(self: *Statement, col: c_int, val: ?f64) void {
        if (val) |v| {
            self.bindDouble(col, v);
        } else {
            self.bindNull(col);
        }
    }

    // Column getters
    pub fn columnInt(self: *Statement, col: c_int) i32 {
        return c.sqlite3_column_int(self.stmt, col);
    }

    pub fn columnInt64(self: *Statement, col: c_int) i64 {
        return c.sqlite3_column_int64(self.stmt, col);
    }

    pub fn columnDouble(self: *Statement, col: c_int) f64 {
        return c.sqlite3_column_double(self.stmt, col);
    }

    pub fn columnText(self: *Statement, col: c_int) ?[]const u8 {
        const ptr = c.sqlite3_column_text(self.stmt, col);
        if (ptr == null) return null;
        const len = c.sqlite3_column_bytes(self.stmt, col);
        if (len <= 0) return null;
        return ptr[0..@intCast(len)];
    }

    pub fn columnBool(self: *Statement, col: c_int) bool {
        return c.sqlite3_column_int(self.stmt, col) != 0;
    }

    pub fn columnOptionalInt(self: *Statement, col: c_int) ?i32 {
        if (c.sqlite3_column_type(self.stmt, col) == c.SQLITE_NULL) return null;
        return self.columnInt(col);
    }

    pub fn columnOptionalInt64(self: *Statement, col: c_int) ?i64 {
        if (c.sqlite3_column_type(self.stmt, col) == c.SQLITE_NULL) return null;
        return self.columnInt64(col);
    }

    pub fn columnOptionalDouble(self: *Statement, col: c_int) ?f64 {
        if (c.sqlite3_column_type(self.stmt, col) == c.SQLITE_NULL) return null;
        return self.columnDouble(col);
    }
};

test "database open and migrate" {
    var db = try Database.open(":memory:");
    defer db.close();

    const version = try db.getSchemaVersion();
    try std.testing.expectEqual(@as(i32, 1), version);
}

test "database insert and query" {
    var db = try Database.open(":memory:");
    defer db.close();

    // Insert a setting
    var insert = try db.prepare("INSERT INTO settings (key, value) VALUES (?, ?)");
    defer insert.finalize();
    insert.bindText(1, "test_key");
    insert.bindText(2, "test_value");
    try insert.exec();

    // Query it back
    var query = try db.prepare("SELECT value FROM settings WHERE key = ?");
    defer query.finalize();
    query.bindText(1, "test_key");
    try std.testing.expect(query.step());
    const val = query.columnText(0).?;
    try std.testing.expectEqualStrings("test_value", val);
}
