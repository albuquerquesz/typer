//! Global key capture straight from evdev (needs the `input` group). Only the
//! key code of a press is used, to pick a sound; nothing is logged or stored.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use evdev::{Device, EventType, Key};
use inotify::{Inotify, WatchMask};

const INPUT_DIR: &str = "/dev/input";
/// Remappers like keyd re-emit every key from a virtual device; ignore the echo.
const DEDUP_WINDOW_MS: u64 = 15;

pub fn spawn(on_press: impl Fn(u16) + Send + Sync + 'static) -> anyhow::Result<()> {
    let watcher = Arc::new(Watcher {
        open: Mutex::new(HashSet::new()),
        on_press: Box::new(on_press),
        last: AtomicU64::new(0),
        epoch: Instant::now(),
    });
    watcher.scan();

    let mut inotify = Inotify::init()?;
    inotify.watches().add(INPUT_DIR, WatchMask::CREATE | WatchMask::ATTRIB)?;
    thread::Builder::new().name("hotplug".into()).spawn(move || {
        let mut buf = [0; 1024];
        loop {
            match inotify.read_events_blocking(&mut buf) {
                Ok(_) => {
                    // udev applies group permissions shortly after the node appears.
                    thread::sleep(Duration::from_millis(250));
                    watcher.scan();
                }
                Err(e) => {
                    eprintln!("typerd: inotify failed, hotplug disabled: {e}");
                    return;
                }
            }
        }
    })?;
    Ok(())
}

struct Watcher {
    open: Mutex<HashSet<PathBuf>>,
    on_press: Box<dyn Fn(u16) + Send + Sync>,
    /// (key code << 48) | ms since `epoch` of the last accepted press.
    last: AtomicU64,
    epoch: Instant,
}

impl Watcher {
    fn scan(self: &Arc<Self>) {
        let Ok(entries) = std::fs::read_dir(INPUT_DIR) else { return };
        for path in entries.flatten().map(|e| e.path()) {
            let is_event = path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("event"));
            if !is_event || self.open.lock().unwrap().contains(&path) {
                continue;
            }
            let Ok(device) = Device::open(&path) else { continue };
            if !is_keyboard(&device) {
                continue;
            }
            eprintln!("typerd: listening on {} ({})", path.display(), device.name().unwrap_or("?"));
            self.open.lock().unwrap().insert(path.clone());
            let this = self.clone();
            let _ = thread::Builder::new().name("evdev".into()).spawn(move || this.listen(&path, device));
        }
    }

    fn listen(&self, path: &Path, mut device: Device) {
        loop {
            let events = match device.fetch_events() {
                Ok(events) => events,
                Err(_) => break, // device unplugged
            };
            for ev in events {
                // value: 1 = press, 0 = release, 2 = autorepeat
                if ev.event_type() == EventType::KEY && ev.value() == 1 && !self.is_echo(ev.code()) {
                    (self.on_press)(ev.code());
                }
            }
        }
        eprintln!("typerd: {} went away", path.display());
        self.open.lock().unwrap().remove(path);
    }

    fn is_echo(&self, code: u16) -> bool {
        let now = self.epoch.elapsed().as_millis() as u64 & 0xFFFF_FFFF_FFFF;
        let packed = (u64::from(code) << 48) | now;
        let prev = self.last.swap(packed, Ordering::Relaxed);
        prev >> 48 == u64::from(code) && now.saturating_sub(prev & 0xFFFF_FFFF_FFFF) < DEDUP_WINDOW_MS
    }
}

fn is_keyboard(device: &Device) -> bool {
    device
        .supported_keys()
        .is_some_and(|keys| keys.contains(Key::KEY_A) && keys.contains(Key::KEY_ENTER))
}
