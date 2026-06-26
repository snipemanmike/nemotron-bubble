import AppKit
import CoreGraphics

enum WindowsVisualRenderer {
    static let bubbleSize = NSSize(width: 152, height: 86)
    static let menuBarPixelSize = 32

    private static let bubbleMargin: Float = 16
    private static let pillWidth: Float = 120
    private static let pillHeight: Float = 54
    private static let waveBarCount = 18

    static func makeBubbleImage(waveform: [CGFloat], recording: Bool, waveEnabled: Bool) -> NSImage? {
        let width = Int(bubbleSize.width)
        let height = Int(bubbleSize.height)
        var pixels = [UInt32](repeating: 0, count: width * height)
        let samples = normalizedWaveform(waveform)
        paintBubble(into: &pixels, width: width, height: height, waveform: samples, recording: recording, waveEnabled: waveEnabled)
        return makeImage(from: pixels, width: width, height: height, pointSize: bubbleSize)
    }

    static func makeMenuBarImage(level: CGFloat, recording: Bool) -> NSImage? {
        let width = menuBarPixelSize
        let height = menuBarPixelSize
        var pixels = [UInt32](repeating: 0, count: width * height)
        paintTrayIcon(into: &pixels, size: width, level: max(0, min(1, Float(level))), recording: recording)
        return makeImage(from: pixels, width: width, height: height, pointSize: NSSize(width: 22, height: 22))
    }

    private static func normalizedWaveform(_ waveform: [CGFloat]) -> [Float] {
        var samples = waveform.map { max(0, min(1, Float($0))) }
        if samples.count < waveBarCount {
            samples = Array(repeating: 0, count: waveBarCount - samples.count) + samples
        } else if samples.count > waveBarCount {
            samples = Array(samples.suffix(waveBarCount))
        }
        return samples
    }

    private static func paintBubble(
        into pixels: inout [UInt32],
        width: Int,
        height: Int,
        waveform: [Float],
        recording: Bool,
        waveEnabled: Bool
    ) {
        let px = bubbleMargin
        let py = bubbleMargin
        let pw = pillWidth
        let ph = pillHeight
        let radius = ph / 2

        fillShadow(
            into: &pixels,
            width: width,
            height: height,
            x: px - 1,
            y: py + 4,
            rectWidth: pw + 2,
            rectHeight: ph + 2,
            radius: radius,
            blur: 13,
            maxAlpha: 0.45
        )

        fillRoundRect(
            into: &pixels,
            width: width,
            height: height,
            x: px,
            y: py,
            rectWidth: pw,
            rectHeight: ph,
            radius: radius,
            top: (46, 51, 62),
            bottom: (26, 29, 36),
            alpha: 0.98
        )

        fillRoundRect(
            into: &pixels,
            width: width,
            height: height,
            x: px + 2,
            y: py + 1.5,
            rectWidth: pw - 4,
            rectHeight: ph * 0.5,
            radius: radius,
            top: (255, 255, 255),
            bottom: (255, 255, 255),
            alpha: 0.05
        )

        let dotRadius: Float = 4.5
        let dotCenterX = px + 15
        let dotCenterY = py + ph / 2
        let dotColor: RGB = recording ? (255, 92, 92) : (104, 112, 126)
        let dotAlpha: Float = recording ? 0.95 : 0.70
        fillRoundRect(
            into: &pixels,
            width: width,
            height: height,
            x: dotCenterX - dotRadius,
            y: dotCenterY - dotRadius,
            rectWidth: dotRadius * 2,
            rectHeight: dotRadius * 2,
            radius: dotRadius,
            top: dotColor,
            bottom: dotColor,
            alpha: dotAlpha
        )

        drawWaveBars(
            into: &pixels,
            width: width,
            height: height,
            waveform: waveform,
            active: recording && waveEnabled,
            leftEdge: dotCenterX + dotRadius,
            pillRight: px + pw,
            y: py,
            pillHeight: ph
        )
    }

    private static func drawWaveBars(
        into pixels: inout [UInt32],
        width: Int,
        height: Int,
        waveform: [Float],
        active: Bool,
        leftEdge: Float,
        pillRight: Float,
        y: Float,
        pillHeight: Float
    ) {
        let count = max(1, waveform.count)
        let areaLeft = leftEdge + 6
        let areaRight = pillRight - 12
        let areaWidth = max(1, areaRight - areaLeft)
        let pitch = areaWidth / Float(count)
        let barWidth = min(4, max(2, pitch * 0.55))
        let centerY = y + pillHeight / 2
        let maxHalf = pillHeight / 2 - 8
        let minHalf: Float = 1.3
        let topColor: RGB = active ? (150, 192, 255) : (78, 86, 100)
        let bottomColor: RGB = active ? (86, 148, 250) : (60, 66, 78)
        let alpha: Float = active ? 0.97 : 0.50

        for (index, level) in waveform.enumerated() {
            let value = active ? max(0, min(1, level)) : 0
            let halfHeight = max(minHalf, value * maxHalf)
            let centerX = areaLeft + pitch * (Float(index) + 0.5)
            fillRoundRect(
                into: &pixels,
                width: width,
                height: height,
                x: centerX - barWidth / 2,
                y: centerY - halfHeight,
                rectWidth: barWidth,
                rectHeight: halfHeight * 2,
                radius: barWidth / 2,
                top: topColor,
                bottom: bottomColor,
                alpha: alpha
            )
        }
    }

    private static func paintTrayIcon(into pixels: inout [UInt32], size: Int, level: Float, recording: Bool) {
        let center = (Float(size) - 1) / 2
        let background: UInt32 = recording ? 0xFF20242E : 0xFF1A1D21

        for y in 0..<size {
            for x in 0..<size {
                let dx = Float(x) - center
                let dy = Float(y) - center
                if sqrt(dx * dx + dy * dy) <= 14.5 {
                    pixels[y * size + x] = background
                }
            }
        }

        let shape: [Float] = [0.30, 0.58, 0.86, 1.0, 0.72, 0.46, 0.66, 0.9, 0.5]
        let accent: UInt32 = recording ? 0xFF7AB8FF : 0xFF59616F
        for (index, shapeValue) in shape.enumerated() {
            let fraction = recording ? min(1, 0.18 + shapeValue * level * 0.95) : shapeValue * 0.5
            let barHeight = Int(fraction * 22)
            let x = 7 + index * 2
            let top = 16 - barHeight / 2
            let bottom = 16 + barHeight / 2
            for yy in top...bottom {
                for xx in x...(x + 1) {
                    if (0..<size).contains(xx), (0..<size).contains(yy) {
                        pixels[yy * size + xx] = accent
                    }
                }
            }
        }

        if recording {
            for y in 3..<9 {
                for x in 23..<29 {
                    let dx = Float(x) - 25.5
                    let dy = Float(y) - 5.5
                    if dx * dx + dy * dy <= 6 {
                        pixels[y * size + x] = 0xFFFF5C5C
                    }
                }
            }
        }
    }

    private typealias RGB = (red: UInt8, green: UInt8, blue: UInt8)

    private static func fillRoundRect(
        into pixels: inout [UInt32],
        width: Int,
        height: Int,
        x: Float,
        y: Float,
        rectWidth: Float,
        rectHeight: Float,
        radius: Float,
        top: RGB,
        bottom: RGB,
        alpha: Float
    ) {
        guard rectWidth > 0, rectHeight > 0 else { return }

        let centerX = x + rectWidth / 2
        let centerY = y + rectHeight / 2
        let halfWidth = rectWidth / 2
        let halfHeight = rectHeight / 2
        let cornerRadius = max(0, min(radius, min(halfWidth, halfHeight)))
        let minX = max(0, Int(floor(x)) - 1)
        let minY = max(0, Int(floor(y)) - 1)
        let maxX = min(width, Int(ceil(x + rectWidth)) + 1)
        let maxY = min(height, Int(ceil(y + rectHeight)) + 1)

        for pixelY in minY..<maxY {
            let fy = Float(pixelY) + 0.5
            let t = max(0, min(1, (fy - y) / rectHeight))
            let red = lerp(Float(top.red), Float(bottom.red), t)
            let green = lerp(Float(top.green), Float(bottom.green), t)
            let blue = lerp(Float(top.blue), Float(bottom.blue), t)

            for pixelX in minX..<maxX {
                let fx = Float(pixelX) + 0.5
                let distance = signedDistanceRoundRect(
                    px: fx,
                    py: fy,
                    centerX: centerX,
                    centerY: centerY,
                    halfWidth: halfWidth,
                    halfHeight: halfHeight,
                    radius: cornerRadius
                )
                let coverage = max(0, min(1, 0.5 - distance))
                if coverage > 0 {
                    blendPixel(&pixels, index: pixelY * width + pixelX, red: red, green: green, blue: blue, alpha: coverage * alpha)
                }
            }
        }
    }

    private static func fillShadow(
        into pixels: inout [UInt32],
        width: Int,
        height: Int,
        x: Float,
        y: Float,
        rectWidth: Float,
        rectHeight: Float,
        radius: Float,
        blur: Float,
        maxAlpha: Float
    ) {
        let centerX = x + rectWidth / 2
        let centerY = y + rectHeight / 2
        let halfWidth = rectWidth / 2
        let halfHeight = rectHeight / 2
        let cornerRadius = max(0, min(radius, min(halfWidth, halfHeight)))
        let minX = max(0, Int(floor(x - blur)))
        let minY = max(0, Int(floor(y - blur)))
        let maxX = min(width, Int(ceil(x + rectWidth + blur)))
        let maxY = min(height, Int(ceil(y + rectHeight + blur)))

        for pixelY in minY..<maxY {
            let fy = Float(pixelY) + 0.5
            for pixelX in minX..<maxX {
                let fx = Float(pixelX) + 0.5
                let distance = max(0, signedDistanceRoundRect(
                    px: fx,
                    py: fy,
                    centerX: centerX,
                    centerY: centerY,
                    halfWidth: halfWidth,
                    halfHeight: halfHeight,
                    radius: cornerRadius
                ))
                let coverage = max(0, min(1, 1 - distance / blur))
                if coverage > 0 {
                    let alpha = coverage * coverage * maxAlpha
                    blendPixel(&pixels, index: pixelY * width + pixelX, red: 0, green: 0, blue: 0, alpha: alpha)
                }
            }
        }
    }

    private static func signedDistanceRoundRect(
        px: Float,
        py: Float,
        centerX: Float,
        centerY: Float,
        halfWidth: Float,
        halfHeight: Float,
        radius: Float
    ) -> Float {
        let qx = abs(px - centerX) - (halfWidth - radius)
        let qy = abs(py - centerY) - (halfHeight - radius)
        let ax = max(qx, 0)
        let ay = max(qy, 0)
        return sqrt(ax * ax + ay * ay) + min(max(qx, qy), 0) - radius
    }

    private static func blendPixel(_ pixels: inout [UInt32], index: Int, red: Float, green: Float, blue: Float, alpha: Float) {
        guard alpha > 0 else { return }
        let sourceAlpha = min(alpha, 1)
        let destination = pixels[index]
        let destinationAlpha = Float((destination >> 24) & 0xff)
        let destinationRed = Float((destination >> 16) & 0xff)
        let destinationGreen = Float((destination >> 8) & 0xff)
        let destinationBlue = Float(destination & 0xff)
        let inverseAlpha = 1 - sourceAlpha

        let outRed = red * sourceAlpha + destinationRed * inverseAlpha
        let outGreen = green * sourceAlpha + destinationGreen * inverseAlpha
        let outBlue = blue * sourceAlpha + destinationBlue * inverseAlpha
        let outAlpha = sourceAlpha * 255 + destinationAlpha * inverseAlpha

        pixels[index] = (UInt32(outAlpha) << 24)
            | (UInt32(outRed) << 16)
            | (UInt32(outGreen) << 8)
            | UInt32(outBlue)
    }

    private static func lerp(_ start: Float, _ end: Float, _ amount: Float) -> Float {
        start + (end - start) * amount
    }

    private static func makeImage(from pixels: [UInt32], width: Int, height: Int, pointSize: NSSize) -> NSImage? {
        var bytes = [UInt8](repeating: 0, count: width * height * 4)
        for (index, pixel) in pixels.enumerated() {
            let offset = index * 4
            bytes[offset] = UInt8((pixel >> 16) & 0xff)
            bytes[offset + 1] = UInt8((pixel >> 8) & 0xff)
            bytes[offset + 2] = UInt8(pixel & 0xff)
            bytes[offset + 3] = UInt8((pixel >> 24) & 0xff)
        }

        let data = Data(bytes)
        guard let provider = CGDataProvider(data: data as CFData),
              let colorSpace = CGColorSpace(name: CGColorSpace.sRGB),
              let cgImage = CGImage(
                width: width,
                height: height,
                bitsPerComponent: 8,
                bitsPerPixel: 32,
                bytesPerRow: width * 4,
                space: colorSpace,
                bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue),
                provider: provider,
                decode: nil,
                shouldInterpolate: true,
                intent: .defaultIntent
              )
        else {
            return nil
        }

        let image = NSImage(cgImage: cgImage, size: pointSize)
        image.isTemplate = false
        return image
    }
}
