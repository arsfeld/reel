// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "Reel",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(
            name: "Reel",
            targets: ["Reel"]
        )
    ],
    targets: [
        .executableTarget(
            name: "Reel",
            dependencies: [],
            path: "Reel",
            exclude: ["Info.plist", "Assets.xcassets"],
            swiftSettings: [
                .unsafeFlags(["-import-objc-header", "../Generated/SwiftBridgeCore.h"])
            ],
            linkerSettings: [
                .unsafeFlags([
                    "-L../",
                    "-L../../target/debug",
                    "-L../../target/release",
                    "-lreel_ffi"
                ])
            ]
        )
    ]
)