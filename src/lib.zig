const std = @import("std");

// Core modules
pub const player = @import("core/player.zig");
pub const database = @import("core/database.zig");
pub const types = @import("core/types.zig");
pub const settings = @import("core/settings.zig");
pub const library = @import("core/library.zig");

// Network modules
pub const http = @import("net/http.zig");
pub const media_server = @import("net/media_server.zig");
pub const plex_types = @import("net/plex/types.zig");
pub const plex_xml = @import("net/plex/xml.zig");
pub const plex_auth = @import("net/plex/auth.zig");
pub const plex_client = @import("net/plex/client.zig");

test {
    std.testing.refAllDecls(@This());
}
