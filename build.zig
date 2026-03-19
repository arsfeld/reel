const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const raw_optimize = b.standardOptimizeOption(.{});
    // Work around a Zig 0.15.2 compiler crash (SIGILL) triggered by
    // zig-sqlite's heavy comptime code in Debug mode.  Default to
    // ReleaseSafe; pass -Doptimize=ReleaseFast etc. to override.
    const optimize: std.builtin.OptimizeMode = if (raw_optimize == .Debug) .ReleaseSafe else raw_optimize;
    const is_linux = target.result.os.tag == .linux;

    // Vendored zig-sqlite module
    const sqlite_mod = b.addModule("sqlite", .{
        .root_source_file = b.path("vendor/zig-sqlite/sqlite.zig"),
        .target = target,
        .optimize = optimize,
        .link_libc = true,
    });
    sqlite_mod.addIncludePath(b.path("vendor/zig-sqlite/c"));
    sqlite_mod.addIncludePath(b.path("vendor/zig-sqlite/sqlite-amalgamation"));
    sqlite_mod.addCSourceFile(.{
        .file = b.path("vendor/zig-sqlite/sqlite-amalgamation/sqlite3.c"),
        .flags = &.{
            "-std=c99",
            "-DSQLITE_THREADSAFE=1",
            "-DSQLITE_DQS=0",
            "-DSQLITE_DEFAULT_WAL_SYNCHRONOUS=1",
            "-DSQLITE_USE_ALLOCA",
            "-DSQLITE_OMIT_DECLTYPE",
        },
    });
    sqlite_mod.addCSourceFile(.{
        .file = b.path("vendor/zig-sqlite/c/workaround.c"),
        .flags = &.{},
    });

    // Core library module
    const core_mod = b.addModule("reel", .{
        .root_source_file = b.path("src/lib.zig"),
        .target = target,
        .optimize = optimize,
        .link_libc = true,
    });
    core_mod.linkSystemLibrary("mpv", .{});
    core_mod.linkSystemLibrary("epoxy", .{});
    core_mod.addImport("sqlite", sqlite_mod);
    core_mod.addIncludePath(b.path("include"));

    // Static library for macOS Swift frontend consumption
    const lib = b.addLibrary(.{
        .name = "reel",
        .linkage = .static,
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/lib.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
        }),
    });
    lib.root_module.linkSystemLibrary("mpv", .{});
    lib.root_module.linkSystemLibrary("epoxy", .{});
    lib.root_module.addImport("sqlite", sqlite_mod);
    lib.root_module.addIncludePath(b.path("include"));
    b.installArtifact(lib);

    // Main executable (GTK frontend — Linux only)
    if (is_linux) {
        const exe = b.addExecutable(.{
            .name = "reel",
            .root_module = b.createModule(.{
                .root_source_file = b.path("src/main.zig"),
                .target = target,
                .optimize = optimize,
                .link_libc = true,
                .imports = &.{
                    .{ .name = "reel", .module = core_mod },
                },
            }),
        });

        // Zig translate-c workaround for GLib 2.86+ / GTK 4.20+
        //
        // translate-c cannot handle _Pragma() and chokes on #error
        // directives in raw header files.  We work around both issues:
        //   1. _Pragma: neutralised via @cDefine("_Pragma(x)", {}) in c.zig
        //   2. #error in gdkversionmacros.h: patched copy in overlay
        //
        // The overlay must be searched BEFORE system pkg-config includes.
        // linkSystemLibrary serialises pkg-config -I flags before
        // addIncludePath, so we link GTK/adwaita without pkg-config
        // (link only) and supply include paths ourselves.
        exe.root_module.addIncludePath(b.path("include/zig-tc-patches"));
        exe.root_module.addIncludePath(b.path("include"));

        // Link GTK/adwaita without pkg-config to control include ordering.
        exe.root_module.linkSystemLibrary("gtk-4", .{ .use_pkg_config = .no });
        exe.root_module.linkSystemLibrary("adwaita-1", .{ .use_pkg_config = .no });
        // GTK depends on GLib/GObject/GIO/GdkPixbuf which must be linked explicitly
        // when not using pkg-config for GTK.
        exe.root_module.linkSystemLibrary("glib-2.0", .{ .use_pkg_config = .no });
        exe.root_module.linkSystemLibrary("gobject-2.0", .{ .use_pkg_config = .no });
        exe.root_module.linkSystemLibrary("gio-2.0", .{ .use_pkg_config = .no });
        exe.root_module.linkSystemLibrary("gdk_pixbuf-2.0", .{ .use_pkg_config = .no });

        // Other libraries can use pkg-config normally.
        exe.root_module.linkSystemLibrary("mpv", .{});
        exe.root_module.linkSystemLibrary("epoxy", .{});
        exe.root_module.linkSystemLibrary("egl", .{});

        // GTK/GLib system include paths added AFTER our overlay.
        addPkgConfigIncludes(b, exe.root_module, &.{ "gtk4", "libadwaita-1", "gdk-pixbuf-2.0" });

        exe.root_module.addImport("sqlite", sqlite_mod);

        b.installArtifact(exe);

        // Run step
        const run_step = b.step("run", "Run Reel");
        const run_cmd = b.addRunArtifact(exe);
        run_step.dependOn(&run_cmd.step);
        run_cmd.step.dependOn(b.getInstallStep());
        if (b.args) |args| {
            run_cmd.addArgs(args);
        }
    }

    // Tests
    const core_tests = b.addTest(.{
        .root_module = core_mod,
    });
    const run_core_tests = b.addRunArtifact(core_tests);

    const test_step = b.step("test", "Run tests");
    test_step.dependOn(&run_core_tests.step);
}

/// Run pkg-config --cflags-only-I for the given packages and add
/// each resulting -I path via addSystemIncludePath on the module.
fn addPkgConfigIncludes(b: *std.Build, mod: *std.Build.Module, packages: []const []const u8) void {
    for (packages) |pkg| {
        var out_code: u8 = 0;
        const stdout = b.runAllowFail(
            &.{ "pkg-config", "--cflags-only-I", pkg },
            &out_code,
            .Inherit,
        ) catch continue;
        if (out_code != 0) continue;
        var iter = std.mem.splitScalar(u8, std.mem.trim(u8, stdout, " \t\n"), ' ');
        while (iter.next()) |flag| {
            if (flag.len > 2 and std.mem.eql(u8, flag[0..2], "-I")) {
                mod.addSystemIncludePath(.{ .cwd_relative = flag[2..] });
            }
        }
    }
}
