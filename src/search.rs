//! Search plugins, in qBittorrent's format (nova3 engines). We ship our own
//! small runner and helper modules (`search/*.py`) and run one `python3`
//! process per enabled engine, reading results as JSON lines.

use crate::{cmd, paths, prefs};
use gtk::glib;
use serde::Deserialize;
use std::cell::RefCell;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

const FILES: &[(&str, &str)] = &[
    ("runner.py", include_str!("../search/runner.py")),
    ("helpers.py", include_str!("../search/helpers.py")),
    ("novaprinter.py", include_str!("../search/novaprinter.py")),
];

pub const CATEGORIES: &[(&str, &str)] = &[
    ("all", "All"),
    ("movies", "Movies"),
    ("tv", "TV shows"),
    ("music", "Music"),
    ("games", "Games"),
    ("anime", "Anime"),
    ("software", "Software"),
    ("pictures", "Pictures"),
    ("books", "Books"),
];

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Engine {
    pub id: String,
    pub name: String,
    pub url: String,
    pub categories: Vec<String>,
    pub version: String,
    pub error: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Hit {
    pub link: String,
    pub name: String,
    pub size: i64,
    pub seeds: i64,
    pub leech: i64,
    pub engine_url: String,
    pub desc_link: String,
    pub pub_date: i64,
    /// Filled in by us: which engine found it.
    #[serde(skip)]
    pub engine: String,
    #[serde(skip)]
    pub engine_id: String,
}

pub fn parse_hit(line: &str) -> Option<Hit> {
    let hit: Hit = serde_json::from_str(line.trim()).ok()?;
    (!hit.link.is_empty() && !hit.name.is_empty()).then_some(hit)
}

pub fn python() -> bool {
    cmd::present("python3")
}

fn runner() -> PathBuf {
    paths::search_dir().join("runner.py")
}

/// Read every installed engine's name and categories (runs Python; call off the UI thread).
pub fn load_engines() -> Vec<Engine> {
    ensure_installed();
    let out = Command::new("python3")
        .arg(runner())
        .arg("info")
        .arg(paths::engines_dir())
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    let Ok(out) = out else { return vec![] };
    String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// Keep our runner and helpers up to date in `~/.local/share/torrents/search`.
pub fn ensure_installed() {
    let dir = paths::search_dir();
    let _ = std::fs::create_dir_all(paths::engines_dir());
    for (name, text) in FILES {
        let path = dir.join(name);
        if std::fs::read_to_string(&path).ok().as_deref() != Some(*text) {
            let _ = cmd::atomic_write(&path, text);
        }
    }
}

pub fn enabled(e: &Engine) -> bool {
    e.error.is_empty() && !prefs::get().disabled_engines.contains(&e.id)
}

/// Copy qBittorrent's installed plugins (and their config files) into ours.
pub fn import_qbittorrent() -> std::io::Result<usize> {
    let from = paths::qbittorrent_engines();
    let to = paths::engines_dir();
    std::fs::create_dir_all(&to)?;
    let mut n = 0;
    for e in std::fs::read_dir(&from)?.flatten() {
        let p = e.path();
        let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
        let name = p.file_name().and_then(|x| x.to_str()).unwrap_or("");
        if name.starts_with("__") || !matches!(ext, "py" | "json" | "png" | "ico") {
            continue;
        }
        let dest = to.join(name);
        // Never overwrite a config file the user already set up here.
        if ext == "json" && dest.exists() {
            continue;
        }
        std::fs::copy(&p, &dest)?;
        if ext == "py" {
            n += 1;
        }
    }
    Ok(n)
}

/// Install one plugin file (.py) by copying it into the engines folder.
pub fn install_file(path: &Path) -> Result<String, String> {
    let name = path.file_name().and_then(|n| n.to_str()).ok_or("That file has no name.")?;
    if !name.ends_with(".py") {
        return Err("Search plugins are Python files (.py).".into());
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if !text.contains("def search") {
        return Err("That file doesn't look like a search plugin.".into());
    }
    cmd::atomic_write(&paths::engines_dir().join(name), &text).map_err(|e| e.to_string())?;
    Ok(name.trim_end_matches(".py").to_string())
}

pub fn remove(id: &str) {
    let _ = std::fs::remove_file(paths::engines_dir().join(format!("{id}.py")));
}

// ---------- Running a search ----------

pub struct Search {
    children: Arc<Mutex<Vec<Child>>>,
}

impl Search {
    pub fn stop(&self) {
        if let Ok(mut c) = self.children.lock() {
            for child in c.iter_mut() {
                let _ = child.kill();
            }
        }
    }
}

pub enum Message {
    Hit(Hit),
    EngineDone { name: String, error: String },
}

thread_local! {
    static CURRENT: RefCell<Option<Search>> = const { RefCell::new(None) };
}

pub fn stop() {
    CURRENT.with(|c| {
        if let Some(s) = c.borrow_mut().take() {
            s.stop();
        }
    });
}

/// Search every engine in `engines` at once. `on_message` runs on the UI thread;
/// `on_done` once every engine has finished (or been stopped, or timed out after 60 s).
pub fn run(
    query: &str,
    category: &str,
    engines: Vec<Engine>,
    on_message: impl Fn(Message) + 'static,
    on_done: impl Fn() + 'static,
) {
    stop();
    let (tx, rx) = async_channel::unbounded::<Option<Message>>();
    let children: Arc<Mutex<Vec<Child>>> = Arc::new(Mutex::new(Vec::new()));
    let total = engines.len();
    for e in engines {
        let cat = if e.categories.iter().any(|c| c == category) { category.to_string() } else { "all".to_string() };
        let spawned = Command::new("python3")
            .arg(runner())
            .arg("search")
            .arg(paths::engines_dir())
            .arg(&e.id)
            .arg(&cat)
            .arg(query)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(c) => c,
            Err(err) => {
                let _ = tx.send_blocking(Some(Message::EngineDone { name: e.name.clone(), error: err.to_string() }));
                let _ = tx.send_blocking(None);
                continue;
            }
        };
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        if let Ok(mut c) = children.lock() {
            c.push(child);
        }
        let tx = tx.clone();
        std::thread::spawn(move || {
            if let Some(out) = stdout {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    if let Some(mut hit) = parse_hit(&line) {
                        hit.engine = e.name.clone();
                        hit.engine_id = e.id.clone();
                        if tx.send_blocking(Some(Message::Hit(hit))).is_err() {
                            return;
                        }
                    }
                }
            }
            let error = stderr
                .map(|s| {
                    BufReader::new(s).lines().map_while(Result::ok).filter(|l| !l.trim().is_empty()).last().unwrap_or_default()
                })
                .unwrap_or_default();
            let _ = tx.send_blocking(Some(Message::EngineDone { name: e.name, error }));
            let _ = tx.send_blocking(None);
        });
    }
    drop(tx);
    let timeout_children = children.clone();
    let timer = glib::timeout_add_local_once(std::time::Duration::from_secs(60), move || {
        if let Ok(mut c) = timeout_children.lock() {
            for child in c.iter_mut() {
                let _ = child.kill();
            }
        }
    });
    CURRENT.with(|c| *c.borrow_mut() = Some(Search { children: children.clone() }));
    glib::spawn_future_local(async move {
        let mut finished = 0;
        while finished < total {
            match rx.recv().await {
                Ok(Some(m)) => on_message(m),
                Ok(None) => finished += 1,
                Err(_) => break,
            }
        }
        timer.remove();
        if let Ok(mut c) = children.lock() {
            for child in c.iter_mut() {
                let _ = child.wait();
            }
        }
        on_done();
    });
}

/// Fetch a result's .torrent through its engine (some sites need the plugin's own
/// download code). Runs off the UI thread; gives back a path or a magnet link.
pub fn download(engine_id: &str, url: &str, done: impl FnOnce(Result<String, String>) + 'static) {
    let (engine_id, url) = (engine_id.to_string(), url.to_string());
    cmd::background(
        move || {
            let out = Command::new("python3")
                .arg(runner())
                .arg("download")
                .arg(paths::engines_dir())
                .arg(&engine_id)
                .arg(&url)
                .stdin(Stdio::null())
                .output()
                .map_err(|e| e.to_string())?;
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            // "path url", as nova2dl prints it.
            let first = text.split_whitespace().next().unwrap_or("").to_string();
            if first.is_empty() {
                Err(String::from_utf8_lossy(&out.stderr).trim().lines().last().unwrap_or("no file came back").to_string())
            } else {
                Ok(first)
            }
        },
        done,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_result_lines() {
        let line = r#"{"link": "magnet:?xt=urn:btih:abc", "name": "Ubuntu 24.04", "size": 6000000000, "seeds": 120, "leech": 4, "engine_url": "https://x", "desc_link": "https://x/1", "pub_date": 1700000000}"#;
        let h = parse_hit(line).unwrap();
        assert_eq!(h.name, "Ubuntu 24.04");
        assert_eq!(h.seeds, 120);
        assert_eq!(h.size, 6_000_000_000);
        assert!(parse_hit("not json").is_none());
        assert!(parse_hit(r#"{"name": "no link"}"#).is_none());
    }
}
