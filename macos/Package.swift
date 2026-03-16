// swift-tools-version: 6.1
import PackageDescription
import Foundation

let packageDir = URL(fileURLWithPath: #filePath).deletingLastPathComponent().path
let repoRoot = packageDir + "/.."

let env = ProcessInfo.processInfo.environment
var libPaths: [String] = ["-L\(repoRoot)/zig-out/lib"]
if let dir = env["REEL_MPV_LIBDIR"] { libPaths.append("-L\(dir)") }
if let dir = env["REEL_EPOXY_LIBDIR"] { libPaths.append("-L\(dir)") }
if let dir = env["REEL_SQLITE_LIBDIR"] { libPaths.append("-L\(dir)") }

let package = Package(
    name: "Reel",
    platforms: [
        .macOS(.v15)   // Will target macOS 26 once SDK is available; v15 for now
    ],
    targets: [
        .systemLibrary(
            name: "ReelCore",
            path: "libreel",
            pkgConfig: nil,
            providers: nil
        ),
        .executableTarget(
            name: "Reel",
            dependencies: ["ReelCore"],
            path: "Reel/Sources",
            exclude: [
                "AppDelegate.swift",
                "MainWindow.swift",
                "SidebarViewController.swift",
                "PlaceholderViewController.swift",
                "PlayerControlsView.swift",
                "SettingsViewController.swift",
                "VideoView.swift",
                "main.swift",
            ],
            swiftSettings: [
                .swiftLanguageMode(.v5),
            ],
            linkerSettings: [
                .unsafeFlags(libPaths),
                .linkedLibrary("reel"),
                .linkedLibrary("mpv"),
                .linkedLibrary("epoxy"),
                .linkedLibrary("sqlite3"),
            ]
        ),
    ]
)
