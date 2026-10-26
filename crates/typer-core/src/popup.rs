use std::process::{Command, Stdio};

use anyhow::Context;

/// Open the control popup, or close it if it is already open: the popup app is
/// single-instance and a second launch makes the running one exit.
///
/// `at` is where the tray icon was clicked, in tray-host coordinates; the popup
/// opens just below it.
pub fn toggle(at: Option<(i32, i32)>) -> anyhow::Result<()> {
    let bin = std::env::var("TYPER_POPUP").unwrap_or_else(|_| "typer".into());
    let mut cmd = Command::new(&bin);
    if let Some((x, y)) = at {
        cmd.arg("--at").arg(format!("{x},{y}"));
    }
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        // Inherited: under typerd this lands in the journal.
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("launching {bin}"))?;
    // Reap it when it closes so a long-lived caller (typerd) doesn't collect zombies.
    std::thread::spawn(move || child.wait());
    Ok(())
}
