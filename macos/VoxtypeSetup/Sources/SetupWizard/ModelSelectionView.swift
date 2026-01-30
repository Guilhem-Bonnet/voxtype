import SwiftUI

struct ModelSelectionView: View {
    @EnvironmentObject var setupState: SetupState
    @StateObject private var downloadMonitor = DownloadMonitor()

    @State private var selectedEngine: TranscriptionEngine = .parakeet
    @State private var selectedModel: String = "parakeet-tdt-0.6b-v3-int8"
    @State private var isDownloading = false
    @State private var downloadComplete = false
    @State private var errorMessage: String?

    // Static model lists to avoid repeated allocations
    private let parakeetModels: [ModelInfo] = [
        ModelInfo(name: "parakeet-tdt-0.6b-v3-int8", engine: .parakeet,
                 description: "Fast, optimized for Apple Silicon", size: "670 MB"),
        ModelInfo(name: "parakeet-tdt-0.6b-v3", engine: .parakeet,
                 description: "Full precision", size: "1.2 GB"),
    ]

    private let whisperModels: [ModelInfo] = [
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

    private var displayedModels: [ModelInfo] {
        selectedEngine == .parakeet ? parakeetModels : whisperModels
    }

    var body: some View {
        VStack(spacing: 20) {
            Spacer()

            Text("Choose Speech Model")
                .font(.title)
                .fontWeight(.bold)

            Text("Select the speech recognition engine and model to use.")
                .foregroundColor(.secondary)

            // Engine picker
            Picker("Engine", selection: $selectedEngine) {
                Text("Parakeet (Recommended)").tag(TranscriptionEngine.parakeet)
                Text("Whisper (Multilingual)").tag(TranscriptionEngine.whisper)
            }
            .pickerStyle(.segmented)
            .padding(.horizontal, 60)

            // Engine description
            Text(selectedEngine == .parakeet
                ? "Parakeet uses NVIDIA's FastConformer model, optimized for Apple Silicon. English only, but very fast."
                : "Whisper is OpenAI's speech recognition model. Supports many languages with excellent accuracy.")
                .font(.callout)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 60)

            // Model list (scrollable for longer lists)
            ScrollView {
                VStack(spacing: 8) {
                    if selectedEngine == .parakeet {
                        ForEach(parakeetModels) { model in
                            ModelRow(
                                model: model,
                                isSelected: selectedModel == model.name,
                                action: { selectedModel = model.name }
                            )
                        }
                    } else {
                        ForEach(whisperModels) { model in
                            ModelRow(
                                model: model,
                                isSelected: selectedModel == model.name,
                                action: { selectedModel = model.name }
                            )
                        }
                    }
                }
                .padding(.horizontal, 40)
            }
            .frame(maxHeight: 220)

            // Download progress
            if isDownloading {
                VStack(spacing: 8) {
                    ProgressView(value: downloadMonitor.progress)
                        .progressViewStyle(.linear)
                    Text("Downloading \(selectedModel)... \(Int(downloadMonitor.progress * 100))%")
                        .font(.callout)
                        .foregroundColor(.secondary)
                    if downloadMonitor.downloadedSize > 0 {
                        Text("\(downloadMonitor.formattedDownloaded) / \(downloadMonitor.formattedTotal)")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                }
                .padding(.horizontal, 60)
            }

            if let error = errorMessage {
                Text(error)
                    .foregroundColor(.red)
                    .font(.callout)
            }

            if downloadComplete {
                HStack {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundColor(.green)
                    Text("Model downloaded successfully!")
                        .foregroundColor(.green)
                }
            }

            Spacer()

            // Navigation
            HStack {
                Button("Back") {
                    withAnimation {
                        setupState.currentStep = .permissions
                    }
                }
                .buttonStyle(WizardButtonStyle())
                .disabled(isDownloading)

                Spacer()

                if downloadComplete {
                    Button("Continue") {
                        withAnimation {
                            setupState.currentStep = .launchAgent
                        }
                    }
                    .buttonStyle(WizardButtonStyle(isPrimary: true))
                } else {
                    Button("Download & Continue") {
                        downloadModel()
                    }
                    .buttonStyle(WizardButtonStyle(isPrimary: true))
                    .disabled(isDownloading || selectedModel.isEmpty)
                }
            }
            .padding(.horizontal, 40)
            .padding(.bottom, 30)
        }
        .onChange(of: selectedEngine) { _ in
            // Select first model when engine changes
            if selectedEngine == .parakeet {
                selectedModel = parakeetModels.first?.name ?? ""
            } else {
                selectedModel = whisperModels.first?.name ?? ""
            }
        }
    }

    func downloadModel() {
        isDownloading = true
        errorMessage = nil

        // Get expected size and start monitoring
        let expectedSize = getExpectedModelSize(selectedModel)
        let modelsDir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/voxtype/models")
        downloadMonitor.startMonitoring(directory: modelsDir, expectedSize: expectedSize, modelName: selectedModel)

        Task {
            do {
                let cli = VoxtypeCLI.shared
                try await cli.downloadModel(selectedModel, engine: selectedEngine)
                await MainActor.run {
                    downloadMonitor.stopMonitoring()
                    isDownloading = false
                    downloadComplete = true
                    _ = cli.setEngine(selectedEngine)
                    _ = cli.setModel(selectedModel, engine: selectedEngine)
                }
            } catch {
                await MainActor.run {
                    downloadMonitor.stopMonitoring()
                    isDownloading = false
                    errorMessage = "Download failed. Please check your internet connection."
                }
            }
        }
    }

    func getExpectedModelSize(_ model: String) -> Int64 {
        // Expected sizes in bytes
        switch model {
        case "parakeet-tdt-0.6b-v3-int8": return 670_000_000
        case "parakeet-tdt-0.6b-v3": return 2_400_000_000
        case "large-v3-turbo": return 1_600_000_000
        case "medium.en": return 1_500_000_000
        case "small.en": return 500_000_000
        case "base.en": return 145_000_000
        case "tiny.en": return 75_000_000
        default: return 1_000_000_000
        }
    }
}

/// Monitors download progress by watching file/directory size
class DownloadMonitor: ObservableObject {
    @Published var progress: Double = 0.0
    @Published var downloadedSize: Int64 = 0
    @Published var expectedSize: Int64 = 0

    private var timer: Timer?
    private var modelsDirectory: URL?
    private var modelName: String = ""
    private var initialSize: Int64 = 0

    var formattedDownloaded: String {
        ByteCountFormatter.string(fromByteCount: downloadedSize, countStyle: .file)
    }

    var formattedTotal: String {
        ByteCountFormatter.string(fromByteCount: expectedSize, countStyle: .file)
    }

    func startMonitoring(directory: URL, expectedSize: Int64, modelName: String) {
        self.modelsDirectory = directory
        self.expectedSize = expectedSize
        self.modelName = modelName
        self.downloadedSize = 0
        self.progress = 0.0
        self.initialSize = getModelSize()

        // Poll every 0.3 seconds
        timer = Timer.scheduledTimer(withTimeInterval: 0.3, repeats: true) { [weak self] _ in
            self?.updateProgress()
        }
    }

    func stopMonitoring() {
        timer?.invalidate()
        timer = nil
        progress = 1.0
    }

    private func updateProgress() {
        let currentSize = getModelSize()
        let downloaded = currentSize - initialSize

        DispatchQueue.main.async {
            self.downloadedSize = downloaded
            if self.expectedSize > 0 {
                self.progress = min(Double(downloaded) / Double(self.expectedSize), 0.99)
            }
        }
    }

    private func getModelSize() -> Int64 {
        guard let modelsDir = modelsDirectory else { return 0 }
        let fm = FileManager.default

        // For Parakeet models (directory)
        let parakeetDir = modelsDir.appendingPathComponent(modelName)
        if fm.fileExists(atPath: parakeetDir.path) {
            return getDirectorySize(parakeetDir)
        }

        // For Whisper models (single .bin file)
        // Map model name to file name
        let whisperFile: String
        switch modelName {
        case "large-v3-turbo": whisperFile = "ggml-large-v3-turbo.bin"
        case "medium.en": whisperFile = "ggml-medium.en.bin"
        case "small.en": whisperFile = "ggml-small.en.bin"
        case "base.en": whisperFile = "ggml-base.en.bin"
        case "tiny.en": whisperFile = "ggml-tiny.en.bin"
        default: whisperFile = "ggml-\(modelName).bin"
        }

        let whisperPath = modelsDir.appendingPathComponent(whisperFile)
        if let attrs = try? fm.attributesOfItem(atPath: whisperPath.path),
           let size = attrs[.size] as? Int64 {
            return size
        }

        return 0
    }

    private func getDirectorySize(_ url: URL) -> Int64 {
        let fm = FileManager.default
        guard let enumerator = fm.enumerator(at: url, includingPropertiesForKeys: [.fileSizeKey], options: [.skipsHiddenFiles]) else {
            return 0
        }

        var total: Int64 = 0
        for case let fileURL as URL in enumerator {
            if let size = try? fileURL.resourceValues(forKeys: [.fileSizeKey]).fileSize {
                total += Int64(size)
            }
        }
        return total
    }
}

struct ModelRow: View {
    let model: ModelInfo
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(model.name)
                        .fontWeight(.medium)
                    Text(model.description)
                        .font(.callout)
                        .foregroundColor(.secondary)
                }

                Spacer()

                Text(model.size)
                    .font(.callout)
                    .foregroundColor(.secondary)

                Image(systemName: isSelected ? "checkmark.circle.fill" : "circle")
                    .foregroundColor(isSelected ? .accentColor : .secondary)
            }
            .padding()
            .background(isSelected ? Color.accentColor.opacity(0.1) : Color(NSColor.controlBackgroundColor))
            .cornerRadius(8)
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(isSelected ? Color.accentColor : Color.clear, lineWidth: 2)
            )
        }
        .buttonStyle(.plain)
    }
}

#Preview {
    ModelSelectionView()
        .environmentObject(SetupState())
        .frame(width: 600, height: 500)
}
