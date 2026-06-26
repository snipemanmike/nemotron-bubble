import AppKit
import Carbon.HIToolbox

final class AppDelegate: NSObject, NSApplicationDelegate {
    private let preferences = Preferences()
    private let engine = EngineClient()
    private let audioCapture = AudioCaptureController()
    private let pasteboardTyper = PasteboardTyper()
    private let bubble = BubbleWindowController()
    private lazy var settingsWindow = SettingsWindowController()

    private var statusItem: NSStatusItem?
    private var hotKey: HotKeyController?
    private var enterHotKey: HotKeyController?
    private var shortcutCaptureMonitor: Any?

    private var startStopItem = NSMenuItem()
    private var settingsItem = NSMenuItem()
    private var copyItem = NSMenuItem()
    private var pasteItem = NSMenuItem()
    private var pasteOnStopItem = NSMenuItem()
    private var copyOnStopItem = NSMenuItem()
    private var liveTypeItem = NSMenuItem()
    private var showBubbleItem = NSMenuItem()

    private var statusText = "Ready"
    private var modelDir = ""
    private var lastTranscript = ""
    private var submitEnterAfterStop = false

    func applicationDidFinishLaunching(_ notification: Notification) {
        configureStatusItem()
        configureBubble()
        configureSettingsWindow()
        configureEngineCallbacks()
        configureAudioCapture()
        engine.start()
        registerHotKey()
        LoginItemController.setEnabled(preferences.startAtLogin)

        if preferences.showFloatingBubble {
            bubble.show()
        }

        if preferences.preloadModel {
            engine.preload()
        }

        refreshAll()
    }

    func applicationWillTerminate(_ notification: Notification) {
        unregisterEnterHotKey()
        audioCapture.stop()
        hotKey?.unregister()
        engine.shutdown()
    }

    private func configureStatusItem() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
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
            guard let self else { return }
            if self.preferences.bubbleClickOpensSettings {
                self.showSettings()
            } else {
                self.toggleRecording()
            }
        }
    }

    private func configureSettingsWindow() {
        settingsWindow.onStartStop = { [weak self] in self?.toggleRecording() }
        settingsWindow.onCopyLatest = { [weak self] in self?.copyLatest() }
        settingsWindow.onClearHistory = { [weak self] in self?.clearHistory() }
        settingsWindow.onCaptureShortcut = { [weak self] in self?.beginShortcutCapture() }
        settingsWindow.onToggleSetting = { [weak self] key in
            self?.toggleSetting(key)
        }
    }

    private func configureEngineCallbacks() {
        engine.onStatus = { [weak self] message in
            self?.setStatus(message)
        }

        engine.onError = { [weak self] message in
            self?.setStatus(message.isEmpty ? "Nemotron error." : message)
        }

        engine.onRecordingChanged = { [weak self] isRecording in
            guard let self else { return }
            self.bubble.setRecording(isRecording)
            if isRecording {
                self.registerEnterHotKeyIfNeeded()
            } else {
                self.audioCapture.stop()
                self.unregisterEnterHotKey()
            }
            self.refreshAll()
        }

        engine.onTranscript = { [weak self] text, delta in
            guard let self else { return }
            self.lastTranscript = text
            self.bubble.setTranscript(text)
            if self.preferences.liveTypeIntoCursor, !delta.isEmpty {
                if !self.pasteboardTyper.typeText(delta) {
                    self.setStatus("Accessibility needed for live typing.")
                }
            }
            self.refreshAll()
        }

        engine.onFinal = { [weak self] text, auto in
            self?.finishTranscript(text, autoStopped: auto)
        }

        engine.onLevel = { [weak self] level in
            guard let self else { return }
            if self.preferences.waveformEnabled {
                self.bubble.setLevel(level)
            } else {
                self.bubble.setLevel(0)
            }
            self.updateMenuBarWaveform(level)
        }

        engine.onModelDir = { [weak self] path in
            self?.modelDir = path
            self?.refreshAll()
        }
    }

    private func configureAudioCapture() {
        audioCapture.onAudio = { [weak self] samples, sampleRate in
            self?.engine.sendAudio(samples: samples, sampleRate: sampleRate)
        }
    }

    private func rebuildMenu() {
        let menu = NSMenu()

        startStopItem = NSMenuItem(
            title: engine.isRecording ? "Stop Dictation" : "Start Dictation",
            action: #selector(toggleRecordingFromMenu),
            keyEquivalent: ""
        )
        startStopItem.target = self
        menu.addItem(startStopItem)

        settingsItem = NSMenuItem(title: "Settings...", action: #selector(openSettings), keyEquivalent: ",")
        settingsItem.target = self
        menu.addItem(settingsItem)

        copyItem = NSMenuItem(title: "Copy Latest", action: #selector(copyLatestFromMenu), keyEquivalent: "")
        copyItem.target = self
        menu.addItem(copyItem)

        pasteItem = NSMenuItem(title: "Paste Latest", action: #selector(pasteLatest), keyEquivalent: "")
        pasteItem.target = self
        menu.addItem(pasteItem)

        menu.addItem(.separator())

        liveTypeItem = toggleMenuItem("Type Live into Cursor", #selector(toggleLiveType))
        menu.addItem(liveTypeItem)

        copyOnStopItem = toggleMenuItem("Copy Final to Clipboard", #selector(toggleCopyOnStop))
        menu.addItem(copyOnStopItem)

        pasteOnStopItem = toggleMenuItem("Paste Final on Stop", #selector(togglePasteOnStop))
        menu.addItem(pasteOnStopItem)

        showBubbleItem = toggleMenuItem("Floating Bubble", #selector(toggleBubble))
        menu.addItem(showBubbleItem)

        menu.addItem(.separator())

        let quitItem = NSMenuItem(title: "Quit", action: #selector(quit), keyEquivalent: "q")
        quitItem.target = self
        menu.addItem(quitItem)

        statusItem?.menu = menu
        refreshMenu()
    }

    private func toggleMenuItem(_ title: String, _ action: Selector) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: "")
        item.target = self
        return item
    }

    private func refreshMenu() {
        startStopItem.title = engine.isRecording ? "Stop Dictation" : "Start Dictation"
        let hasText = !lastTranscript.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        copyItem.isEnabled = hasText
        pasteItem.isEnabled = hasText
        liveTypeItem.state = preferences.liveTypeIntoCursor ? .on : .off
        copyOnStopItem.state = preferences.copyFinalToClipboard ? .on : .off
        pasteOnStopItem.state = preferences.pasteFinalOnStop ? .on : .off
        showBubbleItem.state = preferences.showFloatingBubble ? .on : .off
    }

    private func refreshAll() {
        refreshMenu()
        updateStatusIcon()
        settingsWindow.refresh(
            preferences: preferences,
            status: statusText,
            transcript: lastTranscript,
            modelDir: modelDir,
            hotKeyLabel: hotKeyLabel(),
            isRecording: engine.isRecording
        )
    }

    @objc private func toggleRecordingFromMenu() {
        toggleRecording()
    }

    @objc private func openSettings() {
        showSettings()
    }

    private func showSettings() {
        settingsWindow.show()
        refreshAll()
    }

    private func toggleRecording() {
        if engine.isRecording {
            stopRecording()
        } else {
            startRecording()
        }
    }

    private func startRecording() {
        if preferences.liveTypeIntoCursor || preferences.pasteFinalOnStop || preferences.enterStopsRecording {
            pasteboardTyper.promptForAccessibilityIfNeeded()
        }

        if preferences.showFloatingBubble {
            bubble.show()
        }

        lastTranscript = ""
        bubble.setTranscript("")
        playStartSound()
        setStatus("Starting...")

        do {
            try audioCapture.start()
        } catch {
            let message = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
            setStatus(message)
            return
        }

        engine.startRecording()
    }

    private func stopRecording(fast: Bool = false, submitEnter: Bool = false) {
        submitEnterAfterStop = submitEnter
        audioCapture.stop()
        playStopSound()
        setStatus("Finalizing...")
        engine.stopRecording(fast: fast)
    }

    private func finishTranscript(_ transcript: String, autoStopped: Bool) {
        unregisterEnterHotKey()

        let text = transcript.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else {
            lastTranscript = ""
            bubble.setTranscript("No speech detected.")
            setStatus("Ready. Press \(hotKeyLabel()) to start.")
            return
        }

        lastTranscript = text
        preferences.addHistory(text)
        bubble.setTranscript(text)

        var finalStatus = autoStopped ? "Stopped after silence." : "Stopped."
        if preferences.liveTypeIntoCursor {
            finalStatus = "Stopped. Text was typed live."
        } else if preferences.pasteFinalOnStop {
            finalStatus = pasteboardTyper.paste(text)
                ? "Stopped. Final transcript pasted."
                : "Stopped. Final transcript copied."
        } else if preferences.copyFinalToClipboard {
            pasteboardTyper.copy(text)
            finalStatus = "Stopped. Final transcript copied to clipboard."
        }

        if submitEnterAfterStop {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) {
                _ = self.pasteboardTyper.pressEnter()
            }
        }
        submitEnterAfterStop = false

        setStatus(finalStatus)
    }

    private func setStatus(_ status: String) {
        statusText = status
        bubble.setStatus(status)
        refreshAll()
    }

    @objc private func copyLatestFromMenu() {
        copyLatest()
    }

    private func copyLatest() {
        let text = lastTranscript.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        pasteboardTyper.copy(text)
        setStatus("Latest transcript copied.")
    }

    @objc private func pasteLatest() {
        let text = lastTranscript.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        setStatus(pasteboardTyper.paste(text) ? "Latest transcript pasted." : "Latest transcript copied.")
    }

    private func clearHistory() {
        preferences.clearHistory()
        setStatus("History cleared.")
    }

    @objc private func toggleLiveType() {
        toggleSetting(.liveTypeIntoCursor)
    }

    @objc private func toggleCopyOnStop() {
        toggleSetting(.copyFinalToClipboard)
    }

    @objc private func togglePasteOnStop() {
        toggleSetting(.pasteFinalOnStop)
    }

    @objc private func toggleBubble() {
        toggleSetting(.showFloatingBubble)
    }

    private func toggleSetting(_ key: SettingsKey) {
        switch key {
        case .startAtLogin:
            preferences.startAtLogin.toggle()
            LoginItemController.setEnabled(preferences.startAtLogin)
        case .preloadModel:
            preferences.preloadModel.toggle()
            if preferences.preloadModel {
                engine.preload()
            }
        case .liveTypeIntoCursor:
            preferences.liveTypeIntoCursor.toggle()
            if preferences.liveTypeIntoCursor {
                pasteboardTyper.promptForAccessibilityIfNeeded()
            }
        case .copyFinalToClipboard:
            preferences.copyFinalToClipboard.toggle()
        case .pasteFinalOnStop:
            preferences.pasteFinalOnStop.toggle()
            if preferences.pasteFinalOnStop {
                pasteboardTyper.promptForAccessibilityIfNeeded()
            }
        case .soundsEnabled:
            preferences.soundsEnabled.toggle()
        case .waveformEnabled:
            preferences.waveformEnabled.toggle()
        case .bubbleClickOpensSettings:
            preferences.bubbleClickOpensSettings.toggle()
        case .showFloatingBubble:
            preferences.showFloatingBubble.toggle()
            preferences.showFloatingBubble ? bubble.show() : bubble.hide()
        case .menuBarWaveformEnabled:
            preferences.menuBarWaveformEnabled.toggle()
        case .enterStopsRecording:
            preferences.enterStopsRecording.toggle()
            preferences.enterStopsRecording ? registerEnterHotKeyIfNeeded() : unregisterEnterHotKey()
        }

        setStatus("Settings saved.")
    }

    private func registerHotKey() {
        hotKey?.unregister()
        let controller = HotKeyController(
            keyCode: preferences.hotKeyCode,
            modifiers: preferences.hotKeyModifiers
        ) { [weak self] in
            DispatchQueue.main.async {
                self?.toggleRecording()
            }
        }

        if controller.register() {
            hotKey = controller
        } else {
            setStatus("\(hotKeyLabel()) unavailable. Choose another shortcut.")
        }
    }

    private func beginShortcutCapture() {
        hotKey?.unregister()
        settingsWindow.setShortcutCaptureActive(true)
        setStatus("Press your shortcut... Esc cancels.")
        NSApp.activate(ignoringOtherApps: true)

        shortcutCaptureMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            self?.finishShortcutCapture(event)
            return nil
        }
    }

    private func finishShortcutCapture(_ event: NSEvent) {
        if let monitor = shortcutCaptureMonitor {
            NSEvent.removeMonitor(monitor)
            shortcutCaptureMonitor = nil
        }
        settingsWindow.setShortcutCaptureActive(false)

        if Int(event.keyCode) == kVK_Escape {
            registerHotKey()
            setStatus("Shortcut unchanged.")
            return
        }

        let modifiers = carbonModifiers(from: event.modifierFlags)
        let needsModifier = isAlphaNumericKey(event.keyCode)
        if modifiers == 0 && needsModifier {
            registerHotKey()
            setStatus("Add Control, Option, Shift, or Command to that key.")
            return
        }

        preferences.hotKeyCode = UInt32(event.keyCode)
        preferences.hotKeyModifiers = modifiers
        registerHotKey()
        setStatus("Shortcut set to \(hotKeyLabel()).")
    }

    private func registerEnterHotKeyIfNeeded() {
        guard engine.isRecording, preferences.enterStopsRecording, enterHotKey == nil else { return }
        let controller = HotKeyController(keyCode: UInt32(kVK_Return), modifiers: 0) { [weak self] in
            DispatchQueue.main.async {
                self?.stopRecording(fast: true, submitEnter: true)
            }
        }
        if controller.register() {
            enterHotKey = controller
        }
    }

    private func unregisterEnterHotKey() {
        enterHotKey?.unregister()
        enterHotKey = nil
    }

    private func updateStatusIcon() {
        statusItem?.button?.image = NSImage(
            systemSymbolName: engine.isRecording ? "mic.fill" : "mic.circle",
            accessibilityDescription: "Nemotron Bubble"
        )
        statusItem?.button?.image?.isTemplate = true
        if !engine.isRecording || !preferences.menuBarWaveformEnabled {
            statusItem?.button?.title = ""
        }
    }

    private func updateMenuBarWaveform(_ level: Float) {
        guard engine.isRecording, preferences.menuBarWaveformEnabled else { return }
        let bars = [" ", ".", ":", "|", "||", "|||"]
        let index = min(bars.count - 1, max(0, Int(level * Float(bars.count - 1))))
        statusItem?.button?.title = bars[index]
    }

    private func playStartSound() {
        guard preferences.soundsEnabled else { return }
        NSSound(named: "Tink")?.play()
    }

    private func playStopSound() {
        guard preferences.soundsEnabled else { return }
        NSSound(named: "Pop")?.play()
    }

    private func hotKeyLabel() -> String {
        var parts: [String] = []
        let modifiers = preferences.hotKeyModifiers
        if modifiers & UInt32(controlKey) != 0 { parts.append("Ctrl") }
        if modifiers & UInt32(optionKey) != 0 { parts.append("Option") }
        if modifiers & UInt32(shiftKey) != 0 { parts.append("Shift") }
        if modifiers & UInt32(cmdKey) != 0 { parts.append("Command") }
        parts.append(keyName(preferences.hotKeyCode))
        return parts.joined(separator: " + ")
    }

    private func keyName(_ keyCode: UInt32) -> String {
        switch Int(keyCode) {
        case kVK_Space: "Space"
        case kVK_Return: "Return"
        case kVK_Escape: "Escape"
        case kVK_Tab: "Tab"
        case kVK_Delete: "Delete"
        default:
            if let scalar = keyCodeToString(UInt16(keyCode)) {
                scalar.uppercased()
            } else {
                "Key \(keyCode)"
            }
        }
    }

    private func keyCodeToString(_ keyCode: UInt16) -> String? {
        let source = TISCopyCurrentKeyboardLayoutInputSource().takeRetainedValue()
        guard let data = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData) else {
            return nil
        }
        let keyboardLayout = unsafeBitCast(data, to: CFData.self)
        let pointer = CFDataGetBytePtr(keyboardLayout)
        let layout = unsafeBitCast(pointer, to: UnsafePointer<UCKeyboardLayout>.self)

        var deadKeyState: UInt32 = 0
        var chars = [UniChar](repeating: 0, count: 4)
        var length = 0
        let status = UCKeyTranslate(
            layout,
            keyCode,
            UInt16(kUCKeyActionDisplay),
            0,
            UInt32(LMGetKbdType()),
            OptionBits(kUCKeyTranslateNoDeadKeysBit),
            &deadKeyState,
            chars.count,
            &length,
            &chars
        )
        guard status == noErr, length > 0 else { return nil }
        return String(utf16CodeUnits: chars, count: length)
    }

    private func carbonModifiers(from flags: NSEvent.ModifierFlags) -> UInt32 {
        var modifiers: UInt32 = 0
        if flags.contains(.control) { modifiers |= UInt32(controlKey) }
        if flags.contains(.option) { modifiers |= UInt32(optionKey) }
        if flags.contains(.shift) { modifiers |= UInt32(shiftKey) }
        if flags.contains(.command) { modifiers |= UInt32(cmdKey) }
        return modifiers
    }

    private func isAlphaNumericKey(_ keyCode: UInt16) -> Bool {
        let keys = [
            kVK_ANSI_A, kVK_ANSI_B, kVK_ANSI_C, kVK_ANSI_D, kVK_ANSI_E,
            kVK_ANSI_F, kVK_ANSI_G, kVK_ANSI_H, kVK_ANSI_I, kVK_ANSI_J,
            kVK_ANSI_K, kVK_ANSI_L, kVK_ANSI_M, kVK_ANSI_N, kVK_ANSI_O,
            kVK_ANSI_P, kVK_ANSI_Q, kVK_ANSI_R, kVK_ANSI_S, kVK_ANSI_T,
            kVK_ANSI_U, kVK_ANSI_V, kVK_ANSI_W, kVK_ANSI_X, kVK_ANSI_Y,
            kVK_ANSI_Z, kVK_ANSI_0, kVK_ANSI_1, kVK_ANSI_2, kVK_ANSI_3,
            kVK_ANSI_4, kVK_ANSI_5, kVK_ANSI_6, kVK_ANSI_7, kVK_ANSI_8,
            kVK_ANSI_9
        ]
        return keys.contains(Int(keyCode))
    }

    @objc private func quit() {
        NSApp.terminate(nil)
    }
}
