import Carbon.HIToolbox
import Foundation

final class HotKeyController {
    private let keyCode: UInt32
    private let modifiers: UInt32
    private let onPressed: () -> Void
    private var hotKeyRef: EventHotKeyRef?
    private var handlerRef: EventHandlerRef?

    init(keyCode: UInt32, modifiers: UInt32, onPressed: @escaping () -> Void) {
        self.keyCode = keyCode
        self.modifiers = modifiers
        self.onPressed = onPressed
    }

    deinit {
        unregister()
    }

    func register() -> Bool {
        unregister()

        var eventType = EventTypeSpec(
            eventClass: OSType(kEventClassKeyboard),
            eventKind: UInt32(kEventHotKeyPressed)
        )

        let handler: EventHandlerUPP = { _, eventRef, userData in
            guard let eventRef, let userData else {
                return OSStatus(eventNotHandledErr)
            }

            var hotKeyID = EventHotKeyID()
            let status = GetEventParameter(
                eventRef,
                EventParamName(kEventParamDirectObject),
                EventParamType(typeEventHotKeyID),
                nil,
                MemoryLayout<EventHotKeyID>.size,
                nil,
                &hotKeyID
            )

            guard status == noErr, hotKeyID.signature == HotKeyController.signature else {
                return OSStatus(eventNotHandledErr)
            }

            let controller = Unmanaged<HotKeyController>.fromOpaque(userData).takeUnretainedValue()
            controller.onPressed()
            return noErr
        }

        let installStatus = InstallEventHandler(
            GetEventDispatcherTarget(),
            handler,
            1,
            &eventType,
            Unmanaged.passUnretained(self).toOpaque(),
            &handlerRef
        )
        guard installStatus == noErr else {
            return false
        }

        let hotKeyID = EventHotKeyID(signature: Self.signature, id: 1)
        let registerStatus = RegisterEventHotKey(
            keyCode,
            modifiers,
            hotKeyID,
            GetEventDispatcherTarget(),
            0,
            &hotKeyRef
        )

        if registerStatus != noErr {
            unregister()
            return false
        }

        return true
    }

    func unregister() {
        if let hotKeyRef {
            UnregisterEventHotKey(hotKeyRef)
            self.hotKeyRef = nil
        }

        if let handlerRef {
            RemoveEventHandler(handlerRef)
            self.handlerRef = nil
        }
    }

    private static let signature: OSType = 0x4E54424C
}
