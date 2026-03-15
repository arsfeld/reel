const std = @import("std");

pub const MediaType = enum {
    movie,
    show,
    season,
    episode,
    other,

    pub fn toString(self: MediaType) []const u8 {
        return switch (self) {
            .movie => "movie",
            .show => "show",
            .season => "season",
            .episode => "episode",
            .other => "other",
        };
    }

    pub fn fromString(s: []const u8) ?MediaType {
        if (std.mem.eql(u8, s, "movie")) return .movie;
        if (std.mem.eql(u8, s, "show")) return .show;
        if (std.mem.eql(u8, s, "season")) return .season;
        if (std.mem.eql(u8, s, "episode")) return .episode;
        if (std.mem.eql(u8, s, "other")) return .other;
        return null;
    }
};

pub const MediaSource = enum {
    plex,
    local,

    pub fn toString(self: MediaSource) []const u8 {
        return switch (self) {
            .plex => "plex",
            .local => "local",
        };
    }

    pub fn fromString(s: []const u8) ?MediaSource {
        if (std.mem.eql(u8, s, "plex")) return .plex;
        if (std.mem.eql(u8, s, "local")) return .local;
        return null;
    }
};

pub const DownloadStatus = enum {
    queued,
    downloading,
    paused,
    complete,
    failed,

    pub fn toString(self: DownloadStatus) []const u8 {
        return switch (self) {
            .queued => "queued",
            .downloading => "downloading",
            .paused => "paused",
            .complete => "complete",
            .failed => "failed",
        };
    }

    pub fn fromString(s: []const u8) ?DownloadStatus {
        if (std.mem.eql(u8, s, "queued")) return .queued;
        if (std.mem.eql(u8, s, "downloading")) return .downloading;
        if (std.mem.eql(u8, s, "paused")) return .paused;
        if (std.mem.eql(u8, s, "complete")) return .complete;
        if (std.mem.eql(u8, s, "failed")) return .failed;
        return null;
    }
};

pub const Server = struct {
    id: []const u8,
    name: []const u8,
    client_identifier: []const u8,
    auth_token: ?[]const u8 = null,
    connection_uri: ?[]const u8 = null,
    last_connected_at: ?i64 = null,
};

pub const MediaItem = struct {
    id: i64 = 0,
    source: MediaSource,
    source_id: ?[]const u8 = null,
    server_id: ?[]const u8 = null,
    media_type: MediaType,
    title: []const u8,
    sort_title: ?[]const u8 = null,
    year: ?i32 = null,
    summary: ?[]const u8 = null,
    rating: ?f64 = null,
    duration_ms: ?i64 = null,
    poster_path: ?[]const u8 = null,
    backdrop_path: ?[]const u8 = null,
    tmdb_id: ?i32 = null,
    parent_id: ?i64 = null,
    season_number: ?i32 = null,
    episode_number: ?i32 = null,
    file_path: ?[]const u8 = null,
    added_at: ?i64 = null,
    updated_at: ?i64 = null,
    match_locked: bool = false,
};

pub const WatchProgress = struct {
    media_item_id: i64,
    position_ms: i64 = 0,
    duration_ms: ?i64 = null,
    watched: bool = false,
    last_watched_at: ?i64 = null,
};

pub const Download = struct {
    id: i64 = 0,
    media_item_id: i64,
    server_id: []const u8,
    source_url: []const u8,
    local_path: ?[]const u8 = null,
    total_bytes: ?i64 = null,
    downloaded_bytes: i64 = 0,
    status: DownloadStatus = .queued,
    created_at: ?i64 = null,
    completed_at: ?i64 = null,
    error_message: ?[]const u8 = null,
    part_key: ?[]const u8 = null,
};

pub const ScanPath = struct {
    id: i64 = 0,
    path: []const u8,
    last_scanned_at: ?i64 = null,
};

pub const FavoriteType = enum {
    media_item,
    plex_library,
    scan_path,
    filter,

    pub fn toString(self: FavoriteType) []const u8 {
        return switch (self) {
            .media_item => "media_item",
            .plex_library => "plex_library",
            .scan_path => "scan_path",
            .filter => "filter",
        };
    }

    pub fn fromString(s: []const u8) ?FavoriteType {
        if (std.mem.eql(u8, s, "media_item")) return .media_item;
        if (std.mem.eql(u8, s, "plex_library")) return .plex_library;
        if (std.mem.eql(u8, s, "scan_path")) return .scan_path;
        if (std.mem.eql(u8, s, "filter")) return .filter;
        return null;
    }
};

pub const Favorite = struct {
    id: i64 = 0,
    item_type: FavoriteType,
    item_id: []const u8,
    display_name: []const u8,
    sort_order: i32 = 0,
    created_at: ?i64 = null,
};

pub const CollectionType = enum {
    manual,
    smart,

    pub fn toString(self: CollectionType) []const u8 {
        return switch (self) {
            .manual => "manual",
            .smart => "smart",
        };
    }

    pub fn fromString(s: []const u8) ?CollectionType {
        if (std.mem.eql(u8, s, "manual")) return .manual;
        if (std.mem.eql(u8, s, "smart")) return .smart;
        return null;
    }
};

pub const Collection = struct {
    id: i64 = 0,
    name: []const u8,
    collection_type: CollectionType = .manual,
    description: ?[]const u8 = null,
    poster_path: ?[]const u8 = null,
    show_on_home: bool = true,
    sort_order: i32 = 0,
    created_at: ?i64 = null,
    updated_at: ?i64 = null,
};

pub const CollectionRule = struct {
    id: i64 = 0,
    collection_id: i64 = 0,
    field: []const u8,
    operator: []const u8,
    value: []const u8,
};

pub const Genre = struct {
    id: i64 = 0,
    name: []const u8,
};

test "MediaType round-trip" {
    const mt = MediaType.movie;
    const s = mt.toString();
    const parsed = MediaType.fromString(s).?;
    try std.testing.expectEqual(mt, parsed);
}

test "MediaSource round-trip" {
    const ms = MediaSource.plex;
    try std.testing.expectEqual(ms, MediaSource.fromString(ms.toString()).?);
}

test "DownloadStatus round-trip" {
    const ds = DownloadStatus.downloading;
    try std.testing.expectEqual(ds, DownloadStatus.fromString(ds.toString()).?);
}

test "CollectionType round-trip" {
    const ct = CollectionType.smart;
    try std.testing.expectEqual(ct, CollectionType.fromString(ct.toString()).?);
}
