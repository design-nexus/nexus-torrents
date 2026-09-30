use crate::widgets::{self, Page};
use crate::{actions, live, paths, prefs, store};
use gtk::prelude::*;

fn set(change: impl FnOnce(&mut prefs::Prefs)) {
    prefs::update(change);
    live::apply_settings();
}

pub fn build(page: &Page) {
    let p = prefs::get();

    // ----- Saving -----
    let g = page.group("Saving");
    let (field, entry) = actions::folder_field(&paths::pretty(&paths::expand(&p.save_path)));
    entry.connect_changed(|e| {
        let v = e.text().to_string();
        set(|p| p.save_path = v);
    });
    g.add(&widgets::stacked_row("Default folder", "Where new torrents are saved unless their category says otherwise.", &field));

    let (field, entry) = actions::folder_field(&paths::pretty(&paths::expand(&p.incomplete_path)));
    entry.connect_changed(|e| {
        let v = e.text().to_string();
        set(|p| p.incomplete_path = v);
    });
    let incomplete =
        widgets::stacked_row("Incomplete folder", "Unfinished downloads wait here, then move to their folder.", &field);
    incomplete.set_visible(p.use_incomplete);
    let inc = incomplete.clone();
    let (r, _) = widgets::switch_row(
        "Keep unfinished downloads apart",
        "Download into a separate folder and move each torrent to its real folder when it's done.",
        p.use_incomplete,
        move |on| {
            set(|p| p.use_incomplete = on);
            inc.set_visible(on);
        },
    );
    g.add(&r);
    // Its own container, so clearing a search doesn't bring it back while the switch is off.
    let holder = widgets::vbox(0);
    holder.append(&incomplete);
    g.add(&holder);

    let (r, _) = widgets::switch_row(
        "Reserve disk space up front",
        "Allocate each file's full size when a download starts, so the disk can't fill up halfway.",
        p.preallocate,
        |on| set(|p| p.preallocate = on),
    );
    g.add(&r);

    // ----- Adding torrents -----
    let g = page.group("Adding torrents");
    let (r, _) = widgets::switch_row(
        "Ask before adding",
        "Show the add dialog to choose the folder, category and files. Off adds straight away.",
        p.show_add_dialog,
        |on| set(|p| p.show_add_dialog = on),
    );
    g.add(&r);
    let (r, _) = widgets::switch_row("Start paused", "New torrents wait until you resume them.", p.start_paused, |on| {
        set(|p| p.start_paused = on)
    });
    g.add(&r);
    g.add(&widgets::segmented_row(
        "Content layout",
        "Whether a torrent's files get a folder of their own.",
        &[("original", "Original"), ("subfolder", "Subfolder"), ("none", "No subfolder")],
        &p.content_layout,
        |v| set(|p| p.content_layout = v),
    ));
    let (r, _) = widgets::switch_row(
        "Delete .torrent files after adding",
        "Tidy up your Downloads folder once a torrent file has been added.",
        p.delete_torrent_file,
        |on| set(|p| p.delete_torrent_file = on),
    );
    g.add(&r);

    // ----- Categories -----
    let g = page.group("Categories");
    g.note("Categories sort torrents in the sidebar and can each have their own folder. Right-click one there to edit it.");
    let (r, _) = widgets::switch_row(
        "A subfolder per category",
        "Categories without a folder of their own save into a subfolder named after them.",
        p.category_subfolders,
        |on| set(|p| p.category_subfolders = on),
    );
    g.add(&r);
    let list = widgets::vbox(6);
    g.add(&list);
    let fill = {
        let list = list.clone();
        move || {
            while let Some(c) = list.first_child() {
                list.remove(&c);
            }
            for (name, cat) in store::categories() {
                let where_ = paths::pretty(&store::save_path(&name));
                let desc = if cat.save_path.is_empty() {
                    format!("Saves to <tt>{}</tt>", gtk::glib::markup_escape_text(&where_))
                } else {
                    format!("Own folder: <tt>{}</tt>", gtk::glib::markup_escape_text(&where_))
                };
                let buttons = widgets::hbox(6);
                let edit = gtk::Button::with_label("Edit");
                let n = name.clone();
                edit.connect_clicked(move |_| actions::edit_category_dialog(Some(n.clone())));
                let n = name.clone();
                let remove = widgets::two_click("Remove", "Click again to remove", move || store::remove_category(&n));
                buttons.append(&edit);
                buttons.append(&remove);
                list.append(&widgets::row(&name, &desc, Some(buttons.upcast_ref())));
            }
        }
    };
    fill();
    store::on_lists_changed(fill);
    let (r, _) = widgets::button_row("New category", "Add a category, with its own folder if you like.", "New…", |_| {
        actions::edit_category_dialog(None)
    });
    g.add(&r);

    // ----- Finished -----
    let g = page.group("When a download finishes");
    let (r, _) = widgets::entry_row(
        "Run a command",
        "Runs with <tt>sh</tt>. <tt>%N</tt> name, <tt>%F</tt> content path, <tt>%D</tt> folder, <tt>%L</tt> category, <tt>%I</tt> info hash.",
        &p.on_finish,
        "notify-send \"Done\" %N",
        true,
        |v| set(|p| p.on_finish = v),
    );
    g.add(&r);

    // ----- Trackers -----
    let g = page.group("Trackers");
    let view = gtk::TextView::new();
    view.add_css_class("code-block");
    view.set_wrap_mode(gtk::WrapMode::None);
    view.set_size_request(-1, 90);
    view.buffer().set_text(&p.extra_trackers);
    view.buffer().connect_changed(|b| {
        let text = b.text(&b.start_iter(), &b.end_iter(), false).to_string();
        set(|p| p.extra_trackers = text);
    });
    g.add(&widgets::stacked_row(
        "Add these trackers to new torrents",
        "One announce URL per line. Public trackers help torrents with few peers.",
        &view,
    ));
}
