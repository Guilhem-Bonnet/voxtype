import SwiftUI

struct ModelsSettingsView: View {
    @State private var installedModels: [ModelInfo] = []
    @State private var selectedModel: String = ""
    @State private var isDownloading: Bool = false
    @State private var downloadProgress: String = ""

    var body: some View {
        Form {
            Section {
                if installedModels.isEmpty {
                    Text("No models installed")
                        .foregroundColor(.secondary)
                } else {
                    ForEach(installedModels, id: \.name) { model in
                        HStack {
                            VStack(alignment: .leading) {
                                Text(model.name)
                                    .fontWeight(model.name == selectedModel ? .semibold : .regular)
                                Text(model.size)
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                            }

                            Spacer()

                            if model.name == selectedModel {
                                Image(systemName: "checkmark.circle.fill")
                                    .foregroundColor(.green)
                            } else {
                                Button("Select") {
                                    selectModel(model.name)
                                }
                            }
                        }
                        .padding(.vertical, 4)
                    }
                }
            } header: {
                Text("Installed Models")
            }

            Section {
                VStack(alignment: .leading, spacing: 12) {
                    Text("Parakeet (Recommended)")
                        .font(.headline)

                    HStack {
                        Button("Download parakeet-tdt-0.6b-v3-int8") {
                            downloadModel("parakeet-tdt-0.6b-v3-int8")
                        }
                        .disabled(isDownloading)

                        Text("~640 MB - Quantized, fast")
                            .foregroundColor(.secondary)
                    }

                    HStack {
                        Button("Download parakeet-tdt-0.6b-v3") {
                            downloadModel("parakeet-tdt-0.6b-v3")
                        }
                        .disabled(isDownloading)

                        Text("~1.2 GB - Full precision")
                            .foregroundColor(.secondary)
                    }
                }

                Divider()

                VStack(alignment: .leading, spacing: 12) {
                    Text("Whisper English-Only")
                        .font(.headline)

                    Text("Optimized for English, faster and more accurate")
                        .font(.caption)
                        .foregroundColor(.secondary)

                    HStack {
                        Button("Download base.en") {
                            downloadModel("base.en")
                        }
                        .disabled(isDownloading)

                        Text("~142 MB")
                            .foregroundColor(.secondary)
                    }

                    HStack {
                        Button("Download small.en") {
                            downloadModel("small.en")
                        }
                        .disabled(isDownloading)

                        Text("~466 MB")
                            .foregroundColor(.secondary)
                    }

                    HStack {
                        Button("Download medium.en") {
                            downloadModel("medium.en")
                        }
                        .disabled(isDownloading)

                        Text("~1.5 GB")
                            .foregroundColor(.secondary)
                    }
                }

                Divider()

                VStack(alignment: .leading, spacing: 12) {
                    Text("Whisper Multilingual")
                        .font(.headline)

                    Text("Supports 99 languages, can translate to English")
                        .font(.caption)
                        .foregroundColor(.secondary)

                    HStack {
                        Button("Download base") {
                            downloadModel("base")
                        }
                        .disabled(isDownloading)

                        Text("~142 MB")
                            .foregroundColor(.secondary)
                    }

                    HStack {
                        Button("Download small") {
                            downloadModel("small")
                        }
                        .disabled(isDownloading)

                        Text("~466 MB")
                            .foregroundColor(.secondary)
                    }

                    HStack {
                        Button("Download medium") {
                            downloadModel("medium")
                        }
                        .disabled(isDownloading)

                        Text("~1.5 GB")
                            .foregroundColor(.secondary)
                    }

                    HStack {
                        Button("Download large-v3") {
                            downloadModel("large-v3")
                        }
                        .disabled(isDownloading)

                        Text("~3.1 GB - Best quality")
                            .foregroundColor(.secondary)
                    }

                    HStack {
                        Button("Download large-v3-turbo") {
                            downloadModel("large-v3-turbo")
                        }
                        .disabled(isDownloading)

                        Text("~1.6 GB - Fast, near large quality")
                            .foregroundColor(.secondary)
                    }
                }

                if isDownloading {
                    HStack {
                        ProgressView()
                            .scaleEffect(0.8)
                        Text(downloadProgress)
                            .foregroundColor(.secondary)
                    }
                }
            } header: {
                Text("Download Models")
            }
        }
        .formStyle(.grouped)
        .onAppear {
            loadInstalledModels()
        }
    }

    private func loadInstalledModels() {
        let modelsDir = NSHomeDirectory() + "/Library/Application Support/voxtype/models"

        guard let contents = try? FileManager.default.contentsOfDirectory(atPath: modelsDir) else {
            return
        }

        var models: [ModelInfo] = []

        for item in contents {
            let path = modelsDir + "/" + item

            var isDir: ObjCBool = false
            FileManager.default.fileExists(atPath: path, isDirectory: &isDir)

            if isDir.boolValue && item.contains("parakeet") {
                // Parakeet model directory
                let size = getDirectorySize(path)
                models.append(ModelInfo(name: item, size: formatSize(size), isParakeet: true))
            } else if item.hasPrefix("ggml-") && item.hasSuffix(".bin") {
                // Whisper model file
                if let attrs = try? FileManager.default.attributesOfItem(atPath: path),
                   let size = attrs[.size] as? Int64 {
                    let modelName = item
                        .replacingOccurrences(of: "ggml-", with: "")
                        .replacingOccurrences(of: ".bin", with: "")
                    models.append(ModelInfo(name: modelName, size: formatSize(size), isParakeet: false))
                }
            }
        }

        installedModels = models

        // Get currently selected model from config
        if let engine = ConfigManager.shared.getString("engine"), engine == "parakeet" {
            if let model = ConfigManager.shared.getString("parakeet.model") {
                selectedModel = model
            }
        } else {
            if let model = ConfigManager.shared.getString("whisper.model") {
                selectedModel = model
            }
        }
    }

    private func selectModel(_ name: String) {
        let isParakeet = name.contains("parakeet")

        if isParakeet {
            ConfigManager.shared.updateConfig(key: "engine", value: "\"parakeet\"")
            ConfigManager.shared.updateConfig(key: "model", value: "\"\(name)\"", section: "[parakeet]")
        } else {
            ConfigManager.shared.updateConfig(key: "engine", value: "\"whisper\"")
            ConfigManager.shared.updateConfig(key: "model", value: "\"\(name)\"", section: "[whisper]")
        }

        selectedModel = name
    }

    private func downloadModel(_ name: String) {
        isDownloading = true
        downloadProgress = "Downloading \(name)..."

        DispatchQueue.global().async {
            let result = VoxtypeCLI.run(["setup", "--download", "--model", name])

            DispatchQueue.main.async {
                isDownloading = false
                downloadProgress = ""
                loadInstalledModels()

                if result.success {
                    selectModel(name)
                }
            }
        }
    }

    private func getDirectorySize(_ path: String) -> Int64 {
        var size: Int64 = 0
        if let enumerator = FileManager.default.enumerator(atPath: path) {
            while let file = enumerator.nextObject() as? String {
                let filePath = path + "/" + file
                if let attrs = try? FileManager.default.attributesOfItem(atPath: filePath),
                   let fileSize = attrs[.size] as? Int64 {
                    size += fileSize
                }
            }
        }
        return size
    }

    private func formatSize(_ bytes: Int64) -> String {
        let mb = Double(bytes) / 1_000_000
        if mb >= 1000 {
            return String(format: "%.1f GB", mb / 1000)
        }
        return String(format: "%.0f MB", mb)
    }
}

struct ModelInfo {
    let name: String
    let size: String
    let isParakeet: Bool
}
