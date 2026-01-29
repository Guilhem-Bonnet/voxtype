import SwiftUI

struct WelcomeView: View {
    @EnvironmentObject var setupState: SetupState

    var body: some View {
        VStack(spacing: 30) {
            Spacer()

            // App icon placeholder
            Image(systemName: "mic.circle.fill")
                .resizable()
                .frame(width: 80, height: 80)
                .foregroundColor(.accentColor)

            VStack(spacing: 12) {
                Text("Welcome to Voxtype")
                    .font(.largeTitle)
                    .fontWeight(.bold)

                Text("Push-to-talk voice transcription for macOS")
                    .font(.title3)
                    .foregroundColor(.secondary)
            }

            VStack(alignment: .leading, spacing: 16) {
                FeatureRow(icon: "hand.tap", title: "Push-to-Talk",
                          description: "Hold a key to record, release to transcribe")
                FeatureRow(icon: "text.cursor", title: "Type Anywhere",
                          description: "Transcribed text appears at your cursor")
                FeatureRow(icon: "bolt", title: "Fast & Private",
                          description: "Runs locally on your Mac, no cloud required")
            }
            .padding(.horizontal, 60)
            .padding(.vertical, 20)

            Spacer()

            // Navigation
            HStack {
                Spacer()
                Button("Get Started") {
                    withAnimation {
                        setupState.currentStep = .permissions
                    }
                }
                .buttonStyle(WizardButtonStyle(isPrimary: true))
            }
            .padding(.horizontal, 40)
            .padding(.bottom, 30)
        }
    }
}

struct FeatureRow: View {
    let icon: String
    let title: String
    let description: String

    var body: some View {
        HStack(spacing: 16) {
            Image(systemName: icon)
                .font(.title2)
                .foregroundColor(.accentColor)
                .frame(width: 30)

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
    WelcomeView()
        .environmentObject(SetupState())
        .frame(width: 600, height: 500)
}
