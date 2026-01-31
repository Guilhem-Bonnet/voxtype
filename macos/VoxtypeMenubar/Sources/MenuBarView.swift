import SwiftUI

struct MenuBarView: View {
    @EnvironmentObject var statusMonitor: VoxtypeStatusMonitor

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Status section
            HStack {
                Image(systemName: statusMonitor.iconName)
                    .foregroundColor(statusColor)
                Text(statusMonitor.statusText)
                    .fontWeight(.medium)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)

            Divider()

            // Recording controls
            Button(action: toggleRecording) {
                Label("Toggle Recording", systemImage: "record.circle")
            }
            .keyboardShortcut("r", modifiers: [])
            .disabled(!statusMonitor.daemonRunning)

            Button(action: cancelRecording) {
                Label("Cancel Recording", systemImage: "xmark.circle")
            }
            .disabled(statusMonitor.state != .recording)

            Divider()

            // Settings submenu
            Menu("Settings") {
                Menu("Engine") {
                    Button("Parakeet (Fast)") {
                        setEngine("parakeet")
                    }
                    Button("Whisper") {
                        setEngine("whisper")
                    }
                }

                Menu("Output Mode") {
                    Button("Type Text") {
                        setOutputMode("type")
                    }
                    Button("Clipboard") {
                        setOutputMode("clipboard")
                    }
                    Button("Clipboard + Paste") {
                        setOutputMode("paste")
                    }
                }

                Menu("Hotkey Mode") {
                    Button("Push-to-Talk (hold)") {
                        setHotkeyMode("push_to_talk")
                    }
                    Button("Toggle (press)") {
                        setHotkeyMode("toggle")
                    }
                }
            }

            Divider()

            Button(action: openSetup) {
                Label("Open Setup...", systemImage: "gearshape")
            }

            Button(action: restartDaemon) {
                Label("Restart Daemon", systemImage: "arrow.clockwise")
            }

            Button(action: viewLogs) {
                Label("View Logs", systemImage: "doc.text")
            }

            Divider()

            Button(action: quitApp) {
                Label("Quit Menu Bar", systemImage: "power")
            }
            .keyboardShortcut("q", modifiers: .command)
        }
    }

    private var statusColor: Color {
        switch statusMonitor.state {
        case .idle:
            return .green
        case .recording:
            return .red
        case .transcribing:
            return .orange
        case .stopped:
            return .gray
        }
    }

    // MARK: - Actions

    private func toggleRecording() {
        VoxtypeCLI.run(["record", "toggle"])
    }

    private func cancelRecording() {
        VoxtypeCLI.run(["record", "cancel"])
    }

    private func setEngine(_ engine: String) {
        // Update config file
        updateConfig(key: "engine", value: "\"\(engine)\"", section: nil)
        showNotification(title: "Voxtype", message: "Engine set to \(engine). Restart daemon to apply.")
    }

    private func setOutputMode(_ mode: String) {
        updateConfig(key: "mode", value: "\"\(mode)\"", section: "[output]")
    }

    private func setHotkeyMode(_ mode: String) {
        updateConfig(key: "mode", value: "\"\(mode)\"", section: "[hotkey]")
        showNotification(title: "Voxtype", message: "Hotkey mode changed. Restart daemon to apply.")
    }

    private func openSetup() {
        // Try to open VoxtypeSetup from the app bundle
        let setupPath = Bundle.main.bundlePath
            .replacingOccurrences(of: "VoxtypeMenubar.app", with: "Voxtype.app/Contents/MacOS/VoxtypeSetup")

        if FileManager.default.fileExists(atPath: setupPath) {
            Process.launchedProcess(launchPath: setupPath, arguments: [])
        } else {
            // Fallback: open config file
            let configPath = NSHomeDirectory() + "/Library/Application Support/voxtype/config.toml"
            NSWorkspace.shared.open(URL(fileURLWithPath: configPath))
        }
    }

    private func restartDaemon() {
        VoxtypeCLI.run(["daemon", "restart"], wait: false)
        showNotification(title: "Voxtype", message: "Restarting daemon...")
    }

    private func viewLogs() {
        let logsPath = NSHomeDirectory() + "/Library/Logs/voxtype"
        NSWorkspace.shared.open(URL(fileURLWithPath: logsPath))
    }

    private func quitApp() {
        NSApplication.shared.terminate(nil)
    }

    // MARK: - Helpers

    private func updateConfig(key: String, value: String, section: String?) {
        let configPath = NSHomeDirectory() + "/Library/Application Support/voxtype/config.toml"

        guard var content = try? String(contentsOfFile: configPath, encoding: .utf8) else {
            return
        }

        let pattern = "\(key)\\s*=\\s*\"[^\"]*\""
        let replacement = "\(key) = \(value)"

        if let regex = try? NSRegularExpression(pattern: pattern, options: []) {
            let range = NSRange(content.startIndex..., in: content)
            content = regex.stringByReplacingMatches(in: content, options: [], range: range, withTemplate: replacement)
        }

        try? content.write(toFile: configPath, atomically: true, encoding: .utf8)
    }

    private func showNotification(title: String, message: String) {
        let script = "display notification \"\(message)\" with title \"\(title)\""
        if let appleScript = NSAppleScript(source: script) {
            var error: NSDictionary?
            appleScript.executeAndReturnError(&error)
        }
    }
}
