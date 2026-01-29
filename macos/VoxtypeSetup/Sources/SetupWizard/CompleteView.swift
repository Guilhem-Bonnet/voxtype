import SwiftUI

struct CompleteView: View {
    @EnvironmentObject var setupState: SetupState

    var body: some View {
        VStack(spacing: 30) {
            Spacer()

            Image(systemName: "checkmark.circle.fill")
                .resizable()
                .frame(width: 80, height: 80)
                .foregroundColor(.green)

            VStack(spacing: 12) {
                Text("You're All Set!")
                    .font(.largeTitle)
                    .fontWeight(.bold)

                Text("Voxtype is ready to use")
                    .font(.title3)
                    .foregroundColor(.secondary)
            }

            // Usage instructions
            VStack(alignment: .leading, spacing: 20) {
                InstructionRow(
                    step: "1",
                    title: "Hold your hotkey",
                    description: "By default, hold the Right Option key to start recording"
                )

                InstructionRow(
                    step: "2",
                    title: "Speak clearly",
                    description: "Talk normally - you'll see the orange mic indicator in your menu bar"
                )

                InstructionRow(
                    step: "3",
                    title: "Release to transcribe",
                    description: "Let go of the key and your speech will be typed at your cursor"
                )
            }
            .padding(.horizontal, 60)
            .padding(.vertical, 20)

            // Hotkey reminder
            HStack {
                Image(systemName: "keyboard")
                    .foregroundColor(.accentColor)
                Text("Default hotkey: ")
                    .foregroundColor(.secondary)
                Text("Right Option (⌥)")
                    .fontWeight(.medium)
            }
            .padding()
            .background(Color(NSColor.controlBackgroundColor))
            .cornerRadius(10)

            Spacer()

            // Actions
            HStack(spacing: 20) {
                Button("Open Preferences") {
                    setupState.setupComplete = true
                }
                .buttonStyle(WizardButtonStyle())

                Button("Start Using Voxtype") {
                    NSApplication.shared.terminate(nil)
                }
                .buttonStyle(WizardButtonStyle(isPrimary: true))
            }
            .padding(.horizontal, 40)
            .padding(.bottom, 30)
        }
    }
}

struct InstructionRow: View {
    let step: String
    let title: String
    let description: String

    var body: some View {
        HStack(alignment: .top, spacing: 16) {
            Text(step)
                .font(.title2)
                .fontWeight(.bold)
                .foregroundColor(.white)
                .frame(width: 32, height: 32)
                .background(Color.accentColor)
                .clipShape(Circle())

            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .fontWeight(.medium)
                Text(description)
                    .font(.callout)
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
