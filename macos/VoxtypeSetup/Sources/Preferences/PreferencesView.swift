import SwiftUI

struct PreferencesView: View {
    @EnvironmentObject var setupState: SetupState
    @StateObject private var permissions = PermissionChecker.shared

    @State private var selectedEngine: TranscriptionEngine = .parakeet
    @State private var selectedModel: String = ""
    @State private var autoStartEnabled: Bool = false
    @State private var showingModelDownload = false
    @State private var daemonStatus: String = ""

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Image(systemName: "mic.circle.fill")
                    .font(.title)
                    .foregroundColor(.accentColor)
                Text("Voxtype Preferences")
                    .font(.title2)
                    .fontWeight(.semibold)
                Spacer()

                // Status indicator
                HStack(spacing: 6) {
                    Circle()
                        .fill(daemonRunning ? Color.green : Color.red)
                        .frame(width: 8, height: 8)
                    Text(daemonRunning ? "Running" : "Stopped")
                        .font(.callout)
                        .foregroundColor(.secondary)
                }
            }
            .padding()
            .background(Color(NSColor.controlBackgroundColor))

            Divider()

            ScrollView {
                VStack(spacing: 24) {
                    // Engine & Model
                    PreferenceSection(title: "Speech Recognition", icon: "waveform") {
                        VStack(alignment: .leading, spacing: 12) {
                            Text("Engine")
                                .font(.callout)
                                .foregroundColor(.secondary)

                            Picker("Engine", selection: $selectedEngine) {
                                Text("Parakeet").tag(TranscriptionEngine.parakeet)
                                Text("Whisper").tag(TranscriptionEngine.whisper)
                            }
                            .pickerStyle(.segmented)
                            .onChange(of: selectedEngine) { _ in
                                _ = VoxtypeCLI.shared.setEngine(selectedEngine)
                            }

                            Text("Model")
                                .font(.callout)
                                .foregroundColor(.secondary)
                                .padding(.top, 8)

                            HStack {
                                Text(selectedModel.isEmpty ? "No model selected" : selectedModel)
                                    .foregroundColor(selectedModel.isEmpty ? .secondary : .primary)
                                Spacer()
                                Button("Change...") {
                                    showingModelDownload = true
                                }
                            }
                        }
                    }

                    // Permissions
                    PreferenceSection(title: "Permissions", icon: "lock.shield") {
                        VStack(spacing: 12) {
                            PermissionStatusRow(
                                title: "Microphone",
                                isGranted: permissions.hasMicrophoneAccess,
                                action: { permissions.openMicrophoneSettings() }
                            )
                            PermissionStatusRow(
                                title: "Accessibility",
                                isGranted: permissions.hasAccessibilityAccess,
                                action: { permissions.openAccessibilitySettings() }
                            )
                            PermissionStatusRow(
                                title: "Input Monitoring",
                                isGranted: permissions.hasInputMonitoringAccess,
                                action: { permissions.openInputMonitoringSettings() }
                            )
                        }
                    }

                    // Auto-start
                    PreferenceSection(title: "Startup", icon: "arrow.clockwise") {
                        Toggle(isOn: $autoStartEnabled) {
                            Text("Start Voxtype at login")
                        }
                        .onChange(of: autoStartEnabled) { newValue in
                            if newValue {
                                _ = VoxtypeCLI.shared.installLaunchAgent()
                            } else {
                                _ = VoxtypeCLI.shared.uninstallLaunchAgent()
                            }
                        }
                    }

                    // Daemon control
                    PreferenceSection(title: "Daemon", icon: "gearshape.2") {
                        HStack {
                            Button("Restart Daemon") {
                                VoxtypeCLI.shared.restartDaemon()
                            }

                            Button("Stop Daemon") {
                                VoxtypeCLI.shared.stopDaemon()
                            }

                            Spacer()

                            Button("View Logs") {
                                let logPath = FileManager.default.homeDirectoryForCurrentUser
                                    .appendingPathComponent("Library/Logs/voxtype")
                                NSWorkspace.shared.open(logPath)
                            }
                        }
                    }

                    // Config file
                    PreferenceSection(title: "Advanced", icon: "doc.text") {
                        HStack {
                            Text("For advanced settings, edit the config file directly")
                                .font(.callout)
                                .foregroundColor(.secondary)
                            Spacer()
                            Button("Open Config") {
                                let configPath = FileManager.default.homeDirectoryForCurrentUser
                                    .appendingPathComponent("Library/Application Support/voxtype/config.toml")
                                NSWorkspace.shared.open(configPath)
                            }
                        }
                    }
                }
                .padding()
            }

            Divider()

            // Footer
            HStack {
                Button("Run Setup Again") {
                    setupState.resetWizard()
                }
                .buttonStyle(.borderless)

                Spacer()

                Button("Quit") {
                    NSApplication.shared.terminate(nil)
                }
            }
            .padding()
        }
        .frame(width: 500, height: 600)
        .onAppear {
            loadCurrentSettings()
        }
        .sheet(isPresented: $showingModelDownload) {
            ModelDownloadSheet(selectedEngine: selectedEngine) { model in
                selectedModel = model
            }
        }
    }

    var daemonRunning: Bool {
        !daemonStatus.isEmpty && daemonStatus != "stopped"
    }

    func loadCurrentSettings() {
        let cli = VoxtypeCLI.shared
        autoStartEnabled = cli.hasLaunchAgent()
        daemonStatus = cli.getStatus()
        permissions.refresh()

        // Load current engine and model from config
        if let config = cli.getConfig() {
            if let engine = config["engine"] as? String {
                selectedEngine = engine == "parakeet" ? .parakeet : .whisper
            }
            // TODO: Extract current model from config
        }
    }
}

struct PreferenceSection<Content: View>: View {
    let title: String
    let icon: String
    let content: () -> Content

    init(title: String, icon: String, @ViewBuilder content: @escaping () -> Content) {
        self.title = title
        self.icon = icon
        self.content = content
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Image(systemName: icon)
                    .foregroundColor(.accentColor)
                Text(title)
                    .fontWeight(.semibold)
            }

            content()
                .padding()
                .background(Color(NSColor.controlBackgroundColor))
                .cornerRadius(8)
        }
    }
}

struct PermissionStatusRow: View {
    let title: String
    let isGranted: Bool
    let action: () -> Void

    var body: some View {
        HStack {
            Text(title)
            Spacer()
            if isGranted {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundColor(.green)
            } else {
                Button("Grant") {
                    action()
                }
                .controlSize(.small)
            }
        }
    }
}

struct ModelDownloadSheet: View {
    let selectedEngine: TranscriptionEngine
    let onSelect: (String) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var selectedModel: String = ""
    @State private var isDownloading = false

    // Static model lists
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

    private var models: [ModelInfo] {
        selectedEngine == .parakeet ? parakeetModels : whisperModels
    }

    var body: some View {
        VStack(spacing: 20) {
            Text("Select Model")
                .font(.title2)
                .fontWeight(.semibold)

            List(selection: $selectedModel) {
                ForEach(models) { model in
                    HStack {
                        VStack(alignment: .leading) {
                            Text(model.name)
                                .fontWeight(.medium)
                            Text(model.description)
                                .font(.callout)
                                .foregroundColor(.secondary)
                        }
                        Spacer()
                        Text(model.size)
                            .foregroundColor(.secondary)
                    }
                    .tag(model.name)
                }
            }
            .frame(height: 200)

            if isDownloading {
                ProgressView()
                    .progressViewStyle(.linear)
            }

            HStack {
                Button("Cancel") {
                    dismiss()
                }

                Spacer()

                Button("Download & Use") {
                    downloadAndUse()
                }
                .disabled(selectedModel.isEmpty || isDownloading)
            }
        }
        .padding()
        .frame(width: 400)
    }

    func downloadAndUse() {
        isDownloading = true
        Task {
            do {
                let cli = VoxtypeCLI.shared
                try await cli.downloadModel(selectedModel, engine: selectedEngine)
                _ = cli.setModel(selectedModel, engine: selectedEngine)
                await MainActor.run {
                    onSelect(selectedModel)
                    dismiss()
                }
            } catch {
                isDownloading = false
            }
        }
    }
}

#Preview {
    PreferencesView()
        .environmentObject(SetupState())
}
