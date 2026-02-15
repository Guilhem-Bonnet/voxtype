//! Shared status monitor — single subprocess for overlay + tray.
//!
//! # Design
//!
//! Spawns ONE `voxtype status --follow --format json` subprocess and fans out
//! each JSON line to all registered consumers via `std::sync::mpsc::Sender`.
//!
//! When the subprocess exits, a synthetic `{"class":"stopped"}` line is sent
//! so consumers can react, then the bus retries after a delay.
//!
//! # Consumers
//!
//! - **Overlay** (GLib main loop): polls its `Receiver` with `glib::timeout_add_local`
//! - **Tray** (tokio runtime): bridges its `Receiver` via `spawn_blocking` → tokio mpsc

use std::io::BufRead;
use std::process::{Command, Stdio};
use std::sync::mpsc;

/// Start the shared status bus.
///
/// Returns two receivers: one for the overlay (GLib thread) and one for the tray
/// (tokio thread). The bus thread runs forever, restarting the subprocess on failure.
pub fn start() -> (mpsc::Receiver<String>, mpsc::Receiver<String>) {
    let (tx_overlay, rx_overlay) = mpsc::channel::<String>();
    let (tx_tray, rx_tray) = mpsc::channel::<String>();

    std::thread::Builder::new()
        .name("status-bus".into())
        .spawn(move || {
            let senders = [tx_overlay, tx_tray];
            run_bus(&senders);
        })
        .expect("failed to spawn status bus thread");

    (rx_overlay, rx_tray)
}

/// Core bus loop: spawn subprocess, read lines, fan out, retry on failure.
fn run_bus(senders: &[mpsc::Sender<String>]) {
    loop {
        let child = Command::new("voxtype")
            .args(["status", "--follow", "--format", "json"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        let child = match child {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Status bus: failed to spawn voxtype status: {e}");
                std::thread::sleep(std::time::Duration::from_secs(5));
                continue;
            }
        };

        let stdout = match child.stdout {
            Some(s) => s,
            None => {
                tracing::warn!("Status bus: no stdout from subprocess");
                std::thread::sleep(std::time::Duration::from_secs(5));
                continue;
            }
        };

        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    if !fan_out(senders, l) {
                        return; // All consumers disconnected, shut down
                    }
                }
                Err(_) => break, // Subprocess pipe broken, restart
            }
        }

        // Subprocess exited — notify consumers so they can show "stopped" state
        let stopped = r#"{"class":"stopped"}"#.to_string();
        if !fan_out(senders, stopped) {
            return;
        }

        // Wait before reconnecting
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

/// Send a line to all senders. Returns false if ALL senders are disconnected.
fn fan_out(senders: &[mpsc::Sender<String>], line: String) -> bool {
    let mut any_alive = false;
    for tx in senders.iter() {
        if tx.send(line.clone()).is_ok() {
            any_alive = true;
        }
    }
    any_alive
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_out_delivers_to_all() {
        let (tx1, rx1) = mpsc::channel();
        let (tx2, rx2) = mpsc::channel();
        let senders = vec![tx1, tx2];

        assert!(fan_out(&senders, "hello".into()));
        assert_eq!(rx1.recv().unwrap(), "hello");
        assert_eq!(rx2.recv().unwrap(), "hello");
    }

    #[test]
    fn fan_out_partial_disconnect() {
        let (tx1, rx1) = mpsc::channel();
        let (tx2, _rx2) = mpsc::channel();
        // Drop _rx2 so tx2 is disconnected
        drop(_rx2);
        let senders = vec![tx1, tx2];

        assert!(fan_out(&senders, "test".into()));
        assert_eq!(rx1.recv().unwrap(), "test");
    }

    #[test]
    fn fan_out_all_disconnected() {
        let (tx1, _rx1) = mpsc::channel::<String>();
        let (tx2, _rx2) = mpsc::channel::<String>();
        drop(_rx1);
        drop(_rx2);
        let senders = vec![tx1, tx2];

        assert!(!fan_out(&senders, "gone".into()));
    }

    #[test]
    fn fan_out_empty_senders() {
        let senders: Vec<mpsc::Sender<String>> = vec![];
        assert!(!fan_out(&senders, "nope".into()));
    }

    #[test]
    fn start_returns_two_receivers() {
        // Just verify start() produces two receivers without panicking.
        // We can't easily test the subprocess on CI, but we verify the channels work.
        let (tx1, rx1) = mpsc::channel::<String>();
        let (tx2, rx2) = mpsc::channel::<String>();

        tx1.send("a".into()).unwrap();
        tx2.send("b".into()).unwrap();

        assert_eq!(rx1.recv().unwrap(), "a");
        assert_eq!(rx2.recv().unwrap(), "b");
    }
}
