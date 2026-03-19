const std = @import("std");
const c = @import("c.zig").c;
const app = @import("app.zig");
const http_mod = @import("../../net/http.zig");
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

        // Spawn background thread to test all non-relay connections in parallel
        const thread_ctx = allocator.create(ConnTestCtx) catch continue;
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
        // Copy all non-relay connection URIs
        var conn_count: usize = 0;
        for (server.connections) |conn| {
            if (!conn.relay and conn_count < ConnTestCtx.max_conns) {
                thread_ctx.uris[conn_count] = allocator.dupe(u8, conn.uri) catch continue;
                thread_ctx.is_local[conn_count] = conn.local;
                conn_count += 1;
            }
        }
        thread_ctx.count = conn_count;

        if (conn_count > 0) {
            _ = std.Thread.spawn(.{}, testAllConnections, .{thread_ctx}) catch {
                freeConnTestCtx(thread_ctx);
            };
        } else {
            freeConnTestCtx(thread_ctx);
        }
    }

    setStatus("Syncing library...", true);
    _ = std.Thread.spawn(.{}, syncLibraryBackground, .{}) catch {
        setStatus("Failed to start sync.", false);
    };
    return 0;
}

const ConnTestCtx = struct {
    server_id: []const u8,
    client_id: []const u8,
    server_token: []const u8,
    uris: [max_conns][]const u8 = undefined,
    is_local: [max_conns]bool = undefined,
    count: usize = 0,
    const max_conns = 16;
};

fn freeConnTestCtx(ctx: *ConnTestCtx) void {
    const allocator = app.getAllocator();
    for (ctx.uris[0..ctx.count]) |uri| allocator.free(uri);
    allocator.free(ctx.server_id);
    allocator.free(ctx.client_id);
    allocator.free(ctx.server_token);
    allocator.destroy(ctx);
}

/// Shared state for parallel connection testing. Each thread that succeeds
/// immediately upgrades the server URI if it's better than what's been found so far.
const ConnBest = struct {
    mutex: std.Thread.Mutex = .{},
    best_latency: i32 = std.math.maxInt(i32),
    best_is_local: bool = false,
    found: bool = false,
};

/// Background thread: test all non-relay connections in parallel.
/// Each successful test immediately upgrades the server URI (no waiting for all to finish).
fn testAllConnections(ctx: *ConnTestCtx) void {
    defer freeConnTestCtx(ctx);
    std.log.info("testAllConnections: testing {d} non-relay connections", .{ctx.count});
    if (ctx.count == 0) {
        std.log.info("testAllConnections: no connections to test, staying on relay", .{});
        pending_conn_status = .relay;
        _ = c.g_idle_add(&updateConnectionStatusUI, null);
        return;
    }

    var best = ConnBest{};
    var threads: [ConnTestCtx.max_conns]?std.Thread = .{null} ** ConnTestCtx.max_conns;

    for (0..ctx.count) |i| {
        threads[i] = std.Thread.spawn(.{}, testSingleConnection, .{
            ctx, i, &best,
        }) catch null;
    }

    for (0..ctx.count) |i| {
        if (threads[i]) |t| t.join();
    }

    if (!best.found) {
        std.log.info("No direct connections reachable for {s}, staying on relay", .{ctx.server_id});
        pending_conn_status = .relay;
        _ = c.g_idle_add(&updateConnectionStatusUI, null);
    }
}

fn testSingleConnection(ctx: *ConnTestCtx, idx: usize, best: *ConnBest) void {
    const allocator = app.getAllocator();
    const http_client = app.getHttpClient() orelse return;

    var header_buf: [8]plex_types.Header = undefined;
    const plex_headers = plex_types.PlexHeaders{
        .client_identifier = ctx.client_id,
        .auth_token = ctx.server_token,
    };
    const h = plex_headers.toHeaders(&header_buf);

    var selector = connection_selector.ConnectionSelector.init(allocator, http_client, h);
    if (selector.testConnection(ctx.uris[idx], ctx.is_local[idx], false)) |result| {
        std.log.info("Connection reachable: {s} (latency={d}ms)", .{ ctx.uris[idx], result.latency_ms });

        best.mutex.lock();
        defer best.mutex.unlock();

        const is_local = ctx.is_local[idx];
        // Upgrade if: first result, or local beats non-local, or lower latency in same category
        if (!best.found or
            (is_local and !best.best_is_local) or
            (is_local == best.best_is_local and result.latency_ms < best.best_latency))
        {
            best.best_latency = result.latency_ms;
            best.best_is_local = is_local;
            best.found = true;

            std.log.info("Upgrading server connection to: {s}", .{ctx.uris[idx]});
            const lib = app.getLibrary() orelse return;
            lib.upsertServer(.{
                .id = ctx.server_id,
                .name = ctx.server_id,
                .client_identifier = ctx.server_id,
                .auth_token = ctx.server_token,
                .connection_uri = ctx.uris[idx],
            }) catch |err| {
                std.log.err("Failed to upgrade connection: {}", .{err});
                return;
            };

            // Update UI on the main thread
            pending_conn_status = .direct;
            _ = c.g_idle_add(&updateConnectionStatusUI, null);
        }
    } else {
        std.log.info("Connection unreachable: {s}", .{ctx.uris[idx]});
    }
}

var pending_conn_status: app.ConnectionStatus = .connecting;

fn updateConnectionStatusUI(_: ?*anyopaque) callconv(.c) c.gboolean {
    std.log.info("updateConnectionStatusUI: setting status to {s}", .{@tagName(pending_conn_status)});
    app.setConnectionStatus(pending_conn_status);
    return 0; // G_SOURCE_REMOVE
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

        // Get disabled libraries setting
        const disabled_str = blk: {
            if (app.getSettings()) |s| {
                if (s.getString("plex_disabled_libraries") catch null) |ds| break :blk ds;
            }
            break :blk @as(?[]const u8, null);
        };
        defer if (disabled_str) |ds| allocator.free(ds);

        for (libraries) |section| {
            // Skip disabled libraries
            if (disabled_str) |ds| {
                var iter = std.mem.splitScalar(u8, ds, ',');
                var skip = false;
                while (iter.next()) |entry| {
                    if (std.mem.eql(u8, entry, section.key)) {
                        skip = true;
                        break;
                    }
                }
                if (skip) {
                    std.log.info("Skipping disabled library: {s} (key={s})", .{ section.title, section.key });
                    continue;
                }
            }
            std.log.info("Syncing library: {s} (key={s})", .{ section.title, section.key });
            const items = plex.getItems(section.key) catch |err| {
                std.log.err("Failed to get items for {s}: {}", .{ section.title, err });
                continue;
            };
            defer plex.freeMediaItems(items);

            for (items) |item| {
                if (lib.getBySourceId(.plex, item.rating_key) catch null) |existing| {
                    // Backfill library_section if missing
                    if (existing.library_section == null) {
                        lib.setLibrarySection(existing.id, section.key) catch {};
                    }
                    lib.freeMediaItem(existing);
                    continue;
                }

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
                    .file_path = item.part_key,
                    .poster_path = poster_url,
                    .backdrop_path = backdrop_url,
                    .duration_ms = item.duration_ms,
                    .library_section = section.key,
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
    app.setSyncing(true);
    _ = std.Thread.spawn(.{}, syncAllWorker, .{}) catch |err| {
        std.log.err("Failed to start background sync: {}", .{err});
        app.setSyncing(false);
    };
}

fn syncAllWorker() void {
    const count = doSyncLibrary();
    std.log.info("Background sync complete: {d} items synced", .{count});
    _ = c.g_idle_add(&onSyncDone, null);
}

fn onSyncDone(_: ?*anyopaque) callconv(.c) c.gboolean {
    app.setSyncing(false);
    return 0;
}

/// Called at app startup: discover servers from Plex API, save connections,
/// then test all non-relay connections in parallel to find the best one.
pub fn refreshServerConnections() void {
    _ = std.Thread.spawn(.{}, refreshServerConnectionsWorker, .{}) catch |err| {
        std.log.err("Failed to start connection refresh: {}", .{err});
    };
}

fn refreshServerConnectionsWorker() void {
    const allocator = app.getAllocator();
    const lib = app.getLibrary() orelse return;
    const http_client = app.getHttpClient() orelse return;

    const client_id = getOrCreateClientId(allocator) orelse return;
    defer allocator.free(client_id);

    // Get auth token from first stored server
    const servers = lib.listServers() catch return;
    defer lib.freeServers(servers);

    if (servers.len == 0) return;
    const auth_token = servers[0].auth_token orelse return;

    // Discover servers from Plex API (always fresh)
    var plex = plex_client_mod.PlexClient.init(allocator, http_client, client_id, auth_token);
    const discovered = plex.discoverServers() catch |err| {
        std.log.err("refreshServerConnections: discovery failed: {}", .{err});
        // Fall back to stored servers
        setStatusFromStoredServers(lib);
        return;
    };
    defer {
        for (discovered) |server| {
            allocator.free(server.name);
            allocator.free(server.machine_identifier);
            if (server.access_token) |t| allocator.free(t);
            for (server.connections) |conn| allocator.free(conn.uri);
            allocator.free(server.connections);
        }
        allocator.free(discovered);
    }

    std.log.info("refreshServerConnections: discovered {d} servers", .{discovered.len});

    for (discovered) |server| {
        const server_token = server.access_token orelse auth_token;

        // Build connection list for DB
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

        // Use relay immediately so the app is functional right away
        const immediate_uri = connection_selector.ConnectionSelector.selectImmediate(db_connections) orelse continue;

        // Save server (keep existing URI if it was already upgraded to direct)
        const existing_uri = blk: {
            const existing = lib.getServer(server.machine_identifier) catch break :blk null;
            if (existing) |s| {
                defer lib.freeServer(s);
                if (s.connection_uri) |u| break :blk allocator.dupe(u8, u) catch null;
            }
            break :blk null;
        };
        defer if (existing_uri) |u| allocator.free(u);

        lib.upsertServer(.{
            .id = server.machine_identifier,
            .name = server.name,
            .client_identifier = server.machine_identifier,
            .auth_token = server_token,
            .connection_uri = existing_uri orelse immediate_uri,
        }) catch |err| {
            std.log.err("Failed to save server: {}", .{err});
            continue;
        };

        // Save all connections
        lib.upsertServerConnections(server.machine_identifier, db_connections) catch {};

        std.log.info("Saved server: {s} with {d} connections", .{ server.name, server.connections.len });

        // Spawn connection testing threads
        const test_client_id = allocator.dupe(u8, client_id) catch continue;
        const thread_ctx = allocator.create(ConnTestCtx) catch {
            allocator.free(test_client_id);
            continue;
        };
        thread_ctx.* = .{
            .server_id = allocator.dupe(u8, server.machine_identifier) catch {
                allocator.free(test_client_id);
                allocator.destroy(thread_ctx);
                continue;
            },
            .client_id = test_client_id,
            .server_token = allocator.dupe(u8, server_token) catch {
                allocator.free(thread_ctx.server_id);
                allocator.free(test_client_id);
                allocator.destroy(thread_ctx);
                continue;
            },
        };

        var conn_count: usize = 0;
        for (server.connections) |conn| {
            if (!conn.relay and conn_count < ConnTestCtx.max_conns) {
                thread_ctx.uris[conn_count] = allocator.dupe(u8, conn.uri) catch continue;
                thread_ctx.is_local[conn_count] = conn.local;
                conn_count += 1;
            }
        }
        thread_ctx.count = conn_count;

        if (conn_count > 0) {
            testAllConnections(thread_ctx);
        } else {
            // Only relay available
            pending_conn_status = .relay;
            _ = c.g_idle_add(&updateConnectionStatusUI, null);
            freeConnTestCtx(thread_ctx);
        }
    }
}

/// Set connection status from stored server data (when discovery fails).
const library_mod = @import("../../core/library.zig");

fn setStatusFromStoredServers(lib: *library_mod.Library) void {
    const servers = lib.listServers() catch return;
    defer lib.freeServers(servers);
    if (servers.len > 0 and servers[0].connection_uri != null) {
        pending_conn_status = .direct;
    } else {
        pending_conn_status = .offline;
    }
    _ = c.g_idle_add(&updateConnectionStatusUI, null);
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
