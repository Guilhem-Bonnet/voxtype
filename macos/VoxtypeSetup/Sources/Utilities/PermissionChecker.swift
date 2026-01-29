import Foundation
import AVFoundation
import AppKit

/// Checks and requests macOS permissions required by Voxtype
class PermissionChecker: ObservableObject {
    static let shared = PermissionChecker()

    @Published var hasMicrophoneAccess: Bool = false
    @Published var hasAccessibilityAccess: Bool = false
    @Published var hasInputMonitoringAccess: Bool = false

    private init() {
        refresh()
    }

    /// Refresh all permission states
    func refresh() {
        checkMicrophoneAccess()
        checkAccessibilityAccess()
        checkInputMonitoringAccess()
    }

    // MARK: - Microphone

    private func checkMicrophoneAccess() {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized:
            hasMicrophoneAccess = true
        case .notDetermined, .denied, .restricted:
            hasMicrophoneAccess = false
        @unknown default:
            hasMicrophoneAccess = false
        }
    }

    func requestMicrophoneAccess(completion: @escaping (Bool) -> Void) {
        AVCaptureDevice.requestAccess(for: .audio) { granted in
            DispatchQueue.main.async {
                self.hasMicrophoneAccess = granted
                completion(granted)
            }
        }
    }

    // MARK: - Accessibility

    private func checkAccessibilityAccess() {
        hasAccessibilityAccess = AXIsProcessTrusted()
    }

    func requestAccessibilityAccess() {
        // This opens System Settings to the Accessibility pane
        let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true]
        AXIsProcessTrustedWithOptions(options as CFDictionary)

        // Also open the settings pane directly for clarity
        openAccessibilitySettings()
    }

    func openAccessibilitySettings() {
        let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")!
        NSWorkspace.shared.open(url)
    }

    // MARK: - Input Monitoring

    private func checkInputMonitoringAccess() {
        // There's no direct API to check Input Monitoring permission
        // We use a heuristic: try to create an event tap
        // If it fails, we likely don't have permission
        hasInputMonitoringAccess = canCreateEventTap()
    }

    private func canCreateEventTap() -> Bool {
        // Attempt to create a passive event tap
        // This will fail if Input Monitoring permission is not granted
        let eventMask = (1 << CGEventType.keyDown.rawValue)
        guard let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .listenOnly,
            eventsOfInterest: CGEventMask(eventMask),
            callback: { _, _, event, _ in Unmanaged.passUnretained(event) },
            userInfo: nil
        ) else {
            return false
        }
        // Clean up the tap
        CFMachPortInvalidate(tap)
        return true
    }

    func openInputMonitoringSettings() {
        let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")!
        NSWorkspace.shared.open(url)
    }

    // MARK: - Notifications (optional)

    func openNotificationSettings() {
        let url = URL(string: "x-apple.systempreferences:com.apple.preference.notifications")!
        NSWorkspace.shared.open(url)
    }
}
