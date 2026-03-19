const std = @import("std");
const c = @import("c.zig").c;
const tmdb_types = @import("../../net/tmdb/types.zig");
const image_cache = @import("../../core/image_cache.zig");
const http_mod = @import("../../net/http.zig");
const app = @import("app.zig");


// --- Download queue: fixed worker threads, bounded queue ---

const max_workers = 4;
const max_queue = 256;

const DownloadJob = struct {
    picture: *c.GtkWidget,
    url: []const u8,
    width: c_int,
    height: c_int,
};

var queue: [max_queue]?DownloadJob = .{null} ** max_queue;
var queue_head: usize = 0;
var queue_tail: usize = 0;
var queue_count: usize = 0;
var queue_mutex: std.Thread.Mutex = .{};
var queue_cond: std.Thread.Condition = .{};
var workers_started: bool = false;

fn startWorkers() void {
    if (workers_started) return;
    workers_started = true;
    for (0..max_workers) |_| {
        _ = std.Thread.spawn(.{}, workerLoop, .{}) catch {};
    }
}

fn enqueue(job: DownloadJob) void {
    queue_mutex.lock();
    defer queue_mutex.unlock();

    if (queue_count >= max_queue) {
        // Queue full — drop oldest
        if (queue[queue_head]) |old| {
            c.g_object_unref(@ptrCast(old.picture));
            app.getAllocator().free(old.url);
        }
        queue_head = (queue_head + 1) % max_queue;
        queue_count -= 1;
    }

    queue[queue_tail] = job;
    queue_tail = (queue_tail + 1) % max_queue;
    queue_count += 1;
    queue_cond.signal();
}

fn dequeue() DownloadJob {
    queue_mutex.lock();
    defer queue_mutex.unlock();

    while (queue_count == 0) {
        queue_cond.wait(&queue_mutex);
    }

    const job = queue[queue_head].?;
    queue[queue_head] = null;
    queue_head = (queue_head + 1) % max_queue;
    queue_count -= 1;
    return job;
}

fn workerLoop() void {
    const allocator = app.getAllocator();

    // Each worker gets its own HTTP client (reused across requests)
    var hc = http_mod.HttpClient.init(allocator);
    defer hc.deinit();

    while (true) {
        const job = dequeue();
        processJob(&hc, job);
    }
}

fn processJob(hc: *http_mod.HttpClient, job: DownloadJob) void {
    const allocator = app.getAllocator();
    defer {
        allocator.free(job.url);
    }

    var cache = app.getImageCache() orelse {
        _ = c.g_idle_add(@ptrCast(&unrefWidget), @ptrCast(job.picture));
        return;
    };

    // Check cache again (another worker might have downloaded it)
    if ((cache.getLocalPath(job.url) catch null)) |local_path| {
        defer allocator.free(local_path);
        // Post to main thread to load
        const ctx = allocator.create(LoadCtx) catch {
            _ = c.g_idle_add(@ptrCast(&unrefWidget), @ptrCast(job.picture));
            return;
        };
        ctx.* = .{
            .picture = job.picture,
            .local_path = allocator.dupe(u8, local_path) catch {
                allocator.destroy(ctx);
                _ = c.g_idle_add(@ptrCast(&unrefWidget), @ptrCast(job.picture));
                return;
            },
            .width = job.width,
            .height = job.height,
        };
        _ = c.g_idle_add(@ptrCast(&onLoadFromDisk), @ptrCast(ctx));
        return;
    }

    // Generate local path
    const local_path = cache.localPathForUrl(job.url) catch {
        _ = c.g_idle_add(@ptrCast(&unrefWidget), @ptrCast(job.picture));
        return;
    };

    // Ensure directory exists
    if (std.mem.lastIndexOfScalar(u8, local_path, '/')) |sep| {
        std.fs.cwd().makePath(local_path[0..sep]) catch {};
    }

    // Download
    var response = hc.get(job.url, &.{}) catch {
        allocator.free(local_path);
        _ = c.g_idle_add(@ptrCast(&unrefWidget), @ptrCast(job.picture));
        return;
    };
    defer response.deinit();

    if (response.body.len == 0) {
        allocator.free(local_path);
        _ = c.g_idle_add(@ptrCast(&unrefWidget), @ptrCast(job.picture));
        return;
    }

    // Write to disk
    std.fs.cwd().writeFile(.{ .sub_path = local_path, .data = response.body }) catch {
        allocator.free(local_path);
        _ = c.g_idle_add(@ptrCast(&unrefWidget), @ptrCast(job.picture));
        return;
    };

    // Store in cache
    cache.store(job.url, local_path, @intCast(response.body.len)) catch {};

    // Post to main thread
    const ctx = allocator.create(LoadCtx) catch {
        allocator.free(local_path);
        _ = c.g_idle_add(@ptrCast(&unrefWidget), @ptrCast(job.picture));
        return;
    };
    ctx.* = .{
        .picture = job.picture,
        .local_path = local_path,
        .width = job.width,
        .height = job.height,
    };
    _ = c.g_idle_add(@ptrCast(&onLoadFromDisk), @ptrCast(ctx));
}

const LoadCtx = struct {
    picture: *c.GtkWidget,
    local_path: []const u8,
    width: c_int,
    height: c_int,
};

fn onLoadFromDisk(user_data: ?*anyopaque) callconv(.c) c.gboolean {
    const ctx: *LoadCtx = @ptrCast(@alignCast(user_data orelse return 0));
    const allocator = app.getAllocator();
    defer {
        c.g_object_unref(@ptrCast(ctx.picture));
        allocator.free(ctx.local_path);
        allocator.destroy(ctx);
    }

    loadLocalFile(ctx.picture, ctx.local_path, ctx.width, ctx.height);
    return 0;
}

fn unrefWidget(user_data: ?*anyopaque) callconv(.c) c.gboolean {
    if (user_data) |ptr| c.g_object_unref(ptr);
    return 0;
}

// --- Public API ---

/// Load an image from any URL into a GtkPicture widget.
/// Checks image cache first; if not cached, queues for background download.
pub fn loadImageFromUrl(
    picture: *c.GtkWidget,
    url: ?[]const u8,
    width: c_int,
    height: c_int,
) void {
    const img_url = url orelse return;
    if (img_url.len == 0) return;

    const allocator = app.getAllocator();

    // Check cache synchronously (fast)
    var cache = app.getImageCache() orelse return;
    if ((cache.getLocalPath(img_url) catch null)) |local_path| {
        defer allocator.free(local_path);
        loadLocalFile(picture, local_path, width, height);
        return;
    }

    // Queue for background download
    startWorkers();

    _ = c.g_object_ref(@ptrCast(picture));

    enqueue(.{
        .picture = picture,
        .url = allocator.dupe(u8, img_url) catch {
            c.g_object_unref(@ptrCast(picture));
            return;
        },
        .width = width,
        .height = height,
    });
}

fn loadLocalFile(picture: *c.GtkWidget, local_path: []const u8, width: c_int, height: c_int) void {
    const allocator = app.getAllocator();
    const path_z = allocator.dupeZ(u8, local_path) catch return;
    defer allocator.free(path_z);

    const pixbuf = c.gdk_pixbuf_new_from_file_at_scale(
        path_z.ptr,
        width,
        height,
        1,
        null,
    );
    if (pixbuf == null) return;
    defer c.g_object_unref(@ptrCast(pixbuf));

    const texture = c.gdk_texture_new_for_pixbuf(pixbuf);
    if (texture == null) return;
    defer c.g_object_unref(@ptrCast(texture));

    c.gtk_picture_set_paintable(@ptrCast(picture), @ptrCast(texture));
}

// --- Legacy TMDB API (used by home_view) ---

/// Try to load a TMDB image into a GtkPicture widget.
pub fn loadTmdbImage(
    picture: *c.GtkWidget,
    tmdb_path: ?[]const u8,
    size: tmdb_types.ImageSize,
    width: c_int,
    height: c_int,
) bool {
    const path = tmdb_path orelse return false;
    if (path.len == 0) return false;

    const allocator = app.getAllocator();

    const url = tmdb_types.imageUrl(allocator, size, path) catch return false;
    defer allocator.free(url);

    var cache = app.getImageCache() orelse return false;
    const local_path = (cache.getLocalPath(url) catch return false) orelse return false;
    defer allocator.free(local_path);

    loadLocalFile(picture, local_path, width, height);
    return true;
}

/// Create a GtkPicture widget sized for a poster (130x195 default).
pub fn createPosterPicture(
    tmdb_path: ?[]const u8,
    width: c_int,
    height: c_int,
    media_type_icon: [*:0]const u8,
) *c.GtkWidget {
    const picture = c.gtk_picture_new();
    c.gtk_widget_set_size_request(@ptrCast(picture), width, height);
    c.gtk_picture_set_content_fit(@ptrCast(picture), c.GTK_CONTENT_FIT_COVER);

    if (!loadTmdbImage(@ptrCast(picture), tmdb_path, .w342, width, height)) {
        const icon = c.gtk_image_new_from_icon_name(media_type_icon);
        c.gtk_image_set_pixel_size(@ptrCast(icon), 36);
        c.gtk_widget_set_opacity(@ptrCast(icon), 0.3);
        const frame = c.gtk_frame_new(null);
        c.gtk_widget_set_size_request(@ptrCast(frame), width, height);
        c.gtk_widget_set_overflow(@ptrCast(frame), c.GTK_OVERFLOW_HIDDEN);
        c.gtk_widget_set_halign(@ptrCast(icon), c.GTK_ALIGN_CENTER);
        c.gtk_widget_set_valign(@ptrCast(icon), c.GTK_ALIGN_CENTER);
        c.gtk_frame_set_child(@ptrCast(frame), @ptrCast(icon));
        return @ptrCast(frame);
    }

    return @ptrCast(picture);
}
