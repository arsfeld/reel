const std = @import("std");

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
            .client = .{ .allocator = allocator },
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
        var headers_list: std.ArrayList(std.http.Header) = .{};
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
        const file = if (resume_from > 0)
            std.fs.cwd().openFile(file_path, .{ .mode = .write_only }) catch
                std.fs.cwd().createFile(file_path, .{}) catch return error.WriteFailed
        else
            std.fs.cwd().createFile(file_path, .{}) catch return error.WriteFailed;
        defer file.close();

        // Seek to end if resuming
        if (resume_from > 0) {
            file.seekTo(resume_from) catch return error.WriteFailed;
        }

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

            file.writeAll(buf[0..n]) catch return error.WriteFailed;
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
        // Build extra headers
        var extra: std.ArrayList(std.http.Header) = .{};
        defer extra.deinit(self.allocator);

        for (extra_headers) |h| {
            try extra.append(self.allocator, .{ .name = h.name, .value = h.value });
        }

        // Use the fetch API for simple request/response
        var alloc_writer = std.Io.Writer.Allocating.init(self.allocator);
        defer alloc_writer.deinit();

        const result = self.client.fetch(.{
            .location = .{ .url = url },
            .method = method,
            .payload = body orelse if (method.requestHasBody()) "" else null,
            .extra_headers = extra.items,
            .response_writer = &alloc_writer.writer,
        }) catch |err| {
            std.log.err("HttpClient.doRequest: fetch failed for {s}: {}", .{ url, err });
            return error.RequestFailed;
        };

        const response_body = alloc_writer.toOwnedSlice() catch return error.OutOfMemory;

        return Response{
            .status = result.status,
            .body = response_body,
            .allocator = self.allocator,
        };
    }
};

test "HttpClient init and deinit" {
    var client = HttpClient.init(std.testing.allocator);
    defer client.deinit();
}
