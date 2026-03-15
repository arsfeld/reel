/// Abstract media server interface.
///
/// Uses Zig's comptime interface pattern (Ghostty-style).
/// Plex implements this first; Jellyfin/Emby can be added later.
pub const MediaServerVTable = struct {
    getLibraries: *const fn (ctx: *anyopaque) anyerror!void,
    getStreamUrl: *const fn (ctx: *anyopaque, item_id: []const u8) anyerror![]const u8,
};

/// Common types shared by all media server implementations.
pub const BrowseOptions = struct {
    page: u32 = 0,
    page_size: u32 = 50,
    sort: ?[]const u8 = null,
};

pub const PlayState = enum {
    playing,
    paused,
    stopped,
};
