import Foundation

struct EngineMessage: Decodable {
    let type: String
    let message: String?
    let text: String?
    let delta: String?
    let path: String?
    let level: Float?
    let recording: Bool?
    let auto: Bool?
}

final class EngineClient {
    var onStatus: ((String) -> Void)?
    var onError: ((String) -> Void)?
    var onRecordingChanged: ((Bool) -> Void)?
    var onTranscript: ((_ text: String, _ delta: String) -> Void)?
    var onFinal: ((_ text: String, _ auto: Bool) -> Void)?
    var onLevel: ((Float) -> Void)?
    var onModelDir: ((String) -> Void)?

    private var process: Process?
    private var inputHandle: FileHandle?
    private var outputBuffer = Data()
    private let decoder = JSONDecoder()
    private let writeQueue = DispatchQueue(label: "com.snipemanmike.NemotronBubbleMac.engine.write")

    private(set) var isRecording = false

    func start() {
        guard process == nil else { return }

        guard let engineURL = findEngineURL() else {
            onError?("nemotron-engine was not found in the app bundle.")
            return
        }

        let process = Process()
        process.executableURL = engineURL
        process.arguments = ["--stdin-audio"]
        process.currentDirectoryURL = repositoryRootHint(from: engineURL)
        process.environment = ProcessInfo.processInfo.environment

        let stdin = Pipe()
        let stdout = Pipe()
        let stderr = Pipe()
        process.standardInput = stdin
        process.standardOutput = stdout
        process.standardError = stderr

        stdout.fileHandleForReading.readabilityHandler = { [weak self] handle in
            self?.readOutput(handle.availableData)
        }

        stderr.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty, let text = String(data: data, encoding: .utf8) else { return }
            DispatchQueue.main.async {
                self?.onError?(text.trimmingCharacters(in: .whitespacesAndNewlines))
            }
        }

        process.terminationHandler = { [weak self] _ in
            DispatchQueue.main.async {
                self?.isRecording = false
                self?.onRecordingChanged?(false)
                self?.onStatus?("Nemotron engine stopped.")
                self?.process = nil
                self?.inputHandle = nil
            }
        }

        do {
            try process.run()
            self.process = process
            inputHandle = stdin.fileHandleForWriting
        } catch {
            onError?("Could not start nemotron-engine: \(error.localizedDescription)")
        }
    }

    func preload() {
        send(["type": "preload"])
    }

    func startRecording() {
        start()
        send(["type": "start"])
    }

    func stopRecording(fast: Bool = false) {
        send(["type": "stop", "fast": fast])
    }

    func sendAudio(samples: [Float], sampleRate: Double) {
        guard !samples.isEmpty else { return }

        let data = samples.withUnsafeBufferPointer { buffer -> Data in
            guard let baseAddress = buffer.baseAddress else { return Data() }
            return Data(bytes: baseAddress, count: buffer.count * MemoryLayout<Float>.size)
        }

        send([
            "type": "audio",
            "sample_rate": sampleRate,
            "data": data.base64EncodedString()
        ])
    }

    func shutdown() {
        send(["type": "shutdown"])
        process?.terminate()
        process = nil
        inputHandle = nil
    }

    private func send(_ payload: [String: Any]) {
        writeQueue.async { [weak self] in
            guard let self, let inputHandle = self.inputHandle else { return }
            do {
                let data = try JSONSerialization.data(withJSONObject: payload)
                inputHandle.write(data)
                inputHandle.write(Data([0x0A]))
            } catch {
                DispatchQueue.main.async {
                    self.onError?("Engine command failed: \(error.localizedDescription)")
                }
            }
        }
    }

    private func readOutput(_ data: Data) {
        guard !data.isEmpty else { return }
        outputBuffer.append(data)

        while let newline = outputBuffer.firstIndex(of: 0x0A) {
            let line = outputBuffer[..<newline]
            outputBuffer.removeSubrange(...newline)
            guard !line.isEmpty else { continue }
            handleLine(Data(line))
        }
    }

    private func handleLine(_ data: Data) {
        do {
            let message = try decoder.decode(EngineMessage.self, from: data)
            DispatchQueue.main.async {
                self.handle(message)
            }
        } catch {
            let text = String(data: data, encoding: .utf8) ?? "<invalid utf8>"
            DispatchQueue.main.async {
                self.onError?("Bad engine event: \(text)")
            }
        }
    }

    private func handle(_ message: EngineMessage) {
        switch message.type {
        case "status":
            if let text = message.message {
                onStatus?(text)
            }
        case "error":
            if let text = message.message {
                onError?(text)
            }
        case "recording":
            if let recording = message.recording {
                isRecording = recording
                onRecordingChanged?(recording)
            }
        case "transcript":
            onTranscript?(message.text ?? "", message.delta ?? "")
        case "final":
            isRecording = false
            onRecordingChanged?(false)
            onFinal?(message.text ?? "", message.auto ?? false)
        case "level":
            if let level = message.level {
                onLevel?(level)
            }
        case "model_dir":
            if let path = message.path {
                onModelDir?(path)
            }
        default:
            break
        }
    }

    private func findEngineURL() -> URL? {
        let executableDir = Bundle.main.executableURL?.deletingLastPathComponent()
        let bundled = executableDir?.appendingPathComponent("nemotron-engine")
        if let bundled, FileManager.default.isExecutableFile(atPath: bundled.path) {
            return bundled
        }

        let dev = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent("target/release/nemotron-engine")
        if FileManager.default.isExecutableFile(atPath: dev.path) {
            return dev
        }

        return nil
    }

    private func repositoryRootHint(from engineURL: URL) -> URL? {
        var url = engineURL.deletingLastPathComponent()
        for _ in 0..<8 {
            let candidate = url.appendingPathComponent("Cargo.toml")
            if FileManager.default.fileExists(atPath: candidate.path) {
                return url
            }
            url.deleteLastPathComponent()
        }
        return nil
    }
}
