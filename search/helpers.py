# Helpers for qBittorrent-style search plugins, written for Torrents (MIT).
# Same function names and behaviour plugins expect: retrieve_url, download_file,
# htmlentitydecode and the `headers` dict. Proxies come from the usual
# http_proxy / https_proxy environment variables.

import gzip
import html
import io
import os
import tempfile
import urllib.error
import urllib.parse
import urllib.request

headers = {
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0",
}


def htmlentitydecode(s):
    return html.unescape(s)


def _open(url, custom_headers=None, request_data=None, ssl_context=None):
    hdrs = dict(headers)
    hdrs.update(custom_headers or {})
    data = request_data
    if isinstance(data, str):
        data = data.encode()
    req = urllib.request.Request(url, data=data, headers=hdrs)
    resp = urllib.request.urlopen(req, timeout=30, context=ssl_context)
    body = resp.read()
    if resp.info().get("Content-Encoding") == "gzip" or body[:2] == b"\x1f\x8b":
        try:
            body = gzip.GzipFile(fileobj=io.BytesIO(body)).read()
        except OSError:
            pass
    return resp, body


def retrieve_url(url, custom_headers={}, request_data=None, ssl_context=None, unescape_html_entities=True):
    """Return the page at `url` as text ("" on failure, like qBittorrent's)."""
    try:
        resp, body = _open(url, custom_headers, request_data, ssl_context)
    except (urllib.error.URLError, OSError, ValueError) as e:
        print(f"Connection error: {e}", file=__import__("sys").stderr)
        return ""
    charset = resp.headers.get_content_charset() or "utf-8"
    text = body.decode(charset, "replace")
    return html.unescape(text) if unescape_html_entities else text


def download_file(url, referer=None, ssl_context=None):
    """Save `url` (a .torrent, or a magnet passed straight through) to a temp file.
    Returns "path url", as qBittorrent's helper does."""
    if url.startswith("magnet:"):
        return url + " " + url
    extra = {"Referer": referer} if referer else {}
    _, body = _open(url, extra, None, ssl_context)
    fd, path = tempfile.mkstemp(suffix=".torrent")
    with os.fdopen(fd, "wb") as f:
        f.write(body)
    return path + " " + url


def enable_socks_proxy(enable):
    # Proxies are taken from the environment; nothing to switch here.
    pass
