const std = @import("std");

// Core modules
pub const player = @import("core/player.zig");
pub const database = @import("core/database.zig");
pub const types = @import("core/types.zig");
pub const settings = @import("core/settings.zig");
pub const library = @import("core/library.zig");
pub const scanner = @import("core/scanner.zig");
pub const downloader = @import("core/downloader.zig");
pub const image_cache = @import("core/image_cache.zig");

// Network modules
pub const http = @import("net/http.zig");
pub const media_server = @import("net/media_server.zig");
pub const plex_types = @import("net/plex/types.zig");
pub const plex_xml = @import("net/plex/xml.zig");
pub const plex_auth = @import("net/plex/auth.zig");
pub const plex_client = @import("net/plex/client.zig");
pub const tmdb_types = @import("net/tmdb/types.zig");
pub const tmdb_client = @import("net/tmdb/client.zig");

test {
    std.testing.refAllDecls(@This());
}
