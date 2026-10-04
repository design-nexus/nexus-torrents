use crate::widgets::{self, Page};
use crate::{cmd, live, prefs, theme, window};
use gtk::glib;
use gtk::prelude::*;

const DESKTOP: &str = "io.github.design_nexus.Torrents.desktop";

fn is_default() -> bool {
    cmd::output(&["xdg-mime", "query", "default", "x-scheme-handler/magnet"]).is_some_and(|v| v.trim() == DESKTOP)
}

pub fn build(page: &Page) {
    // ----- This window -----
    let g = page.group("This window");
    let p = prefs::get();
    let app_themes = theme::all();
    let options: Vec<(String, String)> = app_themes.iter().map(|t| (t.id.clone(), t.name.clone())).collect();
    let (theme_row, theme_dd) = widgets::choice_row(
        "Theme",
        "Dracula, Catppuccin, Tokyo Night, One Dark Pro and more. Add your own in <tt>~/.config/torrents/themes</tt>.",
        options,
        &p.theme,
        |id| {
            prefs::update(|p| {
                p.theme = id;
                p.mode = prefs::ThemeMode::Theme;
            });
            theme::apply();
        },
    );
    theme_dd.set_sensitive(p.mode == prefs::ThemeMode::Theme || !theme::omarchy_available());

    if theme::omarchy_available() {
        let dd = theme_dd.clone();
        let (r, _) = widgets::switch_row(
            "Follow Omarchy theme",
            "Match the desktop's colors and update live whenever the Omarchy theme changes.",
            p.mode == prefs::ThemeMode::Omarchy,
            move |on| {
                prefs::update(|p| p.mode = if on { prefs::ThemeMode::Omarchy } else { prefs::ThemeMode::Theme });
                dd.set_sensitive(!on);
                theme::apply();
            },
        );
        g.add(&r);
    }
    g.add(&theme_row);

    let swatches = widgets::hbox(4);
    let refresh_swatches = {
        let swatches = swatches.clone();
        move || {
            while let Some(c) = swatches.first_child() {
                swatches.remove(&c);
            }
            let pal = theme::current_palette();
            for c in [&pal.bg, &pal.surface, &pal.muted, &pal.text, &pal.accent, &pal.danger] {
                let s = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                s.add_css_class("swatch");
                let provider = gtk::CssProvider::new();
                provider.load_from_string(&format!("box {{ background: {c}; }}"));
                #[allow(deprecated)]
                s.style_context().add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_USER);
                swatches.append(&s);
            }
        }
    };
    refresh_swatches();
    let last = std::cell::RefCell::new(theme::current_palette());
    let weak = swatches.downgrade();
    glib::timeout_add_seconds_local(1, move || {
        if weak.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        let now = theme::current_palette();
        if *last.borrow() != now {
            *last.borrow_mut() = now;
            refresh_swatches();
        }
        glib::ControlFlow::Continue
    });
    g.add(&widgets::row("Current colors", "", Some(swatches.upcast_ref())));

    let (r, _) = widgets::switch_row("Glow", "Soft accent glow around focused and selected elements.", p.glow, |on| {
        prefs::update(|p| p.glow = on);
        theme::apply();
    });
    g.add(&r);
    let (r, _) =
        widgets::switch_row("Reduce motion", "Turn off transitions and animations in this window.", p.reduce_motion, |on| {
            prefs::update(|p| p.reduce_motion = on);
            theme::apply();
        });
    g.add(&r);

    // ----- Behaviour -----
    let g = page.group("Behavior");
    let (r, _) = widgets::switch_row(
        "Keep running when closed",
        "Closing the window keeps torrents going. Run <tt>torrents</tt> to bring it back; <tt>Ctrl+Q</tt> quits.",
        p.keep_running,
        |on| prefs::update(|p| p.keep_running = on),
    );
    g.add(&r);
    let (r, _) = widgets::switch_row(
        "Notify when a download finishes",
        "A desktop notification, or a toast while this window is in front.",
        p.notify_finished,
        |on| prefs::update(|p| p.notify_finished = on),
    );
    g.add(&r);
    let (r, _) = widgets::switch_row("Speeds in bits", "Show Mb/s like internet plans do, instead of MiB/s.", p.net_bits, |on| {
        prefs::update(|p| p.net_bits = on);
        live::poke_all();
    });
    g.add(&r);
    let (r, _) = widgets::choice_row(
        "Speed graph history",
        "How far back the speed graphs reach.",
        widgets::opts(&[("60", "1 minute"), ("300", "5 minutes"), ("600", "10 minutes")]),
        &p.history_secs.to_string(),
        |v| {
            prefs::update(|p| p.history_secs = v.parse().unwrap_or(300));
            live::trim();
        },
    );
    g.add(&r);

    let status = widgets::tag(if is_default() { "Default" } else { "Not default" }, if is_default() { "accent" } else { "" });
    let make_default = gtk::Button::with_label("Make default");
    make_default.set_sensitive(!is_default());
    let controls = widgets::hbox(8);
    controls.append(&status);
    controls.append(&make_default);
    let s2 = status.clone();
    make_default.connect_clicked(move |b| {
        let (b, s2) = (b.clone(), s2.clone());
        cmd::run_async(&["xdg-mime", "default", DESKTOP, "x-scheme-handler/magnet", "application/x-bittorrent"], move |res| {
            match res {
                Ok(_) => {
                    s2.set_text("Default");
                    s2.add_css_class("accent");
                    b.set_sensitive(false);
                    window::toast("Magnet links and .torrent files now open in Torrents.");
                }
                Err(e) => window::toast(&format!("Couldn't set the default: {e}")),
            }
        });
    });
    g.add(&widgets::row(
        "Open magnet links and .torrent files",
        "Make Torrents the app your browser hands them to.",
        Some(controls.upcast_ref()),
    ));

    // ----- Keyboard -----
    let g = page.group("Keyboard");
    for (keys, what) in crate::window::SHORTCUTS {
        g.add(&widgets::row(what, "", Some(widgets::keycaps(keys).upcast_ref())));
    }
}
