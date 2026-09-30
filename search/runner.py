# Runs qBittorrent-style search plugins for Torrents (MIT).
#
#   runner.py info     ENGINES_DIR
#   runner.py search   ENGINES_DIR ENGINE CATEGORY QUERY...
#   runner.py download ENGINES_DIR ENGINE URL

import importlib.util
import json
import os
import re
import sys
import urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)


def load(engines_dir, engine_id):
    sys.path.insert(0, engines_dir)
    path = os.path.join(engines_dir, engine_id + ".py")
    spec = importlib.util.spec_from_file_location(engine_id, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return getattr(module, engine_id)()


def version(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            for line in f.readlines()[:10]:
                m = re.match(r"#\s*VERSION:\s*([\d.]+)", line)
                if m:
                    return m.group(1)
    except OSError:
        pass
    return ""


def info(engines_dir):
    for name in sorted(os.listdir(engines_dir)):
        if not name.endswith(".py") or name.startswith("_"):
            continue
        engine_id = name[:-3]
        try:
            e = load(engines_dir, engine_id)
            out = {
                "id": engine_id,
                "name": getattr(e, "name", engine_id),
                "url": getattr(e, "url", ""),
                "categories": sorted(getattr(e, "supported_categories", {"all": ""}).keys()),
                "version": version(os.path.join(engines_dir, name)),
                "error": "",
            }
        except Exception as ex:  # a broken plugin shouldn't hide the others
            out = {"id": engine_id, "name": engine_id, "url": "", "categories": [], "version": "", "error": str(ex)}
        print(json.dumps(out), flush=True)


def search(engines_dir, engine_id, category, query):
    e = load(engines_dir, engine_id)
    cats = getattr(e, "supported_categories", {"all": ""})
    if category not in cats:
        return
    e.search(urllib.parse.quote(query), category)


def download(engines_dir, engine_id, url):
    e = load(engines_dir, engine_id)
    if hasattr(e, "download_torrent"):
        e.download_torrent(url)
    else:
        import helpers
        print(helpers.download_file(url))


def main(argv):
    if len(argv) < 3:
        print(__doc__ or "usage: runner.py info|search|download ...", file=sys.stderr)
        return 2
    cmd, engines_dir = argv[1], argv[2]
    if cmd == "info":
        info(engines_dir)
    elif cmd == "search" and len(argv) >= 6:
        search(engines_dir, argv[3], argv[4], " ".join(argv[5:]))
    elif cmd == "download" and len(argv) >= 5:
        download(engines_dir, argv[3], argv[4])
    else:
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
