import AppKit

final class BubbleWindowController {
    private let panel: NSPanel
    private let bubbleView: BubbleView

    var onToggle: (() -> Void)? {
        get { bubbleView.onClick }
        set { bubbleView.onClick = newValue }
    }

    init() {
        let size = NSSize(width: 152, height: 86)
        bubbleView = BubbleView(frame: NSRect(origin: .zero, size: size))
        panel = NSPanel(
            contentRect: NSRect(origin: .zero, size: size),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )

        panel.contentView = bubbleView
        panel.backgroundColor = .clear
        panel.isOpaque = false
        panel.hasShadow = false
        panel.hidesOnDeactivate = false
        panel.level = .floating
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
    }

    func show() {
        if panel.frame.origin == .zero {
            positionNearLowerRight()
        }
        panel.orderFrontRegardless()
    }

    func hide() {
        panel.orderOut(nil)
    }

    func setRecording(_ isRecording: Bool) {
        bubbleView.isRecording = isRecording
    }

    func setTranscript(_ transcript: String) {
        bubbleView.transcript = transcript
    }

    func setStatus(_ status: String) {
        bubbleView.status = status
    }

    func setLevel(_ level: Float) {
        bubbleView.level = CGFloat(level)
    }

    private func positionNearLowerRight() {
        guard let screen = NSScreen.main else { return }
        let visible = screen.visibleFrame
        panel.setFrameOrigin(NSPoint(
            x: visible.maxX - panel.frame.width - 28,
            y: visible.minY + 28
        ))
    }
}

final class BubbleView: NSView {
    var onClick: (() -> Void)?

    var isRecording = false {
        didSet {
            needsDisplay = true
        }
    }

    var status = "Ready" {
        didSet { needsDisplay = true }
    }

    var transcript = "" {
        didSet { needsDisplay = true }
    }

    var level: CGFloat = 0 {
        didSet {
            waveform.append(max(0, min(1, level)))
            if waveform.count > 18 {
                waveform.removeFirst(waveform.count - 18)
            }
            needsDisplay = true
        }
    }

    private var dragStartMouse = NSPoint.zero
    private var dragStartOrigin = NSPoint.zero
    private var didDrag = false
    private var waveform = Array(repeating: CGFloat(0.08), count: 18)

    override var isFlipped: Bool { true }

    override func mouseDown(with event: NSEvent) {
        dragStartMouse = NSEvent.mouseLocation
        dragStartOrigin = window?.frame.origin ?? .zero
        didDrag = false
    }

    override func mouseDragged(with event: NSEvent) {
        guard let window else { return }
        let mouse = NSEvent.mouseLocation
        let dx = mouse.x - dragStartMouse.x
        let dy = mouse.y - dragStartMouse.y

        if abs(dx) > 3 || abs(dy) > 3 {
            didDrag = true
        }

        window.setFrameOrigin(NSPoint(
            x: dragStartOrigin.x + dx,
            y: dragStartOrigin.y + dy
        ))
    }

    override func mouseUp(with event: NSEvent) {
        if !didDrag {
            onClick?()
        }
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)

        let pillRect = NSRect(x: 16, y: 16, width: 120, height: 54)
        NSGraphicsContext.saveGraphicsState()
        NSShadow().apply {
            $0.shadowOffset = NSSize(width: 0, height: -7)
            $0.shadowBlurRadius = 18
            $0.shadowColor = NSColor.black.withAlphaComponent(0.30)
        }

        let pill = NSBezierPath(roundedRect: pillRect, xRadius: 27, yRadius: 27)
        NSColor(calibratedRed: 0.12, green: 0.14, blue: 0.17, alpha: 0.98).setFill()
        pill.fill()
        NSGraphicsContext.restoreGraphicsState()

        drawMicDot(in: pillRect)
        drawBars(in: pillRect)
    }

    private func drawMicDot(in rect: NSRect) {
        let dotRect = NSRect(x: rect.minX + 18, y: rect.midY - 8, width: 16, height: 16)
        let dot = NSBezierPath(ovalIn: dotRect)
        (isRecording ? NSColor.systemRed : NSColor.systemGreen.withAlphaComponent(0.85)).setFill()
        dot.fill()
    }

    private func drawBars(in rect: NSRect) {
        let barCount = waveform.count
        let areaLeft = rect.minX + 46
        let areaRight = rect.maxX - 16
        let pitch = (areaRight - areaLeft) / CGFloat(max(1, barCount))
        let centerY = rect.midY
        let active = isRecording

        for index in 0..<barCount {
            let value = active ? waveform[index] : 0.10
            let halfHeight = max(2.0, min(19.0, value * 19.0))
            let x = areaLeft + CGFloat(index) * pitch + pitch * 0.32
            let barRect = NSRect(
                x: x,
                y: centerY - halfHeight,
                width: max(2.0, pitch * 0.36),
                height: halfHeight * 2.0
            )
            let path = NSBezierPath(roundedRect: barRect, xRadius: 2, yRadius: 2)
            (isRecording ? NSColor.systemRed : NSColor.systemGreen)
                .withAlphaComponent(isRecording ? 0.82 : 0.30)
                .setFill()
            path.fill()
        }
    }
}

private extension NSShadow {
    func apply(_ configure: (NSShadow) -> Void) {
        configure(self)
        set()
    }
}
