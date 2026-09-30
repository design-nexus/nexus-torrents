//! Well-known locations. Every path honours the XDG overrides so the whole app
//! can be pointed at a scratch copy of `~/.config` for testing.

use std::path::{Path, PathBuf};

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join(fallback))
}

pub fn config_home() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config")
}

pub fn cache_dir() -> PathBuf {
    xdg("XDG_CACHE_HOME", ".cache").join("torrents")
}

pub fn state_home() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state")
}

pub fn data_home() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share")
}

/// `~/.config/torrents`: settings, categories and user themes.
pub fn app_dir() -> PathBuf {
    config_home().join("torrents")
}

pub fn prefs_file() -> PathBuf {
    app_dir().join("settings.toml")
}

pub fn categories_file() -> PathBuf {
    app_dir().join("categories.toml")
}

pub fn custom_themes_dir() -> PathBuf {
    app_dir().join("themes")
}

/// `~/.local/share/torrents`: the session itself.
pub fn data_dir() -> PathBuf {
    data_home().join("torrents")
}

pub fn resume_dir() -> PathBuf {
    data_dir().join("resume")
}

pub fn session_file() -> PathBuf {
    data_dir().join("session.dat")
}

pub fn queue_file() -> PathBuf {
    data_dir().join("queue")
}

/// Per-torrent app data: category, tags, where it should end up.
pub fn torrents_file() -> PathBuf {
    data_dir().join("torrents.json")
}

pub fn search_dir() -> PathBuf {
    data_dir().join("search")
}

pub fn engines_dir() -> PathBuf {
    search_dir().join("engines")
}

pub fn qbittorrent_engines() -> PathBuf {
    data_home().join("qBittorrent/nova3/engines")
}

pub fn downloads() -> PathBuf {
    let dirs = config_home().join("user-dirs.dirs");
    if let Ok(text) = std::fs::read_to_string(dirs) {
        for line in text.lines() {
            if let Some(v) = line.strip_prefix("XDG_DOWNLOAD_DIR=") {
                let v = v.trim().trim_matches('"').replace("$HOME", &home().to_string_lossy());
                if !v.is_empty() {
                    return PathBuf::from(v);
                }
            }
        }
    }
    home().join("Downloads")
}

pub fn omarchy_theme_dir() -> PathBuf {
    state_home().join("omarchy/current/theme")
}

pub fn omarchy_colors() -> PathBuf {
    omarchy_theme_dir().join("colors.toml")
}

/// Replace `$HOME` with `~` for display.
pub fn pretty(path: &Path) -> String {
    let home = home();
    match path.strip_prefix(&home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// Expand a leading `~` typed by the user.
pub fn expand(text: &str) -> PathBuf {
    let t = text.trim();
    if t == "~" {
        return home();
    }
    match t.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None => PathBuf::from(t),
    }
}
