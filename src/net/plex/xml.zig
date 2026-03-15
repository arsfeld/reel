const std = @import("std");

/// Minimal XML attribute parser for Plex responses.
/// Plex XML is simple: flat elements with attributes, no nested text content.
/// Example: <Video ratingKey="123" title="Movie" year="2024" />
pub const XmlElement = struct {
    tag: []const u8,
    attrs: []const Attr,

    pub const Attr = struct {
        name: []const u8,
        value: []const u8,
    };

    pub fn attr(self: *const XmlElement, name: []const u8) ?[]const u8 {
        for (self.attrs) |a| {
            if (std.mem.eql(u8, a.name, name)) return a.value;
        }
        return null;
    }

    pub fn attrInt(self: *const XmlElement, name: []const u8) ?i32 {
        const val = self.attr(name) orelse return null;
        return std.fmt.parseInt(i32, val, 10) catch null;
    }

    pub fn attrInt64(self: *const XmlElement, name: []const u8) ?i64 {
        const val = self.attr(name) orelse return null;
        return std.fmt.parseInt(i64, val, 10) catch null;
    }

    pub fn attrFloat(self: *const XmlElement, name: []const u8) ?f64 {
        const val = self.attr(name) orelse return null;
        return std.fmt.parseFloat(f64, val) catch null;
    }
};

/// Parse XML into a list of elements with their attributes.
/// This is a streaming parser that yields elements one at a time.
pub const XmlParser = struct {
    data: []const u8,
    pos: usize = 0,
    attrs_buf: [64]XmlElement.Attr = undefined,

    pub fn init(data: []const u8) XmlParser {
        return .{ .data = data };
    }

    pub fn next(self: *XmlParser) ?XmlElement {
        while (self.pos < self.data.len) {
            // Find next '<'
            const start = std.mem.indexOfScalarPos(u8, self.data, self.pos, '<') orelse return null;
            self.pos = start + 1;

            // Skip comments, processing instructions, declarations
            if (self.pos < self.data.len and (self.data[self.pos] == '?' or self.data[self.pos] == '!')) {
                const end = std.mem.indexOfScalarPos(u8, self.data, self.pos, '>') orelse return null;
                self.pos = end + 1;
                continue;
            }

            // Skip closing tags
            if (self.pos < self.data.len and self.data[self.pos] == '/') {
                const end = std.mem.indexOfScalarPos(u8, self.data, self.pos, '>') orelse return null;
                self.pos = end + 1;
                continue;
            }

            // Find end of tag
            const end = std.mem.indexOfScalarPos(u8, self.data, self.pos, '>') orelse return null;
            const tag_content = self.data[self.pos..end];

            // Extract tag name
            const name_end = std.mem.indexOfAny(u8, tag_content, " \t\n\r/") orelse tag_content.len;
            const tag_name = tag_content[0..name_end];

            if (tag_name.len == 0) {
                self.pos = end + 1;
                continue;
            }

            // Parse attributes
            var attr_count: usize = 0;
            var attr_pos = name_end;

            while (attr_pos < tag_content.len and attr_count < self.attrs_buf.len) {
                // Skip whitespace
                while (attr_pos < tag_content.len and isWhitespace(tag_content[attr_pos])) {
                    attr_pos += 1;
                }

                if (attr_pos >= tag_content.len or tag_content[attr_pos] == '/' or tag_content[attr_pos] == '>') break;

                // Find '='
                const eq = std.mem.indexOfScalarPos(u8, tag_content, attr_pos, '=') orelse break;
                const attr_name = std.mem.trim(u8, tag_content[attr_pos..eq], " \t\n\r");

                // Find quoted value
                var val_start = eq + 1;
                while (val_start < tag_content.len and tag_content[val_start] != '"') {
                    val_start += 1;
                }
                if (val_start >= tag_content.len) break;
                val_start += 1; // skip opening quote

                const val_end = std.mem.indexOfScalarPos(u8, tag_content, val_start, '"') orelse break;
                const attr_value = tag_content[val_start..val_end];

                self.attrs_buf[attr_count] = .{ .name = attr_name, .value = attr_value };
                attr_count += 1;

                attr_pos = val_end + 1;
            }

            self.pos = end + 1;
            return XmlElement{
                .tag = tag_name,
                .attrs = self.attrs_buf[0..attr_count],
            };
        }
        return null;
    }

    fn isWhitespace(ch: u8) bool {
        return ch == ' ' or ch == '\t' or ch == '\n' or ch == '\r';
    }
};

test "parse simple XML" {
    const xml =
        \\<?xml version="1.0" encoding="UTF-8"?>
        \\<MediaContainer size="2">
        \\  <Video ratingKey="123" title="Test Movie" year="2024" />
        \\  <Video ratingKey="456" title="Another" />
        \\</MediaContainer>
    ;

    var parser = XmlParser.init(xml);

    // First element: MediaContainer
    const container = parser.next().?;
    try std.testing.expectEqualStrings("MediaContainer", container.tag);
    try std.testing.expectEqualStrings("2", container.attr("size").?);

    // Second: Video
    const video1 = parser.next().?;
    try std.testing.expectEqualStrings("Video", video1.tag);
    try std.testing.expectEqualStrings("123", video1.attr("ratingKey").?);
    try std.testing.expectEqualStrings("Test Movie", video1.attr("title").?);
    try std.testing.expectEqual(@as(?i32, 2024), video1.attrInt("year"));

    // Third: Video
    const video2 = parser.next().?;
    try std.testing.expectEqualStrings("456", video2.attr("ratingKey").?);

    // No more elements (closing tags are skipped)
    try std.testing.expectEqual(@as(?XmlElement, null), parser.next());
}

test "parse attributes with special chars" {
    const xml =
        \\<Directory key="/library/sections/1/all" title="Movies" type="movie" />
    ;

    var parser = XmlParser.init(xml);
    const dir = parser.next().?;
    try std.testing.expectEqualStrings("Directory", dir.tag);
    try std.testing.expectEqualStrings("/library/sections/1/all", dir.attr("key").?);
    try std.testing.expectEqualStrings("movie", dir.attr("type").?);
}
