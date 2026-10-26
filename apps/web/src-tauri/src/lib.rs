//! Control popup. It is short-lived on purpose: it opens from the tray icon,
//! talks to `typerd` over its socket, and exits on blur so no WebView stays
//! resident. On Wayland it is a layer-shell surface anchored below the bar, so
//! it behaves like a menu and needs no compositor window rules.

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use tauri::{
  AppHandle, Emitter, LogicalSize, Manager, State as TauriState, WebviewWindow, WebviewWindowBuilder,
  WindowEvent,
};
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

/// Fixed menu width; the height follows the content (see `fit`).
const MENU_WIDTH: f64 = 260.0;

/// Resize the window to the rendered menu height.
#[tauri::command]
fn fit(app: AppHandle, window: WebviewWindow, height: f64) -> CmdResult<()> {
  let height = height.ceil();
  #[cfg(target_os = "linux")]
  app
    .run_on_main_thread(move || {
      if !menu::resize(MENU_WIDTH as i32, height as i32) {
        let _ = window.set_size(LogicalSize::new(MENU_WIDTH, height));
      }
    })
    .map_err(|e| e.to_string())?;
  #[cfg(not(target_os = "linux"))]
  {
    let _ = app;
    window.set_size(LogicalSize::new(MENU_WIDTH, height)).map_err(|e| e.to_string())?;
  }
  Ok(())
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

/// `--at X,Y`: where the tray icon was clicked, passed on by typerd.
fn click_position() -> Option<(i32, i32)> {
  let mut args = std::env::args().skip_while(|a| a != "--at").skip(1);
  let (x, y) = args.next()?.split_once(',').map(|(x, y)| (x.parse().ok(), y.parse().ok()))?;
  Some((x?, y?))
}

#[cfg(target_os = "linux")]
mod menu {
  use std::cell::RefCell;

  use gtk::prelude::*;
  use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
  use tauri::WebviewWindow;

  /// Gap between the bar and the menu, and the fallback left offset.
  const GAP: i32 = 6;
  /// Nudge right of the click so the menu doesn't sit under the pointer.
  const NUDGE: i32 = 6;

  thread_local! {
    static LAYER: RefCell<Option<gtk::ApplicationWindow>> = const { RefCell::new(None) };
  }

  /// Show the webview in a layer surface pinned to the top-left of the output
  /// under the cursor. A zero exclusive zone makes the compositor place it below
  /// the bar, so only the horizontal offset needs computing.
  ///
  /// tao's own window never paints once turned into a layer surface, so the
  /// webview's box moves into a fresh GTK window and tao's stays hidden. That
  /// window gets no tao events, hence `on_blur`. Returns false (keep the plain
  /// window) when not on Wayland.
  pub fn show(window: &WebviewWindow, at: Option<(i32, i32)>, on_blur: impl Fn() + 'static) -> bool {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
      return false;
    }
    let (Ok(old), Ok(vbox)) = (window.gtk_window(), window.default_vbox()) else {
      return false;
    };
    let Some(app) = old.application() else { return false };

    let layer = gtk::ApplicationWindow::new(&app);
    if let Some(visual) = GtkWindowExt::screen(&layer).and_then(|s| s.rgba_visual()) {
      layer.set_visual(Some(&visual));
    }
    layer.set_app_paintable(true);
    old.remove(&vbox);
    layer.add(&vbox);

    layer.init_layer_shell();
    layer.set_namespace("typer");
    layer.set_layer(Layer::Overlay);
    layer.set_keyboard_mode(KeyboardMode::OnDemand);
    layer.set_exclusive_zone(0);
    layer.set_anchor(Edge::Top, true);
    layer.set_anchor(Edge::Left, true);
    layer.set_layer_shell_margin(Edge::Top, GAP);
    let left = at.map_or(GAP, |(x, _)| (x + NUDGE).max(0));
    layer.set_layer_shell_margin(Edge::Left, left);
    let (w, h) = window.inner_size().map_or((260, 200), |s| (s.width as i32, s.height as i32));
    layer.set_size_request(w, h);
    layer.connect_focus_out_event(move |_, _| {
      on_blur();
      gtk::glib::Propagation::Proceed
    });
    layer.show_all();
    LAYER.with(|l| *l.borrow_mut() = Some(layer));
    true
  }

  /// Follow the menu's height. Must run on the GTK main thread. Returns false
  /// when there is no layer window (plain window fallback).
  pub fn resize(width: i32, height: i32) -> bool {
    LAYER.with(|l| {
      if let Some(layer) = l.borrow().as_ref() {
        // Size request only: gtk_window_resize() on a layer window makes GTK
        // freeze painting until a configure that never comes.
        layer.set_size_request(width, height);
        true
      } else {
        false
      }
    })
  }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let armed = Arc::new(AtomicBool::new(false));
  let close_on_blur = armed.clone();
  tauri::Builder::default()
    .manage(AutoClose(armed.clone()))
    // Launching again while open (tray click) closes the popup: a toggle.
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
      fit,
      close
    ])
    .setup(move |app| {
      watch_state(app.handle().clone());
      let config = &app.config().app.windows[0];
      let window = WebviewWindowBuilder::from_config(app.handle(), config)?.build()?;
      #[cfg(target_os = "linux")]
      {
        let handle = app.handle().clone();
        let on_blur = move || {
          if armed.load(Ordering::Relaxed) {
            handle.exit(0);
          }
        };
        if menu::show(&window, click_position(), on_blur) {
          return Ok(());
        }
      }
      window.show()?;
      window.set_focus()?;
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
