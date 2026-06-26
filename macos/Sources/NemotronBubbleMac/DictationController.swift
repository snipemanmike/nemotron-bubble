import AVFoundation
import Speech

enum DictationError: LocalizedError {
    case speechPermissionDenied
    case microphonePermissionDenied
    case speechRecognizerUnavailable
    case microphoneUnavailable

    var errorDescription: String? {
        switch self {
        case .speechPermissionDenied:
            "Enable Speech Recognition in System Settings."
        case .microphonePermissionDenied:
            "Enable Microphone access in System Settings."
        case .speechRecognizerUnavailable:
            "Speech recognition is unavailable right now."
        case .microphoneUnavailable:
            "No microphone input is available."
        }
    }
}

final class DictationController {
    var onRecordingChanged: ((Bool) -> Void)?
    var onTranscriptChanged: ((String) -> Void)?
    var onLevelChanged: ((Float) -> Void)?
    var onError: ((String) -> Void)?

    private let recognizer = SFSpeechRecognizer(locale: Locale(identifier: "en-US"))
    private let audioEngine = AVAudioEngine()
    private var recognitionRequest: SFSpeechAudioBufferRecognitionRequest?
    private var recognitionTask: SFSpeechRecognitionTask?
    private var latestTranscript = ""
    private var stopCompletion: ((String) -> Void)?

    private(set) var isRecording = false

    func requestPermissions(completion: @escaping (Bool, String?) -> Void) {
        var speechGranted = false
        var microphoneGranted = false
        let group = DispatchGroup()

        group.enter()
        SFSpeechRecognizer.requestAuthorization { status in
            speechGranted = status == .authorized
            group.leave()
        }

        group.enter()
        AVCaptureDevice.requestAccess(for: .audio) { granted in
            microphoneGranted = granted
            group.leave()
        }

        group.notify(queue: .main) {
            if speechGranted && microphoneGranted {
                completion(true, "Ready")
            } else if !speechGranted {
                completion(false, DictationError.speechPermissionDenied.errorDescription)
            } else {
                completion(false, DictationError.microphonePermissionDenied.errorDescription)
            }
        }
    }

    func start() throws {
        guard SFSpeechRecognizer.authorizationStatus() == .authorized else {
            throw DictationError.speechPermissionDenied
        }

        guard AVCaptureDevice.authorizationStatus(for: .audio) == .authorized else {
            throw DictationError.microphonePermissionDenied
        }

        guard recognizer?.isAvailable == true else {
            throw DictationError.speechRecognizerUnavailable
        }

        if isRecording {
            return
        }

        latestTranscript = ""
        recognitionTask?.cancel()
        recognitionTask = nil

        let request = SFSpeechAudioBufferRecognitionRequest()
        request.shouldReportPartialResults = true
        if #available(macOS 13.0, *) {
            request.addsPunctuation = true
        }
        recognitionRequest = request

        let inputNode = audioEngine.inputNode
        let format = inputNode.outputFormat(forBus: 0)
        guard format.channelCount > 0 else {
            throw DictationError.microphoneUnavailable
        }

        inputNode.removeTap(onBus: 0)
        inputNode.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak self, weak request] buffer, _ in
            guard let self, let request else { return }
            request.append(buffer)
            self.publishLevel(from: buffer)
        }

        recognitionTask = recognizer?.recognitionTask(with: request) { [weak self] result, error in
            guard let self else { return }

            if let result {
                self.latestTranscript = result.bestTranscription.formattedString
                self.onTranscriptChanged?(self.latestTranscript)
                if result.isFinal {
                    self.finishStopIfNeeded()
                }
            }

            if let error, self.isRecording {
                self.onError?(error.localizedDescription)
                self.stop { _ in }
            }
        }

        audioEngine.prepare()
        try audioEngine.start()
        isRecording = true
        onRecordingChanged?(true)
    }

    func stop(completion: @escaping (String) -> Void) {
        guard isRecording else {
            completion(latestTranscript)
            return
        }

        stopCompletion = completion
        isRecording = false
        onRecordingChanged?(false)

        audioEngine.stop()
        audioEngine.inputNode.removeTap(onBus: 0)
        recognitionRequest?.endAudio()

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.7) { [weak self] in
            self?.finishStopIfNeeded()
        }
    }

    func cancel() {
        isRecording = false
        audioEngine.stop()
        audioEngine.inputNode.removeTap(onBus: 0)
        recognitionTask?.cancel()
        recognitionTask = nil
        recognitionRequest = nil
    }

    private func finishStopIfNeeded() {
        guard let completion = stopCompletion else { return }
        stopCompletion = nil
        recognitionTask?.cancel()
        recognitionTask = nil
        recognitionRequest = nil
        completion(latestTranscript)
    }

    private func publishLevel(from buffer: AVAudioPCMBuffer) {
        guard let channels = buffer.floatChannelData else { return }

        let channelCount = Int(buffer.format.channelCount)
        let frameCount = Int(buffer.frameLength)
        guard channelCount > 0, frameCount > 0 else { return }

        var sum: Float = 0
        for channel in 0..<channelCount {
            let samples = channels[channel]
            for frame in 0..<frameCount {
                let sample = samples[frame]
                sum += sample * sample
            }
        }

        let rms = sqrt(sum / Float(channelCount * frameCount))
        onLevelChanged?(min(1, rms * 22))
    }
}
