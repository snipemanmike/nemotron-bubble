import AppKit
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers

let repoRoot = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
let docsURL = repoRoot.appendingPathComponent("docs")
try FileManager.default.createDirectory(at: docsURL, withIntermediateDirectories: true)

let red = NSColor(calibratedRed: 0.95, green: 0.18, blue: 0.18, alpha: 1.0)
let green = NSColor(calibratedRed: 0.28, green: 0.86, blue: 0.48, alpha: 1.0)
let pillTop = NSColor(calibratedRed: 0.18, green: 0.20, blue: 0.24, alpha: 1.0)
let pillBottom = NSColor(calibratedRed: 0.10, green: 0.11, blue: 0.14, alpha: 1.0)

func makeImage(width: Int, height: Int, draw: (NSRect) -> Void) -> NSImage {
    let image = NSImage(size: NSSize(width: width, height: height))
    image.lockFocus()
    NSGraphicsContext.current?.imageInterpolation = .high
    draw(NSRect(x: 0, y: 0, width: width, height: height))
    image.unlockFocus()
    return image
}

func cgImage(_ image: NSImage) -> CGImage {
    var rect = NSRect(origin: .zero, size: image.size)
    return image.cgImage(forProposedRect: &rect, context: nil, hints: nil)!
}

func writePNG(_ image: NSImage, named name: String) throws {
    let rep = NSBitmapImageRep(cgImage: cgImage(image))
    let data = rep.representation(using: .png, properties: [:])!
    try data.write(to: docsURL.appendingPathComponent(name))
}

func writeGIF(_ frames: [NSImage], named name: String, delay: Double) throws {
    let url = docsURL.appendingPathComponent(name) as CFURL
    let type = UTType.gif.identifier as CFString
    guard let destination = CGImageDestinationCreateWithURL(url, type, frames.count, nil) else {
        throw NSError(domain: "render-readme-assets", code: 1)
    }

    let gifProps: CFDictionary = [
        kCGImagePropertyGIFDictionary: [
            kCGImagePropertyGIFLoopCount: 0
        ]
    ] as CFDictionary
    CGImageDestinationSetProperties(destination, gifProps)

    let frameProps: CFDictionary = [
        kCGImagePropertyGIFDictionary: [
            kCGImagePropertyGIFDelayTime: delay
        ]
    ] as CFDictionary

    for frame in frames {
        CGImageDestinationAddImage(destination, cgImage(frame), frameProps)
    }

    if !CGImageDestinationFinalize(destination) {
        throw NSError(domain: "render-readme-assets", code: 2)
    }
}

func fillVerticalGradient(_ rect: NSRect, top: NSColor, bottom: NSColor, radius: CGFloat) {
    let path = NSBezierPath(roundedRect: rect, xRadius: radius, yRadius: radius)
    path.addClip()
    let gradient = NSGradient(starting: top, ending: bottom)!
    gradient.draw(in: rect, angle: -90)
}

func roundedFill(_ rect: NSRect, color: NSColor, radius: CGFloat) {
    color.setFill()
    NSBezierPath(roundedRect: rect, xRadius: radius, yRadius: radius).fill()
}

func drawText(_ text: String, rect: NSRect, size: CGFloat, weight: NSFont.Weight, color: NSColor, alignment: NSTextAlignment = .left) {
    let paragraph = NSMutableParagraphStyle()
    paragraph.alignment = alignment
    paragraph.lineBreakMode = .byTruncatingTail
    let attrs: [NSAttributedString.Key: Any] = [
        .font: NSFont.systemFont(ofSize: size, weight: weight),
        .foregroundColor: color,
        .paragraphStyle: paragraph
    ]
    (text as NSString).draw(in: rect, withAttributes: attrs)
}

func waveformValues(frame: Int, count: Int) -> [CGFloat] {
    (0..<count).map { index in
        let x = CGFloat(index) / CGFloat(max(1, count - 1))
        let t = CGFloat(frame) * 0.22
        let a = sin((x * 2.2 + t) * .pi)
        let b = sin((x * 4.8 - t * 0.7) * .pi)
        return max(0.08, min(1.0, 0.44 + 0.32 * a + 0.18 * b))
    }
}

func drawBubble(in rect: NSRect, recording: Bool, values: [CGFloat]) {
    NSGraphicsContext.saveGraphicsState()
    let shadow = NSShadow()
    shadow.shadowBlurRadius = 18
    shadow.shadowOffset = NSSize(width: 0, height: -7)
    shadow.shadowColor = NSColor.black.withAlphaComponent(0.28)
    shadow.set()
    fillVerticalGradient(rect, top: pillTop, bottom: pillBottom, radius: rect.height / 2)
    NSGraphicsContext.restoreGraphicsState()

    let dotSize = rect.height * 0.30
    let dotRect = NSRect(x: rect.minX + rect.height * 0.34, y: rect.midY - dotSize / 2, width: dotSize, height: dotSize)
    (recording ? red : green).setFill()
    NSBezierPath(ovalIn: dotRect).fill()

    let barCount = values.count
    let left = rect.minX + rect.height * 0.86
    let right = rect.maxX - rect.height * 0.30
    let pitch = (right - left) / CGFloat(max(1, barCount))
    let barColor = (recording ? red : green).withAlphaComponent(recording ? 0.84 : 0.30)
    barColor.setFill()

    for (index, value) in values.enumerated() {
        let half = max(2.0, min(rect.height * 0.36, value * rect.height * 0.36))
        let bar = NSRect(
            x: left + CGFloat(index) * pitch + pitch * 0.30,
            y: rect.midY - half,
            width: max(2.0, pitch * 0.36),
            height: half * 2.0
        )
        NSBezierPath(roundedRect: bar, xRadius: 2, yRadius: 2).fill()
    }
}

func drawWindowBackground(_ rect: NSRect) {
    fillVerticalGradient(
        rect,
        top: NSColor(calibratedRed: 0.08, green: 0.09, blue: 0.11, alpha: 1),
        bottom: NSColor(calibratedRed: 0.05, green: 0.06, blue: 0.08, alpha: 1),
        radius: 0
    )
}

func drawSettingsWindow(_ rect: NSRect) {
    drawWindowBackground(rect)

    roundedFill(NSRect(x: 28, y: rect.height - 62, width: 4, height: 32), color: NSColor(calibratedRed: 0.35, green: 0.55, blue: 1.0, alpha: 1), radius: 2)
    drawText("Nemotron Bubble", rect: NSRect(x: 44, y: rect.height - 66, width: 340, height: 28), size: 24, weight: .semibold, color: .white)
    drawText("Ready. Nemotron loaded. Command toggles.", rect: NSRect(x: 44, y: rect.height - 92, width: 520, height: 20), size: 13, weight: .medium, color: NSColor.white.withAlphaComponent(0.66))

    let leftCard = NSRect(x: 22, y: 188, width: 354, height: 500)
    let rightCard = NSRect(x: 396, y: 188, width: 282, height: 500)
    roundedFill(leftCard, color: NSColor(calibratedRed: 0.12, green: 0.13, blue: 0.16, alpha: 1), radius: 14)
    roundedFill(rightCard, color: NSColor(calibratedRed: 0.12, green: 0.13, blue: 0.16, alpha: 1), radius: 14)

    drawText("Global shortcut", rect: NSRect(x: 44, y: 650, width: 180, height: 18), size: 13, weight: .medium, color: .white)
    roundedFill(NSRect(x: 236, y: 642, width: 114, height: 28), color: NSColor(calibratedRed: 0.18, green: 0.20, blue: 0.25, alpha: 1), radius: 7)
    drawText("Command", rect: NSRect(x: 236, y: 648, width: 114, height: 16), size: 12, weight: .medium, color: .white, alignment: .center)

    let rows = [
        ("Start at Login", true),
        ("Preload Nemotron", true),
        ("Type Live into Cursor", false),
        ("Copy Final to Clipboard", true),
        ("Paste Final on Stop", true),
        ("Sound Cues", true),
        ("Bubble Waveform", true),
        ("Bubble Click Opens Settings", true),
        ("Floating Bubble", true),
        ("Menu Bar Waveform", true),
        ("Enter Stops Recording", false)
    ]

    for (index, row) in rows.enumerated() {
        let y = 600 - CGFloat(index) * 34
        drawText(row.0, rect: NSRect(x: 44, y: y, width: 230, height: 18), size: 12.5, weight: .regular, color: NSColor.white.withAlphaComponent(0.86))
        let toggle = NSRect(x: 318, y: y - 3, width: 36, height: 20)
        roundedFill(toggle, color: row.1 ? NSColor(calibratedRed: 0.30, green: 0.54, blue: 1.0, alpha: 1) : NSColor(calibratedRed: 0.26, green: 0.28, blue: 0.33, alpha: 1), radius: 10)
        let knobX = row.1 ? toggle.maxX - 18 : toggle.minX + 2
        roundedFill(NSRect(x: knobX, y: toggle.minY + 2, width: 16, height: 16), color: .white, radius: 8)
    }

    drawText("Recent Dictations", rect: NSRect(x: 420, y: 650, width: 210, height: 20), size: 14, weight: .semibold, color: .white)
    roundedFill(NSRect(x: 420, y: 244, width: 232, height: 386), color: NSColor(calibratedRed: 0.15, green: 0.16, blue: 0.20, alpha: 1), radius: 8)

    let history = [
        ("Today, 2:14 PM", "This is a local macOS dictation demo."),
        ("Today, 2:11 PM", "Command can be used as the shortcut."),
        ("Today, 1:58 PM", "Final text is copied or pasted on stop."),
        ("Yesterday, 6:30 PM", "Nemotron runs on-device through the Rust helper.")
    ]

    for (index, item) in history.enumerated() {
        let y = 592 - CGFloat(index) * 82
        drawText(item.0, rect: NSRect(x: 438, y: y, width: 190, height: 16), size: 10.5, weight: .medium, color: NSColor.white.withAlphaComponent(0.48))
        drawText(item.1, rect: NSRect(x: 438, y: y - 42, width: 184, height: 38), size: 12, weight: .regular, color: NSColor.white.withAlphaComponent(0.84))
    }

    roundedFill(NSRect(x: 420, y: 204, width: 92, height: 28), color: NSColor(calibratedRed: 0.30, green: 0.54, blue: 1.0, alpha: 1), radius: 7)
    drawText("Copy Latest", rect: NSRect(x: 420, y: 210, width: 92, height: 16), size: 12, weight: .medium, color: .white, alignment: .center)
    roundedFill(NSRect(x: 520, y: 204, width: 62, height: 28), color: NSColor(calibratedRed: 0.18, green: 0.20, blue: 0.25, alpha: 1), radius: 7)
    drawText("Clear", rect: NSRect(x: 520, y: 210, width: 62, height: 16), size: 12, weight: .medium, color: .white, alignment: .center)
}

let bubbleImage = makeImage(width: 304, height: 172) { rect in
    NSColor.clear.setFill()
    rect.fill()
    drawBubble(in: NSRect(x: 32, y: 32, width: 240, height: 108), recording: true, values: waveformValues(frame: 6, count: 18))
}
try writePNG(bubbleImage, named: "macos-bubble.png")

let settingsImage = makeImage(width: 700, height: 748) { rect in
    drawSettingsWindow(rect)
}
try writePNG(settingsImage, named: "macos-settings.png")

var frames: [NSImage] = []
for frame in 0..<56 {
    frames.append(makeImage(width: 480, height: 220) { rect in
        fillVerticalGradient(
            rect,
            top: NSColor(calibratedRed: 0.07, green: 0.08, blue: 0.10, alpha: 1),
            bottom: NSColor(calibratedRed: 0.11, green: 0.12, blue: 0.15, alpha: 1),
            radius: 0
        )

        roundedFill(NSRect(x: 54, y: 42, width: 372, height: 122), color: NSColor(calibratedRed: 0.96, green: 0.96, blue: 0.94, alpha: 1), radius: 8)
        drawText("Untitled", rect: NSRect(x: 74, y: 136, width: 120, height: 18), size: 11, weight: .medium, color: NSColor.black.withAlphaComponent(0.48))
        let phrase = "Local Nemotron dictation appears here."
        let visible = Int(min(Double(phrase.count), max(0, Double(frame - 10) * 0.95)))
        let text = String(phrase.prefix(visible))
        drawText(text, rect: NSRect(x: 78, y: 88, width: 320, height: 22), size: 15, weight: .regular, color: NSColor.black.withAlphaComponent(0.78))

        roundedFill(NSRect(x: 366, y: 184, width: 72, height: 20), color: NSColor(calibratedWhite: 0.0, alpha: 0.28), radius: 10)
        (frame % 14 < 7 ? red : red.withAlphaComponent(0.60)).setFill()
        NSBezierPath(ovalIn: NSRect(x: 376, y: 190, width: 8, height: 8)).fill()
        drawText("REC", rect: NSRect(x: 390, y: 188, width: 34, height: 12), size: 9, weight: .semibold, color: .white)

        let y = 58 + sin(CGFloat(frame) * 0.08) * 2.0
        drawBubble(in: NSRect(x: 168, y: y, width: 144, height: 65), recording: true, values: waveformValues(frame: frame, count: 18))
    })
}
try writeGIF(frames, named: "macos-demo.gif", delay: 0.055)

print("Rendered macOS README assets in docs/")
