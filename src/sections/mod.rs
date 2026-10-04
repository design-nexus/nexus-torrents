//! Every page, in sidebar order. Transfers has no nav item of its own: the
//! transfer, category and tag filters all lead to it.

use crate::paths;
use crate::widgets::Page;
use std::path::PathBuf;

pub mod appearance;
pub mod bittorrent;
pub mod connection;
pub mod details;
pub mod downloads;
pub mod search;
pub mod speed;
pub mod transfers;

pub struct Section {
    pub id: &'static str,
    pub title: &'static str,
    pub icon: &'static str,
    /// Empty for pages without a nav item.
    pub group: &'static str,
    pub description: &'static str,
    /// Extra words the search should find this page by.
    pub keywords: &'static str,
    /// Files the "Open config" button offers.
    pub files: fn() -> Vec<PathBuf>,
    pub build: fn(&Page),
    /// The page manages its own scrolling (tables).
    pub fill: bool,
}

fn settings_file() -> Vec<PathBuf> {
    vec![paths::prefs_file()]
}

fn download_files() -> Vec<PathBuf> {
    vec![paths::prefs_file(), paths::categories_file()]
}

pub fn all() -> Vec<Section> {
    vec![
        Section {
            id: "transfers",
            title: "Transfers",
            icon: "view-list-symbolic",
            group: "",
            description: "",
            keywords: "",
            files: Vec::new,
            build: transfers::build,
            fill: true,
        },
        Section {
            id: "search",
            title: "Search",
            icon: "system-search-symbolic",
            group: "Discover",
            description: "Find torrents with qBittorrent search plugins.",
            keywords: "find plugins engines nova",
            files: Vec::new,
            build: search::build,
            fill: true,
        },
        Section {
            id: "downloads",
            title: "Downloads",
            icon: "folder-download-symbolic",
            group: "Settings",
            description: "Where torrents are saved and what happens when you add one.",
            keywords: "folder save path location incomplete categories add dialog finished command",
            files: download_files,
            build: downloads::build,
            fill: false,
        },
        Section {
            id: "connection",
            title: "Connection",
            icon: "network-wired-symbolic",
            group: "Settings",
            description: "How Torrents reaches peers: port, network interface and proxy.",
            keywords: "port upnp nat vpn interface bind proxy socks http peers connections utp tcp",
            files: settings_file,
            build: connection::build,
            fill: false,
        },
        Section {
            id: "speed",
            title: "Speed",
            icon: "power-profile-performance-symbolic",
            group: "Settings",
            description: "Limit how much bandwidth torrents use, all the time or on a schedule.",
            keywords: "limit bandwidth rate download upload alternative slow turtle schedule",
            files: settings_file,
            build: speed::build,
            fill: false,
        },
        Section {
            id: "bittorrent",
            title: "BitTorrent",
            icon: "preferences-system-symbolic",
            group: "Settings",
            description: "Peer discovery, encryption, the queue and when to stop seeding.",
            keywords: "dht pex lsd encryption anonymous queue queueing active ratio seeding time share limit",
            files: settings_file,
            build: bittorrent::build,
            fill: false,
        },
        Section {
            id: "appearance",
            title: "Appearance",
            icon: "applications-graphics-symbolic",
            group: "Settings",
            description: "How this window looks and behaves.",
            keywords: "theme colours colors omarchy glow motion background notifications keyboard shortcuts",
            files: settings_file,
            build: appearance::build,
            fill: false,
        },
    ]
}
