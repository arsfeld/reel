const std = @import("std");
const gtk = @import("apprt/gtk/app.zig");

pub fn main() !void {
    var args = std.process.args();
    _ = args.skip(); // skip program name
    const file_path: ?[]const u8 = args.next();
    try gtk.run(file_path);
}
