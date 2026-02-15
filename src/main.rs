//! Voxtype - Push-to-talk voice-to-text for Linux
//!
//! Run with `voxtype` or `voxtype daemon` to start the daemon.
//! Use `voxtype setup` to check dependencies and download models.
//! Use `voxtype transcribe <file>` to transcribe an audio file.

use clap::Parser;
use std::path::PathBuf;
use std::process::Command;
use tracing_subscriber::EnvFilter;
use voxtype::{config, cpu, daemon, setup, transcribe, Cli, Commands, RecordAction, SetupAction};

/// Parse a comma-separated list of driver names into OutputDriver vec
fn parse_driver_order(s: &str) -> Result<Vec<config::OutputDriver>, String> {
    s.split(',')
        .map(|d| d.trim().parse::<config::OutputDriver>())
        .collect()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Install SIGILL handler early to catch illegal instruction crashes
    // and provide a helpful error message instead of core dumping
    cpu::install_sigill_handler();

    // Reset SIGPIPE to default behavior (terminate silently) to avoid panics
    // when output is piped through commands like `head` that close the pipe early
    reset_sigpipe();

    let cli = Cli::parse();

    // Check if this is the worker command (needs stderr-only logging)
    let is_worker = matches!(cli.command, Some(Commands::TranscribeWorker { .. }));

    // Initialize logging
    let log_level = if cli.quiet {
        "error"
    } else {
        match cli.verbose {
            0 => "info",
            1 => "debug",
            _ => "trace",
        }
    };

    if is_worker {
        // Worker uses stderr for logging (stdout is reserved for IPC protocol)
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| EnvFilter::new(format!("voxtype={},warn", log_level))),
            )
            .with_target(false)
            .with_writer(std::io::stderr)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| EnvFilter::new(format!("voxtype={},warn", log_level))),
            )
            .with_target(false)
            .init();
    }

    // Load configuration
    let config_path = cli.config.clone().or_else(config::Config::default_path);
    let mut config = config::load_config(cli.config.as_deref())?;

    // Apply CLI overrides
    if cli.clipboard {
        config.output.mode = config::OutputMode::Clipboard;
    }
    if cli.paste {
        config.output.mode = config::OutputMode::Paste;
    }
    if let Some(model) = cli.model {
        if setup::model::is_valid_model(&model) {
            config.whisper.model = model;
        } else {
            let default_model = &config.whisper.model;
            tracing::warn!(
                "Unknown model '{}', using default model '{}'",
                model,
                default_model
            );
            // Send desktop notification
            let _ = Command::new("notify-send")
                .args([
                    "--app-name=Voxtype",
                    "--expire-time=5000",
                    "Voxtype: Invalid Model",
                    &format!("Unknown model '{}', using '{}'", model, default_model),
                ])
                .spawn();
        }
    }
    if let Some(engine) = cli.engine {
        match engine.to_lowercase().as_str() {
            "whisper" => config.engine = config::TranscriptionEngine::Whisper,
            "parakeet" => config.engine = config::TranscriptionEngine::Parakeet,
            _ => {
                eprintln!("Error: Invalid engine '{}'. Valid options: whisper, parakeet", engine);
                std::process::exit(1);
            }
        }
    }
    if let Some(hotkey) = cli.hotkey {
        config.hotkey.key = hotkey;
    }
    if cli.toggle {
        config.hotkey.mode = config::ActivationMode::Toggle;
    }
    if let Some(delay) = cli.pre_type_delay {
        config.output.pre_type_delay_ms = delay;
    }
    if let Some(delay) = cli.wtype_delay {
        tracing::warn!("--wtype-delay is deprecated, use --pre-type-delay instead");
        config.output.pre_type_delay_ms = delay;
    }
    if cli.no_whisper_context_optimization {
        config.whisper.context_window_optimization = false;
    }
    if let Some(prompt) = cli.initial_prompt {
        config.whisper.initial_prompt = Some(prompt);
    }
    if let Some(ref driver_str) = cli.driver {
        match parse_driver_order(driver_str) {
            Ok(drivers) => {
                config.output.driver_order = Some(drivers);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Run the appropriate command
    match cli.command.unwrap_or(Commands::Daemon) {
        Commands::Daemon => {
            let mut daemon = daemon::Daemon::new(config, config_path);
            daemon.run().await?;
        }

        Commands::Transcribe { file } => {
            transcribe_file(&config, &file)?;
        }

        Commands::TranscribeWorker {
            model,
            language,
            translate,
            threads,
        } => {
            // Internal command: run transcription worker process
            // This is spawned by the daemon when gpu_isolation is enabled
            // Use command-line overrides if provided, otherwise use config
            let mut whisper_config = config.whisper.clone();
            if let Some(m) = model {
                whisper_config.model = m;
            }
            if let Some(l) = language {
                // Parse comma-separated language string back to LanguageConfig
                whisper_config.language = config::LanguageConfig::from_comma_separated(&l);
            }
            if translate {
                whisper_config.translate = true;
            }
            if let Some(t) = threads {
                whisper_config.threads = Some(t);
            }
            transcribe::worker::run_worker(&whisper_config)?;
        }

        Commands::Setup {
            action,
            download,
            model,
            quiet,
            no_post_install,
        } => {
            match action {
                Some(SetupAction::Check) => {
                    setup::run_checks(&config).await?;
                }
                Some(SetupAction::Systemd { uninstall, status }) => {
                    if status {
                        setup::systemd::status().await?;
                    } else if uninstall {
                        setup::systemd::uninstall().await?;
                    } else {
                        setup::systemd::install().await?;
                    }
                }
                Some(SetupAction::Waybar {
                    json,
                    css,
                    install,
                    uninstall,
                }) => {
                    if install {
                        setup::waybar::install()?;
                    } else if uninstall {
                        setup::waybar::uninstall()?;
                    } else if json {
                        println!("{}", setup::waybar::get_json_config());
                    } else if css {
                        println!("{}", setup::waybar::get_css_config());
                    } else {
                        setup::waybar::print_config();
                    }
                }
                Some(SetupAction::Dms {
                    install,
                    uninstall,
                    qml,
                }) => {
                    if install {
                        setup::dms::install()?;
                    } else if uninstall {
                        setup::dms::uninstall()?;
                    } else if qml {
                        println!("{}", setup::dms::get_qml_config());
                    } else {
                        setup::dms::print_config();
                    }
                }
                Some(SetupAction::Model { list, set, restart }) => {
                    if list {
                        setup::model::list_installed();
                    } else if let Some(model_name) = set {
                        setup::model::set_model(&model_name, restart).await?;
                    } else {
                        setup::model::interactive_select().await?;
                    }
                }
                Some(SetupAction::Gpu {
                    enable,
                    disable,
                    status,
                }) => {
                    if status {
                        setup::gpu::show_status();
                    } else if enable {
                        setup::gpu::enable()?;
                    } else if disable {
                        setup::gpu::disable()?;
                    } else {
                        // Default: show status
                        setup::gpu::show_status();
                    }
                }
                Some(SetupAction::Parakeet { enable, disable, status }) => {
                    if status {
                        setup::parakeet::show_status();
                    } else if enable {
                        setup::parakeet::enable()?;
                    } else if disable {
                        setup::parakeet::disable()?;
                    } else {
                        // Default: show status
                        setup::parakeet::show_status();
                    }
                }
                Some(SetupAction::Compositor { compositor_type }) => {
                    setup::compositor::run(&compositor_type).await?;
                }
                None => {
                    // Default: run setup (non-blocking)
                    setup::run_setup(&config, download, model.as_deref(), quiet, no_post_install)
                        .await?;
                }
            }
        }

        Commands::Config => {
            show_config(&config).await?;
        }

        Commands::Status {
            follow,
            format,
            extended,
            icon_theme,
        } => {
            run_status(&config, follow, &format, extended, icon_theme).await?;
        }

        Commands::Record { action } => {
            send_record_command(&config, action)?;
        }

        Commands::Ui { settings } => {
            #[cfg(feature = "gui")]
            {
                voxtype::gui::launch(settings)?;
            }
            #[cfg(not(feature = "gui"))]
            {
                let _ = settings; // suppress unused warning
                eprintln!("GUI non disponible — recompilez avec : cargo build --features gui");
                eprintln!("Prérequis : libgtk-4-dev libadwaita-1-dev");
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

/// Send a record command to the running daemon via Unix signals or file triggers
fn send_record_command(config: &config::Config, action: RecordAction) -> anyhow::Result<()> {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    use voxtype::OutputModeOverride;

    // Read PID from the pid file
    let pid_file = config::Config::runtime_dir().join("pid");

    if !pid_file.exists() {
        eprintln!("Error: Voxtype daemon is not running.");
        eprintln!("Start it with: voxtype daemon");
        std::process::exit(1);
    }

    let pid_str = std::fs::read_to_string(&pid_file)
        .map_err(|e| anyhow::anyhow!("Failed to read PID file: {}", e))?;

    let pid: i32 = pid_str
        .trim()
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid PID in file: {}", e))?;

    // Check if the process is actually running
    if kill(Pid::from_raw(pid), None).is_err() {
        // Process doesn't exist, clean up stale PID file
        let _ = std::fs::remove_file(&pid_file);
        eprintln!("Error: Voxtype daemon is not running (stale PID file removed).");
        eprintln!("Start it with: voxtype daemon");
        std::process::exit(1);
    }

    // Handle cancel separately (uses file trigger instead of signal)
    if matches!(action, RecordAction::Cancel) {
        let cancel_file = config::Config::runtime_dir().join("cancel");
        std::fs::write(&cancel_file, "cancel")
            .map_err(|e| anyhow::anyhow!("Failed to write cancel file: {}", e))?;
        return Ok(());
    }

    // Write output mode override file if specified
    // For file mode, format is "file" or "file:/path/to/file"
    if let Some(mode_override) = action.output_mode_override() {
        let override_file = config::Config::runtime_dir().join("output_mode_override");
        let mode_str = match mode_override {
            OutputModeOverride::Type => "type".to_string(),
            OutputModeOverride::Clipboard => "clipboard".to_string(),
            OutputModeOverride::Paste => "paste".to_string(),
            OutputModeOverride::File => {
                // Check if explicit path was provided with --file=path
                match action.file_path() {
                    Some(path) if !path.is_empty() => format!("file:{}", path),
                    _ => "file".to_string(),
                }
            }
        };
        std::fs::write(&override_file, mode_str)
            .map_err(|e| anyhow::anyhow!("Failed to write output mode override: {}", e))?;
    }

    // Write model override file if specified
    if let Some(model) = action.model_override() {
        let override_file = config::Config::runtime_dir().join("model_override");
        std::fs::write(&override_file, model)
            .map_err(|e| anyhow::anyhow!("Failed to write model override: {}", e))?;
    }

    // Write profile override file if specified
    if let Some(profile_name) = action.profile() {
        // Validate that the profile exists in config
        if config.get_profile(profile_name).is_none() {
            let available = config.profile_names();
            if available.is_empty() {
                eprintln!("Error: Profile '{}' not found.", profile_name);
                eprintln!();
                eprintln!("No profiles are configured. Add profiles to your config.toml:");
                eprintln!();
                eprintln!("  [profiles.{}]", profile_name);
                eprintln!("  post_process_command = \"your-command-here\"");
            } else {
                eprintln!("Error: Profile '{}' not found.", profile_name);
                eprintln!();
                eprintln!("Available profiles: {}", available.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
            }
            std::process::exit(1);
        }

        let profile_file = config::Config::runtime_dir().join("profile_override");
        std::fs::write(&profile_file, profile_name)
            .map_err(|e| anyhow::anyhow!("Failed to write profile override: {}", e))?;
    }

    // For toggle, we need to read current state to decide which signal to send
    let signal = match &action {
        RecordAction::Start { .. } => Signal::SIGUSR1,
        RecordAction::Stop { .. } => Signal::SIGUSR2,
        RecordAction::Toggle { .. } => {
            // Read current state to determine action
            let state_file = match config.resolve_state_file() {
                Some(path) => path,
                None => {
                    eprintln!("Error: Cannot toggle recording without state_file configured.");
                    eprintln!();
                    eprintln!("Add to your config.toml:");
                    eprintln!("  state_file = \"auto\"");
                    eprintln!();
                    eprintln!("Or use explicit start/stop commands:");
                    eprintln!("  voxtype record start");
                    eprintln!("  voxtype record stop");
                    std::process::exit(1);
                }
            };

            let current_state =
                std::fs::read_to_string(&state_file).unwrap_or_else(|_| "idle".to_string());

            if current_state.trim() == "recording" {
                Signal::SIGUSR2 // Stop
            } else {
                Signal::SIGUSR1 // Start
            }
        }
        RecordAction::Cancel => unreachable!(), // Handled above
    };

    kill(Pid::from_raw(pid), signal)
        .map_err(|e| anyhow::anyhow!("Failed to send signal to daemon: {}", e))?;

    Ok(())
}

/// Transcribe an audio file
fn transcribe_file(config: &config::Config, path: &PathBuf) -> anyhow::Result<()> {
    use hound::WavReader;

    println!("Loading audio file: {:?}", path);

    let reader = WavReader::open(path)?;
    let spec = reader.spec();

    println!(
        "Audio format: {} Hz, {} channel(s), {:?}",
        spec.sample_rate, spec.channels, spec.sample_format
    );

    // Convert samples to f32 mono at 16kHz
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = (1 << (spec.bits_per_sample - 1)) as f32;
            reader
                .into_samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / max_val)
                .collect()
        }
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(|s| s.ok())
            .collect(),
    };

    // Mix to mono if stereo
    let mono_samples: Vec<f32> = if spec.channels > 1 {
        samples
            .chunks(spec.channels as usize)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect()
    } else {
        samples
    };

    // Resample to 16kHz if needed
    let final_samples = if spec.sample_rate != 16000 {
        println!("Resampling from {} Hz to 16000 Hz...", spec.sample_rate);
        resample(&mono_samples, spec.sample_rate, 16000)
    } else {
        mono_samples
    };

    println!(
        "Processing {} samples ({:.2}s)...",
        final_samples.len(),
        final_samples.len() as f32 / 16000.0
    );

    // Create transcriber and transcribe
    let transcriber = transcribe::create_transcriber(&config)?;
    let text = transcriber.transcribe(&final_samples)?;

    println!("\n{}", text);
    Ok(())
}

/// Simple linear resampling
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = to_rate as f64 / from_rate as f64;
    let new_len = (samples.len() as f64 * ratio).ceil() as usize;
    let mut output = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let src_idx = i as f64 / ratio;
        let idx = src_idx.floor() as usize;
        let frac = (src_idx - idx as f64) as f32;

        let sample = if idx + 1 < samples.len() {
            samples[idx] * (1.0 - frac) + samples[idx + 1] * frac
        } else {
            samples.get(idx).copied().unwrap_or(0.0)
        };

        output.push(sample);
    }

    output
}

/// JSON output for Waybar consumption.
///
/// STABILITY: These field names MUST NOT change between minor versions (NFR2).
/// The `text`, `alt`, `class`, and `tooltip` fields form the stable Waybar contract.
/// Waybar expects these fields for its custom module JSON protocol.
///
/// - `text`: Display text (icon from theme)
/// - `alt`: State name for Waybar `format-icons` mapping
/// - `class`: State name for CSS styling (one of: `idle`, `recording`, `transcribing`, `stopped`)
/// - `tooltip`: Human-readable status description
/// - `level`: Audio RMS level (0.0–1.0), present only during `recording` state
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
struct WaybarStatus {
    text: String,
    alt: String,
    class: String,
    tooltip: String,
    /// Audio RMS level (0.0–1.0), only present during recording state
    #[serde(skip_serializing_if = "Option::is_none", default)]
    level: Option<f32>,
    /// Whisper model name (only present with `--extended`)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    model: Option<String>,
    /// Audio input device (only present with `--extended`)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    device: Option<String>,
    /// Transcription backend (only present with `--extended`)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    backend: Option<String>,
}

/// Extended status info for JSON output
struct ExtendedStatusInfo {
    model: String,
    device: String,
    backend: String,
}

impl ExtendedStatusInfo {
    fn from_config(config: &config::Config) -> Self {
        let backend = setup::gpu::detect_current_backend()
            .map(|b| match b {
                setup::gpu::Backend::Cpu => "CPU (legacy)",
                setup::gpu::Backend::Native => "CPU (native)",
                setup::gpu::Backend::Avx2 => "CPU (AVX2)",
                setup::gpu::Backend::Avx512 => "CPU (AVX-512)",
                setup::gpu::Backend::Vulkan => "GPU (Vulkan)",
            })
            .unwrap_or("unknown")
            .to_string();

        Self {
            model: config.whisper.model.clone(),
            device: config.audio.device.clone(),
            backend,
        }
    }
}

/// Check if the daemon is actually running by verifying the PID file
fn is_daemon_running() -> bool {
    let pid_path = config::Config::runtime_dir().join("pid");

    // Read PID from file
    let pid_str = match std::fs::read_to_string(&pid_path) {
        Ok(s) => s,
        Err(_) => return false, // No PID file = not running
    };

    let pid: u32 = match pid_str.trim().parse() {
        Ok(p) => p,
        Err(_) => return false, // Invalid PID = not running
    };

    // Check if process exists by testing /proc/{pid}
    std::path::Path::new(&format!("/proc/{}", pid)).exists()
}

/// Run the status command - show current daemon state
async fn run_status(
    config: &config::Config,
    follow: bool,
    format: &str,
    extended: bool,
    icon_theme_override: Option<String>,
) -> anyhow::Result<()> {
    let state_file = config.resolve_state_file();

    if state_file.is_none() {
        eprintln!("Error: state_file is not configured.");
        eprintln!();
        eprintln!("To enable status monitoring, add to your config.toml:");
        eprintln!();
        eprintln!("  state_file = \"auto\"");
        eprintln!();
        eprintln!("This enables external integrations like Waybar to monitor voxtype state.");
        std::process::exit(1);
    }

    let state_path = state_file.unwrap();
    let ext_info = if extended {
        Some(ExtendedStatusInfo::from_config(config))
    } else {
        None
    };

    // Use CLI override if provided, otherwise use config
    let icons = if let Some(ref theme) = icon_theme_override {
        let mut status_config = config.status.clone();
        status_config.icon_theme = theme.clone();
        status_config.resolve_icons()
    } else {
        config.status.resolve_icons()
    };

    if !follow {
        // One-shot: just read and print current state
        // First check if daemon is actually running to avoid stale state
        let state = if !is_daemon_running() {
            "stopped".to_string()
        } else {
            std::fs::read_to_string(&state_path).unwrap_or_else(|_| "stopped".to_string())
        };
        let state = state.trim();

        if format == "json" {
            println!("{}", format_state_json(state, &icons, ext_info.as_ref()));
        } else {
            println!("{}", state);
        }
        return Ok(());
    }

    // Follow mode: watch for changes using inotify
    use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc::channel;
    use std::time::Duration;

    // Print initial state (check if daemon is running to avoid stale state)
    let state = if !is_daemon_running() {
        "stopped".to_string()
    } else {
        std::fs::read_to_string(&state_path).unwrap_or_else(|_| "stopped".to_string())
    };
    let state = state.trim();
    if format == "json" {
        println!("{}", format_state_json(state, &icons, ext_info.as_ref()));
    } else {
        println!("{}", state);
    }

    // Set up file watcher
    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.send(res);
        },
        NotifyConfig::default().with_poll_interval(Duration::from_millis(100)),
    )?;

    // Watch the state file's parent directory (file may not exist yet)
    if let Some(parent) = state_path.parent() {
        std::fs::create_dir_all(parent)?;
        watcher.watch(parent, RecursiveMode::NonRecursive)?;
    }

    // Also try to watch the file directly if it exists
    if state_path.exists() {
        let _ = watcher.watch(&state_path, RecursiveMode::NonRecursive);
    }

    let mut last_state = state.to_string();
    let mut last_level: Option<f32> = None;

    loop {
        // Use shorter timeout during recording to capture level updates at ~20fps
        let timeout = if last_state == "recording" {
            Duration::from_millis(50)
        } else {
            Duration::from_millis(500)
        };

        match rx.recv_timeout(timeout) {
            Ok(Ok(_event)) => {
                // File changed (state or level), read new state
                if let Ok(new_state) = std::fs::read_to_string(&state_path) {
                    let new_state = new_state.trim().to_string();
                    let level = read_audio_level(&state_path);

                    if new_state != last_state || level != last_level {
                        if format == "json" {
                            println!(
                                "{}",
                                format_state_json_with_level(
                                    &new_state,
                                    &icons,
                                    ext_info.as_ref(),
                                    level
                                )
                            );
                        } else {
                            println!("{}", new_state);
                        }
                        last_state = new_state;
                        last_level = level;
                    }
                }
            }
            Ok(Err(e)) => {
                tracing::warn!("Watch error: {:?}", e);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // During recording, poll for level changes even without inotify events
                if last_state == "recording" && format == "json" {
                    let level = read_audio_level(&state_path);
                    if level != last_level {
                        println!(
                            "{}",
                            format_state_json_with_level(
                                &last_state,
                                &icons,
                                ext_info.as_ref(),
                                level
                            )
                        );
                        last_level = level;
                    }
                }

                // Check if daemon stopped (file deleted or process died)
                if (!state_path.exists() || !is_daemon_running()) && last_state != "stopped" {
                    if format == "json" {
                        println!(
                            "{}",
                            format_state_json("stopped", &icons, ext_info.as_ref())
                        );
                    } else {
                        println!("stopped");
                    }
                    last_state = "stopped".to_string();
                    last_level = None;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    Ok(())
}

/// Read the current audio level from the level file (sibling of state file).
///
/// Returns `Some(level)` if the file exists and contains a valid float,
/// `None` otherwise (e.g., not recording, file missing).
fn read_audio_level(state_path: &std::path::Path) -> Option<f32> {
    let level_path = state_path.with_file_name("audio_level");
    std::fs::read_to_string(&level_path)
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .filter(|&l| l >= 0.0 && l <= 1.0)
}

/// Format state as JSON for Waybar consumption.
///
/// Builds a [`WaybarStatus`] and serializes it via `serde_json`.
/// This ensures valid JSON regardless of special characters in field values.
///
/// The `alt` field enables Waybar's `format-icons` feature for custom icon mapping.
///
/// STABILITY: The field names (`text`, `alt`, `class`, `tooltip`) MUST NOT change
/// between minor versions (NFR2 — rétrocompatibilité).
fn format_state_json(
    state: &str,
    icons: &config::ResolvedIcons,
    extended: Option<&ExtendedStatusInfo>,
) -> String {
    format_state_json_with_level(state, icons, extended, None)
}

/// Format state as JSON with an optional audio level for recording state.
///
/// The `level` parameter is the RMS audio level (0.0–1.0) from the audio capture,
/// included only when state is "recording" and level is Some.
fn format_state_json_with_level(
    state: &str,
    icons: &config::ResolvedIcons,
    extended: Option<&ExtendedStatusInfo>,
    level: Option<f32>,
) -> String {
    let (text, base_tooltip) = match state {
        "recording" => (&icons.recording, "Recording..."),
        "transcribing" => (&icons.transcribing, "Transcribing..."),
        "idle" => (&icons.idle, "Voxtype ready - hold hotkey to record"),
        "stopped" => (&icons.stopped, "Voxtype not running"),
        _ => (&icons.idle, "Unknown state"),
    };

    // Only include level during recording state
    let effective_level = if state == "recording" { level } else { None };

    let status = match extended {
        Some(info) => {
            let tooltip = format!(
                "{}\nModel: {}\nDevice: {}\nBackend: {}",
                base_tooltip, info.model, info.device, info.backend
            );
            WaybarStatus {
                text: text.clone(),
                alt: state.to_string(),
                class: state.to_string(),
                tooltip,
                level: effective_level,
                model: Some(info.model.clone()),
                device: Some(info.device.clone()),
                backend: Some(info.backend.clone()),
            }
        }
        None => WaybarStatus {
            text: text.clone(),
            alt: state.to_string(),
            class: state.to_string(),
            tooltip: base_tooltip.to_string(),
            level: effective_level,
            model: None,
            device: None,
            backend: None,
        },
    };

    // serde_json::to_string cannot fail for this struct (all fields are String/Option<String>),
    // but we provide a safe fallback rather than panicking in production.
    serde_json::to_string(&status).unwrap_or_else(|_| {
        format!(
            r#"{{"text":"","alt":"{}","class":"{}","tooltip":"Serialization error"}}"#,
            state, state
        )
    })
}

/// Show current configuration
async fn show_config(config: &config::Config) -> anyhow::Result<()> {
    println!("Current Configuration\n");
    println!("=====================\n");

    println!("[hotkey]");
    println!("  key = {:?}", config.hotkey.key);
    println!("  modifiers = {:?}", config.hotkey.modifiers);
    println!("  mode = {:?}", config.hotkey.mode);

    println!("\n[audio]");
    println!("  device = {:?}", config.audio.device);
    println!("  sample_rate = {}", config.audio.sample_rate);
    println!("  max_duration_secs = {}", config.audio.max_duration_secs);

    println!("\n[audio.feedback]");
    println!("  enabled = {}", config.audio.feedback.enabled);
    println!("  theme = {:?}", config.audio.feedback.theme);
    println!("  volume = {}", config.audio.feedback.volume);

    // Show current engine
    println!("\n[engine]");
    println!("  engine = {:?}", config.engine);

    println!("\n[whisper]");
    println!("  model = {:?}", config.whisper.model);
    println!("  language = {:?}", config.whisper.language);
    println!("  translate = {}", config.whisper.translate);
    if let Some(threads) = config.whisper.threads {
        println!("  threads = {}", threads);
    }

    // Show Parakeet status (experimental)
    println!("\n[parakeet] (EXPERIMENTAL)");
    if let Some(ref parakeet_config) = config.parakeet {
        println!("  model = {:?}", parakeet_config.model);
        if let Some(ref model_type) = parakeet_config.model_type {
            println!("  model_type = {:?}", model_type);
        }
        println!("  on_demand_loading = {}", parakeet_config.on_demand_loading);
    } else {
        println!("  (not configured)");
    }

    // Check for available Parakeet models
    let models_dir = config::Config::models_dir();
    let mut parakeet_models: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&models_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.contains("parakeet") {
                    // Check if it has the required ONNX files
                    let has_encoder = path.join("encoder-model.onnx").exists();
                    let has_decoder = path.join("decoder_joint-model.onnx").exists()
                        || path.join("model.onnx").exists();
                    if has_encoder || has_decoder {
                        parakeet_models.push(name);
                    }
                }
            }
        }
    }
    if parakeet_models.is_empty() {
        println!("  available models: (none found)");
    } else {
        println!("  available models: {}", parakeet_models.join(", "));
    }

    println!("\n[output]");
    println!("  mode = {:?}", config.output.mode);
    println!(
        "  fallback_to_clipboard = {}",
        config.output.fallback_to_clipboard
    );
    if let Some(ref driver_order) = config.output.driver_order {
        println!(
            "  driver_order = [{}]",
            driver_order
                .iter()
                .map(|d| format!("{:?}", d))
                .collect::<Vec<_>>()
                .join(", ")
        );
    } else {
        println!("  driver_order = (default: wtype -> dotool -> ydotool -> clipboard)");
    }
    println!("  type_delay_ms = {}", config.output.type_delay_ms);
    println!("  pre_type_delay_ms = {}", config.output.pre_type_delay_ms);

    println!("\n[output.notification]");
    println!(
        "  on_recording_start = {}",
        config.output.notification.on_recording_start
    );
    println!(
        "  on_recording_stop = {}",
        config.output.notification.on_recording_stop
    );
    println!(
        "  on_transcription = {}",
        config.output.notification.on_transcription
    );

    println!("\n[status]");
    println!("  icon_theme = {:?}", config.status.icon_theme);
    let icons = config.status.resolve_icons();
    println!(
        "  (resolved icons: idle={:?} recording={:?} transcribing={:?} stopped={:?})",
        icons.idle, icons.recording, icons.transcribing, icons.stopped
    );

    if let Some(ref state_file) = config.state_file {
        println!("\n[integration]");
        println!("  state_file = {:?}", state_file);
        if let Some(resolved) = config.resolve_state_file() {
            println!("  (resolves to: {:?})", resolved);
        }
    }

    // Show output chain status
    let output_status = setup::detect_output_chain().await;
    setup::print_output_chain_status(&output_status);

    println!("\n---");
    println!(
        "Config file: {:?}",
        config::Config::default_path().unwrap_or_else(|| PathBuf::from("(not found)"))
    );
    println!("Models dir: {:?}", config::Config::models_dir());

    Ok(())
}

/// Reset SIGPIPE to default behavior (terminate process) instead of the Rust
/// default of ignoring it. This prevents panics when stdout is piped through
/// commands like `head` that close the pipe early.
#[cfg(unix)]
fn reset_sigpipe() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {
    // No-op on non-Unix platforms
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create default test icons (emoji theme)
    fn test_icons() -> config::ResolvedIcons {
        config::ResolvedIcons {
            idle: "🎙️".to_string(),
            recording: "🎤".to_string(),
            transcribing: "⏳".to_string(),
            stopped: "".to_string(),
        }
    }

    // ===== Task 1.1: 4 standard fields always present =====

    #[test]
    fn test_json_idle_has_four_standard_fields() {
        let icons = test_icons();
        let json = format_state_json("idle", &icons, None);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert!(parsed["text"].is_string(), "text field missing");
        assert!(parsed["alt"].is_string(), "alt field missing");
        assert!(parsed["class"].is_string(), "class field missing");
        assert!(parsed["tooltip"].is_string(), "tooltip field missing");
    }

    #[test]
    fn test_json_recording_has_four_standard_fields() {
        let icons = test_icons();
        let json = format_state_json("recording", &icons, None);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(parsed["class"], "recording");
        assert_eq!(parsed["alt"], "recording");
        assert!(parsed["text"].is_string());
        assert!(parsed["tooltip"].is_string());
    }

    #[test]
    fn test_json_transcribing_has_four_standard_fields() {
        let icons = test_icons();
        let json = format_state_json("transcribing", &icons, None);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(parsed["class"], "transcribing");
        assert_eq!(parsed["alt"], "transcribing");
    }

    #[test]
    fn test_json_stopped_has_four_standard_fields() {
        let icons = test_icons();
        let json = format_state_json("stopped", &icons, None);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(parsed["class"], "stopped");
        assert_eq!(parsed["tooltip"], "Voxtype not running");
    }

    // ===== Task 1.2: Extended fields only with --extended =====

    #[test]
    fn test_json_no_extended_fields_without_flag() {
        let icons = test_icons();
        let json = format_state_json("idle", &icons, None);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert!(parsed.get("model").is_none(), "model should be absent without --extended");
        assert!(parsed.get("device").is_none(), "device should be absent without --extended");
        assert!(parsed.get("backend").is_none(), "backend should be absent without --extended");
    }

    #[test]
    fn test_json_extended_fields_present_with_flag() {
        let icons = test_icons();
        let ext = ExtendedStatusInfo {
            model: "base".to_string(),
            device: "default".to_string(),
            backend: "CPU (native)".to_string(),
        };
        let json = format_state_json("idle", &icons, Some(&ext));
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(parsed["model"], "base");
        assert_eq!(parsed["device"], "default");
        assert_eq!(parsed["backend"], "CPU (native)");
    }

    #[test]
    fn test_json_extended_tooltip_includes_extra_info() {
        let icons = test_icons();
        let ext = ExtendedStatusInfo {
            model: "large-v3".to_string(),
            device: "pulse".to_string(),
            backend: "GPU (Vulkan)".to_string(),
        };
        let json = format_state_json("idle", &icons, Some(&ext));
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let tooltip = parsed["tooltip"].as_str().unwrap();
        assert!(tooltip.contains("Model: large-v3"), "tooltip should include model");
        assert!(tooltip.contains("Device: pulse"), "tooltip should include device");
        assert!(tooltip.contains("Backend: GPU (Vulkan)"), "tooltip should include backend");
    }

    // ===== Task 1.3: Stopped state =====

    #[test]
    fn test_json_stopped_state_class() {
        let icons = test_icons();
        let json = format_state_json("stopped", &icons, None);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(parsed["class"], "stopped");
        assert_eq!(parsed["alt"], "stopped");
    }

    // ===== Task 1.4: Special characters produce valid JSON =====

    #[test]
    fn test_json_special_chars_in_model_name() {
        let icons = test_icons();
        let ext = ExtendedStatusInfo {
            model: r#"model "with" quotes & backslash \"#.to_string(),
            device: "device/with/slashes".to_string(),
            backend: "backend<with>angles".to_string(),
        };
        let json = format_state_json("idle", &icons, Some(&ext));
        // Must be parseable as valid JSON despite special characters
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("JSON with special chars must be valid");
        assert!(parsed["model"].as_str().unwrap().contains("quotes"));
    }

    #[test]
    fn test_json_unicode_in_icon_text() {
        let icons = config::ResolvedIcons {
            idle: "🎙️ prêt".to_string(),
            recording: "🎤".to_string(),
            transcribing: "⏳".to_string(),
            stopped: "⛔".to_string(),
        };
        let json = format_state_json("idle", &icons, None);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("JSON with unicode must be valid");
        assert!(parsed["text"].as_str().unwrap().contains("prêt"));
    }

    // ===== Task 2.5: Gold test — verify output structure =====

    #[test]
    fn test_json_class_values_are_state_names() {
        let icons = test_icons();
        for state in &["idle", "recording", "transcribing", "stopped"] {
            let json = format_state_json(state, &icons, None);
            let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
            assert_eq!(parsed["class"].as_str().unwrap(), *state,
                "class field should equal the state name for state '{}'", state);
            assert_eq!(parsed["alt"].as_str().unwrap(), *state,
                "alt field should equal the state name for state '{}'", state);
        }
    }

    #[test]
    fn test_json_unknown_state_falls_back_to_idle_icon() {
        let icons = test_icons();
        let json = format_state_json("outputting", &icons, None);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        // Unknown states use idle icon
        assert_eq!(parsed["text"].as_str().unwrap(), icons.idle);
        assert_eq!(parsed["tooltip"], "Unknown state");
        // But class/alt reflect the actual state string
        assert_eq!(parsed["class"], "outputting");
    }

    #[test]
    fn test_waybar_status_struct_serialization_roundtrip() {
        let status = WaybarStatus {
            text: "🎙️".to_string(),
            alt: "idle".to_string(),
            class: "idle".to_string(),
            tooltip: "Ready".to_string(),
            level: None,
            model: None,
            device: None,
            backend: None,
        };
        let json = serde_json::to_string(&status).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        // Optional fields should be absent when None
        assert!(parsed.get("model").is_none());
        assert!(parsed.get("device").is_none());
        assert!(parsed.get("backend").is_none());
        // Required fields present
        assert_eq!(parsed["text"], "🎙️");
        assert_eq!(parsed["alt"], "idle");
        assert_eq!(parsed["class"], "idle");
        assert_eq!(parsed["tooltip"], "Ready");
    }

    // ===== M3: Missing edge case tests =====

    #[test]
    fn test_json_unknown_state_with_extended() {
        let icons = test_icons();
        let ext = ExtendedStatusInfo {
            model: "base".to_string(),
            device: "default".to_string(),
            backend: "CPU (native)".to_string(),
        };
        let json = format_state_json("outputting", &icons, Some(&ext));
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        // Unknown state uses idle icon even with extended
        assert_eq!(parsed["text"].as_str().unwrap(), icons.idle);
        assert_eq!(parsed["class"], "outputting");
        assert_eq!(parsed["model"], "base");
    }

    #[test]
    fn test_json_empty_icon_strings() {
        let icons = config::ResolvedIcons {
            idle: "".to_string(),
            recording: "".to_string(),
            transcribing: "".to_string(),
            stopped: "".to_string(),
        };
        let json = format_state_json("idle", &icons, None);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON even with empty icons");
        assert_eq!(parsed["text"], "");
        assert_eq!(parsed["class"], "idle");
    }

    #[test]
    fn test_json_field_order_stability() {
        let icons = test_icons();
        let json = format_state_json("idle", &icons, None);
        // serde_json preserves struct field declaration order
        // Verify the order is: text, alt, class, tooltip
        let fields: Vec<&str> = json
            .trim_start_matches('{')
            .trim_end_matches('}')
            .split(',')
            .filter_map(|pair| pair.split(':').next())
            .map(|key| key.trim().trim_matches('"'))
            .collect();
        assert_eq!(fields, vec!["text", "alt", "class", "tooltip"],
            "Field order must be stable: text, alt, class, tooltip");
    }

    // ===== L2: Deserialize roundtrip test =====

    #[test]
    fn test_waybar_status_deserialize_roundtrip() {
        let original = WaybarStatus {
            text: "🎤".to_string(),
            alt: "recording".to_string(),
            class: "recording".to_string(),
            tooltip: "Recording...".to_string(),
            level: None,
            model: Some("large-v3".to_string()),
            device: Some("pulse".to_string()),
            backend: Some("GPU (Vulkan)".to_string()),
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: WaybarStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_waybar_status_deserialize_without_optional_fields() {
        // Simulate what Waybar or an external consumer sees (no extended fields)
        let json = r#"{"text":"🎙️","alt":"idle","class":"idle","tooltip":"Ready"}"#;
        let status: WaybarStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.text, "🎙️");
        assert_eq!(status.level, None);
        assert_eq!(status.model, None);
        assert_eq!(status.device, None);
        assert_eq!(status.backend, None);
    }

    // ===== Story 2.3: Level field tests =====

    #[test]
    fn test_json_level_present_during_recording() {
        let icons = test_icons();
        let json = format_state_json_with_level("recording", &icons, None, Some(0.42));
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let level = parsed["level"].as_f64().expect("level should be a number");
        assert!((level - 0.42).abs() < 0.001, "level should be ~0.42");
    }

    #[test]
    fn test_json_level_absent_during_idle() {
        let icons = test_icons();
        let json = format_state_json_with_level("idle", &icons, None, Some(0.42));
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert!(parsed.get("level").is_none(), "level should be absent during idle");
    }

    #[test]
    fn test_json_level_absent_during_transcribing() {
        let icons = test_icons();
        let json = format_state_json_with_level("transcribing", &icons, None, Some(0.5));
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert!(parsed.get("level").is_none(), "level should be absent during transcribing");
    }

    #[test]
    fn test_json_level_none_not_serialized() {
        let icons = test_icons();
        let json = format_state_json_with_level("recording", &icons, None, None);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert!(parsed.get("level").is_none(), "level None should not be serialized");
    }

    #[test]
    fn test_json_level_with_extended() {
        let icons = test_icons();
        let ext = ExtendedStatusInfo {
            model: "base".to_string(),
            device: "default".to_string(),
            backend: "CPU (native)".to_string(),
        };
        let json = format_state_json_with_level("recording", &icons, Some(&ext), Some(0.75));
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert!(parsed["level"].as_f64().is_some(), "level should be present");
        assert_eq!(parsed["model"], "base", "extended fields should still work");
    }
}
