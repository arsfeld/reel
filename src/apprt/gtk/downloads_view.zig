const std = @import("std");
const c = @cImport({
    @cInclude("adwaita.h");
    @cInclude("gtk/gtk.h");
});
const app = @import("app.zig");
const downloader_mod = @import("../../core/downloader.zig");
const types = @import("../../core/types.zig");
const library_mod = @import("../../core/library.zig");

pub const DownloadsView = struct {
    widget: *c.GtkWidget,
    content_stack: *c.GtkWidget,
    list_box: *c.GtkWidget,
    header_label: *c.GtkWidget,
    timer_id: c.guint = 0,
    // Track previous bytes for speed calculation
    last_poll_bytes: [downloader_mod.max_concurrent]u64 = .{0} ** downloader_mod.max_concurrent,

    pub fn init() DownloadsView {
        const content_stack = c.gtk_stack_new();
        c.gtk_widget_set_vexpand(@ptrCast(content_stack), 1);
        c.gtk_widget_set_hexpand(@ptrCast(content_stack), 1);

        // Empty state
        const status = c.adw_status_page_new();
        c.adw_status_page_set_icon_name(@ptrCast(status), "folder-download-symbolic");
        c.adw_status_page_set_title(@ptrCast(status), "No Downloads");
        c.adw_status_page_set_description(@ptrCast(status),
            "Download Plex items for offline viewing. Downloads will appear here.",
        );
        _ = c.gtk_stack_add_named(@ptrCast(content_stack), @ptrCast(status), "empty");

        // Downloads list view
        const vbox = c.gtk_box_new(c.GTK_ORIENTATION_VERTICAL, 0);
        c.gtk_widget_set_vexpand(@ptrCast(vbox), 1);

        // Header with storage info
        const header_box = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 8);
        c.gtk_widget_set_margin_start(@ptrCast(header_box), 16);
        c.gtk_widget_set_margin_end(@ptrCast(header_box), 16);
        c.gtk_widget_set_margin_top(@ptrCast(header_box), 12);
        c.gtk_widget_set_margin_bottom(@ptrCast(header_box), 4);

        const title = c.gtk_label_new("Downloads");
        c.gtk_widget_add_css_class(@ptrCast(title), "title-3");
        c.gtk_widget_set_halign(@ptrCast(title), c.GTK_ALIGN_START);
        c.gtk_box_append(@ptrCast(header_box), @ptrCast(title));

        const spacer = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 0);
        c.gtk_widget_set_hexpand(@ptrCast(spacer), 1);
        c.gtk_box_append(@ptrCast(header_box), spacer);

        const header_label = c.gtk_label_new("");
        c.gtk_widget_add_css_class(@ptrCast(header_label), "dim-label");
        c.gtk_box_append(@ptrCast(header_box), @ptrCast(header_label));

        c.gtk_box_append(@ptrCast(vbox), @ptrCast(header_box));

        // Scrollable list
        const scrolled = c.gtk_scrolled_window_new();
        c.gtk_widget_set_vexpand(@ptrCast(scrolled), 1);

        const clamp = c.adw_clamp_new();
        c.adw_clamp_set_maximum_size(@ptrCast(clamp), 800);

        const list_box = c.gtk_list_box_new();
        c.gtk_list_box_set_selection_mode(@ptrCast(list_box), c.GTK_SELECTION_NONE);
        c.gtk_widget_add_css_class(@ptrCast(list_box), "boxed-list");
        c.gtk_widget_set_margin_top(@ptrCast(list_box), 8);
        c.gtk_widget_set_margin_bottom(@ptrCast(list_box), 16);
        c.gtk_widget_set_margin_start(@ptrCast(list_box), 16);
        c.gtk_widget_set_margin_end(@ptrCast(list_box), 16);

        c.adw_clamp_set_child(@ptrCast(clamp), @ptrCast(list_box));
        c.gtk_scrolled_window_set_child(@ptrCast(scrolled), @ptrCast(clamp));
        c.gtk_box_append(@ptrCast(vbox), scrolled);

        _ = c.gtk_stack_add_named(@ptrCast(content_stack), @ptrCast(vbox), "list");

        c.gtk_stack_set_visible_child_name(@ptrCast(content_stack), "empty");

        var view = DownloadsView{
            .widget = @ptrCast(content_stack),
            .content_stack = @ptrCast(content_stack),
            .list_box = @ptrCast(list_box),
            .header_label = @ptrCast(header_label),
        };

        // Start a 500ms timer to poll downloads
        view.timer_id = c.g_timeout_add(500, @ptrCast(&onPollTimer), null);

        return view;
    }

    fn onPollTimer(_: ?*anyopaque) callconv(.c) c.gboolean {
        refreshGlobal();
        return 1; // G_SOURCE_CONTINUE
    }
};

// Global view pointer for the timer callback
var global_view: ?*DownloadsView = null;

pub fn setGlobalDownloadsView(v: *DownloadsView) void {
    global_view = v;
}

fn refreshGlobal() void {
    const view = global_view orelse return;
    var dl = app.getDownloader() orelse return;

    const downloads = dl.getAllDownloads() catch return;
    defer dl.freeDownloads(downloads);

    if (downloads.len == 0) {
        c.gtk_stack_set_visible_child_name(@ptrCast(view.content_stack), "empty");
        return;
    }

    c.gtk_stack_set_visible_child_name(@ptrCast(view.content_stack), "list");

    // Update storage header
    const total_bytes = dl.totalCompletedBytes() catch 0;
    var size_buf: [32]u8 = undefined;
    const size_str = formatBytes(&size_buf, @intCast(total_bytes));
    var header_buf: [64]u8 = undefined;
    const header_str = std.fmt.bufPrintZ(&header_buf, "{s} used", .{size_str}) catch "";
    c.gtk_label_set_text(@ptrCast(view.header_label), header_str);

    // Clear existing rows
    while (true) {
        const row = c.gtk_list_box_get_row_at_index(@ptrCast(view.list_box), 0);
        if (row == null) break;
        c.gtk_list_box_remove(@ptrCast(view.list_box), @ptrCast(row));
    }

    // Get library for title lookups
    const lib = app.getLibrary();

    // Add a row for each download
    for (downloads) |download| {
        const row = buildDownloadRow(download, lib);
        c.gtk_list_box_append(@ptrCast(view.list_box), row);
    }
}

fn buildDownloadRow(download: types.Download, lib: ?*library_mod.Library) *c.GtkWidget {
    const row = c.adw_action_row_new();

    // Look up the media item title
    var title_buf: [256]u8 = undefined;
    const title_str: [*:0]const u8 = blk: {
        if (lib) |l| {
            const item = l.getMediaItem(download.media_item_id) catch null orelse break :blk "Unknown";
            defer l.freeMediaItem(item);
            const len = @min(item.title.len, title_buf.len - 1);
            @memcpy(title_buf[0..len], item.title[0..len]);
            title_buf[len] = 0;
            break :blk @ptrCast(&title_buf);
        }
        break :blk "Unknown";
    };

    c.adw_preferences_row_set_title(@ptrCast(row), title_str);

    // Subtitle: status + size info
    var subtitle_buf: [128]u8 = undefined;
    const subtitle = buildSubtitle(&subtitle_buf, download);
    c.adw_action_row_set_subtitle(@ptrCast(row), subtitle);

    // Status icon prefix
    const icon_name: [*:0]const u8 = switch (download.status) {
        .downloading => "content-loading-symbolic",
        .queued => "content-loading-symbolic",
        .paused => "media-playback-pause-symbolic",
        .complete => "emblem-ok-symbolic",
        .failed => "dialog-error-symbolic",
    };
    const prefix_icon = c.gtk_image_new_from_icon_name(icon_name);
    c.adw_action_row_add_prefix(@ptrCast(row), @ptrCast(prefix_icon));

    // Action buttons as suffix
    const btn_box = c.gtk_box_new(c.GTK_ORIENTATION_HORIZONTAL, 4);
    c.gtk_widget_set_valign(@ptrCast(btn_box), c.GTK_ALIGN_CENTER);

    switch (download.status) {
        .downloading => {
            addActionButton(btn_box, "media-playback-pause-symbolic", "Pause", download.id, .pause);
            addActionButton(btn_box, "user-trash-symbolic", "Delete", download.id, .delete);
        },
        .paused => {
            addActionButton(btn_box, "media-playback-start-symbolic", "Resume", download.id, .resume_dl);
            addActionButton(btn_box, "user-trash-symbolic", "Delete", download.id, .delete);
        },
        .queued => {
            addActionButton(btn_box, "user-trash-symbolic", "Cancel", download.id, .delete);
        },
        .complete => {
            addActionButton(btn_box, "media-playback-start-symbolic", "Play", download.id, .play);
            addActionButton(btn_box, "user-trash-symbolic", "Delete", download.id, .delete);
        },
        .failed => {
            addActionButton(btn_box, "view-refresh-symbolic", "Retry", download.id, .retry);
            addActionButton(btn_box, "user-trash-symbolic", "Delete", download.id, .delete);
        },
    }

    c.adw_action_row_add_suffix(@ptrCast(row), @ptrCast(btn_box));

    return @ptrCast(row);
}

const ActionType = enum { pause, resume_dl, delete, play, retry };

fn addActionButton(
    container: *c.GtkWidget,
    icon_name: [*:0]const u8,
    tooltip: [*:0]const u8,
    download_id: i64,
    action: ActionType,
) void {
    const btn = c.gtk_button_new_from_icon_name(icon_name);
    c.gtk_widget_add_css_class(@ptrCast(btn), "flat");
    c.gtk_widget_set_tooltip_text(@ptrCast(btn), tooltip);

    // Store download_id and action in widget data
    c.g_object_set_data(@ptrCast(btn), "dl_id", @ptrFromInt(@as(usize, @intCast(download_id))));
    c.g_object_set_data(@ptrCast(btn), "dl_action", @ptrFromInt(@intFromEnum(action)));

    _ = c.g_signal_connect_data(
        @ptrCast(btn),
        "clicked",
        @ptrCast(&onActionClicked),
        null,
        null,
        c.G_CONNECT_DEFAULT,
    );

    c.gtk_box_append(@ptrCast(container), @ptrCast(btn));
}

fn onActionClicked(button: *c.GtkButton, _: ?*anyopaque) callconv(.c) void {
    const id_ptr = c.g_object_get_data(@ptrCast(button), "dl_id");
    const action_ptr = c.g_object_get_data(@ptrCast(button), "dl_action");
    if (id_ptr == null or action_ptr == null) return;

    const download_id: i64 = @intCast(@intFromPtr(id_ptr));
    const action: ActionType = @enumFromInt(@intFromPtr(action_ptr));

    var dl = app.getDownloader() orelse return;

    switch (action) {
        .pause => dl.pause(download_id) catch {},
        .resume_dl => dl.resumeDownload(download_id) catch {},
        .delete => dl.remove(download_id, true) catch {},
        .retry => dl.setStatus(download_id, .queued) catch {},
        .play => {
            // Get the local path and play it
            if (dl.getDownload(download_id) catch null) |download| {
                defer dl.freeDownload(download);
                if (download.local_path) |path| {
                    app.switchToPlayer(path);
                }
            }
        },
    }
}

fn buildSubtitle(buf: []u8, download: types.Download) [*:0]const u8 {
    var size_buf1: [16]u8 = undefined;
    var size_buf2: [16]u8 = undefined;

    const result = switch (download.status) {
        .downloading => blk: {
            const dl_str = formatBytes(&size_buf1, @intCast(download.downloaded_bytes));
            if (download.total_bytes) |total| {
                const total_str = formatBytes(&size_buf2, @intCast(total));
                const pct = if (total > 0) @divTrunc(download.downloaded_bytes * 100, total) else 0;
                break :blk std.fmt.bufPrintZ(buf, "{s} / {s} ({d}%)", .{ dl_str, total_str, pct });
            }
            break :blk std.fmt.bufPrintZ(buf, "{s} downloading...", .{dl_str});
        },
        .paused => blk: {
            const dl_str = formatBytes(&size_buf1, @intCast(download.downloaded_bytes));
            if (download.total_bytes) |total| {
                const total_str = formatBytes(&size_buf2, @intCast(total));
                break :blk std.fmt.bufPrintZ(buf, "{s} / {s} - Paused", .{ dl_str, total_str });
            }
            break :blk std.fmt.bufPrintZ(buf, "{s} - Paused", .{dl_str});
        },
        .queued => std.fmt.bufPrintZ(buf, "Queued", .{}),
        .complete => blk: {
            if (download.total_bytes) |total| {
                const total_str = formatBytes(&size_buf1, @intCast(total));
                break :blk std.fmt.bufPrintZ(buf, "{s} - Complete", .{total_str});
            }
            break :blk std.fmt.bufPrintZ(buf, "Complete", .{});
        },
        .failed => blk: {
            if (download.error_message) |msg| {
                const len = @min(msg.len, 60);
                break :blk std.fmt.bufPrintZ(buf, "Failed: {s}", .{msg[0..len]});
            }
            break :blk std.fmt.bufPrintZ(buf, "Failed", .{});
        },
    };

    return result catch "";
}

fn formatBytes(buf: []u8, bytes: u64) []const u8 {
    if (bytes >= 1024 * 1024 * 1024) {
        const gb = @as(f64, @floatFromInt(bytes)) / (1024.0 * 1024.0 * 1024.0);
        return std.fmt.bufPrint(buf, "{d:.1} GB", .{gb}) catch "?";
    } else if (bytes >= 1024 * 1024) {
        const mb = @as(f64, @floatFromInt(bytes)) / (1024.0 * 1024.0);
        return std.fmt.bufPrint(buf, "{d:.1} MB", .{mb}) catch "?";
    } else if (bytes >= 1024) {
        const kb = @as(f64, @floatFromInt(bytes)) / 1024.0;
        return std.fmt.bufPrint(buf, "{d:.0} KB", .{kb}) catch "?";
    } else {
        return std.fmt.bufPrint(buf, "{d} B", .{bytes}) catch "?";
    }
}
