// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "YuzukanIME",
    platforms: [.macOS(.v13)],
    targets: [
        .executableTarget(
            name: "YuzukanIME",
            path: "Sources/YuzukanIME",
            linkerSettings: [
                .linkedFramework("InputMethodKit")
            ]
        ),
        .testTarget(
            name: "YuzukanIMETests",
            dependencies: ["YuzukanIME"],
            path: "Tests/YuzukanIMETests"
        ),
    ]
)
