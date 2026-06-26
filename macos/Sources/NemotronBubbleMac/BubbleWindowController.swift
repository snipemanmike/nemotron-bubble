import AppKit

final class BubbleWindowController {
    private let panel: NSPanel
    private let bubbleView: BubbleView

    var onToggle: (() -> Void)? {
        get { bubbleView.onClick }
        set { bubbleView.onClick = newValue }
    }

    init() {
        let size = WindowsVisualRenderer.bubbleSize
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

    func setWaveformEnabled(_ enabled: Bool) {
        bubbleView.waveformEnabled = enabled
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
            if waveform.count > Self.waveBarCount {
                waveform.removeFirst(waveform.count - Self.waveBarCount)
            }
            needsDisplay = true
        }
    }

    var waveformEnabled = true {
        didSet { needsDisplay = true }
    }

    private var dragStartMouse = NSPoint.zero
    private var dragStartOrigin = NSPoint.zero
    private var didDrag = false
    private var waveform = Array(repeating: CGFloat(0.0), count: 18)

    private static let waveBarCount = 18

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

        guard let image = WindowsVisualRenderer.makeBubbleImage(
            waveform: waveform,
            recording: isRecording,
            waveEnabled: waveformEnabled
        ) else {
            return
        }
        NSGraphicsContext.current?.imageInterpolation = .none
        image.draw(
            in: bounds,
            from: NSRect(origin: .zero, size: image.size),
            operation: .sourceOver,
            fraction: 1.0,
            respectFlipped: true,
            hints: nil
        )
    }
}
