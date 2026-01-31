import SwiftUI

struct WhisperSettingsView: View {
    @State private var mode: String = "local"
    @State private var language: String = "en"
    @State private var translate: Bool = false
    @State private var gpuIsolation: Bool = false
    @State private var onDemandLoading: Bool = false
    @State private var initialPrompt: String = ""

    private let languages = [
        ("English", "en"),
        ("Auto-detect", "auto"),
        ("Spanish", "es"),
        ("French", "fr"),
        ("German", "de"),
        ("Italian", "it"),
        ("Portuguese", "pt"),
        ("Dutch", "nl"),
        ("Polish", "pl"),
        ("Russian", "ru"),
        ("Japanese", "ja"),
        ("Chinese", "zh"),
        ("Korean", "ko"),
    ]

    var body: some View {
        Form {
            Section {
                Picker("Mode", selection: $mode) {
                    Text("Local (whisper.cpp)").tag("local")
                    Text("Remote Server").tag("remote")
                }
                .onChange(of: mode) { newValue in
                    ConfigManager.shared.updateConfig(key: "mode", value: "\"\(newValue)\"", section: "[whisper]")
                }

                Text(mode == "local"
                    ? "Run transcription locally using whisper.cpp."
                    : "Send audio to a remote Whisper server or OpenAI API.")
                    .font(.caption)
                    .foregroundColor(.secondary)
            } header: {
                Text("Whisper Mode")
            }

            Section {
                Picker("Language", selection: $language) {
                    ForEach(languages, id: \.1) { name, code in
                        Text(name).tag(code)
                    }
                }
                .onChange(of: language) { newValue in
                    ConfigManager.shared.updateConfig(key: "language", value: "\"\(newValue)\"", section: "[whisper]")
                }

                Toggle("Translate to English", isOn: $translate)
                    .onChange(of: translate) { newValue in
                        ConfigManager.shared.updateConfig(key: "translate", value: newValue ? "true" : "false", section: "[whisper]")
                    }

                Text("When enabled, non-English speech is automatically translated to English.")
                    .font(.caption)
                    .foregroundColor(.secondary)
            } header: {
                Text("Language")
            }

            Section {
                TextField("Initial Prompt", text: $initialPrompt, axis: .vertical)
                    .lineLimit(3...6)
                    .onSubmit {
                        saveInitialPrompt()
                    }

                Text("Hint at terminology, proper nouns, or formatting. Example: \"Technical discussion about Rust and Kubernetes.\"")
                    .font(.caption)
                    .foregroundColor(.secondary)

                Button("Save Prompt") {
                    saveInitialPrompt()
                }
            } header: {
                Text("Initial Prompt")
            }

            Section {
                Toggle("GPU Isolation", isOn: $gpuIsolation)
                    .onChange(of: gpuIsolation) { newValue in
                        ConfigManager.shared.updateConfig(key: "gpu_isolation", value: newValue ? "true" : "false", section: "[whisper]")
                    }

                Text("Run transcription in a subprocess that exits after each use, releasing GPU memory. Useful for laptops with hybrid graphics.")
                    .font(.caption)
                    .foregroundColor(.secondary)

                Toggle("On-Demand Model Loading", isOn: $onDemandLoading)
                    .onChange(of: onDemandLoading) { newValue in
                        ConfigManager.shared.updateConfig(key: "on_demand_loading", value: newValue ? "true" : "false", section: "[whisper]")
                    }

                Text("Load model only when recording starts. Saves memory but adds latency on first recording.")
                    .font(.caption)
                    .foregroundColor(.secondary)
            } header: {
                Text("Performance")
            }
        }
        .formStyle(.grouped)
        .onAppear {
            loadSettings()
        }
    }

    private func loadSettings() {
        let config = ConfigManager.shared.readConfig()

        if let m = config["whisper.mode"]?.replacingOccurrences(of: "\"", with: "") {
            mode = m
        } else if let b = config["whisper.backend"]?.replacingOccurrences(of: "\"", with: "") {
            // Legacy field name
            mode = b
        }

        if let lang = config["whisper.language"]?.replacingOccurrences(of: "\"", with: "") {
            language = lang
        }

        if let trans = config["whisper.translate"] {
            translate = trans == "true"
        }

        if let gpu = config["whisper.gpu_isolation"] {
            gpuIsolation = gpu == "true"
        }

        if let onDemand = config["whisper.on_demand_loading"] {
            onDemandLoading = onDemand == "true"
        }

        if let prompt = config["whisper.initial_prompt"]?.replacingOccurrences(of: "\"", with: "") {
            initialPrompt = prompt
        }
    }

    private func saveInitialPrompt() {
        if initialPrompt.isEmpty {
            ConfigManager.shared.updateConfig(key: "initial_prompt", value: "# empty", section: "[whisper]")
        } else {
            // Escape quotes in the prompt
            let escaped = initialPrompt.replacingOccurrences(of: "\"", with: "\\\"")
            ConfigManager.shared.updateConfig(key: "initial_prompt", value: "\"\(escaped)\"", section: "[whisper]")
        }
    }
}
