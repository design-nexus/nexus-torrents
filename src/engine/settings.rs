//! Turning the preferences into libtorrent settings (by name; the shim looks each
//! one up with `setting_by_name`).

use super::Kv;
use crate::prefs::Prefs;

pub const USER_AGENT: &str = concat!("Torrents/", env!("CARGO_PKG_VERSION"), " libtorrent/2");
/// Azureus-style peer id prefix: NX, version 0.1.0.
pub const FINGERPRINT: &str = "-NX0100-";

fn kv(out: &mut Vec<Kv>, key: &str, value: impl ToString) {
    out.push(Kv { key: key.into(), value: value.to_string() });
}

fn kib(v: i32) -> i64 {
    if v <= 0 { 0 } else { v as i64 * 1024 }
}

/// Whether the alternative speed limits apply right now.
pub fn alt_active(p: &Prefs, weekday: u32, minute_of_day: u32) -> bool {
    if !p.schedule {
        return p.alt_enabled;
    }
    in_schedule(&p.schedule_from, &p.schedule_to, &p.schedule_days, weekday, minute_of_day)
}

fn parse_hm(s: &str) -> Option<u32> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// `weekday` is 0 for Monday … 6 for Sunday. A window may cross midnight.
pub fn in_schedule(from: &str, to: &str, days: &str, weekday: u32, minute: u32) -> bool {
    let (Some(a), Some(b)) = (parse_hm(from), parse_hm(to)) else { return false };
    let day_ok = |d: u32| match days {
        "weekdays" => d < 5,
        "weekends" => d >= 5,
        _ => true,
    };
    if a <= b {
        day_ok(weekday) && minute >= a && minute < b
    } else if minute >= a {
        day_ok(weekday)
    } else if minute < b {
        // After midnight: the window started yesterday.
        day_ok((weekday + 6) % 7)
    } else {
        false
    }
}

pub fn pack(p: &Prefs, alt: bool) -> Vec<Kv> {
    let mut out = Vec::new();
    let o = &mut out;
    kv(o, "user_agent", USER_AGENT);
    kv(o, "peer_fingerprint", FINGERPRINT);

    // Connection
    let listen = if p.interface.trim().is_empty() {
        format!("0.0.0.0:{0},[::]:{0}", p.port)
    } else {
        format!("{}:{}", p.interface.trim(), p.port)
    };
    kv(o, "listen_interfaces", listen);
    kv(o, "outgoing_interfaces", p.interface.trim());
    kv(o, "enable_upnp", p.upnp);
    kv(o, "enable_natpmp", p.upnp);
    kv(o, "connections_limit", if p.max_connections > 0 { p.max_connections } else { i32::MAX / 2 });
    kv(o, "unchoke_slots_limit", if p.max_upload_slots > 0 { p.max_upload_slots } else { -1 });
    let (tcp, utp) = match p.protocol.as_str() {
        "tcp" => (true, false),
        "utp" => (false, true),
        _ => (true, true),
    };
    kv(o, "enable_outgoing_tcp", tcp);
    kv(o, "enable_incoming_tcp", tcp);
    kv(o, "enable_outgoing_utp", utp);
    kv(o, "enable_incoming_utp", utp);

    // Proxy: libtorrent's proxy_type_t (none 0, socks4 1, socks5 2, socks5_pw 3, http 4, http_pw 5).
    let with_login = !p.proxy_user.is_empty();
    let proxy = match p.proxy_type.as_str() {
        "socks4" => 1,
        "socks5" if with_login => 3,
        "socks5" => 2,
        "http" if with_login => 5,
        "http" => 4,
        _ => 0,
    };
    kv(o, "proxy_type", proxy);
    if proxy != 0 {
        kv(o, "proxy_hostname", &p.proxy_host);
        kv(o, "proxy_port", p.proxy_port);
        kv(o, "proxy_username", &p.proxy_user);
        kv(o, "proxy_password", &p.proxy_password);
    }
    kv(o, "proxy_peer_connections", p.proxy_peers);
    kv(o, "proxy_tracker_connections", true);
    kv(o, "proxy_hostnames", p.proxy_hostnames);

    // Speed
    let (dl, ul) = if alt { (p.alt_dl_limit, p.alt_ul_limit) } else { (p.dl_limit, p.ul_limit) };
    kv(o, "download_rate_limit", kib(dl));
    kv(o, "upload_rate_limit", kib(ul));
    kv(o, "rate_limit_ip_overhead", p.limit_overhead);

    // BitTorrent
    kv(o, "enable_dht", p.dht);
    kv(o, "enable_lsd", p.lsd);
    kv(o, "anonymous_mode", p.anonymous);
    // pe_forced 0, pe_enabled 1, pe_disabled 2; pe_plaintext 1, pe_rc4 2, pe_both 3.
    let (policy, level, prefer) = match p.encryption.as_str() {
        "require" => (0, 2, true),
        "disable" => (2, 1, false),
        "prefer" => (1, 3, true),
        _ => (1, 3, false),
    };
    kv(o, "out_enc_policy", policy);
    kv(o, "in_enc_policy", policy);
    kv(o, "allowed_enc_level", level);
    kv(o, "prefer_rc4", prefer);

    // Queueing: without it every torrent is active at once.
    let limit = |on: bool, v: i32| if on && v > 0 { v } else { -1 };
    kv(o, "active_downloads", limit(p.queueing, p.max_active_downloads));
    kv(o, "active_seeds", limit(p.queueing, p.max_active_uploads));
    kv(o, "active_limit", limit(p.queueing, p.max_active_torrents));
    kv(o, "active_checking", 1);
    kv(o, "dont_count_slow_torrents", p.queueing && p.ignore_slow);
    kv(o, "inactive_down_rate", kib(p.slow_dl.max(1)));
    kv(o, "inactive_up_rate", kib(p.slow_ul.max(1)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get<'a>(kv: &'a [Kv], key: &str) -> Option<&'a str> {
        kv.iter().find(|k| k.key == key).map(|k| k.value.as_str())
    }

    #[test]
    fn maps_speed_limits() {
        let p = Prefs { dl_limit: 100, ul_limit: 0, alt_dl_limit: 10, alt_ul_limit: 5, ..Prefs::default() };
        let normal = pack(&p, false);
        assert_eq!(get(&normal, "download_rate_limit"), Some("102400"));
        assert_eq!(get(&normal, "upload_rate_limit"), Some("0"));
        let alt = pack(&p, true);
        assert_eq!(get(&alt, "download_rate_limit"), Some("10240"));
        assert_eq!(get(&alt, "upload_rate_limit"), Some("5120"));
    }

    #[test]
    fn maps_interface_and_port() {
        let p = Prefs { port: 51413, interface: "wg0".into(), ..Prefs::default() };
        let kv = pack(&p, false);
        assert_eq!(get(&kv, "listen_interfaces"), Some("wg0:51413"));
        assert_eq!(get(&kv, "outgoing_interfaces"), Some("wg0"));
        let p = Prefs { port: 6881, ..Prefs::default() };
        assert_eq!(get(&pack(&p, false), "listen_interfaces"), Some("0.0.0.0:6881,[::]:6881"));
    }

    #[test]
    fn queueing_off_means_unlimited() {
        let p = Prefs { queueing: false, ..Prefs::default() };
        let kv = pack(&p, false);
        assert_eq!(get(&kv, "active_downloads"), Some("-1"));
        assert_eq!(get(&kv, "active_limit"), Some("-1"));
    }

    #[test]
    fn maps_encryption_and_proxy() {
        let p = Prefs { encryption: "require".into(), proxy_type: "socks5".into(), proxy_user: "me".into(), ..Prefs::default() };
        let kv = pack(&p, false);
        assert_eq!(get(&kv, "out_enc_policy"), Some("0"));
        assert_eq!(get(&kv, "allowed_enc_level"), Some("2"));
        assert_eq!(get(&kv, "proxy_type"), Some("3"));
    }

    #[test]
    fn schedule_windows() {
        // 08:00–20:00 every day.
        assert!(in_schedule("08:00", "20:00", "every", 2, 9 * 60));
        assert!(!in_schedule("08:00", "20:00", "every", 2, 21 * 60));
        // Weekdays only: Saturday is out.
        assert!(!in_schedule("08:00", "20:00", "weekdays", 5, 9 * 60));
        // Overnight 22:00–06:00 that starts on weekdays: Saturday 01:00 belongs to Friday.
        assert!(in_schedule("22:00", "06:00", "weekdays", 5, 60));
        assert!(!in_schedule("22:00", "06:00", "weekdays", 0, 60));
        assert!(!in_schedule("bad", "06:00", "every", 0, 60));
    }
}
