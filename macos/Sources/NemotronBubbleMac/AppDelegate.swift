import AppKit
import Carbon.HIToolbox

final class AppDelegate: NSObject, NSApplicationDelegate {
    private let preferences = Preferences()
    private let dictation = DictationController()
    private let pasteboardTyper = PasteboardTyper()
    private let bubble = BubbleWindowController()

    private var statusItem: NSStatusItem?
    private var hotKey: HotKeyController?
    private var startStopItem = NSMenuItem()
    private var copyItem = NSMenuItem()
    private var pasteItem = NSMenuItem()
    private var pasteOnStopItem = NSMenuItem()
    private var copyOnStopItem = NSMenuItem()
    private var showBubbleItem = NSMenuItem()

    private var lastTranscript = ""

    func applicationDidFinishLaunching(_ notification: Notification) {
        configureStatusItem()
        configureBubble()
        configureDictationCallbacks()
        requestSpeechAndMicAccess()
        registerHotKey()

        if preferences.showBubble {
            bubble.show()
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        hotKey?.unregister()
        dictation.cancel()
    }

    private func configureStatusItem() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        item.button?.image = NSImage(
            systemSymbolName: "mic.circle",
            accessibilityDescription: "Nemotron Bubble"
        )
        item.button?.image?.isTemplate = true
        statusItem = item
        rebuildMenu()
    }

    private func configureBubble() {
        bubble.onToggle = { [weak self] in
            self?.toggleRecording()
        }
    }

    private func configureDictationCallbacks() {
        dictation.onRecordingChanged = { [weak self] isRecording in
            DispatchQueue.main.async {
                self?.bubble.setRecording(isRecording)
                self?.refreshMenu()
            }
        }

        dictation.onTranscriptChanged = { [weak self] transcript in
            DispatchQueue.main.async {
                self?.lastTranscript = transcript
                self?.bubble.setTranscript(transcript)
                self?.refreshMenu()
            }
        }

        dictation.onLevelChanged = { [weak self] level in
            DispatchQueue.main.async {
                self?.bubble.setLevel(level)
            }
        }

        dictation.onError = { [weak self] message in
            DispatchQueue.main.async {
                self?.bubble.setStatus(message)
            }
        }
    }

    private func requestSpeechAndMicAccess() {
        dictation.requestPermissions { [weak self] _, message in
            guard let self else { return }
            self.bubble.setStatus(message ?? "Ready")
        }
    }

    private func registerHotKey() {
        let controller = HotKeyController(
            keyCode: UInt32(kVK_Space),
            modifiers: UInt32(controlKey)
        ) { [weak self] in
            DispatchQueue.main.async {
                self?.toggleRecording()
            }
        }

        if controller.register() {
            hotKey = controller
        } else {
            bubble.setStatus("Ctrl-Space unavailable")
        }
    }

    private func rebuildMenu() {
        let menu = NSMenu()

        startStopItem = NSMenuItem(
            title: dictation.isRecording ? "Stop Dictation" : "Start Dictation",
            action: #selector(toggleRecordingFromMenu),
            keyEquivalent: ""
        )
        startStopItem.target = self
        menu.addItem(startStopItem)

        copyItem = NSMenuItem(title: "Copy Latest", action: #selector(copyLatest), keyEquivalent: "")
        copyItem.target = self
        copyItem.isEnabled = !lastTranscript.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        menu.addItem(copyItem)

        pasteItem = NSMenuItem(title: "Paste Latest", action: #selector(pasteLatest), keyEquivalent: "")
        pasteItem.target = self
        pasteItem.isEnabled = copyItem.isEnabled
        menu.addItem(pasteItem)

        menu.addItem(.separator())

        copyOnStopItem = NSMenuItem(
            title: "Copy on Stop",
            action: #selector(toggleCopyOnStop),
            keyEquivalent: ""
        )
        copyOnStopItem.target = self
        copyOnStopItem.state = preferences.copyOnStop ? .on : .off
        menu.addItem(copyOnStopItem)

        pasteOnStopItem = NSMenuItem(
            title: "Paste on Stop",
            action: #selector(togglePasteOnStop),
            keyEquivalent: ""
        )
        pasteOnStopItem.target = self
        pasteOnStopItem.state = preferences.pasteOnStop ? .on : .off
        menu.addItem(pasteOnStopItem)

        showBubbleItem = NSMenuItem(
            title: "Show Bubble",
            action: #selector(toggleBubble),
            keyEquivalent: ""
        )
        showBubbleItem.target = self
        showBubbleItem.state = preferences.showBubble ? .on : .off
        menu.addItem(showBubbleItem)

        menu.addItem(.separator())

        let quitItem = NSMenuItem(title: "Quit", action: #selector(quit), keyEquivalent: "q")
        quitItem.target = self
        menu.addItem(quitItem)

        statusItem?.menu = menu
    }

    private func refreshMenu() {
        startStopItem.title = dictation.isRecording ? "Stop Dictation" : "Start Dictation"
        copyItem.isEnabled = !lastTranscript.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        pasteItem.isEnabled = copyItem.isEnabled
        copyOnStopItem.state = preferences.copyOnStop ? .on : .off
        pasteOnStopItem.state = preferences.pasteOnStop ? .on : .off
        showBubbleItem.state = preferences.showBubble ? .on : .off
    }

    @objc private func toggleRecordingFromMenu() {
        toggleRecording()
    }

    private func toggleRecording() {
        if dictation.isRecording {
            stopRecording()
        } else {
            startRecording()
        }
    }

    private func startRecording() {
        if preferences.pasteOnStop {
            pasteboardTyper.promptForAccessibilityIfNeeded()
        }

        do {
            bubble.show()
            bubble.setTranscript("")
            bubble.setStatus("Listening")
            try dictation.start()
        } catch {
            let message = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
            bubble.setStatus(message)
        }
    }

    private func stopRecording() {
        bubble.setStatus("Finishing")
        dictation.stop { [weak self] transcript in
            DispatchQueue.main.async {
                self?.finishTranscript(transcript)
            }
        }
    }

    private func finishTranscript(_ transcript: String) {
        let text = transcript.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else {
            bubble.setStatus("Ready")
            return
        }

        lastTranscript = text
        bubble.setTranscript(text)

        if preferences.copyOnStop {
            pasteboardTyper.copy(text)
        }

        if preferences.pasteOnStop {
            if pasteboardTyper.paste(text) {
                bubble.setStatus("Pasted")
            } else {
                bubble.setStatus("Copied")
            }
        } else if preferences.copyOnStop {
            bubble.setStatus("Copied")
        } else {
            bubble.setStatus("Ready")
        }

        refreshMenu()
    }

    @objc private func copyLatest() {
        let text = lastTranscript.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        pasteboardTyper.copy(text)
        bubble.setStatus("Copied")
    }

    @objc private func pasteLatest() {
        let text = lastTranscript.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        if !pasteboardTyper.paste(text) {
            bubble.setStatus("Copied")
        }
    }

    @objc private func toggleCopyOnStop() {
        preferences.copyOnStop.toggle()
        refreshMenu()
    }

    @objc private func togglePasteOnStop() {
        preferences.pasteOnStop.toggle()
        if preferences.pasteOnStop {
            pasteboardTyper.promptForAccessibilityIfNeeded()
        }
        refreshMenu()
    }

    @objc private func toggleBubble() {
        preferences.showBubble.toggle()
        if preferences.showBubble {
            bubble.show()
        } else {
            bubble.hide()
        }
        refreshMenu()
    }

    @objc private func quit() {
        NSApp.terminate(nil)
    }
}
