# Reel Media Center: Best Practices Research

**Date:** 2026-03-14
**Scope:** Practical integration patterns for Zig + libmpv + GTK4 + Plex API, following Ghostty's architecture

---

## 1. Zig + libmpv Integration

### Existing Bindings: zmpv

The [zmpv](https://github.com/hasanpasha/zmpv) project provides Zig bindings for libmpv, last updated March 2025. It wraps the core client API (create, initialize, command, properties, events) but does **not** wrap the render API (`render.h`). For Reel, you will need to extend these bindings or write render API bindings directly.

**Adding zmpv to build.zig:**

```zig
const zmpv_dep = b.dependency("zmpv", .{
    .target = target,
    .optimize = optimize,
});
exe.root_module.addImport("zmpv", zmpv_dep.module("zmpv"));
exe.linkSystemLibrary("mpv");
exe.linkLibC();
```

**Basic mpv usage via zmpv:**

```zig
const zmpv = @import("zmpv");

// Create and initialize
var mpv = try zmpv.Mpv.create(allocator);
defer mpv.terminate_destroy();

try mpv.set_option("osc", .Flag, &@as(c_int, 1));
try mpv.initialize();

// Load a file
try mpv.command_async(0, &.{ "loadfile", path });

// Observe properties
try mpv.observe_property(1, "pause", .Flag);
try mpv.observe_property(2, "time-pos", .INT64);

// Event loop
while (true) {
    const event = mpv.wait_event(10000);
    switch (event.event_id) {
        .Shutdown => break,
        .PropertyChange => { /* handle property updates */ },
        .EndFile => { /* handle end of file */ },
        else => {},
    }
}
```

### Render API: Direct C Binding Required

The render API (`mpv/render.h`) must be bound directly since zmpv does not cover it. This is the critical API for embedding mpv video into a GTK4 GtkGLArea.

**Core render API types (from render.h):**

```zig
const mpv = @cImport({
    @cInclude("mpv/client.h");
    @cInclude("mpv/render.h");
    @cInclude("mpv/render_gl.h");
});
```

**Key functions to bind:**

| Function | Purpose |
|----------|---------|
| `mpv_render_context_create()` | Initialize renderer with OpenGL params |
| `mpv_render_context_render()` | Render a frame to the current FBO |
| `mpv_render_context_set_update_callback()` | Get notified when new frame is ready |
| `mpv_render_context_update()` | Check for pending updates (returns flags) |
| `mpv_render_context_free()` | Cleanup render context |

**Creating a render context with OpenGL (Zig translation of the C pattern):**

```zig
fn initRenderContext(mpv_handle: *mpv.mpv_handle, get_proc_fn: mpv.mpv_opengl_cb_get_proc_address_fn) !*mpv.mpv_render_context {
    var gl_init_params = mpv.mpv_opengl_init_params{
        .get_proc_address = get_proc_fn,
        .get_proc_address_ctx = null,
    };

    var params = [_]mpv.mpv_render_param{
        .{ .type = mpv.MPV_RENDER_PARAM_API_TYPE, .data = @ptrCast(@constCast(mpv.MPV_RENDER_API_TYPE_OPENGL)) },
        .{ .type = mpv.MPV_RENDER_PARAM_OPENGL_INIT_PARAMS, .data = @ptrCast(&gl_init_params) },
        .{ .type = mpv.MPV_RENDER_PARAM_ADVANCED_CONTROL, .data = @ptrCast(&@as(c_int, 1)) },
        .{ .type = mpv.MPV_RENDER_PARAM_INVALID, .data = null },
    };

    var render_ctx: ?*mpv.mpv_render_context = null;
    const err = mpv.mpv_render_context_create(&render_ctx, mpv_handle, &params);
    if (err < 0) return error.RenderContextCreationFailed;
    return render_ctx.?;
}
```

**Rendering a frame into the current FBO:**

```zig
fn renderFrame(render_ctx: *mpv.mpv_render_context, fbo: c_int, width: c_int, height: c_int) void {
    var fbo_params = mpv.mpv_opengl_fbo{
        .fbo = fbo,
        .w = width,
        .h = height,
        .internal_format = 0,
    };

    var params = [_]mpv.mpv_render_param{
        .{ .type = mpv.MPV_RENDER_PARAM_OPENGL_FBO, .data = @ptrCast(&fbo_params) },
        .{ .type = mpv.MPV_RENDER_PARAM_FLIP_Y, .data = @ptrCast(&@as(c_int, 1)) },
        .{ .type = mpv.MPV_RENDER_PARAM_INVALID, .data = null },
    };

    _ = mpv.mpv_render_context_render(render_ctx, &params);
}
```

**Update callback pattern (signals new frame availability):**

```zig
fn onMpvRenderUpdate(ctx: ?*anyopaque) callconv(.C) void {
    // This is called from mpv's thread - must not call mpv APIs here.
    // Signal the GTK main thread to queue a redraw.
    const gl_area: *c.GtkGLArea = @ptrCast(@alignCast(ctx));
    c.gtk_gl_area_queue_render(gl_area);
}

// Register:
mpv.mpv_render_context_set_update_callback(render_ctx, onMpvRenderUpdate, @ptrCast(gl_area));
```

### Threading Model

- The render functions can be called from any thread, but only one call at a time
- The render thread must NOT call non-render mpv APIs (deadlock risk)
- With `MPV_RENDER_PARAM_ADVANCED_CONTROL`: the render thread cannot wait on core
- The OpenGL context must be current and consistent per render context
- Update callbacks must NOT invoke mpv APIs -- they should only signal the UI thread

### Reference: Celluloid Pattern

[Celluloid](https://github.com/celluloid-player/celluloid) (GTK4 mpv frontend) uses this exact pattern: GtkGLArea for the render surface, libmpv render API for frame delivery, `g_idle_add` to bridge mpv's update callback to GTK's main loop. Reel should follow this same architecture.

---

## 2. Zig + GTK4

### Two Approaches

**Approach A: Direct @cImport (Ghostty's original approach)**

Use `@cImport` to directly import GTK4 headers. This is simpler but loses type safety and requires manual signal wiring.

```zig
const c = @cImport({
    @cInclude("gtk/gtk.h");
});

pub fn main() void {
    const app = c.gtk_application_new("com.example.reel", c.G_APPLICATION_DEFAULT_FLAGS);
    _ = c.g_signal_connect_data(
        @ptrCast(app),
        "activate",
        @ptrCast(&activate),
        null,
        null,
        0,
    );
    const status = c.g_application_run(@ptrCast(app), 0, null);
    c.g_object_unref(@ptrCast(app));
    std.process.exit(@intCast(status));
}

fn activate(app: *c.GtkApplication, _: ?*anyopaque) callconv(.C) void {
    const window = c.gtk_application_window_new(app);
    c.gtk_window_set_title(@ptrCast(window), "Reel");
    c.gtk_window_set_default_size(@ptrCast(window), 1280, 720);
    c.gtk_window_present(@ptrCast(window));
}
```

**Approach B: zig-gobject (Ghostty's current approach, recommended)**

The [zig-gobject](https://github.com/ianprime0509/zig-gobject) project generates typed Zig bindings from GObject introspection metadata. This is what Ghostty now uses for its gtk-ng rewrite. It provides:

- Type-safe method calls (no `@ptrCast` everywhere)
- Signal connection with proper type checking
- GObject class definition from Zig
- Property and signal declaration

**build.zig setup with zig-gobject:**

```zig
const gobject_dep = b.dependency("gobject", .{});
exe.root_module.addImport("gtk", gobject_dep.module("gtk4"));
exe.root_module.addImport("gdk", gobject_dep.module("gdk4"));
exe.root_module.addImport("glib", gobject_dep.module("glib2"));
exe.root_module.addImport("gio", gobject_dep.module("gio2"));
```

### GtkGLArea for Video Rendering

GtkGLArea is the key widget for embedding libmpv's OpenGL output. The pattern:

1. GtkGLArea creates and manages its own GdkGLContext
2. On the `render` signal, mpv renders into the current FBO
3. The mpv update callback calls `gtk_gl_area_queue_render()` to trigger redraws

**GtkGLArea lifecycle signals:**
- `realize` -- GL context is created; initialize mpv render context here
- `render` -- called when a frame should be drawn; call `mpv_render_context_render()` here
- `unrealize` -- GL context is being destroyed; free mpv render context here
- `resize` -- widget size changed; update mpv's display size

**Important**: GTK4 constrains all GL operations to the main thread (`must_draw_from_app_thread = true` in Ghostty's terminology). The mpv update callback runs on mpv's thread and must only signal the main thread, never call GL or GTK functions directly.

### Signal Handling from Zig

With direct `@cImport`, use `g_signal_connect_data` (not the macro `g_signal_connect`, which `translate-c` cannot handle):

```zig
// g_signal_connect is a macro that Zig can't translate.
// Use g_signal_connect_data directly:
_ = c.g_signal_connect_data(
    @ptrCast(gl_area),           // instance
    "render",                      // signal name
    @ptrCast(&onRender),          // callback
    @ptrCast(player_state),       // user_data
    null,                          // destroy_data
    0,                             // connect_flags
);
```

### Zero-Cost Binding Patterns (from Ian Johnson)

For hand-crafted bindings that wrap the C API with Zig ergonomics:

**Transparent extern structs:**
```zig
pub const Window = extern struct {
    inner: c.GtkWindow,

    pub fn setTitle(self: *Window, title: [*:0]const u8) void {
        c.gtk_window_set_title(@ptrCast(self), title);
    }

    pub fn setDefaultSize(self: *Window, width: c_int, height: c_int) void {
        c.gtk_window_set_default_size(@ptrCast(self), width, height);
    }
};
```

**Type-safe enums:**
```zig
pub const Align = enum(c_uint) {
    fill = 0,
    start = 1,
    end = 2,
    center = 3,
};
```

**Flags as packed structs:**
```zig
pub const ApplicationFlags = packed struct(c_uint) {
    is_service: bool = false,
    handles_open: bool = false,
    _padding: u30 = 0,
};
```

### Recommendation for Reel

Start with Approach A (`@cImport`) for rapid prototyping, then migrate to zig-gobject (Approach B) once the core architecture stabilizes. Ghostty's experience shows that zig-gobject is now mature enough for production use and dramatically improves code quality, especially for memory management with GObject lifecycles.

---

## 3. Ghostty's Architecture

Ghostty is the definitive reference for the "Zig core + native frontends" pattern. The following is derived from the [Ghostty source](https://github.com/ghostty-org/ghostty), [Mitchell Hashimoto's talk](https://mitchellh.com/writing/ghostty-and-useful-zig-patterns), and the [gtk-ng PR](https://github.com/ghostty-org/ghostty/pull/7961).

### Directory Structure

```
ghostty/
├── build.zig                  # Build configuration
├── build.zig.zon              # Dependency manifest
├── include/
│   └── ghostty.h              # C API header (1000+ lines, manually maintained)
├── src/
│   ├── main_ghostty.zig       # GTK entry point
│   ├── main_c.zig             # Library entry point (for macOS/embedded)
│   ├── App.zig                # Global application state
│   ├── Surface.zig            # Single terminal instance
│   ├── apprt/                 # Application runtime abstraction
│   │   ├── action.zig         # Action dispatch definitions
│   │   ├── embedded.zig       # Library mode (macOS C ABI)
│   │   ├── gtk.zig            # GTK runtime selector
│   │   └── gtk/               # GTK platform implementation
│   │       ├── App.zig
│   │       └── Surface.zig
│   ├── terminal/              # VT emulation core
│   ├── termio/                # I/O layer (PTY management)
│   ├── renderer/              # GPU rendering
│   │   ├── Thread.zig         # Renderer event loop
│   │   ├── Metal.zig          # macOS Metal backend
│   │   └── OpenGL.zig         # Cross-platform OpenGL
│   ├── font/                  # Font system
│   ├── config/                # Configuration system
│   └── input/                 # Input handling
├── macos/
│   └── Sources/Ghostty/       # Swift/AppKit implementation
│       ├── TerminalController.swift
│       └── SurfaceView.swift
└── pkg/                       # Linux packaging, GTK UI definitions
```

### Recommended Reel Structure (adapted)

```
reel/
├── build.zig
├── build.zig.zon
├── include/
│   └── reel.h                 # C API header for libreel
├── src/
│   ├── main.zig               # GTK entry point (Linux)
│   ├── main_c.zig             # Library entry point (macOS/embedded)
│   ├── root.zig               # Library root exports
│   │
│   ├── core/                  # libreel core (platform-agnostic)
│   │   ├── App.zig            # Application state, lifecycle
│   │   ├── Player.zig         # mpv control, playback state
│   │   ├── Library.zig        # Media library (SQLite)
│   │   ├── Scanner.zig        # File/network scanner
│   │   └── Settings.zig       # User settings
│   │
│   ├── plex/                  # Plex client
│   │   ├── Client.zig         # HTTP client, auth
│   │   ├── Auth.zig           # PIN-based auth flow
│   │   ├── Library.zig        # Section/metadata browsing
│   │   ├── Playback.zig       # Direct play URL construction
│   │   └── Timeline.zig       # Scrobbling, watch status
│   │
│   ├── tmdb/                  # TMDB metadata client
│   │   ├── Client.zig
│   │   └── Models.zig
│   │
│   ├── mpv/                   # mpv integration
│   │   ├── Handle.zig         # mpv instance wrapper
│   │   ├── Render.zig         # Render API bindings
│   │   └── Events.zig         # Event dispatch
│   │
│   ├── db/                    # SQLite database
│   │   ├── Database.zig
│   │   ├── migrations/
│   │   └── models/
│   │
│   └── apprt/                 # Platform runtimes
│       ├── gtk/               # Linux GTK4 frontend
│       │   ├── App.zig
│       │   ├── Window.zig
│       │   ├── VideoWidget.zig  # GtkGLArea + mpv
│       │   ├── LibraryView.zig
│       │   └── PlayerControls.zig
│       └── embedded.zig       # macOS C ABI bridge
│
├── macos/                     # Swift/AppKit frontend
│   ├── Reel.xcodeproj
│   └── Sources/
│       ├── ReelApp.swift
│       ├── VideoView.swift
│       └── LibraryView.swift
│
└── nix/                       # Nix build support
    ├── flake.nix
    └── default.nix
```

### C ABI Design Pattern

Ghostty exposes a C ABI from its Zig core via `export fn` declarations in a dedicated file (`main_c.zig`). The C header (`include/ghostty.h`) is **manually maintained** to match.

**Key patterns:**

1. **Opaque handles**: The core types are exposed as opaque pointers

```zig
// src/main_c.zig
const App = core.App;

export fn reel_app_new(config: *const ReelConfig) ?*App {
    const app = App.init(config) catch return null;
    return app;
}

export fn reel_app_destroy(app: *App) void {
    app.deinit();
}

export fn reel_app_tick(app: *App) void {
    app.tick();
}
```

```c
// include/reel.h
typedef struct reel_app_s reel_app_t;

reel_app_t* reel_app_new(const reel_config_t* config);
void reel_app_destroy(reel_app_t* app);
void reel_app_tick(reel_app_t* app);
```

2. **Callback-based communication**: The host app provides function pointers

```zig
pub const AppCallbacks = extern struct {
    userdata: ?*anyopaque,
    wakeup: ?*const fn (?*anyopaque) callconv(.C) void,
    perform_action: ?*const fn (?*anyopaque, Action) callconv(.C) bool,
};
```

3. **Action dispatch**: A tagged union for all possible actions

```zig
pub const ActionTag = enum(c_int) {
    play = 0,
    pause = 1,
    seek = 2,
    load_media = 3,
    set_volume = 4,
    // ...
};

pub const Action = extern struct {
    tag: ActionTag,
    data: extern union {
        seek: extern struct { position_ms: i64 },
        load_media: extern struct { url: [*:0]const u8 },
        set_volume: extern struct { volume: f64 },
        // ...
    },
};
```

### Comptime Interface Pattern

Ghostty uses comptime generics to swap platform implementations at compile time with zero runtime overhead:

```zig
pub fn Surface(comptime AppRuntime: type) type {
    return struct {
        rt_surface: AppRuntime.Surface,
        // ... shared fields

        pub fn init(rt_app: *AppRuntime) !@This() {
            // Platform-agnostic initialization
            const rt_surface = try AppRuntime.Surface.init(rt_app);
            return .{
                .rt_surface = rt_surface,
            };
        }
    };
}
```

### Threading Architecture

Ghostty uses dedicated threads with message-passing mailboxes:

```
Main Thread (GTK event loop)
  |
  |-- receives user input
  |-- dispatches actions
  |-- handles GL rendering (GTK constraint)
  |
  v
Termio Thread (I/O)            Renderer Thread
  |                               |
  |-- manages PTY                 |-- runs at ~120fps
  |-- processes terminal output   |-- reads terminal state
  |-- signals renderer            |-- submits GL commands
```

For Reel, adapt this to:

```
Main Thread (GTK event loop)
  |
  |-- receives user input, UI events
  |-- handles GL rendering via GtkGLArea
  |-- dispatches player commands
  |
  v
Player Thread                   Network Thread
  |                               |
  |-- mpv event loop              |-- Plex API requests
  |-- property observation        |-- TMDB metadata fetch
  |-- signals main thread         |-- timeline reporting
```

### Platform Constraint Handling

```zig
// Statically checked at compile time
const must_draw_from_app_thread =
    if (@hasDecl(apprt.App, "must_draw_from_app_thread"))
        apprt.App.must_draw_from_app_thread
    else
        false;
```

---

## 4. Plex API

### Authentication: PIN-Based Flow

Source: [Plex Forum: Authenticating with Plex](https://forums.plex.tv/t/authenticating-with-plex/609370)

**Required headers for ALL Plex API requests:**

```
X-Plex-Product: Reel
X-Plex-Client-Identifier: <unique-uuid-per-install>
X-Plex-Version: 0.1.0
X-Plex-Platform: Linux
X-Plex-Device: PC
Accept: application/json
```

**Step 1: Generate a PIN**

```
POST https://plex.tv/api/v2/pins

Headers:
  X-Plex-Product: Reel
  X-Plex-Client-Identifier: <uuid>
  Accept: application/json

Body: (optional, for strong auth)
  strong=true

Response:
{
  "id": 123456789,
  "code": "ABCD",
  "authToken": null,
  "expiresAt": "2026-03-14T23:30:00Z",
  "clientIdentifier": "<uuid>"
}
```

**Step 2: Direct user to authenticate**

Open the default browser to:
```
https://app.plex.tv/auth#?clientID=<uuid>&code=ABCD&context%5Bdevice%5D%5Bproduct%5D=Reel
```

For the polling approach (native apps): the user enters the 4-character code at `https://plex.tv/link`.

**Step 3: Poll for completion**

```
GET https://plex.tv/api/v2/pins/<pinID>

Headers:
  X-Plex-Client-Identifier: <uuid>
  Accept: application/json

Response (after user authenticates):
{
  "id": 123456789,
  "code": "ABCD",
  "authToken": "abc123def456..."
}
```

Poll every 1-2 seconds. Timeout after 30 minutes (PIN expires). When `authToken` is non-null, authentication is complete.

**Step 4: Verify token**

```
GET https://plex.tv/api/v2/user

Headers:
  X-Plex-Token: <authToken>
  Accept: application/json
```

Returns HTTP 200 with user data if valid, 401 if invalid.

### Server Discovery

After authentication, discover the user's servers:

```
GET https://plex.tv/api/v2/resources?includeHttps=1&includeRelay=1

Headers:
  X-Plex-Token: <authToken>
  Accept: application/json
```

Returns a list of servers with connection URIs. Use the local connection when on the same network for direct play.

### Library Browsing

All endpoints below are against the PMS (Plex Media Server) directly:

**Base URL**: `http://<server-ip>:32400`

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/library/sections` | GET | List all libraries (Movies, TV Shows, etc.) |
| `/library/sections/{id}/all` | GET | All items in a library section |
| `/library/sections/{id}/firstCharacter` | GET | Alphabetic index |
| `/library/onDeck` | GET | "Continue Watching" items |
| `/library/recentlyAdded` | GET | Recently added items |
| `/library/metadata/{ratingKey}` | GET | Full metadata for an item |
| `/library/metadata/{ratingKey}/children` | GET | Seasons of a show, episodes of a season |
| `/search?query=<string>` | GET | Full-text search |
| `/photo/:/transcode?url=<path>&width=W&height=H` | GET | Resized artwork |

**Pagination parameters:**
- `X-Plex-Container-Start=<offset>`
- `X-Plex-Container-Size=<limit>`

### Direct Play URL Construction

When browsing metadata, media items include `Part` elements with a `key` attribute:

```xml
<MediaContainer>
  <Video ratingKey="12345" title="Movie Title" ...>
    <Media videoCodec="h264" audioCodec="aac" container="mp4" ...>
      <Part key="/library/parts/67890/file.mp4" file="/path/to/file.mp4" size="1234567890" />
    </Media>
  </Video>
</MediaContainer>
```

The direct play URL is:
```
http://<server-ip>:32400/library/parts/67890/file.mp4?X-Plex-Token=<token>
```

This URL can be passed directly to libmpv for playback without any transcoding.

For remote access (via Plex relay), use the HTTPS connection URI from resource discovery.

### Timeline Reporting (Scrobbling)

Report playback progress to sync watch status:

```
GET http://<server-ip>:32400/:/timeline
  ?ratingKey=12345
  &key=/library/metadata/12345
  &state=playing           (playing|paused|stopped)
  &time=<milliseconds>     (current playback position)
  &duration=<milliseconds> (total duration)
  &X-Plex-Token=<token>
```

**Reporting frequency:**
- Every 10 seconds on LAN/WAN
- Every 20 seconds on cellular
- Immediately on play state changes (play, pause, stop)

**Mark as watched/unwatched:**

```
GET /:/scrobble?key=12345&identifier=com.plexapp.plugins.library&X-Plex-Token=<token>
GET /:/unscrobble?key=12345&identifier=com.plexapp.plugins.library&X-Plex-Token=<token>
```

**Report partial progress:**

```
GET /:/progress?key=12345&time=<milliseconds>&identifier=com.plexapp.plugins.library&X-Plex-Token=<token>
```

### Response Format

- Default is XML. Request JSON via `Accept: application/json` header
- All timestamps are Unix epoch format
- Use JSON for easier parsing in Zig (XML parsing in Zig is less ergonomic)

### API Reference

Full endpoint list: [Plex Web API Overview](https://github.com/Arcanemagus/plex-api/wiki/Plex-Web-API-Overview)
Official docs: [Plex Developer API](https://developer.plex.tv/pms/)
Community docs: [plexapi.dev](https://plexapi.dev/Intro)

---

## 5. Zig Build System Patterns

Source: [Zig Build System documentation](https://ziglang.org/learn/build-system/)

### build.zig.zon Dependency Manifest

```zig
.{
    .name = .reel,
    .version = "0.1.0",
    .minimum_zig_version = "0.14.0",
    .dependencies = .{
        .zmpv = .{
            .url = "https://github.com/hasanpasha/zmpv/archive/<hash>.tar.gz",
            .hash = "1220...",
        },
        .gobject = .{
            .url = "https://github.com/ianprime0509/zig-gobject/releases/download/v0.X.X/bindings.tar.gz",
            .hash = "1220...",
        },
    },
    .paths = .{
        "build.zig",
        "build.zig.zon",
        "src",
        "include",
    },
}
```

**Hash discovery**: Set hash to a placeholder, run `zig build`, and Zig will report the correct hash.

### Linking C Libraries

```zig
pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // --- Core library (libreel) ---
    const libreel = b.addStaticLibrary(.{
        .name = "reel",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/root.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });
    libreel.linkLibC();
    libreel.linkSystemLibrary("mpv");
    libreel.linkSystemLibrary("sqlite3");
    b.installArtifact(libreel);

    // --- GTK executable (Linux) ---
    const exe = b.addExecutable(.{
        .name = "reel",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/main.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });
    exe.linkLibrary(libreel);
    exe.linkLibC();
    exe.linkSystemLibrary("gtk4");
    exe.linkSystemLibrary("epoxy");  // for GL function loading
    b.installArtifact(exe);

    // --- Dependencies ---
    const zmpv_dep = b.dependency("zmpv", .{
        .target = target,
        .optimize = optimize,
    });
    libreel.root_module.addImport("zmpv", zmpv_dep.module("zmpv"));

    // --- Run step ---
    const run_cmd = b.addRunArtifact(exe);
    run_cmd.step.dependOn(b.getInstallStep());
    const run_step = b.step("run", "Run Reel");
    run_step.dependOn(&run_cmd.step);
}
```

### Conditional Compilation per Platform

```zig
// Build options
const app_runtime = b.option(
    enum { gtk, embedded, none },
    "app-runtime",
    "Which application runtime to use",
) orelse .gtk;

// Pass to code as comptime value
const options = b.addOptions();
options.addOption(@TypeOf(app_runtime), "app_runtime", app_runtime);
exe.root_module.addOptions("build_options", options);

// In Zig source:
const build_options = @import("build_options");
const apprt = switch (build_options.app_runtime) {
    .gtk => @import("apprt/gtk.zig"),
    .embedded => @import("apprt/embedded.zig"),
    .none => @import("apprt/none.zig"),
};
```

### Cross-Platform Targets

```zig
// Build for specific target
// zig build -Dtarget=aarch64-macos

// In build.zig, detect target for conditional linking:
const is_linux = target.result.os.tag == .linux;
const is_macos = target.result.os.tag == .macos;

if (is_linux) {
    exe.linkSystemLibrary("gtk4");
    exe.linkSystemLibrary("epoxy");
}
if (is_macos) {
    // macOS: build static library for Swift consumption
    exe.linkFramework("CoreGraphics");
    exe.linkFramework("Metal");
}
```

### Static Library for macOS/Swift

Following Ghostty's pattern for producing a `.a` file that Swift can link:

```zig
const lib = b.addStaticLibrary(.{
    .name = "reel",
    .root_module = b.createModule(.{
        .root_source_file = b.path("src/main_c.zig"),
        .target = b.resolveTargetQuery(.{
            .cpu_arch = .aarch64,
            .os_tag = .macos,
        }),
        .optimize = optimize,
    }),
});
lib.bundle_compiler_rt = true;
lib.linkLibC();
```

Then use `libtool` to merge dependencies, `lipo` for universal binary, and package as XCFramework.

---

## 6. Cross-Platform Zig Projects with Native Frontends

### The Pattern

This is the "Ghostty pattern" and also used by [Zylix](https://dev.to/kotsutsumi_50/zylix-a-zig-based-ui-framework-for-7-platforms-4a):

1. **Core library in Zig** -- all business logic, no UI
2. **C ABI boundary** -- `export fn` declarations + manually maintained C header
3. **Platform frontends** -- each consumes the C API in its native toolkit

### C ABI Design Rules

From Ghostty's experience and Mitchell Hashimoto's [Zig+SwiftUI article](https://mitchellh.com/writing/zig-and-swiftui):

1. **Only C-compatible types in exported functions**: no comptime params, no slices, no optionals (use nullable pointers). Within the function body, full Zig is available.

2. **Opaque types for complex state**: expose `*App`, `*Player` as opaque pointers. The frontend never inspects their contents.

3. **Callback-based event flow**: the host provides function pointers that the core calls. This inverts control -- the host owns the event loop.

4. **Manual header maintenance**: Zig does not auto-generate C headers. The header file must be written and kept in sync manually. (Ghostty's `ghostty.h` is 1000+ lines.)

5. **Stable enum values**: use explicit integer values for all enums crossing the ABI boundary so they remain stable across versions.

### Module Organization Pattern

```zig
// src/root.zig -- Library root, re-exports public API
pub const App = @import("core/App.zig");
pub const Player = @import("core/Player.zig");
pub const PlexClient = @import("plex/Client.zig");
pub const TmdbClient = @import("tmdb/Client.zig");

// Comptime interface for platform runtime
pub fn Runtime(comptime Impl: type) type {
    return struct {
        impl: Impl,

        pub fn init(self: *@This()) !void {
            return self.impl.init();
        }

        pub fn requestRedraw(self: *@This()) void {
            self.impl.requestRedraw();
        }
    };
}
```

### Header Generation Workaround

Since Zig does not generate headers, use a convention:

```zig
// src/main_c.zig
// Every export fn here must have a corresponding declaration in include/reel.h

export fn reel_player_new(config: *const PlayerConfig) ?*Player {
    // ...
}

export fn reel_player_load(player: *Player, url: [*:0]const u8) c_int {
    // ...
}

export fn reel_player_get_position(player: *Player) i64 {
    // ...
}
```

```c
// include/reel.h
#ifndef REEL_H
#define REEL_H

#include <stdint.h>
#include <stdbool.h>

typedef struct reel_player_s reel_player_t;
typedef struct reel_player_config_s reel_player_config_t;

reel_player_t* reel_player_new(const reel_player_config_t* config);
int reel_player_load(reel_player_t* player, const char* url);
int64_t reel_player_get_position(reel_player_t* player);

#endif
```

### macOS XCFramework Packaging

From [Integrating Zig and SwiftUI](https://mitchellh.com/writing/zig-and-swiftui):

1. Build static lib for each arch: `zig build -Dtarget=aarch64-macos` and `zig build -Dtarget=x86_64-macos`
2. Merge dependencies with `libtool`
3. Create universal binary with `lipo -create arm64/libreel.a x86_64/libreel.a -output libreel.a`
4. Create `include/module.modulemap`:
   ```
   module ReelKit {
       umbrella header "reel.h"
       export *
   }
   ```
5. Package as XCFramework: `xcodebuild -create-xcframework -library libreel.a -headers include/ -output ReelKit.xcframework`
6. In Swift: `import ReelKit`

---

## Summary: Integration Points for Reel

### The GTK + mpv Rendering Pipeline

This is the most critical integration and ties together sections 1, 2, and 3:

```
GTK Main Loop
    |
    v
GtkGLArea (realize signal)
    |-- Create mpv handle (mpv_create + mpv_initialize)
    |-- Make GL context current
    |-- Create mpv_render_context with OpenGL params
    |-- Set mpv update callback -> gtk_gl_area_queue_render()
    |-- Set mpv "vo" option to "libmpv"
    |
GtkGLArea (render signal)
    |-- Get current FBO from OpenGL
    |-- Call mpv_render_context_render() with FBO params
    |-- Return TRUE (we handled rendering)
    |
GtkGLArea (unrealize signal)
    |-- mpv_render_context_free()
    |-- mpv_terminate_destroy()
    |
mpv update callback (mpv thread -> main thread bridge)
    |-- Called by mpv when new frame is ready
    |-- Calls gtk_gl_area_queue_render() (thread-safe)
    |-- Does NOT call any other mpv or GTK APIs
```

### Critical Configuration for libmpv Embedded Rendering

```zig
// After mpv_create, before mpv_initialize:
mpv.mpv_set_option_string(handle, "vo", "libmpv");           // Use render API, not own window
mpv.mpv_set_option_string(handle, "hwdec", "auto-safe");     // Hardware acceleration
mpv.mpv_set_option_string(handle, "terminal", "no");         // No terminal output
mpv.mpv_set_option_string(handle, "input-default-bindings", "no"); // We handle input
mpv.mpv_set_option_string(handle, "input-vo-keyboard", "no");     // No keyboard grab
mpv.mpv_set_option_string(handle, "osc", "no");              // No on-screen controller
```

### Key References

- [mpv render API documentation](https://www.ccoderun.ca/programming/doxygen/mpv/render_8h.html)
- [mpv-examples GTK render PR](https://github.com/mpv-player/mpv-examples/pull/44/files)
- [zmpv Zig bindings](https://github.com/hasanpasha/zmpv)
- [zig-gobject bindings](https://github.com/ianprime0509/zig-gobject)
- [Ghostty source](https://github.com/ghostty-org/ghostty)
- [Ghostty architecture talk](https://mitchellh.com/writing/ghostty-and-useful-zig-patterns)
- [Zig + SwiftUI integration](https://mitchellh.com/writing/zig-and-swiftui)
- [Ghostty gtk-ng PR](https://github.com/ghostty-org/ghostty/pull/7961)
- [Zero-cost Zig bindings](https://ianjohnson.dev/posts/zero-cost-bindings-with-zig/)
- [Plex authentication forum post](https://forums.plex.tv/t/authenticating-with-plex/609370)
- [Plex Web API overview](https://github.com/Arcanemagus/plex-api/wiki/Plex-Web-API-Overview)
- [plexapi.dev documentation](https://plexapi.dev/Intro)
- [Zig build system reference](https://ziglang.org/learn/build-system/)
- [Celluloid (GTK4 mpv frontend)](https://github.com/celluloid-player/celluloid)
- [Zylix multi-platform Zig UI](https://dev.to/kotsutsumi_50/zylix-a-zig-based-ui-framework-for-7-platforms-4a)
