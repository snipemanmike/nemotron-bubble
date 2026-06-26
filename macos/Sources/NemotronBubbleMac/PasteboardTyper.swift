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

    func typeText(_ text: String) -> Bool {
        guard !text.isEmpty else { return true }
        guard AXIsProcessTrusted() else {
            promptForAccessibilityIfNeeded()
            return false
        }

        for chunk in text.utf16.chunked(into: 20) {
            chunk.withUnsafeBufferPointer { buffer in
                guard let baseAddress = buffer.baseAddress else { return }
                let source = CGEventSource(stateID: .hidSystemState)
                let down = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: true)
                down?.keyboardSetUnicodeString(stringLength: buffer.count, unicodeString: baseAddress)
                down?.post(tap: .cghidEventTap)

                let up = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: false)
                up?.keyboardSetUnicodeString(stringLength: buffer.count, unicodeString: baseAddress)
                up?.post(tap: .cghidEventTap)
            }
        }

        return true
    }

    func pressEnter() -> Bool {
        guard AXIsProcessTrusted() else {
            promptForAccessibilityIfNeeded()
            return false
        }

        let source = CGEventSource(stateID: .hidSystemState)
        let enterKey: CGKeyCode = 0x24
        let keyDown = CGEvent(keyboardEventSource: source, virtualKey: enterKey, keyDown: true)
        let keyUp = CGEvent(keyboardEventSource: source, virtualKey: enterKey, keyDown: false)
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

private extension String.UTF16View {
    func chunked(into size: Int) -> [[UInt16]] {
        var chunks: [[UInt16]] = []
        var current: [UInt16] = []
        current.reserveCapacity(size)

        for unit in self {
            current.append(unit)
            if current.count == size {
                chunks.append(current)
                current.removeAll(keepingCapacity: true)
            }
        }

        if !current.isEmpty {
            chunks.append(current)
        }
        return chunks
    }
}
