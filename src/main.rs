//! Torrents — a BitTorrent client for Omarchy, built on libtorrent.

mod actions;
mod add_dialog;
mod cmd;
mod engine;
mod fmt;
mod graph;
mod live;
mod model;
mod paths;
mod prefs;
mod search;
mod sections;
mod store;
mod table;
mod theme;
mod widgets;
mod window;

use gtk::prelude::*;
use gtk::{gio, glib};

const APP_ID: &str = "io.github.design_nexus.Torrents";

const USAGE: &str = "Usage: torrents [--section ID] [--quit] [FILE.torrent | MAGNET | URL]...\n\
\n\
  --section ID   open (or switch the open window) to a page: all, downloading, seeding,\n\
                 completed, paused, active, inactive, errored, cat:NAME, tag:NAME, search,\n\
                 downloads, connection, speed, bittorrent, appearance\n\
  --quit         stop the running instance (after saving its state)\n\
\n\
Torrent files, magnet links and links to .torrent files are added to the running window.\n";

fn main() -> glib::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{USAGE}");
        return glib::ExitCode::SUCCESS;
    }
    glib::set_application_name("Torrents");

    let mut flags = gio::ApplicationFlags::HANDLES_COMMAND_LINE;
    // Snapshots run beside a real instance instead of handing it their arguments.
    if std::env::var_os("TORRENTS_SNAPSHOT").is_some() {
        flags |= gio::ApplicationFlags::NON_UNIQUE;
    }
    let app = gtk::Application::builder().application_id(APP_ID).flags(flags).build();
    app.connect_command_line(|app, cl| {
        let argv: Vec<String> = cl.arguments().iter().map(|a| a.to_string_lossy().to_string()).collect();
        if argv.iter().any(|a| a == "--quit") {
            app.quit();
            return glib::ExitCode::SUCCESS;
        }
        let mut section = None;
        let mut items = Vec::new();
        let mut i = 1;
        while i < argv.len() {
            if argv[i] == "--section" {
                section = argv.get(i + 1).cloned();
                i += 2;
                continue;
            }
            items.push(argv[i].clone());
            i += 1;
        }
        window::present(app, section.as_deref());
        // Paths are relative to where `torrents` was run, not to the running instance.
        let cwd = cl.cwd();
        for item in items {
            let path = std::path::Path::new(&item);
            if !item.contains("://")
                && !item.starts_with("magnet:")
                && path.is_relative()
                && let Some(cwd) = &cwd
            {
                actions::add_link(&cwd.join(path).to_string_lossy());
                continue;
            }
            actions::add_link(&item);
        }
        glib::ExitCode::SUCCESS
    });
    app.connect_shutdown(|_| live::shutdown());
    app.run()
}
