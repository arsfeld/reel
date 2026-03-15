const std = @import("std");

pub const image_base_url = "https://image.tmdb.org/t/p/";

pub const ImageSize = enum {
    w92,
    w154,
    w185,
    w342,
    w500,
    w780,
    original,

    pub fn toString(self: ImageSize) []const u8 {
        return switch (self) {
            .w92 => "w92",
            .w154 => "w154",
            .w185 => "w185",
            .w342 => "w342",
            .w500 => "w500",
            .w780 => "w780",
            .original => "original",
        };
    }
};

/// Build a full image URL from a TMDB path.
/// path is like "/kqjL17yufvn9OVLyXYpvtyrFfak.jpg"
pub fn imageUrl(allocator: std.mem.Allocator, size: ImageSize, path: []const u8) ![]const u8 {
    return std.fmt.allocPrint(allocator, "{s}{s}{s}", .{ image_base_url, size.toString(), path });
}

pub const SearchResult = struct {
    id: i32,
    title: []const u8, // "title" for movies, "name" for TV
    overview: ?[]const u8 = null,
    release_date: ?[]const u8 = null, // "release_date" for movies, "first_air_date" for TV
    poster_path: ?[]const u8 = null,
    backdrop_path: ?[]const u8 = null,
    vote_average: ?f64 = null,
};

pub const MovieDetail = struct {
    id: i32,
    title: []const u8,
    overview: ?[]const u8 = null,
    release_date: ?[]const u8 = null,
    runtime: ?i32 = null, // minutes
    poster_path: ?[]const u8 = null,
    backdrop_path: ?[]const u8 = null,
    vote_average: ?f64 = null,
    genres: []const Genre = &.{},
};

pub const TVDetail = struct {
    id: i32,
    name: []const u8,
    overview: ?[]const u8 = null,
    first_air_date: ?[]const u8 = null,
    poster_path: ?[]const u8 = null,
    backdrop_path: ?[]const u8 = null,
    vote_average: ?f64 = null,
    number_of_seasons: ?i32 = null,
    number_of_episodes: ?i32 = null,
};

pub const Genre = struct {
    id: i32,
    name: []const u8,
};

test "imageUrl construction" {
    const url = try imageUrl(std.testing.allocator, .w342, "/kqjL17yufvn9OVLyXYpvtyrFfak.jpg");
    defer std.testing.allocator.free(url);
    try std.testing.expectEqualStrings("https://image.tmdb.org/t/p/w342/kqjL17yufvn9OVLyXYpvtyrFfak.jpg", url);
}
