const std = @import("std");
const database = @import("database.zig");
const types = @import("types.zig");
const http = @import("../net/http.zig");

fn defaultIo() std.Io {
    return std.Io.Threaded.global_single_threaded.io();
}

fn unixTimestamp() i64 {
    var ts: std.c.timespec = undefined;
    _ = std.c.clock_gettime(.REALTIME, &ts);
    return @intCast(ts.sec);
}

pub const max_concurrent: usize = 2;
const max_retries: u32 = 3;
const retry_delays_ms = [_]u64{ 5_000, 30_000, 120_000 };

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

/// Result struct for the existing-download check query.
const ExistingDownloadRow = struct {
    id: i64,
    status: types.DownloadStatus,
};

/// Result struct for single-column count queries.
const CountRow = struct {
    count: i64,
};

/// Result struct for single-column sum queries.
const SumRow = struct {
    total: i64,
};

/// Result struct for single-column local_path queries.
const LocalPathRow = struct {
    local_path: ?[]const u8,
};

pub const Downloader = struct {
    db: *database.Database,
    allocator: std.mem.Allocator,
    // Worker thread state
    http_client: ?*http.HttpClient = null,
    worker_thread: ?std.Thread = null,
    running: bool = false,
    cancel_current: bool = false,

    pub fn init(allocator: std.mem.Allocator, db: *database.Database) Downloader {
        return .{ .db = db, .allocator = allocator };
    }

    /// Start the background download worker thread.
    pub fn startWorker(self: *Downloader, http_client: *http.HttpClient) !void {
        if (self.worker_thread != null) return;

        self.http_client = http_client;
        @atomicStore(bool, &self.running, true, .monotonic);
        @atomicStore(bool, &self.cancel_current, false, .monotonic);

        // Reset any downloads that were interrupted by last shutdown
        try self.resetInterrupted();

        self.worker_thread = try std.Thread.spawn(.{}, workerLoop, .{self});
    }

    /// Stop the background download worker thread.
    pub fn stopWorker(self: *Downloader) void {
        @atomicStore(bool, &self.running, false, .monotonic);
        @atomicStore(bool, &self.cancel_current, true, .monotonic);

        if (self.worker_thread) |thread| {
            thread.join();
            self.worker_thread = null;
        }
    }

    /// Pause a specific download.
    pub fn pause(self: *Downloader, id: i64) !void {
        const dl = try self.getDownload(id) orelse return;
        defer self.freeDownload(dl);

        if (dl.status == .downloading) {
            // Signal the worker to cancel the current download
            @atomicStore(bool, &self.cancel_current, true, .monotonic);
        }
        try self.setStatus(id, .paused);
    }

    /// Resume a paused download by setting it back to queued.
    pub fn resumeDownload(self: *Downloader, id: i64) !void {
        try self.setStatus(id, .queued);
    }

    fn workerLoop(self: *Downloader) void {
        while (@atomicLoad(bool, &self.running, .monotonic)) {
            self.processQueue() catch |err| {
                std.log.err("Download worker error: {}", .{err});
            };

            // Sleep 2 seconds between queue checks
            std.Thread.sleep(2 * std.time.ns_per_s);
        }
    }

    fn processQueue(self: *Downloader) !void {
        // Check if we can start more downloads
        const active = try self.activeCount();
        if (active >= max_concurrent) return;

        // Get next queued download
        const queued = try self.getByStatus(.queued);
        defer self.freeDownloads(queued);

        if (queued.len == 0) return;

        const dl = queued[0];
        self.processDownload(dl) catch |err| {
            std.log.err("Failed to process download {d}: {}", .{ dl.id, err });
        };
    }

    fn processDownload(self: *Downloader, dl: types.Download) !void {
        const client = self.http_client orelse return;

        // Check disk space before starting
        if (dl.local_path) |path| {
            if (dl.total_bytes) |total| {
                if (!checkDiskSpace(path, @intCast(total))) {
                    try self.setFailed(dl.id, "Insufficient disk space");
                    return;
                }
            }
        }

        // Ensure download directory exists
        if (dl.local_path) |path| {
            if (std.mem.lastIndexOfScalar(u8, path, '/')) |dir_end| {
                std.Io.Dir.cwd().createDirPath(defaultIo(), path[0..dir_end]) catch {};
            }
        }

        try self.setStatus(dl.id, .downloading);
        @atomicStore(bool, &self.cancel_current, false, .monotonic);

        const resume_from: u64 = if (dl.downloaded_bytes > 0) @intCast(dl.downloaded_bytes) else 0;

        // Build the download URL (use source_url directly)
        const url = dl.source_url;
        const file_path = dl.local_path orelse {
            try self.setFailed(dl.id, "No local path configured");
            return;
        };

        // Store context for the progress callback via threadlocal
        tl_downloader = self;
        tl_download_id = dl.id;

        // Retry loop
        var retries: u32 = 0;
        while (retries <= max_retries) : (retries += 1) {
            if (!@atomicLoad(bool, &self.running, .monotonic)) return;

            const current_offset: u64 = blk: {
                if (retries > 0) {
                    // Re-read the download to get updated bytes
                    const updated = try self.getDownload(dl.id) orelse return;
                    defer self.freeDownload(updated);
                    break :blk if (updated.downloaded_bytes > 0) @intCast(updated.downloaded_bytes) else 0;
                }
                break :blk resume_from;
            };

            client.downloadToFile(
                url,
                file_path,
                current_offset,
                &.{},
                &progressCallback,
                &self.cancel_current,
            ) catch |err| {
                switch (err) {
                    error.Cancelled => {
                        // Check if this was a pause or a stop
                        if (!@atomicLoad(bool, &self.running, .monotonic)) return;
                        // Paused — update progress and leave as paused
                        return;
                    },
                    error.AuthenticationFailed => {
                        try self.setFailed(dl.id, "Authentication expired");
                        return;
                    },
                    error.NotFound => {
                        try self.setFailed(dl.id, "File not found on server");
                        return;
                    },
                    error.WriteFailed => {
                        try self.setFailed(dl.id, "Disk write failed (disk full?)");
                        return;
                    },
                    else => {
                        // Transient error — retry with backoff
                        if (retries < max_retries) {
                            const delay_idx = @min(retries, retry_delays_ms.len - 1);
                            std.Thread.sleep(retry_delays_ms[delay_idx] * std.time.ns_per_ms);
                            continue;
                        }
                        try self.setFailed(dl.id, "Download failed after retries");
                        return;
                    },
                }
            };

            // Download completed successfully — verify file and update final size
            if (dl.local_path) |path| {
                const file_stat = std.Io.Dir.cwd().statFile(defaultIo(), path, .{}) catch {
                    try self.setFailed(dl.id, "Downloaded file not found");
                    return;
                };
                try self.updateProgress(dl.id, @intCast(file_stat.size), @intCast(file_stat.size));
            }
            try self.setStatus(dl.id, .complete);
            return;
        }
    }

    /// Queue a new download. Returns error.AlreadyExists if a non-failed download exists.
    pub fn enqueue(self: *Downloader, req: DownloadRequest) !i64 {
        // Check for existing download of same media item
        const existing_failed_id: ?i64 = blk: {
            const row = self.db.db.oneAlloc(
                ExistingDownloadRow,
                self.allocator,
                "SELECT id, status FROM downloads WHERE media_item_id = ?{i64} LIMIT 1",
                .{},
                .{req.media_item_id},
            ) catch return error.SqlExecFailed;

            if (row) |existing| {
                switch (existing.status) {
                    .complete, .downloading, .queued, .paused => return error.AlreadyExists,
                    .failed => break :blk existing.id,
                }
            }
            break :blk null;
        };

        // Remove failed entry
        if (existing_failed_id) |failed_id| {
            self.db.db.exec(
                "DELETE FROM downloads WHERE id = ?{i64}",
                .{},
                .{failed_id},
            ) catch return error.SqlExecFailed;
        }

        const local_path = try std.fs.path.join(self.allocator, &.{ req.download_dir, req.filename });
        defer self.allocator.free(local_path);

        const now = unixTimestamp();

        self.db.db.exec(
            \\INSERT INTO downloads
            \\  (media_item_id, server_id, source_url, local_path, status, created_at, part_key)
            \\VALUES (?{i64}, ?{[]const u8}, ?{[]const u8}, ?{[]const u8}, 'queued', ?{i64}, ?{?[]const u8})
        , .{}, .{
            req.media_item_id,
            req.server_id,
            req.source_url,
            local_path,
            now,
            req.part_key,
        }) catch return error.SqlExecFailed;

        return self.db.db.getLastInsertRowID();
    }

    /// Get all downloads with given status.
    pub fn getByStatus(self: *Downloader, status: types.DownloadStatus) ![]types.Download {
        var stmt = try self.db.db.prepare(
            \\SELECT id, media_item_id, server_id, source_url, local_path,
            \\       total_bytes, downloaded_bytes, status, created_at, completed_at,
            \\       error_message, part_key
            \\ FROM downloads WHERE status = ?{[]const u8}
            \\ ORDER BY created_at ASC
        );
        defer stmt.deinit();
        return stmt.all(types.Download, self.allocator, .{}, .{status.toString()});
    }

    /// Get all downloads (all statuses).
    pub fn getAllDownloads(self: *Downloader) ![]types.Download {
        var stmt = try self.db.db.prepare(
            \\SELECT id, media_item_id, server_id, source_url, local_path,
            \\       total_bytes, downloaded_bytes, status, created_at, completed_at,
            \\       error_message, part_key
            \\ FROM downloads ORDER BY created_at ASC
        );
        defer stmt.deinit();
        return stmt.all(types.Download, self.allocator, .{}, .{});
    }

    /// Get a single download by ID.
    pub fn getDownload(self: *Downloader, id: i64) !?types.Download {
        return self.db.db.oneAlloc(
            types.Download,
            self.allocator,
            \\SELECT id, media_item_id, server_id, source_url, local_path,
            \\       total_bytes, downloaded_bytes, status, created_at, completed_at,
            \\       error_message, part_key
            \\ FROM downloads WHERE id = ?{i64}
        ,
            .{},
            .{id},
        );
    }

    /// Get download for a specific media item (if any).
    pub fn getByMediaItemId(self: *Downloader, media_item_id: i64) !?types.Download {
        return self.db.db.oneAlloc(
            types.Download,
            self.allocator,
            \\SELECT id, media_item_id, server_id, source_url, local_path,
            \\       total_bytes, downloaded_bytes, status, created_at, completed_at,
            \\       error_message, part_key
            \\ FROM downloads WHERE media_item_id = ?{i64} LIMIT 1
        ,
            .{},
            .{media_item_id},
        );
    }

    /// Get the local file path for a completed download of a media item.
    pub fn getCompletedLocalPath(self: *Downloader, media_item_id: i64) !?[]const u8 {
        const row = try self.db.db.oneAlloc(
            LocalPathRow,
            self.allocator,
            "SELECT local_path FROM downloads WHERE media_item_id = ?{i64} AND status = 'complete' LIMIT 1",
            .{},
            .{media_item_id},
        );
        if (row) |r| {
            return r.local_path;
        }
        return null;
    }

    /// Update download progress.
    pub fn updateProgress(self: *Downloader, id: i64, downloaded: i64, total: ?i64) !void {
        self.db.db.exec(
            "UPDATE downloads SET downloaded_bytes = ?{i64}, total_bytes = ?{?i64} WHERE id = ?{i64}",
            .{},
            .{ downloaded, total, id },
        ) catch return error.SqlExecFailed;
    }

    /// Update download status.
    pub fn setStatus(self: *Downloader, id: i64, status: types.DownloadStatus) !void {
        self.db.db.exec(
            "UPDATE downloads SET status = ?{[]const u8} WHERE id = ?{i64}",
            .{},
            .{ status.toString(), id },
        ) catch return error.SqlExecFailed;

        if (status == .complete) {
            self.db.db.exec(
                "UPDATE downloads SET completed_at = ?{i64} WHERE id = ?{i64}",
                .{},
                .{ unixTimestamp(), id },
            ) catch return error.SqlExecFailed;
        }
    }

    /// Set status to failed with an error message.
    pub fn setFailed(self: *Downloader, id: i64, error_message: []const u8) !void {
        self.db.db.exec(
            "UPDATE downloads SET status = 'failed', error_message = ?{[]const u8} WHERE id = ?{i64}",
            .{},
            .{ error_message, id },
        ) catch return error.SqlExecFailed;
    }

    /// Reset any 'downloading' entries to 'queued' (for crash recovery on restart).
    pub fn resetInterrupted(self: *Downloader) !void {
        self.db.db.exec(
            "UPDATE downloads SET status = 'queued' WHERE status = 'downloading'",
            .{},
            .{},
        ) catch return error.SqlExecFailed;
    }

    /// Remove a download (and optionally its file).
    pub fn remove(self: *Downloader, id: i64, delete_file: bool) !void {
        if (delete_file) {
            if (try self.getDownload(id)) |dl| {
                defer self.freeDownload(dl);
                if (dl.local_path) |path| {
                    std.Io.Dir.cwd().deleteFile(defaultIo(), path) catch {};
                }
            }
        }

        self.db.db.exec(
            "DELETE FROM downloads WHERE id = ?{i64}",
            .{},
            .{id},
        ) catch return error.SqlExecFailed;
    }

    /// Count active (downloading) downloads.
    pub fn activeCount(self: *Downloader) !u32 {
        const row = self.db.db.one(
            CountRow,
            "SELECT COUNT(*) FROM downloads WHERE status = 'downloading'",
            .{},
            .{},
        ) catch return error.SqlExecFailed;
        if (row) |r| return @intCast(r.count);
        return 0;
    }

    /// Get total bytes of completed downloads.
    pub fn totalCompletedBytes(self: *Downloader) !i64 {
        const row = self.db.db.one(
            SumRow,
            "SELECT COALESCE(SUM(total_bytes), 0) FROM downloads WHERE status = 'complete'",
            .{},
            .{},
        ) catch return error.SqlExecFailed;
        if (row) |r| return r.total;
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
};

// Thread-local state for progress callback (needed because fn pointers can't capture context)
threadlocal var tl_downloader: ?*Downloader = null;
threadlocal var tl_download_id: i64 = 0;

fn progressCallback(downloaded: u64, total: u64) bool {
    const dl = tl_downloader orelse return true;
    const id = tl_download_id;

    // Update progress in database
    const total_i: ?i64 = if (total > 0) @intCast(total) else null;
    dl.updateProgress(id, @intCast(downloaded), total_i) catch {};

    return true;
}

test "downloader enqueue and retrieve" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    // Insert prerequisite rows for foreign keys
    try db.db.execMulti("INSERT INTO servers (id, name, client_identifier) VALUES ('server1', 'Test', 'uuid')", .{});
    try db.db.execMulti("INSERT INTO media_items (id, source, media_type, title) VALUES (1, 'plex', 'movie', 'Test')", .{});

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

    try db.db.execMulti("INSERT INTO servers (id, name, client_identifier) VALUES ('s1', 'Test', 'uuid')", .{});
    try db.db.execMulti("INSERT INTO media_items (id, source, media_type, title) VALUES (1, 'plex', 'movie', 'Test')", .{});

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

    try db.db.execMulti("INSERT INTO servers (id, name, client_identifier) VALUES ('s1', 'Test', 'uuid')", .{});
    try db.db.execMulti("INSERT INTO media_items (id, source, media_type, title) VALUES (1, 'plex', 'movie', 'Test')", .{});

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

    try db.db.execMulti("INSERT INTO servers (id, name, client_identifier) VALUES ('s1', 'Test', 'uuid')", .{});
    try db.db.execMulti("INSERT INTO media_items (id, source, media_type, title) VALUES (1, 'plex', 'movie', 'Test')", .{});

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

    try db.db.execMulti("INSERT INTO servers (id, name, client_identifier) VALUES ('s1', 'Test', 'uuid')", .{});
    try db.db.execMulti("INSERT INTO media_items (id, source, media_type, title) VALUES (1, 'plex', 'movie', 'Test')", .{});
    try db.db.execMulti("INSERT INTO media_items (id, source, media_type, title) VALUES (2, 'plex', 'movie', 'Test2')", .{});

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

    try db.db.execMulti("INSERT INTO servers (id, name, client_identifier) VALUES ('s1', 'Test', 'uuid')", .{});
    try db.db.execMulti("INSERT INTO media_items (id, source, media_type, title) VALUES (1, 'plex', 'movie', 'Test')", .{});

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

    try db.db.execMulti("INSERT INTO servers (id, name, client_identifier) VALUES ('s1', 'Test', 'uuid')", .{});
    try db.db.execMulti("INSERT INTO media_items (id, source, media_type, title) VALUES (1, 'plex', 'movie', 'Test')", .{});

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
