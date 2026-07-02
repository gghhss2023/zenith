import Foundation

final class ClaudeBridge {
    enum BridgeError: LocalizedError {
        case cliNotFound
        case timeout
        case failed(String)

        var errorDescription: String? {
            switch self {
            case .cliNotFound:
                return "claude CLI not found (checked /usr/local/bin, /opt/homebrew/bin, ~/.local/bin)"
            case .timeout:
                return "Request timed out (60s)"
            case .failed(let msg):
                return msg
            }
        }
    }

    // GUI apps don't inherit the shell PATH, so probe known install locations.
    static func findCLI() -> String? {
        let candidates = [
            "/usr/local/bin/claude",
            "/opt/homebrew/bin/claude",
            NSHomeDirectory() + "/.local/bin/claude",
        ]
        return candidates.first { FileManager.default.isExecutableFile(atPath: $0) }
    }

    private let queue = DispatchQueue(label: "zenith.claude-bridge")
    private var process: Process?
    private var timeoutItem: DispatchWorkItem?

    func query(
        prompt: String,
        model: String,
        onDelta: @escaping (String) -> Void,
        onDone: @escaping (Result<String, BridgeError>) -> Void
    ) {
        cancel()
        guard let cli = Self.findCLI() else {
            onDone(.failure(.cliNotFound))
            return
        }

        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: cli)
        // Flags verified against claude CLI v2.1.139:
        // - isolation: no user settings/hooks/plugins, no tools, no MCP servers
        // - streaming: stream-json + include-partial-messages yields text_delta events
        proc.arguments = [
            "-p", prompt,
            "--model", model,
            "--setting-sources", "",
            "--tools", "",
            "--strict-mcp-config",
            "--output-format", "stream-json",
            "--verbose",
            "--include-partial-messages",
        ]
        let stdout = Pipe()
        proc.standardOutput = stdout
        proc.standardError = Pipe()

        var buffer = Data()
        var fullText = ""
        var resultError: String?
        var didTimeout = false

        stdout.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let chunk = handle.availableData
            guard let self = self, !chunk.isEmpty else { return }
            self.queue.async {
                buffer.append(chunk)
                while let nlRange = buffer.range(of: Data([0x0A])) {
                    let lineData = buffer.subdata(in: buffer.startIndex..<nlRange.lowerBound)
                    buffer.removeSubrange(buffer.startIndex..<nlRange.upperBound)
                    guard let obj = try? JSONSerialization.jsonObject(with: lineData) as? [String: Any],
                          let type = obj["type"] as? String else { continue }
                    if type == "stream_event",
                       let event = obj["event"] as? [String: Any],
                       event["type"] as? String == "content_block_delta",
                       let delta = event["delta"] as? [String: Any],
                       delta["type"] as? String == "text_delta",
                       let text = delta["text"] as? String {
                        fullText += text
                        DispatchQueue.main.async { onDelta(text) }
                    } else if type == "result" {
                        if obj["is_error"] as? Bool == true {
                            resultError = obj["result"] as? String ?? "unknown error"
                        }
                    }
                }
            }
        }

        proc.terminationHandler = { [weak self] _ in
            guard let self = self else { return }
            self.queue.async {
                stdout.fileHandleForReading.readabilityHandler = nil
                self.timeoutItem?.cancel()
                let result: Result<String, BridgeError>
                if didTimeout {
                    result = .failure(.timeout)
                } else if let err = resultError {
                    result = .failure(.failed(err))
                } else if fullText.isEmpty {
                    result = .failure(.failed("empty response"))
                } else {
                    result = .success(fullText)
                }
                DispatchQueue.main.async { onDone(result) }
            }
        }

        let timeout = DispatchWorkItem { [weak proc] in
            didTimeout = true
            proc?.terminate()
        }
        timeoutItem = timeout
        queue.asyncAfter(deadline: .now() + 60, execute: timeout)

        do {
            try proc.run()
            process = proc
        } catch {
            onDone(.failure(.failed("failed to launch claude: \(error.localizedDescription)")))
        }
    }

    func cancel() {
        timeoutItem?.cancel()
        timeoutItem = nil
        if let proc = process, proc.isRunning {
            proc.terminationHandler = nil
            proc.terminate()
        }
        process = nil
    }
}
