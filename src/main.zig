const std = @import("std");
const gtk = @import("apprt/gtk/app.zig");

pub fn main() !void {
    const args = try std.process.argsAlloc(std.heap.c_allocator);
    defer std.process.argsFree(std.heap.c_allocator, args);

    const file_path: ?[]const u8 = if (args.len > 1) args[1] else null;
    try gtk.run(file_path);
}
