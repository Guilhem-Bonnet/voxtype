//! System tray icon for Voxtype
//!
//! Implements the [StatusNotifierItem] D-Bus protocol via the `ksni` crate.
//! The tray icon reflects the current daemon state (idle/recording/transcribing/stopped)
//! and provides a context menu with quick actions.
//!
//! # Architecture
//!
//! The tray runs on a dedicated thread with its own tokio runtime, independently
//! of the GTK4 main loop. It monitors daemon state via the shared status bus
//! (`status_bus` module) which provides a single `voxtype status --follow` subprocess.
//!
//! [StatusNotifierItem]: https://www.freedesktop.org/wiki/Specifications/StatusNotifierItem/

use ksni::menu::{MenuItem, StandardItem};
use ksni::{ToolTip, TrayMethods};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Voxtype daemon state as seen by the tray.
#[derive(Debug, Clone, PartialEq)]
enum TrayState {
    Idle,
    Recording,
    Transcribing,
    Stopped,
}

/// Optional information about an available update, displayed in the tray.
#[derive(Debug, Clone, Default)]
struct TrayUpdateInfo {
    /// Available version string (e.g. "0.6.0"), or empty when no update.
    version: String,
}

impl TrayState {
    fn from_class(class: &str) -> Self {
        match class {
            "recording" => Self::Recording,
            "transcribing" => Self::Transcribing,
            "idle" => Self::Idle,
            _ => Self::Stopped,
        }
    }

    /// Freedesktop icon name for the current state.
    fn icon_name(&self) -> &'static str {
        match self {
            Self::Idle => "audio-input-microphone-symbolic",
            Self::Recording => "media-record-symbolic",
            Self::Transcribing => "view-refresh-symbolic",
            Self::Stopped => "microphone-sensitivity-muted-symbolic",
        }
    }

    /// Human-readable label for the menu status line.
    fn label(&self) -> &'static str {
        match self {
            Self::Idle => "Prêt",
            Self::Recording => "Enregistrement…",
            Self::Transcribing => "Transcription…",
            Self::Stopped => "Daemon inactif",
        }
    }

    /// Tooltip body text.
    fn tooltip_body(&self) -> &'static str {
        match self {
            Self::Idle => "Maintenez la touche pour dicter",
            Self::Recording => "Enregistrement en cours",
            Self::Transcribing => "Transcription en cours…",
            Self::Stopped => "Le daemon Voxtype n'est pas en cours d'exécution",
        }
    }
}

/// Shared flag to signal the quit action from the tray thread to the GTK main thread.
static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);

/// System tray icon for Voxtype.
///
/// Implements `ksni::Tray` to register a StatusNotifierItem via D-Bus.
/// The icon, tooltip and menu update automatically when the daemon state changes.
/// When an update is available, the tooltip and menu reflect it.
#[derive(Debug)]
struct VoxtypeTray {
    state: TrayState,
    update_info: TrayUpdateInfo,
    /// Shared flag: when true, the GTK main loop will call app.quit()
    quit_flag: Arc<AtomicBool>,
}

impl ksni::Tray for VoxtypeTray {
    fn id(&self) -> String {
        "voxtype".into()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }

    fn title(&self) -> String {
        "Voxtype".into()
    }

    fn status(&self) -> ksni::Status {
        if !self.update_info.version.is_empty() {
            return ksni::Status::NeedsAttention;
        }
        match self.state {
            TrayState::Recording => ksni::Status::NeedsAttention,
            _ => ksni::Status::Active,
        }
    }

    fn icon_name(&self) -> String {
        self.state.icon_name().into()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        // Left-click on tray icon → toggle recording (primary action)
        if let Err(e) = Command::new("voxtype")
            .args(["record", "toggle"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            tracing::warn!("Failed to toggle recording: {e}");
        }
    }

    fn tool_tip(&self) -> ToolTip {
        let mut description = self.state.tooltip_body().to_string();
        if !self.update_info.version.is_empty() {
            description.push_str(&format!(
                "\nMise à jour disponible : {}",
                self.update_info.version
            ));
        }
        ToolTip {
            title: format!("Voxtype — {}", self.state.label()),
            description,
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items: Vec<MenuItem<Self>> = Vec::new();

        // "🎙 Enregistrer / ⏹ Arrêter" — primary action, toggle recording
        let record_label = match self.state {
            TrayState::Recording => "⏹ Arrêter l'enregistrement",
            _ => "🎙 Enregistrer",
        };
        items.push(
            StandardItem {
                label: record_label.into(),
                icon_name: match self.state {
                    TrayState::Recording => "media-playback-stop-symbolic".into(),
                    _ => "media-record-symbolic".into(),
                },
                activate: Box::new(|_| {
                    let _ = Command::new("voxtype")
                        .args(["record", "toggle"])
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn();
                }),
                ..Default::default()
            }
            .into(),
        );

        items.push(MenuItem::Separator);

        // "Paramètres" → open settings window via D-Bus GAction
        // (the GTK app registers 'open-settings' action on D-Bus)
        items.push(
            StandardItem {
                label: "Paramètres".into(),
                icon_name: "preferences-system-symbolic".into(),
                activate: Box::new(|_| {
                    let _ = Command::new("busctl")
                        .args([
                            "--user", "call",
                            "io.github.voxtype.Voxtype",
                            "/io/github/voxtype/Voxtype",
                            "org.gtk.Actions", "Activate",
                            "sava{sv}",
                            "open-settings", "0", "0",
                        ])
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn();
                }),
                ..Default::default()
            }
            .into(),
        );

        // "État: {état}" — informational with icon
        items.push(
            StandardItem {
                label: format!("État : {}", self.state.label()),
                icon_name: self.state.icon_name().into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
        );

        // "Mise à jour disponible" — shown only when update exists
        if !self.update_info.version.is_empty() {
            let version = self.update_info.version.clone();
            items.push(
                StandardItem {
                    label: format!("⬆ Mise à jour {} disponible", version),
                    icon_name: "software-update-available-symbolic".into(),
                    activate: Box::new(move |_| {
                        let _ = Command::new("busctl")
                            .args([
                                "--user", "call",
                                "io.github.voxtype.Voxtype",
                                "/io/github/voxtype/Voxtype",
                                "org.gtk.Actions", "Activate",
                                "sava{sv}",
                                "open-settings", "0", "0",
                            ])
                            .stdin(Stdio::null())
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .spawn();
                    }),
                    ..Default::default()
                }
                .into(),
            );
        }

        items.push(MenuItem::Separator);

        // "Quitter l'interface" → graceful shutdown (daemon keeps running)
        items.push(
            StandardItem {
                label: "Quitter l'interface".into(),
                icon_name: "application-exit-symbolic".into(),
                activate: Box::new(|tray: &mut VoxtypeTray| {
                    // Signal the GTK main loop to quit gracefully
                    tray.quit_flag.store(true, Ordering::SeqCst);
                    QUIT_REQUESTED.store(true, Ordering::SeqCst);
                }),
                ..Default::default()
            }
            .into(),
        );

        items
    }
}

/// Start the tray icon on a dedicated thread.
///
/// This function spawns a background thread with its own tokio runtime.
/// The tray icon registers via D-Bus and monitors daemon state changes.
/// It runs independently of the GTK4 main loop.
///
/// # Panics
///
/// Panics if the tokio runtime cannot be created.
pub fn start_tray(app: &libadwaita::Application, status_rx: std::sync::mpsc::Receiver<String>) {
    use gtk4::prelude::*;
    let app_clone = app.clone();

    // Poll quit flag from the GLib main loop — when the tray signals quit,
    // we call app.quit() from the GTK thread (safe, unlike process::exit).
    gtk4::glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
        if QUIT_REQUESTED.load(Ordering::SeqCst) {
            app_clone.quit();
            return gtk4::glib::ControlFlow::Break;
        }
        gtk4::glib::ControlFlow::Continue
    });

    let quit_flag = Arc::new(AtomicBool::new(false));

    std::thread::Builder::new()
        .name("voxtype-tray".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("failed to create tokio runtime for tray");

            rt.block_on(async {
                let tray = VoxtypeTray {
                    state: TrayState::Idle,
                    update_info: TrayUpdateInfo::default(),
                    quit_flag: quit_flag.clone(),
                };

                let handle = match tray.spawn().await {
                    Ok(h) => h,
                    Err(e) => {
                        tracing::warn!("Could not start tray icon: {e}");
                        tracing::info!(
                            "Install a StatusNotifierHost (e.g., GNOME AppIndicator extension)"
                        );
                        return;
                    }
                };

                // Start periodic update checking (Story 6.1/6.2)
                let update_handle = handle.clone();
                tokio::spawn(check_for_updates_periodic(update_handle));

                // Monitor daemon state via shared status bus
                monitor_state(handle, status_rx).await;
            });
        })
        .expect("failed to spawn tray thread");
}

/// Periodically check for updates via GitHub API and update the tray.
///
/// Runs one check at startup, then every 24 hours. Respects config and dismissal.
async fn check_for_updates_periodic(handle: ksni::Handle<VoxtypeTray>) {
    use super::update;

    // Check interval: 24 hours
    let interval = std::time::Duration::from_secs(24 * 60 * 60);

    loop {
        // Respect config
        if update::is_check_enabled() {
            // Run the HTTP check in a blocking context (curl subprocess)
            let result = tokio::task::spawn_blocking(update::check_github_release)
                .await
                .unwrap_or(None);

            if let Some(info) = result {
                if !update::is_dismissed(&info.version) {
                    let version = info.version.clone();
                    handle
                        .update(move |tray: &mut VoxtypeTray| {
                            tray.update_info = TrayUpdateInfo { version };
                        })
                        .await;
                }
            } else {
                // No update or check failed — clear any previous info
                handle
                    .update(|tray: &mut VoxtypeTray| {
                        tray.update_info = TrayUpdateInfo::default();
                    })
                    .await;
            }
        }

        tokio::time::sleep(interval).await;
    }
}

/// Monitor daemon state using the shared status bus receiver.
///
/// Bridges the `std::sync::mpsc::Receiver` into the tokio async world
/// via `spawn_blocking`, then processes lines asynchronously.
async fn monitor_state(
    handle: ksni::Handle<VoxtypeTray>,
    status_rx: std::sync::mpsc::Receiver<String>,
) {
    // Bridge std::sync::mpsc → tokio::sync::mpsc so we don't block the runtime
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(32);

    tokio::task::spawn_blocking(move || {
        for line in status_rx.iter() {
            if tx.blocking_send(line).is_err() {
                break; // Receiver dropped
            }
        }
    });

    // Process lines asynchronously — doesn't block the runtime
    while let Some(line) = rx.recv().await {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
            if let Some(class) = json.get("class").and_then(|v| v.as_str()) {
                let new_state = TrayState::from_class(class);
                handle
                    .update(move |tray: &mut VoxtypeTray| {
                        if tray.state != new_state {
                            tray.state = new_state.clone();
                        }
                    })
                    .await;
            }
        }
    }

    // Bus ended — set stopped state
    handle
        .update(|tray: &mut VoxtypeTray| {
            tray.state = TrayState::Stopped;
        })
        .await;
}
