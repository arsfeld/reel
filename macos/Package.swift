// swift-tools-version: 6.1
import PackageDescription
import Foundation

let packageDir = URL(fileURLWithPath: #filePath).deletingLastPathComponent().path
let repoRoot = packageDir + "/.."

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
                .unsafeFlags([
                    "-L\(repoRoot)/zig-out/lib",
                    "-L/opt/homebrew/lib",
                    "-L/opt/homebrew/opt/mpv/lib",
                ]),
                .linkedLibrary("reel"),
                .linkedLibrary("mpv"),
                .linkedLibrary("epoxy"),
                .linkedLibrary("sqlite3"),
            ]
        ),
    ]
)
