import SwiftUI

struct SetupWizardView: View {
    @EnvironmentObject var setupState: SetupState

    var body: some View {
        VStack(spacing: 0) {
            // Progress indicator
            ProgressBar(currentStep: setupState.currentStep)
                .padding(.horizontal, 40)
                .padding(.vertical, 16)

            Divider()

            // Step content
            Group {
                switch setupState.currentStep {
                case .welcome:
                    WelcomeView()
                case .permissions:
                    PermissionsView()
                case .model:
                    ModelSelectionView()
                case .launchAgent:
                    LaunchAgentView()
                case .complete:
                    CompleteView()
                }
            }
        }
        .frame(width: 600, height: 550)
        .background(Color(NSColor.windowBackgroundColor))
    }
}

struct ProgressBar: View {
    let currentStep: SetupStep

    var body: some View {
        HStack(spacing: 0) {
            ForEach(Array(SetupStep.allCases.enumerated()), id: \.element) { index, step in
                if index > 0 {
                    Rectangle()
                        .fill(index <= currentStep.rawValue ? Color.accentColor : Color.secondary.opacity(0.3))
                        .frame(height: 2)
                }

                VStack(spacing: 4) {
                    Circle()
                        .fill(index <= currentStep.rawValue ? Color.accentColor : Color.secondary.opacity(0.3))
                        .frame(width: 10, height: 10)

                    Text(step.title)
                        .font(.caption2)
                        .foregroundColor(index == currentStep.rawValue ? .primary : .secondary)
                }
                .frame(width: 80)
            }
        }
    }
}

// MARK: - Navigation Button Style

struct WizardButtonStyle: ButtonStyle {
    var isPrimary: Bool = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .padding(.horizontal, 20)
            .padding(.vertical, 10)
            .background(isPrimary ? Color.accentColor : Color.secondary.opacity(0.2))
            .foregroundColor(isPrimary ? .white : .primary)
            .cornerRadius(8)
            .opacity(configuration.isPressed ? 0.8 : 1.0)
    }
}

#Preview {
    SetupWizardView()
        .environmentObject(SetupState())
}
