//! Update checking module for Voxtype.
//!
//! # Architecture
//!
//! Checks for new versions by querying the GitHub Releases API.
//! Designed to be consumed independently by both the GTK settings window
//! (GLib main loop) and the system tray (tokio runtime).
//!
//! - **Startup check**: One check at launch.
//! - **Periodic check**: Repeats every 24 hours while the GUI is running.
//! - **Config-gated**: Disabled when `[update] check_enabled = false`.
//! - **Silent failure**: Network errors never surface to the user.
//! - **Dismissal**: Users can dismiss a version; the decision is persisted
//!   to `$XDG_CACHE_HOME/voxtype/update-dismissed`.
//!
//! # STABILITY: internal — not part of public API

use std::path::PathBuf;
use std::process::Command;

/// GitHub API URL for the latest release.
const GITHUB_RELEASES_URL: &str =
    "https://api.github.com/repos/peteonrails/voxtype/releases/latest";

/// Information about an available update.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// Semantic version string of the remote release (e.g. "0.6.0").
    pub version: String,
    /// URL to the release page / changelog.
    pub changelog_url: String,
}

// =========================================================================
// Public API
// =========================================================================

/// Check if an update is available by querying the GitHub Releases API.
///
/// Returns `Some(UpdateInfo)` when a newer version exists, `None` otherwise.
/// Fails silently on any network or parsing error (returns `None`).
pub fn check_github_release() -> Option<UpdateInfo> {
    let output = Command::new("curl")
        .args(["-sfL", "--max-time", "10", GITHUB_RELEASES_URL])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let json_str = String::from_utf8(output.stdout).ok()?;
    let json: serde_json::Value = serde_json::from_str(&json_str).ok()?;

    let tag = json.get("tag_name")?.as_str()?;
    let remote_version = tag.trim_start_matches('v');
    let current_version = env!("CARGO_PKG_VERSION");

    if is_newer(remote_version, current_version) {
        let changelog_url = json
            .get("html_url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Some(UpdateInfo {
            version: remote_version.to_string(),
            changelog_url,
        })
    } else {
        None
    }
}

/// Returns `true` if the user's config allows update checks.
///
/// Reads `~/.config/voxtype/config.toml` and looks for
/// `[update] check_enabled = false`. Defaults to `true` when
/// the key is absent or unreadable.
pub fn is_check_enabled() -> bool {
    let config_path = match directories::ProjectDirs::from("", "", "voxtype") {
        Some(dirs) => dirs.config_dir().join("config.toml"),
        None => return true,
    };

    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(_) => return true,
    };

    // Parse just enough to extract [update].check_enabled
    if let Ok(toml) = content.parse::<toml::Value>() {
        if let Some(update) = toml.get("update") {
            if let Some(enabled) = update.get("check_enabled") {
                return enabled.as_bool().unwrap_or(true);
            }
        }
    }

    true
}

/// Returns `true` if the given version was previously dismissed by the user.
pub fn is_dismissed(version: &str) -> bool {
    if let Some(path) = dismissed_file_path() {
        if let Ok(content) = std::fs::read_to_string(path) {
            return content.trim() == version;
        }
    }
    false
}

/// Persist the user's decision to dismiss a specific version.
pub fn dismiss_version(version: &str) {
    if let Some(path) = dismissed_file_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, version);
    }
}

// =========================================================================
// Internals
// =========================================================================

/// Path to the dismissed-version cache file.
fn dismissed_file_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "voxtype")
        .map(|dirs| dirs.cache_dir().join("update-dismissed"))
}

/// Rough semver comparison: `remote > current`.
///
/// Splits on `.`, compares numeric segments left-to-right.
/// Falls back to string inequality for non-numeric suffixes.
fn is_newer(remote: &str, current: &str) -> bool {
    let r_parts: Vec<&str> = remote.split('.').collect();
    let c_parts: Vec<&str> = current.split('.').collect();

    let len = r_parts.len().max(c_parts.len());
    for i in 0..len {
        let r = r_parts.get(i).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        let c = c_parts.get(i).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        if r > c {
            return true;
        }
        if r < c {
            return false;
        }
    }
    false // equal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_basic() {
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("0.6.0", "0.5.5"));
        assert!(is_newer("0.5.6", "0.5.5"));
        assert!(is_newer("1.0.0", "0.99.99"));
    }

    #[test]
    fn test_is_newer_equal() {
        assert!(!is_newer("0.5.5", "0.5.5"));
        assert!(!is_newer("1.0.0", "1.0.0"));
    }

    #[test]
    fn test_is_newer_older() {
        assert!(!is_newer("0.5.4", "0.5.5"));
        assert!(!is_newer("0.4.0", "0.5.5"));
    }

    #[test]
    fn test_is_newer_different_lengths() {
        assert!(is_newer("1.0.0.1", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.0.1"));
    }
}
