import SwiftUI

struct LaunchAgentView: View {
    @EnvironmentObject var setupState: SetupState
    @StateObject private var cli = VoxtypeCLI.shared

    @State private var enableAutoStart = true
    @State private var isInstalling = false
    @State private var installComplete = false
    @State private var errorMessage: String?

    var body: some View {
        VStack(spacing: 20) {
            Spacer()

            Image(systemName: "arrow.clockwise.circle.fill")
                .resizable()
                .frame(width: 60, height: 60)
                .foregroundColor(.accentColor)

            Text("Auto-Start")
                .font(.title)
                .fontWeight(.bold)

            Text("Would you like Voxtype to start automatically when you log in?")
                .multilineTextAlignment(.center)
                .foregroundColor(.secondary)
                .padding(.horizontal, 60)

            VStack(spacing: 16) {
                Toggle(isOn: $enableAutoStart) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Start Voxtype at Login")
                            .fontWeight(.medium)
                        Text("Voxtype will run in the background and be ready whenever you need it")
                            .font(.callout)
                            .foregroundColor(.secondary)
                    }
                }
                .toggleStyle(.switch)
                .padding()
                .background(Color(NSColor.controlBackgroundColor))
                .cornerRadius(10)
            }
            .padding(.horizontal, 40)

            if let error = errorMessage {
                Text(error)
                    .foregroundColor(.red)
                    .font(.callout)
            }

            if installComplete {
                HStack {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundColor(.green)
                    Text(enableAutoStart ? "Auto-start enabled!" : "Skipped auto-start")
                        .foregroundColor(.green)
                }
            }

            // Info box
            HStack(alignment: .top, spacing: 12) {
                Image(systemName: "info.circle")
                    .foregroundColor(.blue)
                VStack(alignment: .leading, spacing: 4) {
                    Text("How it works")
                        .fontWeight(.medium)
                    Text("Voxtype runs as a background service. It listens for your hotkey and transcribes speech when triggered. You can always start/stop it manually from Terminal.")
                        .font(.callout)
                        .foregroundColor(.secondary)
                }
            }
            .padding()
            .background(Color.blue.opacity(0.1))
            .cornerRadius(10)
            .padding(.horizontal, 40)

            Spacer()

            // Navigation
            HStack {
                Button("Back") {
                    withAnimation {
                        setupState.currentStep = .model
                    }
                }
                .buttonStyle(WizardButtonStyle())
                .disabled(isInstalling)

                Spacer()

                Button("Continue") {
                    installAndContinue()
                }
                .buttonStyle(WizardButtonStyle(isPrimary: true))
                .disabled(isInstalling)
            }
            .padding(.horizontal, 40)
            .padding(.bottom, 30)
        }
    }

    func installAndContinue() {
        isInstalling = true
        errorMessage = nil

        DispatchQueue.global(qos: .userInitiated).async {
            var success = true

            if enableAutoStart {
                success = cli.installLaunchAgent()
            }

            DispatchQueue.main.async {
                isInstalling = false

                if success {
                    installComplete = true

                    // Start the daemon
                    if enableAutoStart {
                        cli.startDaemon()
                    }

                    // Move to complete after a brief delay
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                        withAnimation {
                            setupState.currentStep = .complete
                        }
                    }
                } else {
                    errorMessage = "Failed to install auto-start. You can set this up later."
                }
            }
        }
    }
}

#Preview {
    LaunchAgentView()
        .environmentObject(SetupState())
        .frame(width: 600, height: 500)
}
