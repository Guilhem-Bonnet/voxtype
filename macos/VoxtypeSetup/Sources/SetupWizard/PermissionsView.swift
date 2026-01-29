import SwiftUI

struct PermissionsView: View {
    @EnvironmentObject var setupState: SetupState
    @StateObject private var permissions = PermissionChecker.shared
    @State private var isCheckingPermissions = false

    var allPermissionsGranted: Bool {
        permissions.hasMicrophoneAccess &&
        permissions.hasAccessibilityAccess &&
        permissions.hasInputMonitoringAccess
    }

    var body: some View {
        VStack(spacing: 20) {
            Spacer()

            Text("Permissions Required")
                .font(.title)
                .fontWeight(.bold)

            Text("Voxtype needs a few permissions to work properly.\nClick each button to grant access.")
                .multilineTextAlignment(.center)
                .foregroundColor(.secondary)
                .padding(.bottom, 10)

            VStack(spacing: 16) {
                PermissionRow(
                    title: "Microphone",
                    description: "To capture your voice for transcription",
                    icon: "mic.fill",
                    isGranted: permissions.hasMicrophoneAccess,
                    action: {
                        permissions.requestMicrophoneAccess { _ in }
                    }
                )

                PermissionRow(
                    title: "Accessibility",
                    description: "To type transcribed text into applications",
                    icon: "accessibility",
                    isGranted: permissions.hasAccessibilityAccess,
                    action: {
                        permissions.requestAccessibilityAccess()
                    }
                )

                PermissionRow(
                    title: "Input Monitoring",
                    description: "To detect your push-to-talk hotkey",
                    icon: "keyboard",
                    isGranted: permissions.hasInputMonitoringAccess,
                    action: {
                        permissions.openInputMonitoringSettings()
                    }
                )
            }
            .padding(.horizontal, 40)

            // Refresh button
            Button(action: {
                isCheckingPermissions = true
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                    permissions.refresh()
                    isCheckingPermissions = false
                }
            }) {
                HStack {
                    if isCheckingPermissions {
                        ProgressView()
                            .scaleEffect(0.8)
                            .frame(width: 16, height: 16)
                    } else {
                        Image(systemName: "arrow.clockwise")
                    }
                    Text("Check Permissions")
                }
            }
            .buttonStyle(.borderless)
            .padding(.top, 10)

            if allPermissionsGranted {
                HStack {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundColor(.green)
                    Text("All permissions granted!")
                        .foregroundColor(.green)
                }
                .padding(.top, 10)
            }

            Spacer()

            // Navigation
            HStack {
                Button("Back") {
                    withAnimation {
                        setupState.currentStep = .welcome
                    }
                }
                .buttonStyle(WizardButtonStyle())

                Spacer()

                Button("Continue") {
                    withAnimation {
                        setupState.currentStep = .model
                    }
                }
                .buttonStyle(WizardButtonStyle(isPrimary: true))
                .disabled(!allPermissionsGranted)
            }
            .padding(.horizontal, 40)
            .padding(.bottom, 30)
        }
    }
}

struct PermissionRow: View {
    let title: String
    let description: String
    let icon: String
    let isGranted: Bool
    let action: () -> Void

    var body: some View {
        HStack(spacing: 16) {
            Image(systemName: icon)
                .font(.title2)
                .foregroundColor(isGranted ? .green : .accentColor)
                .frame(width: 30)

            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .fontWeight(.medium)
                Text(description)
                    .font(.callout)
                    .foregroundColor(.secondary)
            }

            Spacer()

            if isGranted {
                Image(systemName: "checkmark.circle.fill")
                    .foregroundColor(.green)
                    .font(.title2)
            } else {
                Button("Grant Access") {
                    action()
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.small)
            }
        }
        .padding()
        .background(Color(NSColor.controlBackgroundColor))
        .cornerRadius(10)
    }
}

#Preview {
    PermissionsView()
        .environmentObject(SetupState())
        .frame(width: 600, height: 500)
}
