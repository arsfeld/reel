const std = @import("std");
const database = @import("database.zig");
const http = @import("../net/http.zig");

pub const ImageCache = struct {
    db: *database.Database,
    allocator: std.mem.Allocator,
    cache_dir: []const u8,

    pub fn init(allocator: std.mem.Allocator, db: *database.Database, cache_dir: []const u8) ImageCache {
        // Ensure cache directory exists
        std.fs.cwd().makePath(cache_dir) catch {};
        return .{ .db = db, .allocator = allocator, .cache_dir = cache_dir };
    }

    /// Get the local file path for a cached image, or null if not cached.
    pub fn getLocalPath(self: *ImageCache, url: []const u8) !?[]const u8 {
        var stmt = try self.db.prepare(
            "SELECT local_path FROM image_cache WHERE url = ?"
        );
        defer stmt.finalize();
        stmt.bindText(1, url);

        if (stmt.step()) {
            if (stmt.columnText(0)) |path| {
                // Verify file still exists
                std.fs.cwd().access(path, .{}) catch return null;
                return try self.allocator.dupe(u8, path);
            }
        }
        return null;
    }

    /// Store a cached image entry in the database.
    pub fn store(self: *ImageCache, url: []const u8, local_path: []const u8, size_bytes: i64) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare(
            \\INSERT OR REPLACE INTO image_cache (url, local_path, size_bytes, cached_at)
            \\VALUES (?, ?, ?, ?)
        );
        defer stmt.finalize();
        stmt.bindText(1, url);
        stmt.bindText(2, local_path);
        stmt.bindInt64(3, size_bytes);
        stmt.bindInt64(4, std.time.timestamp());
        try stmt.exec();
    }

    /// Generate a deterministic local filename from a URL using a hash.
    pub fn localPathForUrl(self: *ImageCache, url: []const u8) ![]const u8 {
        // Hash the URL to create a unique filename
        var hash: [16]u8 = undefined;
        std.crypto.hash.Md5.hash(url, &hash, .{});

        var hex_buf: [32]u8 = undefined;
        const hex = std.fmt.bufPrint(&hex_buf, "{s}", .{std.fmt.bytesToHex(hash, .lower)}) catch return error.FormatFailed;

        // Determine extension from URL
        const ext = blk: {
            const last_dot = std.mem.lastIndexOfScalar(u8, url, '.') orelse break :blk ".jpg";
            const after_dot = url[last_dot..];
            if (after_dot.len <= 5 and after_dot.len > 1) {
                // Check for query string
                if (std.mem.indexOfScalar(u8, after_dot, '?')) |q| {
                    break :blk after_dot[0..q];
                }
                break :blk after_dot;
            }
            break :blk ".jpg";
        };

        return std.fmt.allocPrint(self.allocator, "{s}/{s}{s}", .{
            self.cache_dir, hex, ext,
        });
    }

    /// Get the total size of cached images in bytes.
    pub fn totalSize(self: *ImageCache) !i64 {
        var stmt = try self.db.prepare(
            "SELECT COALESCE(SUM(size_bytes), 0) FROM image_cache"
        );
        defer stmt.finalize();
        if (stmt.step()) {
            return stmt.columnInt64(0);
        }
        return 0;
    }

    /// Pin an image URL so it won't be evicted by LRU (for downloaded items).
    pub fn pin(self: *ImageCache, url: []const u8) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare("UPDATE image_cache SET pinned = 1 WHERE url = ?");
        defer stmt.finalize();
        stmt.bindText(1, url);
        try stmt.exec();
    }

    /// Unpin an image URL so it can be evicted normally.
    pub fn unpin(self: *ImageCache, url: []const u8) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare("UPDATE image_cache SET pinned = 0 WHERE url = ?");
        defer stmt.finalize();
        stmt.bindText(1, url);
        try stmt.exec();
    }

    /// Evict oldest unpinned cached images until total size is under max_bytes.
    pub fn evictToSize(self: *ImageCache, max_bytes: i64) !void {
        while (try self.totalSize() > max_bytes) {
            // Delete oldest unpinned entry
            var stmt = try self.db.prepare(
                "SELECT url, local_path FROM image_cache WHERE pinned = 0 ORDER BY cached_at ASC LIMIT 1"
            );
            defer stmt.finalize();

            if (stmt.step()) {
                const url = stmt.columnText(0) orelse break;
                const local_path = stmt.columnText(1);

                // Delete the file
                if (local_path) |path| {
                    std.fs.cwd().deleteFile(path) catch {};
                }

                // Delete from DB
                self.db.mutex.lock();
                defer self.db.mutex.unlock();

                var del_stmt = self.db.prepare(
                    "DELETE FROM image_cache WHERE url = ?"
                ) catch break;
                defer del_stmt.finalize();
                del_stmt.bindText(1, url);
                del_stmt.exec() catch {};
            } else {
                break;
            }
        }
    }

    /// Clear all cached images.
    pub fn clearAll(self: *ImageCache) !void {
        // Get all local paths and delete files
        var stmt = try self.db.prepare("SELECT local_path FROM image_cache");
        defer stmt.finalize();

        while (stmt.step()) {
            if (stmt.columnText(0)) |path| {
                std.fs.cwd().deleteFile(path) catch {};
            }
        }

        self.db.mutex.lock();
        defer self.db.mutex.unlock();
        try self.db.exec("DELETE FROM image_cache");
    }
};

test "image cache local path generation" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var cache = ImageCache.init(std.testing.allocator, &db, "/tmp/reel-test-cache");

    const path = try cache.localPathForUrl("https://image.tmdb.org/t/p/w500/abc123.jpg");
    defer std.testing.allocator.free(path);

    try std.testing.expect(std.mem.startsWith(u8, path, "/tmp/reel-test-cache/"));
    try std.testing.expect(std.mem.endsWith(u8, path, ".jpg"));
}

test "image cache store and retrieve" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var cache = ImageCache.init(std.testing.allocator, &db, "/tmp/reel-test-cache");

    // Store an entry (we won't create a real file)
    try cache.store("https://example.com/poster.jpg", "/tmp/reel-test-cache/abcd.jpg", 1024);

    // Total size
    const size = try cache.totalSize();
    try std.testing.expectEqual(@as(i64, 1024), size);
}
