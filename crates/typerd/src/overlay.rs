//! Keeps the key-overlay process (`typer-keys`) running while it is enabled.

use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

/// How often to check whether the overlay died.
const POLL: Duration = Duration::from_millis(500);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

pub struct Overlay {
    tx: Sender<bool>,
}

impl Overlay {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name("overlay".into())
            .spawn(move || {
                let mut enabled = false;
                let mut child: Option<Child> = None;
                let mut backoff = Duration::from_secs(1);
                let mut next_start = Instant::now();
                loop {
                    match rx.recv_timeout(POLL) {
                        Ok(on) => {
                            enabled = on;
                            backoff = Duration::from_secs(1);
                            next_start = Instant::now();
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => enabled = false,
                    }
                    // Reap an overlay that exited on its own (crash, daemon restart).
                    if let Some(c) = &mut child
                        && let Ok(Some(status)) = c.try_wait()
                    {
                        eprintln!("typerd: key overlay exited ({status})");
                        child = None;
                        next_start = Instant::now() + backoff;
                        backoff = (backoff * 2).min(MAX_BACKOFF);
                    }
                    match (&mut child, enabled) {
                        (None, true) if Instant::now() >= next_start => {
                            child = start();
                            if child.is_none() {
                                next_start = Instant::now() + backoff;
                                backoff = (backoff * 2).min(MAX_BACKOFF);
                            }
                        }
                        (Some(c), false) => {
                            let _ = c.kill();
                            let _ = c.wait();
                            child = None;
                        }
                        _ => {}
                    }
                }
            })
            .expect("spawning overlay thread");
        Self { tx }
    }

    pub fn set_enabled(&self, enabled: bool) {
        let _ = self.tx.send(enabled);
    }
}

fn start() -> Option<Child> {
    let bin = std::env::var("TYPER_KEYS_BIN").unwrap_or_else(|_| "typer-keys".into());
    Command::new(&bin)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .inspect_err(|e| eprintln!("typerd: could not start {bin}: {e}"))
        .ok()
}
