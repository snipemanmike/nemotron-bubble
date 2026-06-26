import AppKit

final class BubbleWindowController {
    private let panel: NSPanel
    private let bubbleView: BubbleView

    var onToggle: (() -> Void)? {
        get { bubbleView.onClick }
        set { bubbleView.onClick = newValue }
    }

    init() {
        let size = NSSize(width: 206, height: 84)
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
            status = isRecording ? "Listening" : "Ready"
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
        didSet { needsDisplay = true }
    }

    private var dragStartMouse = NSPoint.zero
    private var dragStartOrigin = NSPoint.zero
    private var didDrag = false

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

        let shadowRect = bounds.insetBy(dx: 8, dy: 8)
        NSGraphicsContext.saveGraphicsState()
        NSShadow().apply {
            $0.shadowOffset = NSSize(width: 0, height: 7)
            $0.shadowBlurRadius = 18
            $0.shadowColor = NSColor.black.withAlphaComponent(0.24)
        }

        let pill = NSBezierPath(roundedRect: shadowRect, xRadius: 24, yRadius: 24)
        NSColor(calibratedWhite: 0.08, alpha: 0.90).setFill()
        pill.fill()
        NSGraphicsContext.restoreGraphicsState()

        drawMicDot(in: shadowRect)
        drawText(in: shadowRect)
        drawBars(in: shadowRect)
    }

    private func drawMicDot(in rect: NSRect) {
        let dotRect = NSRect(x: rect.minX + 18, y: rect.minY + 23, width: 18, height: 18)
        let dot = NSBezierPath(ovalIn: dotRect)
        (isRecording ? NSColor.systemRed : NSColor.systemGreen).setFill()
        dot.fill()
    }

    private func drawText(in rect: NSRect) {
        let title = status as NSString
        let titleAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 13, weight: .semibold),
            .foregroundColor: NSColor.white
        ]
        title.draw(in: NSRect(x: rect.minX + 46, y: rect.minY + 15, width: 132, height: 18), withAttributes: titleAttrs)

        let preview = transcript.isEmpty ? "Ctrl-Space" : transcript
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineBreakMode = .byTruncatingTail

        let bodyAttrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 11, weight: .regular),
            .foregroundColor: NSColor.white.withAlphaComponent(0.70),
            .paragraphStyle: paragraph
        ]
        (preview as NSString).draw(
            in: NSRect(x: rect.minX + 46, y: rect.minY + 34, width: 132, height: 16),
            withAttributes: bodyAttrs
        )
    }

    private func drawBars(in rect: NSRect) {
        let barCount = 9
        let baseX = rect.minX + 46
        let baseY = rect.minY + 56
        let boostedLevel = min(1, max(0.04, level))

        for index in 0..<barCount {
            let phase = CGFloat(index) / CGFloat(max(1, barCount - 1))
            let wave = 0.35 + 0.65 * sin((phase + boostedLevel) * .pi)
            let height = max(3, 14 * boostedLevel * wave)
            let barRect = NSRect(
                x: baseX + CGFloat(index) * 8,
                y: baseY + (15 - height) / 2,
                width: 4,
                height: height
            )
            let path = NSBezierPath(roundedRect: barRect, xRadius: 2, yRadius: 2)
            (isRecording ? NSColor.systemRed : NSColor.systemGreen)
                .withAlphaComponent(isRecording ? 0.82 : 0.34)
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
