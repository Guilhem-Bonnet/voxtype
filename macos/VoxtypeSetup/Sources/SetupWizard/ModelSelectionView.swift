import SwiftUI

struct ModelSelectionView: View {
    @EnvironmentObject var setupState: SetupState
    @StateObject private var cli = VoxtypeCLI.shared

    @State private var selectedEngine: TranscriptionEngine = .parakeet
    @State private var selectedModel: String = "parakeet-tdt-0.6b-v3-int8"
    @State private var isDownloading = false
    @State private var downloadComplete = false
    @State private var errorMessage: String?

    var filteredModels: [ModelInfo] {
        cli.availableModels().filter { $0.engine == selectedEngine }
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
                Text("Parakeet (Recommended for Mac)").tag(TranscriptionEngine.parakeet)
                Text("Whisper (Multilingual)").tag(TranscriptionEngine.whisper)
            }
            .pickerStyle(.segmented)
            .padding(.horizontal, 60)
            .onChange(of: selectedEngine) { newValue in
                // Select first model for the engine
                selectedModel = filteredModels.first?.name ?? ""
            }

            // Engine description
            Text(engineDescription)
                .font(.callout)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 60)

            // Model list
            VStack(spacing: 8) {
                ForEach(filteredModels) { model in
                    ModelRow(
                        model: model,
                        isSelected: selectedModel == model.name,
                        action: { selectedModel = model.name }
                    )
                }
            }
            .padding(.horizontal, 40)

            // Download progress
            if isDownloading {
                VStack(spacing: 8) {
                    ProgressView(value: cli.downloadProgress)
                        .progressViewStyle(.linear)
                    Text("Downloading \(selectedModel)... \(Int(cli.downloadProgress * 100))%")
                        .font(.callout)
                        .foregroundColor(.secondary)
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

                if downloadComplete || cli.hasModel() {
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
    }

    var engineDescription: String {
        switch selectedEngine {
        case .parakeet:
            return "Parakeet uses NVIDIA's FastConformer model, optimized for Apple Silicon. English only, but very fast."
        case .whisper:
            return "Whisper is OpenAI's speech recognition model. Supports many languages with excellent accuracy."
        }
    }

    func downloadModel() {
        isDownloading = true
        errorMessage = nil

        Task {
            do {
                try await cli.downloadModel(selectedModel, engine: selectedEngine)
                await MainActor.run {
                    isDownloading = false
                    downloadComplete = true
                    // Also set the model in config
                    _ = cli.setEngine(selectedEngine)
                    _ = cli.setModel(selectedModel, engine: selectedEngine)
                }
            } catch {
                await MainActor.run {
                    isDownloading = false
                    errorMessage = "Download failed. Please check your internet connection."
                }
            }
        }
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
