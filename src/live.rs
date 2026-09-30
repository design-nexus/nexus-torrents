//! The UI side of the engine: sends commands, receives updates, keeps speed
//! history for the graphs, and tells visible widgets to refresh.

use crate::engine::{self, Command, Event, Update};
use crate::prefs;
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

type Callback = Rc<dyn Fn(&Update)>;
type EventCallback = Rc<dyn Fn(&Event)>;

#[derive(Default)]
struct Live {
    engine: Option<engine::Handle>,
    latest: Option<Rc<Update>>,
    history: HashMap<String, VecDeque<f64>>,
    subs: Vec<(glib::WeakRef<gtk::Widget>, Callback)>,
    always: Vec<Callback>,
    events: Vec<EventCallback>,
}

thread_local! {
    static LIVE: RefCell<Live> = RefCell::new(Live::default());
    static SETTINGS_PENDING: Cell<bool> = const { Cell::new(false) };
    static ALT_NOW: Cell<bool> = const { Cell::new(false) };
}

/// How many samples a graph shows.
pub fn capacity() -> usize {
    let p = prefs::get();
    ((p.history_secs * 1000) / p.interval_ms.max(250)) as usize + 1
}

/// Whether the alternative speed limits apply right now (manually or by schedule).
pub fn alt_active() -> bool {
    let now = glib::DateTime::now_local().ok();
    let (weekday, minute) = now.map(|d| ((d.day_of_week() - 1) as u32, (d.hour() * 60 + d.minute()) as u32)).unwrap_or((0, 0));
    engine::settings::alt_active(&prefs::get(), weekday, minute)
}

fn pack() -> Vec<engine::Kv> {
    let alt = alt_active();
    ALT_NOW.with(|a| a.set(alt));
    engine::settings::pack(&prefs::get(), alt)
}

pub fn start() {
    let (handle, rx) = engine::start(pack());
    LIVE.with(|l| l.borrow_mut().engine = Some(handle));
    glib::spawn_future_local(async move {
        while let Ok(update) = rx.recv().await {
            receive(update);
        }
    });
    // The speed schedule: re-check every half minute.
    glib::timeout_add_seconds_local(30, || {
        if alt_active() != ALT_NOW.with(|a| a.get()) {
            apply_settings_now();
            poke_all();
        }
        glib::ControlFlow::Continue
    });
}

pub fn send(cmd: Command) {
    LIVE.with(|l| {
        if let Some(e) = &l.borrow().engine {
            e.send(cmd);
        }
    });
}

/// Re-send the settings ~450 ms after the last change (typing a port, dragging a limit).
pub fn apply_settings() {
    if !SETTINGS_PENDING.with(|p| p.replace(true)) {
        glib::timeout_add_local_once(std::time::Duration::from_millis(450), || {
            SETTINGS_PENDING.with(|p| p.set(false));
            apply_settings_now();
        });
    }
}

pub fn apply_settings_now() {
    send(Command::Settings(pack()));
}

/// Save everything and stop the engine. Blocks up to ~12 s while libtorrent
/// hands over resume data.
pub fn shutdown() {
    let engine = LIVE.with(|l| l.borrow_mut().engine.take());
    if let Some(e) = engine {
        let (tx, rx) = std::sync::mpsc::channel();
        e.send(Command::Shutdown(tx));
        let _ = rx.recv_timeout(std::time::Duration::from_secs(12));
    }
    crate::store::flush();
}

pub fn latest() -> Option<Rc<Update>> {
    LIVE.with(|l| l.borrow().latest.clone())
}

pub fn history(key: &str) -> Vec<f64> {
    LIVE.with(|l| l.borrow().history.get(key).map(|v| v.iter().copied().collect()).unwrap_or_default())
}

/// Drop history beyond the current capacity (after the length changes).
pub fn trim() {
    let cap = capacity();
    LIVE.with(|l| {
        for v in l.borrow_mut().history.values_mut() {
            while v.len() > cap {
                v.pop_front();
            }
        }
    });
}

/// Call `f` with each update while `widget` is on screen, and once whenever it
/// comes back on screen. Stops by itself when the widget is destroyed.
pub fn on_tick<W: IsA<gtk::Widget>>(widget: &W, f: impl Fn(&Update) + 'static) {
    let f: Callback = Rc::new(f);
    let g = f.clone();
    widget.connect_map(move |_| {
        if let Some(u) = latest() {
            g(&u);
        }
    });
    if let Some(u) = latest() {
        f(&u);
    }
    let weak = widget.upcast_ref::<gtk::Widget>().downgrade();
    LIVE.with(|l| l.borrow_mut().subs.push((weak, f)));
}

/// Call `f` with every update, visible or not (the model, policies).
pub fn on_update(f: impl Fn(&Update) + 'static) {
    LIVE.with(|l| l.borrow_mut().always.push(Rc::new(f)));
}

pub fn on_event(f: impl Fn(&Event) + 'static) {
    LIVE.with(|l| l.borrow_mut().events.push(Rc::new(f)));
}

/// Re-run the visible widgets' refresh with the latest update (after a local change).
pub fn poke_all() {
    if let Some(u) = latest() {
        dispatch(&u, false);
    }
}

fn push(history: &mut HashMap<String, VecDeque<f64>>, cap: usize, key: String, value: f64) {
    let v = history.entry(key).or_default();
    v.push_back(value);
    while v.len() > cap {
        v.pop_front();
    }
}

fn record(history: &mut HashMap<String, VecDeque<f64>>, u: &Update) {
    let cap = capacity();
    push(history, cap, "dl".into(), u.stats.dl_rate as f64);
    push(history, cap, "ul".into(), u.stats.ul_rate as f64);
    let mut alive = std::collections::HashSet::new();
    for t in &u.torrents {
        push(history, cap, format!("t.{}.dl", t.id), t.dl_rate as f64);
        push(history, cap, format!("t.{}.ul", t.id), t.ul_rate as f64);
        alive.insert(format!("t.{}.dl", t.id));
        alive.insert(format!("t.{}.ul", t.id));
    }
    history.retain(|k, _| !k.starts_with("t.") || alive.contains(k));
}

fn dispatch(u: &Update, all: bool) {
    let (always, callbacks): (Vec<Callback>, Vec<Callback>) = LIVE.with(|l| {
        let mut l = l.borrow_mut();
        l.subs.retain(|(w, _)| w.upgrade().is_some());
        let subs = l.subs.iter().filter(|(w, _)| w.upgrade().is_some_and(|w| w.is_mapped())).map(|(_, f)| f.clone()).collect();
        (if all { l.always.clone() } else { Vec::new() }, subs)
    });
    for f in always {
        f(u);
    }
    for f in callbacks {
        f(u);
    }
}

fn receive(update: Update) {
    let update = Rc::new(update);
    let handlers = LIVE.with(|l| {
        let mut l = l.borrow_mut();
        record(&mut l.history, &update);
        l.latest = Some(update.clone());
        l.events.clone()
    });
    for e in &update.events {
        for h in &handlers {
            h(e);
        }
    }
    dispatch(&update, true);
}
