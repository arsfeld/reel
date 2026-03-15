const std = @import("std");
const types = @import("types.zig");

pub const video_extensions = [_][]const u8{
    ".mkv", ".mp4", ".avi", ".m4v", ".ts",
    ".mov", ".wmv", ".flv", ".webm", ".mpg",
    ".mpeg", ".m2ts", ".vob", ".ogm",
};

pub const ParsedFilename = struct {
    title: []const u8,
    year: ?i32 = null,
    season: ?i32 = null,
    episode: ?i32 = null,
    is_tv: bool = false,
};

/// Check if a filename has a recognized video extension.
pub fn isVideoFile(path: []const u8) bool {
    const ext = std.fs.path.extension(path);
    for (video_extensions) |ve| {
        if (std.ascii.eqlIgnoreCase(ext, ve)) return true;
    }
    return false;
}

/// Parse a filename following Plex naming conventions.
/// Movies: "Movie Name (2024).mkv" or "Movie Name (2024)/Movie Name (2024).mkv"
/// TV: "Show Name - S01E05 - Episode Title.mkv" or "Show Name/Season 01/S01E05.mkv"
pub fn parseFilename(basename: []const u8) ParsedFilename {
    // Strip extension
    const name = stripExtension(basename);

    // Try TV pattern first: S01E02 or s01e02
    if (parseTVPattern(name)) |tv| return tv;

    // Try movie pattern: "Title (Year)"
    if (parseMoviePattern(name)) |movie| return movie;

    // Fallback: unrecognized pattern - will be categorized as "other"
    return .{ .title = name, .is_tv = false };
}

fn parseTVPattern(name: []const u8) ?ParsedFilename {
    // Look for SxxExx pattern (case insensitive)
    var i: usize = 0;
    while (i + 5 < name.len) : (i += 1) {
        if ((name[i] == 'S' or name[i] == 's') and
            std.ascii.isDigit(name[i + 1]))
        {
            // Find the 'E' or 'e'
            var j = i + 2;
            while (j < name.len and std.ascii.isDigit(name[j])) : (j += 1) {}

            if (j < name.len and (name[j] == 'E' or name[j] == 'e') and
                j + 1 < name.len and std.ascii.isDigit(name[j + 1]))
            {
                const season = std.fmt.parseInt(i32, name[i + 1 .. j], 10) catch continue;
                var k = j + 1;
                while (k < name.len and std.ascii.isDigit(name[k])) : (k += 1) {}
                const episode = std.fmt.parseInt(i32, name[j + 1 .. k], 10) catch continue;

                // Title is everything before the SxxExx, trimmed
                var title_end = i;
                while (title_end > 0 and (name[title_end - 1] == ' ' or name[title_end - 1] == '-' or name[title_end - 1] == '.')) {
                    title_end -= 1;
                }

                const title = if (title_end > 0) name[0..title_end] else name;

                return ParsedFilename{
                    .title = title,
                    .season = season,
                    .episode = episode,
                    .is_tv = true,
                };
            }
        }
        // Also handle lowercase at any position
    }
    return null;
}

fn parseMoviePattern(name: []const u8) ?ParsedFilename {
    // Look for (YYYY) pattern at end
    if (name.len < 7) return null; // minimum: "X (YYYY)"

    // Find last '(' that's followed by 4 digits and ')'
    var i: usize = name.len;
    while (i > 0) {
        i -= 1;
        if (name[i] == '(' and i + 5 < name.len and name[i + 5] == ')') {
            const year_str = name[i + 1 .. i + 5];
            const year = std.fmt.parseInt(i32, year_str, 10) catch continue;
            if (year >= 1900 and year <= 2100) {
                var title_end = i;
                while (title_end > 0 and name[title_end - 1] == ' ') {
                    title_end -= 1;
                }
                if (title_end > 0) {
                    return ParsedFilename{
                        .title = name[0..title_end],
                        .year = year,
                    };
                }
            }
        }
    }
    return null;
}

/// Determine the MediaType from a parsed filename.
/// - TV patterns (SxxExx) → .episode
/// - Movie patterns (title + year) → .movie
/// - Unrecognized → .other
pub fn mediaTypeFromParsed(parsed: ParsedFilename) types.MediaType {
    if (parsed.is_tv) return .episode;
    if (parsed.year != null) return .movie;
    return .other;
}

fn stripExtension(name: []const u8) []const u8 {
    const ext = std.fs.path.extension(name);
    if (ext.len > 0) {
        return name[0 .. name.len - ext.len];
    }
    return name;
}

/// Scan a directory recursively for video files.
/// Returns a list of absolute paths to video files.
pub fn scanDirectory(allocator: std.mem.Allocator, dir_path: []const u8) ![][]const u8 {
    var results: std.ArrayList([]const u8) = .{};
    errdefer {
        for (results.items) |item| allocator.free(item);
        results.deinit(allocator);
    }

    try scanDirRecursive(allocator, dir_path, &results);
    return results.toOwnedSlice(allocator);
}

fn scanDirRecursive(allocator: std.mem.Allocator, dir_path: []const u8, results: *std.ArrayList([]const u8)) !void {
    var dir = std.fs.cwd().openDir(dir_path, .{ .iterate = true }) catch return;
    defer dir.close();

    var iter = dir.iterate();
    while (try iter.next()) |entry| {
        const full_path = try std.fs.path.join(allocator, &.{ dir_path, entry.name });

        switch (entry.kind) {
            .file => {
                if (isVideoFile(entry.name)) {
                    try results.append(allocator, full_path);
                } else {
                    allocator.free(full_path);
                }
            },
            .directory => {
                defer allocator.free(full_path);
                try scanDirRecursive(allocator, full_path, results);
            },
            else => {
                allocator.free(full_path);
            },
        }
    }
}

test "isVideoFile" {
    try std.testing.expect(isVideoFile("movie.mkv"));
    try std.testing.expect(isVideoFile("movie.MP4"));
    try std.testing.expect(isVideoFile("movie.avi"));
    try std.testing.expect(!isVideoFile("movie.txt"));
    try std.testing.expect(!isVideoFile("movie.srt"));
}

test "parseFilename movie with year" {
    const result = parseFilename("The Matrix (1999).mkv");
    try std.testing.expectEqualStrings("The Matrix", result.title);
    try std.testing.expectEqual(@as(?i32, 1999), result.year);
    try std.testing.expectEqual(false, result.is_tv);
}

test "parseFilename TV episode" {
    const result = parseFilename("Breaking Bad - S01E01 - Pilot.mkv");
    try std.testing.expectEqualStrings("Breaking Bad", result.title);
    try std.testing.expectEqual(@as(?i32, 1), result.season);
    try std.testing.expectEqual(@as(?i32, 1), result.episode);
    try std.testing.expectEqual(true, result.is_tv);
}

test "parseFilename TV compact" {
    const result = parseFilename("Severance.S02E03.mkv");
    try std.testing.expectEqualStrings("Severance", result.title);
    try std.testing.expectEqual(@as(?i32, 2), result.season);
    try std.testing.expectEqual(@as(?i32, 3), result.episode);
}

test "parseFilename no metadata" {
    const result = parseFilename("random_video.mp4");
    try std.testing.expectEqualStrings("random_video", result.title);
    try std.testing.expectEqual(@as(?i32, null), result.year);
}

test "mediaTypeFromParsed" {
    try std.testing.expectEqual(types.MediaType.movie, mediaTypeFromParsed(parseFilename("The Matrix (1999).mkv")));
    try std.testing.expectEqual(types.MediaType.episode, mediaTypeFromParsed(parseFilename("Breaking Bad - S01E01.mkv")));
    try std.testing.expectEqual(types.MediaType.other, mediaTypeFromParsed(parseFilename("random_video.mp4")));
}
