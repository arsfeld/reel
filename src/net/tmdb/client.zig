const std = @import("std");
const http = @import("../http.zig");
const tmdb_types = @import("types.zig");

const base_url = "https://api.themoviedb.org/3";

pub const TmdbClient = struct {
    allocator: std.mem.Allocator,
    http_client: *http.HttpClient,
    api_key: []const u8,

    pub fn init(allocator: std.mem.Allocator, http_client: *http.HttpClient, api_key: []const u8) TmdbClient {
        return .{
            .allocator = allocator,
            .http_client = http_client,
            .api_key = api_key,
        };
    }

    fn authHeaders(self: *TmdbClient, buf: []http.Header) []const http.Header {
        buf[0] = .{ .name = "Authorization", .value = self.api_key };
        buf[1] = .{ .name = "Accept", .value = "application/json" };
        return buf[0..2];
    }

    /// Search for movies by title.
    pub fn searchMovies(self: *TmdbClient, query: []const u8) ![]tmdb_types.SearchResult {
        const encoded = try urlEncode(self.allocator, query);
        defer self.allocator.free(encoded);

        const url = try std.fmt.allocPrint(self.allocator, "{s}/search/movie?query={s}", .{ base_url, encoded });
        defer self.allocator.free(url);

        return self.fetchSearchResults(url);
    }

    /// Search for TV shows by title.
    pub fn searchTV(self: *TmdbClient, query: []const u8) ![]tmdb_types.SearchResult {
        const encoded = try urlEncode(self.allocator, query);
        defer self.allocator.free(encoded);

        const url = try std.fmt.allocPrint(self.allocator, "{s}/search/tv?query={s}", .{ base_url, encoded });
        defer self.allocator.free(url);

        return self.fetchSearchResults(url);
    }

    /// Get movie details with credits and images.
    pub fn getMovieDetail(self: *TmdbClient, movie_id: i32) !?tmdb_types.MovieDetail {
        const url = try std.fmt.allocPrint(
            self.allocator,
            "{s}/movie/{d}?append_to_response=credits,images",
            .{ base_url, movie_id },
        );
        defer self.allocator.free(url);

        var header_buf: [4]http.Header = undefined;
        const headers = self.authHeaders(&header_buf);

        var response = try self.http_client.get(url, headers);
        defer response.deinit();

        if (response.status != .ok) return null;

        const parsed = std.json.parseFromSlice(std.json.Value, self.allocator, response.body, .{}) catch return null;
        defer parsed.deinit();

        const root = parsed.value.object;

        return tmdb_types.MovieDetail{
            .id = @intCast(root.get("id").?.integer),
            .title = try dupeJsonString(self.allocator, root.get("title")),
            .overview = try dupeOptionalJsonString(self.allocator, root.get("overview")),
            .release_date = try dupeOptionalJsonString(self.allocator, root.get("release_date")),
            .runtime = if (root.get("runtime")) |v| switch (v) {
                .integer => |i| @intCast(i),
                else => null,
            } else null,
            .poster_path = try dupeOptionalJsonString(self.allocator, root.get("poster_path")),
            .backdrop_path = try dupeOptionalJsonString(self.allocator, root.get("backdrop_path")),
            .vote_average = if (root.get("vote_average")) |v| switch (v) {
                .float => |f| f,
                .integer => |i| @floatFromInt(i),
                else => null,
            } else null,
        };
    }

    /// Get TV show details.
    pub fn getTVDetail(self: *TmdbClient, tv_id: i32) !?tmdb_types.TVDetail {
        const url = try std.fmt.allocPrint(
            self.allocator,
            "{s}/tv/{d}?append_to_response=credits,images",
            .{ base_url, tv_id },
        );
        defer self.allocator.free(url);

        var header_buf: [4]http.Header = undefined;
        const headers = self.authHeaders(&header_buf);

        var response = try self.http_client.get(url, headers);
        defer response.deinit();

        if (response.status != .ok) return null;

        const parsed = std.json.parseFromSlice(std.json.Value, self.allocator, response.body, .{}) catch return null;
        defer parsed.deinit();

        const root = parsed.value.object;

        return tmdb_types.TVDetail{
            .id = @intCast(root.get("id").?.integer),
            .name = try dupeJsonString(self.allocator, root.get("name")),
            .overview = try dupeOptionalJsonString(self.allocator, root.get("overview")),
            .first_air_date = try dupeOptionalJsonString(self.allocator, root.get("first_air_date")),
            .poster_path = try dupeOptionalJsonString(self.allocator, root.get("poster_path")),
            .backdrop_path = try dupeOptionalJsonString(self.allocator, root.get("backdrop_path")),
            .vote_average = if (root.get("vote_average")) |v| switch (v) {
                .float => |f| f,
                .integer => |i| @floatFromInt(i),
                else => null,
            } else null,
            .number_of_seasons = if (root.get("number_of_seasons")) |v| switch (v) {
                .integer => |i| @intCast(i),
                else => null,
            } else null,
            .number_of_episodes = if (root.get("number_of_episodes")) |v| switch (v) {
                .integer => |i| @intCast(i),
                else => null,
            } else null,
        };
    }

    pub fn freeMovieDetail(self: *TmdbClient, detail: tmdb_types.MovieDetail) void {
        self.allocator.free(detail.title);
        if (detail.overview) |s| self.allocator.free(s);
        if (detail.release_date) |s| self.allocator.free(s);
        if (detail.poster_path) |s| self.allocator.free(s);
        if (detail.backdrop_path) |s| self.allocator.free(s);
    }

    pub fn freeTVDetail(self: *TmdbClient, detail: tmdb_types.TVDetail) void {
        self.allocator.free(detail.name);
        if (detail.overview) |s| self.allocator.free(s);
        if (detail.first_air_date) |s| self.allocator.free(s);
        if (detail.poster_path) |s| self.allocator.free(s);
        if (detail.backdrop_path) |s| self.allocator.free(s);
    }

    pub fn freeSearchResults(self: *TmdbClient, results: []tmdb_types.SearchResult) void {
        for (results) |r| {
            self.allocator.free(r.title);
            if (r.overview) |s| self.allocator.free(s);
            if (r.release_date) |s| self.allocator.free(s);
            if (r.poster_path) |s| self.allocator.free(s);
            if (r.backdrop_path) |s| self.allocator.free(s);
        }
        self.allocator.free(results);
    }

    fn fetchSearchResults(self: *TmdbClient, url: []const u8) ![]tmdb_types.SearchResult {
        var header_buf: [4]http.Header = undefined;
        const headers = self.authHeaders(&header_buf);

        var response = try self.http_client.get(url, headers);
        defer response.deinit();

        if (response.status != .ok) return error.RequestFailed;

        const parsed = std.json.parseFromSlice(std.json.Value, self.allocator, response.body, .{}) catch
            return error.RequestFailed;
        defer parsed.deinit();

        const results_val = parsed.value.object.get("results") orelse return &.{};
        const results_arr = switch (results_val) {
            .array => |a| a,
            else => return &.{},
        };

        var results: std.ArrayList(tmdb_types.SearchResult) = .{};

        for (results_arr.items) |item| {
            const obj = switch (item) {
                .object => |o| o,
                else => continue,
            };

            const id_val = obj.get("id") orelse continue;
            const id: i32 = switch (id_val) {
                .integer => |i| @intCast(i),
                else => continue,
            };

            // Movies use "title", TV uses "name"
            const title = obj.get("title") orelse obj.get("name") orelse continue;
            const title_str = switch (title) {
                .string => |s| s,
                else => continue,
            };

            try results.append(self.allocator, .{
                .id = id,
                .title = try self.allocator.dupe(u8, title_str),
                .overview = try dupeOptionalJsonString(self.allocator, obj.get("overview")),
                .release_date = try dupeOptionalJsonString(self.allocator, obj.get("release_date") orelse obj.get("first_air_date")),
                .poster_path = try dupeOptionalJsonString(self.allocator, obj.get("poster_path")),
                .backdrop_path = try dupeOptionalJsonString(self.allocator, obj.get("backdrop_path")),
                .vote_average = if (obj.get("vote_average")) |v| switch (v) {
                    .float => |f| f,
                    .integer => |i| @floatFromInt(i),
                    else => null,
                } else null,
            });
        }

        return results.toOwnedSlice(self.allocator);
    }
};

fn dupeJsonString(allocator: std.mem.Allocator, val: ?std.json.Value) ![]const u8 {
    if (val) |v| {
        switch (v) {
            .string => |s| return allocator.dupe(u8, s),
            else => {},
        }
    }
    return allocator.dupe(u8, "");
}

fn dupeOptionalJsonString(allocator: std.mem.Allocator, val: ?std.json.Value) !?[]const u8 {
    if (val) |v| {
        switch (v) {
            .string => |s| return try allocator.dupe(u8, s),
            .null => return null,
            else => {},
        }
    }
    return null;
}

fn urlEncode(allocator: std.mem.Allocator, input: []const u8) ![]const u8 {
    var result: std.ArrayList(u8) = .{};
    for (input) |ch| {
        if (std.ascii.isAlphanumeric(ch) or ch == '-' or ch == '_' or ch == '.' or ch == '~') {
            try result.append(allocator, ch);
        } else if (ch == ' ') {
            try result.append(allocator, '+');
        } else {
            try result.writer(allocator).print("%{X:0>2}", .{ch});
        }
    }
    return result.toOwnedSlice(allocator);
}

test "urlEncode" {
    const encoded = try urlEncode(std.testing.allocator, "The Matrix (1999)");
    defer std.testing.allocator.free(encoded);
    try std.testing.expectEqualStrings("The+Matrix+%281999%29", encoded);
}

test "TmdbClient init" {
    var hc = http.HttpClient.init(std.testing.allocator);
    defer hc.deinit();

    const client = TmdbClient.init(std.testing.allocator, &hc, "Bearer test-key");
    _ = client;
}
