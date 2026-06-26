import Foundation

struct HistoryItem: Codable {
    let timestamp: String
    let text: String
}

final class Preferences {
    private let defaults = UserDefaults.standard

    var startAtLogin: Bool {
        get { bool(for: "startAtLogin", default: true) }
        set { defaults.set(newValue, forKey: "startAtLogin") }
    }

    var preloadModel: Bool {
        get { bool(for: "preloadModel", default: true) }
        set { defaults.set(newValue, forKey: "preloadModel") }
    }

    var liveTypeIntoCursor: Bool {
        get { bool(for: "liveTypeIntoCursor", default: false) }
        set { defaults.set(newValue, forKey: "liveTypeIntoCursor") }
    }

    var copyFinalToClipboard: Bool {
        get { bool(for: "copyFinalToClipboard", default: true) }
        set { defaults.set(newValue, forKey: "copyFinalToClipboard") }
    }

    var pasteFinalOnStop: Bool {
        get { bool(for: "pasteFinalOnStop", default: true) }
        set { defaults.set(newValue, forKey: "pasteFinalOnStop") }
    }

    var soundsEnabled: Bool {
        get { bool(for: "soundsEnabled", default: true) }
        set { defaults.set(newValue, forKey: "soundsEnabled") }
    }

    var waveformEnabled: Bool {
        get { bool(for: "waveformEnabled", default: true) }
        set { defaults.set(newValue, forKey: "waveformEnabled") }
    }

    var bubbleClickOpensSettings: Bool {
        get { bool(for: "bubbleClickOpensSettings", default: true) }
        set { defaults.set(newValue, forKey: "bubbleClickOpensSettings") }
    }

    var showFloatingBubble: Bool {
        get { bool(for: "showFloatingBubble", default: true) }
        set { defaults.set(newValue, forKey: "showFloatingBubble") }
    }

    var menuBarWaveformEnabled: Bool {
        get { bool(for: "menuBarWaveformEnabled", default: true) }
        set { defaults.set(newValue, forKey: "menuBarWaveformEnabled") }
    }

    var enterStopsRecording: Bool {
        get { bool(for: "enterStopsRecording", default: false) }
        set { defaults.set(newValue, forKey: "enterStopsRecording") }
    }

    var commandHoldToRecord: Bool {
        get { bool(for: "commandHoldToRecord", default: true) }
        set { defaults.set(newValue, forKey: "commandHoldToRecord") }
    }

    var pasteDelayMs: Int {
        get {
            guard defaults.object(forKey: "pasteDelayMs") != nil else { return 60 }
            return defaults.integer(forKey: "pasteDelayMs")
        }
        set { defaults.set(newValue, forKey: "pasteDelayMs") }
    }

    var historyLimit: Int {
        get {
            guard defaults.object(forKey: "historyLimit") != nil else { return 20 }
            return defaults.integer(forKey: "historyLimit")
        }
        set { defaults.set(newValue, forKey: "historyLimit") }
    }

    var hotKeyCode: UInt32 {
        get {
            guard defaults.object(forKey: "hotKeyCode") != nil else { return 49 }
            return UInt32(defaults.integer(forKey: "hotKeyCode"))
        }
        set { defaults.set(Int(newValue), forKey: "hotKeyCode") }
    }

    var hotKeyModifiers: UInt32 {
        get {
            guard defaults.object(forKey: "hotKeyModifiers") != nil else { return 0x1000 }
            return UInt32(defaults.integer(forKey: "hotKeyModifiers"))
        }
        set { defaults.set(Int(newValue), forKey: "hotKeyModifiers") }
    }

    var history: [HistoryItem] {
        get {
            guard let data = defaults.data(forKey: "history") else { return [] }
            return (try? JSONDecoder().decode([HistoryItem].self, from: data)) ?? []
        }
        set {
            if let data = try? JSONEncoder().encode(newValue) {
                defaults.set(data, forKey: "history")
            }
        }
    }

    func addHistory(_ text: String) {
        let clean = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !clean.isEmpty else { return }

        var items = history
        items.insert(HistoryItem(timestamp: Self.timestamp(), text: clean), at: 0)
        items = Array(items.prefix(max(1, historyLimit)))
        history = items
    }

    func clearHistory() {
        history = []
    }

    private func bool(for key: String, default defaultValue: Bool) -> Bool {
        guard defaults.object(forKey: key) != nil else {
            return defaultValue
        }
        return defaults.bool(forKey: key)
    }

    private static func timestamp() -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "MMM d, h:mm a"
        return formatter.string(from: Date())
    }
}
