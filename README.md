# Torrents

A BitTorrent client for [Omarchy](https://omarchy.org) with qBittorrent's engine and
feature set, styled like the other Nexus apps (Strata, Settings, Tasks): dark, calm
and dense, themed from the Omarchy palette, and at home tiled at half-screen.

## What it does

- **The engine is qBittorrent's.** Torrents runs on
  [libtorrent-rasterbar](https://libtorrent.org) 2.x through a small C++ shim, so it
  supports what qBittorrent supports: v1, v2 and hybrid torrents, magnet links, DHT,
  peer exchange, local discovery, µTP, encryption, UPnP/NAT-PMP, proxies, and
  fast-resume.
- **Transfers:** every torrent with its progress, status, speeds, ETA, seeds, peers,
  ratio, category and tags.
  - Filter by status in the sidebar (Downloading, Seeding, Completed, Paused, Active,
    Inactive, Errored), by category or by tag, each with a live count. You can also
    filter by name.
  - Sort by any column. Columns you don't need can be hidden, and the less important
    ones step aside when the window is narrow.
  - The right-click menu has everything qBittorrent's does: resume, force resume,
    pause, remove (optionally with the files), move, rename, speed limits, category,
    tags, queue order, recheck, reannounce, download in order, first and last pieces
    first, super seeding, copy name/hash/magnet, open the folder, export the .torrent.
- **Details pane:** a pieces bar and all the figures (General), trackers you can add
  to or remove, live peers with qBittorrent's flag letters, files with per-file
  priorities, and a speed graph.
- **Adding:** .torrent files, magnet links, links to .torrent files and bare
  info-hashes, from the toolbar (<kbd>Ctrl</kbd>+<kbd>O</kbd> and <kbd>Ctrl</kbd>+<kbd>U</kbd>),
  by dropping them on the window, from the command line, or from your browser. The
  add dialog (like qBittorrent's) picks the folder, category, tags, name, content
  layout and files. For a magnet link, it fetches the metadata first.
- **Queueing, categories and tags:** limit how many torrents download and seed at
  once. Slow torrents can be left out of the count. Categories can have their own
  folders.
- **Search:** qBittorrent's search plugins work as they are. **Import from
  qBittorrent** copies the ones you already have. You can also install a plugin from a
  file or a link. Every enabled plugin is searched at once and results stream in.
- **Speed:** global limits, plus alternative limits (the turtle in the sidebar) that
  can switch on a schedule. There's also a live graph.
- **Seeding limits:** stop at a share ratio or after a seeding time, then pause,
  remove (with or without the files), or switch to super seeding.
- **When a download finishes:** get a notification, run a command (`%N`, `%F`, `%D`,
  `%L`, `%I`), and move it out of the incomplete folder if you use one.
- **Bind to your VPN:** pick its interface under Connection and torrents stop
  whenever the VPN is down.

## Install

```sh
git clone https://github.com/design-nexus/nexus-torrents
cd nexus-torrents
./install.sh            # add --default to open magnet links and .torrent files with it
```

This installs any missing build dependencies (`rust gtk4 libtorrent-rasterbar boost`),
builds with Cargo, and puts `torrents` in `~/.local/bin` with a launcher and an icon.
Search plugins need `python3`, and adding `.torrent` links needs `curl`.

To remove it, run `./uninstall.sh`. Add `--purge` to also delete its settings and
session. Your downloads are never touched.

## Use

```sh
torrents                          # open (or focus) the window
torrents file.torrent 'magnet:?…' # add to the running window
torrents --section search         # jump to a page
torrents --quit                   # save everything and stop
```

Closing the window quits, unless you turn on **Keep running when closed** under
Appearance. Torrents then keeps going in the background, and `torrents` brings the
window back.

| Keys | |
| --- | --- |
| <kbd>Ctrl</kbd>+<kbd>O</kbd> / <kbd>Ctrl</kbd>+<kbd>U</kbd> | Add a .torrent file / a magnet link |
| <kbd>Ctrl</kbd>+<kbd>F</kbd> | Filter the list (or search settings on a settings page) |
| <kbd>Space</kbd> | Pause or resume the selection |
| <kbd>Delete</kbd> | Remove the selection |
| <kbd>Ctrl</kbd>+<kbd>C</kbd> | Copy the selection's magnet links |
| <kbd>Ctrl</kbd>+<kbd>W</kbd> / <kbd>Ctrl</kbd>+<kbd>Q</kbd> | Close the window / quit |

## Files

| Where | What |
| --- | --- |
| `~/.config/torrents/settings.toml` | Every setting, and this window's look |
| `~/.config/torrents/categories.toml` | Categories (with their folders) and tags |
| `~/.config/torrents/themes/*.toml` | Your own themes |
| `~/.local/share/torrents/resume/` | Fast-resume data, one file per torrent |
| `~/.local/share/torrents/torrents.json` | Each torrent's category and tags |
| `~/.local/share/torrents/session.dat` | DHT state |
| `~/.local/share/torrents/search/engines/` | Search plugins |

## Look

Torrents follows `~/Projects/STYLE.md`. It uses semantic colour tokens only, ships all
15 bundled themes, reads user themes, and follows the Omarchy theme live by default.
It has no title bar, hides its scrollbars, and switches to an icon-only sidebar under
980 px.

## Licence

MIT. libtorrent-rasterbar is BSD-licensed. The search helpers in `search/` are our
own, written to be compatible with qBittorrent's plugin format.
