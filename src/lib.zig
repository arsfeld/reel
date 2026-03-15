const std = @import("std");

// Core modules
pub const player = @import("core/player.zig");
pub const database = @import("core/database.zig");
pub const types = @import("core/types.zig");
pub const settings = @import("core/settings.zig");
pub const library = @import("core/library.zig");

// Network modules
pub const http = @import("net/http.zig");

test {
    std.testing.refAllDecls(@This());
}
