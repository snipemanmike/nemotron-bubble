import AVFoundation

enum AudioCaptureError: LocalizedError {
    case microphoneDenied
    case microphoneUnavailable

    var errorDescription: String? {
        switch self {
        case .microphoneDenied:
            "Enable Microphone access for Nemotron Bubble in System Settings."
        case .microphoneUnavailable:
            "No microphone input is available."
        }
    }
}

final class AudioCaptureController {
    var onAudio: ((_ samples: [Float], _ sampleRate: Double) -> Void)?

    private let audioEngine = AVAudioEngine()
    private let callbackQueue = DispatchQueue(label: "com.snipemanmike.NemotronBubbleMac.audio")

    private(set) var isRunning = false

    func requestPermission(completion: @escaping (Bool) -> Void) {
        AVCaptureDevice.requestAccess(for: .audio) { granted in
            DispatchQueue.main.async {
                completion(granted)
            }
        }
    }

    func start() throws {
        if AVCaptureDevice.authorizationStatus(for: .audio) == .notDetermined {
            let semaphore = DispatchSemaphore(value: 0)
            var granted = false
            AVCaptureDevice.requestAccess(for: .audio) { allowed in
                granted = allowed
                semaphore.signal()
            }
            semaphore.wait()
            if !granted {
                throw AudioCaptureError.microphoneDenied
            }
        }

        guard AVCaptureDevice.authorizationStatus(for: .audio) == .authorized else {
            throw AudioCaptureError.microphoneDenied
        }

        if isRunning {
            return
        }

        let inputNode = audioEngine.inputNode
        let format = inputNode.outputFormat(forBus: 0)
        guard format.channelCount > 0 else {
            throw AudioCaptureError.microphoneUnavailable
        }

        inputNode.removeTap(onBus: 0)
        inputNode.installTap(onBus: 0, bufferSize: 4096, format: format) { [weak self] buffer, _ in
            guard let self else { return }
            let sampleRate = buffer.format.sampleRate
            let samples = Self.monoSamples(from: buffer)
            guard !samples.isEmpty else { return }

            self.callbackQueue.async {
                self.onAudio?(samples, sampleRate)
            }
        }

        audioEngine.prepare()
        try audioEngine.start()
        isRunning = true
    }

    func stop() {
        guard isRunning else { return }
        audioEngine.stop()
        audioEngine.inputNode.removeTap(onBus: 0)
        isRunning = false
    }

    private static func monoSamples(from buffer: AVAudioPCMBuffer) -> [Float] {
        guard let channels = buffer.floatChannelData else { return [] }

        let channelCount = Int(buffer.format.channelCount)
        let frameCount = Int(buffer.frameLength)
        guard channelCount > 0, frameCount > 0 else { return [] }

        if channelCount == 1 {
            return Array(UnsafeBufferPointer(start: channels[0], count: frameCount))
        }

        var mono = [Float](repeating: 0, count: frameCount)
        for channel in 0..<channelCount {
            let source = channels[channel]
            for frame in 0..<frameCount {
                mono[frame] += source[frame]
            }
        }

        let scale = 1.0 / Float(channelCount)
        for frame in 0..<frameCount {
            mono[frame] *= scale
        }
        return mono
    }
}
