const std = @import("std");
const gtk = @import("apprt/gtk/app.zig");

pub fn main(init: std.process.Init.Minimal) !void {
    var it = std.process.Args.Iterator.init(init.args);
    _ = it.next(); // skip program name
    const file_path: ?[]const u8 = it.next();
    try gtk.run(file_path);
}
