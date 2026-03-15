const std = @import("std");

pub const HttpError = error{
    RequestFailed,
    InvalidUrl,
    ConnectionFailed,
    Timeout,
    TooManyRedirects,
    OutOfMemory,
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
