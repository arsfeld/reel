const std = @import("std");

pub const PlexHeaders = struct {
    pub const product = "Reel";
    pub const platform = switch (@import("builtin").os.tag) {
        .macos => "macOS",
        else => "Linux",
    };
    pub const device = "Desktop";
    pub const version = "0.1.0";

    client_identifier: []const u8,
    auth_token: ?[]const u8 = null,

    pub fn toHeaders(self: *const PlexHeaders, buf: []Header) []const Header {
        var i: usize = 0;
        buf[i] = .{ .name = "X-Plex-Product", .value = product };
        i += 1;
        buf[i] = .{ .name = "X-Plex-Platform", .value = platform };
        i += 1;
        buf[i] = .{ .name = "X-Plex-Device", .value = device };
        i += 1;
        buf[i] = .{ .name = "X-Plex-Version", .value = version };
        i += 1;
        buf[i] = .{ .name = "X-Plex-Client-Identifier", .value = self.client_identifier };
        i += 1;
        buf[i] = .{ .name = "Accept", .value = "application/json" };
        i += 1;
        if (self.auth_token) |token| {
            buf[i] = .{ .name = "X-Plex-Token", .value = token };
            i += 1;
        }
        return buf[0..i];
    }
};

pub const Header = @import("../http.zig").Header;

pub const PinResponse = struct {
    id: i64,
    code: []const u8,
    auth_token: ?[]const u8 = null,
    expires_at: ?[]const u8 = null,
};

pub const PlexServer = struct {
    name: []const u8,
    machine_identifier: []const u8,
    access_token: ?[]const u8 = null,
    connections: []Connection = &.{},

    pub const Connection = struct {
        uri: []const u8,
        local: bool = false,
        relay: bool = false,
        protocol: []const u8 = "https",
    };
};

pub const PlexLibrary = struct {
    key: []const u8,
    title: []const u8,
    library_type: []const u8, // "movie", "show", "artist", etc.
};

pub const PlexMediaItem = struct {
    rating_key: []const u8,
    title: []const u8,
    media_type: []const u8, // "movie", "episode", "show", "season"
    summary: ?[]const u8 = null,
    year: ?i32 = null,
    rating: ?f64 = null,
    duration_ms: ?i64 = null,
    thumb: ?[]const u8 = null,
    art: ?[]const u8 = null,
    parent_rating_key: ?[]const u8 = null,
    grandparent_rating_key: ?[]const u8 = null,
    grandparent_title: ?[]const u8 = null,
    parent_index: ?i32 = null,
    index: ?i32 = null,
    view_offset: ?i64 = null,
    part_key: ?[]const u8 = null,
};

pub const PlexMarker = struct {
    marker_type: []const u8, // "credits", "intro", etc.
    start_time_ms: i64,
    end_time_ms: i64,
};

pub const TimelineState = enum {
    playing,
    paused,
    stopped,

    pub fn toString(self: TimelineState) []const u8 {
        return switch (self) {
            .playing => "playing",
            .paused => "paused",
            .stopped => "stopped",
        };
    }
};

test "PlexHeaders builds correctly" {
    var buf: [8]Header = undefined;
    const h = PlexHeaders{ .client_identifier = "test-uuid" };
    const headers = h.toHeaders(&buf);
    try std.testing.expectEqual(@as(usize, 6), headers.len);
}

test "PlexHeaders with token" {
    var buf: [8]Header = undefined;
    const h = PlexHeaders{ .client_identifier = "test-uuid", .auth_token = "abc123" };
    const headers = h.toHeaders(&buf);
    try std.testing.expectEqual(@as(usize, 7), headers.len);
}
