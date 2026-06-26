// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "NemotronBubbleMac",
    platforms: [
        .macOS(.v13)
    ],
    products: [
        .executable(name: "NemotronBubbleMac", targets: ["NemotronBubbleMac"])
    ],
    targets: [
        .executableTarget(
            name: "NemotronBubbleMac"
        )
    ]
)
