// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "Zenith",
    platforms: [.macOS(.v14)],
    targets: [
        .systemLibrary(
            name: "CZenith",
            path: "Sources/CZenith"
        ),
        .executableTarget(
            name: "Zenith",
            dependencies: ["CZenith"],
            path: "Sources/Zenith",
            exclude: ["Shaders.metal"],
            linkerSettings: [
                .unsafeFlags([
                    "../target/release/libzenith_ffi.a",
                ]),
                .linkedFramework("Metal"),
                .linkedFramework("MetalKit"),
                .linkedFramework("AppKit"),
                .linkedFramework("CoreText"),
                .linkedFramework("CoreGraphics"),
                .linkedFramework("CoreFoundation"),
            ]
        ),
    ]
)
