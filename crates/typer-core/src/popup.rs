use std::process::{Command, Stdio};

use anyhow::Context;

/// Open the control popup, or close it if it is already open: the popup app is
/// single-instance and a second launch makes the running one exit.
pub fn toggle() -> anyhow::Result<()> {
    let bin = std::env::var("TYPER_POPUP").unwrap_or_else(|_| "typer".into());
    let mut child = Command::new(&bin)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("launching {bin}"))?;
    // Reap it when it closes so a long-lived caller (typerd) doesn't collect zombies.
    std::thread::spawn(move || child.wait());
    Ok(())
}
