//! The boundary to the C++ shim (`engine/shim.cpp`). Only plain values cross it:
//! torrents are named by their id (the hex info-hash, v1 when there is one).

// `create_torrent` is only used by the engine tests for now.
#[allow(dead_code)]
#[cxx::bridge(namespace = "nt")]
pub mod bridge {
    /// One libtorrent setting by name (`download_rate_limit`, `enable_dht`, …).
    #[derive(Debug, Clone, PartialEq)]
    struct Kv {
        key: String,
        value: String,
    }

    #[derive(Debug, Clone, Default)]
    struct AddParams {
        /// A magnet link, or empty when `torrent` holds a .torrent file.
        magnet: String,
        torrent: Vec<u8>,
        /// Fast-resume data from an earlier session (takes precedence).
        resume: Vec<u8>,
        save_path: String,
        /// Rename the top folder (empty keeps the torrent's name).
        name: String,
        paused: bool,
        auto_managed: bool,
        sequential: bool,
        first_last: bool,
        skip_check: bool,
        preallocate: bool,
        disable_pex: bool,
        /// 0 original, 1 always make a subfolder, 2 never make one.
        content_layout: u8,
        /// One per file; empty means all normal.
        file_priorities: Vec<u8>,
        trackers: Vec<String>,
        dl_limit: i32,
        ul_limit: i32,
    }

    #[derive(Debug, Clone, Default)]
    struct Status {
        id: String,
        name: String,
        save_path: String,
        /// libtorrent's torrent_status::state_t.
        state: u8,
        paused: bool,
        auto_managed: bool,
        sequential: bool,
        super_seeding: bool,
        has_metadata: bool,
        moving: bool,
        is_finished: bool,
        is_seeding: bool,
        progress: f64,
        size: i64,
        wanted: i64,
        wanted_done: i64,
        total_done: i64,
        uploaded: i64,
        downloaded: i64,
        dl_rate: i64,
        ul_rate: i64,
        seeds: i32,
        peers: i32,
        seeds_total: i32,
        peers_total: i32,
        queue_position: i32,
        availability: f64,
        error: String,
        added: i64,
        completed: i64,
        last_activity: i64,
        seeding_secs: i64,
        active_secs: i64,
        dl_limit: i32,
        ul_limit: i32,
        num_pieces: i32,
        pieces_done: i32,
        piece_length: i32,
        tracker: String,
        next_announce: i64,
    }

    #[derive(Debug, Clone, Default)]
    struct FileEntry {
        path: String,
        size: i64,
        done: i64,
        priority: u8,
    }

    #[derive(Debug, Clone, Default)]
    struct Peer {
        address: String,
        client: String,
        /// qBittorrent-style flag letters (D d U u K ? I E X H L P).
        flags: String,
        connection: String,
        progress: f64,
        dl_rate: i64,
        ul_rate: i64,
        downloaded: i64,
        uploaded: i64,
    }

    #[derive(Debug, Clone, Default)]
    struct Tracker {
        url: String,
        tier: i32,
        /// 0 not contacted, 1 working, 2 updating, 3 error.
        status: u8,
        message: String,
        seeds: i32,
        peers: i32,
        downloaded: i32,
        next_announce: i64,
    }

    #[derive(Debug, Clone, Default)]
    struct Details {
        id: String,
        files: Vec<FileEntry>,
        peers: Vec<Peer>,
        trackers: Vec<Tracker>,
        /// One byte per piece: 0 missing, 1 downloading, 2 have.
        pieces: Vec<u8>,
        comment: String,
        creator: String,
        created: i64,
        hash_v1: String,
        hash_v2: String,
        private_torrent: bool,
        magnet: String,
    }

    /// What a .torrent (or fetched magnet) contains, for the add dialog.
    #[derive(Debug, Clone, Default)]
    struct TorrentInfo {
        id: String,
        name: String,
        size: i64,
        files: Vec<FileEntry>,
        comment: String,
        private_torrent: bool,
        piece_length: i32,
        num_pieces: i32,
    }

    #[derive(Debug, Clone, Default)]
    struct Event {
        /// See `EventKind` in engine/mod.rs.
        kind: u8,
        id: String,
        text: String,
        data: Vec<u8>,
    }

    #[derive(Debug, Clone, Default)]
    struct Stats {
        dl_rate: i64,
        ul_rate: i64,
        dht_nodes: i64,
        downloaded: i64,
        uploaded: i64,
        listen_port: i32,
        has_incoming: bool,
    }

    unsafe extern "C++" {
        include!("nexus-torrents/engine/shim.h");

        type Session;

        fn new_session(settings: &Vec<Kv>, state: &[u8]) -> UniquePtr<Session>;
        /// Unknown or rejected keys come back as a message.
        fn apply_settings(self: Pin<&mut Session>, settings: &Vec<Kv>) -> String;
        fn add_torrent(self: Pin<&mut Session>, params: &AddParams) -> Result<String>;
        /// Start fetching a magnet's metadata without downloading anything.
        fn fetch_metadata(self: Pin<&mut Session>, magnet: &str, save_path: &str) -> Result<String>;
        fn cancel_fetch(self: Pin<&mut Session>, id: &str);
        fn remove(self: Pin<&mut Session>, id: &str, with_files: bool);
        fn pause(self: Pin<&mut Session>, id: &str);
        fn resume(self: Pin<&mut Session>, id: &str, force: bool, auto_managed: bool);
        fn recheck(self: Pin<&mut Session>, id: &str);
        fn reannounce(self: Pin<&mut Session>, id: &str);
        /// 0 up, 1 down, 2 top, 3 bottom.
        fn queue_move(self: Pin<&mut Session>, id: &str, how: u8);
        fn set_file_priorities(self: Pin<&mut Session>, id: &str, priorities: &[u8]);
        fn set_limits(self: Pin<&mut Session>, id: &str, dl: i32, ul: i32);
        fn set_sequential(self: Pin<&mut Session>, id: &str, on: bool);
        fn set_first_last(self: Pin<&mut Session>, id: &str, on: bool);
        fn set_super_seeding(self: Pin<&mut Session>, id: &str, on: bool);
        fn move_storage(self: Pin<&mut Session>, id: &str, path: &str);
        fn rename(self: Pin<&mut Session>, id: &str, name: &str);
        fn add_tracker(self: Pin<&mut Session>, id: &str, url: &str, tier: i32);
        fn remove_tracker(self: Pin<&mut Session>, id: &str, url: &str);
        fn save_resume(self: Pin<&mut Session>, id: &str);
        /// Ask every torrent with changes for its resume data. Returns how many were asked.
        fn save_all_resume(self: Pin<&mut Session>, all: bool) -> i32;
        fn post_updates(self: Pin<&mut Session>);
        fn statuses(self: Pin<&mut Session>) -> Vec<Status>;
        fn details(self: Pin<&mut Session>, id: &str, with_peers: bool) -> Details;
        fn poll(self: Pin<&mut Session>) -> Vec<Event>;
        fn stats(self: Pin<&mut Session>) -> Stats;
        fn session_state(self: &Session) -> Vec<u8>;
        fn torrent_file(self: &Session, id: &str) -> Vec<u8>;
        fn pause_session(self: Pin<&mut Session>);

        fn parse_torrent(data: &[u8]) -> Result<TorrentInfo>;
        fn parse_magnet(uri: &str) -> Result<TorrentInfo>;
        /// Make a .torrent from a file or folder (piece size chosen automatically).
        fn create_torrent(path: &str, trackers: &Vec<String>, private_torrent: bool) -> Result<Vec<u8>>;
    }
}

pub use bridge::*;
