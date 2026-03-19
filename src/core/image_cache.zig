const std = @import("std");
const database = @import("database.zig");
const sqlite = database.sqlite;
const http = @import("../net/http.zig");

fn defaultIo() std.Io {
    return std.Io.Threaded.global_single_threaded.io();
}

fn unixTimestamp() i64 {
    var ts: std.c.timespec = undefined;
    _ = std.c.clock_gettime(.REALTIME, &ts);
    return @intCast(ts.sec);
}

pub const ImageCache = struct {
    db: *database.Database,
    allocator: std.mem.Allocator,
    cache_dir: []const u8,

    pub fn init(allocator: std.mem.Allocator, db: *database.Database, cache_dir: []const u8) ImageCache {
        // Ensure cache directory exists
        std.Io.Dir.cwd().createDirPath(defaultIo(), cache_dir) catch {};
        return .{ .db = db, .allocator = allocator, .cache_dir = cache_dir };
    }

    /// Get the local file path for a cached image, or null if not cached.
    pub fn getLocalPath(self: *ImageCache, url: []const u8) !?[]const u8 {
        const row = self.db.db.oneAlloc(
            struct { local_path: []const u8 },
            self.allocator,
            "SELECT local_path FROM image_cache WHERE url = ?{[]const u8}",
            .{},
            .{url},
        ) catch return null;
        if (row) |r| {
            // Verify file still exists
            std.Io.Dir.cwd().access(defaultIo(), r.local_path, .{}) catch {
                self.allocator.free(r.local_path);
                return null;
            };
            return r.local_path;
        }
        return null;
    }

    /// Store a cached image entry in the database.
    pub fn store(self: *ImageCache, url: []const u8, local_path: []const u8, size_bytes: i64) !void {
        self.db.db.exec(
            "INSERT OR REPLACE INTO image_cache (url, local_path, size_bytes, cached_at) VALUES (?{[]const u8}, ?{[]const u8}, ?{i64}, ?{i64})",
            .{},
            .{ url, local_path, size_bytes, unixTimestamp() },
        ) catch return error.SqlExecFailed;
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
        const row = self.db.db.one(
            struct { total: i64 },
            "SELECT COALESCE(SUM(size_bytes), 0) AS total FROM image_cache",
            .{},
            .{},
        ) catch return 0;
        if (row) |r| return r.total;
        return 0;
    }

    /// Pin an image URL so it won't be evicted by LRU (for downloaded items).
    pub fn pin(self: *ImageCache, url: []const u8) !void {
        self.db.db.exec(
            "UPDATE image_cache SET pinned = 1 WHERE url = ?{[]const u8}",
            .{},
            .{url},
        ) catch return error.SqlExecFailed;
    }

    /// Unpin an image URL so it can be evicted normally.
    pub fn unpin(self: *ImageCache, url: []const u8) !void {
        self.db.db.exec(
            "UPDATE image_cache SET pinned = 0 WHERE url = ?{[]const u8}",
            .{},
            .{url},
        ) catch return error.SqlExecFailed;
    }

    /// Evict oldest unpinned cached images until total size is under max_bytes.
    pub fn evictToSize(self: *ImageCache, max_bytes: i64) !void {
        while (try self.totalSize() > max_bytes) {
            const row = self.db.db.oneAlloc(
                struct { url: []const u8, local_path: []const u8 },
                self.allocator,
                "SELECT url, local_path FROM image_cache WHERE pinned = 0 ORDER BY cached_at ASC LIMIT 1",
                .{},
                .{},
            ) catch break;

            if (row) |r| {
                defer self.allocator.free(r.url);
                defer self.allocator.free(r.local_path);

                // Delete the file
                std.Io.Dir.cwd().deleteFile(defaultIo(),r.local_path) catch {};

                // Delete from DB
                self.db.db.exec(
                    "DELETE FROM image_cache WHERE url = ?{[]const u8}",
                    .{},
                    .{r.url},
                ) catch {};
            } else {
                break;
            }
        }
    }

    /// Clear all cached images.
    pub fn clearAll(self: *ImageCache) !void {
        // Get all local paths and delete files
        var stmt = self.db.db.prepare(
            "SELECT local_path FROM image_cache",
        ) catch return error.SqlExecFailed;
        defer stmt.deinit();

        var iter = stmt.iteratorAlloc(
            struct { local_path: []const u8 },
            self.allocator,
            .{},
        ) catch return error.SqlExecFailed;

        while (true) {
            const entry = iter.nextAlloc(self.allocator, .{}) catch break;
            if (entry) |e| {
                defer self.allocator.free(e.local_path);
                std.Io.Dir.cwd().deleteFile(defaultIo(),e.local_path) catch {};
            } else break;
        }

        self.db.db.exec(
            "DELETE FROM image_cache",
            .{},
            .{},
        ) catch return error.SqlExecFailed;
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
