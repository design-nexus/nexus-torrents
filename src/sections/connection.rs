use crate::widgets::{self, Page};
use crate::{live, prefs};
use gtk::prelude::*;

fn set(change: impl FnOnce(&mut prefs::Prefs)) {
    prefs::update(change);
    live::apply_settings();
}

/// Network interfaces worth binding to (not loopback).
fn interfaces() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir("/sys/class/net")
        .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n != "lo").collect())
        .unwrap_or_default();
    names.sort();
    names
}

pub fn build(page: &Page) {
    let p = prefs::get();

    // ----- Listening -----
    let g = page.group("Listening");
    let (r, spin) = widgets::spin_row(
        "Port",
        "Other peers connect to you here. Forward it on your router if UPnP isn't available.",
        (1024.0, 65535.0),
        1.0,
        0,
        p.port as f64,
        "",
        |v| set(|p| p.port = v as u16),
    );
    let random = gtk::Button::with_label("Random");
    random.set_valign(gtk::Align::Center);
    random.connect_clicked(move |_| spin.set_value(prefs::random_port() as f64));
    r.append(&random);
    g.add(&r);
    let (r, _) = widgets::switch_row(
        "Open the port automatically",
        "Ask your router to forward the port with UPnP and NAT-PMP.",
        p.upnp,
        |on| set(|p| p.upnp = on),
    );
    g.add(&r);
    g.add(&widgets::segmented_row(
        "Protocols",
        "µTP is gentler on your connection; plain TCP reaches some older clients.",
        &[("both", "TCP and µTP"), ("tcp", "TCP"), ("utp", "µTP")],
        &p.protocol,
        |v| set(|p| p.protocol = v),
    ));

    // ----- Interface -----
    let g = page.group("Network interface");
    let mut options = vec![("".to_string(), "Any interface".to_string())];
    let mut names = interfaces();
    if !p.interface.is_empty() && !names.contains(&p.interface) {
        names.push(p.interface.clone());
    }
    options.extend(names.into_iter().map(|n| (n.clone(), n)));
    let (r, _) = widgets::choice_row(
        "Bind to",
        "Pick your VPN's interface (<tt>wg0</tt>, <tt>tun0</tt>) and torrents stop whenever the VPN is down.",
        options,
        &p.interface,
        |v| set(|p| p.interface = v),
    );
    g.add(&r);

    // ----- Limits -----
    let g = page.group("Connections");
    let (r, _) = widgets::spin_row(
        "Most connections",
        "Across all torrents. Too many can overwhelm a home router.",
        (0.0, 10000.0),
        10.0,
        0,
        p.max_connections as f64,
        "",
        |v| set(|p| p.max_connections = v as i32),
    );
    g.add(&r);
    let (r, _) = widgets::spin_row(
        "Upload slots",
        "How many peers you upload to at once. 0 means no limit.",
        (0.0, 1000.0),
        1.0,
        0,
        p.max_upload_slots as f64,
        "",
        |v| set(|p| p.max_upload_slots = v as i32),
    );
    g.add(&r);

    // ----- Proxy -----
    let g = page.group("Proxy");
    let details = widgets::vbox(6);
    details.set_visible(p.proxy_type != "none");
    let (r, _) = widgets::entry_row("Host", "", &p.proxy_host, "proxy.example", true, |v| set(|p| p.proxy_host = v));
    details.append(&r);
    let (r, _) =
        widgets::spin_row("Port", "", (1.0, 65535.0), 1.0, 0, p.proxy_port as f64, "", |v| set(|p| p.proxy_port = v as u16));
    details.append(&r);
    let (r, _) = widgets::entry_row("User name", "Leave empty if the proxy doesn't need one.", &p.proxy_user, "", false, |v| {
        set(|p| p.proxy_user = v)
    });
    details.append(&r);
    let (r, e) = widgets::entry_row("Password", "", &p.proxy_password, "", false, |v| set(|p| p.proxy_password = v));
    e.set_visibility(false);
    e.set_input_purpose(gtk::InputPurpose::Password);
    details.append(&r);
    let (r, _) = widgets::switch_row(
        "Use it for peers too",
        "Send peer connections through the proxy, not just trackers.",
        p.proxy_peers,
        |on| set(|p| p.proxy_peers = on),
    );
    details.append(&r);
    let (r, _) = widgets::switch_row(
        "Look up names through it",
        "Let the proxy resolve tracker host names, so DNS doesn't leak.",
        p.proxy_hostnames,
        |on| set(|p| p.proxy_hostnames = on),
    );
    details.append(&r);
    let d = details.clone();
    let (r, _) = widgets::choice_row(
        "Proxy",
        "Route traffic through a SOCKS or HTTP proxy.",
        widgets::opts(&[("none", "None"), ("socks5", "SOCKS5"), ("socks4", "SOCKS4"), ("http", "HTTP")]),
        &p.proxy_type,
        move |v| {
            d.set_visible(v != "none");
            set(|p| p.proxy_type = v);
        },
    );
    g.add(&r);
    g.add(&details);
}
