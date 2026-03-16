const std = @import("std");
const http = @import("../http.zig");
const plex_types = @import("types.zig");
const xml = @import("xml.zig");

pub const AuthError = error{
    PinRequestFailed,
    PinExpired,
    PinPollFailed,
    BrowserOpenFailed,
    InvalidResponse,
    OutOfMemory,
    RequestFailed,
    InvalidUrl,
    ConnectionFailed,
    Timeout,
    TooManyRedirects,
};

pub const AuthState = enum {
    idle,
    awaiting_pin, // PIN displayed, waiting for user to enter at plex.tv/link
    authenticated,
    failed,
};

pub const PlexAuth = struct {
    client: *http.HttpClient,
    headers: plex_types.PlexHeaders,
    allocator: std.mem.Allocator,
    state: AuthState = .idle,
    pin_id: ?i64 = null,
    pin_code: ?[]const u8 = null,
    auth_token: ?[]const u8 = null,

    pub fn init(allocator: std.mem.Allocator, client: *http.HttpClient, client_identifier: []const u8) PlexAuth {
        return .{
            .client = client,
            .allocator = allocator,
            .headers = .{ .client_identifier = client_identifier },
        };
    }

    pub fn deinit(self: *PlexAuth) void {
        if (self.pin_code) |code| self.allocator.free(code);
        if (self.auth_token) |token| self.allocator.free(token);
    }

    /// Request a new PIN from plex.tv.
    /// Returns the 4-character code the user should enter at plex.tv/link.
    pub fn requestPin(self: *PlexAuth) ![]const u8 {
        var header_buf: [8]plex_types.Header = undefined;
        const headers = self.headers.toHeaders(&header_buf);

        var response = try self.client.post(
            "https://plex.tv/api/v2/pins?strong=true",
            headers,
            null,
        );
        defer response.deinit();

        if (response.status != .ok and response.status != .created) {
            std.log.err("PIN request HTTP status: {d} body: {s}", .{ @intFromEnum(response.status), response.body[0..@min(response.body.len, 500)] });
            return error.PinRequestFailed;
        }

        // Parse XML response for id and code
        var parser = xml.XmlParser.init(response.body);
        while (parser.next()) |elem| {
            if (std.mem.eql(u8, elem.tag, "pin")) {
                const id = elem.attrInt64("id") orelse continue;
                const code = elem.attr("code") orelse continue;

                self.pin_id = id;
                if (self.pin_code) |old| self.allocator.free(old);
                self.pin_code = try self.allocator.dupe(u8, code);
                self.state = .awaiting_pin;

                return self.pin_code.?;
            }
        }

        return error.InvalidResponse;
    }

    /// Open the user's browser to plex.tv/link.
    pub fn openBrowser(self: *PlexAuth) !void {
        _ = self;
        var child = std.process.Child.init(
            &.{ "xdg-open", "https://plex.tv/link" },
            std.heap.c_allocator,
        );
        child.spawn() catch return error.BrowserOpenFailed;
    }

    /// Poll plex.tv to check if the user has entered the PIN.
    /// Returns the auth token if authenticated, null if still waiting.
    pub fn pollPin(self: *PlexAuth) !?[]const u8 {
        const pin_id = self.pin_id orelse return error.PinPollFailed;

        var url_buf: [128]u8 = undefined;
        const url = std.fmt.bufPrint(&url_buf, "https://plex.tv/api/v2/pins/{d}", .{pin_id}) catch
            return error.PinPollFailed;
        const url_z = std.heap.c_allocator.dupeZ(u8, url) catch return error.OutOfMemory;
        defer std.heap.c_allocator.free(url_z);

        var header_buf: [8]plex_types.Header = undefined;
        const headers = self.headers.toHeaders(&header_buf);

        var response = try self.client.get(url, headers);
        defer response.deinit();

        if (response.status != .ok) return error.PinPollFailed;

        // Check if authToken is populated
        var parser = xml.XmlParser.init(response.body);
        while (parser.next()) |elem| {
            if (std.mem.eql(u8, elem.tag, "pin")) {
                if (elem.attr("authToken")) |token| {
                    if (token.len > 0) {
                        if (self.auth_token) |old| self.allocator.free(old);
                        self.auth_token = try self.allocator.dupe(u8, token);
                        self.state = .authenticated;
                        return self.auth_token.?;
                    }
                }
                // Not yet authenticated
                return null;
            }
        }

        return error.InvalidResponse;
    }

    /// Generate a UUID v4 for use as X-Plex-Client-Identifier.
    pub fn generateClientId(allocator: std.mem.Allocator) ![]const u8 {
        var bytes: [16]u8 = undefined;
        std.crypto.random.bytes(&bytes);

        // Set version 4
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        // Set variant
        bytes[8] = (bytes[8] & 0x3f) | 0x80;

        return std.fmt.allocPrint(allocator, "{x:0>2}{x:0>2}{x:0>2}{x:0>2}-{x:0>2}{x:0>2}-{x:0>2}{x:0>2}-{x:0>2}{x:0>2}-{x:0>2}{x:0>2}{x:0>2}{x:0>2}{x:0>2}{x:0>2}", .{
            bytes[0],  bytes[1],  bytes[2],  bytes[3],
            bytes[4],  bytes[5],
            bytes[6],  bytes[7],
            bytes[8],  bytes[9],
            bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        });
    }
};

test "generateClientId format" {
    const id = try PlexAuth.generateClientId(std.testing.allocator);
    defer std.testing.allocator.free(id);

    // UUID v4 format: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx (36 chars)
    try std.testing.expectEqual(@as(usize, 36), id.len);
    try std.testing.expectEqual(@as(u8, '-'), id[8]);
    try std.testing.expectEqual(@as(u8, '-'), id[13]);
    try std.testing.expectEqual(@as(u8, '-'), id[18]);
    try std.testing.expectEqual(@as(u8, '-'), id[23]);
}
