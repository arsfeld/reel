const std = @import("std");
pub const player = @import("core/player.zig");

test {
    std.testing.refAllDecls(@This());
}
