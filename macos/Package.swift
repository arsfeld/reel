// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "Reel",
    platforms: [
        .macOS(.v13)
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
            linkerSettings: [
                .unsafeFlags(["-L../../zig-out/lib"]),
                .linkedLibrary("reel"),
                .linkedLibrary("mpv"),
                .linkedLibrary("epoxy"),
                .linkedLibrary("sqlite3"),
            ]
        ),
    ]
)
