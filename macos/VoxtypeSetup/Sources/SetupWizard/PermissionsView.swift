import SwiftUI

struct PermissionsView: View {
    @EnvironmentObject var setupState: SetupState
    @StateObject private var permissions = PermissionChecker.shared
    @State private var isCheckingPermissions = false
    @State private var refreshTimer: Timer?

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
                ManualPermissionRow(
                    title: "Microphone",
                    description: "Add Voxtype.app to Microphone list",
                    icon: "mic.fill",
                    isGranted: permissions.hasMicrophoneAccess,
                    openAction: {
                        permissions.openMicrophoneSettings()
                    },
                    confirmAction: {
                        permissions.confirmMicrophoneAccess()
                    }
                )

                ManualPermissionRow(
                    title: "Accessibility",
                    description: "Add Voxtype.app to Accessibility list",
                    icon: "accessibility",
                    isGranted: permissions.hasAccessibilityAccess,
                    openAction: {
                        permissions.requestAccessibilityAccess()
                    },
                    confirmAction: {
                        permissions.confirmAccessibilityAccess()
                    }
                )

                ManualPermissionRow(
                    title: "Input Monitoring",
                    description: "Add Voxtype.app to Input Monitoring list",
                    icon: "keyboard",
                    isGranted: permissions.hasInputMonitoringAccess,
                    openAction: {
                        permissions.openInputMonitoringSettings()
                    },
                    confirmAction: {
                        permissions.confirmInputMonitoringAccess()
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
        .onAppear {
            // Start polling for permission changes
            refreshTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { _ in
                permissions.refresh()
            }
        }
        .onDisappear {
            refreshTimer?.invalidate()
            refreshTimer = nil
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

struct ManualPermissionRow: View {
    let title: String
    let description: String
    let icon: String
    let isGranted: Bool
    let openAction: () -> Void
    let confirmAction: () -> Void

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
                HStack(spacing: 8) {
                    Button("Open Settings") {
                        openAction()
                    }
                    .controlSize(.small)

                    Button("Done") {
                        confirmAction()
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                }
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
