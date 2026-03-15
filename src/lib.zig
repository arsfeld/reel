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

// ── C ABI Exports ──────────────────────────────────────────

const allocator = std.heap.c_allocator;

// Player

const ReelState = enum(c_int) {
    idle = 0,
    playing = 1,
    paused = 2,
    stopped = 3,
};

const PlayerWrapper = struct {
    p: player.Player,
    state: ReelState = .idle,
    position: f64 = 0,
    duration: f64 = 0,
};

export fn reel_player_create() ?*PlayerWrapper {
    const pw = allocator.create(PlayerWrapper) catch return null;
    pw.* = .{ .p = player.Player.init() catch {
        allocator.destroy(pw);
        return null;
    } };
    return pw;
}

export fn reel_player_destroy(pw: ?*PlayerWrapper) void {
    const p = pw orelse return;
    p.p.deinit();
    allocator.destroy(p);
}

export fn reel_player_load_file(pw: ?*PlayerWrapper, path: [*:0]const u8) c_int {
    const p = pw orelse return -1;
    p.p.loadFile(std.mem.span(path)) catch return -2;
    p.state = .playing;
    return 0;
}

export fn reel_player_toggle_pause(pw: ?*PlayerWrapper) c_int {
    const p = pw orelse return -1;
    p.p.togglePause() catch return -3;
    p.state = if (p.state == .paused) .playing else .paused;
    return 0;
}

export fn reel_player_seek(pw: ?*PlayerWrapper, seconds: f64) c_int {
    const p = pw orelse return -1;
    p.p.seek(seconds) catch return -3;
    return 0;
}

export fn reel_player_seek_absolute(pw: ?*PlayerWrapper, seconds: f64) c_int {
    const p = pw orelse return -1;
    p.p.seekAbsolute(seconds) catch return -3;
    return 0;
}

export fn reel_player_set_volume(pw: ?*PlayerWrapper, volume: f64) c_int {
    const p = pw orelse return -1;
    p.p.setVolume(volume) catch return -3;
    return 0;
}

export fn reel_player_toggle_mute(pw: ?*PlayerWrapper) c_int {
    const p = pw orelse return -1;
    p.p.toggleMute() catch return -3;
    return 0;
}

export fn reel_player_cycle_sub(pw: ?*PlayerWrapper) c_int {
    const p = pw orelse return -1;
    p.p.cycleSub() catch return -3;
    return 0;
}

export fn reel_player_cycle_audio(pw: ?*PlayerWrapper) c_int {
    const p = pw orelse return -1;
    p.p.cycleAudio() catch return -3;
    return 0;
}

export fn reel_player_get_position(pw: ?*PlayerWrapper) f64 {
    const p = pw orelse return 0;
    // Query mpv directly for current position
    const ev = p.p.waitEvent(0);
    switch (ev) {
        .property_change => |pc| switch (pc) {
            .time_pos => |pos| if (pos) |v| {
                p.position = v;
            },
            .duration => |dur| if (dur) |v| {
                p.duration = v;
            },
            else => {},
        },
        else => {},
    }
    return p.position;
}

export fn reel_player_get_duration(pw: ?*PlayerWrapper) f64 {
    const p = pw orelse return 0;
    return p.duration;
}

export fn reel_player_get_volume(pw: ?*PlayerWrapper) f64 {
    const p = pw orelse return 100;
    const ev = p.p.waitEvent(0);
    switch (ev) {
        .property_change => |pc| switch (pc) {
            .volume => |vol| return vol,
            else => {},
        },
        else => {},
    }
    return 100;
}

export fn reel_player_stop(pw: ?*PlayerWrapper) c_int {
    const p = pw orelse return -1;
    // Use the "stop" command to halt playback
    p.p.loadFile("") catch {};
    p.state = .stopped;
    p.position = 0;
    p.duration = 0;
    return 0;
}

/// Process pending mpv events and update cached position/duration/state.
/// Call this from the frontend's polling timer to keep state fresh.
export fn reel_player_poll_events(pw: ?*PlayerWrapper) void {
    const p = pw orelse return;
    // Drain all pending events
    while (true) {
        const ev = p.p.waitEvent(0);
        switch (ev) {
            .property_change => |pc| switch (pc) {
                .time_pos => |pos| if (pos) |v| {
                    p.position = v;
                },
                .duration => |dur| if (dur) |v| {
                    p.duration = v;
                },
                .pause => |paused| {
                    p.state = if (paused) .paused else .playing;
                },
                .eof_reached => |eof| {
                    if (eof) p.state = .stopped;
                },
                else => {},
            },
            .end_file => {
                p.state = .stopped;
            },
            .idle, .shutdown => break,
            .unknown => break,
            .file_loaded => {},
        }
    }
}

export fn reel_player_get_state(pw: ?*PlayerWrapper) c_int {
    const p = pw orelse return 0;
    return @intFromEnum(p.state);
}

// Database

export fn reel_db_open(path: [*:0]const u8) ?*database.Database {
    const db = allocator.create(database.Database) catch return null;
    db.* = database.Database.open(path) catch {
        allocator.destroy(db);
        return null;
    };
    return db;
}

export fn reel_db_close(db: ?*database.Database) void {
    const d = db orelse return;
    d.close();
    allocator.destroy(d);
}

// Library

export fn reel_library_create(db: ?*database.Database) ?*library.Library {
    const d = db orelse return null;
    const lib = allocator.create(library.Library) catch return null;
    lib.* = library.Library.init(allocator, d);
    return lib;
}

export fn reel_library_destroy(lib: ?*library.Library) void {
    const l = lib orelse return;
    allocator.destroy(l);
}

export fn reel_library_get_item_count(lib: ?*library.Library, media_type: c_int) i32 {
    const l = lib orelse return 0;
    const mt = intToMediaType(media_type) orelse return 0;
    return @intCast(l.getItemCount(mt) catch return 0);
}

export fn reel_library_server_count(lib: ?*library.Library) i32 {
    const l = lib orelse return 0;
    const servers = l.listServers() catch return 0;
    defer l.freeServers(servers);
    return @intCast(servers.len);
}

export fn reel_library_add_favorite(
    lib: ?*library.Library,
    item_type: [*:0]const u8,
    item_id: [*:0]const u8,
    display_name: [*:0]const u8,
) i64 {
    const l = lib orelse return -1;
    const ft = types.FavoriteType.fromString(std.mem.span(item_type)) orelse return -1;
    return l.addFavorite(.{
        .item_type = ft,
        .item_id = std.mem.span(item_id),
        .display_name = std.mem.span(display_name),
    }) catch return -1;
}

export fn reel_library_remove_favorite(lib: ?*library.Library, id: i64) c_int {
    const l = lib orelse return -1;
    l.removeFavorite(id) catch return -1;
    return 0;
}

export fn reel_library_add_scan_path(lib: ?*library.Library, path: [*:0]const u8) i64 {
    const l = lib orelse return -1;
    return l.insertScanPath(std.mem.span(path)) catch return -1;
}

export fn reel_library_remove_scan_path(lib: ?*library.Library, id: i64) c_int {
    const l = lib orelse return -1;
    l.deleteScanPath(id) catch return -1;
    return 0;
}

// Settings

export fn reel_settings_get(db: ?*database.Database, key: [*:0]const u8) ?[*:0]const u8 {
    const d = db orelse return null;
    var s = settings.Settings.init(allocator, d);
    const val = (s.getString(std.mem.span(key)) catch return null) orelse return null;
    // Caller must free this string (or we leak — acceptable for simple bridging)
    return @ptrCast(val.ptr);
}

export fn reel_settings_set(db: ?*database.Database, key: [*:0]const u8, value: [*:0]const u8) c_int {
    const d = db orelse return -1;
    var s = settings.Settings.init(allocator, d);
    s.setString(std.mem.span(key), std.mem.span(value)) catch return -1;
    return 0;
}

// Downloads

export fn reel_download_create(db: ?*database.Database) ?*downloader.Downloader {
    const d = db orelse return null;
    const dl = allocator.create(downloader.Downloader) catch return null;
    dl.* = downloader.Downloader.init(allocator, d);
    return dl;
}

export fn reel_download_destroy(dl: ?*downloader.Downloader) void {
    const d = dl orelse return;
    allocator.destroy(d);
}

export fn reel_download_enqueue(
    dl: ?*downloader.Downloader,
    media_item_id: i64,
    server_id: [*:0]const u8,
    source_url: [*:0]const u8,
    download_dir: [*:0]const u8,
    filename: [*:0]const u8,
) i64 {
    const d = dl orelse return -1;
    return d.enqueue(.{
        .media_item_id = media_item_id,
        .server_id = std.mem.span(server_id),
        .source_url = std.mem.span(source_url),
        .download_dir = std.mem.span(download_dir),
        .filename = std.mem.span(filename),
    }) catch return -1;
}

export fn reel_download_pause(dl: ?*downloader.Downloader, id: i64) c_int {
    const d = dl orelse return -1;
    d.pause(id) catch return -1;
    return 0;
}

export fn reel_download_resume(dl: ?*downloader.Downloader, id: i64) c_int {
    const d = dl orelse return -1;
    d.resumeDownload(id) catch return -1;
    return 0;
}

export fn reel_download_remove(dl: ?*downloader.Downloader, id: i64, delete_file: bool) c_int {
    const d = dl orelse return -1;
    d.remove(id, delete_file) catch return -1;
    return 0;
}

export fn reel_download_get_local_path(dl: ?*downloader.Downloader, media_item_id: i64) ?[*:0]const u8 {
    const d = dl orelse return null;
    const path = (d.getCompletedLocalPath(media_item_id) catch return null) orelse return null;
    // Caller responsible for memory (acceptable for bridging)
    return @ptrCast(path.ptr);
}

// Collections

export fn reel_collection_create(
    lib: ?*library.Library,
    name: [*:0]const u8,
    collection_type: c_int,
    description: ?[*:0]const u8,
) i64 {
    const l = lib orelse return -1;
    const ct: types.CollectionType = switch (collection_type) {
        0 => .manual,
        1 => .smart,
        else => return -1,
    };
    const desc: ?[]const u8 = if (description) |d| std.mem.span(d) else null;
    return l.createCollection(std.mem.span(name), ct, desc) catch return -1;
}

export fn reel_collection_delete(lib: ?*library.Library, id: i64) c_int {
    const l = lib orelse return -1;
    l.deleteCollection(id) catch return -1;
    return 0;
}

/// Writes a flat array of ReelCollectionC structs into caller-provided output.
/// Returns the number of collections written, or -1 on error.
const ReelCollectionC = extern struct {
    id: i64,
    name: [*:0]const u8,
    collection_type: c_int, // 0=manual, 1=smart
    description: ?[*:0]const u8,
};

export fn reel_collection_list(
    lib: ?*library.Library,
    out_ptr: ?[*]ReelCollectionC,
    out_count: ?*i32,
) c_int {
    const l = lib orelse return -1;
    const count_ptr = out_count orelse return -1;

    const cols = l.listCollections() catch return -1;

    // If out_ptr is null, just return the count
    if (out_ptr == null) {
        count_ptr.* = @intCast(cols.len);
        l.freeCollections(cols);
        return 0;
    }

    const out = out_ptr.?;
    for (cols, 0..) |col, i| {
        out[i] = .{
            .id = col.id,
            .name = @ptrCast(allocator.dupeZ(u8, col.name) catch {
                l.freeCollections(cols);
                return -1;
            }),
            .collection_type = switch (col.collection_type) {
                .manual => 0,
                .smart => 1,
            },
            .description = if (col.description) |d|
                @ptrCast(allocator.dupeZ(u8, d) catch {
                    l.freeCollections(cols);
                    return -1;
                })
            else
                null,
        };
    }
    count_ptr.* = @intCast(cols.len);
    l.freeCollections(cols);
    return 0;
}

export fn reel_collection_add_item(lib: ?*library.Library, collection_id: i64, media_item_id: i64) c_int {
    const l = lib orelse return -1;
    l.addToCollection(collection_id, media_item_id) catch return -1;
    return 0;
}

export fn reel_collection_remove_item(lib: ?*library.Library, collection_id: i64, media_item_id: i64) c_int {
    const l = lib orelse return -1;
    l.removeFromCollection(collection_id, media_item_id) catch return -1;
    return 0;
}

// Genres

export fn reel_genre_set(
    lib: ?*library.Library,
    media_item_id: i64,
    genre_names: ?[*]const [*:0]const u8,
    count: c_int,
) c_int {
    const l = lib orelse return -1;
    if (count < 0) return -1;
    const cnt: usize = @intCast(count);
    if (cnt == 0) {
        // Clear all genres for this item
        l.setMediaItemGenres(media_item_id, &.{}) catch return -1;
        return 0;
    }
    const names_ptr = genre_names orelse return -1;

    // Convert C string array to Zig slices
    var name_slices = allocator.alloc([]const u8, cnt) catch return -1;
    defer allocator.free(name_slices);
    for (0..cnt) |i| {
        name_slices[i] = std.mem.span(names_ptr[i]);
    }
    l.setMediaItemGenres(media_item_id, name_slices) catch return -1;
    return 0;
}

// Match lock

export fn reel_match_set_locked(lib: ?*library.Library, media_item_id: i64, locked: bool) c_int {
    const l = lib orelse return -1;
    l.setMatchLocked(media_item_id, locked) catch return -1;
    return 0;
}

// Plex Auth

const PlexAuthWrapper = struct {
    http_client: http.HttpClient,
    plex_auth: plex_auth.PlexAuth,
    plex_client: plex_client.PlexClient,
    client_identifier: []const u8,
};

const ReelPlexServerC = extern struct {
    id: [*:0]const u8,
    name: [*:0]const u8,
    uri: [*:0]const u8,
    access_token: [*:0]const u8,
};

export fn reel_plex_create(db: ?*database.Database) ?*PlexAuthWrapper {
    const d = db orelse return null;
    var s = settings.Settings.init(allocator, d);

    // Load or generate client_identifier
    const client_id = blk: {
        if (s.getString(settings.keys.client_identifier) catch null) |existing| {
            break :blk existing;
        }
        const new_id = plex_auth.PlexAuth.generateClientId(allocator) catch return null;
        s.setString(settings.keys.client_identifier, new_id) catch {};
        break :blk new_id;
    };

    const pw = allocator.create(PlexAuthWrapper) catch return null;
    pw.* = .{
        .http_client = http.HttpClient.init(allocator),
        .plex_auth = undefined,
        .plex_client = undefined,
        .client_identifier = client_id,
    };
    pw.plex_auth = plex_auth.PlexAuth.init(allocator, &pw.http_client, client_id);
    pw.plex_client = plex_client.PlexClient.init(allocator, &pw.http_client, client_id, null);
    return pw;
}

export fn reel_plex_destroy(pw: ?*PlexAuthWrapper) void {
    const p = pw orelse return;
    p.plex_auth.deinit();
    p.http_client.deinit();
    allocator.free(p.client_identifier);
    allocator.destroy(p);
}

export fn reel_plex_request_pin(pw: ?*PlexAuthWrapper) ?[*:0]const u8 {
    const p = pw orelse return null;
    const code = p.plex_auth.requestPin() catch |err| {
        std.log.err("reel_plex_request_pin failed: {}", .{err});
        return null;
    };
    return allocator.dupeZ(u8, code) catch null;
}

/// Returns the full auth URL to open in the browser (e.g. https://app.plex.tv/auth#?...).
/// Must be called after reel_plex_request_pin succeeds.
export fn reel_plex_get_auth_url(pw: ?*PlexAuthWrapper) ?[*:0]const u8 {
    const p = pw orelse return null;
    const code = p.plex_auth.pin_code orelse return null;
    const url = std.fmt.allocPrint(
        allocator,
        "https://app.plex.tv/auth#?clientID={s}&code={s}&context%5Bdevice%5D%5Bproduct%5D=Reel",
        .{ p.client_identifier, code },
    ) catch return null;
    // Append null terminator
    const url_z = allocator.dupeZ(u8, url) catch return null;
    allocator.free(url);
    return url_z;
}

export fn reel_plex_poll_pin(pw: ?*PlexAuthWrapper) ?[*:0]const u8 {
    const p = pw orelse return null;
    const token = (p.plex_auth.pollPin() catch |err| {
        std.log.err("reel_plex_poll_pin failed: {}", .{err});
        return null;
    }) orelse return null;
    std.log.info("reel_plex_poll_pin: got token", .{});
    // Also set the token on the plex_client for server discovery
    p.plex_client.setAuthToken(token);
    return allocator.dupeZ(u8, token) catch null;
}

export fn reel_plex_discover_servers(
    pw: ?*PlexAuthWrapper,
    out_ptr: ?[*]ReelPlexServerC,
    max: c_int,
) c_int {
    const p = pw orelse return -1;
    const out = out_ptr orelse return -1;
    if (max <= 0) return -1;

    const servers = p.plex_client.discoverServers() catch |err| {
        std.log.err("reel_plex_discover_servers failed: {}", .{err});
        return -1;
    };
    std.log.info("reel_plex_discover_servers: found {d} servers", .{servers.len});
    defer {
        for (servers) |srv| {
            allocator.free(srv.name);
            allocator.free(srv.machine_identifier);
            if (srv.access_token) |t| allocator.free(t);
            for (srv.connections) |conn| allocator.free(conn.uri);
            allocator.free(srv.connections);
        }
        allocator.free(servers);
    }

    const count: usize = @min(servers.len, @as(usize, @intCast(max)));
    for (servers[0..count], 0..) |srv, i| {
        // Pick best URI: prefer first non-local HTTPS connection, fall back to first connection
        const uri = blk: {
            for (srv.connections) |conn| {
                if (!conn.local) break :blk conn.uri;
            }
            if (srv.connections.len > 0) break :blk srv.connections[0].uri;
            break :blk "";
        };

        out[i] = .{
            .id = @ptrCast(allocator.dupeZ(u8, srv.machine_identifier) catch return -1),
            .name = @ptrCast(allocator.dupeZ(u8, srv.name) catch return -1),
            .uri = @ptrCast(allocator.dupeZ(u8, uri) catch return -1),
            .access_token = @ptrCast(allocator.dupeZ(u8, srv.access_token orelse "") catch return -1),
        };
    }

    return @intCast(count);
}

// ── Plex Sync ──────────────────────────────────────────

/// Sync a Plex server's libraries into the local media_items table.
/// Fetches all library sections, then all items in each section,
/// then children (seasons/episodes) for shows.
/// Returns the number of items synced, or -1 on error.
export fn reel_plex_sync(
    lib: ?*library.Library,
    server_id: [*:0]const u8,
    server_uri: [*:0]const u8,
    auth_token: [*:0]const u8,
) i32 {
    const l = lib orelse return -1;
    const sid = std.mem.span(server_id);
    const uri = std.mem.span(server_uri);
    const token = std.mem.span(auth_token);

    // Read the client_identifier from settings (needed for Plex headers)
    const client_id = blk: {
        var s = settings.Settings.init(allocator, l.db);
        break :blk (s.getString(settings.keys.client_identifier) catch null) orelse "reel-default";
    };

    var hc = http.HttpClient.init(allocator);
    defer hc.deinit();

    var client = plex_client.PlexClient.init(allocator, &hc, client_id, token);
    client.setServerUri(uri);

    // Get library sections
    const libs = client.getLibraries() catch |err| {
        std.log.err("plex_sync: getLibraries failed: {}", .{err});
        return -1;
    };
    defer {
        for (libs) |lb| {
            allocator.free(lb.key);
            allocator.free(lb.title);
            allocator.free(lb.library_type);
        }
        allocator.free(libs);
    }

    var total_synced: i32 = 0;
    const now = std.time.timestamp();

    for (libs) |lb| {
        // Get items in this section
        const items = client.getItems(lb.key) catch |err| {
            std.log.err("plex_sync: getItems for section '{s}' failed: {}", .{ lb.title, err });
            continue;
        };
        defer client.freeMediaItems(items);

        for (items) |plex_item| {
            const mt = plexTypeToMediaType(plex_item.media_type);

            // Construct poster/art URLs from Plex thumb paths
            const poster_url = if (plex_item.thumb) |thumb|
                std.fmt.allocPrint(allocator, "{s}{s}?X-Plex-Token={s}", .{ uri, thumb, token }) catch null
            else
                null;
            defer if (poster_url) |p| allocator.free(p);

            const art_url = if (plex_item.art) |art|
                std.fmt.allocPrint(allocator, "{s}{s}?X-Plex-Token={s}", .{ uri, art, token }) catch null
            else
                null;
            defer if (art_url) |a| allocator.free(a);

            // Check if already exists
            const existing = l.getBySourceId(.plex, plex_item.rating_key) catch null;
            if (existing) |e| {
                l.freeMediaItem(e);
                continue; // Already synced
            }

            const parent_id: ?i64 = if (plex_item.parent_rating_key) |prk| blk: {
                const parent = l.getBySourceId(.plex, prk) catch null;
                if (parent) |p| {
                    defer l.freeMediaItem(p);
                    break :blk p.id;
                }
                break :blk null;
            } else null;

            const item_id = l.insertMediaItem(.{
                .source = .plex,
                .source_id = plex_item.rating_key,
                .server_id = sid,
                .media_type = mt,
                .title = plex_item.title,
                .year = plex_item.year,
                .summary = plex_item.summary,
                .rating = plex_item.rating,
                .duration_ms = plex_item.duration_ms,
                .poster_path = poster_url,
                .backdrop_path = art_url,
                .parent_id = parent_id,
                .season_number = plex_item.parent_index,
                .episode_number = plex_item.index,
                .added_at = now,
                .updated_at = now,
            }) catch |err| {
                std.log.err("plex_sync: insertMediaItem failed: {}", .{err});
                continue;
            };
            total_synced += 1;

            // For shows, fetch seasons and episodes
            if (mt == .show) {
                syncChildren(l, &client, plex_item.rating_key, item_id, sid, uri, token, now) catch |err| {
                    std.log.err("plex_sync: syncChildren failed for '{s}': {}", .{ plex_item.title, err });
                };
            }
        }
    }

    std.log.info("plex_sync: synced {d} items from server '{s}'", .{ total_synced, sid });
    return total_synced;
}

fn syncChildren(
    l: *library.Library,
    client: *plex_client.PlexClient,
    parent_rating_key: []const u8,
    parent_db_id: i64,
    server_id: []const u8,
    uri: []const u8,
    token: []const u8,
    now: i64,
) !void {
    const children = client.getChildren(parent_rating_key) catch return;
    defer client.freeMediaItems(children);

    for (children) |child| {
        const mt = plexTypeToMediaType(child.media_type);

        // Check if already exists
        const existing = l.getBySourceId(.plex, child.rating_key) catch null;
        if (existing) |e| {
            l.freeMediaItem(e);
            continue;
        }

        const poster_url = if (child.thumb) |thumb|
            std.fmt.allocPrint(allocator, "{s}{s}?X-Plex-Token={s}", .{ uri, thumb, token }) catch null
        else
            null;
        defer if (poster_url) |p| allocator.free(p);

        const child_id = l.insertMediaItem(.{
            .source = .plex,
            .source_id = child.rating_key,
            .server_id = server_id,
            .media_type = mt,
            .title = child.title,
            .year = child.year,
            .summary = child.summary,
            .rating = child.rating,
            .duration_ms = child.duration_ms,
            .poster_path = poster_url,
            .parent_id = parent_db_id,
            .season_number = child.parent_index orelse child.index,
            .episode_number = child.index,
            .added_at = now,
            .updated_at = now,
        }) catch continue;

        // Recurse for seasons → episodes
        if (mt == .season) {
            syncChildren(l, client, child.rating_key, child_id, server_id, uri, token, now) catch {};
        }
    }
}

/// Sync all stored Plex servers. Reads auth tokens from the servers table.
/// Returns total items synced across all servers, or -1 on error.
export fn reel_plex_sync_all(lib: ?*library.Library) i32 {
    const l = lib orelse return -1;
    const servers = l.listServers() catch return -1;
    defer l.freeServers(servers);

    var total: i32 = 0;
    for (servers) |srv| {
        const uri = srv.connection_uri orelse continue;
        const token = srv.auth_token orelse continue;
        const result = reel_plex_sync(lib, @ptrCast(allocator.dupeZ(u8, srv.id) catch continue), @ptrCast(allocator.dupeZ(u8, uri) catch continue), @ptrCast(allocator.dupeZ(u8, token) catch continue));
        if (result > 0) total += result;
    }
    return total;
}

fn plexTypeToMediaType(plex_type: []const u8) types.MediaType {
    if (std.mem.eql(u8, plex_type, "movie")) return .movie;
    if (std.mem.eql(u8, plex_type, "show")) return .show;
    if (std.mem.eql(u8, plex_type, "season")) return .season;
    if (std.mem.eql(u8, plex_type, "episode")) return .episode;
    return .other;
}

// Server management

const ReelServerC = extern struct {
    id: [*:0]const u8,
    name: [*:0]const u8,
    connection_uri: [*:0]const u8,
};

export fn reel_server_upsert(
    lib: ?*library.Library,
    id: [*:0]const u8,
    name: [*:0]const u8,
    connection_uri: [*:0]const u8,
    auth_token: ?[*:0]const u8,
) c_int {
    const l = lib orelse return -1;
    l.upsertServer(.{
        .id = std.mem.span(id),
        .name = std.mem.span(name),
        .client_identifier = std.mem.span(id),
        .auth_token = if (auth_token) |t| std.mem.span(t) else null,
        .connection_uri = std.mem.span(connection_uri),
        .last_connected_at = std.time.timestamp(),
    }) catch return -1;
    return 0;
}

export fn reel_server_delete(lib: ?*library.Library, id: [*:0]const u8) c_int {
    const l = lib orelse return -1;
    l.deleteServer(std.mem.span(id)) catch return -1;
    return 0;
}

export fn reel_server_list(
    lib: ?*library.Library,
    out_ptr: ?[*]ReelServerC,
    out_count: ?*i32,
) c_int {
    const l = lib orelse return -1;
    const count_ptr = out_count orelse return -1;

    const servers = l.listServers() catch return -1;

    if (out_ptr == null) {
        count_ptr.* = @intCast(servers.len);
        l.freeServers(servers);
        return 0;
    }

    const out = out_ptr.?;
    for (servers, 0..) |srv, i| {
        out[i] = .{
            .id = @ptrCast(allocator.dupeZ(u8, srv.id) catch {
                l.freeServers(servers);
                return -1;
            }),
            .name = @ptrCast(allocator.dupeZ(u8, srv.name) catch {
                l.freeServers(servers);
                return -1;
            }),
            .connection_uri = @ptrCast(allocator.dupeZ(u8, srv.connection_uri orelse "") catch {
                l.freeServers(servers);
                return -1;
            }),
        };
    }
    count_ptr.* = @intCast(servers.len);
    l.freeServers(servers);
    return 0;
}

// ── Library Queries ──────────────────────────────────────────

const ReelMediaItemC = extern struct {
    id: i64,
    media_type: c_int, // ReelMediaType
    source: c_int, // ReelMediaSource
    title: [*:0]const u8,
    sort_title: ?[*:0]const u8,
    year: i32,
    summary: ?[*:0]const u8,
    rating: f64,
    duration_ms: i64,
    poster_path: ?[*:0]const u8,
    backdrop_path: ?[*:0]const u8,
    parent_id: i64,
    season_number: i32,
    episode_number: i32,
    file_path: ?[*:0]const u8,
};

fn mediaTypeToInt(mt: types.MediaType) c_int {
    return switch (mt) {
        .movie => 0,
        .show => 1,
        .season => 2,
        .episode => 3,
        .other => 4,
    };
}

fn mediaSourceToInt(ms: types.MediaSource) c_int {
    return switch (ms) {
        .plex => 0,
        .local => 1,
    };
}

fn intToSortField(val: c_int) ?library.Library.SortField {
    return switch (val) {
        0 => .title,
        1 => .year,
        2 => .rating,
        3 => .added_at,
        else => null,
    };
}

fn intToSortOrder(val: c_int) ?library.Library.SortOrder {
    return switch (val) {
        0 => .asc,
        1 => .desc,
        else => null,
    };
}

fn dupeZOpt(s: ?[]const u8) ?[*:0]const u8 {
    const str = s orelse return null;
    return @ptrCast(allocator.dupeZ(u8, str) catch return null);
}

fn dupeZReq(s: []const u8) ?[*:0]const u8 {
    return @ptrCast(allocator.dupeZ(u8, s) catch return null);
}

fn convertMediaItem(item: types.MediaItem) ?ReelMediaItemC {
    return .{
        .id = item.id,
        .media_type = mediaTypeToInt(item.media_type),
        .source = mediaSourceToInt(item.source),
        .title = dupeZReq(item.title) orelse return null,
        .sort_title = dupeZOpt(item.sort_title),
        .year = item.year orelse 0,
        .summary = dupeZOpt(item.summary),
        .rating = item.rating orelse 0,
        .duration_ms = item.duration_ms orelse 0,
        .poster_path = dupeZOpt(item.poster_path),
        .backdrop_path = dupeZOpt(item.backdrop_path),
        .parent_id = item.parent_id orelse 0,
        .season_number = item.season_number orelse 0,
        .episode_number = item.episode_number orelse 0,
        .file_path = dupeZOpt(item.file_path),
    };
}

fn fillMediaItems(l: *library.Library, items: []types.MediaItem, out: [*]ReelMediaItemC, max: usize) i32 {
    const count = @min(items.len, max);
    for (items[0..count], 0..) |item, i| {
        out[i] = convertMediaItem(item) orelse {
            l.freeMediaItems(items);
            return -1;
        };
    }
    l.freeMediaItems(items);
    return @intCast(count);
}

export fn reel_library_get_items(
    lib: ?*library.Library,
    media_type: c_int,
    sort_by: c_int,
    sort_order: c_int,
    limit: i32,
    offset: i32,
    out_items: ?[*]ReelMediaItemC,
    max_items: i32,
) i32 {
    const l = lib orelse return -1;
    const out = out_items orelse return -1;
    const mt = intToMediaType(media_type) orelse return -1;
    const sf = intToSortField(sort_by) orelse return -1;
    const so = intToSortOrder(sort_order) orelse return -1;
    if (limit < 0 or offset < 0 or max_items <= 0) return -1;

    const items = l.getItemsByType(mt, sf, so, @intCast(limit), @intCast(offset)) catch return -1;
    return fillMediaItems(l, items, out, @intCast(max_items));
}

export fn reel_library_get_recently_added(
    lib: ?*library.Library,
    limit: i32,
    out_items: ?[*]ReelMediaItemC,
    max_items: i32,
) i32 {
    const l = lib orelse return -1;
    const out = out_items orelse return -1;
    if (limit <= 0 or max_items <= 0) return -1;

    const items = l.getRecentlyAdded(@intCast(limit)) catch return -1;
    return fillMediaItems(l, items, out, @intCast(max_items));
}

export fn reel_library_get_continue_watching(
    lib: ?*library.Library,
    limit: i32,
    out_items: ?[*]ReelMediaItemC,
    max_items: i32,
) i32 {
    const l = lib orelse return -1;
    const out = out_items orelse return -1;
    if (limit <= 0 or max_items <= 0) return -1;

    const items = l.getContinueWatching(@intCast(limit)) catch return -1;
    return fillMediaItems(l, items, out, @intCast(max_items));
}

export fn reel_library_search(
    lib: ?*library.Library,
    query: [*:0]const u8,
    out_items: ?[*]ReelMediaItemC,
    max_items: i32,
) i32 {
    const l = lib orelse return -1;
    const out = out_items orelse return -1;
    if (max_items <= 0) return -1;

    const items = l.searchItems(std.mem.span(query), null) catch return -1;
    return fillMediaItems(l, items, out, @intCast(max_items));
}

export fn reel_library_get_item(
    lib: ?*library.Library,
    id: i64,
    out_item: ?*ReelMediaItemC,
) c_int {
    const l = lib orelse return -1;
    const out = out_item orelse return -1;
    const item = (l.getMediaItem(id) catch return -1) orelse return -7; // REEL_ERR_NOT_FOUND
    defer l.freeMediaItem(item);
    out.* = convertMediaItem(item) orelse return -1;
    return 0;
}

export fn reel_library_get_items_by_parent(
    lib: ?*library.Library,
    parent_id: i64,
    out_items: ?[*]ReelMediaItemC,
    max_items: i32,
) i32 {
    const l = lib orelse return -1;
    const out = out_items orelse return -1;
    if (max_items <= 0) return -1;

    const items = l.getItemsByParent(parent_id) catch return -1;
    return fillMediaItems(l, items, out, @intCast(max_items));
}

// ── Genre Queries ──────────────────────────────────────────

const ReelGenreC = extern struct {
    id: i64,
    name: [*:0]const u8,
};

export fn reel_library_get_genres(
    lib: ?*library.Library,
    out_ptr: ?[*]ReelGenreC,
    out_count: ?*i32,
) c_int {
    const l = lib orelse return -1;
    const count_ptr = out_count orelse return -1;

    const genres = l.getDistinctGenres() catch return -1;

    if (out_ptr == null) {
        count_ptr.* = @intCast(genres.len);
        l.freeGenres(genres);
        return 0;
    }

    const out = out_ptr.?;
    for (genres, 0..) |g, i| {
        out[i] = .{
            .id = g.id,
            .name = dupeZReq(g.name) orelse {
                l.freeGenres(genres);
                return -1;
            },
        };
    }
    count_ptr.* = @intCast(genres.len);
    l.freeGenres(genres);
    return 0;
}

export fn reel_library_get_items_by_genre(
    lib: ?*library.Library,
    genre_name: [*:0]const u8,
    limit: i32,
    out_items: ?[*]ReelMediaItemC,
    max_items: i32,
) i32 {
    const l = lib orelse return -1;
    const out = out_items orelse return -1;
    if (limit <= 0 or max_items <= 0) return -1;

    const items = l.getItemsByGenre(std.mem.span(genre_name), @intCast(limit)) catch return -1;
    return fillMediaItems(l, items, out, @intCast(max_items));
}

// ── Favorites List ──────────────────────────────────────────

const ReelFavoriteC = extern struct {
    id: i64,
    item_type: [*:0]const u8,
    item_id: [*:0]const u8,
    display_name: [*:0]const u8,
    sort_order: i32,
};

export fn reel_library_list_favorites(
    lib: ?*library.Library,
    out_ptr: ?[*]ReelFavoriteC,
    out_count: ?*i32,
) c_int {
    const l = lib orelse return -1;
    const count_ptr = out_count orelse return -1;

    const favs = l.listFavorites() catch return -1;

    if (out_ptr == null) {
        count_ptr.* = @intCast(favs.len);
        l.freeFavorites(favs);
        return 0;
    }

    const out = out_ptr.?;
    for (favs, 0..) |f, i| {
        out[i] = .{
            .id = f.id,
            .item_type = dupeZReq(f.item_type.toString()) orelse {
                l.freeFavorites(favs);
                return -1;
            },
            .item_id = dupeZReq(f.item_id) orelse {
                l.freeFavorites(favs);
                return -1;
            },
            .display_name = dupeZReq(f.display_name) orelse {
                l.freeFavorites(favs);
                return -1;
            },
            .sort_order = f.sort_order,
        };
    }
    count_ptr.* = @intCast(favs.len);
    l.freeFavorites(favs);
    return 0;
}

// ── Scan Paths List ──────────────────────────────────────────

const ReelScanPathC = extern struct {
    id: i64,
    path: [*:0]const u8,
};

export fn reel_library_list_scan_paths(
    lib: ?*library.Library,
    out_ptr: ?[*]ReelScanPathC,
    out_count: ?*i32,
) c_int {
    const l = lib orelse return -1;
    const count_ptr = out_count orelse return -1;

    const paths = l.listScanPaths() catch return -1;

    if (out_ptr == null) {
        count_ptr.* = @intCast(paths.len);
        l.freeScanPaths(paths);
        return 0;
    }

    const out = out_ptr.?;
    for (paths, 0..) |p, i| {
        out[i] = .{
            .id = p.id,
            .path = dupeZReq(p.path) orelse {
                l.freeScanPaths(paths);
                return -1;
            },
        };
    }
    count_ptr.* = @intCast(paths.len);
    l.freeScanPaths(paths);
    return 0;
}

// ── Watch Progress ──────────────────────────────────────────

const ReelWatchProgressC = extern struct {
    media_item_id: i64,
    position_ms: i64,
    duration_ms: i64, // 0 if unknown
    watched: c_int, // 0 or 1
};

export fn reel_library_get_watch_progress(
    lib: ?*library.Library,
    media_item_id: i64,
    out: ?*ReelWatchProgressC,
) c_int {
    const l = lib orelse return -1;
    const out_ptr = out orelse return -1;
    const wp = (l.getWatchProgress(media_item_id) catch return -1) orelse return -7;
    out_ptr.* = .{
        .media_item_id = wp.media_item_id,
        .position_ms = wp.position_ms,
        .duration_ms = wp.duration_ms orelse 0,
        .watched = if (wp.watched) 1 else 0,
    };
    return 0;
}

export fn reel_library_update_watch_progress(
    lib: ?*library.Library,
    media_item_id: i64,
    position_ms: i64,
    duration_ms: i64,
    watched: c_int,
) c_int {
    const l = lib orelse return -1;
    l.updateWatchProgress(.{
        .media_item_id = media_item_id,
        .position_ms = position_ms,
        .duration_ms = if (duration_ms > 0) duration_ms else null,
        .watched = watched != 0,
        .last_watched_at = std.time.timestamp(),
    }) catch return -1;
    return 0;
}

// ── Download List ──────────────────────────────────────────

const ReelDownloadC = extern struct {
    id: i64,
    media_item_id: i64,
    status: [*:0]const u8,
    downloaded_bytes: i64,
    total_bytes: i64, // 0 if unknown
    local_path: ?[*:0]const u8,
    error_message: ?[*:0]const u8,
};

export fn reel_download_list(
    dl: ?*downloader.Downloader,
    out_ptr: ?[*]ReelDownloadC,
    out_count: ?*i32,
) c_int {
    const d = dl orelse return -1;
    const count_ptr = out_count orelse return -1;

    const downloads = d.getAllDownloads() catch return -1;

    if (out_ptr == null) {
        count_ptr.* = @intCast(downloads.len);
        d.freeDownloads(downloads);
        return 0;
    }

    const out = out_ptr.?;
    for (downloads, 0..) |item, i| {
        out[i] = .{
            .id = item.id,
            .media_item_id = item.media_item_id,
            .status = dupeZReq(item.status.toString()) orelse {
                d.freeDownloads(downloads);
                return -1;
            },
            .downloaded_bytes = item.downloaded_bytes,
            .total_bytes = item.total_bytes orelse 0,
            .local_path = dupeZOpt(item.local_path),
            .error_message = dupeZOpt(item.error_message),
        };
    }
    count_ptr.* = @intCast(downloads.len);
    d.freeDownloads(downloads);
    return 0;
}

// ── Memory Management ──────────────────────────────────────────

export fn reel_free(ptr: ?*anyopaque) void {
    // Free a single allocation made by the C allocator
    // This is used for strings returned by various functions
    if (ptr) |p| {
        // We can't know the exact size, but for c_allocator (libc malloc),
        // we can just call free directly
        const c = @cImport(@cInclude("stdlib.h"));
        c.free(p);
    }
}

// ── Collection Items ──────────────────────────────────────────

export fn reel_collection_get_items(
    lib: ?*library.Library,
    collection_id: i64,
    out_items: ?[*]ReelMediaItemC,
    max_items: i32,
) i32 {
    const l = lib orelse return -1;
    const out = out_items orelse return -1;
    if (max_items <= 0) return -1;

    const items = l.getCollectionItems(collection_id) catch return -1;
    return fillMediaItems(l, items, out, @intCast(max_items));
}

// Helpers

fn intToMediaType(val: c_int) ?types.MediaType {
    return switch (val) {
        0 => .movie,
        1 => .show,
        2 => .season,
        3 => .episode,
        4 => .other,
        else => null,
    };
}

test {
    std.testing.refAllDecls(@This());
}
