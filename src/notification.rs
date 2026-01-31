//! Platform-specific desktop notifications
//!
//! Provides a unified interface for sending desktop notifications on
//! different platforms:
//! - Linux: Uses notify-send (libnotify)
//! - macOS: Uses native UserNotifications framework (appears under "Voxtype" in settings)

use std::process::Stdio;

#[cfg(target_os = "linux")]
use tokio::process::Command;

/// Send a desktop notification with the given title and body.
///
/// This function is async and non-blocking. Notification failures are
/// logged but don't propagate errors (notifications are best-effort).
pub async fn send(title: &str, body: &str) {
    #[cfg(target_os = "linux")]
    send_linux(title, body).await;

    #[cfg(target_os = "macos")]
    send_macos_native(title, body);

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        tracing::debug!("Notifications not supported on this platform");
        let _ = (title, body); // Suppress unused warnings
    }
}

/// Send a notification on Linux using notify-send
#[cfg(target_os = "linux")]
async fn send_linux(title: &str, body: &str) {
    let result = Command::new("notify-send")
        .args([
            "--app-name=Voxtype",
            "--expire-time=2000",
            title,
            body,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;

    if let Err(e) = result {
        tracing::debug!("Failed to send notification: {}", e);
    }
}

/// Send a macOS notification using terminal-notifier
/// Falls back to osascript if terminal-notifier is not installed
#[cfg(target_os = "macos")]
fn send_macos_native(title: &str, body: &str) {
    // Try bundled terminal-notifier first, then system PATH, then osascript
    let bundled_path =
        "/Applications/Voxtype.app/Contents/Resources/terminal-notifier.app/Contents/MacOS/terminal-notifier";

    let notifier_paths = [bundled_path, "terminal-notifier"];

    for notifier in notifier_paths {
        let result = std::process::Command::new(notifier)
            .args(["-title", title, "-message", body, "-sender", "io.voxtype.menubar"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        match result {
            Ok(status) if status.success() => {
                tracing::debug!("Sent notification via {}", notifier);
                return;
            }
            _ => continue,
        }
    }

    // Fallback to osascript
    tracing::debug!("terminal-notifier not available, using osascript");
    send_macos_osascript_sync(title, body);
}

/// Fallback notification via osascript (if native fails)
#[cfg(target_os = "macos")]
fn send_macos_osascript_sync(title: &str, body: &str) {
    let escaped_title = title.replace('"', "\\\"");
    let escaped_body = body.replace('"', "\\\"");

    let script = format!(
        r#"display notification "{}" with title "{}""#,
        escaped_body, escaped_title
    );

    let _ = std::process::Command::new("osascript")
        .args(["-e", &script])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Send a notification synchronously (blocking).
///
/// Used in non-async contexts like early startup warnings.
pub fn send_sync(title: &str, body: &str) {
    #[cfg(target_os = "linux")]
    send_linux_sync(title, body);

    #[cfg(target_os = "macos")]
    send_macos_native(title, body);

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (title, body); // Suppress unused warnings
    }
}

/// Send a notification on Linux using notify-send (synchronous)
#[cfg(target_os = "linux")]
fn send_linux_sync(title: &str, body: &str) {
    let _ = std::process::Command::new("notify-send")
        .args([
            "--app-name=Voxtype",
            "--expire-time=5000",
            title,
            body,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_quote_escaping() {
        // Test that quotes are properly escaped for AppleScript
        let title = r#"Test "title""#;
        let escaped = title.replace('"', "\\\"");
        assert_eq!(escaped, r#"Test \"title\""#);
    }
}
