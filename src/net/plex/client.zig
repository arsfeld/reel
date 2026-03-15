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

        if (response.status != .ok) return error.RequestFailed;

        var servers: std.ArrayList(plex_types.PlexServer) = .{};

        var parser = xml.XmlParser.init(response.body);
        while (parser.next()) |elem| {
            if (std.mem.eql(u8, elem.tag, "Device")) {
                const provides = elem.attr("provides") orelse continue;
                if (std.mem.indexOf(u8, provides, "server") == null) continue;

                try servers.append(self.allocator, .{
                    .name = try self.allocator.dupe(u8, elem.attr("name") orelse continue),
                    .machine_identifier = try self.allocator.dupe(u8, elem.attr("clientIdentifier") orelse continue),
                    .access_token = if (elem.attr("accessToken")) |t| try self.allocator.dupe(u8, t) else null,
                });
            }
        }

        return servers.toOwnedSlice(self.allocator);
    }

    /// Get library sections from the connected server.
    pub fn getLibraries(self: *PlexClient) ![]plex_types.PlexLibrary {
        const base = self.server_uri orelse return error.InvalidUrl;
        const url = try std.fmt.allocPrint(self.allocator, "{s}/library/sections", .{base});
        defer self.allocator.free(url);

        var header_buf: [8]plex_types.Header = undefined;
        const headers = self.headers.toHeaders(&header_buf);

        var response = try self.http_client.get(url, headers);
        defer response.deinit();

        if (response.status != .ok) return error.RequestFailed;

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

        var response = try self.http_client.get(url, headers);
        defer response.deinit();

        if (response.status != .ok) return error.RequestFailed;

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
