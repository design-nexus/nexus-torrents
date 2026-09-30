// A thin, value-only wrapper around libtorrent's session for the Rust side.
#pragma once

#include "rust/cxx.h"

#include <libtorrent/session.hpp>
#include <libtorrent/torrent_handle.hpp>
#include <libtorrent/torrent_status.hpp>

#include <map>
#include <memory>
#include <set>
#include <string>

namespace nt {

struct Kv;
struct AddParams;
struct Status;
struct Details;
struct Event;
struct Stats;
struct TorrentInfo;

class Session {
public:
    Session(rust::Vec<Kv> const& settings, rust::Slice<const std::uint8_t> state);

    rust::String apply_settings(rust::Vec<Kv> const& settings);
    rust::String add_torrent(AddParams const& params);
    rust::String fetch_metadata(rust::Str magnet, rust::Str save_path);
    void cancel_fetch(rust::Str id);
    void remove(rust::Str id, bool with_files);
    void pause(rust::Str id);
    void resume(rust::Str id, bool force, bool auto_managed);
    void recheck(rust::Str id);
    void reannounce(rust::Str id);
    void queue_move(rust::Str id, std::uint8_t how);
    void set_file_priorities(rust::Str id, rust::Slice<const std::uint8_t> priorities);
    void set_limits(rust::Str id, std::int32_t dl, std::int32_t ul);
    void set_sequential(rust::Str id, bool on);
    void set_first_last(rust::Str id, bool on);
    void set_super_seeding(rust::Str id, bool on);
    void move_storage(rust::Str id, rust::Str path);
    void rename(rust::Str id, rust::Str name);
    void add_tracker(rust::Str id, rust::Str url, std::int32_t tier);
    void remove_tracker(rust::Str id, rust::Str url);
    void save_resume(rust::Str id);
    std::int32_t save_all_resume(bool all);
    void post_updates();
    rust::Vec<Status> statuses();
    Details details(rust::Str id, bool with_peers);
    rust::Vec<Event> poll();
    Stats stats();
    rust::Vec<std::uint8_t> session_state() const;
    rust::Vec<std::uint8_t> torrent_file(rust::Str id) const;
    void pause_session();

private:
    lt::torrent_handle find(rust::Str id) const;

    std::unique_ptr<lt::session> ses_;
    std::map<std::string, lt::torrent_handle> handles_;
    std::map<std::string, lt::torrent_status> status_;
    // Magnets being fetched for the add dialog: not shown, removed once they have metadata.
    std::set<std::string> fetching_;
    // Counter indices for session stats.
    int idx_dht_nodes_ = -1;
    int idx_recv_payload_ = -1;
    int idx_sent_payload_ = -1;
    std::int64_t last_recv_ = 0, last_sent_ = 0;
    std::int64_t dl_rate_ = 0, ul_rate_ = 0, dht_nodes_ = 0, recv_total_ = 0, sent_total_ = 0;
    lt::clock_type::time_point last_stats_;
};

std::unique_ptr<Session> new_session(rust::Vec<Kv> const& settings, rust::Slice<const std::uint8_t> state);
TorrentInfo parse_torrent(rust::Slice<const std::uint8_t> data);
TorrentInfo parse_magnet(rust::Str uri);
rust::Vec<std::uint8_t> create_torrent(rust::Str path, rust::Vec<rust::String> const& trackers, bool private_torrent);

} // namespace nt
