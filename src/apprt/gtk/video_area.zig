const std = @import("std");
const c = @import("c.zig").c;
const player_mod = @import("../../core/player.zig");

pub const VideoArea = struct {
    widget: ?*c.GtkWidget = null,
    player: ?*player_mod.Player = null,

    pub fn init(player: *player_mod.Player) VideoArea {
        const gl_area = c.gtk_gl_area_new();
        c.gtk_gl_area_set_required_version(@ptrCast(gl_area), 3, 3);
        c.gtk_gl_area_set_auto_render(@ptrCast(gl_area), 0); // We control rendering via mpv callbacks
        c.gtk_widget_set_hexpand(gl_area, 1);
        c.gtk_widget_set_vexpand(gl_area, 1);

        var self = VideoArea{
            .widget = gl_area,
            .player = player,
        };
        _ = &self;

        // Store player pointer as widget data for callbacks
        c.g_object_set_data(@ptrCast(gl_area), "player", @ptrCast(player));

        _ = c.g_signal_connect_data(
            @ptrCast(gl_area),
            "realize",
            @ptrCast(&onRealize),
            @ptrCast(player),
            null,
            c.G_CONNECT_DEFAULT,
        );

        _ = c.g_signal_connect_data(
            @ptrCast(gl_area),
            "render",
            @ptrCast(&onRender),
            @ptrCast(player),
            null,
            c.G_CONNECT_DEFAULT,
        );

        _ = c.g_signal_connect_data(
            @ptrCast(gl_area),
            "unrealize",
            @ptrCast(&onUnrealize),
            @ptrCast(player),
            null,
            c.G_CONNECT_DEFAULT,
        );

        return .{ .widget = gl_area, .player = player };
    }
};

fn onRealize(gl_area: *c.GtkGLArea, user_data: ?*anyopaque) callconv(.c) void {
    c.gtk_gl_area_make_current(gl_area);
    if (c.gtk_gl_area_get_error(gl_area) != null) {
        std.log.err("GL area has error on realize", .{});
        return;
    }

    const player: *player_mod.Player = @ptrCast(@alignCast(user_data orelse return));

    player.initRender(&getProcAddress) catch |err| {
        std.log.err("Failed to init mpv render context: {}", .{err});
        return;
    };

    // Set the update callback: when mpv has a new frame, queue a redraw.
    // CRITICAL: Do NOT call any mpv API from this callback.
    // We use g_idle_add to schedule the redraw on the main thread.
    const widget_ptr: ?*anyopaque = @ptrCast(@as(*c.GtkWidget, @ptrCast(gl_area)));
    c.g_object_set_data(@ptrCast(gl_area), "gl_area_self", widget_ptr);

    player.setRenderUpdateCallback(&onMpvUpdate, widget_ptr);
}

fn onMpvUpdate(ctx: ?*anyopaque) callconv(.c) void {
    // Schedule a redraw on the GTK main thread.
    // Do NOT call any mpv API here.
    _ = c.g_idle_add(&queueRender, ctx);
}

fn queueRender(user_data: ?*anyopaque) callconv(.c) c_int {
    const widget: *c.GtkGLArea = @ptrCast(@alignCast(user_data orelse return 0));
    c.gtk_gl_area_queue_render(widget);
    return 0; // G_SOURCE_REMOVE - run only once
}

fn onRender(gl_area: *c.GtkGLArea, _: *c.GdkGLContext, user_data: ?*anyopaque) callconv(.c) c_int {
    const player: *player_mod.Player = @ptrCast(@alignCast(user_data orelse return 0));

    // GTK4 renders into its own FBO, not the default framebuffer.
    // We MUST get the current FBO binding to pass to mpv.
    var fbo: c.GLint = 0;
    c.glGetIntegerv(c.GL_FRAMEBUFFER_BINDING, &fbo);

    const width = c.gtk_widget_get_width(@ptrCast(gl_area));
    const height = c.gtk_widget_get_height(@ptrCast(gl_area));

    player.render(fbo, @intCast(width), @intCast(height));

    return 1; // We handled it
}

fn onUnrealize(_: *c.GtkGLArea, user_data: ?*anyopaque) callconv(.c) void {
    const player: *player_mod.Player = @ptrCast(@alignCast(user_data orelse return));
    if (player.render_ctx) |ctx| {
        c.mpv_render_context_free(@ptrCast(ctx));
        player.render_ctx = null;
    }
}

fn getProcAddress(_: ?*anyopaque, name: [*c]const u8) callconv(.c) ?*anyopaque {
    // Use eglGetProcAddress (preferred on modern GTK4/Wayland)
    // Fall back to glXGetProcAddress for X11
    const addr = c.eglGetProcAddress(name);
    if (addr) |a| return @ptrCast(@constCast(a));
    return null;
}
