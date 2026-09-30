//! Our own data about torrents, beside libtorrent's: each torrent's category and
//! tags (`~/.local/share/torrents/torrents.json`), and the list of categories and
//! tags (`~/.config/torrents/categories.toml`). Writes are debounced and atomic.

use crate::{cmd, paths, prefs};
use gtk::glib;
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Meta {
    pub category: String,
    pub tags: Vec<String>,
    /// Where a torrent downloading into the incomplete folder moves when done.
    pub target: Option<String>,
    /// First and last pieces of each file come first (libtorrent keeps no flag for it).
    pub first_last: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Category {
    /// Empty: the default folder (plus a subfolder named after the category, if enabled).
    pub save_path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Lists {
    categories: BTreeMap<String, Category>,
    tags: BTreeSet<String>,
}

#[derive(Default)]
struct Store {
    meta: HashMap<String, Meta>,
    lists: Lists,
}

thread_local! {
    static STORE: RefCell<Store> = RefCell::new(load());
    static DIRTY: Cell<bool> = const { Cell::new(false) };
    static LISTENERS: RefCell<Vec<Box<dyn Fn()>>> = const { RefCell::new(Vec::new()) };
}

fn load() -> Store {
    let meta =
        std::fs::read_to_string(paths::torrents_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let lists = std::fs::read_to_string(paths::categories_file()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
    Store { meta, lists }
}

/// Save about 450 ms after the last change.
fn changed(lists: bool) {
    if !DIRTY.with(|d| d.replace(true)) {
        glib::timeout_add_local_once(std::time::Duration::from_millis(450), flush);
    }
    if lists {
        LISTENERS.with(|l| {
            for f in l.borrow().iter() {
                f();
            }
        });
    }
}

pub fn flush() {
    DIRTY.with(|d| d.set(false));
    STORE.with(|s| {
        let s = s.borrow();
        if let Ok(text) = serde_json::to_string_pretty(&s.meta) {
            let _ = cmd::atomic_write(&paths::torrents_file(), &text);
        }
        if let Ok(text) = toml::to_string_pretty(&s.lists) {
            let _ = cmd::atomic_write(&paths::categories_file(), &text);
        }
    });
}

/// Call `f` whenever categories or tags are added or removed.
pub fn on_lists_changed(f: impl Fn() + 'static) {
    LISTENERS.with(|l| l.borrow_mut().push(Box::new(f)));
}

pub fn meta(id: &str) -> Meta {
    STORE.with(|s| s.borrow().meta.get(id).cloned().unwrap_or_default())
}

pub fn set_meta(id: &str, meta: Meta) {
    let new_lists = STORE.with(|s| {
        let mut s = s.borrow_mut();
        let mut added = false;
        if !meta.category.is_empty() && !s.lists.categories.contains_key(&meta.category) {
            s.lists.categories.insert(meta.category.clone(), Category::default());
            added = true;
        }
        for t in &meta.tags {
            added |= s.lists.tags.insert(t.clone());
        }
        s.meta.insert(id.to_string(), meta);
        added
    });
    changed(new_lists);
}

pub fn update_meta(ids: &[String], f: impl Fn(&mut Meta)) {
    for id in ids {
        let mut m = meta(id);
        f(&mut m);
        set_meta(id, m);
    }
}

pub fn forget(id: &str) {
    STORE.with(|s| s.borrow_mut().meta.remove(id));
    changed(false);
}

pub fn categories() -> BTreeMap<String, Category> {
    STORE.with(|s| s.borrow().lists.categories.clone())
}

pub fn tags() -> Vec<String> {
    STORE.with(|s| s.borrow().lists.tags.iter().cloned().collect())
}

pub fn set_category(name: &str, cat: Category) {
    STORE.with(|s| s.borrow_mut().lists.categories.insert(name.to_string(), cat));
    changed(true);
}

/// Remove a category; its torrents become uncategorized.
pub fn remove_category(name: &str) {
    STORE.with(|s| {
        let mut s = s.borrow_mut();
        s.lists.categories.remove(name);
        for m in s.meta.values_mut() {
            if m.category == name {
                m.category.clear();
            }
        }
    });
    changed(true);
}

pub fn add_tag(name: &str) {
    let added = STORE.with(|s| s.borrow_mut().lists.tags.insert(name.to_string()));
    if added {
        changed(true);
    }
}

pub fn remove_tag(name: &str) {
    STORE.with(|s| {
        let mut s = s.borrow_mut();
        s.lists.tags.remove(name);
        for m in s.meta.values_mut() {
            m.tags.retain(|t| t != name);
        }
    });
    changed(true);
}

/// Where a torrent in `category` is saved.
pub fn save_path(category: &str) -> PathBuf {
    let p = prefs::get();
    resolve_save_path(&p.save_path, p.category_subfolders, category, categories().get(category))
}

pub fn resolve_save_path(default: &str, subfolders: bool, category: &str, cat: Option<&Category>) -> PathBuf {
    let base = paths::expand(default);
    if category.is_empty() {
        return base;
    }
    match cat {
        Some(c) if !c.save_path.trim().is_empty() => {
            let p = paths::expand(&c.save_path);
            if p.is_absolute() { p } else { base.join(p) }
        }
        _ if subfolders => base.join(category),
        _ => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_paths() {
        let movies = Category { save_path: "/media/films".into() };
        let rel = Category { save_path: "Shows".into() };
        assert_eq!(resolve_save_path("/dl", true, "", None), PathBuf::from("/dl"));
        assert_eq!(resolve_save_path("/dl", true, "Linux", None), PathBuf::from("/dl/Linux"));
        assert_eq!(resolve_save_path("/dl", false, "Linux", None), PathBuf::from("/dl"));
        assert_eq!(resolve_save_path("/dl", true, "Movies", Some(&movies)), PathBuf::from("/media/films"));
        assert_eq!(resolve_save_path("/dl", false, "TV", Some(&rel)), PathBuf::from("/dl/Shows"));
    }
}
