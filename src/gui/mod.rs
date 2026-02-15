//! GUI module for Voxtype
//!
//! Provides the graphical user interface components:
//! - Recording overlay with waveform visualization
//! - System tray icon with status indication
//! - Settings/preferences window (future)
//!
//! This module is gated behind the `gui` feature flag.
//! Build with: `cargo build --features gui`
//!
//! # Architecture
//!
//! The GUI runs as a GTK4/libadwaita application launched via `voxtype ui`.
//! It communicates with the daemon by:
//! - Reading status via `voxtype status --follow --format json` (stdout JSON stream)
//! - Sending commands via `voxtype record start/stop/toggle/cancel` (CLI invocation)
//!
//! The system tray runs on a separate thread with its own tokio runtime,
//! using the StatusNotifierItem D-Bus protocol via `ksni`.
//!
//! No new IPC mechanism is introduced — the GUI is a consumer of existing CLI commands.

pub mod overlay;
pub mod settings;
pub mod tray;
pub mod update;

use anyhow::Result;
use gtk4::prelude::*;
use libadwaita as adw;

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

/// Application ID for single-instance enforcement via D-Bus
const APP_ID: &str = "io.github.voxtype.Voxtype";

/// Whether to open the settings window on activation.
static OPEN_SETTINGS: AtomicBool = AtomicBool::new(false);

/// Guard to prevent starting the tray icon more than once.
static TRAY_STARTED: AtomicBool = AtomicBool::new(false);

/// Guard to prevent starting the overlay monitor more than once.
static OVERLAY_STARTED: AtomicBool = AtomicBool::new(false);

// Hold guard storage — kept alive for the app lifetime, released on quit.
// thread_local because ApplicationHoldGuard is not Sync, but GTK callbacks
// always run on the main thread.
thread_local! {
    static HOLD_GUARD: RefCell<Option<gtk4::gio::ApplicationHoldGuard>> = const { RefCell::new(None) };
}

/// Launch the Voxtype GUI application.
///
/// This is the main entry point called by `voxtype ui`.
/// Uses `GtkApplication` for single-instance enforcement:
/// if an instance is already running, the existing window is raised instead.
///
/// When `open_settings` is true, the preferences window is shown immediately.
///
/// # Errors
///
/// Returns an error if GTK initialization fails or the application
/// cannot acquire its D-Bus name.
pub fn launch(open_settings: bool) -> Result<()> {
    OPEN_SETTINGS.store(open_settings, Ordering::SeqCst);

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .build();

    // Hold the application so it stays alive even with no visible windows
    // (the tray icon and overlay monitor run in the background).
    // The guard is stored in HOLD_GUARD and released on graceful quit.
    app.connect_startup(|app| {
        let guard = app.hold();
        HOLD_GUARD.with(|cell| {
            *cell.borrow_mut() = Some(guard);
        });
    });

    // Handle graceful shutdown: release the hold guard so GTK can exit cleanly
    app.connect_shutdown(|_| {
        HOLD_GUARD.with(|cell| {
            let _ = cell.borrow_mut().take(); // Drop the guard → releases the hold
        });
    });

    // Register D-Bus action so the tray (or any remote caller) can open settings
    // via: busctl --user call io.github.voxtype.Voxtype /io/github/voxtype/Voxtype org.gtk.Actions.Activate ...
    let settings_action = gtk4::gio::SimpleAction::new("open-settings", None);
    {
        let app_ref = app.clone();
        settings_action.connect_activate(move |_, _| {
            settings::show_settings(&app_ref);
        });
    }
    app.add_action(&settings_action);

    app.connect_activate(on_activate);

    // Run with empty args — CLI args are already parsed by clap
    let exit_code = app.run_with_args::<&str>(&[]);
    if exit_code != gtk4::glib::ExitCode::SUCCESS {
        anyhow::bail!("GTK application exited with error");
    }

    Ok(())
}

/// Called when the application is activated (first launch or re-activation).
fn on_activate(app: &adw::Application) {
    // If a window already exists, just present it (single-instance)
    if let Some(window) = app.active_window() {
        window.present();
        return;
    }

    // Start system tray icon on a dedicated thread (StatusNotifierItem via D-Bus)
    // Guard against starting it twice (on_activate is called again on re-activation)
    if !TRAY_STARTED.swap(true, Ordering::SeqCst) {
        tray::start_tray(app);
    }

    // Open settings window if requested (via --settings flag or re-activation)
    if OPEN_SETTINGS.load(Ordering::SeqCst) {
        settings::show_settings(app);
        OPEN_SETTINGS.store(false, Ordering::SeqCst);
    }

    // Start monitoring daemon status in background (only once)
    // The overlay creates its own window and manages visibility based on state
    if !OVERLAY_STARTED.swap(true, Ordering::SeqCst) {
        let app_clone = app.clone();
        gtk4::glib::spawn_future_local(async move {
            if let Err(e) = overlay::start_status_monitor(app_clone).await {
                tracing::error!("Status monitor error: {}", e);
            }
        });
    }
}
