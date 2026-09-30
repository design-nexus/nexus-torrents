# prettyPrinter for qBittorrent-style search plugins, written for Torrents (MIT).
# Prints one JSON object per result so Torrents can read them safely.

import json
import re
import sys

_UNITS = {"B": 0, "KB": 1, "MB": 2, "GB": 3, "TB": 4, "PB": 5,
          "KIB": 1, "MIB": 2, "GIB": 3, "TIB": 4, "PIB": 5}


def anySizeToBytes(size_string):
    """"1.5 GB" -> bytes (binary multiples, like qBittorrent). -1 if unknown."""
    if isinstance(size_string, (int, float)):
        return int(size_string)
    m = re.match(r"\s*([\d.,]+)\s*([a-zA-Z]*)", str(size_string))
    if not m:
        return -1
    try:
        value = float(m.group(1).replace(",", ""))
    except ValueError:
        return -1
    unit = m.group(2).upper() or "B"
    if unit not in _UNITS:
        return -1
    return int(value * (1024 ** _UNITS[unit]))


def _int(v, default=-1):
    try:
        return int(str(v).replace(",", "").strip())
    except (TypeError, ValueError):
        return default


def prettyPrinter(dictionary):
    out = {
        "link": str(dictionary.get("link", "")),
        "name": " ".join(str(dictionary.get("name", "")).split()),
        "size": anySizeToBytes(dictionary.get("size", -1)),
        "seeds": _int(dictionary.get("seeds", -1)),
        "leech": _int(dictionary.get("leech", -1)),
        "engine_url": str(dictionary.get("engine_url", "")),
        "desc_link": str(dictionary.get("desc_link", "")),
        "pub_date": _int(dictionary.get("pub_date", -1)),
    }
    sys.stdout.write(json.dumps(out) + "\n")
    sys.stdout.flush()
