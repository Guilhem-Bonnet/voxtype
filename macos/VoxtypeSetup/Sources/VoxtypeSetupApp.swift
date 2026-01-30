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
    }
}

/// Tracks overall setup state
class SetupState: ObservableObject {
    @Published var setupComplete: Bool = false
    @Published var currentStep: SetupStep = .welcome

    private let wizardCompletedKey = "wizardCompleted"

    init() {
        // Only show preferences if wizard was explicitly completed
        setupComplete = UserDefaults.standard.bool(forKey: wizardCompletedKey)
    }

    /// Mark wizard as completed (called when user finishes the wizard)
    func markWizardComplete() {
        UserDefaults.standard.set(true, forKey: wizardCompletedKey)
        setupComplete = true
    }

    /// Reset to show wizard again
    func resetWizard() {
        UserDefaults.standard.set(false, forKey: wizardCompletedKey)
        setupComplete = false
        currentStep = .welcome
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
