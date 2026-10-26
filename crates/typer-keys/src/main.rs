//! Key overlay: a transparent, click-through layer surface that shows the keys
//! being pressed. Started and stopped by typerd; keys come from its socket.
//!
//! In move mode (from the tray menu) the surface takes input so it can be
//! dragged; dropping it saves the position through typerd.

mod history;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{cairo, gdk, glib};
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use typer_core::{Config, OverlayConfig, Request, client};

use crate::history::History;

const WIDTH: i32 = 720;
const HEIGHT: i32 = 64;
const FONT: &str = "Sans Bold 20";
const GAP: f64 = 18.0;
const PAD: f64 = 10.0;
/// Redraw rate while keys are fading.
const FRAME: Duration = Duration::from_millis(33);

enum Msg {
    Key(String),
    Moving(bool),
    Quit,
}

struct Overlay {
    window: gtk::Window,
    area: gtk::DrawingArea,
    history: RefCell<History>,
    animating: Cell<bool>,
    moving: Cell<bool>,
    /// Margins (= position on `monitor`).
    pos: Cell<(i32, i32)>,
    monitor: RefCell<Option<gdk::Monitor>>,
    /// Margins when move mode started, to restore on Esc.
    saved_pos: Cell<(i32, i32)>,
    /// Pointer position inside the surface when a drag started.
    drag: Cell<Option<(f64, f64)>>,
}

fn main() {
    gtk::init().expect("GTK needs a display");
    let config = Config::load().overlay;
    let overlay = Overlay::new(&config);

    #[allow(deprecated)] // glib 0.18's channel is the simplest bridge from threads
    let (tx, rx) = glib::MainContext::channel(glib::Priority::DEFAULT);
    let keys_tx = tx.clone();
    thread::spawn(move || {
        let _ = client::subscribe_keys(|label| {
            let _ = keys_tx.send(Msg::Key(label));
        });
        let _ = keys_tx.send(Msg::Quit);
    });
    thread::spawn(move || {
        let _ = client::subscribe(|state| {
            let _ = tx.send(Msg::Moving(state.overlay_moving));
        });
        let _ = tx.send(Msg::Quit);
    });

    let o = overlay.clone();
    rx.attach(None, move |msg| {
        match msg {
            Msg::Key(label) => o.push(label),
            Msg::Moving(moving) => o.set_moving(moving),
            Msg::Quit => {
                // typerd went away; it restarts us when it comes back.
                gtk::main_quit();
                return glib::ControlFlow::Break;
            }
        }
        glib::ControlFlow::Continue
    });

    gtk::main();
}

impl Overlay {
    fn new(config: &OverlayConfig) -> Rc<Self> {
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        if let Some(visual) = GtkWindowExt::screen(&window).and_then(|s| s.rgba_visual()) {
            window.set_visual(Some(&visual));
        }
        window.set_app_paintable(true);
        // Size request only: resizing a layer window freezes GTK painting.
        window.set_size_request(WIDTH, HEIGHT);

        window.init_layer_shell();
        window.set_namespace("typer-keys");
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::None);
        // Ignore the bar's exclusive zone: margins are from the monitor's edge.
        window.set_exclusive_zone(-1);
        window.set_anchor(Edge::Top, true);
        window.set_anchor(Edge::Left, true);

        let monitor = find_monitor(config.monitor.as_deref());
        if let Some(m) = &monitor {
            window.set_monitor(m);
        }
        let pos = if config.x >= 0 && config.y >= 0 {
            (config.x, config.y)
        } else {
            // Not placed yet: bottom-center.
            let g = monitor.as_ref().map(|m| m.geometry());
            let (w, h) = g.map_or((1920, 1080), |g| (g.width(), g.height()));
            ((w - WIDTH) / 2, h - HEIGHT - 120)
        };

        let area = gtk::DrawingArea::new();
        window.add(&area);
        window.add_events(
            gdk::EventMask::BUTTON_PRESS_MASK
                | gdk::EventMask::BUTTON_RELEASE_MASK
                | gdk::EventMask::POINTER_MOTION_MASK,
        );

        let overlay = Rc::new(Self {
            window,
            area,
            history: RefCell::new(History::default()),
            animating: Cell::new(false),
            moving: Cell::new(false),
            pos: Cell::new(pos),
            monitor: RefCell::new(monitor),
            saved_pos: Cell::new(pos),
            drag: Cell::new(None),
        });
        overlay.clamp();
        overlay.apply_pos();
        overlay.connect_signals();
        overlay.window.show_all();
        overlay.set_click_through(true);
        overlay
    }

    fn connect_signals(self: &Rc<Self>) {
        let o = self.clone();
        self.area.connect_draw(move |_, cr| {
            o.draw(cr);
            glib::Propagation::Stop
        });

        let o = self.clone();
        self.window.connect_button_press_event(move |_, ev| {
            if o.moving.get() && ev.button() == 1 {
                o.drag.set(Some(ev.position()));
            }
            glib::Propagation::Stop
        });
        let o = self.clone();
        self.window.connect_motion_notify_event(move |_, ev| {
            if !ev.state().contains(gdk::ModifierType::BUTTON1_MASK) {
                // The press was lost (e.g. the surface moved to another monitor).
                o.drag.set(None);
            }
            if let Some((sx, sy)) = o.drag.get() {
                // The surface follows the pointer, so the offset from where the
                // drag started is how far to move.
                let (x, y) = ev.position();
                let (mx, my) = o.pos.get();
                o.pos.set((mx + (x - sx) as i32, my + (y - sy) as i32));
                o.follow_monitor();
                o.clamp();
                o.apply_pos();
            }
            glib::Propagation::Stop
        });
        let o = self.clone();
        self.window.connect_button_release_event(move |_, ev| {
            if o.drag.take().is_some() && ev.button() == 1 {
                o.save();
            }
            glib::Propagation::Stop
        });
        let o = self.clone();
        self.window.connect_key_press_event(move |_, ev| {
            if o.moving.get() {
                match ev.keyval() {
                    gdk::keys::constants::Return | gdk::keys::constants::KP_Enter => o.save(),
                    gdk::keys::constants::Escape => {
                        o.pos.set(o.saved_pos.get());
                        o.apply_pos();
                        send(Request::MoveOverlay { moving: false });
                    }
                    _ => {}
                }
            }
            glib::Propagation::Stop
        });
    }

    fn push(self: &Rc<Self>, label: String) {
        self.history.borrow_mut().push(label, Instant::now());
        self.area.queue_draw();
        self.start_animation();
    }

    /// Redraw at FRAME while anything is visible; idle otherwise.
    fn start_animation(self: &Rc<Self>) {
        if self.animating.replace(true) {
            return;
        }
        let o = self.clone();
        glib::timeout_add_local(FRAME, move || {
            let visible = o.history.borrow_mut().prune(Instant::now());
            o.area.queue_draw();
            if visible {
                glib::ControlFlow::Continue
            } else {
                o.animating.set(false);
                glib::ControlFlow::Break
            }
        });
    }

    fn set_moving(&self, moving: bool) {
        if self.moving.replace(moving) == moving {
            return;
        }
        if moving {
            self.saved_pos.set(self.pos.get());
        }
        self.drag.set(None);
        self.set_click_through(!moving);
        self.window.set_keyboard_mode(if moving {
            KeyboardMode::OnDemand
        } else {
            KeyboardMode::None
        });
        if moving {
            self.window.present();
        }
        self.area.queue_draw();
    }

    fn set_click_through(&self, on: bool) {
        // An empty input region lets clicks fall through to what is below.
        let region = on.then(cairo::Region::create);
        self.window.input_shape_combine_region(region.as_ref());
    }

    /// Dragged past its monitor's edge: hop to the monitor under its center.
    /// Layer surfaces belong to one output, so this remaps the surface (and
    /// ends the drag; the user grabs it again there).
    fn follow_monitor(&self) {
        let Some(current) = self.monitor.borrow().clone() else {
            return;
        };
        let g = current.geometry();
        let (x, y) = self.pos.get();
        let (cx, cy) = (g.x() + x + WIDTH / 2, g.y() + y + HEIGHT / 2);
        let Some(display) = gdk::Display::default() else {
            return;
        };
        let Some(next) = (0..display.n_monitors())
            .filter_map(|i| display.monitor(i))
            .find(|m| contains(&m.geometry(), cx, cy))
        else {
            return;
        };
        if next == current {
            return;
        }
        let n = next.geometry();
        self.pos.set((g.x() + x - n.x(), g.y() + y - n.y()));
        self.window.set_monitor(&next);
        *self.monitor.borrow_mut() = Some(next);
        self.drag.set(None);
    }

    /// Keep the whole overlay on its monitor.
    fn clamp(&self) {
        let Some(g) = self.monitor.borrow().as_ref().map(|m| m.geometry()) else {
            return;
        };
        let (x, y) = self.pos.get();
        self.pos.set((
            x.clamp(0, (g.width() - WIDTH).max(0)),
            y.clamp(0, (g.height() - HEIGHT).max(0)),
        ));
    }

    fn apply_pos(&self) {
        let (x, y) = self.pos.get();
        self.window.set_layer_shell_margin(Edge::Left, x);
        self.window.set_layer_shell_margin(Edge::Top, y);
    }

    fn save(&self) {
        let (x, y) = self.pos.get();
        let monitor = self.monitor.borrow().as_ref().and_then(monitor_name);
        send(Request::SetOverlayPosition { monitor, x, y });
    }

    fn draw(&self, cr: &cairo::Context) {
        cr.set_operator(cairo::Operator::Source);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        let _ = cr.paint();
        cr.set_operator(cairo::Operator::Over);

        if self.moving.get() {
            draw_move_frame(cr);
            draw_text(
                cr,
                "Arraste para posicionar · Enter salva · Esc cancela",
                PAD,
                1.0,
            );
            return;
        }

        let now = Instant::now();
        let history = self.history.borrow();
        let layouts: Vec<_> = history
            .items
            .iter()
            .map(|item| {
                let layout = pangocairo::functions::create_layout(cr);
                layout.set_font_description(Some(&gtk::pango::FontDescription::from_string(FONT)));
                layout.set_text(&item.text());
                let width = f64::from(layout.pixel_size().0);
                (layout, width, item.alpha(now))
            })
            .collect();

        // Newest on the right; drop the oldest ones that don't fit.
        let mut total = PAD;
        let first = layouts
            .iter()
            .rposition(|(_, w, _)| {
                total += w + GAP;
                total > f64::from(WIDTH) - PAD
            })
            .map_or(0, |i| i + 1);
        let mut x = PAD;
        for (layout, width, alpha) in &layouts[first..] {
            draw_layout(cr, layout, x, *alpha);
            x += width + GAP;
        }
    }
}

/// White text with a dark outline and soft shadow: readable on any background
/// without a box behind it.
fn draw_layout(cr: &cairo::Context, layout: &gtk::pango::Layout, x: f64, alpha: f64) {
    let y = (f64::from(HEIGHT) - f64::from(layout.pixel_size().1)) / 2.0;
    cr.save().ok();
    cr.move_to(x + 1.0, y + 2.0);
    pangocairo::functions::layout_path(cr, layout);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.35 * alpha);
    let _ = cr.fill();

    cr.move_to(x, y);
    pangocairo::functions::layout_path(cr, layout);
    cr.set_line_join(cairo::LineJoin::Round);
    cr.set_line_width(4.0);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.7 * alpha);
    let _ = cr.stroke_preserve();
    cr.set_source_rgba(1.0, 1.0, 1.0, alpha);
    let _ = cr.fill();
    cr.restore().ok();
}

fn draw_text(cr: &cairo::Context, text: &str, x: f64, alpha: f64) {
    let layout = pangocairo::functions::create_layout(cr);
    layout.set_font_description(Some(&gtk::pango::FontDescription::from_string(
        "Sans Bold 13",
    )));
    layout.set_text(text);
    draw_layout(cr, &layout, x, alpha);
}

fn draw_move_frame(cr: &cairo::Context) {
    let (w, h) = (f64::from(WIDTH), f64::from(HEIGHT));
    cr.rectangle(1.0, 1.0, w - 2.0, h - 2.0);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.25);
    let _ = cr.fill_preserve();
    cr.set_dash(&[6.0, 4.0], 0.0);
    cr.set_line_width(2.0);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    let _ = cr.stroke();
    cr.set_dash(&[], 0.0);
}

fn send(req: Request) {
    // Off the UI thread: typerd answers by broadcasting the new state.
    thread::spawn(move || {
        if let Err(e) = client::request(&req) {
            eprintln!("typer-keys: {e:#}");
        }
    });
}

fn contains(g: &gdk::Rectangle, x: i32, y: i32) -> bool {
    (g.x()..g.x() + g.width()).contains(&x) && (g.y()..g.y() + g.height()).contains(&y)
}

/// GDK 3 on Wayland only exposes the output's model (e.g. `0x1646`), which
/// tells the user's monitors apart well enough to restore the position.
fn monitor_name(m: &gdk::Monitor) -> Option<String> {
    m.model().map(|s| s.to_string())
}

fn find_monitor(name: Option<&str>) -> Option<gdk::Monitor> {
    let display = gdk::Display::default()?;
    let monitors: Vec<_> = (0..display.n_monitors())
        .filter_map(|i| display.monitor(i))
        .collect();
    name.and_then(|n| {
        monitors
            .iter()
            .find(|m| monitor_name(m).as_deref() == Some(n))
            .cloned()
    })
    .or_else(|| display.primary_monitor())
    .or_else(|| monitors.into_iter().next())
}
