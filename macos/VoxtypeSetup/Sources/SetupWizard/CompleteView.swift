import SwiftUI

struct CompleteView: View {
    @EnvironmentObject var setupState: SetupState

    var body: some View {
        VStack(spacing: 16) {
            Spacer(minLength: 10)

            Image(systemName: "checkmark.circle.fill")
                .resizable()
                .frame(width: 60, height: 60)
                .foregroundColor(.green)

            VStack(spacing: 8) {
                Text("You're All Set!")
                    .font(.title)
                    .fontWeight(.bold)

                Text("Voxtype is ready to use")
                    .font(.body)
                    .foregroundColor(.secondary)
            }

            // Usage instructions
            VStack(alignment: .leading, spacing: 12) {
                InstructionRow(
                    step: "1",
                    title: "Hold your hotkey",
                    description: "Hold the Right Option key to start recording"
                )

                InstructionRow(
                    step: "2",
                    title: "Speak clearly",
                    description: "You'll see an orange mic indicator in your menu bar"
                )

                InstructionRow(
                    step: "3",
                    title: "Release to transcribe",
                    description: "Let go and your speech will be typed at your cursor"
                )
            }
            .padding(.horizontal, 50)
            .padding(.vertical, 12)

            // Hotkey reminder
            HStack {
                Image(systemName: "keyboard")
                    .foregroundColor(.accentColor)
                Text("Default hotkey: ")
                    .foregroundColor(.secondary)
                Text("Right Option (⌥)")
                    .fontWeight(.medium)
            }
            .padding(10)
            .background(Color(NSColor.controlBackgroundColor))
            .cornerRadius(8)

            Spacer(minLength: 10)

            // Actions
            HStack(spacing: 20) {
                Button("Open Preferences") {
                    setupState.markWizardComplete()
                }
                .buttonStyle(WizardButtonStyle())

                Button("Start Using Voxtype") {
                    // Save completion state and quit immediately
                    UserDefaults.standard.set(true, forKey: "wizardCompleted")
                    NSApplication.shared.terminate(nil)
                }
                .buttonStyle(WizardButtonStyle(isPrimary: true))
            }
            .padding(.horizontal, 40)
            .padding(.bottom, 20)
        }
    }
}

struct InstructionRow: View {
    let step: String
    let title: String
    let description: String

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Text(step)
                .font(.body)
                .fontWeight(.bold)
                .foregroundColor(.white)
                .frame(width: 24, height: 24)
                .background(Color.accentColor)
                .clipShape(Circle())

            VStack(alignment: .leading, spacing: 1) {
                Text(title)
                    .font(.callout)
                    .fontWeight(.medium)
                Text(description)
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Spacer()
        }
    }
}

#Preview {
    CompleteView()
        .environmentObject(SetupState())
        .frame(width: 600, height: 500)
}
