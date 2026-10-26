use std::thread;
use std::time::Duration;

use anyhow::{Context, bail};
use clap::{Parser, Subcommand};
use serde_json::json;
use typer_core::client;
use typer_core::{Request, Response, State};

#[derive(Parser)]
#[command(
    name = "typerctl",
    version,
    about = "Control the typer keyboard sound daemon"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print the current state
    Status {
        /// Keep running and print every change
        #[arg(long)]
        follow: bool,
        /// Waybar custom-module JSON output
        #[arg(long)]
        waybar: bool,
    },
    /// Toggle sounds on/off
    Toggle,
    /// Turn sounds on
    On,
    /// Turn sounds off
    Off,
    /// Set the volume: `40`, `+5` or `-5`
    #[command(allow_negative_numbers = true)]
    Volume { value: String },
    /// Switch to a sound pack by id
    Pack { id: String },
    /// List available sound packs
    Packs,
    /// Play a sample key with the active pack
    Preview,
    /// Rescan packs and reload the active one
    Reload,
    /// Open or close the control popup
    Popup,
    /// Key overlay: `on`, `off`, `toggle` or `move` (drag it, then click to drop)
    Keys { action: KeysAction },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum KeysAction {
    On,
    Off,
    Toggle,
    Move,
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Cmd::Status {
            follow: false,
            waybar,
        } => print_state(&client::request_state(&Request::Status)?, waybar),
        Cmd::Status {
            follow: true,
            waybar,
        } => follow(waybar),
        Cmd::Toggle => print_state(&client::request_state(&Request::Toggle)?, false),
        Cmd::On => print_state(
            &client::request_state(&Request::SetEnabled { enabled: true })?,
            false,
        ),
        Cmd::Off => print_state(
            &client::request_state(&Request::SetEnabled { enabled: false })?,
            false,
        ),
        Cmd::Volume { value } => {
            let req = parse_volume(&value)?;
            print_state(&client::request_state(&req)?, false)
        }
        Cmd::Pack { id } => print_state(&client::request_state(&Request::SetPack { id })?, false),
        Cmd::Packs => {
            let Response::Packs { packs } = client::request(&Request::ListPacks)? else {
                bail!("unexpected response from typerd");
            };
            let active = client::request_state(&Request::Status)
                .map(|s| s.pack)
                .unwrap_or_default();
            for p in packs {
                let mark = if p.id == active { "*" } else { " " };
                println!("{mark} {:<28} {}", p.id, p.name);
            }
            Ok(())
        }
        Cmd::Preview => client::request(&Request::Preview).map(drop),
        Cmd::Reload => print_state(&client::request_state(&Request::Reload)?, false),
        Cmd::Popup => typer_core::popup::toggle(None),
        Cmd::Keys { action } => {
            let req = match action {
                KeysAction::On => Request::SetOverlay { enabled: true },
                KeysAction::Off => Request::SetOverlay { enabled: false },
                KeysAction::Toggle => {
                    let on = client::request_state(&Request::Status)?.overlay;
                    Request::SetOverlay { enabled: !on }
                }
                KeysAction::Move => Request::MoveOverlay { moving: true },
            };
            print_state(&client::request_state(&req)?, false)
        }
    }
}

fn parse_volume(value: &str) -> anyhow::Result<Request> {
    let n: i16 = value
        .trim_start_matches('+')
        .parse()
        .with_context(|| format!("invalid volume {value:?}"))?;
    Ok(if value.starts_with(['+', '-']) {
        Request::AdjustVolume { delta: n }
    } else {
        Request::SetVolume {
            volume: n.clamp(0, 100) as u8,
        }
    })
}

fn print_state(state: &State, waybar: bool) -> anyhow::Result<()> {
    if waybar {
        println!("{}", waybar_json(Some(state)));
    } else {
        let onoff = if state.enabled { "on" } else { "off" };
        let keys = if state.overlay { " · keys on" } else { "" };
        println!(
            "{onoff} · {}% · {} ({}){keys}",
            state.volume, state.pack_name, state.pack
        );
    }
    Ok(())
}

fn waybar_json(state: Option<&State>) -> serde_json::Value {
    match state {
        Some(s) if s.enabled => json!({
            "text": format!("⌨ {}%", s.volume),
            "alt": "on",
            "class": "on",
            "percentage": s.volume,
            "tooltip": format!("typer: on\nPack: {}\nVolume: {}%\n\nClick: menu · Right-click: toggle · Scroll: volume", s.pack_name, s.volume),
        }),
        Some(s) => json!({
            "text": "⌨ off",
            "alt": "off",
            "class": "off",
            "percentage": s.volume,
            "tooltip": format!("typer: off\nPack: {}", s.pack_name),
        }),
        None => {
            json!({ "text": "⌨ ‒", "alt": "down", "class": "down", "tooltip": "typerd is not running" })
        }
    }
}

/// Stream state forever, reconnecting when the daemon restarts.
fn follow(waybar: bool) -> anyhow::Result<()> {
    loop {
        let result = client::subscribe(|state| {
            let _ = print_state(&state, waybar);
        });
        if waybar {
            println!("{}", waybar_json(None));
        } else if let Err(e) = result {
            eprintln!("{e:#}");
        }
        thread::sleep(Duration::from_secs(2));
    }
}
