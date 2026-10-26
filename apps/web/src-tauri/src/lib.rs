//! Control popup. It is short-lived on purpose: it opens from Waybar, talks to
//! `typerd` over its socket, and exits on blur so no WebView stays resident.

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State as TauriState, WindowEvent};
use typer_core::{PackInfo, Request, Response, State, client, paths};

type CmdResult<T> = Result<T, String>;

fn state(req: Request) -> CmdResult<State> {
  client::request_state(&req).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn get_state() -> CmdResult<State> {
  state(Request::Status)
}

#[tauri::command]
fn set_enabled(enabled: bool) -> CmdResult<State> {
  state(Request::SetEnabled { enabled })
}

#[tauri::command]
fn set_volume(volume: u8) -> CmdResult<State> {
  state(Request::SetVolume { volume })
}

#[tauri::command]
fn set_pack(id: String) -> CmdResult<State> {
  state(Request::SetPack { id })
}

#[tauri::command]
fn preview() -> CmdResult<State> {
  state(Request::Preview)
}

#[tauri::command]
fn list_packs() -> CmdResult<Vec<PackInfo>> {
  match client::request(&Request::ListPacks).map_err(|e| format!("{e:#}"))? {
    Response::Packs { packs } => Ok(packs),
    other => Err(format!("unexpected response: {other:?}")),
  }
}

#[tauri::command]
fn open_packs_dir() -> CmdResult<()> {
  let dir = paths::packs_dir();
  std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
  Command::new("xdg-open").arg(&dir).spawn().map_err(|e| e.to_string())?;
  Ok(())
}

/// Set once the pointer has entered the popup. Until then, losing focus does not
/// close it: Hyprland hands focus back to the window under the cursor right after
/// the popup maps, before the user has had a chance to reach it.
struct AutoClose(Arc<AtomicBool>);

#[tauri::command]
fn arm_autoclose(armed: TauriState<AutoClose>) {
  armed.0.store(true, Ordering::Relaxed);
}

#[tauri::command]
fn close(app: AppHandle) {
  app.exit(0);
}

/// Forward daemon state changes (from typerctl, binds, Waybar) to the UI.
fn watch_state(app: AppHandle) {
  thread::spawn(move || {
    loop {
      let _ = client::subscribe(|s| {
        let _ = app.emit("state-changed", Some(s));
      });
      let _ = app.emit("state-changed", None::<State>);
      thread::sleep(Duration::from_secs(2));
    }
  });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let armed = Arc::new(AtomicBool::new(false));
  let close_on_blur = armed.clone();
  tauri::Builder::default()
    .manage(AutoClose(armed))
    // Launching again while open (Waybar click) closes the popup: a toggle.
    .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| app.exit(0)))
    .invoke_handler(tauri::generate_handler![
      get_state,
      set_enabled,
      set_volume,
      set_pack,
      preview,
      list_packs,
      open_packs_dir,
      arm_autoclose,
      close
    ])
    .setup(|app| {
      watch_state(app.handle().clone());
      if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_focus();
      }
      Ok(())
    })
    .on_window_event(move |window, event| {
      if let WindowEvent::Focused(false) = event
        && close_on_blur.load(Ordering::Relaxed)
      {
        window.app_handle().exit(0);
      }
    })
    .run(tauri::generate_context!())
    .expect("error while running typer popup");
}
