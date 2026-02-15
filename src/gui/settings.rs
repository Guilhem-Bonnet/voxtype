//! App Settings window — AdwPreferencesWindow for Voxtype configuration.
//!
//! # Architecture
//!
//! The settings window uses `AdwPreferencesWindow` with pages:
//! - **État** : Real-time daemon state (idle/recording/transcribing/stopped)
//! - **Audio** : Microphone, max duration, audio feedback
//! - **Transcription** : Model, language, engine
//! - **Raccourcis** : Hotkey, push-to-talk/toggle mode
//! - **Diagnostic** : Effective config, recent logs
//!
//! # Communication
//!
//! The settings window reads config via `voxtype config` CLI output
//! and writes changes by editing `~/.config/voxtype/config.toml` directly.
//! State monitoring uses `voxtype status --follow --format json --extended`.
//!
//! Changes require restarting the daemon service to take effect.

use gtk4::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

/// Resolve the user config file path via XDG_CONFIG_HOME (with $HOME fallback).
fn config_path() -> String {
    if let Some(dirs) = directories::ProjectDirs::from("", "", "voxtype") {
        dirs.config_dir().join("config.toml").to_string_lossy().to_string()
    } else {
        format!("{}/.config/voxtype/config.toml", std::env::var("HOME").unwrap_or_default())
    }
}

/// Helper: restart the voxtype systemd user service with visual feedback on the button.
fn restart_service_button(btn: &gtk4::Button) {
    btn.set_sensitive(false);
    btn.set_label("Redémarrage…");

    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    std::thread::spawn(move || {
        let ok = Command::new("systemctl")
            .args(["--user", "restart", "voxtype"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        let _ = tx.send(ok);
    });

    let btn_weak = btn.downgrade();
    fn poll_restart(
        rx: std::sync::mpsc::Receiver<bool>,
        btn_weak: gtk4::glib::WeakRef<gtk4::Button>,
    ) {
        match rx.try_recv() {
            Ok(ok) => {
                if let Some(b) = btn_weak.upgrade() {
                    if ok {
                        b.set_label("✓ Redémarré");
                    } else {
                        b.set_label("✗ Échec");
                        b.add_css_class("error");
                    }
                    b.set_sensitive(true);
                    let btn_w2 = b.downgrade();
                    gtk4::glib::timeout_add_local_once(
                        std::time::Duration::from_secs(3),
                        move || {
                            if let Some(b2) = btn_w2.upgrade() {
                                b2.set_label("Redémarrer le service");
                                b2.remove_css_class("error");
                            }
                        },
                    );
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                gtk4::glib::timeout_add_local_once(
                    std::time::Duration::from_millis(200),
                    move || poll_restart(rx, btn_weak),
                );
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                if let Some(b) = btn_weak.upgrade() {
                    b.set_label("✗ Échec");
                    b.add_css_class("error");
                    b.set_sensitive(true);
                }
            }
        }
    }
    gtk4::glib::timeout_add_local_once(
        std::time::Duration::from_millis(200),
        move || poll_restart(rx, btn_weak),
    );
}

/// Read the config.toml as a TOML table. Returns empty table on error.
fn load_config() -> toml::Table {
    std::fs::read_to_string(config_path())
        .ok()
        .and_then(|s| s.parse::<toml::Table>().ok())
        .unwrap_or_default()
}

/// Write a single key into a TOML section, preserving comments and file structure.
///
/// Strategy: line-based search & replace within the target `[section]`.
/// If the key exists (commented or not), replace/uncomment it.
/// If the key doesn't exist, append it after the section header.
/// If the section doesn't exist, append both section and key at the end.
fn save_config_value(section: &str, key: &str, value: &str) -> bool {
    let path = config_path();

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to read config: {}", e);
            return false;
        }
    };

    let final_content = edit_toml_content(&content, section, key, value);

    // Use atomic write via temp file to avoid race conditions
    let tmp_path = format!("{}.tmp", path);
    if let Err(e) = std::fs::write(&tmp_path, &final_content) {
        tracing::error!("Failed to write config: {}", e);
        let _ = std::fs::remove_file(&tmp_path);
        return false;
    }
    if let Err(e) = std::fs::rename(&tmp_path, &path) {
        tracing::error!("Failed to rename config: {}", e);
        let _ = std::fs::remove_file(&tmp_path);
        return false;
    }
    let toml_value = format_toml_value(value);
    tracing::info!("Config updated: [{}].{} = {}", section, key, toml_value);
    true
}

/// Pure function: edit TOML content to set `[section].key = value`.
///
/// Preserves comments, blank lines, and file structure.
/// Returns the new file content string.
fn edit_toml_content(content: &str, section: &str, key: &str, value: &str) -> String {
    let toml_value = format_toml_value(value);
    let new_line = format!("{} = {}", key, toml_value);

    let lines: Vec<&str> = content.lines().collect();
    let mut result: Vec<String> = Vec::with_capacity(lines.len() + 2);

    // Find section boundaries
    let section_header = format!("[{}]", section);

    // First pass: find if section and key exist
    let mut section_start: Option<usize> = None;
    let mut key_line: Option<usize> = None;
    let mut next_section_after: Option<usize> = None;
    {
        let mut in_sec = false;
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && !trimmed.starts_with("[[") {
                if in_sec && next_section_after.is_none() {
                    next_section_after = Some(i);
                }
                if trimmed == section_header
                    || trimmed.starts_with(&format!("[{}]", section))
                {
                    section_start = Some(i);
                    in_sec = true;
                } else {
                    in_sec = false;
                }
            } else if in_sec {
                // Check for the key (possibly commented out)
                let uncommented = trimmed.trim_start_matches('#').trim();
                if uncommented.starts_with(key)
                    && uncommented[key.len()..].trim_start().starts_with('=')
                {
                    key_line = Some(i);
                }
            }
        }
        if in_sec && next_section_after.is_none() {
            next_section_after = Some(lines.len());
        }
    }

    if let Some(kl) = key_line {
        // Key exists — replace the line
        for (i, line) in lines.iter().enumerate() {
            if i == kl {
                result.push(new_line.clone());
            } else {
                result.push(line.to_string());
            }
        }
    } else if let Some(ss) = section_start {
        // Section exists but key doesn't — insert after section header
        let insert_at = next_section_after.unwrap_or(lines.len());
        // Find last non-empty line in section to insert after it
        let mut insert_pos = ss + 1;
        for i in (ss + 1..insert_at).rev() {
            if !lines[i].trim().is_empty() {
                insert_pos = i + 1;
                break;
            }
        }
        for (i, line) in lines.iter().enumerate() {
            result.push(line.to_string());
            if i + 1 == insert_pos {
                result.push(new_line.clone());
            }
        }
    } else {
        // Section doesn't exist — append at end
        result.extend(lines.iter().map(|l| l.to_string()));
        result.push(String::new());
        result.push(section_header);
        result.push(new_line);
    }

    let new_content = result.join("\n");
    // Preserve trailing newline
    if content.ends_with('\n') && !new_content.ends_with('\n') {
        format!("{}\n", new_content)
    } else {
        new_content
    }
}

/// Format a value as a proper TOML value string.
fn format_toml_value(value: &str) -> String {
    if value == "true" || value == "false" {
        return value.to_string();
    }
    if value.parse::<i64>().is_ok() || value.parse::<f64>().is_ok() {
        return value.to_string();
    }
    // String value — quote it
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Build and present the App Settings window.
///
/// Creates an `AdwPreferencesWindow` with all configuration sections.
/// Uses the GLib main loop for async operations (status monitoring, log refresh).
pub fn show_settings(app: &adw::Application) {
    let window = adw::PreferencesWindow::builder()
        .application(app)
        .title("Voxtype — Réglages")
        .default_width(600)
        .default_height(700)
        .build();

    // Add preference pages
    window.add(&build_state_page());
    window.add(&build_audio_page());
    window.add(&build_transcription_page());
    window.add(&build_shortcuts_page());
    window.add(&build_diagnostic_page());

    // Start update check in background — uses a toast notification
    // instead of a banner to avoid breaking the AdwPreferencesWindow layout
    check_for_updates_toast(&window);

    window.present();
}

// ==========================================================================
// Story 4.2: Section État temps réel
// ==========================================================================

fn build_state_page() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("État")
        .icon_name("system-run-symbolic")
        .build();

    let group = adw::PreferencesGroup::builder()
        .title("État du daemon")
        .description("Supervision en temps réel de Voxtype")
        .build();

    // State indicator row
    let state_row = adw::ActionRow::builder()
        .title("État courant")
        .subtitle("Chargement…")
        .build();

    let state_icon = gtk4::Image::from_icon_name("emblem-synchronizing-symbolic");
    state_row.add_prefix(&state_icon);

    // Start/stop button for when daemon is stopped
    let start_button = gtk4::Button::builder()
        .label("Démarrer")
        .valign(gtk4::Align::Center)
        .css_classes(["suggested-action"])
        .visible(false)
        .build();

    start_button.connect_clicked(|_| {
        let _ = Command::new("systemctl")
            .args(["--user", "start", "voxtype"])
            .status();
    });

    state_row.add_suffix(&start_button);
    group.add(&state_row);

    // Extended info rows
    let model_row = adw::ActionRow::builder()
        .title("Modèle")
        .subtitle("—")
        .build();
    group.add(&model_row);

    let device_row = adw::ActionRow::builder()
        .title("Périphérique audio")
        .subtitle("—")
        .build();
    group.add(&device_row);

    let backend_row = adw::ActionRow::builder()
        .title("Backend")
        .subtitle("—")
        .build();
    group.add(&backend_row);

    page.add(&group);

    // Status monitoring timer (updates every 2s to avoid excessive fork+exec)
    // Uses WeakRef so the timer stops automatically when the page is destroyed
    let state_row_weak = state_row.downgrade();
    let state_icon_weak = state_icon.downgrade();
    let start_button_weak = start_button.downgrade();
    let model_row_weak = model_row.downgrade();
    let device_row_weak = device_row.downgrade();
    let backend_row_weak = backend_row.downgrade();

    gtk4::glib::timeout_add_local(std::time::Duration::from_millis(2000), move || {
        // Stop the timer if any widget has been destroyed (window closed)
        let Some(state_row_ref) = state_row_weak.upgrade() else {
            return gtk4::glib::ControlFlow::Break;
        };
        let Some(state_icon_ref) = state_icon_weak.upgrade() else {
            return gtk4::glib::ControlFlow::Break;
        };
        let Some(start_button_ref) = start_button_weak.upgrade() else {
            return gtk4::glib::ControlFlow::Break;
        };

        // Run status check in a thread to avoid blocking GTK main loop
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        std::thread::spawn(move || {
            if let Ok(output) = Command::new("voxtype")
                .args(["status", "--format", "json", "--extended"])
                .output()
            {
                if let Ok(s) = String::from_utf8(output.stdout) {
                    let _ = tx.send(s);
                }
            }
        });

        // Poll result after a short delay (non-blocking)
        let model_w = model_row_weak.clone();
        let device_w = device_row_weak.clone();
        let backend_w = backend_row_weak.clone();
        let sr = state_row_ref.clone();
        let si = state_icon_ref.clone();
        let sb = start_button_ref.clone();
        gtk4::glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
            if let Ok(json_str) = rx.try_recv() {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str.trim()) {
                    let class = json.get("class").and_then(|v| v.as_str()).unwrap_or("stopped");
                    let (label, icon, show_start) = match class {
                        "idle" => ("Prêt", "emblem-ok-symbolic", false),
                        "recording" => ("Enregistrement", "media-record-symbolic", false),
                        "transcribing" => ("Transcription…", "view-refresh-symbolic", false),
                        _ => ("Daemon inactif", "dialog-error-symbolic", true),
                    };
                    sr.set_subtitle(label);
                    si.set_icon_name(Some(icon));
                    sb.set_visible(show_start);
                    if let Some(m) = model_w.upgrade() {
                        if let Some(model) = json.get("model").and_then(|v| v.as_str()) {
                            m.set_subtitle(model);
                        }
                    }
                    if let Some(d) = device_w.upgrade() {
                        if let Some(device) = json.get("device").and_then(|v| v.as_str()) {
                            d.set_subtitle(device);
                        }
                    }
                    if let Some(b) = backend_w.upgrade() {
                        if let Some(backend) = json.get("backend").and_then(|v| v.as_str()) {
                            b.set_subtitle(backend);
                        }
                    }
                }
            }
        });

        gtk4::glib::ControlFlow::Continue
    });

    page
}

// ==========================================================================
// Story 4.3: Section Configuration (Audio + Transcription + Raccourcis)
// ==========================================================================

fn build_audio_page() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("Audio")
        .icon_name("audio-input-microphone-symbolic")
        .build();

    let config = load_config();
    let audio = config.get("audio").and_then(|v| v.as_table());

    let group = adw::PreferencesGroup::builder()
        .title("Capture audio")
        .description("Configuration du microphone et de l'enregistrement")
        .build();

    // Device selection — load from config
    let current_device = audio
        .and_then(|a| a.get("device"))
        .and_then(|v| v.as_str())
        .unwrap_or("default");

    let device_row = adw::EntryRow::builder()
        .title("Périphérique")
        .text(current_device)
        .build();
    device_row.connect_apply(|row| {
        let val = row.text().to_string();
        if !val.is_empty() {
            save_config_value("audio", "device", &val);
        }
    });
    // Also save on focus-out
    device_row.connect_entry_activated(|row| {
        let val = row.text().to_string();
        if !val.is_empty() {
            save_config_value("audio", "device", &val);
        }
    });
    group.add(&device_row);

    // Max duration — load from config
    let current_duration = audio
        .and_then(|a| a.get("max_duration_secs"))
        .and_then(|v| v.as_integer())
        .unwrap_or(60) as f64;

    let duration_row = adw::SpinRow::builder()
        .title("Durée max (secondes)")
        .adjustment(&gtk4::Adjustment::new(current_duration, 5.0, 300.0, 5.0, 10.0, 0.0))
        .build();
    duration_row.adjustment().connect_value_changed(|adj| {
        let val = adj.value() as i64;
        save_config_value("audio", "max_duration_secs", &val.to_string());
    });
    group.add(&duration_row);

    // Audio feedback
    let feedback_group = adw::PreferencesGroup::builder()
        .title("Feedback audio")
        .build();

    let feedback_enabled = audio
        .and_then(|a| a.get("feedback"))
        .and_then(|v| v.as_table())
        .and_then(|f| f.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let feedback_row = adw::SwitchRow::builder()
        .title("Sons de feedback")
        .subtitle("Jouer un son au début et à la fin de l'enregistrement")
        .active(feedback_enabled)
        .build();
    feedback_row.connect_active_notify(|row| {
        save_config_value("audio.feedback", "enabled", if row.is_active() { "true" } else { "false" });
    });
    feedback_group.add(&feedback_row);

    page.add(&group);
    page.add(&feedback_group);

    // Restart notice
    let notice = adw::PreferencesGroup::builder()
        .description("Les modifications nécessitent un redémarrage du service :\nsystemctl --user restart voxtype")
        .build();

    let restart_btn = gtk4::Button::builder()
        .label("Redémarrer le service")
        .css_classes(["suggested-action"])
        .halign(gtk4::Align::Center)
        .build();
    restart_btn.connect_clicked(|btn| {
        restart_service_button(btn);
    });
    notice.add(&restart_btn);
    page.add(&notice);

    page
}

fn build_transcription_page() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("Transcription")
        .icon_name("document-edit-symbolic")
        .build();

    let config = load_config();
    let whisper = config.get("whisper").and_then(|v| v.as_table());

    let group = adw::PreferencesGroup::builder()
        .title("Moteur de transcription")
        .build();

    // Model selection — load from config
    let models_list = [
        "tiny", "tiny.en", "base", "base.en", "small", "small.en",
        "medium", "medium.en", "large-v3", "large-v3-turbo",
    ];
    let current_model = whisper
        .and_then(|w| w.get("model"))
        .and_then(|v| v.as_str())
        .unwrap_or("base");

    let model_row = adw::ComboRow::builder()
        .title("Modèle")
        .build();

    let models = gtk4::StringList::new(&models_list);
    model_row.set_model(Some(&models));

    // Set selection to current model
    let current_idx = models_list.iter().position(|m| *m == current_model).unwrap_or(2);
    model_row.set_selected(current_idx as u32);

    model_row.connect_selected_notify(move |row| {
        let idx = row.selected() as usize;
        if idx < models_list.len() {
            save_config_value("whisper", "model", models_list[idx]);
        }
    });
    group.add(&model_row);

    // Language — load from config
    let current_lang = whisper
        .and_then(|w| w.get("language"))
        .map(|v| match v {
            toml::Value::String(s) => s.clone(),
            toml::Value::Array(a) => a.iter()
                .filter_map(|x| x.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            _ => "fr".to_string(),
        })
        .unwrap_or_else(|| "fr".to_string());

    let lang_row = adw::EntryRow::builder()
        .title("Langue (ex: fr, en, auto)")
        .text(&current_lang)
        .build();
    lang_row.connect_apply(|row| {
        let val = row.text().to_string().trim().to_string();
        if !val.is_empty() {
            save_config_value("whisper", "language", &val);
        }
    });
    group.add(&lang_row);

    // Backend selection
    let current_backend = whisper
        .and_then(|w| w.get("backend"))
        .and_then(|v| v.as_str())
        .unwrap_or("local");

    let backends = ["local", "remote"];
    let backend_row = adw::ComboRow::builder()
        .title("Backend")
        .build();
    backend_row.set_model(Some(&gtk4::StringList::new(&backends)));
    let backend_idx = backends.iter().position(|b| *b == current_backend).unwrap_or(0);
    backend_row.set_selected(backend_idx as u32);
    backend_row.connect_selected_notify(move |row| {
        let idx = row.selected() as usize;
        if idx < backends.len() {
            save_config_value("whisper", "backend", backends[idx]);
        }
    });
    group.add(&backend_row);

    page.add(&group);

    // Restart notice
    let notice = adw::PreferencesGroup::builder()
        .description("Les modifications nécessitent un redémarrage du service.")
        .build();
    let restart_btn = gtk4::Button::builder()
        .label("Redémarrer le service")
        .css_classes(["suggested-action"])
        .halign(gtk4::Align::Center)
        .build();
    restart_btn.connect_clicked(|btn| {
        restart_service_button(btn);
    });
    notice.add(&restart_btn);
    page.add(&notice);

    page
}

fn build_shortcuts_page() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("Raccourcis")
        .icon_name("preferences-desktop-keyboard-shortcuts-symbolic")
        .build();

    let config = load_config();
    let hotkey = config.get("hotkey").and_then(|v| v.as_table());

    let group = adw::PreferencesGroup::builder()
        .title("Touche de raccourci")
        .build();

    // Hotkey display — load from config
    let current_key = hotkey
        .and_then(|h| h.get("key"))
        .and_then(|v| v.as_str())
        .unwrap_or("F9");

    let hotkey_row = adw::EntryRow::builder()
        .title("Touche d'enregistrement")
        .text(current_key)
        .build();
    hotkey_row.connect_apply(|row| {
        let val = row.text().to_string().trim().to_string();
        if !val.is_empty() {
            save_config_value("hotkey", "key", &val);
        }
    });
    group.add(&hotkey_row);

    // Hotkey enabled
    let hotkey_enabled = hotkey
        .and_then(|h| h.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let enabled_row = adw::SwitchRow::builder()
        .title("Hotkey intégré actif")
        .subtitle("Désactiver si vous utilisez un keybinding GNOME/Sway")
        .active(hotkey_enabled)
        .build();
    enabled_row.connect_active_notify(|row| {
        save_config_value("hotkey", "enabled", if row.is_active() { "true" } else { "false" });
    });
    group.add(&enabled_row);

    // Mode switch — load from config
    let current_mode = hotkey
        .and_then(|h| h.get("mode"))
        .and_then(|v| v.as_str())
        .unwrap_or("push_to_talk");

    let mode_row = adw::SwitchRow::builder()
        .title("Mode toggle")
        .subtitle("Actif : appui = start/stop. Inactif : maintenir = enregistrer (push-to-talk)")
        .active(current_mode == "toggle")
        .build();
    mode_row.connect_active_notify(|row| {
        let mode = if row.is_active() { "toggle" } else { "push_to_talk" };
        save_config_value("hotkey", "mode", mode);
    });
    group.add(&mode_row);

    page.add(&group);

    // GNOME keybinding info
    let gnome_group = adw::PreferencesGroup::builder()
        .title("Keybinding GNOME")
        .description("Si le hotkey intégré est désactivé, configurez un raccourci GNOME :\nParamètres > Clavier > Raccourcis personnalisés\nCommande : voxtype record toggle")
        .build();
    page.add(&gnome_group);

    page
}

// ==========================================================================
// Story 4.4: Section Diagnostic
// ==========================================================================

fn build_diagnostic_page() -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title("Diagnostic")
        .icon_name("utilities-terminal-symbolic")
        .build();

    // Effective config section
    let config_group = adw::PreferencesGroup::builder()
        .title("Configuration effective")
        .build();

    let config_text = Rc::new(RefCell::new(String::new()));
    let config_view = gtk4::TextView::builder()
        .editable(false)
        .monospace(true)
        .wrap_mode(gtk4::WrapMode::Word)
        .top_margin(8)
        .bottom_margin(8)
        .left_margin(8)
        .right_margin(8)
        .build();

    let config_scroll = gtk4::ScrolledWindow::builder()
        .min_content_height(200)
        .child(&config_view)
        .build();

    // Refresh button
    let config_refresh_button = gtk4::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Rafraîchir")
        .valign(gtk4::Align::Center)
        .build();

    let config_view_clone = config_view.clone();
    let config_text_clone = config_text.clone();
    config_refresh_button.connect_clicked(move |_| {
        if let Ok(output) = Command::new("voxtype").args(["config"]).output() {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            config_view_clone.buffer().set_text(&text);
            *config_text_clone.borrow_mut() = text;
        }
    });

    let config_row = adw::ActionRow::builder()
        .title("Configuration")
        .activatable_widget(&config_refresh_button)
        .build();
    config_row.add_suffix(&config_refresh_button);
    config_group.add(&config_row);
    config_group.add(&config_scroll);

    // Initial load
    if let Ok(output) = Command::new("voxtype").args(["config"]).output() {
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        config_view.buffer().set_text(&text);
        *config_text.borrow_mut() = text;
    }

    page.add(&config_group);

    // Logs section
    let logs_group = adw::PreferencesGroup::builder()
        .title("Logs récents")
        .build();

    let logs_view = gtk4::TextView::builder()
        .editable(false)
        .monospace(true)
        .wrap_mode(gtk4::WrapMode::Word)
        .top_margin(8)
        .bottom_margin(8)
        .left_margin(8)
        .right_margin(8)
        .build();

    let logs_scroll = gtk4::ScrolledWindow::builder()
        .min_content_height(250)
        .child(&logs_view)
        .build();

    // Time filter combo
    let time_filter = gtk4::DropDown::from_strings(&["5 min", "1h", "Aujourd'hui"]);
    time_filter.set_selected(0);
    time_filter.set_valign(gtk4::Align::Center);

    // Refresh logs button
    let logs_refresh = gtk4::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Rafraîchir les logs")
        .valign(gtk4::Align::Center)
        .build();

    let logs_view_clone = logs_view.clone();
    let time_filter_clone = time_filter.clone();
    logs_refresh.connect_clicked(move |_| {
        let since = match time_filter_clone.selected() {
            0 => "5m ago",
            1 => "1h ago",
            _ => "today",
        };
        if let Ok(output) = Command::new("journalctl")
            .args(["--user", "-u", "voxtype", "--no-pager", "-n", "50", "--since", since])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            logs_view_clone.buffer().set_text(&text);
            // Auto-scroll to bottom
            let iter = logs_view_clone.buffer().end_iter();
            logs_view_clone.buffer().place_cursor(&iter);
        }
    });

    let logs_row = adw::ActionRow::builder()
        .title("Journaux")
        .build();
    logs_row.add_suffix(&time_filter);
    logs_row.add_suffix(&logs_refresh);
    logs_group.add(&logs_row);
    logs_group.add(&logs_scroll);

    // Initial log load
    if let Ok(output) = Command::new("journalctl")
        .args(["--user", "-u", "voxtype", "--no-pager", "-n", "50", "--since", "5m ago"])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        logs_view.buffer().set_text(&text);
    }

    // Error pattern detection
    let logs_hint = adw::ActionRow::builder()
        .title("💡 Aide contextuelle")
        .subtitle("Aucune erreur connue détectée")
        .build();
    logs_hint.add_css_class("dim-label");
    logs_group.add(&logs_hint);

    page.add(&logs_group);
    page
}

// ==========================================================================
// Story 6.1/6.2: Notification de mise à jour (refactored)
// ==========================================================================

/// Check for updates and show a toast in the AdwPreferencesWindow.
///
/// Uses AdwToast which is natively supported by AdwPreferencesWindow,
/// avoiding the need to manipulate the internal widget tree.
fn check_for_updates_toast(window: &adw::PreferencesWindow) {
    use super::update;

    // Respect config: if check disabled, do nothing
    if !update::is_check_enabled() {
        return;
    }

    let (tx, rx) = std::sync::mpsc::channel::<Option<(String, String)>>();

    std::thread::spawn(move || {
        let result = update::check_github_release().map(|info| {
            (info.version, info.changelog_url)
        });
        let _ = tx.send(result);
    });

    let window_weak = window.downgrade();
    gtk4::glib::timeout_add_local_once(std::time::Duration::from_millis(100), move || {
        fn poll_result(
            rx: std::sync::mpsc::Receiver<Option<(String, String)>>,
            window_weak: gtk4::glib::WeakRef<adw::PreferencesWindow>,
        ) {
            use super::update;

            match rx.try_recv() {
                Ok(Some((version, url))) => {
                    if update::is_dismissed(&version) {
                        return;
                    }
                    if let Some(window) = window_weak.upgrade() {
                        let toast = adw::Toast::builder()
                            .title(format!("Voxtype {version} disponible"))
                            .button_label("Changelog")
                            .timeout(0) // persist until dismissed
                            .build();

                        let dismiss_version = version.clone();
                        toast.connect_button_clicked(move |_| {
                            let _ = gtk4::gio::AppInfo::launch_default_for_uri(
                                &url,
                                None::<&gtk4::gio::AppLaunchContext>,
                            );
                        });
                        toast.connect_dismissed(move |_| {
                            update::dismiss_version(&dismiss_version);
                        });

                        window.add_toast(toast);
                    }
                }
                Ok(None) => { /* No update available */ }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    gtk4::glib::timeout_add_local_once(
                        std::time::Duration::from_millis(500),
                        move || poll_result(rx, window_weak),
                    );
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {}
            }
        }
        poll_result(rx, window_weak);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // =================================================================
    // format_toml_value
    // =================================================================

    #[test]
    fn format_toml_value_bool() {
        assert_eq!(format_toml_value("true"), "true");
        assert_eq!(format_toml_value("false"), "false");
    }

    #[test]
    fn format_toml_value_integer() {
        assert_eq!(format_toml_value("42"), "42");
        assert_eq!(format_toml_value("0"), "0");
        assert_eq!(format_toml_value("-1"), "-1");
    }

    #[test]
    fn format_toml_value_float() {
        assert_eq!(format_toml_value("3.14"), "3.14");
    }

    #[test]
    fn format_toml_value_string() {
        assert_eq!(format_toml_value("hello"), "\"hello\"");
        assert_eq!(format_toml_value("with spaces"), "\"with spaces\"");
    }

    #[test]
    fn format_toml_value_string_escapes() {
        assert_eq!(format_toml_value(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(format_toml_value(r"back\slash"), r#""back\\slash""#);
    }

    // =================================================================
    // edit_toml_content — replace existing key
    // =================================================================

    #[test]
    fn edit_toml_replace_existing_key() {
        let input = "[audio]\ndevice = \"default\"\nmax_duration_secs = 30\n";
        let result = edit_toml_content(input, "audio", "device", "hw:1");
        assert!(result.contains("device = \"hw:1\""));
        assert!(result.contains("max_duration_secs = 30"));
    }

    #[test]
    fn edit_toml_replace_bool() {
        let input = "[hotkey]\nenabled = true\n";
        let result = edit_toml_content(input, "hotkey", "enabled", "false");
        assert!(result.contains("enabled = false"));
    }

    // =================================================================
    // edit_toml_content — uncomment commented key
    // =================================================================

    #[test]
    fn edit_toml_uncomment_key() {
        let input = "[whisper]\n# model = \"base\"\n";
        let result = edit_toml_content(input, "whisper", "model", "large-v3");
        assert!(result.contains("model = \"large-v3\""));
        assert!(!result.contains('#'));
    }

    // =================================================================
    // edit_toml_content — add key to existing section
    // =================================================================

    #[test]
    fn edit_toml_add_key_to_section() {
        let input = "[audio]\ndevice = \"default\"\n";
        let result = edit_toml_content(input, "audio", "max_duration_secs", "60");
        assert!(result.contains("max_duration_secs = 60"));
        assert!(result.contains("device = \"default\""));
    }

    // =================================================================
    // edit_toml_content — create new section
    // =================================================================

    #[test]
    fn edit_toml_create_new_section() {
        let input = "[audio]\ndevice = \"default\"\n";
        let result = edit_toml_content(input, "hotkey", "enabled", "true");
        assert!(result.contains("[hotkey]"));
        assert!(result.contains("enabled = true"));
        // Original content preserved
        assert!(result.contains("[audio]"));
        assert!(result.contains("device = \"default\""));
    }

    #[test]
    fn edit_toml_create_section_on_empty() {
        let result = edit_toml_content("", "whisper", "model", "base");
        assert!(result.contains("[whisper]"));
        assert!(result.contains("model = \"base\""));
    }

    // =================================================================
    // edit_toml_content — preserves trailing newline
    // =================================================================

    #[test]
    fn edit_toml_preserves_trailing_newline() {
        let input = "[audio]\ndevice = \"default\"\n";
        let result = edit_toml_content(input, "audio", "device", "hw:1");
        assert!(result.ends_with('\n'));
    }

    // =================================================================
    // edit_toml_content — multi-section file
    // =================================================================

    #[test]
    fn edit_toml_multi_section() {
        let input = "\
[audio]
device = \"default\"

[whisper]
model = \"base\"

[hotkey]
enabled = true
";
        let result = edit_toml_content(input, "whisper", "model", "large-v3");
        assert!(result.contains("model = \"large-v3\""));
        // Other sections untouched
        assert!(result.contains("device = \"default\""));
        assert!(result.contains("enabled = true"));
    }

    // =================================================================
    // edit_toml_content — dotted section (e.g. [audio.feedback])
    // =================================================================

    #[test]
    fn edit_toml_dotted_section() {
        let input = "[audio.feedback]\nenabled = true\n";
        let result = edit_toml_content(input, "audio.feedback", "enabled", "false");
        assert!(result.contains("enabled = false"));
    }
}
