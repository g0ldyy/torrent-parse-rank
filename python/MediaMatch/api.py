"""Small dependency-free matching API; can be embedded or served over HTTP."""

from __future__ import annotations

import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from RTN import parse

from .index import MetadataIndex


def match_title(index: MetadataIndex, title: str, year: int | None = None,
                *, series: bool | None = None, limit: int = 10) -> list[dict]:
    """Parse with existing RTN parsing, then return metadata candidates and parse details."""
    parsed = parse(title)
    inferred_series = bool(parsed.seasons or parsed.episodes) if series is None else series
    return [{"imdb_id": item.imdb_id, "title": item.title, "year": item.year,
             "category": item.category, "matched_title": item.matched_title,
             "anime_ids": item.anime_ids, "parsed": parsed.model_dump(mode="json")}
            for item in index.search(parsed.parsed_title, year or parsed.year,
                                     series=inferred_series,
                                     required_max_season=max(parsed.seasons, default=0), limit=limit)]


def serve(index: MetadataIndex, host: str = "127.0.0.1", port: int = 8765) -> None:
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            from urllib.parse import parse_qs, urlparse
            request = urlparse(self.path)
            if request.path != "/match":
                self.send_error(404)
                return
            params = parse_qs(request.query)
            title = params.get("title", [""])[0]
            if not title:
                self.send_error(400, "title is required")
                return
            try:
                year = int(params["year"][0]) if params.get("year") else None
                payload = match_title(index, title, year)
            except (TypeError, ValueError) as exc:
                self.send_error(400, str(exc))
                return
            body = json.dumps(payload, ensure_ascii=False).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_args):
            return

    ThreadingHTTPServer((host, port), Handler).serve_forever()
