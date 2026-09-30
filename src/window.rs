//! The main window: Strata-style navigation sidebar (transfer filters with live
//! counts, categories, tags, then Search and the settings pages) and a stack of
//! pages that are built the first time they're shown. Every transfer filter
//! shows the same Transfers page.

use crate::model::{Filter, STATUS_FILTERS};
use crate::sections::{self, Section, transfers};
use crate::widgets::{self, SEARCH};
use crate::{actions, fmt, live, prefs, store, theme};
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

struct Ui {
    app: gtk::Application,
    window: gtk::ApplicationWindow,
    stack: gtk::Stack,
    /// Nav buttons by id: filter ids ("all", "cat:Movies"…) and page ids.
    nav_items: HashMap<String, gtk::Button>,
    nav_counts: HashMap<String, gtk::Label>,
    nav_groups: Vec<(gtk::Label, Vec<String>)>,
    categories_box: gtk::Box,
    tags_box: gtk::Box,
    filter_groups: Vec<gtk::Widget>,
    pages: HashMap<&'static str, gtk::ScrolledWindow>,
    sections: Vec<Section>,
    current: String,
    overlay: gtk::Overlay,
    compact_hide: Vec<gtk::Widget>,
}

thread_local! {
    static UI: RefCell<Option<Rc<RefCell<Ui>>>> = const { RefCell::new(None) };
    static NARROW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static COMPACT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn ui() -> Option<Rc<RefCell<Ui>>> {
    UI.with(|u| u.borrow().clone())
}

pub fn window() -> Option<gtk::ApplicationWindow> {
    ui().map(|u| u.borrow().window.clone())
}

pub fn present(app: &gtk::Application, section: Option<&str>) {
    if let Some(ui) = ui() {
        let window = ui.borrow().window.clone();
        if let Some(s) = section {
            navigate(s);
        }
        window.present();
        return;
    }
    theme::install();
    install_icons();
    live::start();
    actions::install(app);
    build(app);
    let start = section.map(String::from).unwrap_or_else(|| prefs::get().last_section);
    navigate(&start);
    // Developer aid: TORRENTS_SNAPSHOT=/path.png renders the window to a PNG
    // (invisibly) and quits, so layouts can be checked without a visible window.
    if let Some(out) = std::env::var_os("TORRENTS_SNAPSHOT") {
        snapshot_and_quit(app, std::path::PathBuf::from(out));
        return;
    }
    if let Some(ui) = ui() {
        ui.borrow().window.present();
    }
}

/// Our own symbolic icons (the icon theme may lack a turtle or a magnet).
fn install_icons() {
    const ICONS: &[(&str, &str)] = &[
        ("torrents-turtle-symbolic.svg", include_str!("../data/icons/torrents-turtle-symbolic.svg")),
        ("torrents-magnet-symbolic.svg", include_str!("../data/icons/torrents-magnet-symbolic.svg")),
    ];
    let dir = crate::paths::cache_dir().join("icons");
    for (name, svg) in ICONS {
        let path = dir.join(name);
        if std::fs::read_to_string(&path).ok().as_deref() != Some(*svg) {
            let _ = crate::cmd::atomic_write(&path, svg);
        }
    }
    if let Some(display) = gdk::Display::default() {
        gtk::IconTheme::for_display(&display).add_search_path(&dir);
    }
}

fn nav_button(id: &str, title: &str, icon: &str, tooltip: &str, count: bool) -> (gtk::Button, Option<gtk::Label>, gtk::Label) {
    let button = gtk::Button::new();
    button.add_css_class("nav-item");
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.append(&gtk::Image::from_icon_name(icon));
    let l = widgets::label(title, "nav-label");
    l.set_hexpand(true);
    l.set_ellipsize(gtk::pango::EllipsizeMode::End);
    content.append(&l);
    let readout = count.then(|| {
        let r = widgets::label("", "nav-readout");
        r.add_css_class("mono");
        content.append(&r);
        r
    });
    button.set_child(Some(&content));
    button.set_tooltip_text(Some(tooltip));
    let id = id.to_string();
    button.connect_clicked(move |_| navigate(&id));
    (button, readout, l)
}

fn group_heading(title: &str, add: Option<(&str, fn())>) -> (gtk::Box, gtk::Label) {
    let row = widgets::hbox(4);
    let g = widgets::label(&title.to_uppercase(), "nav-group");
    g.set_hexpand(true);
    row.append(&g);
    if let Some((tip, f)) = add {
        let b = gtk::Button::from_icon_name("list-add-symbolic");
        b.add_css_class("flat");
        b.add_css_class("nav-add");
        b.set_tooltip_text(Some(tip));
        b.connect_clicked(move |_| f());
        row.append(&b);
    }
    (row, g)
}

fn build(app: &gtk::Application) {
    let window =
        gtk::ApplicationWindow::builder().application(app).title("Torrents").default_width(1120).default_height(800).build();
    window.add_css_class("torrents-window");
    // No client-side titlebar: Hyprland manages the window, like Strata.
    window.set_titlebar(Some(&gtk::Box::new(gtk::Orientation::Horizontal, 0)));
    window.set_icon_name(Some("io.github.design_nexus.Torrents"));

    let sections = sections::all();

    // ----- Sidebar -----
    let nav = gtk::Box::new(gtk::Orientation::Vertical, 0);
    nav.add_css_class("settings-navigation");
    nav.set_hexpand(false);
    let heading = widgets::label("TORRENTS", "menu-heading");
    let mut compact_hide: Vec<gtk::Widget> = vec![heading.clone().upcast()];
    nav.append(&heading);

    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Search settings"));
    search.add_css_class("settings-search");
    nav.append(&search);
    compact_hide.push(search.clone().upcast());

    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let mut nav_items = HashMap::new();
    let mut nav_counts = HashMap::new();
    let mut nav_groups: Vec<(gtk::Label, Vec<String>)> = Vec::new();
    let mut filter_groups: Vec<gtk::Widget> = Vec::new();

    // Transfers
    let transfers_box = widgets::vbox(0);
    let (h, g) = group_heading("Transfers", None);
    compact_hide.push(g.clone().upcast());
    transfers_box.append(&h);
    for (id, title, icon) in STATUS_FILTERS {
        let (b, count, label) = nav_button(id, title, icon, title, true);
        if let Some(c) = count {
            compact_hide.push(c.clone().upcast());
            nav_counts.insert(id.to_string(), c);
        }
        compact_hide.push(label.upcast());
        transfers_box.append(&b);
        nav_items.insert(id.to_string(), b);
    }
    list.append(&transfers_box);
    filter_groups.push(transfers_box.upcast());

    // Categories and tags are filled by `rebuild_lists`.
    let categories_box = widgets::vbox(0);
    list.append(&categories_box);
    filter_groups.push(categories_box.clone().upcast());
    let tags_box = widgets::vbox(0);
    list.append(&tags_box);
    filter_groups.push(tags_box.clone().upcast());

    let mut last_group = "";
    // The Transfers page has no nav item of its own: the filters above lead to it.
    for s in sections.iter().filter(|s| !s.group.is_empty()) {
        if s.group != last_group {
            let g = widgets::label(&s.group.to_uppercase(), "nav-group");
            compact_hide.push(g.clone().upcast());
            list.append(&g);
            nav_groups.push((g, Vec::new()));
            last_group = s.group;
        }
        let (b, _, label) = nav_button(s.id, s.title, s.icon, s.description, false);
        compact_hide.push(label.upcast());
        list.append(&b);
        nav_items.insert(s.id.to_string(), b);
        if let Some(g) = nav_groups.last_mut() {
            g.1.push(s.id.to_string());
        }
    }
    let nav_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        // Scrolls with wheel, trackpad and keyboard; no visible scrollbar.
        .vscrollbar_policy(gtk::PolicyType::External)
        .vexpand(true)
        .child(&list)
        .build();
    nav.append(&nav_scroll);

    // Footer: overall speeds, the alternative-speed switch, and the version.
    let footer = widgets::vbox(6);
    footer.add_css_class("nav-footer");
    let rates = widgets::hbox(12);
    let down = widgets::label("↓ 0 B/s", "footer-rate");
    down.add_css_class("mono");
    let up = widgets::label("↑ 0 B/s", "footer-rate");
    up.add_css_class("mono");
    rates.append(&down);
    rates.append(&up);
    footer.append(&rates);
    let line = widgets::hbox(6);
    let version = widgets::label(concat!("Torrents ", env!("CARGO_PKG_VERSION")), "dim");
    version.set_hexpand(true);
    version.set_ellipsize(gtk::pango::EllipsizeMode::End);
    line.append(&version);
    line.append(&alt_speed_chip());
    footer.append(&line);
    let net = widgets::label("", "footer-net");
    net.add_css_class("mono");
    footer.append(&net);
    nav.append(&footer);
    compact_hide.push(footer.clone().upcast());
    {
        let (down, up, net) = (down.clone(), up.clone(), net.clone());
        live::on_tick(&footer, move |u| {
            down.set_text(&format!("↓ {}", fmt::net_rate(u.stats.dl_rate as f64)));
            up.set_text(&format!("↑ {}", fmt::net_rate(u.stats.ul_rate as f64)));
            let mut parts = vec![format!("Port {}", u.stats.listen_port)];
            if prefs::get().dht {
                parts.push(format!("DHT {}", u.stats.dht_nodes));
            }
            if let Some(free) = free_space(&prefs::get().save_path) {
                parts.push(format!("{} free", fmt::bytes(free as f64)));
            }
            net.set_text(&parts.join(" · "));
            net.set_tooltip_text(Some(if u.stats.has_incoming {
                "Listening for incoming connections."
            } else {
                "Not listening: check the port and network interface in Connection."
            }));
        });
    }

    // ----- Content -----
    let stack = gtk::Stack::new();
    stack.add_css_class("settings-content");
    stack.set_hexpand(true);
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(if prefs::get().reduce_motion { 0 } else { 160 });

    let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    body.append(&nav);
    body.append(&stack);

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&body));
    window.set_child(Some(&overlay));

    // ----- Keys -----
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let s2 = search.clone();
    let w2 = window.clone();
    let app2 = app.clone();
    keys.connect_key_pressed(move |_, key, _, mods| {
        let ctrl = mods.contains(gdk::ModifierType::CONTROL_MASK);
        match key {
            gdk::Key::f if ctrl => {
                if is_transfers() {
                    transfers::focus_search();
                } else {
                    s2.grab_focus();
                }
                glib::Propagation::Stop
            }
            gdk::Key::o if ctrl => {
                actions::open_torrent_files();
                glib::Propagation::Stop
            }
            gdk::Key::u if ctrl => {
                actions::add_magnet_dialog("");
                glib::Propagation::Stop
            }
            gdk::Key::q if ctrl => {
                app2.quit();
                glib::Propagation::Stop
            }
            gdk::Key::w if ctrl => {
                w2.close();
                glib::Propagation::Stop
            }
            gdk::Key::Escape if s2.has_focus() || !s2.text().is_empty() => {
                s2.set_text("");
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });
    window.add_controller(keys);
    search.connect_search_changed(|e| filter(&e.text()));
    search.connect_activate(|_| focus_first_hit());

    // Drop .torrent files or magnet links anywhere on the window.
    let drop = gtk::DropTarget::new(glib::Type::INVALID, gdk::DragAction::COPY);
    drop.set_types(&[gdk::FileList::static_type(), glib::Type::STRING]);
    drop.connect_drop(|_, value, _, _| {
        if let Ok(files) = value.get::<gdk::FileList>() {
            for f in files.files() {
                if let Some(p) = f.path() {
                    actions::add_torrent_path(&p);
                } else {
                    actions::add_link(&f.uri());
                }
            }
            return true;
        }
        if let Ok(text) = value.get::<String>() {
            for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
                actions::add_link(line);
            }
            return true;
        }
        false
    });
    window.add_controller(drop);

    // Closing the window either hides it (torrents keep going) or quits.
    let app3 = app.clone();
    window.connect_close_request(move |w| {
        if prefs::get().keep_running {
            w.set_visible(false);
            glib::Propagation::Stop
        } else {
            app3.quit();
            glib::Propagation::Proceed
        }
    });

    // Narrow windows (a tiled half-screen) get an icon-only sidebar.
    let apply_width = {
        let nav = nav.clone();
        move |w: &gtk::ApplicationWindow| {
            let width = if w.width() > 0 { w.width() } else { w.default_width() };
            let compact = width > 0 && width < 980;
            if compact == nav.has_css_class("compact") && nav.has_css_class("sized") {
                return;
            }
            nav.add_css_class("sized");
            if compact {
                nav.add_css_class("compact");
            } else {
                nav.remove_css_class("compact");
            }
            set_compact(compact);
        }
    };
    let aw = apply_width.clone();
    window.connect_default_width_notify(move |w| aw(w));
    let aw = apply_width.clone();
    window.connect_realize(move |w| aw(w));
    // Tiled windows are resized by the compositor; watch the real size too.
    let w2 = window.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(400), move || {
        apply_width(&w2);
        glib::ControlFlow::Continue
    });

    let ui = Ui {
        app: app.clone(),
        window,
        stack,
        nav_items,
        nav_counts,
        nav_groups,
        categories_box,
        tags_box,
        filter_groups,
        pages: HashMap::new(),
        sections,
        current: String::new(),
        overlay,
        compact_hide,
    };
    UI.with(|u| *u.borrow_mut() = Some(Rc::new(RefCell::new(ui))));
    rebuild_lists();
    store::on_lists_changed(rebuild_lists);
    live::on_update(update_counts);
}

fn free_space(path: &str) -> Option<u64> {
    let mut p = crate::paths::expand(path);
    while !p.exists() {
        p = p.parent()?.to_path_buf();
    }
    let c = std::ffi::CString::new(p.to_string_lossy().as_bytes()).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` is a valid C string and `st` a properly sized out-parameter.
    let ok = unsafe { libc::statvfs(c.as_ptr(), &mut st) } == 0;
    ok.then(|| st.f_bavail as u64 * st.f_frsize as u64)
}

thread_local! {
    static ALT_CHIPS: RefCell<Vec<gtk::Button>> = const { RefCell::new(Vec::new()) };
}

/// The turtle chip: alternative speed limits on or off.
fn alt_speed_chip() -> gtk::Button {
    let b = gtk::Button::new();
    b.add_css_class("chip");
    b.add_css_class("alt-chip");
    let content = widgets::hbox(6);
    content.append(&gtk::Image::from_icon_name("torrents-turtle-symbolic"));
    content.append(&widgets::label("Slow", ""));
    b.set_child(Some(&content));
    b.connect_clicked(|_| actions::toggle_alt_speed());
    ALT_CHIPS.with(|c| c.borrow_mut().push(b.clone()));
    refresh_alt_speed();
    b
}

pub fn refresh_alt_speed() {
    let on = live::alt_active();
    let p = prefs::get();
    let tip = format!(
        "Alternative speed limits ({} down, {} up){}",
        limit_text(p.alt_dl_limit),
        limit_text(p.alt_ul_limit),
        if p.schedule { ", on a schedule" } else { "" }
    );
    ALT_CHIPS.with(|c| {
        for b in c.borrow().iter() {
            b.set_tooltip_text(Some(&tip));
            if on {
                b.add_css_class("selected");
            } else {
                b.remove_css_class("selected");
            }
        }
    });
}

pub fn limit_text(kib: i32) -> String {
    if kib <= 0 { "unlimited".into() } else { fmt::rate(kib as f64 * 1024.0) }
}

/// Fill the Categories and Tags groups (after one is added or removed).
fn rebuild_lists() {
    let Some(ui) = ui() else { return };
    let (cbox, tbox) = {
        let u = ui.borrow();
        (u.categories_box.clone(), u.tags_box.clone())
    };
    for bx in [&cbox, &tbox] {
        while let Some(c) = bx.first_child() {
            bx.remove(&c);
        }
    }
    {
        let mut u = ui.borrow_mut();
        u.nav_items.retain(|id, _| !id.starts_with("cat:") && !id.starts_with("tag:"));
        u.nav_counts.retain(|id, _| !id.starts_with("cat:") && !id.starts_with("tag:"));
    }
    let compact = COMPACT.with(|c| c.get());
    let add = |bx: &gtk::Box, filter: Filter, icon: &str, menu: bool| {
        let id = filter.id();
        let title = filter.title();
        let (b, count, label) = nav_button(&id, &title, icon, &title, true);
        label.set_visible(!compact);
        if menu {
            actions::attach_list_menu(&b, &filter);
        }
        bx.append(&b);
        let mut u = ui.borrow_mut();
        if let Some(c) = count {
            c.set_visible(!compact);
            u.compact_hide.push(c.clone().upcast());
            u.nav_counts.insert(id.clone(), c);
        }
        u.compact_hide.push(label.upcast());
        u.nav_items.insert(id, b);
    };

    let (h, g) = group_heading("Categories", Some(("New category", || actions::edit_category_dialog(None))));
    g.set_visible(!compact);
    h.set_visible(!compact);
    ui.borrow_mut().compact_hide.push(h.clone().upcast());
    cbox.append(&h);
    add(&cbox, Filter::Category(None), "folder-symbolic", false);
    for name in store::categories().keys() {
        add(&cbox, Filter::Category(Some(name.clone())), "folder-symbolic", true);
    }

    let (h, g) = group_heading("Tags", Some(("New tag", actions::new_tag_dialog)));
    g.set_visible(!compact);
    h.set_visible(!compact);
    ui.borrow_mut().compact_hide.push(h.clone().upcast());
    tbox.append(&h);
    add(&tbox, Filter::Tag(None), "tag-symbolic", false);
    for name in store::tags() {
        add(&tbox, Filter::Tag(Some(name.clone())), "tag-symbolic", true);
    }

    // Keep the highlight on the current filter (or fall back if it was removed).
    let current = ui.borrow().current.clone();
    if Filter::from_id(&current).is_some() {
        if ui.borrow().nav_items.contains_key(&current) {
            mark_active(&current);
        } else {
            navigate("all");
        }
    }
    if let Some(u) = live::latest() {
        update_counts(&u);
    }
}

fn update_counts(u: &crate::engine::Update) {
    let Some(ui) = ui() else { return };
    let counts: Vec<(String, gtk::Label)> = ui.borrow().nav_counts.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    let metas: Vec<(&crate::engine::Status, store::Meta)> = u.torrents.iter().map(|t| (t, store::meta(&t.id))).collect();
    for (id, label) in counts {
        let Some(f) = Filter::from_id(&id) else { continue };
        let n = metas.iter().filter(|(s, m)| f.matches(s, m)).count();
        label.set_text(&n.to_string());
    }
}

fn set_compact(compact: bool) {
    COMPACT.with(|c| c.set(compact));
    NARROW.with(|n| n.set(compact));
    let Some(ui) = ui() else { return };
    let (hide, pages): (Vec<gtk::Widget>, Vec<gtk::ScrolledWindow>) = {
        let u = ui.borrow();
        (u.compact_hide.clone(), u.pages.values().cloned().collect())
    };
    for w in hide {
        w.set_visible(!compact);
    }
    for page in pages {
        mark_page(&page, compact);
    }
    transfers::set_narrow(compact);
}

fn mark_page(page: &gtk::ScrolledWindow, narrow: bool) {
    if let Some(body) = page.child().and_then(|v| v.first_child()) {
        if narrow {
            body.add_css_class("narrow");
        } else {
            body.remove_css_class("narrow");
        }
    }
}

fn ensure_built(id: &'static str) {
    let Some(ui) = ui() else { return };
    if ui.borrow().pages.contains_key(id) {
        return;
    }
    let section = {
        let u = ui.borrow();
        u.sections.iter().find(|s| s.id == id).map(|s| (s.id, s.title, s.description, (s.files)(), s.build, s.fill))
    };
    let Some((sid, title, description, files, build, fill)) = section else { return };
    let page = widgets::page(sid, title, description, &files);
    if fill {
        page.fill();
    }
    build(&page);
    mark_page(&page.root, NARROW.with(|n| n.get()));
    let stack = ui.borrow().stack.clone();
    stack.add_named(&page.root, Some(sid));
    ui.borrow_mut().pages.insert(sid, page.root);
}

fn mark_active(id: &str) {
    let Some(ui) = ui() else { return };
    let u = ui.borrow();
    for (k, b) in &u.nav_items {
        if k == id {
            b.add_css_class("active");
        } else {
            b.remove_css_class("active");
        }
    }
}

pub fn is_transfers() -> bool {
    ui().is_some_and(|u| Filter::from_id(&u.borrow().current).is_some())
}

pub fn navigate(id: &str) {
    let Some(ui) = ui() else { return };
    let (page_id, nav_id): (&'static str, String) = if let Some(f) = Filter::from_id(id) {
        ensure_built("transfers");
        transfers::set_filter(f.clone());
        ("transfers", f.id())
    } else {
        let found = ui.borrow().sections.iter().find(|s| s.id == id && s.id != "transfers").map(|s| s.id);
        match found {
            Some(sid) => (sid, sid.to_string()),
            None => {
                ensure_built("transfers");
                transfers::set_filter(Filter::All);
                ("transfers", "all".to_string())
            }
        }
    };
    ensure_built(page_id);
    mark_active(&nav_id);
    {
        let mut u = ui.borrow_mut();
        u.stack.set_visible_child_name(page_id);
        u.current = nav_id.clone();
    }
    prefs::update(|p| p.last_section = nav_id);
}

fn filter(query: &str) {
    let Some(ui) = ui() else { return };
    let q = query.trim().to_lowercase();
    let terms: Vec<&str> = q.split_whitespace().collect();

    if !terms.is_empty() {
        let ids: Vec<&'static str> = ui.borrow().sections.iter().filter(|s| s.searchable).map(|s| s.id).collect();
        for id in ids {
            ensure_built(id);
        }
    }

    let mut section_hits: HashMap<String, usize> = HashMap::new();
    let title_hits: Vec<&'static str> = {
        let u = ui.borrow();
        u.sections
            .iter()
            .filter(|s| {
                let hay = format!("{} {} {}", s.title, s.description, s.keywords).to_lowercase();
                !terms.is_empty() && terms.iter().all(|t| hay.contains(t))
            })
            .map(|s| s.id)
            .collect()
    };

    SEARCH.with(|s| {
        let items = s.borrow();
        let mut groups_visible: HashMap<gtk::Widget, bool> = HashMap::new();
        for item in items.iter() {
            item.row.remove_css_class("search-hit");
            let hit = !terms.is_empty() && terms.iter().all(|t| item.text.contains(t));
            let whole_section = title_hits.iter().any(|id| *id == item.section);
            let show = terms.is_empty() || hit || whole_section;
            item.row.set_visible(show);
            if hit {
                *section_hits.entry(item.section.clone()).or_default() += 1;
            }
            if let Some(g) = &item.group {
                let e = groups_visible.entry(g.clone()).or_insert(false);
                *e |= show;
            }
        }
        for (g, visible) in groups_visible {
            g.set_visible(visible);
        }
    });

    let u = ui.borrow();
    // While searching, the sidebar lists only the pages with matches.
    for g in &u.filter_groups {
        g.set_visible(terms.is_empty());
    }
    let mut first_match: Option<&'static str> = None;
    for s in &u.sections {
        let Some(button) = u.nav_items.get(s.id) else { continue };
        let visible = terms.is_empty() || section_hits.contains_key(s.id) || title_hits.contains(&s.id);
        button.set_visible(visible);
        if visible && first_match.is_none() && !terms.is_empty() {
            first_match = Some(s.id);
        }
    }
    for (label, ids) in &u.nav_groups {
        label.set_visible(ids.iter().any(|id| u.nav_items.get(id).is_some_and(|b| b.is_visible())));
    }
    let current = u.current.clone();
    let current_visible = u.nav_items.get(&current).is_some_and(|b| b.is_visible());
    drop(u);
    if let Some(first) = first_match
        && (!current_visible || !section_hits.contains_key(&current))
    {
        navigate(first);
    }
    highlight_first_hit(&terms);
}

fn highlight_first_hit(terms: &[&str]) {
    if terms.is_empty() {
        return;
    }
    let Some(ui) = ui() else { return };
    let current = ui.borrow().current.clone();
    let row = SEARCH.with(|s| {
        s.borrow().iter().find(|i| i.section == current && terms.iter().all(|t| i.text.contains(t))).map(|i| i.row.clone())
    });
    if let Some(row) = row {
        row.add_css_class("search-hit");
        scroll_to(&row);
    }
}

fn scroll_to(row: &gtk::Widget) {
    let Some(ui) = ui() else { return };
    let current = ui.borrow().current.clone();
    let Some(page) = ui.borrow().pages.iter().find(|(k, _)| **k == current).map(|(_, v)| v.clone()) else { return };
    let row = row.clone();
    glib::idle_add_local_once(move || {
        if let Some(child) = page.child()
            && let Some(p) = row.compute_point(&child, &gtk::graphene::Point::new(0.0, 0.0))
        {
            let adj = page.vadjustment();
            adj.set_value((p.y() as f64 - 80.0).max(0.0));
        }
    });
}

fn focus_first_hit() {
    let Some(ui) = ui() else { return };
    let current = ui.borrow().current.clone();
    let row = SEARCH
        .with(|s| s.borrow().iter().find(|i| i.section == current && i.row.has_css_class("search-hit")).map(|i| i.row.clone()));
    if let Some(row) = row {
        row.child_focus(gtk::DirectionType::TabForward);
    }
}

/// Show a short message at the bottom of the window.
pub fn toast(message: &str) {
    let Some(ui) = ui() else {
        eprintln!("torrents: {message}");
        return;
    };
    let overlay = ui.borrow().overlay.clone();
    let label = gtk::Label::new(Some(message));
    label.set_wrap(true);
    label.set_max_width_chars(70);
    let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bx.add_css_class("toast");
    bx.append(&label);
    bx.set_halign(gtk::Align::Center);
    bx.set_valign(gtk::Align::End);
    bx.set_can_target(false);
    overlay.add_overlay(&bx);
    glib::timeout_add_local_once(std::time::Duration::from_millis(3500), move || {
        overlay.remove_overlay(&bx);
    });
}

/// A desktop notification (for when the window is hidden or unfocused).
pub fn notify(title: &str, body: &str) {
    let Some(ui) = ui() else { return };
    let (app, window) = {
        let u = ui.borrow();
        (u.app.clone(), u.window.clone())
    };
    if window.is_visible() && window.is_active() {
        toast(&format!("{title}: {body}"));
        return;
    }
    let n = gio::Notification::new(title);
    n.set_body(Some(body));
    n.set_icon(&gio::ThemedIcon::new("io.github.design_nexus.Torrents"));
    app.send_notification(None, &n);
}

fn snapshot_and_quit(app: &gtk::Application, out: std::path::PathBuf) {
    let Some(ui) = ui() else { return };
    let window = ui.borrow().window.clone();
    window.set_opacity(0.01);
    // A distinct title lets a window rule float it at a set size for screenshots.
    window.set_title(Some("Torrents snapshot"));
    let env = |k: &str, d: i32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    window.set_default_size(env("TORRENTS_SNAPSHOT_W", 1120), env("TORRENTS_SNAPSHOT_H", 820));
    // A fixed-size window floats in Hyprland, so the snapshot gets the size asked for
    // instead of whatever tile is free.
    window.set_resizable(false);
    window.present();
    let app = app.clone();
    let delay = env("TORRENTS_SNAPSHOT_DELAY", 2500) as u64;
    // TORRENTS_SNAPSHOT_SELECT=1 selects the first torrent, to show the details pane.
    if std::env::var_os("TORRENTS_SNAPSHOT_SELECT").is_some() {
        glib::timeout_add_local_once(std::time::Duration::from_millis(delay / 2), transfers::select_first);
    }
    glib::timeout_add_local_once(std::time::Duration::from_millis(delay), move || {
        // TORRENTS_SNAPSHOT_PAGE=1 renders the whole current page, not just what fits.
        // TORRENTS_SNAPSHOT_DIALOG=1 renders the open dialog instead of the main window.
        let dialog = std::env::var_os("TORRENTS_SNAPSHOT_DIALOG").is_some().then(|| {
            gtk::Window::list_toplevels()
                .into_iter()
                .filter_map(|w| w.downcast::<gtk::Window>().ok())
                .find(|w| w.is_visible() && w.upcast_ref::<gtk::Widget>() != window.upcast_ref::<gtk::Widget>())
                .and_then(|w| w.child())
        });
        let target = if let Some(d) = dialog {
            d
        } else if std::env::var_os("TORRENTS_SNAPSHOT_PAGE").is_some() {
            UI.with(|cell| cell.borrow().clone()).and_then(|u| {
                let u = u.borrow();
                let page = if Filter::from_id(&u.current).is_some() { "transfers" } else { u.current.as_str() };
                u.pages.iter().find(|(k, _)| **k == page).and_then(|(_, p)| p.child()).and_then(|v| v.first_child())
            })
        } else {
            window.child()
        };
        if let Some(child) = target {
            let paintable = gtk::WidgetPaintable::new(Some(&child));
            let (w, h) = (child.width(), child.height());
            let snapshot = gtk::Snapshot::new();
            snapshot.append_color(&gdk::RGBA::BLACK, &gtk::graphene::Rect::new(0.0, 0.0, w as f32, h as f32));
            paintable.snapshot(&snapshot, w as f64, h as f64);
            if let (Some(node), Some(renderer)) = (snapshot.to_node(), window.renderer()) {
                let texture = renderer.render_texture(node, None);
                match texture.save_to_png(&out) {
                    Ok(()) => {
                        let (min, _, _, _) = window.measure(gtk::Orientation::Horizontal, -1);
                        println!("snapshot {w}x{h} (window needs at least {min} px) -> {}", out.display())
                    }
                    Err(e) => eprintln!("snapshot failed: {e}"),
                }
            }
        }
        app.quit();
    });
}
