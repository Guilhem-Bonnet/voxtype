import Foundation

/// Bridge to call the voxtype CLI binary
class VoxtypeCLI: ObservableObject {
    static let shared = VoxtypeCLI()

    /// Path to the voxtype binary
    private let binaryPath: String

    /// Current download progress (0.0 to 1.0)
    @Published var downloadProgress: Double = 0.0
    @Published var isDownloading: Bool = false
    @Published var downloadError: String? = nil

    private init() {
        // Look for voxtype in standard locations
        let candidates = [
            "/Applications/Voxtype.app/Contents/MacOS/voxtype",
            "/usr/local/bin/voxtype",
            "/opt/homebrew/bin/voxtype",
            Bundle.main.bundlePath + "/Contents/MacOS/voxtype"
        ]

        binaryPath = candidates.first { FileManager.default.fileExists(atPath: $0) }
            ?? "/Applications/Voxtype.app/Contents/MacOS/voxtype"
    }

    // MARK: - Status Checks

    /// Check if a speech model is downloaded
    func hasModel() -> Bool {
        let output = run(["status", "--json"])
        // If status works without error about missing model, we have one
        return !output.contains("model not found") && !output.contains("No model")
    }

    /// Check if LaunchAgent is installed
    func hasLaunchAgent() -> Bool {
        let plistPath = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/LaunchAgents/io.voxtype.daemon.plist")
        return FileManager.default.fileExists(atPath: plistPath.path)
    }

    /// Get current daemon status
    func getStatus() -> String {
        return run(["status"]).trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// Get current configuration
    func getConfig() -> [String: Any]? {
        let output = run(["config", "--json"])
        guard let data = output.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return nil }
        return json
    }

    // MARK: - Model Management

    /// Get list of available models
    func availableModels() -> [ModelInfo] {
        // Return hardcoded list for now - could parse from CLI later
        return [
            ModelInfo(name: "parakeet-tdt-0.6b-v3-int8", engine: .parakeet,
                     description: "Fast, optimized for Apple Silicon", size: "670 MB"),
            ModelInfo(name: "parakeet-tdt-0.6b-v3", engine: .parakeet,
                     description: "Full precision", size: "1.2 GB"),
            ModelInfo(name: "large-v3-turbo", engine: .whisper,
                     description: "Best accuracy, multilingual", size: "1.6 GB"),
            ModelInfo(name: "medium.en", engine: .whisper,
                     description: "Good accuracy, English only", size: "1.5 GB"),
            ModelInfo(name: "small.en", engine: .whisper,
                     description: "Balanced speed/accuracy", size: "500 MB"),
            ModelInfo(name: "base.en", engine: .whisper,
                     description: "Fast, English only", size: "145 MB"),
            ModelInfo(name: "tiny.en", engine: .whisper,
                     description: "Fastest, lower accuracy", size: "75 MB"),
        ]
    }

    /// Download a model (async with progress)
    func downloadModel(_ model: String, engine: TranscriptionEngine) async throws {
        await MainActor.run {
            isDownloading = true
            downloadProgress = 0.0
            downloadError = nil
        }

        defer {
            Task { @MainActor in
                isDownloading = false
            }
        }

        let args: [String]
        switch engine {
        case .parakeet:
            args = ["setup", "parakeet", "--download", model]
        case .whisper:
            args = ["setup", "model", "--download", model]
        }

        // Run with progress monitoring
        let process = Process()
        process.executableURL = URL(fileURLWithPath: binaryPath)
        process.arguments = args

        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = pipe

        try process.run()

        // Monitor output for progress updates
        let handle = pipe.fileHandleForReading
        for try await line in handle.bytes.lines {
            if let progress = parseProgress(line) {
                await MainActor.run {
                    self.downloadProgress = progress
                }
            }
        }

        process.waitUntilExit()

        if process.terminationStatus != 0 {
            await MainActor.run {
                self.downloadError = "Download failed"
            }
            throw CLIError.downloadFailed
        }

        await MainActor.run {
            downloadProgress = 1.0
        }
    }

    private func parseProgress(_ line: String) -> Double? {
        // Parse progress from CLI output like "Downloading... 45%"
        if let range = line.range(of: #"(\d+)%"#, options: .regularExpression),
           let percent = Double(line[range].dropLast()) {
            return percent / 100.0
        }
        return nil
    }

    // MARK: - Configuration

    /// Set the transcription engine
    func setEngine(_ engine: TranscriptionEngine) -> Bool {
        let engineStr = engine == .parakeet ? "parakeet" : "whisper"
        let output = run(["config", "set", "engine", engineStr])
        return !output.contains("error")
    }

    /// Set the model
    func setModel(_ model: String, engine: TranscriptionEngine) -> Bool {
        let args: [String]
        switch engine {
        case .parakeet:
            args = ["setup", "parakeet", "--set", model]
        case .whisper:
            args = ["setup", "model", "--set", model]
        }
        let output = run(args)
        return !output.contains("error")
    }

    // MARK: - LaunchAgent

    /// Install the LaunchAgent for auto-start
    func installLaunchAgent() -> Bool {
        let output = run(["setup", "launchd"])
        return !output.contains("error") && !output.contains("failed")
    }

    /// Uninstall the LaunchAgent
    func uninstallLaunchAgent() -> Bool {
        let output = run(["setup", "launchd", "--uninstall"])
        return !output.contains("error")
    }

    /// Start the daemon
    func startDaemon() {
        _ = run(["daemon"])
    }

    /// Stop the daemon
    func stopDaemon() {
        let _ = shell("pkill -f 'voxtype daemon'")
    }

    /// Restart the daemon
    func restartDaemon() {
        stopDaemon()
        Thread.sleep(forTimeInterval: 0.5)
        startDaemon()
    }

    // MARK: - Helpers

    /// Run a voxtype command and return output
    private func run(_ arguments: [String]) -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: binaryPath)
        process.arguments = arguments

        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = pipe

        do {
            try process.run()
            process.waitUntilExit()

            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            return String(data: data, encoding: .utf8) ?? ""
        } catch {
            return "error: \(error.localizedDescription)"
        }
    }

    /// Run a shell command
    private func shell(_ command: String) -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/zsh")
        process.arguments = ["-c", command]

        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = pipe

        do {
            try process.run()
            process.waitUntilExit()

            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            return String(data: data, encoding: .utf8) ?? ""
        } catch {
            return ""
        }
    }
}

// MARK: - Supporting Types

enum TranscriptionEngine: String, CaseIterable {
    case parakeet = "parakeet"
    case whisper = "whisper"

    var displayName: String {
        switch self {
        case .parakeet: return "Parakeet"
        case .whisper: return "Whisper"
        }
    }
}

struct ModelInfo: Identifiable {
    let id = UUID()
    let name: String
    let engine: TranscriptionEngine
    let description: String
    let size: String
}

enum CLIError: Error {
    case downloadFailed
    case configFailed
}
