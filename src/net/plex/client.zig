const std = @import("std");
const http = @import("../http.zig");
const plex_types = @import("types.zig");
const xml = @import("xml.zig");
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

        var parser = xml.XmlParser.init(response.body);
        var in_server_device = false;
        var current_connections: std.ArrayList(plex_types.PlexServer.Connection) = .{};
        var current_server: ?plex_types.PlexServer = null;

        while (parser.next()) |elem| {
            if (std.mem.eql(u8, elem.tag, "Device") or std.mem.eql(u8, elem.tag, "resource")) {
                // Flush previous server if any
                if (current_server) |*srv| {
                    srv.connections = current_connections.toOwnedSlice(self.allocator) catch &.{};
                    try servers.append(self.allocator, srv.*);
                    current_connections = .{};
                }

                const provides = elem.attr("provides") orelse {
                    in_server_device = false;
                    current_server = null;
                    continue;
                };
                if (std.mem.indexOf(u8, provides, "server") == null) {
                    in_server_device = false;
                    current_server = null;
                    continue;
                }

                in_server_device = true;
                current_server = .{
                    .name = try self.allocator.dupe(u8, elem.attr("name") orelse continue),
                    .machine_identifier = try self.allocator.dupe(u8, elem.attr("clientIdentifier") orelse continue),
                    .access_token = if (elem.attr("accessToken")) |t| try self.allocator.dupe(u8, t) else null,
                };
            } else if (in_server_device and (std.mem.eql(u8, elem.tag, "Connection") or std.mem.eql(u8, elem.tag, "connection"))) {
                const uri = elem.attr("uri") orelse continue;
                const local_str = elem.attr("local") orelse "0";
                const is_local = std.mem.eql(u8, local_str, "1");
                const relay_str = elem.attr("relay") orelse "0";
                const is_relay = std.mem.eql(u8, relay_str, "1");
                const protocol = elem.attr("protocol") orelse "https";
                try current_connections.append(self.allocator, .{
                    .uri = try self.allocator.dupe(u8, uri),
                    .local = is_local,
                    .relay = is_relay,
                    .protocol = try self.allocator.dupe(u8, protocol),
                });
            }
        }

        // Flush last server
        if (current_server) |*srv| {
            srv.connections = current_connections.toOwnedSlice(self.allocator) catch &.{};
            try servers.append(self.allocator, srv.*);
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

        var parser = xml.XmlParser.init(response.body);
        while (parser.next()) |elem| {
            if (std.mem.eql(u8, elem.tag, "Directory")) {
                try libraries.append(self.allocator, .{
                    .key = try self.allocator.dupe(u8, elem.attr("key") orelse continue),
                    .title = try self.allocator.dupe(u8, elem.attr("title") orelse continue),
                    .library_type = try self.allocator.dupe(u8, elem.attr("type") orelse continue),
                });
            }
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

        var parser = xml.XmlParser.init(response.body);
        while (parser.next()) |elem| {
            if (std.mem.eql(u8, elem.tag, "Video") or
                std.mem.eql(u8, elem.tag, "Directory"))
            {
                try items.append(self.allocator, .{
                    .rating_key = try self.allocator.dupe(u8, elem.attr("ratingKey") orelse continue),
                    .title = try self.allocator.dupe(u8, elem.attr("title") orelse continue),
                    .media_type = try self.allocator.dupe(u8, elem.attr("type") orelse "unknown"),
                    .summary = if (elem.attr("summary")) |s| try self.allocator.dupe(u8, s) else null,
                    .year = elem.attrInt("year"),
                    .rating = elem.attrFloat("rating"),
                    .duration_ms = elem.attrInt64("duration"),
                    .thumb = if (elem.attr("thumb")) |s| try self.allocator.dupe(u8, s) else null,
                    .art = if (elem.attr("art")) |s| try self.allocator.dupe(u8, s) else null,
                    .parent_rating_key = if (elem.attr("parentRatingKey")) |s| try self.allocator.dupe(u8, s) else null,
                    .grandparent_rating_key = if (elem.attr("grandparentRatingKey")) |s| try self.allocator.dupe(u8, s) else null,
                    .grandparent_title = if (elem.attr("grandparentTitle")) |s| try self.allocator.dupe(u8, s) else null,
                    .parent_index = elem.attrInt("parentIndex"),
                    .index = elem.attrInt("index"),
                    .view_offset = elem.attrInt64("viewOffset"),
                    .part_key = if (elem.attr("key")) |s| try self.allocator.dupe(u8, s) else null,
                });
            }
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

test "PlexClient init" {
    var hc = http.HttpClient.init(std.testing.allocator);
    defer hc.deinit();

    var client = PlexClient.init(std.testing.allocator, &hc, "test-uuid", "test-token");
    _ = &client;
    client.setServerUri("http://localhost:32400");

    try std.testing.expectEqualStrings("http://localhost:32400", client.server_uri.?);
}
