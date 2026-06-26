import AppKit
import ApplicationServices

final class PasteboardTyper {
    func copy(_ text: String) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)
    }

    func paste(_ text: String) -> Bool {
        copy(text)

        guard AXIsProcessTrusted() else {
            promptForAccessibilityIfNeeded()
            return false
        }

        let source = CGEventSource(stateID: .hidSystemState)
        let commandVKey: CGKeyCode = 0x09

        let keyDown = CGEvent(keyboardEventSource: source, virtualKey: commandVKey, keyDown: true)
        keyDown?.flags = .maskCommand

        let keyUp = CGEvent(keyboardEventSource: source, virtualKey: commandVKey, keyDown: false)
        keyUp?.flags = .maskCommand

        keyDown?.post(tap: .cghidEventTap)
        keyUp?.post(tap: .cghidEventTap)
        return true
    }

    func promptForAccessibilityIfNeeded() {
        guard !AXIsProcessTrusted() else { return }
        let promptKey = kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String
        let options = [promptKey: true] as CFDictionary
        _ = AXIsProcessTrustedWithOptions(options)
    }
}
