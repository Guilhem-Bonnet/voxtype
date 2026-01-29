import SwiftUI

@main
struct VoxtypeSetupApp: App {
    @StateObject private var setupState = SetupState()

    var body: some Scene {
        WindowGroup {
            if setupState.setupComplete {
                PreferencesView()
                    .environmentObject(setupState)
            } else {
                SetupWizardView()
                    .environmentObject(setupState)
            }
        }
        .windowStyle(.hiddenTitleBar)
        .windowResizability(.contentSize)
    }
}

/// Tracks overall setup state
class SetupState: ObservableObject {
    @Published var setupComplete: Bool = false
    @Published var currentStep: SetupStep = .welcome

    init() {
        // Check if setup has been completed before
        setupComplete = checkSetupComplete()
    }

    private func checkSetupComplete() -> Bool {
        // Setup is complete if:
        // 1. All permissions are granted
        // 2. A model is downloaded
        // 3. LaunchAgent is installed
        let permissions = PermissionChecker.shared
        let hasPermissions = permissions.hasMicrophoneAccess &&
                            permissions.hasAccessibilityAccess &&
                            permissions.hasInputMonitoringAccess
        let hasModel = VoxtypeCLI.shared.hasModel()
        let hasLaunchAgent = VoxtypeCLI.shared.hasLaunchAgent()

        return hasPermissions && hasModel && hasLaunchAgent
    }

    func recheckSetup() {
        setupComplete = checkSetupComplete()
    }
}

enum SetupStep: Int, CaseIterable {
    case welcome
    case permissions
    case model
    case launchAgent
    case complete

    var title: String {
        switch self {
        case .welcome: return "Welcome"
        case .permissions: return "Permissions"
        case .model: return "Speech Model"
        case .launchAgent: return "Auto-Start"
        case .complete: return "Complete"
        }
    }
}
