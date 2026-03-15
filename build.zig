const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // Core library module
    const core_mod = b.addModule("reel", .{
        .root_source_file = b.path("src/lib.zig"),
        .target = target,
        .optimize = optimize,
        .link_libc = true,
    });
    core_mod.linkSystemLibrary("mpv", .{});
    core_mod.linkSystemLibrary("epoxy", .{});
    core_mod.linkSystemLibrary("sqlite3", .{});

    // Main executable (GTK frontend)
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
    exe.root_module.linkSystemLibrary("sqlite3", .{});

    b.installArtifact(exe);

    // Run step
    const run_step = b.step("run", "Run Reel");
    const run_cmd = b.addRunArtifact(exe);
    run_step.dependOn(&run_cmd.step);
    run_cmd.step.dependOn(b.getInstallStep());
    if (b.args) |args| {
        run_cmd.addArgs(args);
    }

    // Tests
    const core_tests = b.addTest(.{
        .root_module = core_mod,
    });
    const run_core_tests = b.addRunArtifact(core_tests);

    const test_step = b.step("test", "Run tests");
    test_step.dependOn(&run_core_tests.step);
}
