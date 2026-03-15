# Technology Reference: Reel Media Center

Comprehensive API references and best practices for the Zig-based media center
application, gathered March 2026.

---

## 1. libmpv C API

### 1.1 Core Lifecycle

The fundamental usage pattern is: create handle, set options, initialize, send
commands, process events, destroy.

**mpv_create**
```c
mpv_handle *mpv_create(void);
```
- Creates a new mpv instance in a pre-initialized state.
- Returns NULL on error (out of memory, or LC_NUMERIC locale not set to "C").
- Most API functions cannot be called until `mpv_initialize()` is called.

**mpv_initialize**
```c
int mpv_initialize(mpv_handle *ctx);
```
- Initializes an uninitialized mpv instance.
- Loads config files (mpv.conf) by default. Set "config" and "config-dir"
  options before calling if you need to control this.
- Certain options (config paths, script loading, encoding mode) must be set
  before this call.
- Returns 0 on success, negative error code on failure.

**mpv_destroy / mpv_terminate_destroy**
```c
void mpv_destroy(mpv_handle *ctx);
void mpv_terminate_destroy(mpv_handle *ctx);
```
- `mpv_destroy` disconnects the handle but keeps the player alive if other
  handles exist.
- `mpv_terminate_destroy` stops playback and destroys the player core.

### 1.2 Configuration

**mpv_set_property / mpv_set_property_string** (preferred)
```c
int mpv_set_property(mpv_handle *ctx, const char *name,
                     mpv_format format, void *data);
int mpv_set_property_string(mpv_handle *ctx, const char *name,
                            const char *data);
```
- Can be used before `mpv_initialize()` for option-backed properties.
- Preferred over the deprecated `mpv_set_option()` / `mpv_set_option_string()`.
- Properties are "essentially variables which can be queried or set at runtime."
- Option names should NOT include the leading "--" prefix.

**mpv_set_option / mpv_set_option_string** (semi-deprecated)
```c
int mpv_set_option(mpv_handle *ctx, const char *name,
                   mpv_format format, void *data);
int mpv_set_option_string(mpv_handle *ctx, const char *name,
                          const char *data);
```
- Sets options before initialization. Semi-deprecated; use `mpv_set_property()`
  instead for most purposes.

### 1.3 Command Execution

**mpv_command** (synchronous)
```c
int mpv_command(mpv_handle *ctx, const char **args);
```
- Sends a command using a NULL-terminated string array.
- Commands match input.conf syntax.
- Example: `const char *cmd[] = {"loadfile", "/path/to/video.mp4", NULL};`

**mpv_command_async** (asynchronous)
```c
int mpv_command_async(mpv_handle *ctx, uint64_t reply_userdata,
                      const char **args);
```
- Executes commands asynchronously.
- Results arrive as `MPV_EVENT_COMMAND_REPLY` events.
- The `reply_userdata` correlates requests with replies.
- Safe to call from the render API thread.

### 1.4 Event Loop

**mpv_wait_event**
```c
mpv_event *mpv_wait_event(mpv_handle *ctx, double timeout);
```
- Blocks until an event arrives or timeout expires.
- `timeout=0` for non-blocking polling mode.
- The returned pointer is valid until the next `mpv_wait_event()` call.
- Only one thread may call this on the same handle at a time.
- Essential to call regularly to prevent event queue overflow.

**mpv_event structure**
```c
typedef struct mpv_event {
    mpv_event_id event_id;      // Event type (MPV_EVENT_NONE, etc.)
    int error;                   // Status code (>=0 success, <0 error)
    uint64_t reply_userdata;     // Correlates async requests/replies
    void *data;                  // Event-specific data pointer
} mpv_event;
```

**Key event types:**
- `MPV_EVENT_NONE` -- No event (timeout or queue empty)
- `MPV_EVENT_SHUTDOWN` -- Player is shutting down
- `MPV_EVENT_LOG_MESSAGE` -- Log message (if enabled)
- `MPV_EVENT_FILE_LOADED` -- File has been loaded
- `MPV_EVENT_END_FILE` -- Playback ended
- `MPV_EVENT_PROPERTY_CHANGE` -- Observed property changed
- `MPV_EVENT_COMMAND_REPLY` -- Async command completed

**mpv_observe_property**
```c
int mpv_observe_property(mpv_handle *mpv, uint64_t reply_userdata,
                         const char *name, mpv_format format);
```
- Registers for property change notifications.
- Delivers updates as `MPV_EVENT_PROPERTY_CHANGE` events.
- Always sends an initial change notification on registration.

### 1.5 Render API (render.h / render_gl.h)

The render API enables custom rendering using OpenGL. It replaced the older
`opengl_cb` API.

**Render Lifecycle:**
1. `mpv_render_context_create()` -- Initialize renderer with GL params
2. `mpv_render_context_set_update_callback()` -- Register frame notification
3. On callback: `mpv_render_context_update()` -- Check for new frames
4. When `MPV_RENDER_UPDATE_FRAME` is set: `mpv_render_context_render()` -- Render
5. Optionally: `mpv_render_context_report_swap()` -- Report display timing
6. Cleanup: `mpv_render_context_free()` -- Must be called before mpv core destruction

**mpv_render_context_create**
```c
int mpv_render_context_create(mpv_render_context **res, mpv_handle *mpv,
                              mpv_render_param *params);
```
Required params for OpenGL:
```c
mpv_render_param params[] = {
    {MPV_RENDER_PARAM_API_TYPE, MPV_RENDER_API_TYPE_OPENGL},
    {MPV_RENDER_PARAM_OPENGL_INIT_PARAMS, &(mpv_opengl_init_params){
        .get_proc_address = my_get_proc_address_fn,
    }},
    {MPV_RENDER_PARAM_ADVANCED_CONTROL, &(int){1}},
    {0}  // terminator
};
```

**mpv_opengl_init_params**
```c
typedef struct mpv_opengl_init_params {
    void *(*get_proc_address)(void *ctx, const char *name);
    void *get_proc_address_ctx;
} mpv_opengl_init_params;
```
- `get_proc_address`: Retrieves OpenGL function pointers. Typically delegates
  to the toolkit's GL loader (e.g., `epoxy_gl_get_proc_address` for GTK).
- libmpv does NOT link GL libraries directly.

**mpv_render_context_set_update_callback**
```c
void mpv_render_context_set_update_callback(mpv_render_context *ctx,
                                           mpv_render_update_fn callback,
                                           void *callback_ctx);
typedef void (*mpv_render_update_fn)(void *cb_ctx);
```
- Callback is invoked when a new frame is available or redraw is needed.
- Can be called from ANY thread.
- You MUST NOT call any mpv API from within the callback.
- The callback should merely signal the main/render thread to redraw.

**mpv_render_context_render**
```c
int mpv_render_context_render(mpv_render_context *ctx,
                              mpv_render_param *params);
```
With OpenGL FBO target:
```c
mpv_opengl_fbo opengl_fbo = {
    .fbo = framebuffer_id,  // 0 for default framebuffer
    .w = width,
    .h = height,
    .internal_format = 0    // 0 if unknown
};
mpv_render_param params[] = {
    {MPV_RENDER_PARAM_OPENGL_FBO, &opengl_fbo},
    {MPV_RENDER_PARAM_FLIP_Y, &(int){1}},
    {0}
};
mpv_render_context_render(mpv_gl, params);
```

**Threading Rules:**
- The client API is generally fully thread-safe.
- Only one `mpv_render_*` call at a time per context.
- The OpenGL context must be "current" for OpenGL backend calls.
- Do NOT call non-safe libmpv functions from the render thread.
- With `MPV_RENDER_PARAM_ADVANCED_CONTROL`, you must guarantee no lock/wait
  dependencies toward the render thread, or deadlocks will freeze the core.

### 1.6 GTK4 + libmpv Integration Pattern

Based on the mpv-examples GTK PR (#44), the integration works as follows:

```c
// 1. Get proc address via epoxy (GTK's GL loader)
static void *get_proc_address(void *ctx, const char *name) {
    return (void *)epoxy_gl_get_proc_address(name);
}

// 2. Create render context after GtkGLArea is realized
static void realize(GtkGLArea *area, gpointer user_data) {
    gtk_gl_area_make_current(area);
    // ... create mpv_render_context here ...
}

// 3. Render callback connected to GtkGLArea "render" signal
static gboolean render(GtkGLArea *area, GdkGLContext *context,
                       gpointer user_data) {
    struct mpv_player *player = user_data;
    if ((mpv_render_context_update(player->render_context)
         & MPV_RENDER_UPDATE_FRAME)) {
        gint fbo = -1;
        glGetIntegerv(GL_FRAMEBUFFER_BINDING, &fbo);
        mpv_opengl_fbo opengl_fbo = {fbo, player->width, player->height, 0};
        mpv_render_param params[] = {
            {MPV_RENDER_PARAM_OPENGL_FBO, &opengl_fbo},
            {MPV_RENDER_PARAM_FLIP_Y, &(int){1}},
            {MPV_RENDER_PARAM_INVALID, NULL}
        };
        mpv_render_context_render(player->render_context, params);
    }
    gtk_gl_area_queue_render(area);
    return TRUE;
}

// 4. mpv event wakeup dispatches to GTK main loop
static void on_mpv_events(void *ctx) {
    g_idle_add_full(G_PRIORITY_HIGH_IDLE, process_events, ctx, NULL);
}

// 5. mpv render update callback triggers GtkGLArea redraw
static void on_mpv_render_update(void *ctx) {
    // Signal GtkGLArea to queue a render
    gtk_gl_area_queue_render(GTK_GL_AREA(player->gl_area));
}
```

Key points:
- Use `g_idle_add_full()` to dispatch mpv events into the GTK main loop.
- Use `gtk_gl_area_queue_render()` to trigger redraws from the update callback.
- Get the current FBO via `glGetIntegerv(GL_FRAMEBUFFER_BINDING, ...)` since
  GTK4 renders into its own FBO, not the default framebuffer.

### 1.7 Key References

- Header files: `include/mpv/client.h`, `include/mpv/render.h`, `include/mpv/render_gl.h`
- Examples: https://github.com/mpv-player/mpv-examples/tree/master/libmpv
- GTK example PR: https://github.com/mpv-player/mpv-examples/pull/44
- Manual: https://mpv.io/manual/master/

---

## 2. GTK4 C API

### 2.1 GtkApplication Lifecycle

**Creating and running an application:**
```c
int main(int argc, char **argv) {
    GtkApplication *app;
    int status;

    app = gtk_application_new("com.example.reel",
                              G_APPLICATION_DEFAULT_FLAGS);
    g_signal_connect(app, "activate", G_CALLBACK(activate), NULL);
    status = g_application_run(G_APPLICATION(app), argc, argv);
    g_object_unref(app);

    return status;
}
```

**Activate handler -- builds the UI:**
```c
static void activate(GtkApplication *app, gpointer user_data) {
    GtkWidget *window;

    window = gtk_application_window_new(app);
    gtk_window_set_title(GTK_WINDOW(window), "Reel");
    gtk_window_set_default_size(GTK_WINDOW(window), 1280, 720);
    gtk_window_present(GTK_WINDOW(window));
}
```

**Lifecycle signals:**
- `startup` -- Application is starting, set up resources
- `activate` -- Application is launched / brought to foreground
- `shutdown` -- Application is quitting

### 2.2 Signal Connection

**g_signal_connect macro:**
```c
gulong g_signal_connect(instance, detailed_signal, c_handler, data);
```
- `instance`: The GObject to connect to
- `detailed_signal`: Signal name as a string ("activate", "render", "realize")
- `c_handler`: Callback cast via `G_CALLBACK()`
- `data`: User data pointer (passed as last argument to the handler)

Handler signature pattern:
```c
static void handler_name(EmitterType *emitter, /* signal-specific args */, gpointer user_data)
```

Handlers are called synchronously, before the default handler.

### 2.3 GtkGLArea Widget

**Creating and connecting signals:**
```c
GtkWidget *gl_area = gtk_gl_area_new();
g_signal_connect(gl_area, "realize", G_CALLBACK(on_realize), user_data);
g_signal_connect(gl_area, "render", G_CALLBACK(on_render), user_data);
```

**Realize signal -- initialize GL state:**
```c
static void on_realize(GtkGLArea *area) {
    gtk_gl_area_make_current(area);
    if (gtk_gl_area_get_error(area) != NULL)
        return;
    // Initialize GL resources (shaders, buffers, mpv render context)
}
```

**Render signal -- draw each frame:**
```c
static gboolean on_render(GtkGLArea *area, GdkGLContext *context) {
    // GL context is already current
    // The viewport is already set to the widget's allocation size

    glClearColor(0, 0, 0, 0);
    glClear(GL_COLOR_BUFFER_BIT);

    // Get the FBO that GTK is using (NOT necessarily 0)
    GLuint screen_fb = 0;
    glGetIntegerv(GL_FRAMEBUFFER_BINDING, &screen_fb);

    // Draw your content here...

    return TRUE;  // We handled rendering
}
```

**Triggering redraws:**
```c
gtk_gl_area_queue_render(GTK_GL_AREA(gl_area));
```

**Setting GL version requirements:**
```c
gtk_gl_area_set_required_version(GTK_GL_AREA(gl_area), 3, 3);
```

### 2.4 Layout Widgets

**GtkBox -- linear layout:**
```c
GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 5);  // 5px spacing
gtk_box_append(GTK_BOX(box), child_widget);
```

**GtkHeaderBar -- window titlebar:**
```c
GtkWidget *header = gtk_header_bar_new();
gtk_header_bar_pack_start(GTK_HEADER_BAR(header), left_widget);
gtk_header_bar_pack_end(GTK_HEADER_BAR(header), right_widget);
gtk_window_set_titlebar(GTK_WINDOW(window), header);
```

**GtkApplicationWindow:**
```c
GtkWidget *window = gtk_application_window_new(app);
gtk_window_set_child(GTK_WINDOW(window), content_widget);
```

### 2.5 GLib Main Loop Integration

GTK4 uses the GLib main loop internally. To integrate external event sources:

**Idle callbacks** (run when main loop is idle):
```c
g_idle_add(callback_function, user_data);
g_idle_add_full(G_PRIORITY_HIGH_IDLE, callback_function, user_data, NULL);
```

**Timeout callbacks** (run after delay):
```c
g_timeout_add(milliseconds, callback_function, user_data);
```

**Manual iteration** (alternative to `gtk_main()`):
```c
while (g_list_model_get_n_items(gtk_window_get_toplevels()) > 0)
    g_main_context_iteration(NULL, TRUE);
```

In GTK4, `gtk_main()` is deprecated. Use `g_application_run()` instead, which
handles the main loop internally.

### 2.6 Key References

- GTK4 API docs: https://docs.gtk.org/gtk4/
- Getting Started: https://docs.gtk.org/gtk4/getting_started.html
- GtkGLArea: https://docs.gtk.org/gtk4/class.GLArea.html
- GtkApplication: https://docs.gtk.org/gtk4/class.Application.html
- GtkHeaderBar: https://docs.gtk.org/gtk4/class.HeaderBar.html
- Tutorial: https://toshiocp.github.io/Gtk4-tutorial/

---

## 3. Zig @cImport and C Interop

### 3.1 Importing C Headers

```zig
const c = @cImport({
    @cInclude("mpv/client.h");
    @cInclude("mpv/render.h");
    @cInclude("mpv/render_gl.h");
    @cInclude("gtk/gtk.h");
    @cInclude("sqlite3.h");
});
```

Best practice: Use a single `@cImport` block per application to avoid duplicate
symbol issues and reduce compilation overhead. Assign to a `const c` by
convention.

You can use `@cDefine` and `@cUndef` within the block:
```zig
const c = @cImport({
    @cDefine("_GNU_SOURCE", {});
    @cInclude("mpv/client.h");
});
```

### 3.2 Linking System Libraries in build.zig

For Zig 0.14.x:
```zig
const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const exe = b.addExecutable(.{
        .name = "reel",
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
    });

    // Link system libraries via pkg-config
    exe.linkLibC();
    exe.linkSystemLibrary2("mpv", .{ .use_pkg_config = .force });
    exe.linkSystemLibrary2("gtk4", .{ .use_pkg_config = .force });
    exe.linkSystemLibrary2("sqlite3", .{ .use_pkg_config = .force });
    // epoxy is needed for GL proc address resolution with GTK
    exe.linkSystemLibrary2("epoxy", .{ .use_pkg_config = .force });

    b.installArtifact(exe);

    const run_cmd = b.addRunArtifact(exe);
    run_cmd.step.dependOn(b.getInstallStep());
    if (b.args) |args| {
        run_cmd.addArgs(args);
    }

    const run_step = b.step("run", "Run the application");
    run_step.dependOn(&run_cmd.step);
}
```

Notes:
- `linkSystemLibrary2` with `.use_pkg_config = .force` is the recommended
  approach on NixOS/Linux to resolve include and library paths automatically.
- `linkLibC()` is required when interfacing with C libraries.
- On NixOS, pkg-config must be in `nativeBuildInputs` of your shell.

For Zig 0.15.x (if upgrading):
- `addStaticLibrary` becomes `addLibrary(.{ .linkage = .static })`.
- System library linking may move to `exe.root_module.linkSystemLibrary()`.

### 3.3 Handling C Callbacks from Zig

C libraries frequently use callback function pointers with an opaque `void*`
user data parameter. The Zig pattern:

**Step 1: Define the callback with C calling convention**
```zig
fn mpvRenderUpdateCallback(ctx: ?*anyopaque) callconv(.C) void {
    // This is called from mpv's thread -- do NOT call mpv API here.
    // Signal the GTK main loop to queue a render.
    const player: *Player = @ptrCast(@alignCast(ctx.?));
    // Use g_idle_add or similar to dispatch to main thread
    _ = c.g_idle_add_full(
        c.G_PRIORITY_HIGH_IDLE,
        processEvents,
        @ptrCast(player),
        null,
    );
}
```

**Step 2: Register the callback, casting the Zig pointer to anyopaque**
```zig
c.mpv_render_context_set_update_callback(
    render_ctx,
    mpvRenderUpdateCallback,
    @ptrCast(self),  // *Player -> *anyopaque
);
```

**Step 3: Inside the callback, recover the typed pointer**
```zig
const self: *Player = @ptrCast(@alignCast(data.?));
```

Key casting operations:
- `@ptrCast(ptr)` -- Convert between pointer types (e.g., `*Player` to `*anyopaque`)
- `@alignCast(ptr)` -- Adjust pointer alignment (required when going from
  `*anyopaque` which has alignment 1 back to a type with stricter alignment)
- `.?` -- Unwrap an optional pointer (`?*anyopaque` to `*anyopaque`)
- `callconv(.C)` -- Required for all functions passed to C as function pointers.
  Zig functions without this annotation use the Zig calling convention, which
  is incompatible.

### 3.4 Memory Management Across C/Zig Boundary

**Key principles:**
- Zig has NO default allocator. Functions accept an `Allocator` parameter.
- C uses `malloc`/`free`. When linking libc, Zig exposes `std.heap.c_allocator`.
- Use `std.heap.c_allocator` for memory that crosses the C boundary (e.g.,
  strings passed to C functions that may call `free()`).
- For Zig-internal allocations, use `std.heap.GeneralPurposeAllocator` or
  another Zig allocator.
- C strings are null-terminated (`[*:0]const u8` in Zig). Use
  `std.mem.span()` to convert to a Zig slice.

**C string handling:**
```zig
// C string to Zig slice
const c_str: [*:0]const u8 = c.some_function();
const zig_str: []const u8 = std.mem.span(c_str);

// Zig string literal to C string (string literals are already null-terminated)
const name: [*:0]const u8 = "my-app";
c.some_c_function(name);

// Dynamic Zig string to C string
const buf = try allocator.dupeZ(u8, zig_string);
defer allocator.free(buf);
c.some_c_function(buf.ptr);
```

**C pointers:**
- `[*c]T` is the C pointer type. Avoid using it directly; prefer typed Zig pointers.
- C pointers can be null and coerce to/from integers.
- `@cImport` translates C types to use `[*c]` pointers which Zig code should
  convert to proper Zig pointer types at the boundary.

### 3.5 Key References

- Zig documentation: https://ziglang.org/documentation/master/
- Zig guide C interop: https://zig.guide/working-with-c/
- C interop chapter: https://zighelp.org/chapter-4/
- Callback patterns: https://eliasdorneles.com/til/posts/about-zig-structs-and-using-ptrcast-for-getting-zig-data-in-c-callbacks/
- Build system: https://ziglang.org/learn/build-system/

---

## 4. SQLite from Zig

### 4.1 Option A: zig-sqlite (vrischmann/zig-sqlite)

A comptime-powered wrapper providing type-safe queries.

**Adding as dependency:**
```bash
zig fetch --save git+https://github.com/vrischmann/zig-sqlite
```

**build.zig configuration:**
```zig
const sqlite = b.dependency("sqlite", .{
    .target = target,
    .optimize = optimize,
});
exe.root_module.addImport("sqlite", sqlite.module("sqlite"));
```

**Usage examples:**
```zig
const sqlite = @import("sqlite");

// Open database
var db = try sqlite.Db.init(.{
    .mode = sqlite.Db.Mode{ .File = "/path/to/reel.db" },
    .open_flags = .{ .write = true, .create = true },
    .threading_mode = .MultiThread,
});
defer db.deinit();

// Create table
try db.exec("CREATE TABLE IF NOT EXISTS movies (id INTEGER PRIMARY KEY, title TEXT, tmdb_id INTEGER)", .{}, .{});

// Insert with bound parameters
try db.exec("INSERT INTO movies (title, tmdb_id) VALUES (?, ?)", .{}, .{ "Inception", 27205 });

// Query single row
const row = try db.one(
    struct { title: [256:0]u8, tmdb_id: i64 },
    .{},
    "SELECT title, tmdb_id FROM movies WHERE id = ?",
    .{1},
);

// Iterate multiple rows
var stmt = try db.prepare("SELECT id, title FROM movies WHERE tmdb_id > ?");
defer stmt.deinit();
var iter = try stmt.iterator(struct { id: i64, title: [256:0]u8 }, .{0});
while (try iter.next(.{})) |row| {
    std.debug.print("Movie: {s}\n", .{std.mem.span(&row.title)});
}
```

**Features:**
- Comptime type checking of bind parameters
- Automatic mapping of result rows to Zig structs
- Thread-safety modes (SingleThread, MultiThread, Serialized)
- Bundles SQLite source or can link system SQLite
- Tracks Zig master on main branch; tagged branches for releases (e.g., `zig-0.15.1`)

### 4.2 Option B: zqlite.zig (karlseguin/zqlite.zig)

A thinner wrapper that does NOT bundle SQLite -- you link it yourself.

**Adding as dependency:**
```bash
zig fetch --save git+https://github.com/karlseguin/zqlite.zig#master
```

**build.zig configuration (system SQLite):**
```zig
const zqlite = b.dependency("zqlite", .{
    .target = target,
    .optimize = optimize,
});
exe.linkLibC();
exe.linkSystemLibrary("sqlite3");
exe.root_module.addImport("zqlite", zqlite.module("zqlite"));
```

**Usage examples:**
```zig
const zqlite = @import("zqlite");

// Open database
const flags = zqlite.OpenFlags.Create | zqlite.OpenFlags.EXResCode;
var conn = try zqlite.open("/tmp/reel.sqlite", flags);
defer conn.close();

// Execute
try conn.exec("INSERT INTO movies (title) VALUES (?1), (?2)", .{"Inception", "Interstellar"});

// Single row
if (try conn.row("SELECT title FROM movies LIMIT 1", .{})) |row| {
    defer row.deinit();
    std.debug.print("title: {s}\n", .{row.text(0)});
}

// Multiple rows
var rows = try conn.rows("SELECT id, title FROM movies", .{});
defer rows.deinit();
while (rows.next()) |row| {
    std.debug.print("id={d}, title={s}\n", .{row.int(0), row.text(1)});
}
```

**Features:**
- Thread-safe connection pooling via `zqlite.Pool`
- Transaction support with automatic rollback via `errdefer`
- Nullable and non-nullable data type getters
- Blob handling with explicit `zqlite.blob()` wrapper
- Tested with SQLite 3.50.4

### 4.3 Option C: Direct C API via @cImport

For maximum control without a wrapper:

```zig
const c = @cImport({
    @cInclude("sqlite3.h");
});

var db: ?*c.sqlite3 = null;
const rc = c.sqlite3_open("/path/to/reel.db", &db);
if (rc != c.SQLITE_OK) {
    // handle error
    const errmsg = c.sqlite3_errmsg(db);
    std.debug.print("Error: {s}\n", .{std.mem.span(errmsg)});
}
defer _ = c.sqlite3_close(db);

// Prepare statement
var stmt: ?*c.sqlite3_stmt = null;
_ = c.sqlite3_prepare_v2(db, "SELECT * FROM movies", -1, &stmt, null);
defer _ = c.sqlite3_finalize(stmt);

while (c.sqlite3_step(stmt) == c.SQLITE_ROW) {
    const title = c.sqlite3_column_text(stmt, 1);
    // ...
}
```

### 4.4 Recommendation for Reel

**zig-sqlite** is the best fit because:
- Bundles SQLite (simpler dependency management)
- Comptime type safety aligns with Zig philosophy
- Struct-based row mapping reduces boilerplate
- Actively maintained with version-tagged branches

---

## 5. Nix Flake for Zig Projects

### 5.1 Complete flake.nix for Reel

```nix
{
  description = "Reel - Native Media Center";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            # Build tools
            zig
            zls            # Zig Language Server
            pkg-config

            # Native library development packages
            mpv-unwrapped  # libmpv
            gtk4
            sqlite
            libepoxy       # GL function loader (needed for GTK GL)

            # GTK4 runtime dependencies
            glib
            graphene
            gdk-pixbuf
            pango
            cairo
            harfbuzz

            # For HTTP requests (TMDB API, Plex API)
            curl
            openssl
          ];

          # Ensure Zig can find native library headers and .so files
          shellHook = ''
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [
              pkgs.mpv-unwrapped
              pkgs.gtk4
              pkgs.sqlite
              pkgs.libepoxy
              pkgs.glib
            ]}:$LD_LIBRARY_PATH"
          '';

          # Disable hardening flags that conflict with Zig compilation
          hardeningDisable = [ "all" ];
        };
      }
    );
}
```

### 5.2 Key Configuration Details

**pkg-config placement:** Must be in `nativeBuildInputs` (not `buildInputs`) so
that `mkShell` arranges `PKG_CONFIG_PATH` to find the `.pc` files automatically.

**hardeningDisable:** Critical for Zig development on NixOS. Without this, Nix
enables security hardening flags (stack protector, FORTIFY_SOURCE, etc.) that
conflict with Zig's compilation model and cause link errors.

**Package naming:**
- `mpv-unwrapped` provides the libmpv shared library and headers (without the
  mpv wrapper script)
- `gtk4` provides both the runtime and development files
- `sqlite` provides libsqlite3
- `libepoxy` provides the GL function loading library used by GTK

**Using the shell:**
```bash
nix develop              # Enter the dev shell
zig build                # Build with system libraries resolved via pkg-config
zig build run            # Build and run
```

### 5.3 Alternative: zig2nix

For more sophisticated Nix integration (building Zig projects as Nix
derivations, handling zon dependencies):

```bash
# Initialize from zig2nix template
nix flake init -t github:Cloudef/zig2nix#master
```

zig2nix provides:
- Automatically updated Zig compiler builds
- `.zon` file conversion to Nix derivations
- Lock file support for reproducible builds
- Cross-compilation support

Repository: https://github.com/Cloudef/zig2nix

### 5.4 Key References

- Nix flakes wiki: https://nixos.wiki/wiki/Flakes
- Dev shells examples: https://michael.stapelberg.ch/posts/2025-07-27-dev-shells-with-nix-4-quick-examples/
- Zig + Nix wiki: https://github.com/ziglang/zig/wiki/development-with-nix
- zig2nix: https://github.com/Cloudef/zig2nix
- Zig on NixOS include paths: https://ziggit.dev/t/correct-include-paths-in-nixos/8502

---

## 6. TMDB API v3

### 6.1 Authentication

**Bearer Token (recommended):**
All requests use an API Read Access Token as a Bearer token in the Authorization
header. This works for both v3 and v4 endpoints.

```
Authorization: Bearer eyJhbGciOiJIUzI1NiJ9...
```

Obtain the token from: TMDB Account Settings > API section > "API Read Access Token"

**API Key (legacy v3 alternative):**
Pass `api_key=<your_key>` as a query parameter. Bearer token is preferred as it
provides a single authentication process across both v3 and v4.

### 6.2 Base URL

```
https://api.themoviedb.org/3/
```

### 6.3 Movie Search

**Endpoint:** `GET /3/search/movie`

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| query | string | Yes | Search term |
| include_adult | boolean | No | Include adult content |
| language | string | No | Default: "en-US" |
| primary_release_year | integer | No | Filter by release year |
| page | integer | No | Page number (default: 1) |
| region | string | No | Geographic region filter |
| year | integer | No | Year filter |

**Response:**
```json
{
  "page": 1,
  "results": [
    {
      "adult": false,
      "backdrop_path": "/path.jpg",
      "genre_ids": [28, 12],
      "id": 27205,
      "original_language": "en",
      "original_title": "Inception",
      "overview": "A thief who steals...",
      "popularity": 98.5,
      "poster_path": "/poster.jpg",
      "release_date": "2010-07-16",
      "title": "Inception",
      "video": false,
      "vote_average": 8.4,
      "vote_count": 35000
    }
  ],
  "total_pages": 1,
  "total_results": 1
}
```

### 6.4 TV Search

**Endpoint:** `GET /3/search/tv`

**Parameters:**
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| query | string | Yes | Search term |
| first_air_date_year | integer | No | Filter by first air date year |
| include_adult | boolean | No | Include adult content |
| language | string | No | Default: "en-US" |
| page | integer | No | Page number (default: 1) |

**Response fields (per result):**
```
adult, backdrop_path, genre_ids, id, origin_country[],
original_language, original_name, overview, popularity,
poster_path, first_air_date, name, vote_average, vote_count
```

### 6.5 Movie Details

**Endpoint:** `GET /3/movie/{movie_id}`

```
GET https://api.themoviedb.org/3/movie/27205
Authorization: Bearer <token>
```

Supports `append_to_response` for sub-requests in a single call:
```
GET /3/movie/27205?append_to_response=videos,credits,images
```

### 6.6 TV Details

**Endpoint:** `GET /3/tv/{series_id}`

```
GET https://api.themoviedb.org/3/tv/1399
Authorization: Bearer <token>
```

### 6.7 Image URLs

Images returned by the API contain partial paths (e.g., `/1E5baAaEse26fej7uHcjOgEE2t2.jpg`).
To construct a full URL, combine three components:

```
{base_url}{size}{file_path}
```

**Base URLs:**
- HTTP: `http://image.tmdb.org/t/p/`
- HTTPS: `https://image.tmdb.org/t/p/` (use this)

**Available sizes:**

| Type | Sizes |
|------|-------|
| poster_sizes | w92, w154, w185, w342, w500, w780, original |
| backdrop_sizes | w300, w780, w1280, original |
| logo_sizes | w45, w92, w154, w185, w300, w500, original |
| profile_sizes | w45, w185, h632, original |
| still_sizes | w92, w185, w300, original |

**Example:**
```
https://image.tmdb.org/t/p/w500/1E5baAaEse26fej7uHcjOgEE2t2.jpg
```

These sizes can also be retrieved programmatically via the Configuration
endpoint: `GET /3/configuration`.

### 6.8 Rate Limiting

As of December 2019, TMDB removed explicit rate limits. The legacy limit was
40 requests per 10 seconds. While no formal rate limit is currently enforced,
it is good practice to:
- Cache responses locally (in SQLite)
- Batch requests where possible using `append_to_response`
- Avoid unnecessary repeated calls for the same data

### 6.9 Other Useful Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /3/movie/{id}/credits` | Cast and crew |
| `GET /3/movie/{id}/images` | Posters, backdrops |
| `GET /3/movie/{id}/videos` | Trailers, teasers |
| `GET /3/tv/{id}/season/{num}` | Season details |
| `GET /3/tv/{id}/season/{num}/episode/{num}` | Episode details |
| `GET /3/search/multi` | Search movies, TV, and people |
| `GET /3/genre/movie/list` | All movie genres |
| `GET /3/genre/tv/list` | All TV genres |
| `GET /3/configuration` | Image base URLs, sizes |

### 6.10 Key References

- Developer docs: https://developer.themoviedb.org/docs/getting-started
- Authentication: https://developer.themoviedb.org/docs/authentication-application
- Image basics: https://developer.themoviedb.org/docs/image-basics
- API reference: https://developer.themoviedb.org/reference/
- Rate limiting: https://developer.themoviedb.org/docs/rate-limiting

---

## Cross-Cutting Concerns

### Zig + GTK4 + libmpv Integration Pattern

The critical integration point is rendering libmpv video inside a GTK4 window
via GtkGLArea. The recommended architecture:

1. **GTK4 owns the GL context** via GtkGLArea.
2. **libmpv renders into GTK's FBO** via the render API.
3. **mpv events dispatch into GTK's main loop** via `g_idle_add()`.
4. **Frame updates trigger GtkGLArea redraws** via `gtk_gl_area_queue_render()`.

In Zig, this means:
- All GTK and mpv C functions are accessed via a single `@cImport` block.
- Callbacks use `callconv(.C)` with `?*anyopaque` for user data.
- The Player struct holds both the mpv handle and GTK widget references.
- `@ptrCast` and `@alignCast` bridge typed Zig pointers through C's void pointers.

### Build Pipeline

```
flake.nix (Nix)          -->  provides system libs + pkg-config
  |
build.zig (Zig)          -->  links mpv, gtk4, sqlite3, epoxy via pkg-config
  |
src/main.zig (Zig)       -->  @cImport all C headers, implement core + frontend
```
