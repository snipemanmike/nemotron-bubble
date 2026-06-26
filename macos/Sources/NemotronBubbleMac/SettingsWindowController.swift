import AppKit

enum SettingsKey: CaseIterable {
    case startAtLogin
    case preloadModel
    case liveTypeIntoCursor
    case copyFinalToClipboard
    case pasteFinalOnStop
    case soundsEnabled
    case waveformEnabled
    case bubbleClickOpensSettings
    case showFloatingBubble
    case menuBarWaveformEnabled
    case enterStopsRecording

    var title: String {
        switch self {
        case .startAtLogin: "Start at Login"
        case .preloadModel: "Preload Nemotron"
        case .liveTypeIntoCursor: "Type Live into Cursor"
        case .copyFinalToClipboard: "Copy Final to Clipboard"
        case .pasteFinalOnStop: "Paste Final on Stop"
        case .soundsEnabled: "Sound Cues"
        case .waveformEnabled: "Bubble Waveform"
        case .bubbleClickOpensSettings: "Bubble Click Opens Settings"
        case .showFloatingBubble: "Floating Bubble"
        case .menuBarWaveformEnabled: "Menu Bar Waveform"
        case .enterStopsRecording: "Enter Stops Recording"
        }
    }
}

final class SettingsWindowController: NSWindowController {
    var onToggleSetting: ((SettingsKey) -> Void)?
    var onStartStop: (() -> Void)?
    var onCopyLatest: (() -> Void)?
    var onClearHistory: (() -> Void)?
    var onCaptureShortcut: (() -> Void)?

    private let statusLabel = NSTextField(labelWithString: "Ready")
    private let transcriptLabel = NSTextField(wrappingLabelWithString: "Tap Ctrl-Space to dictate.")
    private let modelLabel = NSTextField(labelWithString: "")
    private let shortcutButton = NSButton(title: "Ctrl + Space", target: nil, action: nil)
    private let startStopButton = NSButton(title: "Start", target: nil, action: nil)
    private let copyLatestButton = NSButton(title: "Copy Latest", target: nil, action: nil)
    private let clearHistoryButton = NSButton(title: "Clear", target: nil, action: nil)
    private let historyTextView = NSTextView()
    private var toggleButtons: [SettingsKey: NSButton] = [:]

    init() {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 700, height: 748),
            styleMask: [.titled, .closable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = "Nemotron Bubble Settings"
        window.center()
        super.init(window: window)
        buildContent()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func refresh(
        preferences: Preferences,
        status: String,
        transcript: String,
        modelDir: String,
        hotKeyLabel: String,
        isRecording: Bool
    ) {
        statusLabel.stringValue = status
        transcriptLabel.stringValue = transcript.isEmpty ? "Tap \(hotKeyLabel) to dictate." : transcript
        modelLabel.stringValue = modelDir.isEmpty ? "Model: not loaded" : "Model: \(modelDir)"
        shortcutButton.title = hotKeyLabel
        startStopButton.title = isRecording ? "Stop" : "Start"

        toggleButtons[.startAtLogin]?.state = preferences.startAtLogin ? .on : .off
        toggleButtons[.preloadModel]?.state = preferences.preloadModel ? .on : .off
        toggleButtons[.liveTypeIntoCursor]?.state = preferences.liveTypeIntoCursor ? .on : .off
        toggleButtons[.copyFinalToClipboard]?.state = preferences.copyFinalToClipboard ? .on : .off
        toggleButtons[.pasteFinalOnStop]?.state = preferences.pasteFinalOnStop ? .on : .off
        toggleButtons[.soundsEnabled]?.state = preferences.soundsEnabled ? .on : .off
        toggleButtons[.waveformEnabled]?.state = preferences.waveformEnabled ? .on : .off
        toggleButtons[.bubbleClickOpensSettings]?.state = preferences.bubbleClickOpensSettings ? .on : .off
        toggleButtons[.showFloatingBubble]?.state = preferences.showFloatingBubble ? .on : .off
        toggleButtons[.menuBarWaveformEnabled]?.state = preferences.menuBarWaveformEnabled ? .on : .off
        toggleButtons[.enterStopsRecording]?.state = preferences.enterStopsRecording ? .on : .off

        let history = preferences.history
            .map { "\($0.timestamp)\n\($0.text)" }
            .joined(separator: "\n\n")
        historyTextView.string = history.isEmpty ? "No recent dictations." : history
        copyLatestButton.isEnabled = !preferences.history.isEmpty
        clearHistoryButton.isEnabled = !preferences.history.isEmpty
    }

    func show() {
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    func setShortcutCaptureActive(_ active: Bool) {
        shortcutButton.title = active ? "Press shortcut..." : shortcutButton.title
    }

    private func buildContent() {
        guard let contentView = window?.contentView else { return }

        let root = NSStackView()
        root.orientation = .vertical
        root.spacing = 14
        root.edgeInsets = NSEdgeInsets(top: 20, left: 24, bottom: 20, right: 24)
        root.translatesAutoresizingMaskIntoConstraints = false
        contentView.addSubview(root)

        NSLayoutConstraint.activate([
            root.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            root.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            root.topAnchor.constraint(equalTo: contentView.topAnchor),
            root.bottomAnchor.constraint(equalTo: contentView.bottomAnchor)
        ])

        let title = NSTextField(labelWithString: "Nemotron Bubble")
        title.font = .systemFont(ofSize: 24, weight: .semibold)
        root.addArrangedSubview(title)

        statusLabel.font = .systemFont(ofSize: 14, weight: .medium)
        root.addArrangedSubview(statusLabel)

        transcriptLabel.font = .systemFont(ofSize: 13)
        transcriptLabel.maximumNumberOfLines = 3
        root.addArrangedSubview(transcriptLabel)

        modelLabel.font = .systemFont(ofSize: 11)
        modelLabel.textColor = .secondaryLabelColor
        modelLabel.lineBreakMode = .byTruncatingMiddle
        root.addArrangedSubview(modelLabel)

        let commandRow = NSStackView()
        commandRow.orientation = .horizontal
        commandRow.spacing = 8
        root.addArrangedSubview(commandRow)

        startStopButton.bezelStyle = .rounded
        startStopButton.target = self
        startStopButton.action = #selector(startStopClicked)
        commandRow.addArrangedSubview(startStopButton)

        copyLatestButton.bezelStyle = .rounded
        copyLatestButton.target = self
        copyLatestButton.action = #selector(copyLatestClicked)
        commandRow.addArrangedSubview(copyLatestButton)

        clearHistoryButton.bezelStyle = .rounded
        clearHistoryButton.target = self
        clearHistoryButton.action = #selector(clearHistoryClicked)
        commandRow.addArrangedSubview(clearHistoryButton)

        let shortcutRow = NSStackView()
        shortcutRow.orientation = .horizontal
        shortcutRow.spacing = 10
        root.addArrangedSubview(shortcutRow)

        let shortcutLabel = NSTextField(labelWithString: "Global shortcut")
        shortcutLabel.font = .systemFont(ofSize: 13, weight: .medium)
        shortcutRow.addArrangedSubview(shortcutLabel)

        shortcutButton.bezelStyle = .rounded
        shortcutButton.target = self
        shortcutButton.action = #selector(shortcutClicked)
        shortcutRow.addArrangedSubview(shortcutButton)

        let toggles = NSGridView()
        toggles.rowSpacing = 8
        toggles.columnSpacing = 20
        root.addArrangedSubview(toggles)

        for pair in SettingsKey.allCases.chunked(into: 2) {
            let views = pair.map(makeToggle)
            if views.count == 2 {
                toggles.addRow(with: views)
            } else {
                toggles.addRow(with: [views[0], NSView()])
            }
        }

        let historyLabel = NSTextField(labelWithString: "Recent Dictations")
        historyLabel.font = .systemFont(ofSize: 14, weight: .semibold)
        root.addArrangedSubview(historyLabel)

        let scroll = NSScrollView()
        scroll.borderType = .bezelBorder
        scroll.hasVerticalScroller = true
        scroll.translatesAutoresizingMaskIntoConstraints = false
        historyTextView.isEditable = false
        historyTextView.font = .systemFont(ofSize: 12)
        historyTextView.textColor = .labelColor
        historyTextView.backgroundColor = .textBackgroundColor
        scroll.documentView = historyTextView
        root.addArrangedSubview(scroll)
        scroll.heightAnchor.constraint(equalToConstant: 250).isActive = true
    }

    private func makeToggle(_ key: SettingsKey) -> NSButton {
        let button = NSButton(checkboxWithTitle: key.title, target: self, action: #selector(toggleClicked(_:)))
        button.tag = SettingsKey.allCases.firstIndex(of: key) ?? 0
        button.font = .systemFont(ofSize: 13)
        toggleButtons[key] = button
        return button
    }

    @objc private func toggleClicked(_ sender: NSButton) {
        guard sender.tag < SettingsKey.allCases.count else { return }
        onToggleSetting?(SettingsKey.allCases[sender.tag])
    }

    @objc private func startStopClicked() {
        onStartStop?()
    }

    @objc private func copyLatestClicked() {
        onCopyLatest?()
    }

    @objc private func clearHistoryClicked() {
        onClearHistory?()
    }

    @objc private func shortcutClicked() {
        onCaptureShortcut?()
    }
}

private extension Array {
    func chunked(into size: Int) -> [[Element]] {
        stride(from: 0, to: count, by: size).map {
            Array(self[$0..<Swift.min($0 + size, count)])
        }
    }
}
