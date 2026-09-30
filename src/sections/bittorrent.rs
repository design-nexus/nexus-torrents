use crate::widgets::{self, Page};
use crate::{live, prefs};
use gtk::prelude::*;

fn set(change: impl FnOnce(&mut prefs::Prefs)) {
    prefs::update(change);
    live::apply_settings();
}

fn count_row(title: &str, desc: &str, value: i32, f: impl Fn(i32) + 'static) -> gtk::Box {
    let (r, _) = widgets::spin_row(title, desc, (0.0, 1000.0), 1.0, 0, value as f64, "", move |v| f(v as i32));
    r
}

pub fn build(page: &Page) {
    let p = prefs::get();

    // ----- Finding peers -----
    let g = page.group("Finding peers");
    let (r, _) = widgets::switch_row("DHT", "Find peers without a tracker, through the distributed hash table.", p.dht, |on| {
        set(|p| p.dht = on)
    });
    g.add(&r);
    let (r, _) = widgets::switch_row(
        "Peer exchange",
        "Swap peer lists with the peers you're connected to. Applies to torrents added from now on.",
        p.pex,
        |on| set(|p| p.pex = on),
    );
    g.add(&r);
    let (r, _) = widgets::switch_row("Local peer discovery", "Find peers on your own network.", p.lsd, |on| set(|p| p.lsd = on));
    g.add(&r);
    g.note("Private torrents never use DHT, peer exchange or local discovery, whatever these say.");

    // ----- Privacy -----
    let g = page.group("Privacy");
    g.add(&widgets::segmented_row(
        "Encryption",
        "Require it to hide torrent traffic from your provider, at the cost of fewer peers.",
        &[("allow", "Allow"), ("prefer", "Prefer"), ("require", "Require"), ("disable", "Off")],
        &p.encryption,
        |v| set(|p| p.encryption = v),
    ));
    let (r, _) = widgets::switch_row(
        "Anonymous mode",
        "Don't tell trackers and peers which client you use or your local address. Some private trackers refuse it.",
        p.anonymous,
        |on| set(|p| p.anonymous = on),
    );
    g.add(&r);

    // ----- Queue -----
    let g = page.group("Queue");
    let limits = widgets::vbox(6);
    limits.set_visible(p.queueing);
    limits.append(&count_row("Active downloads", "Others wait in line.", p.max_active_downloads, |v| {
        set(|p| p.max_active_downloads = v)
    }));
    limits.append(&count_row("Active uploads", "Seeding torrents at once.", p.max_active_uploads, |v| {
        set(|p| p.max_active_uploads = v)
    }));
    limits.append(&count_row("Active torrents", "Downloads and uploads together.", p.max_active_torrents, |v| {
        set(|p| p.max_active_torrents = v)
    }));
    let slow = widgets::vbox(6);
    slow.set_visible(p.ignore_slow);
    let (r, _) = widgets::spin_row("Slow download below", "", (1.0, 100_000.0), 1.0, 0, p.slow_dl as f64, "KiB/s", |v| {
        set(|p| p.slow_dl = v as i32)
    });
    slow.append(&r);
    let (r, _) = widgets::spin_row("Slow upload below", "", (1.0, 100_000.0), 1.0, 0, p.slow_ul as f64, "KiB/s", |v| {
        set(|p| p.slow_ul = v as i32)
    });
    slow.append(&r);
    let s2 = slow.clone();
    let (r, _) = widgets::switch_row(
        "Don't count slow torrents",
        "Stalled torrents don't hold up the queue; the next one starts.",
        p.ignore_slow,
        move |on| {
            s2.set_visible(on);
            set(|p| p.ignore_slow = on);
        },
    );
    limits.append(&r);
    limits.append(&slow);
    let l2 = limits.clone();
    let (r, _) = widgets::switch_row(
        "Queue torrents",
        "Run only a few at a time, in the order they're queued. Off runs every torrent at once.",
        p.queueing,
        move |on| {
            l2.set_visible(on);
            set(|p| p.queueing = on);
        },
    );
    g.add(&r);
    g.add(&limits);

    // ----- Seeding limits -----
    let g = page.group("Seeding limits");
    g.note("0 means no limit. Force-resumed and super-seeding torrents are left alone.");
    let (r, _) = widgets::spin_row(
        "Stop at share ratio",
        "Uploaded divided by downloaded.",
        (0.0, 1000.0),
        0.1,
        2,
        p.ratio_limit,
        "×",
        |v| prefs::update(|p| p.ratio_limit = v),
    );
    g.add(&r);
    let (r, _) =
        widgets::spin_row("Stop after seeding for", "", (0.0, 525_600.0), 10.0, 0, p.seed_time_limit as f64, "min", |v| {
            prefs::update(|p| p.seed_time_limit = v as i64)
        });
    g.add(&r);
    let (r, _) = widgets::choice_row(
        "Then",
        "What happens when a torrent reaches either limit.",
        widgets::opts(&[
            ("pause", "Pause it"),
            ("remove", "Remove it"),
            ("remove-files", "Remove it and its files"),
            ("superseed", "Switch to super seeding"),
        ]),
        &p.limit_action,
        |v| prefs::update(|p| p.limit_action = v),
    );
    g.add(&r);
}
