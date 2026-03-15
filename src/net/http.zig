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
            .client = std.http.Client{ .allocator = allocator },
        };
    }

    pub fn deinit(self: *HttpClient) void {
        self.client.deinit();
    }

    pub fn get(self: *HttpClient, url: []const u8, headers: []const Header) !Response {
        return self.request(.GET, url, headers, null);
    }

    pub fn post(self: *HttpClient, url: []const u8, headers: []const Header, body: ?[]const u8) !Response {
        return self.request(.POST, url, headers, body);
    }

    /// Download a URL to a file on disk with resume support and progress reporting.
    /// The progress callback returns true to continue, false to cancel.
    pub fn downloadToFile(
        self: *HttpClient,
        url: []const u8,
        file_path: []const u8,
        resume_from: u64,
        extra_headers: []const Header,
        progress_cb: ?*const fn (downloaded: u64, total: u64) bool,
    ) HttpError!void {
        const uri = std.Uri.parse(url) catch return error.InvalidUrl;

        var server_header_buffer: [16 * 1024]u8 = undefined;

        // Build headers including Range for resume
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

        var req = self.client.open(.GET, uri, .{
            .server_header_buffer = &server_header_buffer,
            .extra_headers = headers_list.items,
        }) catch return error.ConnectionFailed;
        defer req.deinit();

        req.send() catch return error.RequestFailed;
        req.finish() catch return error.RequestFailed;
        req.wait() catch return error.RequestFailed;

        // Check status
        switch (req.status) {
            .ok, .partial_content => {},
            .unauthorized => return error.AuthenticationFailed,
            .not_found => return error.NotFound,
            else => return error.RequestFailed,
        }

        // Determine total size from Content-Length or Content-Range
        var total_size: u64 = 0;
        const content_length = req.response.content_length;
        if (content_length) |cl| {
            total_size = resume_from + cl;
        }

        // Open file for writing (append if resuming, create if new)
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

        // Read and write in chunks
        var downloaded: u64 = resume_from;
        var buf: [8192]u8 = undefined;
        var reader = req.reader();
        var bytes_since_callback: u64 = 0;

        while (true) {
            const n = reader.read(&buf) catch return error.RequestFailed;
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

    fn request(
        self: *HttpClient,
        method: std.http.Method,
        url: []const u8,
        extra_headers: []const Header,
        body: ?[]const u8,
    ) !Response {
        const uri = std.Uri.parse(url) catch return error.InvalidUrl;

        var server_header_buffer: [16 * 1024]u8 = undefined;

        // Build extra headers
        var extra: std.ArrayList(std.http.Header) = .{};
        defer extra.deinit(self.allocator);

        for (extra_headers) |h| {
            try extra.append(self.allocator, .{ .name = h.name, .value = h.value });
        }

        var req = self.client.open(method, uri, .{
            .server_header_buffer = &server_header_buffer,
            .extra_headers = extra.items,
        }) catch return error.ConnectionFailed;
        defer req.deinit();

        if (body) |b| {
            req.transfer_encoding = .{ .content_length = b.len };
        }

        req.send() catch return error.RequestFailed;

        if (body) |b| {
            req.writer().writeAll(b) catch return error.RequestFailed;
        }

        req.finish() catch return error.RequestFailed;
        req.wait() catch return error.RequestFailed;

        const response_body = req.reader().readAllAlloc(self.allocator, 10 * 1024 * 1024) catch return error.OutOfMemory;

        return Response{
            .status = req.status,
            .body = response_body,
            .allocator = self.allocator,
        };
    }
};

test "HttpClient init and deinit" {
    var client = HttpClient.init(std.testing.allocator);
    defer client.deinit();
}
