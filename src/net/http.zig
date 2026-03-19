const std = @import("std");

fn defaultIo() std.Io {
    return std.Io.Threaded.global_single_threaded.io();
}

pub const HttpError = error{
    RequestFailed,
    InvalidUrl,
    ConnectionFailed,
    Timeout,
    TooManyRedirects,
    OutOfMemory,
    AuthenticationFailed,
    NotFound,
    WriteFailed,
    Cancelled,
};

pub const Header = struct {
    name: []const u8,
    value: []const u8,
};

pub const Response = struct {
    status: std.http.Status,
    body: []const u8,
    allocator: std.mem.Allocator,

    pub fn deinit(self: *Response) void {
        self.allocator.free(self.body);
    }
};

pub const HttpClient = struct {
    allocator: std.mem.Allocator,
    client: std.http.Client,

    pub fn init(allocator: std.mem.Allocator) HttpClient {
        return .{
            .allocator = allocator,
            .client = .{ .allocator = allocator, .io = defaultIo() },
        };
    }

    pub fn deinit(self: *HttpClient) void {
        self.client.deinit();
    }

    pub fn get(self: *HttpClient, url: []const u8, headers: []const Header) !Response {
        return self.doRequest(.GET, url, headers, null);
    }

    pub fn post(self: *HttpClient, url: []const u8, headers: []const Header, body: ?[]const u8) !Response {
        return self.doRequest(.POST, url, headers, body);
    }

    /// Download a URL to a file on disk with resume support and progress reporting.
    /// The progress callback returns true to continue, false to cancel.
    /// The cancel_flag pointer is checked atomically each chunk; set to true to abort.
    pub fn downloadToFile(
        self: *HttpClient,
        url: []const u8,
        file_path: []const u8,
        resume_from: u64,
        extra_headers: []const Header,
        progress_cb: ?*const fn (downloaded: u64, total: u64) bool,
        cancel_flag: ?*bool,
    ) HttpError!void {
        const uri = std.Uri.parse(url) catch return error.InvalidUrl;

        // Build headers
        const sio = defaultIo();
        var headers_list: std.ArrayList(std.http.Header) = .empty;
        defer headers_list.deinit(self.allocator);

        for (extra_headers) |h| {
            headers_list.append(self.allocator, .{ .name = h.name, .value = h.value }) catch return error.OutOfMemory;
        }

        // Add Range header for resume
        var range_buf: [64]u8 = undefined;
        if (resume_from > 0) {
            const range_val = std.fmt.bufPrint(&range_buf, "bytes={d}-", .{resume_from}) catch return error.OutOfMemory;
            headers_list.append(self.allocator, .{ .name = "Range", .value = range_val }) catch return error.OutOfMemory;
        }

        var req = self.client.request(.GET, uri, .{
            .extra_headers = headers_list.items,
        }) catch return error.ConnectionFailed;
        defer req.deinit();

        req.sendBodiless() catch return error.RequestFailed;

        var redirect_buf: [8 * 1024]u8 = undefined;
        var response = req.receiveHead(&redirect_buf) catch return error.RequestFailed;

        // Check status
        switch (response.head.status) {
            .ok, .partial_content => {},
            .unauthorized => return error.AuthenticationFailed,
            .not_found => return error.NotFound,
            else => return error.RequestFailed,
        }

        // Determine total size
        var total_size: u64 = 0;
        if (response.head.content_length) |cl| {
            total_size = resume_from + cl;
        }

        // Open file for writing
        const cwd = std.Io.Dir.cwd();
        const file = if (resume_from > 0)
            cwd.openFile(sio, file_path, .{ .mode = .write_only }) catch
                cwd.createFile(sio, file_path, .{}) catch return error.WriteFailed
        else
            cwd.createFile(sio, file_path, .{}) catch return error.WriteFailed;
        defer file.close(sio);

        // Get response body reader
        var transfer_buf: [64]u8 = undefined;
        const reader = response.reader(&transfer_buf);

        // Read and write in chunks using readVec
        var downloaded: u64 = resume_from;
        var bytes_since_callback: u64 = 0;

        while (true) {
            // Check cancel flag
            if (cancel_flag) |flag| {
                if (@atomicLoad(bool, flag, .monotonic)) return error.Cancelled;
            }

            var buf: [8192]u8 = undefined;
            var bufs = [_][]u8{buf[0..]};
            const n = reader.readVec(&bufs) catch return error.RequestFailed;
            if (n == 0) break;

            file.writePositionalAll(sio, buf[0..n], downloaded) catch return error.WriteFailed;
            downloaded += n;
            bytes_since_callback += n;

            // Report progress every ~100KB
            if (bytes_since_callback >= 102400) {
                bytes_since_callback = 0;
                if (progress_cb) |cb| {
                    if (!cb(downloaded, total_size)) {
                        return error.Cancelled;
                    }
                }
            }
        }

        // Final progress report
        if (progress_cb) |cb| {
            _ = cb(downloaded, total_size);
        }
    }

    fn doRequest(
        self: *HttpClient,
        method: std.http.Method,
        url: []const u8,
        extra_headers: []const Header,
        body: ?[]const u8,
    ) !Response {
        const uri = std.Uri.parse(url) catch return error.InvalidUrl;

        // Build extra headers
        var extra: std.ArrayList(std.http.Header) = .empty;
        defer extra.deinit(self.allocator);

        for (extra_headers) |h| {
            try extra.append(self.allocator, .{ .name = h.name, .value = h.value });
        }

        // Fresh client per request to avoid any shared TLS/connection state
        var client: std.http.Client = .{ .allocator = self.allocator, .io = defaultIo() };
        defer client.deinit();

        var req = client.request(method, uri, .{
            .extra_headers = extra.items,
        }) catch |err| {
            std.log.err("http: {s} {s} connection failed: {}", .{ @tagName(method), url, err });
            return error.ConnectionFailed;
        };
        defer req.deinit();

        if (body) |payload| {
            req.transfer_encoding = .{ .content_length = payload.len };
            var req_body = req.sendBodyUnflushed(&.{}) catch return error.RequestFailed;
            req_body.writer.writeAll(payload) catch return error.RequestFailed;
            req_body.end() catch return error.RequestFailed;
            if (req.connection) |conn| conn.flush() catch return error.RequestFailed;
        } else if (method.requestHasBody()) {
            // POST/PUT/PATCH with no body: send empty body (content-length: 0)
            req.transfer_encoding = .{ .content_length = 0 };
            var req_body = req.sendBodyUnflushed(&.{}) catch return error.RequestFailed;
            req_body.end() catch return error.RequestFailed;
            if (req.connection) |conn| conn.flush() catch return error.RequestFailed;
        } else {
            req.sendBodiless() catch return error.RequestFailed;
        }

        // Heap-allocate redirect buffer to reduce stack pressure (called from Swift Task with limited stack)
        const redirect_buf = self.allocator.alloc(u8, 8 * 1024) catch return error.OutOfMemory;
        defer self.allocator.free(redirect_buf);
        var response = req.receiveHead(redirect_buf) catch return error.RequestFailed;

        // Read and decompress body (handles gzip/deflate/zstd automatically)
        const decompress_buf = switch (response.head.content_encoding) {
            .identity => @as([]u8, &.{}),
            .deflate, .gzip => self.allocator.alloc(u8, std.compress.flate.max_window_len) catch return error.OutOfMemory,
            .zstd => self.allocator.alloc(u8, std.compress.zstd.default_window_len) catch return error.OutOfMemory,
            else => return error.RequestFailed,
        };
        defer if (response.head.content_encoding != .identity) self.allocator.free(decompress_buf);

        var transfer_buf: [64]u8 = undefined;
        var decompress: std.http.Decompress = undefined;
        const reader = response.readerDecompressing(&transfer_buf, &decompress, decompress_buf);

        const response_body = reader.allocRemaining(self.allocator, .unlimited) catch return error.RequestFailed;

        return Response{
            .status = response.head.status,
            .body = response_body,
            .allocator = self.allocator,
        };
    }
};

test "HttpClient init and deinit" {
    var client = HttpClient.init(std.testing.allocator);
    defer client.deinit();
}
