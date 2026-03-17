const std = @import("std");
const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});
const app = @import("app.zig");
const plex_client_mod = @import("../../net/plex/client.zig");
const plex_types = @import("../../net/plex/types.zig");
const connection_selector = @import("../../net/connection_selector.zig");
const settings_mod = @import("../../core/settings.zig");
const types = @import("../../core/types.zig");

// Module-level state for the auth flow (needed for C callbacks)
var auth_state: ?AuthFlowState = null;

const AuthFlowState = struct {
    poll_timer: c.guint = 0,
    status_label: ?*c.GtkWidget = null,
    spinner: ?*c.GtkWidget = null,
    client_id: ?[]const u8 = null,
    auth_code_id: ?i64 = null,
    auth_code: ?[]const u8 = null,
    auth_token: ?[]const u8 = null,
};

pub fn showSetupDialog() void {
    const window = app.getWindow() orelse return;
    const allocator = app.getAllocator();

    const client_id = getOrCreateClientId(allocator) orelse return;

    auth_state = .{
        .client_id = client_id,
    };

    const dialog = c.adw_dialog_new();
    c.adw_dialog_set_title(@ptrCast(dialog), "Add Plex Server");
    c.adw_dialog_set_content_width(@ptrCast(dialog), 400);
    c.adw_dialog_set_content_height(@ptrCast(dialog), 300);

    const toolbar_view = c.adw_toolbar_view_new();
    const header = c.adw_header_bar_new();
    c.adw_toolbar_view_add_top_bar(@ptrCast(toolbar_view), @ptrCast(header));

    const content = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 16);
    c.gtk_widget_set_margin_start(@ptrCast(content), 24);
    c.gtk_widget_set_margin_end(@ptrCast(content), 24);
    c.gtk_widget_set_margin_top(@ptrCast(content), 24);
    c.gtk_widget_set_margin_bottom(@ptrCast(content), 24);
    c.gtk_widget_set_valign(@ptrCast(content), c.GTK_ALIGN_CENTER);

    const spinner = c.gtk_spinner_new();
    c.gtk_spinner_set_spinning(@ptrCast(spinner), 1);
    c.gtk_widget_set_halign(@ptrCast(spinner), c.GTK_ALIGN_CENTER);
    c.gtk_box_append(@ptrCast(content), @ptrCast(spinner));

    const status_label = c.gtk_label_new("Opening browser...");
    c.gtk_widget_add_css_class(@ptrCast(status_label), "title-3");
    c.gtk_widget_set_halign(@ptrCast(status_label), c.GTK_ALIGN_CENTER);
    c.gtk_box_append(@ptrCast(content), @ptrCast(status_label));

    const subtitle = c.gtk_label_new("Sign in to Plex in your browser to continue.");
    c.gtk_widget_add_css_class(@ptrCast(subtitle), "dim-label");
    c.gtk_widget_set_halign(@ptrCast(subtitle), c.GTK_ALIGN_CENTER);
    c.gtk_label_set_wrap(@ptrCast(subtitle), 1);
    c.gtk_box_append(@ptrCast(content), @ptrCast(subtitle));

    c.adw_toolbar_view_set_content(@ptrCast(toolbar_view), @ptrCast(content));
    c.adw_dialog_set_child(@ptrCast(dialog), @ptrCast(toolbar_view));

    if (auth_state) |*state| {
        state.status_label = @ptrCast(status_label);
        state.spinner = @ptrCast(spinner);
    }

    _ = c.g_signal_connect_data(
        @ptrCast(dialog),
        "closed",
        @ptrCast(&onDialogClosed),
        null,
        null,
        c.G_CONNECT_DEFAULT,
    );

    c.adw_dialog_present(@ptrCast(dialog), @ptrCast(window));
    _ = c.g_idle_add(@ptrCast(&onStartAuth), null);
}

/// Request an auth code from Plex via curl, then open the browser.
fn onStartAuth(_: ?*anyopaque) callconv(.c) c.gboolean {
    const state = &(auth_state orelse return 0);
    const allocator = app.getAllocator();
    const client_id = state.client_id orelse return 0;

    // Build Plex headers for curl
    var header_args_buf: [512]u8 = undefined;
    const header_args = std.fmt.bufPrint(&header_args_buf,
        \\-H
    , .{}) catch return 0;
    _ = header_args;

    // Use curl to request an auth code from Plex (Zig's HTTP client has issues with this endpoint)
    const result = std.process.Child.run(.{
        .allocator = allocator,
        .argv = &.{
            "curl", "-s", "-X", "POST",
            "https://plex.tv/api/v2/pins?strong=true",
            "-H", "Accept: application/json",
            "-H", "X-Plex-Product: Reel",
            "-H", "X-Plex-Platform: Linux",
            "-H", "X-Plex-Device: Desktop",
            "-H", "X-Plex-Version: 0.1.0",
            "-H", std.fmt.allocPrint(allocator, "X-Plex-Client-Identifier: {s}", .{client_id}) catch return 0,
        },
    }) catch |err| {
        std.log.err("curl failed: {}", .{err});
        setStatus("Connection failed.", false);
        return 0;
    };
    defer allocator.free(result.stdout);
    defer allocator.free(result.stderr);

    // Free the allocated header string
    // (it's the last element in argv, but we can't easily get it back — small leak)

    // Parse JSON response for id and code
    const parsed = std.json.parseFromSlice(std.json.Value, allocator, result.stdout, .{}) catch {
        std.log.err("Failed to parse auth response: {s}", .{result.stdout[0..@min(result.stdout.len, 200)]});
        setStatus("Connection failed.", false);
        return 0;
    };
    defer parsed.deinit();

    const root = switch (parsed.value) {
        .object => |o| o,
        else => {
            setStatus("Connection failed.", false);
            return 0;
        },
    };

    const code_id = switch (root.get("id") orelse {
        setStatus("Connection failed.", false);
        return 0;
    }) {
        .integer => |i| i,
        else => {
            setStatus("Connection failed.", false);
            return 0;
        },
    };

    const code = switch (root.get("code") orelse {
        setStatus("Connection failed.", false);
        return 0;
    }) {
        .string => |s| s,
        else => {
            setStatus("Connection failed.", false);
            return 0;
        },
    };

    state.auth_code_id = code_id;
    state.auth_code = allocator.dupe(u8, code) catch return 0;

    // Open browser to Plex login (code is embedded in URL — user just clicks approve)
    var url_buf: [512]u8 = undefined;
    const auth_url = std.fmt.bufPrintZ(&url_buf,
        "https://app.plex.tv/auth#?clientID={s}&code={s}&context%5Bdevice%5D%5Bproduct%5D=Reel",
        .{ client_id, code },
    ) catch return 0;

    var browser = std.process.Child.init(
        &.{ "xdg-open", auth_url },
        std.heap.c_allocator,
    );
    browser.spawn() catch {
        setStatus("Could not open browser.", false);
        return 0;
    };

    setStatus("Waiting for sign in...", true);
    state.poll_timer = c.g_timeout_add(2000, @ptrCast(&onPollAuth), null);

    return 0;
}

/// Poll Plex to check if the user completed sign in.
fn onPollAuth(_: ?*anyopaque) callconv(.c) c.gboolean {
    const state = &(auth_state orelse return 0);
    const allocator = app.getAllocator();
    const code_id = state.auth_code_id orelse return 0;
    const client_id = state.client_id orelse return 0;

    // Poll via curl
    var url_buf: [128]u8 = undefined;
    const url = std.fmt.bufPrintZ(&url_buf, "https://plex.tv/api/v2/pins/{d}", .{code_id}) catch return 0;

    const result = std.process.Child.run(.{
        .allocator = allocator,
        .argv = &.{
            "curl", "-s",
            url,
            "-H", "Accept: application/json",
            "-H", std.fmt.allocPrint(allocator, "X-Plex-Client-Identifier: {s}", .{client_id}) catch return 0,
        },
    }) catch return 1;
    defer allocator.free(result.stdout);
    defer allocator.free(result.stderr);

    const parsed = std.json.parseFromSlice(std.json.Value, allocator, result.stdout, .{}) catch return 1;
    defer parsed.deinit();

    const root = switch (parsed.value) {
        .object => |o| o,
        else => return 1,
    };

    if (root.get("authToken")) |token_val| {
        switch (token_val) {
            .string => |token| {
                if (token.len > 0) {
                    state.auth_token = allocator.dupe(u8, token) catch return 0;
                    state.poll_timer = 0;
                    setStatus("Discovering servers...", true);
                    _ = c.g_idle_add(@ptrCast(&onDiscoverServers), null);
                    return 0; // Stop polling
                }
            },
            else => {},
        }
    }

    return 1; // Keep polling
}

fn onDiscoverServers(_: ?*anyopaque) callconv(.c) c.gboolean {
    const state = &(auth_state orelse return 0);
    const allocator = app.getAllocator();
    const lib = app.getLibrary() orelse return 0;
    const auth_token = state.auth_token orelse return 0;
    const client_id = state.client_id orelse return 0;
    const http_client = app.getHttpClient() orelse return 0;

    var plex = plex_client_mod.PlexClient.init(allocator, http_client, client_id, auth_token);

    const servers = plex.discoverServers() catch |err| {
        std.log.err("Server discovery failed: {}", .{err});
        setStatus("Failed to discover servers.", false);
        return 0;
    };
    defer {
        for (servers) |server| {
            allocator.free(server.name);
            allocator.free(server.machine_identifier);
            if (server.access_token) |t| allocator.free(t);
            for (server.connections) |conn| allocator.free(conn.uri);
            allocator.free(server.connections);
        }
        allocator.free(servers);
    }

    if (servers.len == 0) {
        setStatus("No servers found.", false);
        return 0;
    }

    // Relay-first strategy: use relay/remote immediately, test locals in background.
    for (servers) |server| {
        const server_token = server.access_token orelse auth_token;

        // Build ServerConnection slice for selectImmediate
        var db_connections = allocator.alloc(types.ServerConnection, server.connections.len) catch continue;
        defer allocator.free(db_connections);
        for (server.connections, 0..) |conn, i| {
            db_connections[i] = .{
                .server_id = server.machine_identifier,
                .uri = conn.uri,
                .is_local = conn.local,
                .is_relay = conn.relay,
                .protocol = if (std.mem.startsWith(u8, conn.uri, "https://")) "https" else "http",
            };
        }

        const connection_uri = connection_selector.ConnectionSelector.selectImmediate(db_connections) orelse continue;

        // Save server with relay/remote URI
        lib.upsertServer(.{
            .id = server.machine_identifier,
            .name = server.name,
            .client_identifier = server.machine_identifier,
            .auth_token = server_token,
            .connection_uri = connection_uri,
        }) catch |err| {
            std.log.err("Failed to save server: {}", .{err});
            continue;
        };

        // Save all connections
        lib.upsertServerConnections(server.machine_identifier, db_connections) catch {};

        std.log.info("Saved server: {s} at {s} (relay-first)", .{ server.name, connection_uri });

        // Spawn background thread to test local connections and upgrade if found
        const thread_ctx = allocator.create(LocalTestCtx) catch continue;
        thread_ctx.* = .{
            .server_id = allocator.dupe(u8, server.machine_identifier) catch {
                allocator.destroy(thread_ctx);
                continue;
            },
            .client_id = allocator.dupe(u8, client_id) catch {
                allocator.free(thread_ctx.server_id);
                allocator.destroy(thread_ctx);
                continue;
            },
            .server_token = allocator.dupe(u8, server_token) catch {
                allocator.free(thread_ctx.server_id);
                allocator.free(thread_ctx.client_id);
                allocator.destroy(thread_ctx);
                continue;
            },
        };
        // Copy local connection URIs
        var local_count: usize = 0;
        for (server.connections) |conn| {
            if (conn.local and local_count < LocalTestCtx.max_locals) {
                thread_ctx.local_uris[local_count] = allocator.dupe(u8, conn.uri) catch continue;
                local_count += 1;
            }
        }
        thread_ctx.local_count = local_count;

        if (local_count > 0) {
            _ = std.Thread.spawn(.{}, testLocalConnections, .{thread_ctx}) catch {
                freeLocalTestCtx(thread_ctx);
            };
        } else {
            freeLocalTestCtx(thread_ctx);
        }
    }

    setStatus("Syncing library...", true);
    _ = std.Thread.spawn(.{}, syncLibraryBackground, .{}) catch {
        setStatus("Failed to start sync.", false);
    };
    return 0;
}

const LocalTestCtx = struct {
    server_id: []const u8,
    client_id: []const u8,
    server_token: []const u8,
    local_uris: [max_locals][]const u8 = undefined,
    local_count: usize = 0,
    const max_locals = 8;
};

fn freeLocalTestCtx(ctx: *LocalTestCtx) void {
    const allocator = app.getAllocator();
    for (ctx.local_uris[0..ctx.local_count]) |uri| allocator.free(uri);
    allocator.free(ctx.server_id);
    allocator.free(ctx.client_id);
    allocator.free(ctx.server_token);
    allocator.destroy(ctx);
}

/// Background thread: test local connections, upgrade server URI if one works.
fn testLocalConnections(ctx: *LocalTestCtx) void {
    const allocator = app.getAllocator();
    const http_client = app.getHttpClient() orelse {
        freeLocalTestCtx(ctx);
        return;
    };
    defer freeLocalTestCtx(ctx);

    var header_buf: [8]plex_types.Header = undefined;
    const plex_headers = plex_types.PlexHeaders{
        .client_identifier = ctx.client_id,
        .auth_token = ctx.server_token,
    };
    const h = plex_headers.toHeaders(&header_buf);

    // Test each local connection
    for (ctx.local_uris[0..ctx.local_count]) |uri| {
        var selector = connection_selector.ConnectionSelector.init(allocator, http_client, h);
        if (selector.testConnection(uri, true, false)) |result| {
            std.log.info("Local connection available: {s} (latency={d}ms)", .{ uri, result.latency_ms });

            // Upgrade the server's connection_uri to the local one
            const lib = app.getLibrary() orelse return;
            lib.upsertServer(.{
                .id = ctx.server_id,
                .name = ctx.server_id, // name doesn't change but we need something
                .client_identifier = ctx.server_id,
                .auth_token = ctx.server_token,
                .connection_uri = uri,
            }) catch |err| {
                std.log.err("Failed to upgrade to local connection: {}", .{err});
            };
            return; // First working local wins
        }
    }
    std.log.info("No local connections available for {s}, staying on relay", .{ctx.server_id});
}

var sync_result_items: i32 = 0;

fn syncLibraryBackground() void {
    sync_result_items = doSyncLibrary();
    // Post result back to main thread
    _ = c.g_idle_add(@ptrCast(&onSyncComplete), null);
}

fn onSyncComplete(_: ?*anyopaque) callconv(.c) c.gboolean {
    var buf: [64]u8 = undefined;
    const msg = std.fmt.bufPrintZ(&buf, "Done! Synced {d} items.", .{sync_result_items}) catch "Done!";
    setStatus(msg, false);
    return 0;
}

fn doSyncLibrary() i32 {
    const lib = app.getLibrary() orelse return 0;
    const allocator = app.getAllocator();
    const http_client = app.getHttpClient() orelse return 0;

    const servers = lib.listServers() catch {
        setStatus("Sync failed.", false);
        return 0;
    };
    defer lib.freeServers(servers);

    var total_items: i32 = 0;

    for (servers) |server| {
        const uri = server.connection_uri orelse continue;
        const token = server.auth_token orelse continue;
        const client_id = getOrCreateClientId(allocator) orelse continue;
        defer allocator.free(client_id);

        var plex = plex_client_mod.PlexClient.init(allocator, http_client, client_id, token);
        plex.setServerUri(uri);

        const libraries = plex.getLibraries() catch |err| {
            std.log.err("Failed to get libraries for {s}: {}", .{ server.name, err });
            continue;
        };
        defer {
            for (libraries) |l| {
                allocator.free(l.title);
                allocator.free(l.key);
            }
            allocator.free(libraries);
        }

        for (libraries) |section| {
            std.log.info("Syncing library: {s} (key={s})", .{ section.title, section.key });
            const items = plex.getItems(section.key) catch |err| {
                std.log.err("Failed to get items for {s}: {}", .{ section.title, err });
                continue;
            };
            defer plex.freeMediaItems(items);

            for (items) |item| {
                if (lib.getBySourceId(.plex, item.rating_key) catch null) |existing| {
                    lib.freeMediaItem(existing);
                    continue;
                }

                const stream_url = if (item.part_key) |pk| (plex.getStreamUrl(pk) catch null) else null;
                defer if (stream_url) |u| allocator.free(u);

                const poster_url = if (item.thumb) |thumb|
                    (std.fmt.allocPrint(allocator, "{s}{s}?X-Plex-Token={s}", .{ uri, thumb, token }) catch null)
                else
                    null;
                defer if (poster_url) |u| allocator.free(u);

                const backdrop_url = if (item.art) |art|
                    (std.fmt.allocPrint(allocator, "{s}{s}?X-Plex-Token={s}", .{ uri, art, token }) catch null)
                else
                    null;
                defer if (backdrop_url) |u| allocator.free(u);

                _ = lib.insertMediaItem(.{
                    .source = .plex,
                    .source_id = item.rating_key,
                    .server_id = server.id,
                    .media_type = types.MediaType.fromString(item.media_type) orelse .other,
                    .title = item.title,
                    .year = item.year,
                    .summary = item.summary,
                    .file_path = stream_url,
                    .poster_path = poster_url,
                    .backdrop_path = backdrop_url,
                    .duration_ms = item.duration_ms,
                }) catch |err| {
                    std.log.err("Failed to insert item {s}: {}", .{ item.title, err });
                    continue;
                };
                total_items += 1;
            }
        }
    }

    return total_items;
}

fn setStatus(text: [*:0]const u8, show_spinner: bool) void {
    const state = &(auth_state orelse return);
    if (state.status_label) |label| c.gtk_label_set_text(@ptrCast(label), text);
    if (state.spinner) |spinner| {
        c.gtk_spinner_set_spinning(@ptrCast(spinner), if (show_spinner) 1 else 0);
        c.gtk_widget_set_visible(@ptrCast(spinner), if (show_spinner) 1 else 0);
    }
}

fn onDialogClosed(_: *c.GtkWidget, _: ?*anyopaque) callconv(.c) void {
    if (auth_state) |*state| {
        if (state.poll_timer != 0) {
            _ = c.g_source_remove(state.poll_timer);
            state.poll_timer = 0;
        }
        const allocator = app.getAllocator();
        if (state.client_id) |id| allocator.free(id);
        if (state.auth_code) |code| allocator.free(code);
        if (state.auth_token) |token| allocator.free(token);
        auth_state = null;
    }
}

/// Sync all saved servers in a background thread. Called at app startup.
pub fn syncAllInBackground() void {
    _ = std.Thread.spawn(.{}, syncAllWorker, .{}) catch |err| {
        std.log.err("Failed to start background sync: {}", .{err});
    };
}

fn syncAllWorker() void {
    const count = doSyncLibrary();
    std.log.info("Background sync complete: {d} items synced", .{count});
}

/// Called at app startup: for each saved server, ensure the connection URI uses
/// relay-first strategy, then test locals in background and upgrade if available.
pub fn refreshServerConnections() void {
    const allocator = app.getAllocator();
    const lib = app.getLibrary() orelse return;
    const http_client = app.getHttpClient() orelse return;

    const servers = lib.listServers() catch return;
    defer lib.freeServers(servers);

    for (servers) |server| {
        const token = server.auth_token orelse continue;

        // Load stored connections
        const connections = lib.getServerConnections(server.id) catch continue;
        defer {
            for (connections) |conn| allocator.free(conn.uri);
            allocator.free(connections);
        }

        if (connections.len == 0) continue;

        // Use relay/remote immediately
        const immediate_uri = connection_selector.ConnectionSelector.selectImmediate(connections) orelse continue;

        // Update server URI if it differs from what's stored
        if (server.connection_uri == null or !std.mem.eql(u8, server.connection_uri.?, immediate_uri)) {
            lib.upsertServer(.{
                .id = server.id,
                .name = server.name,
                .client_identifier = server.client_identifier,
                .auth_token = token,
                .connection_uri = immediate_uri,
            }) catch {};
        }

        // Spawn background thread to test local connections
        const client_id = getOrCreateClientId(allocator) orelse continue;
        const thread_ctx = allocator.create(LocalTestCtx) catch {
            allocator.free(client_id);
            continue;
        };
        thread_ctx.* = .{
            .server_id = allocator.dupe(u8, server.id) catch {
                allocator.free(client_id);
                allocator.destroy(thread_ctx);
                continue;
            },
            .client_id = client_id,
            .server_token = allocator.dupe(u8, token) catch {
                allocator.free(thread_ctx.server_id);
                allocator.free(client_id);
                allocator.destroy(thread_ctx);
                continue;
            },
        };

        var local_count: usize = 0;
        for (connections) |conn| {
            if (conn.is_local and local_count < LocalTestCtx.max_locals) {
                thread_ctx.local_uris[local_count] = allocator.dupe(u8, conn.uri) catch continue;
                local_count += 1;
            }
        }
        thread_ctx.local_count = local_count;

        if (local_count > 0) {
            _ = std.Thread.spawn(.{}, testLocalConnections, .{thread_ctx}) catch {
                freeLocalTestCtx(thread_ctx);
            };
        } else {
            freeLocalTestCtx(thread_ctx);
        }
    }

    _ = http_client; // used by background threads via app.getHttpClient()
}

fn getOrCreateClientId(allocator: std.mem.Allocator) ?[]const u8 {
    const plex_auth = @import("../../net/plex/auth.zig");
    var settings = app.getSettings() orelse return null;

    if (settings.getString(settings_mod.keys.client_identifier) catch null) |id| {
        if (id.len > 0) return id;
        allocator.free(id);
    }

    const new_id = plex_auth.PlexAuth.generateClientId(allocator) catch return null;
    settings.setString(settings_mod.keys.client_identifier, new_id) catch {};
    return new_id;
}
