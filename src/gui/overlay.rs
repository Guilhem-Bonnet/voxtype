//! Recording overlay window — the floating waveform display.
//!
//! # Architecture
//!
//! The overlay is a small, floating, undecorated GTK4 window (~280×56px) that:
//! - Appears when the daemon enters `recording` state
//! - Shows animated waveform bars driven by the `level` field from JSON status
//! - Shows a recording timer (mm:ss)
//! - Has a cancel button (✕) and ESC key binding
//! - Disappears when the daemon leaves recording/transcribing
//!
//! The overlay is a `UTILITY` type window (always-on-top, no taskbar entry)
//! positioned at bottom-center of the screen.
//!
//! # Communication
//!
//! The overlay communicates with the daemon via:
//! - `voxtype status --follow --format json` (reads state + level)
//! - `voxtype record cancel` (cancel action)
//!
//! No new IPC mechanism is introduced.

use anyhow::Result;
use gtk4::prelude::*;
use gtk4::glib;
use libadwaita as adw;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

/// Number of bars in the waveform visualization
const WAVEFORM_BARS: usize = 24;

/// Overlay window dimensions
const OVERLAY_WIDTH: i32 = 280;
const OVERLAY_HEIGHT: i32 = 56;

/// State tracked by the overlay
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayState {
    Hidden,
    Recording,
    Transcribing,
    Error(String),
}

/// Shared state for the overlay, updated by the status monitor
pub struct OverlayData {
    pub state: OverlayState,
    pub level: f32,
    pub recording_start: Option<Instant>,
    pub levels_history: Vec<f32>,
}

impl Default for OverlayData {
    fn default() -> Self {
        Self {
            state: OverlayState::Hidden,
            level: 0.0,
            recording_start: None,
            levels_history: vec![0.0; WAVEFORM_BARS],
        }
    }
}

/// The overlay widget bundle
pub struct Overlay {
    pub window: gtk4::Window,
    pub waveform: gtk4::DrawingArea,
    pub timer_label: gtk4::Label,
    pub status_label: gtk4::Label,
    pub data: Rc<RefCell<OverlayData>>,
}

impl Overlay {
    /// Create the overlay window (initially hidden).
    ///
    /// The window is configured as a floating utility window that:
    /// - Stays on top of other windows
    /// - Does not appear in the taskbar
    /// - Does not steal focus from the active window
    /// - Has rounded corners and semi-transparent background via CSS
    pub fn new(_app: &adw::Application) -> Self {
        let window = gtk4::Window::builder()
            .title("Voxtype Recording")
            .default_width(OVERLAY_WIDTH)
            .default_height(OVERLAY_HEIGHT)
            .resizable(false)
            .decorated(false)
            .build();

        // Utility window — no taskbar entry, best-effort always-on-top
        window.set_focus_on_click(false);

        // CSS for the overlay styling
        let css_provider = gtk4::CssProvider::new();
        css_provider.load_from_data(OVERLAY_CSS);
        gtk4::style_context_add_provider_for_display(
            &gtk4::gdk::Display::default().expect("display"),
            &css_provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        // Apply overlay CSS class to the window
        window.add_css_class("overlay-recording");

        let data = Rc::new(RefCell::new(OverlayData::default()));

        // Build content
        let hbox = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .margin_start(12)
            .margin_end(12)
            .margin_top(8)
            .margin_bottom(8)
            .build();

        // Waveform drawing area
        let waveform = gtk4::DrawingArea::builder()
            .width_request(160)
            .height_request(40)
            .build();

        // Set up waveform drawing callback
        let data_clone = data.clone();
        waveform.set_draw_func(move |_area, cr, width, height| {
            draw_waveform(cr, width, height, &data_clone);
        });

        // Timer label
        let timer_label = gtk4::Label::builder()
            .label("00:00")
            .css_classes(["timer-label"])
            .build();

        // Status label (for transcribing/error messages)
        let status_label = gtk4::Label::builder()
            .label("")
            .css_classes(["status-label"])
            .visible(false)
            .build();

        // Cancel button
        let cancel_btn = gtk4::Button::builder()
            .label("✕")
            .css_classes(["cancel-btn"])
            .tooltip_text("Annuler (ESC)")
            .build();

        let window_weak = window.downgrade();
        cancel_btn.connect_clicked(move |_| {
            let _ = std::process::Command::new("voxtype")
                .args(["record", "cancel"])
                .spawn();
            if let Some(w) = window_weak.upgrade() {
                w.set_visible(false);
            }
        });

        hbox.append(&waveform);

        let vbox = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(2)
            .valign(gtk4::Align::Center)
            .build();
        vbox.append(&timer_label);
        vbox.append(&status_label);
        hbox.append(&vbox);
        hbox.append(&cancel_btn);

        window.set_child(Some(&hbox));

        // ESC key to cancel recording
        let esc_controller = gtk4::EventControllerKey::new();
        let window_weak2 = window.downgrade();
        esc_controller.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                let _ = std::process::Command::new("voxtype")
                    .args(["record", "cancel"])
                    .spawn();
                if let Some(w) = window_weak2.upgrade() {
                    w.set_visible(false);
                }
                return gtk4::glib::Propagation::Stop;
            }
            gtk4::glib::Propagation::Proceed
        });
        window.add_controller(esc_controller);

        // Start hidden
        window.set_visible(false);

        // Set up periodic refresh timer for waveform and timer
        let waveform_clone = waveform.clone();
        let timer_label_clone = timer_label.clone();
        let data_refresh = data.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            let d = data_refresh.borrow();
            if d.state == OverlayState::Recording {
                // Trigger waveform redraw
                waveform_clone.queue_draw();

                // Update timer
                if let Some(start) = d.recording_start {
                    let elapsed = start.elapsed();
                    let secs = elapsed.as_secs();
                    timer_label_clone.set_label(&format!("{:02}:{:02}", secs / 60, secs % 60));
                }
            }
            glib::ControlFlow::Continue
        });

        Self {
            window,
            waveform,
            timer_label,
            status_label,
            data,
        }
    }

    /// Show the overlay for recording
    pub fn show_recording(&self) {
        {
            let mut d = self.data.borrow_mut();
            d.state = OverlayState::Recording;
            d.recording_start = Some(Instant::now());
            d.levels_history = vec![0.0; WAVEFORM_BARS];
        }
        self.status_label.set_visible(false);
        self.timer_label.set_visible(true);
        self.timer_label.set_label("00:00");
        self.window.remove_css_class("transcribing");
        self.window.remove_css_class("error");
        self.window.add_css_class("recording");
        self.window.set_visible(true);
    }

    /// Transition to transcribing state (freeze waveform, show spinner text)
    pub fn show_transcribing(&self) {
        {
            let mut d = self.data.borrow_mut();
            d.state = OverlayState::Transcribing;
        }
        self.window.remove_css_class("recording");
        self.window.add_css_class("transcribing");
        self.status_label.set_label("Transcription...");
        self.status_label.set_visible(true);
    }

    /// Show an error message
    pub fn show_error(&self, message: &str) {
        {
            let mut d = self.data.borrow_mut();
            d.state = OverlayState::Error(message.to_string());
        }
        self.window.remove_css_class("recording");
        self.window.remove_css_class("transcribing");
        self.window.add_css_class("error");
        self.status_label.set_label(message);
        self.status_label.set_visible(true);
        self.timer_label.set_visible(false);

        // Auto-hide after 5 seconds
        let window_weak = self.window.downgrade();
        glib::timeout_add_local_once(std::time::Duration::from_secs(5), move || {
            if let Some(w) = window_weak.upgrade() {
                w.set_visible(false);
            }
        });
    }

    /// Hide the overlay (fade-out effect via CSS transition)
    pub fn hide(&self) {
        {
            let mut d = self.data.borrow_mut();
            d.state = OverlayState::Hidden;
            d.recording_start = None;
        }
        self.window.set_visible(false);
        self.window.remove_css_class("recording");
        self.window.remove_css_class("transcribing");
        self.window.remove_css_class("error");
    }

    /// Update the audio level (called from status monitor)
    pub fn update_level(&self, level: f32) {
        let mut d = self.data.borrow_mut();
        d.level = level;
        d.levels_history.push(level);
        if d.levels_history.len() > WAVEFORM_BARS {
            d.levels_history.remove(0);
        }
    }
}

/// Draw the waveform bars using Cairo
fn draw_waveform(cr: &gtk4::cairo::Context, width: i32, height: i32, data: &Rc<RefCell<OverlayData>>) {
    let d = data.borrow();
    let w = width as f64;
    let h = height as f64;
    let bar_count = d.levels_history.len();
    if bar_count == 0 {
        return;
    }

    let bar_width = (w / bar_count as f64) * 0.7;
    let bar_gap = (w / bar_count as f64) * 0.3;
    let max_bar_height = h * 0.85;
    let center_y = h / 2.0;

    // Accent color based on state
    let (r, g, b) = match d.state {
        OverlayState::Recording => (0.35, 0.65, 1.0),    // Blue accent
        OverlayState::Transcribing => (0.6, 0.6, 0.6),   // Grey (frozen)
        _ => (0.4, 0.4, 0.4),
    };

    for (i, &level) in d.levels_history.iter().enumerate() {
        let bar_h = (level * max_bar_height as f32).max(2.0) as f64;
        let x = i as f64 * (bar_width + bar_gap) + bar_gap / 2.0;
        let y = center_y - bar_h / 2.0;

        // Rounded rectangle for each bar
        let radius = (bar_width / 2.0).min(3.0);
        cr.new_sub_path();
        cr.arc(x + bar_width - radius, y + radius, radius, -std::f64::consts::FRAC_PI_2, 0.0);
        cr.arc(x + bar_width - radius, y + bar_h - radius, radius, 0.0, std::f64::consts::FRAC_PI_2);
        cr.arc(x + radius, y + bar_h - radius, radius, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
        cr.arc(x + radius, y + radius, radius, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
        cr.close_path();

        // Alpha varies with level for depth effect
        let alpha = 0.5 + (level as f64 * 0.5);
        cr.set_source_rgba(r, g, b, alpha);
        let _ = cr.fill();
    }
}

/// Start monitoring daemon status via `voxtype status --follow --format json`.
///
/// This spawns the status command and parses each JSON line to update
/// the overlay visibility and waveform.
pub async fn start_status_monitor(app: adw::Application) -> Result<()> {
    use std::process::{Command, Stdio};
    use std::io::BufRead;

    let overlay = Rc::new(Overlay::new(&app));

    // Spawn `voxtype status --follow --format json` in a background thread
    // and send parsed lines back to the GLib main loop
    let (tx, rx) = std::sync::mpsc::channel::<String>();

    std::thread::Builder::new()
        .name("overlay-status".into())
        .spawn(move || {
            loop {
                let child = Command::new("voxtype")
                    .args(["status", "--follow", "--format", "json"])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn();

                let child = match child {
                    Ok(c) => c,
                    Err(_) => {
                        std::thread::sleep(std::time::Duration::from_secs(5));
                        continue;
                    }
                };

                let stdout = match child.stdout {
                    Some(s) => s,
                    None => {
                        std::thread::sleep(std::time::Duration::from_secs(5));
                        continue;
                    }
                };

                let reader = std::io::BufReader::new(stdout);
                for line in reader.lines() {
                    match line {
                        Ok(l) => {
                            if tx.send(l).is_err() {
                                return; // Receiver dropped, app shutting down
                            }
                        }
                        Err(_) => break, // Process died, restart loop
                    }
                }

                // Process ended — wait and retry
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        })?;

    // Poll the channel from the GLib main loop
    let overlay_ref = overlay.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        while let Ok(line) = rx.try_recv() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                let class = json.get("class").and_then(|v| v.as_str()).unwrap_or("");
                let level = json.get("level").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;

                match class {
                    "recording" => {
                        overlay_ref.update_level(level);
                        let d = overlay_ref.data.borrow();
                        if d.state != OverlayState::Recording {
                            drop(d);
                            overlay_ref.show_recording();
                        }
                    }
                    "transcribing" => {
                        let d = overlay_ref.data.borrow();
                        if d.state != OverlayState::Transcribing {
                            drop(d);
                            overlay_ref.show_transcribing();
                        }
                    }
                    "idle" => {
                        let d = overlay_ref.data.borrow();
                        let was_active = d.state == OverlayState::Recording
                            || d.state == OverlayState::Transcribing;
                        drop(d);
                        if was_active {
                            overlay_ref.hide();
                        }
                    }
                    "stopped" => {
                        let d = overlay_ref.data.borrow();
                        let was_active = d.state == OverlayState::Recording
                            || d.state == OverlayState::Transcribing;
                        drop(d);
                        if was_active {
                            overlay_ref.show_error("Daemon inactif — systemctl --user start voxtype");
                        }
                    }
                    _ => {}
                }
            }
        }
        glib::ControlFlow::Continue
    });

    // Keep the future alive (it's spawned via spawn_future_local)
    // The actual work is done in the thread + timer above
    loop {
        glib::timeout_future(std::time::Duration::from_secs(3600)).await;
    }
}

/// CSS styling for the overlay
const OVERLAY_CSS: &str = r#"
/* ===== Overlay Window ===== */
.overlay-recording {
    background: alpha(@window_bg_color, 0.95);
    border-radius: 24px;
    box-shadow: 0 4px 16px alpha(black, 0.25), 0 1px 4px alpha(black, 0.15);
    padding: 12px;
    /* Story 5.2: Fade-in/out transition */
    transition: opacity 150ms ease-in, background 200ms ease;
}

/* ===== Timer Label ===== */
.timer-label {
    font-family: monospace;
    font-size: 16px;
    font-weight: bold;
    min-width: 48px;
    color: @window_fg_color;
    transition: opacity 200ms ease, color 200ms ease;
}

/* ===== Status Label ===== */
.status-label {
    font-size: 11px;
    opacity: 0.7;
    transition: opacity 200ms ease, color 200ms ease;
}

/* ===== Cancel Button ===== */
.cancel-btn {
    min-width: 32px;
    min-height: 32px;
    border-radius: 16px;
    font-size: 14px;
    padding: 4px;
    background: transparent;
    border: none;
    transition: background 150ms ease;
}

.cancel-btn:hover {
    background: alpha(@destructive_color, 0.2);
}

/* ===== State-Specific Styles ===== */

/* Recording state: accent-colored timer, pulsing waveform */
.recording .timer-label {
    color: @accent_color;
}

/* Transcribing state: dimmed timer, frozen waveform with fade */
.transcribing .timer-label {
    opacity: 0.5;
}

.transcribing .overlay-recording {
    background: alpha(@window_bg_color, 0.92);
}

/* Error state: red status label */
.error .status-label {
    color: @error_color;
    font-weight: bold;
}

.error .overlay-recording {
    box-shadow: 0 4px 16px alpha(@error_color, 0.15), 0 1px 4px alpha(black, 0.15);
}
"#;
