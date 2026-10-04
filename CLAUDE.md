# Torrents — notes for working on this repo

- GTK4 (gtk4-rs 0.11) + Rust, no libadwaita. The window is one flat, monospace surface
  split by hairlines: a top bar (sidebar toggle, `Torrents / <filter>`, search, settings,
  close), the sidebar, the page and a status bar (`F1 Shortcuts`, speeds, port, DHT,
  free space, the slow-mode turtle). Pages have no title header. The settings pages
  (group `Settings` in `sections::all`) open in `settings_dialog.rs`, a card over the
  window with its own search over `widgets::SEARCH`; `navigate("speed")` etc. open it.
  Theme, window, widgets, graph and stylesheet started as copies of Tasks (`~/Projects/nexus-tasks`).
  Every colour is a `@theme_*` token; cairo drawing gets colours from `theme::palette()`.
- Engine: libtorrent-rasterbar 2.x through `engine/shim.{h,cpp}` and the cxx bridge in
  `src/engine/ffi.rs`. Only plain values cross it; torrents are keyed by
  `info_hashes().get_best()` in hex. Settings go over by libtorrent name
  (`setting_by_name`), built from prefs in `engine/settings.rs`. Build needs `boost` headers.
- `engine/mod.rs` runs its own thread that owns the session. The UI sends `Command`s, and
  the thread sends an `Update` about once a second (statuses, stats, events, and details
  for the selected torrent) over an async channel. The engine thread writes resume
  data (`resume/<id>.fastresume`), `queue` and `session.dat` itself.
  On quit, `live::shutdown` blocks until the resume data is saved (10 s cap).
- UI state beside libtorrent's (categories, tags, incomplete-folder target, first/last
  flag) is in `store.rs` (`torrents.json`, `categories.toml`), with debounced writes.
- Transfers table: one `TorrentObject` per torrent. `relayout` works out order and
  visibility, and pokes the GTK filter/sorter only when the order changes.
  GTK re-binds rows synchronously, so never hold a `STATE` borrow across store,
  filter, sorter, selection or column-visibility changes. `table.rs` is the same idea
  for the details and search tables.
- Search runs qBittorrent nova3 plugins with our own MIT shims (`search/*.py`, embedded
  and installed to `~/.local/share/torrents/search`). There's one `python3` process per
  engine, and results come back as JSON lines.
- Checks: `cargo clippy --all-targets -- -D warnings`, `cargo test`, and
  `cargo test -- --ignored` (seeds a file between two local sessions over 127.0.0.1:46881–2).
- Visual check with a scratch profile (quit any running instance first; it's single-instance):
  `XDG_CONFIG_HOME=/tmp/c XDG_DATA_HOME=/tmp/d TORRENTS_SNAPSHOT=/tmp/x.png torrents --section speed`.
  - `TORRENTS_SNAPSHOT_PAGE=1` renders the whole page.
  - `TORRENTS_SNAPSHOT_SELECT=1` selects the first torrent.
  - `TORRENTS_SNAPSHOT_DIALOG=1` renders an open dialog.
  - `TORRENTS_SNAPSHOT_SEARCH=words` runs a search.
  - `TORRENTS_SNAPSHOT_DELAY=ms` waits before rendering.
