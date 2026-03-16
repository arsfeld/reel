const std = @import("std");
const http = @import("../http.zig");
const plex_types = @import("types.zig");
const auth_mod = @import("auth.zig");

pub const PlexClient = struct {
    allocator: std.mem.Allocator,
    http_client: *http.HttpClient,
    headers: plex_types.PlexHeaders,
    server_uri: ?[]const u8 = null,

    pub fn init(
        allocator: std.mem.Allocator,
        http_client: *http.HttpClient,
        client_identifier: []const u8,
        auth_token: ?[]const u8,
    ) PlexClient {
        return .{
            .allocator = allocator,
            .http_client = http_client,
            .headers = .{
                .client_identifier = client_identifier,
                .auth_token = auth_token,
            },
        };
    }

    pub fn setServerUri(self: *PlexClient, uri: []const u8) void {
        self.server_uri = uri;
    }

    pub fn setAuthToken(self: *PlexClient, token: []const u8) void {
        self.headers.auth_token = token;
    }

    /// Discover available Plex servers for the authenticated user.
    pub fn discoverServers(self: *PlexClient) ![]plex_types.PlexServer {
        var header_buf: [8]plex_types.Header = undefined;
        const headers = self.headers.toHeaders(&header_buf);

        var response = try self.http_client.get(
            "https://plex.tv/api/v2/resources?includeHttps=1&includeRelay=1",
            headers,
        );
        defer response.deinit();

        if (response.status != .ok) {
            std.log.err("discoverServers HTTP status: {d}", .{@intFromEnum(response.status)});
            return error.RequestFailed;
        }

        std.log.info("discoverServers response ({d} bytes): {s}", .{ response.body.len, response.body[0..@min(response.body.len, 500)] });

        var servers: std.ArrayList(plex_types.PlexServer) = .{};

        const parsed = std.json.parseFromSlice(std.json.Value, self.allocator, response.body, .{}) catch
            return error.RequestFailed;
        defer parsed.deinit();

        const devices = switch (parsed.value) {
            .array => |a| a,
            else => return error.RequestFailed,
        };

        for (devices.items) |device| {
            const obj = switch (device) {
                .object => |o| o,
                else => continue,
            };

            const provides = switch (obj.get("provides") orelse continue) {
                .string => |s| s,
                else => continue,
            };
            if (std.mem.indexOf(u8, provides, "server") == null) continue;

            const name = switch (obj.get("name") orelse continue) {
                .string => |s| s,
                else => continue,
            };
            const client_id = switch (obj.get("clientIdentifier") orelse continue) {
                .string => |s| s,
                else => continue,
            };

            // Parse nested connections array
            var conns: std.ArrayList(plex_types.PlexServer.Connection) = .{};
            if (obj.get("connections")) |conns_val| {
                if (conns_val == .array) {
                    for (conns_val.array.items) |conn| {
                        const c = switch (conn) {
                            .object => |o| o,
                            else => continue,
                        };
                        const uri = switch (c.get("uri") orelse continue) {
                            .string => |s| s,
                            else => continue,
                        };
                        try conns.append(self.allocator, .{
                            .uri = try self.allocator.dupe(u8, uri),
                            .local = jsonBool(c.get("local")),
                            .relay = jsonBool(c.get("relay")),
                            .protocol = try self.allocator.dupe(u8, if (c.get("protocol")) |v| switch (v) {
                                .string => |s| s,
                                else => "https",
                            } else "https"),
                        });
                    }
                }
            }

            try servers.append(self.allocator, .{
                .name = try self.allocator.dupe(u8, name),
                .machine_identifier = try self.allocator.dupe(u8, client_id),
                .access_token = if (obj.get("accessToken")) |v| switch (v) {
                    .string => |s| try self.allocator.dupe(u8, s),
                    else => null,
                } else null,
                .connections = conns.toOwnedSlice(self.allocator) catch &.{},
            });
        }

        return servers.toOwnedSlice(self.allocator);
    }

    /// Get library sections from the connected server.
    pub fn getLibraries(self: *PlexClient) ![]plex_types.PlexLibrary {
        const base = self.server_uri orelse {
            std.log.err("PlexClient.getLibraries: no server_uri set", .{});
            return error.InvalidUrl;
        };
        const url = try std.fmt.allocPrint(self.allocator, "{s}/library/sections", .{base});
        defer self.allocator.free(url);

        std.log.info("PlexClient.getLibraries: GET {s}", .{url});

        var header_buf: [8]plex_types.Header = undefined;
        const headers = self.headers.toHeaders(&header_buf);

        var response = self.http_client.get(url, headers) catch |err| {
            std.log.err("PlexClient.getLibraries: HTTP request failed: {}", .{err});
            return err;
        };
        defer response.deinit();

        std.log.info("PlexClient.getLibraries: HTTP {d}, body_len={d}", .{ @intFromEnum(response.status), response.body.len });
        if (response.status != .ok) {
            std.log.err("PlexClient.getLibraries: non-200 response: {d}, body: {s}", .{ @intFromEnum(response.status), response.body[0..@min(response.body.len, 500)] });
            return error.RequestFailed;
        }

        var libraries: std.ArrayList(plex_types.PlexLibrary) = .{};

        const parsed = std.json.parseFromSlice(std.json.Value, self.allocator, response.body, .{}) catch
            return error.RequestFailed;
        defer parsed.deinit();

        const mc = getMediaContainer(parsed.value) orelse return libraries.toOwnedSlice(self.allocator);
        const dirs = switch (mc.get("Directory") orelse return libraries.toOwnedSlice(self.allocator)) {
            .array => |a| a,
            else => return libraries.toOwnedSlice(self.allocator),
        };

        for (dirs.items) |item| {
            const obj = switch (item) {
                .object => |o| o,
                else => continue,
            };
            try libraries.append(self.allocator, .{
                .key = try self.allocator.dupe(u8, jsonString(obj.get("key")) orelse continue),
                .title = try self.allocator.dupe(u8, jsonString(obj.get("title")) orelse continue),
                .library_type = try self.allocator.dupe(u8, jsonString(obj.get("type")) orelse continue),
            });
        }

        return libraries.toOwnedSlice(self.allocator);
    }

    /// Browse items in a library section.
    pub fn getItems(self: *PlexClient, section_key: []const u8) ![]plex_types.PlexMediaItem {
        const base = self.server_uri orelse return error.InvalidUrl;
        const url = try std.fmt.allocPrint(self.allocator, "{s}/library/sections/{s}/all", .{ base, section_key });
        defer self.allocator.free(url);

        return self.fetchMediaItems(url);
    }

    /// Get On Deck items (continue watching).
    pub fn getOnDeck(self: *PlexClient) ![]plex_types.PlexMediaItem {
        const base = self.server_uri orelse return error.InvalidUrl;
        const url = try std.fmt.allocPrint(self.allocator, "{s}/library/onDeck", .{base});
        defer self.allocator.free(url);

        return self.fetchMediaItems(url);
    }

    /// Get recently added items.
    pub fn getRecentlyAdded(self: *PlexClient) ![]plex_types.PlexMediaItem {
        const base = self.server_uri orelse return error.InvalidUrl;
        const url = try std.fmt.allocPrint(self.allocator, "{s}/library/recentlyAdded", .{base});
        defer self.allocator.free(url);

        return self.fetchMediaItems(url);
    }

    /// Get metadata for a specific item.
    pub fn getMetadata(self: *PlexClient, rating_key: []const u8) !?plex_types.PlexMediaItem {
        const base = self.server_uri orelse return error.InvalidUrl;
        const url = try std.fmt.allocPrint(self.allocator, "{s}/library/metadata/{s}", .{ base, rating_key });
        defer self.allocator.free(url);

        const items = try self.fetchMediaItems(url);
        defer self.allocator.free(items);

        if (items.len > 0) return items[0];
        return null;
    }

    /// Get children of an item (seasons of a show, episodes of a season).
    pub fn getChildren(self: *PlexClient, rating_key: []const u8) ![]plex_types.PlexMediaItem {
        const base = self.server_uri orelse return error.InvalidUrl;
        const url = try std.fmt.allocPrint(self.allocator, "{s}/library/metadata/{s}/children", .{ base, rating_key });
        defer self.allocator.free(url);

        return self.fetchMediaItems(url);
    }

    /// Construct a direct play URL for a media item.
    pub fn getStreamUrl(self: *PlexClient, part_key: []const u8) ![]const u8 {
        const base = self.server_uri orelse return error.InvalidUrl;
        const token = self.headers.auth_token orelse return error.RequestFailed;

        return std.fmt.allocPrint(self.allocator, "{s}{s}?X-Plex-Token={s}", .{ base, part_key, token });
    }

    /// Report playback timeline to Plex (scrobbling).
    pub fn reportTimeline(
        self: *PlexClient,
        rating_key: []const u8,
        state: plex_types.TimelineState,
        time_ms: i64,
        duration_ms: i64,
    ) !void {
        const base = self.server_uri orelse return error.InvalidUrl;
        const url = try std.fmt.allocPrint(
            self.allocator,
            "{s}/:/timeline?ratingKey={s}&state={s}&time={d}&duration={d}",
            .{ base, rating_key, state.toString(), time_ms, duration_ms },
        );
        defer self.allocator.free(url);

        var header_buf: [8]plex_types.Header = undefined;
        const headers = self.headers.toHeaders(&header_buf);

        var response = try self.http_client.get(url, headers);
        defer response.deinit();
        // Timeline reporting failures are non-fatal
    }

    /// Search the Plex library.
    pub fn search(self: *PlexClient, query: []const u8) ![]plex_types.PlexMediaItem {
        const base = self.server_uri orelse return error.InvalidUrl;
        const url = try std.fmt.allocPrint(self.allocator, "{s}/search?query={s}", .{ base, query });
        defer self.allocator.free(url);

        return self.fetchMediaItems(url);
    }

    fn fetchMediaItems(self: *PlexClient, url: []const u8) ![]plex_types.PlexMediaItem {
        var header_buf: [8]plex_types.Header = undefined;
        const headers = self.headers.toHeaders(&header_buf);

        std.log.info("PlexClient.fetchMediaItems: GET {s}", .{url});

        var response = self.http_client.get(url, headers) catch |err| {
            std.log.err("PlexClient.fetchMediaItems: HTTP request failed for {s}: {}", .{ url, err });
            return err;
        };
        defer response.deinit();

        if (response.status != .ok) {
            std.log.err("PlexClient.fetchMediaItems: HTTP {d} for {s}", .{ @intFromEnum(response.status), url });
            return error.RequestFailed;
        }
        std.log.info("PlexClient.fetchMediaItems: HTTP 200, body_len={d}", .{response.body.len});

        var items: std.ArrayList(plex_types.PlexMediaItem) = .{};

        const parsed = std.json.parseFromSlice(std.json.Value, self.allocator, response.body, .{}) catch
            return error.RequestFailed;
        defer parsed.deinit();

        const mc = getMediaContainer(parsed.value) orelse return items.toOwnedSlice(self.allocator);
        const metadata = switch (mc.get("Metadata") orelse return items.toOwnedSlice(self.allocator)) {
            .array => |a| a,
            else => return items.toOwnedSlice(self.allocator),
        };

        for (metadata.items) |entry| {
            const obj = switch (entry) {
                .object => |o| o,
                else => continue,
            };
            try items.append(self.allocator, .{
                .rating_key = try self.allocator.dupe(u8, jsonString(obj.get("ratingKey")) orelse continue),
                .title = try self.allocator.dupe(u8, jsonString(obj.get("title")) orelse continue),
                .media_type = try self.allocator.dupe(u8, jsonString(obj.get("type")) orelse "unknown"),
                .summary = try dupeOptionalJsonString(self.allocator, obj.get("summary")),
                .year = jsonInt(i32, obj.get("year")),
                .rating = jsonFloat(obj.get("rating")),
                .duration_ms = jsonInt(i64, obj.get("duration")),
                .thumb = try dupeOptionalJsonString(self.allocator, obj.get("thumb")),
                .art = try dupeOptionalJsonString(self.allocator, obj.get("art")),
                .parent_rating_key = try dupeOptionalJsonString(self.allocator, obj.get("parentRatingKey")),
                .grandparent_rating_key = try dupeOptionalJsonString(self.allocator, obj.get("grandparentRatingKey")),
                .grandparent_title = try dupeOptionalJsonString(self.allocator, obj.get("grandparentTitle")),
                .parent_index = jsonInt(i32, obj.get("parentIndex")),
                .index = jsonInt(i32, obj.get("index")),
                .view_offset = jsonInt(i64, obj.get("viewOffset")),
                .part_key = try dupeOptionalJsonString(self.allocator, obj.get("key")),
            });
        }

        return items.toOwnedSlice(self.allocator);
    }

    pub fn freeMediaItems(self: *PlexClient, items: []plex_types.PlexMediaItem) void {
        for (items) |item| {
            self.allocator.free(item.rating_key);
            self.allocator.free(item.title);
            self.allocator.free(item.media_type);
            if (item.summary) |s| self.allocator.free(s);
            if (item.thumb) |s| self.allocator.free(s);
            if (item.art) |s| self.allocator.free(s);
            if (item.parent_rating_key) |s| self.allocator.free(s);
            if (item.grandparent_rating_key) |s| self.allocator.free(s);
            if (item.grandparent_title) |s| self.allocator.free(s);
            if (item.part_key) |s| self.allocator.free(s);
        }
        self.allocator.free(items);
    }
};

/// Extract the MediaContainer object from a parsed JSON value.
fn getMediaContainer(value: std.json.Value) ?std.json.ObjectMap {
    const root = switch (value) {
        .object => |o| o,
        else => return null,
    };
    return switch (root.get("MediaContainer") orelse return null) {
        .object => |o| o,
        else => null,
    };
}

/// Extract a boolean from a JSON value, defaulting to false.
/// Handles both proper booleans and integer 0/1 values.
fn jsonBool(val: ?std.json.Value) bool {
    if (val) |v| {
        return switch (v) {
            .bool => |b| b,
            .integer => |i| i != 0,
            else => false,
        };
    }
    return false;
}

/// Extract a string from a JSON value, returning null if not a string.
fn jsonString(val: ?std.json.Value) ?[]const u8 {
    if (val) |v| {
        return switch (v) {
            .string => |s| s,
            else => null,
        };
    }
    return null;
}

/// Extract an integer from a JSON value, returning null if not an integer.
fn jsonInt(comptime T: type, val: ?std.json.Value) ?T {
    if (val) |v| {
        return switch (v) {
            .integer => |i| @intCast(i),
            else => null,
        };
    }
    return null;
}

/// Extract a float from a JSON value, handling both float and integer types.
fn jsonFloat(val: ?std.json.Value) ?f64 {
    if (val) |v| {
        return switch (v) {
            .float => |f| f,
            .integer => |i| @floatFromInt(i),
            else => null,
        };
    }
    return null;
}

/// Dupe an optional JSON string value using the allocator.
fn dupeOptionalJsonString(allocator: std.mem.Allocator, val: ?std.json.Value) !?[]const u8 {
    if (val) |v| {
        switch (v) {
            .string => |s| return try allocator.dupe(u8, s),
            else => return null,
        }
    }
    return null;
}

test "PlexClient init" {
    var hc = http.HttpClient.init(std.testing.allocator);
    defer hc.deinit();

    var client = PlexClient.init(std.testing.allocator, &hc, "test-uuid", "test-token");
    _ = &client;
    client.setServerUri("http://localhost:32400");

    try std.testing.expectEqualStrings("http://localhost:32400", client.server_uri.?);
}

test "getMediaContainer extracts nested object" {
    const json =
        \\{"MediaContainer":{"size":2,"Metadata":[{"ratingKey":"123","title":"Test"}]}}
    ;
    const parsed = try std.json.parseFromSlice(std.json.Value, std.testing.allocator, json, .{});
    defer parsed.deinit();

    const mc = getMediaContainer(parsed.value).?;
    const metadata = mc.get("Metadata").?.array;
    try std.testing.expectEqual(@as(usize, 1), metadata.items.len);
}

test "getMediaContainer returns null for empty response" {
    const json =
        \\{"MediaContainer":{"size":0}}
    ;
    const parsed = try std.json.parseFromSlice(std.json.Value, std.testing.allocator, json, .{});
    defer parsed.deinit();

    const mc = getMediaContainer(parsed.value).?;
    try std.testing.expectEqual(@as(?std.json.Value, null), mc.get("Metadata"));
}

test "jsonString extracts string values" {
    const json =
        \\{"name":"test","count":42}
    ;
    const parsed = try std.json.parseFromSlice(std.json.Value, std.testing.allocator, json, .{});
    defer parsed.deinit();
    const obj = parsed.value.object;

    try std.testing.expectEqualStrings("test", jsonString(obj.get("name")).?);
    try std.testing.expectEqual(@as(?[]const u8, null), jsonString(obj.get("count")));
    try std.testing.expectEqual(@as(?[]const u8, null), jsonString(obj.get("missing")));
}

test "jsonInt extracts integer values" {
    const json =
        \\{"year":2024,"name":"test"}
    ;
    const parsed = try std.json.parseFromSlice(std.json.Value, std.testing.allocator, json, .{});
    defer parsed.deinit();
    const obj = parsed.value.object;

    try std.testing.expectEqual(@as(?i32, 2024), jsonInt(i32, obj.get("year")));
    try std.testing.expectEqual(@as(?i32, null), jsonInt(i32, obj.get("name")));
    try std.testing.expectEqual(@as(?i32, null), jsonInt(i32, obj.get("missing")));
}

test "jsonFloat handles float and integer" {
    const json =
        \\{"rating":7.5,"count":10}
    ;
    const parsed = try std.json.parseFromSlice(std.json.Value, std.testing.allocator, json, .{});
    defer parsed.deinit();
    const obj = parsed.value.object;

    try std.testing.expectEqual(@as(?f64, 7.5), jsonFloat(obj.get("rating")));
    try std.testing.expectEqual(@as(?f64, 10.0), jsonFloat(obj.get("count")));
    try std.testing.expectEqual(@as(?f64, null), jsonFloat(obj.get("missing")));
}

test "jsonBool handles booleans and integers" {
    const json =
        \\{"local":true,"relay":false,"flag":1,"off":0}
    ;
    const parsed = try std.json.parseFromSlice(std.json.Value, std.testing.allocator, json, .{});
    defer parsed.deinit();
    const obj = parsed.value.object;

    try std.testing.expect(jsonBool(obj.get("local")));
    try std.testing.expect(!jsonBool(obj.get("relay")));
    try std.testing.expect(jsonBool(obj.get("flag")));
    try std.testing.expect(!jsonBool(obj.get("off")));
    try std.testing.expect(!jsonBool(obj.get("missing")));
}

test "parse Plex metadata JSON" {
    const json =
        \\{"MediaContainer":{"size":1,"Metadata":[{"ratingKey":"123","title":"Test Movie","type":"movie","year":2024,"rating":7.5,"duration":7200000,"summary":"A test film","thumb":"/thumb/123","art":"/art/123","key":"/library/metadata/123","parentRatingKey":"100","grandparentRatingKey":"50","grandparentTitle":"Show","parentIndex":1,"index":5,"viewOffset":3600000}]}}
    ;
    const parsed = try std.json.parseFromSlice(std.json.Value, std.testing.allocator, json, .{});
    defer parsed.deinit();

    const mc = getMediaContainer(parsed.value).?;
    const metadata = mc.get("Metadata").?.array;
    try std.testing.expectEqual(@as(usize, 1), metadata.items.len);

    const obj = metadata.items[0].object;
    try std.testing.expectEqualStrings("123", jsonString(obj.get("ratingKey")).?);
    try std.testing.expectEqualStrings("Test Movie", jsonString(obj.get("title")).?);
    try std.testing.expectEqualStrings("movie", jsonString(obj.get("type")).?);
    try std.testing.expectEqual(@as(?i32, 2024), jsonInt(i32, obj.get("year")));
    try std.testing.expectEqual(@as(?f64, 7.5), jsonFloat(obj.get("rating")));
    try std.testing.expectEqual(@as(?i64, 7200000), jsonInt(i64, obj.get("duration")));
    try std.testing.expectEqualStrings("A test film", jsonString(obj.get("summary")).?);
    try std.testing.expectEqual(@as(?i32, 1), jsonInt(i32, obj.get("parentIndex")));
    try std.testing.expectEqual(@as(?i32, 5), jsonInt(i32, obj.get("index")));
    try std.testing.expectEqual(@as(?i64, 3600000), jsonInt(i64, obj.get("viewOffset")));
}

test "parse Plex resources JSON (bare array)" {
    const json =
        \\[{"name":"My Server","clientIdentifier":"abc123","provides":"server","accessToken":"tok","connections":[{"uri":"https://192.168.1.1:32400","local":true,"relay":false,"protocol":"https"}]},{"name":"Player","clientIdentifier":"def456","provides":"player"}]
    ;
    const parsed = try std.json.parseFromSlice(std.json.Value, std.testing.allocator, json, .{});
    defer parsed.deinit();

    const devices = parsed.value.array;
    try std.testing.expectEqual(@as(usize, 2), devices.items.len);

    // First device is a server
    const server = devices.items[0].object;
    try std.testing.expectEqualStrings("My Server", jsonString(server.get("name")).?);
    try std.testing.expectEqualStrings("server", jsonString(server.get("provides")).?);

    // It has connections
    const conns = server.get("connections").?.array;
    try std.testing.expectEqual(@as(usize, 1), conns.items.len);
    const conn = conns.items[0].object;
    try std.testing.expectEqualStrings("https://192.168.1.1:32400", jsonString(conn.get("uri")).?);
    try std.testing.expect(jsonBool(conn.get("local")));
    try std.testing.expect(!jsonBool(conn.get("relay")));
}

test "parse Plex library sections JSON" {
    const json =
        \\{"MediaContainer":{"size":2,"Directory":[{"key":"1","type":"movie","title":"Movies"},{"key":"2","type":"show","title":"TV Shows"}]}}
    ;
    const parsed = try std.json.parseFromSlice(std.json.Value, std.testing.allocator, json, .{});
    defer parsed.deinit();

    const mc = getMediaContainer(parsed.value).?;
    const dirs = mc.get("Directory").?.array;
    try std.testing.expectEqual(@as(usize, 2), dirs.items.len);

    const movie_lib = dirs.items[0].object;
    try std.testing.expectEqualStrings("1", jsonString(movie_lib.get("key")).?);
    try std.testing.expectEqualStrings("movie", jsonString(movie_lib.get("type")).?);
    try std.testing.expectEqualStrings("Movies", jsonString(movie_lib.get("title")).?);
}
