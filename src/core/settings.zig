const std = @import("std");
const database = @import("database.zig");

pub const Settings = struct {
    db: *database.Database,
    allocator: std.mem.Allocator,

    pub fn init(allocator: std.mem.Allocator, db: *database.Database) Settings {
        return .{ .db = db, .allocator = allocator };
    }

    /// Returns an allocated string that the caller must free.
    pub fn getString(self: *Settings, key: []const u8) !?[]const u8 {
        var stmt = self.db.prepare("SELECT value FROM settings WHERE key = ?") catch return null;
        defer stmt.finalize();
        stmt.bindText(1, key);
        if (stmt.step()) {
            if (stmt.columnText(0)) |text| {
                return try self.allocator.dupe(u8, text);
            }
        }
        return null;
    }

    pub fn setString(self: *Settings, key: []const u8, value: []const u8) !void {
        var stmt = try self.db.prepare("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)");
        defer stmt.finalize();
        stmt.bindText(1, key);
        stmt.bindText(2, value);
        try stmt.exec();
    }

    pub fn getInt(self: *Settings, key: []const u8) !?i64 {
        const val = try self.getString(key) orelse return null;
        defer self.allocator.free(val);
        return std.fmt.parseInt(i64, val, 10) catch null;
    }

    pub fn setInt(self: *Settings, key: []const u8, value: i64) !void {
        var buf: [32]u8 = undefined;
        const str = std.fmt.bufPrint(&buf, "{d}", .{value}) catch return error.FormatFailed;
        try self.setString(key, str);
    }

    pub fn getBool(self: *Settings, key: []const u8) !?bool {
        const val = try self.getString(key) orelse return null;
        defer self.allocator.free(val);
        if (std.mem.eql(u8, val, "true") or std.mem.eql(u8, val, "1")) return true;
        if (std.mem.eql(u8, val, "false") or std.mem.eql(u8, val, "0")) return false;
        return null;
    }

    pub fn setBool(self: *Settings, key: []const u8, value: bool) !void {
        try self.setString(key, if (value) "true" else "false");
    }

    pub fn delete(self: *Settings, key: []const u8) !void {
        var stmt = try self.db.prepare("DELETE FROM settings WHERE key = ?");
        defer stmt.finalize();
        stmt.bindText(1, key);
        try stmt.exec();
    }
};

// Well-known settings keys
pub const keys = struct {
    pub const tmdb_api_key = "tmdb_api_key";
    pub const download_path = "download_path";
    pub const scan_interval_minutes = "scan_interval_minutes";
    pub const client_identifier = "client_identifier";
    pub const preferred_subtitle_lang = "preferred_subtitle_lang";
    pub const preferred_audio_lang = "preferred_audio_lang";
    pub const image_cache_max_mb = "image_cache_max_mb";

    // Subtitle appearance
    pub const sub_font = "sub_font";
    pub const sub_font_size = "sub_font_size";
    pub const sub_color = "sub_color"; // stored as #AARRGGBB
    pub const sub_border_color = "sub_border_color";
    pub const sub_border_size = "sub_border_size";
    pub const sub_back_color = "sub_back_color";
    pub const sub_pos = "sub_pos"; // 0-100, 100=bottom
};

test "settings round-trip" {
    var db = try database.Database.open(":memory:");
    defer db.close();

    var settings = Settings.init(std.testing.allocator, &db);

    try settings.setString("name", "Reel");
    const name = (try settings.getString("name")).?;
    defer std.testing.allocator.free(name);
    try std.testing.expectEqualStrings("Reel", name);

    try settings.setInt("count", 42);
    try std.testing.expectEqual(@as(?i64, 42), try settings.getInt("count"));

    try settings.setBool("enabled", true);
    try std.testing.expectEqual(@as(?bool, true), try settings.getBool("enabled"));
}
