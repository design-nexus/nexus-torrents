use crate::graph::{self, Scale, Series, Tone};
use crate::widgets::{self, Page};
use crate::{fmt, live, prefs, window};
use gtk::prelude::*;

fn set(change: impl FnOnce(&mut prefs::Prefs)) {
    prefs::update(change);
    live::apply_settings();
    window::refresh_alt_speed();
}

fn limit_row(title: &str, desc: &str, value: i32, f: impl Fn(i32) + 'static) -> gtk::Box {
    let (r, _) = widgets::spin_row(title, desc, (0.0, 1_000_000.0), 64.0, 0, value as f64, "KiB/s", move |v| f(v as i32));
    r
}

pub fn build(page: &Page) {
    let p = prefs::get();

    let g = page.group("Right now");
    let graph = graph::graph(
        vec![Series::new("dl", "Download", Tone::Accent), Series::new("ul", "Upload", Tone::Second)],
        Scale::Auto { floor: 64.0 * 1024.0 },
        fmt::net_rate,
        140,
    );
    let (card, summary) = widgets::graph_card("All torrents", &graph.root);
    live::on_tick(&card, move |u| {
        summary.set_text(&format!("↓ {}  ↑ {}", fmt::net_rate(u.stats.dl_rate as f64), fmt::net_rate(u.stats.ul_rate as f64)));
    });
    g.add(&card);

    // ----- Global -----
    let g = page.group("Limits");
    g.note("0 means unlimited.");
    g.add(&limit_row("Download", "The most all torrents may download together.", p.dl_limit, |v| set(|p| p.dl_limit = v)));
    g.add(&limit_row("Upload", "The most all torrents may upload together.", p.ul_limit, |v| set(|p| p.ul_limit = v)));

    // ----- Alternative -----
    let g = page.group("Alternative limits");
    g.note("Slow mode, for when you need the connection for something else. The turtle in the status bar switches it.");
    g.add(&limit_row("Download", "", p.alt_dl_limit, |v| set(|p| p.alt_dl_limit = v)));
    g.add(&limit_row("Upload", "", p.alt_ul_limit, |v| set(|p| p.alt_ul_limit = v)));

    let times = widgets::vbox(6);
    times.set_visible(p.schedule);
    let (r, _) = widgets::entry_row("From", "24-hour time.", &p.schedule_from, "08:00", true, |v| set(|p| p.schedule_from = v));
    times.append(&r);
    let (r, _) = widgets::entry_row("Until", "", &p.schedule_to, "20:00", true, |v| set(|p| p.schedule_to = v));
    times.append(&r);
    times.append(&widgets::segmented_row(
        "On",
        "",
        &[("every", "Every day"), ("weekdays", "Weekdays"), ("weekends", "Weekends")],
        &p.schedule_days,
        |v| set(|p| p.schedule_days = v),
    ));
    let t = times.clone();
    let (r, _) = widgets::switch_row(
        "Switch on a schedule",
        "Use the alternative limits during set hours, and the normal ones otherwise.",
        p.schedule,
        move |on| {
            t.set_visible(on);
            set(|p| p.schedule = on);
        },
    );
    g.add(&r);
    g.add(&times);

    // ----- Options -----
    let g = page.group("Options");
    let (r, _) = widgets::switch_row(
        "Count protocol overhead",
        "Include the traffic peers use to talk to each other, not just file data.",
        p.limit_overhead,
        |on| set(|p| p.limit_overhead = on),
    );
    g.add(&r);
}
