//! Settings as a card over the window: the settings pages listed on the left,
//! the chosen page on the right, and a search across all of them.

use crate::sections::{self, Section};
use crate::widgets::{self, SEARCH};
use crate::{cmd, paths, window};
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

/// The sections in this group are settings pages, shown here and not in the sidebar.
pub const GROUP: &str = "Settings";

/// Each settings page's one-line summary in the dialog's list.
const SUMMARIES: &[(&str, &str)] = &[
    ("downloads", "Folders, adding, categories"),
    ("connection", "Port, interface, proxy"),
    ("speed", "Limits, schedule"),
    ("bittorrent", "Discovery, queue, seeding"),
    ("appearance", "Theme, window, shortcuts"),
];

struct Dialog {
    veil: gtk::Box,
    card: gtk::Box,
    title: gtk::Label,
    /// Holds the current page's "open the file" button.
    edit: gtk::Box,
    search: gtk::SearchEntry,
    empty: gtk::Label,
    stack: gtk::Stack,
    sections: Vec<Section>,
    buttons: HashMap<&'static str, gtk::Button>,
    pages: HashMap<&'static str, gtk::ScrolledWindow>,
    current: &'static str,
}

thread_local! {
    static DIALOG: RefCell<Option<Dialog>> = const { RefCell::new(None) };
}

pub fn is_settings(id: &str) -> bool {
    sections::all().iter().any(|s| s.id == id && s.group == GROUP)
}

/// Open the dialog, on `page` if given, else where it was last.
pub fn open(page: Option<&str>) {
    let Some(overlay) = window::overlay() else { return };
    if DIALOG.with(|d| d.borrow().is_none()) {
        let dialog = build();
        overlay.add_overlay(&dialog.veil);
        DIALOG.with(|d| *d.borrow_mut() = Some(dialog));
    }
    let id = DIALOG.with(|d| {
        let d = d.borrow();
        let d = d.as_ref()?;
        d.veil.set_visible(true);
        let want = page.and_then(|p| d.sections.iter().find(|s| s.id == p)).map(|s| s.id);
        want.or((!d.current.is_empty()).then_some(d.current)).or(d.sections.first().map(|s| s.id))
    });
    if let Some(w) = window::window() {
        fit(&w);
    }
    if let Some(id) = id {
        if page.is_some() {
            clear_search();
        }
        select(id);
    }
}

pub fn is_open() -> bool {
    DIALOG.with(|d| d.borrow().as_ref().is_some_and(|d| d.veil.is_visible()))
}

pub fn close() {
    DIALOG.with(|d| {
        if let Some(d) = d.borrow().as_ref() {
            d.veil.set_visible(false);
        }
    });
}

/// Esc clears the search first, then closes.
pub fn escape() {
    let search = DIALOG.with(|d| d.borrow().as_ref().map(|d| d.search.clone()));
    match search {
        Some(s) if !s.text().is_empty() => s.set_text(""),
        _ => close(),
    }
}

pub fn focus_search() {
    if let Some(s) = DIALOG.with(|d| d.borrow().as_ref().map(|d| d.search.clone())) {
        s.grab_focus();
    }
}

fn clear_search() {
    if let Some(s) = DIALOG.with(|d| d.borrow().as_ref().map(|d| d.search.clone()))
        && !s.text().is_empty()
    {
        s.set_text("");
    }
}

/// Size the card to the window: as large as the window allows, up to a limit.
pub fn fit(window: &gtk::ApplicationWindow) {
    let (w, h) = (window.width(), window.height());
    if w <= 0 || h <= 0 {
        return;
    }
    let size = ((w - 48).clamp(0, 1040), (h - 48).clamp(0, 720));
    DIALOG.with(|d| {
        if let Some(d) = d.borrow().as_ref()
            && d.card.size_request() != size
        {
            d.card.set_size_request(size.0, size.1);
            // A tiled half-screen window gets a slimmer list and less padding.
            if size.0 < 900 {
                d.card.add_css_class("narrow");
            } else {
                d.card.remove_css_class("narrow");
            }
        }
    });
}

fn build() -> Dialog {
    let veil = gtk::Box::new(gtk::Orientation::Vertical, 0);
    veil.add_css_class("settings-veil");
    veil.set_hexpand(true);
    veil.set_vexpand(true);
    veil.set_visible(false);
    let card = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    card.add_css_class("settings-dialog");
    card.set_halign(gtk::Align::Center);
    card.set_valign(gtk::Align::Center);
    card.set_vexpand(true);
    card.set_overflow(gtk::Overflow::Hidden);
    veil.append(&card);

    // A click on the dimmed window around the card closes it.
    let click = gtk::GestureClick::new();
    let c = card.clone();
    click.connect_pressed(move |g, _, x, y| {
        let Some(veil) = g.widget() else { return };
        let inside = c.compute_bounds(&veil).is_some_and(|b| b.contains_point(&gtk::graphene::Point::new(x as f32, y as f32)));
        if !inside {
            close();
        }
    });
    veil.add_controller(click);

    // ----- The list of pages -----
    let side = widgets::vbox(0);
    side.add_css_class("settings-dialog-side");
    side.append(&widgets::label("SETTINGS", "dialog-heading"));
    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Search settings"));
    search.add_css_class("settings-search");
    side.append(&search);
    let list = widgets::vbox(4);
    let sections: Vec<Section> = sections::all().into_iter().filter(|s| s.group == GROUP).collect();
    let mut buttons = HashMap::new();
    for s in &sections {
        let button = gtk::Button::new();
        button.add_css_class("dialog-nav");
        button.set_tooltip_text(Some(s.description));
        let content = widgets::hbox(14);
        let image = gtk::Image::from_icon_name(s.icon);
        image.set_valign(gtk::Align::Start);
        content.append(&image);
        let text = widgets::vbox(1);
        text.append(&widgets::label(s.title, "dialog-nav-title"));
        if let Some((_, summary)) = SUMMARIES.iter().find(|(id, _)| *id == s.id) {
            let l = widgets::label(summary, "dialog-nav-summary");
            l.set_ellipsize(gtk::pango::EllipsizeMode::End);
            text.append(&l);
        }
        content.append(&text);
        button.set_child(Some(&content));
        let id = s.id;
        button.connect_clicked(move |_| {
            clear_search();
            select(id);
        });
        list.append(&button);
        buttons.insert(s.id, button);
    }
    let list_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::External)
        .vexpand(true)
        .child(&list)
        .build();
    side.append(&list_scroll);
    card.append(&side);

    // ----- The chosen page -----
    let main = widgets::vbox(0);
    main.add_css_class("settings-dialog-main");
    main.set_hexpand(true);
    let head = widgets::hbox(4);
    head.add_css_class("dialog-head");
    let title = widgets::label("", "dialog-title");
    title.set_hexpand(true);
    head.append(&title);
    let edit = widgets::hbox(0);
    head.append(&edit);
    let x = gtk::Button::from_icon_name("window-close-symbolic");
    x.add_css_class("bar-button");
    x.set_tooltip_text(Some("Close (Esc)"));
    x.set_valign(gtk::Align::Center);
    x.connect_clicked(|_| close());
    head.append(&x);
    main.append(&head);
    let empty = widgets::label("", "dialog-empty");
    empty.set_xalign(0.5);
    empty.set_vexpand(true);
    empty.set_visible(false);
    main.append(&empty);
    let stack = gtk::Stack::new();
    stack.set_vexpand(true);
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(if crate::prefs::get().reduce_motion { 0 } else { 160 });
    main.append(&stack);
    card.append(&main);

    search.connect_search_changed(|e| on_search(&e.text()));
    search.connect_activate(|_| focus_first_hit());
    Dialog {
        veil,
        card,
        title,
        edit,
        search,
        empty,
        stack,
        sections,
        buttons,
        pages: HashMap::new(),
        current: "",
    }
}

fn ensure_built(id: &'static str) {
    let todo = DIALOG.with(|d| {
        let d = d.borrow();
        let d = d.as_ref()?;
        if d.pages.contains_key(id) {
            return None;
        }
        d.sections.iter().find(|s| s.id == id).map(|s| (s.build, s.fill))
    });
    let Some((build, fill)) = todo else { return };
    let page = widgets::page(id);
    if fill {
        page.fill();
    }
    build(&page);
    DIALOG.with(|d| {
        if let Some(d) = d.borrow_mut().as_mut() {
            d.stack.add_named(&page.root, Some(id));
            d.pages.insert(id, page.root);
        }
    });
}

fn select(id: &'static str) {
    ensure_built(id);
    DIALOG.with(|d| {
        let mut d = d.borrow_mut();
        let Some(d) = d.as_mut() else { return };
        let Some(s) = d.sections.iter().find(|s| s.id == id) else { return };
        d.title.set_text(s.title);
        while let Some(c) = d.edit.first_child() {
            d.edit.remove(&c);
        }
        let files = (s.files)();
        if !files.is_empty() {
            d.edit.append(&edit_button(&files));
        }
        for (k, b) in &d.buttons {
            if *k == id {
                b.add_css_class("active");
            } else {
                b.remove_css_class("active");
            }
        }
        d.stack.set_visible_child_name(id);
        d.current = id;
    });
}

/// Open the page's config file, or pick one when there are several.
fn edit_button(files: &[PathBuf]) -> gtk::Widget {
    if let [path] = files {
        let path = path.clone();
        let b = gtk::Button::from_icon_name("text-editor-symbolic");
        b.add_css_class("bar-button");
        b.set_valign(gtk::Align::Center);
        b.set_tooltip_text(Some(&format!("Open {}", paths::pretty(&path))));
        b.connect_clicked(move |_| cmd::open_in_editor(&path));
        return b.upcast();
    }
    let menu = gtk::MenuButton::new();
    menu.set_icon_name("text-editor-symbolic");
    menu.add_css_class("bar-button");
    menu.set_valign(gtk::Align::Center);
    menu.set_tooltip_text(Some("Open a config file"));
    let list = widgets::vbox(2);
    let popover = gtk::Popover::new();
    for path in files {
        let b = gtk::Button::with_label(&paths::pretty(path));
        b.add_css_class("flat");
        if let Some(label) = b.child().and_downcast::<gtk::Label>() {
            label.set_xalign(0.0);
        }
        let (p, pop) = (path.clone(), popover.clone());
        b.connect_clicked(move |_| {
            pop.popdown();
            cmd::open_in_editor(&p);
        });
        list.append(&b);
    }
    popover.set_child(Some(&list));
    menu.set_popover(Some(&popover));
    menu.upcast()
}

/// Filter every settings card as you type: pages without a match leave the
/// list, and the first match is shown and highlighted.
fn on_search(query: &str) {
    let q = query.trim().to_lowercase();
    let terms: Vec<&str> = q.split_whitespace().collect();
    let ids: Vec<&'static str> =
        DIALOG.with(|d| d.borrow().as_ref().map(|d| d.sections.iter().map(|s| s.id).collect()).unwrap_or_default());
    if !terms.is_empty() {
        for id in &ids {
            ensure_built(id);
        }
    }
    let title_hits: Vec<&'static str> = DIALOG.with(|d| {
        let d = d.borrow();
        let Some(d) = d.as_ref() else { return Vec::new() };
        d.sections
            .iter()
            .filter(|s| {
                let hay = format!("{} {} {}", s.title, s.description, s.keywords).to_lowercase();
                !terms.is_empty() && terms.iter().all(|t| hay.contains(t))
            })
            .map(|s| s.id)
            .collect()
    });

    let mut hits: HashMap<String, usize> = HashMap::new();
    SEARCH.with(|s| {
        let items = s.borrow();
        let mut groups: HashMap<gtk::Widget, bool> = HashMap::new();
        for item in items.iter().filter(|i| ids.contains(&i.section.as_str())) {
            item.row.remove_css_class("search-hit");
            let hit = !terms.is_empty() && terms.iter().all(|t| item.text.contains(t));
            let whole = title_hits.iter().any(|id| *id == item.section);
            let show = terms.is_empty() || hit || whole;
            item.row.set_visible(show);
            if hit || whole {
                *hits.entry(item.section.clone()).or_default() += 1;
            }
            if let Some(g) = &item.group {
                *groups.entry(g.clone()).or_insert(false) |= show;
            }
        }
        for (g, visible) in groups {
            g.set_visible(visible);
        }
    });

    let target = DIALOG.with(|d| {
        let d = d.borrow();
        let d = d.as_ref()?;
        let mut first = None;
        for s in &d.sections {
            let visible = terms.is_empty() || hits.contains_key(s.id);
            if let Some(b) = d.buttons.get(s.id) {
                b.set_visible(visible);
            }
            if visible && first.is_none() {
                first = Some(s.id);
            }
        }
        let nothing = !terms.is_empty() && first.is_none();
        d.empty.set_text(&format!("No settings match “{}”", query.trim()));
        d.empty.set_visible(nothing);
        d.stack.set_visible(!nothing);
        if terms.is_empty() || hits.contains_key(d.current) { None } else { first }
    });
    if let Some(id) = target {
        select(id);
    }
    highlight_first_hit(&terms);
}

fn current_page() -> Option<(&'static str, gtk::ScrolledWindow)> {
    DIALOG.with(|d| d.borrow().as_ref().and_then(|d| d.pages.get(d.current).map(|p| (d.current, p.clone()))))
}

fn highlight_first_hit(terms: &[&str]) {
    if terms.is_empty() {
        return;
    }
    let Some((current, page)) = current_page() else { return };
    let row = SEARCH.with(|s| {
        s.borrow().iter().find(|i| i.section == current && terms.iter().all(|t| i.text.contains(t))).map(|i| i.row.clone())
    });
    if let Some(row) = row {
        row.add_css_class("search-hit");
        glib::idle_add_local_once(move || {
            if let Some(child) = page.child()
                && let Some(p) = row.compute_point(&child, &gtk::graphene::Point::new(0.0, 0.0))
            {
                page.vadjustment().set_value((p.y() as f64 - 80.0).max(0.0));
            }
        });
    }
}

fn focus_first_hit() {
    let Some((current, _)) = current_page() else { return };
    let row = SEARCH
        .with(|s| s.borrow().iter().find(|i| i.section == current && i.row.has_css_class("search-hit")).map(|i| i.row.clone()));
    if let Some(row) = row {
        row.child_focus(gtk::DirectionType::TabForward);
    }
}
