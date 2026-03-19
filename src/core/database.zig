const std = @import("std");
pub const sqlite = @import("sqlite");

pub const Database = struct {
    db: sqlite.Db,

    pub fn open(path: [*:0]const u8) !Database {
        var self = try openRaw(path);

        // Detect incompatible pre-rewrite schema (had a 'sources' table).
        // Rename the old DB and start fresh.
        if (self.hasTable("sources")) {
            self.close();
            renameOldDb(path);
            self = try openRaw(path);
        }

        try self.migrate();
        return self;
    }

    fn openRaw(path: [*:0]const u8) !Database {
        const span = std.mem.span(path);
        var db = sqlite.Db.init(.{
            .mode = .{ .File = span },
            .open_flags = .{ .write = true, .create = true },
            .threading_mode = .Serialized,
        }) catch return error.DatabaseOpenFailed;

        // Enable WAL mode for concurrent reads
        _ = db.pragma(void, .{}, "journal_mode", "WAL") catch {};
        // Enable foreign keys
        _ = db.pragma(void, .{}, "foreign_keys", "ON") catch {};

        return Database{ .db = db };
    }

    fn hasTable(self: *Database, name: []const u8) bool {
        const result = self.db.one(
            struct { n: i32 },
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?{[]const u8}",
            .{},
            .{name},
        ) catch return false;
        return result != null;
    }

    fn renameOldDb(path: [*:0]const u8) void {
        const span = std.mem.span(path);
        var buf: [512]u8 = undefined;
        const backup = std.fmt.bufPrintZ(&buf, "{s}.old", .{span}) catch return;
        std.fs.cwd().rename(span, backup) catch {};
        // Also clean up WAL/SHM files
        var wal_buf: [512]u8 = undefined;
        var shm_buf: [512]u8 = undefined;
        const wal = std.fmt.bufPrintZ(&wal_buf, "{s}-wal", .{span}) catch return;
        const shm = std.fmt.bufPrintZ(&shm_buf, "{s}-shm", .{span}) catch return;
        var wal_old: [512]u8 = undefined;
        var shm_old: [512]u8 = undefined;
        const wal_bak = std.fmt.bufPrintZ(&wal_old, "{s}-wal", .{backup}) catch return;
        const shm_bak = std.fmt.bufPrintZ(&shm_old, "{s}-shm", .{backup}) catch return;
        std.fs.cwd().rename(wal, wal_bak) catch {};
        std.fs.cwd().rename(shm, shm_bak) catch {};
    }

    pub fn close(self: *Database) void {
        // Passive WAL checkpoint on clean shutdown
        _ = sqlite.c.sqlite3_wal_checkpoint_v2(self.db.db, null, sqlite.c.SQLITE_CHECKPOINT_PASSIVE, null, null);
        self.db.deinit();
    }

    pub fn getSchemaVersion(self: *Database) !i32 {
        const result = self.db.one(
            struct { version: i32 },
            "SELECT version FROM schema_version ORDER BY version DESC LIMIT 1",
            .{},
            .{},
        ) catch return 0;
        if (result) |r| return r.version;
        return 0;
    }

    fn setSchemaVersion(self: *Database, version: i32) !void {
        self.db.exec(
            "INSERT OR REPLACE INTO schema_version (version) VALUES (?{i32})",
            .{},
            .{version},
        ) catch return error.SqlExecFailed;
    }

    pub fn getLastInsertRowId(self: *Database) i64 {
        return self.db.getLastInsertRowID();
    }

    fn migrate(self: *Database) !void {
        const version = self.getSchemaVersion() catch 0;

        // Ensure all base tables exist (idempotent).
        self.db.execMulti(
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
            \\CREATE TABLE IF NOT EXISTS favorites (
            \\    id INTEGER PRIMARY KEY AUTOINCREMENT,
            \\    item_type TEXT NOT NULL,
            \\    item_id TEXT NOT NULL,
            \\    display_name TEXT NOT NULL,
            \\    sort_order INTEGER NOT NULL DEFAULT 0,
            \\    created_at INTEGER
            \\);
            \\
            \\CREATE INDEX IF NOT EXISTS idx_media_items_source ON media_items(source, source_id);
            \\CREATE INDEX IF NOT EXISTS idx_media_items_type ON media_items(media_type);
            \\CREATE INDEX IF NOT EXISTS idx_media_items_parent ON media_items(parent_id);
            \\CREATE INDEX IF NOT EXISTS idx_media_items_tmdb ON media_items(tmdb_id);
            \\CREATE INDEX IF NOT EXISTS idx_media_items_added ON media_items(added_at);
            \\CREATE INDEX IF NOT EXISTS idx_media_items_title ON media_items(sort_title, title);
        , .{}) catch return error.SqlExecFailed;

        if (version < 3) {
            self.db.execMulti("ALTER TABLE downloads ADD COLUMN error_message TEXT", .{}) catch {};
            self.db.execMulti("ALTER TABLE downloads ADD COLUMN part_key TEXT", .{}) catch {};
            self.db.execMulti("CREATE INDEX IF NOT EXISTS idx_downloads_status ON downloads(status)", .{}) catch {};
            self.db.execMulti("CREATE INDEX IF NOT EXISTS idx_downloads_media_item_id ON downloads(media_item_id)", .{}) catch {};
            self.db.execMulti("ALTER TABLE image_cache ADD COLUMN pinned INTEGER DEFAULT 0", .{}) catch {};
            try self.setSchemaVersion(3);
        }

        if (version < 4) {
            self.db.execMulti(
                \\CREATE TABLE IF NOT EXISTS genres (
                \\    id INTEGER PRIMARY KEY AUTOINCREMENT,
                \\    name TEXT NOT NULL UNIQUE
                \\);
                \\
                \\CREATE TABLE IF NOT EXISTS media_item_genres (
                \\    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
                \\    genre_id INTEGER NOT NULL REFERENCES genres(id) ON DELETE CASCADE,
                \\    PRIMARY KEY (media_item_id, genre_id)
                \\);
                \\
                \\CREATE INDEX IF NOT EXISTS idx_media_item_genres_genre ON media_item_genres(genre_id);
            , .{}) catch return error.SqlExecFailed;

            self.db.execMulti("ALTER TABLE media_items ADD COLUMN match_locked INTEGER NOT NULL DEFAULT 0", .{}) catch {};

            self.db.execMulti(
                \\CREATE TABLE IF NOT EXISTS collections (
                \\    id INTEGER PRIMARY KEY AUTOINCREMENT,
                \\    name TEXT NOT NULL,
                \\    collection_type TEXT NOT NULL DEFAULT 'manual',
                \\    description TEXT,
                \\    poster_path TEXT,
                \\    show_on_home INTEGER NOT NULL DEFAULT 1,
                \\    sort_order INTEGER NOT NULL DEFAULT 0,
                \\    created_at INTEGER,
                \\    updated_at INTEGER
                \\);
                \\
                \\CREATE TABLE IF NOT EXISTS collection_items (
                \\    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
                \\    media_item_id INTEGER NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
                \\    sort_order INTEGER NOT NULL DEFAULT 0,
                \\    added_at INTEGER,
                \\    PRIMARY KEY (collection_id, media_item_id)
                \\);
                \\
                \\CREATE TABLE IF NOT EXISTS collection_rules (
                \\    id INTEGER PRIMARY KEY AUTOINCREMENT,
                \\    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
                \\    field TEXT NOT NULL,
                \\    operator TEXT NOT NULL,
                \\    value TEXT NOT NULL
                \\);
                \\
                \\CREATE INDEX IF NOT EXISTS idx_collection_rules_collection ON collection_rules(collection_id);
            , .{}) catch return error.SqlExecFailed;

            try self.setSchemaVersion(4);
        }

        if (version < 5) {
            self.db.execMulti(
                \\CREATE TABLE IF NOT EXISTS server_connections (
                \\    id INTEGER PRIMARY KEY AUTOINCREMENT,
                \\    server_id TEXT NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
                \\    uri TEXT NOT NULL,
                \\    is_local INTEGER NOT NULL DEFAULT 0,
                \\    is_relay INTEGER NOT NULL DEFAULT 0,
                \\    protocol TEXT NOT NULL DEFAULT 'https',
                \\    latency_ms INTEGER,
                \\    UNIQUE(server_id, uri)
                \\);
                \\
                \\CREATE INDEX IF NOT EXISTS idx_server_connections_server ON server_connections(server_id);
            , .{}) catch return error.SqlExecFailed;
            try self.setSchemaVersion(5);
        }

        if (version < 6) {
            self.db.execMulti("ALTER TABLE media_items ADD COLUMN library_section TEXT", .{}) catch {};
            self.db.execMulti("CREATE INDEX IF NOT EXISTS idx_media_items_library_section ON media_items(library_section)", .{}) catch {};
            try self.setSchemaVersion(6);
        }
    }
};

test "database open and migrate" {
    var db = try Database.open(":memory:");
    defer db.close();

    const version = try db.getSchemaVersion();
    try std.testing.expectEqual(@as(i32, 6), version);
}

test "database insert and query" {
    var db = try Database.open(":memory:");
    defer db.close();

    // Insert a setting
    db.db.exec(
        "INSERT INTO settings (key, value) VALUES (?{[]const u8}, ?{[]const u8})",
        .{},
        .{ "test_key", "test_value" },
    ) catch return error.SqlExecFailed;

    // Query it back
    const result = db.db.one(
        struct { value: []const u8 },
        "SELECT value FROM settings WHERE key = ?{[]const u8}",
        .{},
        .{"test_key"},
    ) catch return error.SqlExecFailed;

    try std.testing.expect(result != null);
    try std.testing.expectEqualStrings("test_value", result.?.value);
}
