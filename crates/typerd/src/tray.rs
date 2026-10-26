//! System tray icon (StatusNotifierItem) for Waybar's `tray` module.
//!
//! Deliberately exposes no dbusmenu: without a `Menu` property, Waybar calls
//! `ContextMenu` on right-click, which opens the typer popup.
//!   left click → on/off · right click → popup · middle click → preview · scroll → volume

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use zbus::blocking::{Connection, connection, fdo::DBusProxy};
use zbus::interface;
use zbus::names::BusName;

use crate::{Daemon, icon};

const ITEM_PATH: &str = "/StatusNotifierItem";
const ITEM_IFACE: &str = "org.kde.StatusNotifierItem";
const WATCHER: &str = "org.kde.StatusNotifierWatcher";

/// SNI `ToolTip`: (icon name, icon pixmaps as (w, h, ARGB32), title, description)
type ToolTip = (String, Vec<(i32, i32, Vec<u8>)>, String, String);

struct Item {
    daemon: Arc<Daemon>,
    /// Last menu toggle, to collapse repeated ContextMenu calls (see `context_menu`).
    last_menu: Mutex<Option<Instant>>,
}

/// Waybar sends ContextMenu several times per right-click; since the popup
/// toggles, an even count would open and close it in the same instant.
const MENU_DEBOUNCE: Duration = Duration::from_millis(300);

/// D-Bus handlers run on zbus' executor; anything that may emit signals back
/// through the same connection must leave it first.
fn detached(daemon: &Arc<Daemon>, f: impl FnOnce(&Arc<Daemon>) + Send + 'static) {
    let d = daemon.clone();
    thread::spawn(move || f(&d));
}

#[interface(name = "org.kde.StatusNotifierItem")]
impl Item {
    #[zbus(property)]
    fn category(&self) -> &str {
        "Hardware"
    }

    #[zbus(property)]
    fn id(&self) -> &str {
        "typer"
    }

    #[zbus(property)]
    fn title(&self) -> &str {
        "typer"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }

    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        false
    }

    /// Empty so hosts use our pixmap: theme symbolic icons render dark in Waybar.
    #[zbus(property)]
    fn icon_name(&self) -> &str {
        ""
    }

    #[zbus(property)]
    fn icon_pixmap(&self) -> Vec<icon::Pixmap> {
        icon::pixmaps(self.daemon.enabled())
    }

    #[zbus(property)]
    fn tool_tip(&self) -> ToolTip {
        let s = self.daemon.state();
        let body = if s.enabled {
            format!("{} · {}%", s.pack_name, s.volume)
        } else {
            format!("Desligado · {}", s.pack_name)
        };
        (String::new(), Vec::new(), "typer".into(), body)
    }

    fn activate(&self, _x: i32, _y: i32) {
        detached(&self.daemon, |d| {
            d.update(|c| c.enabled = !c.enabled);
        });
    }

    fn secondary_activate(&self, _x: i32, _y: i32) {
        detached(&self.daemon, |d| d.preview());
    }

    fn context_menu(&self, x: i32, y: i32) {
        let mut last = self.last_menu.lock().unwrap();
        if last.is_some_and(|t| t.elapsed() < MENU_DEBOUNCE) {
            return;
        }
        *last = Some(Instant::now());
        eprintln!("typerd: menu at {x},{y}");
        if let Err(e) = typer_core::popup::toggle(Some((x, y))) {
            eprintln!("typerd: could not open popup: {e:#}");
        }
    }

    fn scroll(&self, delta: i32, orientation: &str) {
        if !orientation.eq_ignore_ascii_case("vertical") || delta == 0 {
            return;
        }
        // Waybar sends -1 for scroll up.
        let step: i16 = if delta < 0 { 5 } else { -5 };
        detached(&self.daemon, move |d| {
            d.update(|c| c.volume = (i16::from(c.volume) + step).clamp(0, 100) as u8);
        });
    }
}

/// Publish the tray icon and keep it registered across Waybar restarts.
pub fn spawn(daemon: Arc<Daemon>) -> anyhow::Result<Connection> {
    let name = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
    let conn = connection::Builder::session()?
        .name(name.as_str())?
        .serve_at(ITEM_PATH, Item { daemon, last_menu: Mutex::new(None) })?
        .build()?;

    register(&conn, &name);

    let watcher_conn = conn.clone();
    thread::Builder::new().name("tray".into()).spawn(move || {
        let watch = || -> zbus::Result<()> {
            let dbus = DBusProxy::new(&watcher_conn)?;
            let watcher = BusName::try_from(WATCHER)?;
            for change in dbus.receive_name_owner_changed_with_args(&[(0, WATCHER)])? {
                let args = change.args()?;
                if args.name() == &watcher && args.new_owner().is_some() {
                    register(&watcher_conn, &name);
                }
            }
            Ok(())
        };
        if let Err(e) = watch() {
            eprintln!("typerd: tray watcher stopped: {e}");
        }
    })?;
    Ok(conn)
}

fn register(conn: &Connection, name: &str) {
    let result = conn.call_method(Some(WATCHER), "/StatusNotifierWatcher", Some(WATCHER), "RegisterStatusNotifierItem", &name);
    match result {
        Ok(_) => eprintln!("typerd: tray icon registered"),
        // No tray host yet (Waybar not running); we'll register when it appears.
        Err(e) => eprintln!("typerd: no tray host yet ({e})"),
    }
}

/// Ask the tray host to re-read icon and tooltip after a state change.
pub fn notify(conn: &Connection) {
    for signal in ["NewIcon", "NewToolTip"] {
        let _ = conn.emit_signal(None::<&str>, ITEM_PATH, ITEM_IFACE, signal, &());
    }
}
