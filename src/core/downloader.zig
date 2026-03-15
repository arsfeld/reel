const std = @import("std");
const database = @import("database.zig");
const types = @import("types.zig");

pub const max_concurrent: usize = 2;

pub const DownloadRequest = struct {
    media_item_id: i64,
    server_id: []const u8,
    source_url: []const u8,
    download_dir: []const u8,
    filename: []const u8,
    part_key: ?[]const u8 = null,
};

pub const DownloadProgress = struct {
    download_id: i64,
    downloaded_bytes: i64,
    total_bytes: ?i64,
    status: types.DownloadStatus,
};

const select_columns =
    \\SELECT id, media_item_id, server_id, source_url, local_path,
    \\       total_bytes, downloaded_bytes, status, created_at, completed_at,
    \\       error_message, part_key
;

pub const Downloader = struct {
    db: *database.Database,
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator, db: *database.Database) Downloader {
        return .{ .db = db, .allocator = allocator };
    }

    /// Queue a new download. Returns error.AlreadyExists if a non-failed download exists.
    pub fn enqueue(self: *Downloader, req: DownloadRequest) !i64 {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        // Check for existing download of same media item
        const existing_failed_id: ?i64 = blk: {
            var check_stmt = try self.db.prepare(
                "SELECT id, status FROM downloads WHERE media_item_id = ? LIMIT 1",
            );
            defer check_stmt.finalize();
            check_stmt.bindInt64(1, req.media_item_id);

            if (check_stmt.step()) {
                const existing_status = types.DownloadStatus.fromString(
                    check_stmt.columnText(1) orelse "queued",
                ) orelse .queued;

                switch (existing_status) {
                    .complete, .downloading, .queued, .paused => return error.AlreadyExists,
                    .failed => break :blk check_stmt.columnInt64(0),
                }
            }
            break :blk null;
        };

        // Remove failed entry outside the check statement scope
        if (existing_failed_id) |failed_id| {
            var del_stmt = try self.db.prepare("DELETE FROM downloads WHERE id = ?");
            defer del_stmt.finalize();
            del_stmt.bindInt64(1, failed_id);
            try del_stmt.exec();
        }

        const local_path = try std.fs.path.join(self.allocator, &.{ req.download_dir, req.filename });
        defer self.allocator.free(local_path);

        const now = std.time.timestamp();

        var stmt = try self.db.prepare(
            \\INSERT INTO downloads
            \\  (media_item_id, server_id, source_url, local_path, status, created_at, part_key)
            \\VALUES (?, ?, ?, ?, 'queued', ?, ?)
        );
        defer stmt.finalize();

        stmt.bindInt64(1, req.media_item_id);
        stmt.bindText(2, req.server_id);
        stmt.bindText(3, req.source_url);
        stmt.bindText(4, local_path);
        stmt.bindInt64(5, now);
        stmt.bindOptionalText(6, req.part_key);

        try stmt.exec();
        return stmt.lastInsertRowId();
    }

    /// Get all downloads with given status.
    pub fn getByStatus(self: *Downloader, status: types.DownloadStatus) ![]types.Download {
        var stmt = try self.db.prepare(select_columns ++
            \\ FROM downloads WHERE status = ?
            \\ ORDER BY created_at ASC
        );
        defer stmt.finalize();
        stmt.bindText(1, status.toString());

        var results: std.ArrayList(types.Download) = .{};
        while (stmt.step()) {
            try results.append(self.allocator, try readDownload(self.allocator, &stmt));
        }
        return results.toOwnedSlice(self.allocator);
    }

    /// Get all downloads (all statuses).
    pub fn getAllDownloads(self: *Downloader) ![]types.Download {
        var stmt = try self.db.prepare(select_columns ++
            \\ FROM downloads ORDER BY created_at ASC
        );
        defer stmt.finalize();

        var results: std.ArrayList(types.Download) = .{};
        while (stmt.step()) {
            try results.append(self.allocator, try readDownload(self.allocator, &stmt));
        }
        return results.toOwnedSlice(self.allocator);
    }

    /// Get a single download by ID.
    pub fn getDownload(self: *Downloader, id: i64) !?types.Download {
        var stmt = try self.db.prepare(select_columns ++
            \\ FROM downloads WHERE id = ?
        );
        defer stmt.finalize();
        stmt.bindInt64(1, id);

        if (stmt.step()) {
            return try readDownload(self.allocator, &stmt);
        }
        return null;
    }

    /// Get download for a specific media item (if any).
    pub fn getByMediaItemId(self: *Downloader, media_item_id: i64) !?types.Download {
        var stmt = try self.db.prepare(select_columns ++
            \\ FROM downloads WHERE media_item_id = ? LIMIT 1
        );
        defer stmt.finalize();
        stmt.bindInt64(1, media_item_id);

        if (stmt.step()) {
            return try readDownload(self.allocator, &stmt);
        }
        return null;
    }

    /// Get the local file path for a completed download of a media item.
    pub fn getCompletedLocalPath(self: *Downloader, media_item_id: i64) !?[]const u8 {
        var stmt = try self.db.prepare(
            "SELECT local_path FROM downloads WHERE media_item_id = ? AND status = 'complete' LIMIT 1",
        );
        defer stmt.finalize();
        stmt.bindInt64(1, media_item_id);

        if (stmt.step()) {
            if (stmt.columnText(0)) |path| {
                return try self.allocator.dupe(u8, path);
            }
        }
        return null;
    }

    /// Update download progress.
    pub fn updateProgress(self: *Downloader, id: i64, downloaded: i64, total: ?i64) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare(
            \\UPDATE downloads SET downloaded_bytes = ?, total_bytes = ?
            \\WHERE id = ?
        );
        defer stmt.finalize();
        stmt.bindInt64(1, downloaded);
        stmt.bindOptionalInt64(2, total);
        stmt.bindInt64(3, id);
        try stmt.exec();
    }

    /// Update download status.
    pub fn setStatus(self: *Downloader, id: i64, status: types.DownloadStatus) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare("UPDATE downloads SET status = ? WHERE id = ?");
        defer stmt.finalize();
        stmt.bindText(1, status.toString());
        stmt.bindInt64(2, id);
        try stmt.exec();

        if (status == .complete) {
            var ts_stmt = try self.db.prepare("UPDATE downloads SET completed_at = ? WHERE id = ?");
            defer ts_stmt.finalize();
            ts_stmt.bindInt64(1, std.time.timestamp());
            ts_stmt.bindInt64(2, id);
            try ts_stmt.exec();
        }
    }

    /// Set status to failed with an error message.
    pub fn setFailed(self: *Downloader, id: i64, error_message: []const u8) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare(
            "UPDATE downloads SET status = 'failed', error_message = ? WHERE id = ?",
        );
        defer stmt.finalize();
        stmt.bindText(1, error_message);
        stmt.bindInt64(2, id);
        try stmt.exec();
    }

    /// Reset any 'downloading' entries to 'queued' (for crash recovery on restart).
    pub fn resetInterrupted(self: *Downloader) !void {
        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        try self.db.exec("UPDATE downloads SET status = 'queued' WHERE status = 'downloading'");
    }

    /// Remove a download (and optionally its file).
    pub fn remove(self: *Downloader, id: i64, delete_file: bool) !void {
        if (delete_file) {
            if (try self.getDownload(id)) |dl| {
                defer self.freeDownload(dl);
                if (dl.local_path) |path| {
                    std.fs.cwd().deleteFile(path) catch {};
                }
            }
        }

        self.db.mutex.lock();
        defer self.db.mutex.unlock();

        var stmt = try self.db.prepare("DELETE FROM downloads WHERE id = ?");
        defer stmt.finalize();
        stmt.bindInt64(1, id);
        try stmt.exec();
    }

    /// Count active (downloading) downloads.
    pub fn activeCount(self: *Downloader) !u32 {
        var stmt = try self.db.prepare("SELECT COUNT(*) FROM downloads WHERE status = 'downloading'");
        defer stmt.finalize();
        if (stmt.step()) {
            return @intCast(stmt.columnInt(0));
        }
        return 0;
    }

    /// Get total bytes of completed downloads.
    pub fn totalCompletedBytes(self: *Downloader) !i64 {
        var stmt = try self.db.prepare(
            "SELECT COALESCE(SUM(total_bytes), 0) FROM downloads WHERE status = 'complete'",
        );
        defer stmt.finalize();
        if (stmt.step()) {
            return stmt.columnInt64(0);
        }
        return 0;
    }

    /// Check if disk has enough space for a download.
    pub fn checkDiskSpace(path: []const u8, required_bytes: u64) bool {
        // Use Linux statvfs to check available space
        const c = @cImport(@cInclude("sys/statvfs.h"));
        var stat: c.struct_statvfs = undefined;
        // Need a null-terminated path
        var path_buf: [4096]u8 = undefined;
        if (path.len >= path_buf.len) return true;
        @memcpy(path_buf[0..path.len], path);
        path_buf[path.len] = 0;

        if (c.statvfs(@ptrCast(&path_buf), &stat) != 0) return true;

        const available: u64 = @as(u64, stat.f_bavail) * @as(u64, stat.f_frsize);
        const needed = required_bytes + (required_bytes / 10); // 10% margin
        return available > needed;
    }

    pub fn freeDownload(self: *Downloader, dl: types.Download) void {
        self.allocator.free(dl.server_id);
        self.allocator.free(dl.source_url);
        if (dl.local_path) |p| self.allocator.free(p);
        if (dl.error_message) |m| self.allocator.free(m);
        if (dl.part_key) |k| self.allocator.free(k);
    }

    pub fn freeDownloads(self: *Downloader, downloads: []types.Download) void {
        for (downloads) |dl| self.freeDownload(dl);
        self.allocator.free(downloads);
    }

    fn readDownload(allocator: std.mem.Allocator, stmt: *database.Statement) !types.Download {
        return types.Download{
            .id = stmt.columnInt64(0),
            .media_item_id = stmt.columnInt64(1),
            .server_id = try allocator.dupe(u8, stmt.columnText(2) orelse ""),
            .source_url = try allocator.dupe(u8, stmt.columnText(3) orelse ""),
            .local_path = if (stmt.columnText(4)) |t| try allocator.dupe(u8, t) else null,
            .total_bytes = stmt.columnOptionalInt64(5),
            .downloaded_bytes = stmt.columnInt64(6),
            .status = types.DownloadStatus.fromString(stmt.columnText(7) orelse "queued") orelse .queued,
            .created_at = stmt.columnOptionalInt64(8),
            .completed_at = stmt.columnOptionalInt64(9),
            .error_message = if (stmt.columnText(10)) |t| try allocator.dupe(u8, t) else null,
            .part_key = if (stmt.columnText(11)) |t| try allocator.dupe(u8, t) else null,
        };
    }
};

test "downloader enqueue and retrieve" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    // Insert prerequisite rows for foreign keys
    try db.exec("INSERT INTO servers (id, name, client_identifier) VALUES ('server1', 'Test', 'uuid')");
    try db.exec("INSERT INTO media_items (id, source, media_type, title) VALUES (1, 'plex', 'movie', 'Test')");

    var dl = Downloader.init(std.testing.allocator, &db);

    const id = try dl.enqueue(.{
        .media_item_id = 1,
        .server_id = "server1",
        .source_url = "http://example.com/video.mkv",
        .download_dir = "/tmp/reel",
        .filename = "movie.mkv",
        .part_key = "/library/parts/123/file.mkv",
    });

    try std.testing.expect(id > 0);

    const queued = try dl.getByStatus(.queued);
    defer dl.freeDownloads(queued);
    try std.testing.expectEqual(@as(usize, 1), queued.len);
    try std.testing.expectEqualStrings("http://example.com/video.mkv", queued[0].source_url);
    try std.testing.expectEqualStrings("/library/parts/123/file.mkv", queued[0].part_key.?);
}

test "downloader duplicate prevention" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    try db.exec("INSERT INTO servers (id, name, client_identifier) VALUES ('s1', 'Test', 'uuid')");
    try db.exec("INSERT INTO media_items (id, source, media_type, title) VALUES (1, 'plex', 'movie', 'Test')");

    var dl = Downloader.init(std.testing.allocator, &db);

    _ = try dl.enqueue(.{
        .media_item_id = 1,
        .server_id = "s1",
        .source_url = "http://example.com/v.mkv",
        .download_dir = "/tmp",
        .filename = "v.mkv",
    });

    // Second enqueue of same media_item_id should fail
    const result = dl.enqueue(.{
        .media_item_id = 1,
        .server_id = "s1",
        .source_url = "http://example.com/v.mkv",
        .download_dir = "/tmp",
        .filename = "v.mkv",
    });
    try std.testing.expectError(error.AlreadyExists, result);
}

test "downloader re-enqueue after failure" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    try db.exec("INSERT INTO servers (id, name, client_identifier) VALUES ('s1', 'Test', 'uuid')");
    try db.exec("INSERT INTO media_items (id, source, media_type, title) VALUES (1, 'plex', 'movie', 'Test')");

    var dl = Downloader.init(std.testing.allocator, &db);

    const id1 = try dl.enqueue(.{
        .media_item_id = 1,
        .server_id = "s1",
        .source_url = "http://example.com/v.mkv",
        .download_dir = "/tmp",
        .filename = "v.mkv",
    });

    // Mark as failed
    try dl.setFailed(id1, "Connection lost");

    // Should be able to re-enqueue after failure
    const id2 = try dl.enqueue(.{
        .media_item_id = 1,
        .server_id = "s1",
        .source_url = "http://example.com/v.mkv",
        .download_dir = "/tmp",
        .filename = "v.mkv",
    });
    try std.testing.expect(id2 > 0);
    try std.testing.expect(id2 != id1);
}

test "downloader status updates" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    try db.exec("INSERT INTO servers (id, name, client_identifier) VALUES ('s1', 'Test', 'uuid')");
    try db.exec("INSERT INTO media_items (id, source, media_type, title) VALUES (1, 'plex', 'movie', 'Test')");

    var dl = Downloader.init(std.testing.allocator, &db);

    const id = try dl.enqueue(.{
        .media_item_id = 1,
        .server_id = "s1",
        .source_url = "http://example.com/v.mkv",
        .download_dir = "/tmp",
        .filename = "v.mkv",
    });

    try dl.setStatus(id, .downloading);
    try dl.updateProgress(id, 5000, 10000);

    const download = (try dl.getDownload(id)).?;
    defer dl.freeDownload(download);
    try std.testing.expectEqual(types.DownloadStatus.downloading, download.status);
    try std.testing.expectEqual(@as(i64, 5000), download.downloaded_bytes);
    try std.testing.expectEqual(@as(?i64, 10000), download.total_bytes);
}

test "downloader getByMediaItemId" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    try db.exec("INSERT INTO servers (id, name, client_identifier) VALUES ('s1', 'Test', 'uuid')");
    try db.exec("INSERT INTO media_items (id, source, media_type, title) VALUES (1, 'plex', 'movie', 'Test')");
    try db.exec("INSERT INTO media_items (id, source, media_type, title) VALUES (2, 'plex', 'movie', 'Test2')");

    var dl = Downloader.init(std.testing.allocator, &db);

    _ = try dl.enqueue(.{
        .media_item_id = 1,
        .server_id = "s1",
        .source_url = "http://example.com/v.mkv",
        .download_dir = "/tmp",
        .filename = "v.mkv",
    });

    // Should find download for item 1
    const found = (try dl.getByMediaItemId(1)).?;
    defer dl.freeDownload(found);
    try std.testing.expectEqual(@as(i64, 1), found.media_item_id);

    // Should not find download for item 2
    const not_found = try dl.getByMediaItemId(2);
    try std.testing.expect(not_found == null);
}

test "downloader resetInterrupted" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    try db.exec("INSERT INTO servers (id, name, client_identifier) VALUES ('s1', 'Test', 'uuid')");
    try db.exec("INSERT INTO media_items (id, source, media_type, title) VALUES (1, 'plex', 'movie', 'Test')");

    var dl = Downloader.init(std.testing.allocator, &db);

    const id = try dl.enqueue(.{
        .media_item_id = 1,
        .server_id = "s1",
        .source_url = "http://example.com/v.mkv",
        .download_dir = "/tmp",
        .filename = "v.mkv",
    });

    try dl.setStatus(id, .downloading);
    try dl.resetInterrupted();

    const download = (try dl.getDownload(id)).?;
    defer dl.freeDownload(download);
    try std.testing.expectEqual(types.DownloadStatus.queued, download.status);
}

test "downloader error message" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    try db.exec("INSERT INTO servers (id, name, client_identifier) VALUES ('s1', 'Test', 'uuid')");
    try db.exec("INSERT INTO media_items (id, source, media_type, title) VALUES (1, 'plex', 'movie', 'Test')");

    var dl = Downloader.init(std.testing.allocator, &db);

    const id = try dl.enqueue(.{
        .media_item_id = 1,
        .server_id = "s1",
        .source_url = "http://example.com/v.mkv",
        .download_dir = "/tmp",
        .filename = "v.mkv",
    });

    try dl.setFailed(id, "Connection refused");

    const download = (try dl.getDownload(id)).?;
    defer dl.freeDownload(download);
    try std.testing.expectEqual(types.DownloadStatus.failed, download.status);
    try std.testing.expectEqualStrings("Connection refused", download.error_message.?);
}
