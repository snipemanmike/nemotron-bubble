import Foundation

final class Preferences {
    private let defaults = UserDefaults.standard

    var copyOnStop: Bool {
        get { bool(for: "copyOnStop", default: true) }
        set { defaults.set(newValue, forKey: "copyOnStop") }
    }

    var pasteOnStop: Bool {
        get { bool(for: "pasteOnStop", default: true) }
        set { defaults.set(newValue, forKey: "pasteOnStop") }
    }

    var showBubble: Bool {
        get { bool(for: "showBubble", default: true) }
        set { defaults.set(newValue, forKey: "showBubble") }
    }

    private func bool(for key: String, default defaultValue: Bool) -> Bool {
        guard defaults.object(forKey: key) != nil else {
            return defaultValue
        }
        return defaults.bool(forKey: key)
    }
}
