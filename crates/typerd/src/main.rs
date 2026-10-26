mod audio;
mod icon;
mod input;
mod sounds;
mod tray;

use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::thread;
use std::time::Duration;

use anyhow::{Context, bail};
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
use signal_hook::iterator::Signals;
use typer_core::config::DEFAULT_PACK;
use typer_core::keycodes::KEY_A;
use typer_core::{Config, Request, Response, State, pack, paths};

use crate::audio::Audio;
use crate::sounds::Sounds;

struct Daemon {
    config: Mutex<Config>,
    enabled: AtomicBool,
    sounds: RwLock<Arc<Sounds>>,
    audio: Audio,
    subscribers: Mutex<Vec<UnixStream>>,
    /// Cheap per-press gain jitter so repeated keys don't sound identical.
    jitter: AtomicU32,
    /// Session bus connection of the tray icon, when a session bus exists.
    tray: OnceLock<zbus::blocking::Connection>,
}

impl Daemon {
    fn state(&self) -> State {
        let config = self.config.lock().unwrap();
        let sounds = self.sounds.read().unwrap();
        State {
            enabled: config.enabled,
            volume: config.volume,
            pack: sounds.id.clone(),
            pack_name: sounds.name.clone(),
        }
    }

    fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    fn on_press(&self, code: u16) {
        if !self.enabled() {
            return;
        }
        self.play(code);
    }

    fn play(&self, code: u16) {
        let Some(sample) = self.sounds.read().unwrap().for_key(code) else { return };
        let j = self.jitter.fetch_add(0x9E37_79B9, Ordering::Relaxed);
        let gain = 0.9 + (j >> 24) as f32 / 255.0 * 0.2;
        self.audio.trigger(sample, gain);
    }

    fn update(&self, f: impl FnOnce(&mut Config)) -> State {
        {
            let mut config = self.config.lock().unwrap();
            f(&mut config);
            config.volume = config.volume.min(100);
            self.enabled.store(config.enabled, Ordering::Relaxed);
            self.audio.set_volume(config.volume);
            self.audio.set_playing(config.enabled);
            if let Err(e) = config.save() {
                eprintln!("typerd: could not save config: {e:#}");
            }
        }
        self.broadcast()
    }

    fn broadcast(&self) -> State {
        let state = self.state();
        let line = encode(&Response::State(state.clone()));
        self.subscribers.lock().unwrap().retain_mut(|s| s.write_all(&line).is_ok());
        if let Some(conn) = self.tray.get() {
            tray::notify(conn);
        }
        state
    }

    /// Play a sample key even while disabled.
    fn preview(self: &Arc<Self>) {
        self.audio.set_playing(true);
        self.play(KEY_A);
        if !self.enabled() {
            // Let the preview ring out, then pause again if still disabled.
            let d = self.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(500));
                if !d.enabled() {
                    d.audio.set_playing(false);
                }
            });
        }
    }

    fn set_pack(&self, id: &str) -> anyhow::Result<State> {
        let sounds = Sounds::load(id, self.audio.sample_rate)?;
        *self.sounds.write().unwrap() = Arc::new(sounds);
        Ok(self.update(|c| c.pack = id.to_owned()))
    }

    fn reload(&self) {
        let id = self.config.lock().unwrap().pack.clone();
        match self.set_pack(&id) {
            Ok(_) => eprintln!("typerd: reloaded pack {id}"),
            Err(e) => eprintln!("typerd: reload of {id} failed: {e:#}"),
        }
    }

    fn handle(self: &Arc<Self>, req: Request, stream: &UnixStream) -> Option<Response> {
        Some(match req {
            Request::Status => Response::State(self.state()),
            Request::Toggle => Response::State(self.update(|c| c.enabled = !c.enabled)),
            Request::SetEnabled { enabled } => Response::State(self.update(|c| c.enabled = enabled)),
            Request::SetVolume { volume } => Response::State(self.update(|c| c.volume = volume)),
            Request::AdjustVolume { delta } => Response::State(self.update(|c| {
                c.volume = (i16::from(c.volume) + delta).clamp(0, 100) as u8;
            })),
            Request::SetPack { id } => match self.set_pack(&id) {
                Ok(state) => Response::State(state),
                Err(e) => Response::Error { message: format!("{e:#}") },
            },
            Request::ListPacks => Response::Packs { packs: pack::list() },
            Request::Preview => {
                self.preview();
                Response::State(self.state())
            }
            Request::Reload => {
                self.reload();
                Response::State(self.state())
            }
            Request::Subscribe => {
                let mut writer = stream.try_clone().ok()?;
                writer.write_all(&encode(&Response::State(self.state()))).ok()?;
                self.subscribers.lock().unwrap().push(writer);
                return None;
            }
        })
    }
}

fn encode(res: &Response) -> Vec<u8> {
    let mut line = serde_json::to_vec(res).expect("response serializes");
    line.push(b'\n');
    line
}

fn serve_client(daemon: &Arc<Daemon>, stream: UnixStream) {
    let Ok(mut writer) = stream.try_clone() else { return };
    for line in BufReader::new(&stream).lines() {
        let Ok(line) = line else { return };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(req) => daemon.handle(req, &stream),
            Err(e) => Some(Response::Error { message: format!("bad request: {e}") }),
        };
        if let Some(res) = response
            && writer.write_all(&encode(&res)).is_err() {
                return;
            }
    }
}

fn bind_socket() -> anyhow::Result<UnixListener> {
    let path = paths::socket();
    match UnixListener::bind(&path) {
        Ok(l) => Ok(l),
        Err(e) if e.kind() == ErrorKind::AddrInUse => {
            if UnixStream::connect(&path).is_ok() {
                bail!("typerd is already running ({})", path.display());
            }
            fs::remove_file(&path)?; // stale socket from a crash
            Ok(UnixListener::bind(&path)?)
        }
        Err(e) => Err(e).with_context(|| format!("binding {}", path.display())),
    }
}

fn main() -> anyhow::Result<()> {
    let listener = bind_socket()?;
    run(listener).inspect_err(|_| {
        let _ = fs::remove_file(paths::socket());
    })
}

fn run(listener: UnixListener) -> anyhow::Result<()> {
    let config = Config::load();
    let audio = Audio::start(config.volume, config.enabled).context("starting audio output")?;

    let sounds = Sounds::load(&config.pack, audio.sample_rate).or_else(|e| {
        eprintln!("typerd: pack {} unavailable ({e:#}), using {DEFAULT_PACK}", config.pack);
        Sounds::load(DEFAULT_PACK, audio.sample_rate)
    })?;
    eprintln!("typerd: {} Hz output, pack {}", audio.sample_rate, sounds.id);

    let daemon = Arc::new(Daemon {
        enabled: AtomicBool::new(config.enabled),
        config: Mutex::new(Config { pack: sounds.id.clone(), ..config }),
        sounds: RwLock::new(Arc::new(sounds)),
        audio,
        subscribers: Mutex::new(Vec::new()),
        jitter: AtomicU32::new(1),
        tray: OnceLock::new(),
    });

    match tray::spawn(daemon.clone()) {
        Ok(conn) => {
            let _ = daemon.tray.set(conn);
        }
        Err(e) => eprintln!("typerd: tray icon unavailable: {e:#}"),
    }

    let d = daemon.clone();
    input::spawn(move |code| d.on_press(code)).context("watching /dev/input")?;

    let d = daemon.clone();
    let mut signals = Signals::new([SIGHUP, SIGINT, SIGTERM])?;
    thread::spawn(move || {
        for sig in signals.forever() {
            if sig == SIGHUP {
                d.reload();
            } else {
                let _ = fs::remove_file(paths::socket());
                std::process::exit(0);
            }
        }
    });

    for stream in listener.incoming().flatten() {
        let d = daemon.clone();
        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
        thread::spawn(move || serve_client(&d, stream));
    }
    Ok(())
}
