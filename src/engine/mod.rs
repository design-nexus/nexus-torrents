//! The engine thread. It owns the libtorrent session (through the C++ shim),
//! takes [`Command`]s from the UI, and once a second sends an [`Update`] back
//! over a channel. It also keeps fast-resume data, the queue order and the DHT
//! state on disk. Nothing here touches GTK.

pub mod ffi;
pub mod settings;

pub use ffi::{AddParams, Details, FileEntry, Kv, Peer, Stats, Status, TorrentInfo, Tracker};

use crate::paths;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub enum Event {
    AddFailed {
        id: String,
        error: String,
    },
    /// A magnet fetched for the add dialog now has its .torrent.
    Metadata {
        id: String,
        name: String,
        torrent: Vec<u8>,
    },
    MetadataReceived {
        id: String,
    },
    Finished {
        id: String,
        name: String,
    },
    Error {
        id: String,
        text: String,
    },
    Removed {
        id: String,
    },
    Moved {
        id: String,
    },
    MoveFailed {
        id: String,
        text: String,
    },
    DeleteFailed {
        id: String,
        text: String,
    },
    Warning {
        text: String,
    },
    /// Every torrent from the last session has been added back.
    Restored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Queue {
    Up = 0,
    Down = 1,
    Top = 2,
    Bottom = 3,
}

pub enum Command {
    Settings(Vec<Kv>),
    Add(Box<AddParams>, String),
    Fetch {
        magnet: String,
    },
    CancelFetch(String),
    Remove {
        ids: Vec<String>,
        with_files: bool,
    },
    Pause(Vec<String>),
    Resume {
        ids: Vec<String>,
        force: bool,
    },
    Recheck(Vec<String>),
    Reannounce(Vec<String>),
    Queue(Vec<String>, Queue),
    FilePriorities(String, Vec<u8>),
    Limits(Vec<String>, i32, i32),
    Sequential(Vec<String>, bool),
    FirstLast(Vec<String>, bool),
    SuperSeed(Vec<String>, bool),
    Move(Vec<String>, String),
    Rename(String, String),
    AddTracker(String, String),
    RemoveTracker(String, String),
    /// Which torrent the details pane shows (and whether it wants peers).
    Select(Option<String>, bool),
    Export(String, PathBuf),
    Shutdown(mpsc::Sender<()>),
}

#[derive(Debug, Clone, Default)]
pub struct Update {
    pub torrents: Vec<Status>,
    pub stats: Stats,
    pub events: Vec<Event>,
    pub details: Option<Details>,
}

pub struct Handle {
    tx: mpsc::Sender<Command>,
}

impl Handle {
    pub fn send(&self, cmd: Command) {
        let _ = self.tx.send(cmd);
    }
}

pub fn start(settings: Vec<Kv>) -> (Handle, async_channel::Receiver<Update>) {
    let (tx, rx) = mpsc::channel();
    let (utx, urx) = async_channel::unbounded();
    std::thread::Builder::new()
        .name("engine".into())
        .spawn(move || run(rx, utx, settings))
        .expect("could not start the engine thread");
    (Handle { tx }, urx)
}

fn resume_file(id: &str) -> PathBuf {
    paths::resume_dir().join(format!("{id}.fastresume"))
}

fn write_bytes(path: &std::path::Path, data: &[u8]) {
    let Some(dir) = path.parent() else { return };
    let _ = std::fs::create_dir_all(dir);
    let tmp = dir.join(format!(".{}.tmp", path.file_name().unwrap_or_default().to_string_lossy()));
    if std::fs::write(&tmp, data).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

fn event(e: ffi::Event) -> Option<Event> {
    let (id, text) = (e.id, e.text);
    Some(match e.kind {
        2 => Event::AddFailed { id, error: text },
        3 => Event::Metadata { id, name: text, torrent: e.data },
        4 => Event::MetadataReceived { id },
        5 => Event::Finished { id, name: text },
        6 => Event::Error { id, text },
        8 => Event::Removed { id },
        10 => Event::Moved { id },
        11 => Event::MoveFailed { id, text },
        13 => Event::Warning { text: format!("Can't listen for peers: {text}") },
        14 => Event::DeleteFailed { id, text },
        _ => return None,
    })
}

/// Torrents from the last session, in queue order.
fn saved_torrents() -> Vec<(String, Vec<u8>)> {
    let Ok(entries) = std::fs::read_dir(paths::resume_dir()) else { return vec![] };
    let order: Vec<String> = std::fs::read_to_string(paths::queue_file())
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let rank: HashMap<&str, usize> = order.iter().enumerate().map(|(i, id)| (id.as_str(), i)).collect();
    let mut found: Vec<(String, Vec<u8>)> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension().is_none_or(|x| x != "fastresume") {
                return None;
            }
            let id = path.file_stem()?.to_string_lossy().to_string();
            Some((id, std::fs::read(&path).ok()?))
        })
        .collect();
    found.sort_by_key(|(id, _)| rank.get(id.as_str()).copied().unwrap_or(usize::MAX));
    found
}

fn write_queue(torrents: &[Status]) {
    let mut list: Vec<&Status> = torrents.iter().collect();
    list.sort_by_key(|s| if s.queue_position >= 0 { (0, s.queue_position as i64) } else { (1, s.added) });
    let text: String = list.iter().map(|s| format!("{}\n", s.id)).collect();
    write_bytes(&paths::queue_file(), text.as_bytes());
}

struct Engine {
    ses: cxx::UniquePtr<ffi::Session>,
    events: Vec<Event>,
    selected: Option<(String, bool)>,
}

impl Engine {
    fn ses(&mut self) -> std::pin::Pin<&mut ffi::Session> {
        self.ses.pin_mut()
    }

    /// Drain libtorrent's alerts: keep resume data on disk, pass the rest on.
    fn drain(&mut self) {
        for e in self.ses().poll() {
            match e.kind {
                7 => write_bytes(&resume_file(&e.id), &e.data),
                8 => {
                    let _ = std::fs::remove_file(resume_file(&e.id));
                    if let Some(ev) = event(e) {
                        self.events.push(ev);
                    }
                }
                _ => {
                    if let Some(ev) = event(e) {
                        self.events.push(ev);
                    }
                }
            }
        }
    }

    fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::Settings(kv) => {
                let errors = self.ses().apply_settings(&kv);
                if !errors.is_empty() {
                    eprintln!("torrents: settings: {errors}");
                }
            }
            Command::Add(params, id) => match self.ses().add_torrent(&params) {
                Ok(real) => {
                    self.ses().save_resume(&real);
                }
                Err(e) => self.events.push(Event::AddFailed { id, error: e.what().to_string() }),
            },
            Command::Fetch { magnet } => {
                let tmp = std::env::temp_dir().join("torrents-metadata");
                if let Err(e) = self.ses().fetch_metadata(&magnet, &tmp.to_string_lossy()) {
                    let id = ffi::parse_magnet(&magnet).map(|i| i.id).unwrap_or_default();
                    self.events.push(Event::AddFailed { id, error: e.what().to_string() });
                }
            }
            Command::CancelFetch(id) => self.ses().cancel_fetch(&id),
            Command::Remove { ids, with_files } => {
                for id in ids {
                    self.ses().remove(&id, with_files);
                    let _ = std::fs::remove_file(resume_file(&id));
                }
            }
            Command::Pause(ids) => ids.iter().for_each(|id| self.ses().pause(id)),
            Command::Resume { ids, force } => ids.iter().for_each(|id| self.ses().resume(id, force, true)),
            Command::Recheck(ids) => ids.iter().for_each(|id| self.ses().recheck(id)),
            Command::Reannounce(ids) => ids.iter().for_each(|id| self.ses().reannounce(id)),
            Command::Queue(ids, how) => {
                // Moving several down (or to the top) goes in reverse so they keep their order.
                let reverse = matches!(how, Queue::Down | Queue::Top);
                let list: Vec<&String> = if reverse { ids.iter().rev().collect() } else { ids.iter().collect() };
                for id in list {
                    self.ses().queue_move(id, how as u8);
                }
            }
            Command::FilePriorities(id, prios) => self.ses().set_file_priorities(&id, &prios),
            Command::Limits(ids, dl, ul) => ids.iter().for_each(|id| self.ses().set_limits(id, dl, ul)),
            Command::Sequential(ids, on) => ids.iter().for_each(|id| self.ses().set_sequential(id, on)),
            Command::FirstLast(ids, on) => ids.iter().for_each(|id| self.ses().set_first_last(id, on)),
            Command::SuperSeed(ids, on) => ids.iter().for_each(|id| self.ses().set_super_seeding(id, on)),
            Command::Move(ids, path) => ids.iter().for_each(|id| self.ses().move_storage(id, &path)),
            Command::Rename(id, name) => {
                self.ses().rename(&id, &name);
                self.ses().save_resume(&id);
            }
            Command::AddTracker(id, url) => self.ses().add_tracker(&id, &url, 0),
            Command::RemoveTracker(id, url) => self.ses().remove_tracker(&id, &url),
            Command::Select(id, peers) => self.selected = id.map(|i| (i, peers)),
            Command::Export(id, path) => {
                let data = self.ses.torrent_file(&id);
                if data.is_empty() {
                    self.events
                        .push(Event::Warning { text: "This torrent has no metadata yet, so it can't be exported.".into() });
                } else {
                    write_bytes(&path, &data);
                }
            }
            Command::Shutdown(_) => {}
        }
    }

    fn update(&mut self) -> Update {
        self.ses().post_updates();
        // The status alerts arrive a moment after they're asked for.
        std::thread::sleep(Duration::from_millis(60));
        self.drain();
        let details = self.selected.clone().map(|(id, peers)| self.ses().details(&id, peers));
        Update { torrents: self.ses().statuses(), stats: self.ses().stats(), events: std::mem::take(&mut self.events), details }
    }

    /// Save every torrent's resume data and the DHT state, waiting (up to 10 s)
    /// for libtorrent to hand the resume data over.
    fn shutdown(&mut self) {
        let torrents = self.ses().statuses();
        write_queue(&torrents);
        self.ses().pause_session();
        let mut waiting = self.ses().save_all_resume(true);
        let deadline = Instant::now() + Duration::from_secs(10);
        while waiting > 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
            for e in self.ses().poll() {
                if e.kind == 7 {
                    write_bytes(&resume_file(&e.id), &e.data);
                    waiting -= 1;
                } else if e.kind == 12 {
                    waiting -= 1;
                }
            }
        }
        write_bytes(&paths::session_file(), &self.ses.session_state());
    }
}

fn run(rx: mpsc::Receiver<Command>, tx: async_channel::Sender<Update>, settings: Vec<Kv>) {
    let state = std::fs::read(paths::session_file()).unwrap_or_default();
    let ses = ffi::new_session(&settings, &state);
    let mut engine = Engine { ses, events: Vec::new(), selected: None };

    for (id, data) in saved_torrents() {
        let params = AddParams { resume: data, ..Default::default() };
        if let Err(e) = engine.ses().add_torrent(&params) {
            eprintln!("torrents: could not restore {id}: {}", e.what());
        }
    }
    engine.events.push(Event::Restored);

    let tick = Duration::from_millis(1000);
    let mut next_tick = Instant::now();
    let mut next_save = Instant::now() + Duration::from_secs(60);
    loop {
        let wait = next_tick.saturating_duration_since(Instant::now());
        match rx.recv_timeout(wait) {
            Ok(Command::Shutdown(done)) => {
                engine.shutdown();
                let _ = done.send(());
                return;
            }
            Ok(cmd) => {
                engine.handle(cmd);
                // Show the effect of a click right away rather than at the next tick.
                next_tick = next_tick.min(Instant::now() + Duration::from_millis(80));
                continue;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                engine.shutdown();
                return;
            }
        }
        let update = engine.update();
        if Instant::now() >= next_save {
            engine.ses().save_all_resume(false);
            write_queue(&update.torrents);
            next_save = Instant::now() + Duration::from_secs(60);
        }
        if tx.send_blocking(update).is_err() {
            engine.shutdown();
            return;
        }
        next_tick = Instant::now() + tick;
    }
}

#[cfg(test)]
mod tests {
    use super::ffi::{self, AddParams, Kv};
    use std::time::{Duration, Instant};

    fn settings(port: u16) -> Vec<Kv> {
        let kv = |k: &str, v: &str| Kv { key: k.into(), value: v.into() };
        vec![
            kv("listen_interfaces", &format!("127.0.0.1:{port}")),
            kv("enable_dht", "false"),
            kv("enable_lsd", "false"),
            kv("enable_upnp", "false"),
            kv("enable_natpmp", "false"),
            kv("allow_multiple_connections_per_ip", "true"),
        ]
    }

    fn wait(ses: &mut cxx::UniquePtr<ffi::Session>, secs: u64, done: impl Fn(&[ffi::Status]) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < deadline {
            ses.pin_mut().post_updates();
            std::thread::sleep(Duration::from_millis(200));
            ses.pin_mut().poll();
            if done(&ses.pin_mut().statuses()) {
                return true;
            }
        }
        false
    }

    /// Seed a file from one session and fetch it by magnet (metadata and all) in another.
    #[test]
    #[ignore = "opens local ports; run with --ignored"]
    fn downloads_from_a_local_seed() {
        let root = std::env::temp_dir().join(format!("torrents-test-{}", std::process::id()));
        let (seed_dir, leech_dir) = (root.join("seed"), root.join("leech"));
        std::fs::create_dir_all(&seed_dir).unwrap();
        std::fs::create_dir_all(&leech_dir).unwrap();
        let data: Vec<u8> = (0..3_000_000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
        let file = seed_dir.join("payload.bin");
        std::fs::write(&file, &data).unwrap();

        let torrent = ffi::create_torrent(&file.to_string_lossy(), &vec![], false).unwrap();
        let info = ffi::parse_torrent(&torrent).unwrap();
        assert_eq!(info.name, "payload.bin");
        assert_eq!(info.size, data.len() as i64);

        let mut seed = ffi::new_session(&settings(46881), &[]);
        let params =
            AddParams { torrent, save_path: seed_dir.to_string_lossy().into(), auto_managed: false, ..Default::default() };
        let id = seed.pin_mut().add_torrent(&params).unwrap();
        assert_eq!(id, info.id);
        assert!(wait(&mut seed, 20, |s| s.iter().any(|t| t.is_seeding)), "the seed never finished checking");

        let mut leech = ffi::new_session(&settings(46882), &[]);
        let magnet = format!("magnet:?xt=urn:btih:{}&x.pe=127.0.0.1:46881", info.id);
        let params =
            AddParams { magnet, save_path: leech_dir.to_string_lossy().into(), auto_managed: false, ..Default::default() };
        leech.pin_mut().add_torrent(&params).unwrap();
        let done = {
            let deadline = Instant::now() + Duration::from_secs(60);
            let mut ok = false;
            while Instant::now() < deadline {
                seed.pin_mut().post_updates();
                seed.pin_mut().poll();
                if wait(&mut leech, 1, |s| s.iter().any(|t| t.is_seeding)) {
                    ok = true;
                    break;
                }
            }
            ok
        };
        assert!(done, "the download didn't finish");
        assert_eq!(std::fs::read(leech_dir.join("payload.bin")).unwrap(), data);
        let details = leech.pin_mut().details(&info.id, true);
        assert_eq!(details.files.len(), 1);
        assert!(details.pieces.iter().all(|p| *p == 2));
        assert!(!leech.torrent_file(&info.id).is_empty());

        // Pause, resume, move the files, then remove the torrent with its data.
        leech.pin_mut().pause(&info.id);
        assert!(wait(&mut leech, 10, |s| s.iter().any(|t| t.paused && !t.auto_managed)), "didn't pause");
        leech.pin_mut().resume(&info.id, false, true);
        assert!(wait(&mut leech, 10, |s| s.iter().any(|t| !t.paused)), "didn't resume");
        let moved = root.join("moved");
        std::fs::create_dir_all(&moved).unwrap();
        leech.pin_mut().move_storage(&info.id, &moved.to_string_lossy());
        let target = moved.to_string_lossy().to_string();
        assert!(wait(&mut leech, 10, |s| s.iter().any(|t| t.save_path == target)), "didn't move");
        assert!(moved.join("payload.bin").exists());
        leech.pin_mut().remove(&info.id, true);
        assert!(leech.pin_mut().statuses().is_empty());
        let deadline = Instant::now() + Duration::from_secs(10);
        while moved.join("payload.bin").exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
            leech.pin_mut().poll();
        }
        assert!(!moved.join("payload.bin").exists(), "remove with files left the data behind");
        let _ = std::fs::remove_dir_all(&root);
    }
}
