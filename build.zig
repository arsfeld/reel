const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});
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
        exe.root_module.linkSystemLibrary("gtk4", .{});
        exe.root_module.linkSystemLibrary("libadwaita-1", .{});
        exe.root_module.linkSystemLibrary("mpv", .{});
        exe.root_module.linkSystemLibrary("epoxy", .{});
        exe.root_module.linkSystemLibrary("egl", .{});
        exe.root_module.addImport("sqlite", sqlite_mod);
        exe.root_module.addIncludePath(b.path("include"));

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
