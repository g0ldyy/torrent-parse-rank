"""SQLite metadata index and deterministic IMDb V4 style title matching."""

from __future__ import annotations

import re
import sqlite3
import unicodedata
from dataclasses import dataclass
from pathlib import Path

_SERIES = {"tvSeries", "tvMiniSeries"}
_MOVIES = {"movie", "tvMovie", "short", "tvShort", "tvSpecial"}


def clean_title(value: str) -> str:
    value = unicodedata.normalize("NFKD", value).encode("ascii", "ignore").decode()
    return re.sub(r"[^a-z0-9]+", "", value.lower())


@dataclass(frozen=True)
class Match:
    imdb_id: str
    title: str
    year: int
    category: str
    matched_title: str
    anime_ids: dict[str, str]


class MetadataIndex:
    """Persistent SQLite index. Pass a writable path; parent directories are created."""

    def __init__(self, path: str | Path = "metadata.sqlite3"):
        self.path = str(path)
        Path(self.path).parent.mkdir(parents=True, exist_ok=True)
        with self.connect() as db:
            db.executescript("""
                CREATE TABLE IF NOT EXISTS titles (
                    imdb_id TEXT NOT NULL, title TEXT NOT NULL, clean_title TEXT NOT NULL,
                    year INTEGER NOT NULL, category TEXT NOT NULL, region TEXT,
                    original INTEGER NOT NULL DEFAULT 0, ordering INTEGER NOT NULL DEFAULT 0,
                    type TEXT NOT NULL DEFAULT '',
                    PRIMARY KEY (imdb_id, clean_title, year, region, ordering)
                );
                CREATE INDEX IF NOT EXISTS title_lookup ON titles(clean_title, year);
                CREATE INDEX IF NOT EXISTS title_suffix ON titles(clean_title);
                CREATE TABLE IF NOT EXISTS seasons (
                    imdb_id TEXT PRIMARY KEY, max_season INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS anime_ids (
                    group_id TEXT NOT NULL, type TEXT NOT NULL, value TEXT NOT NULL,
                    PRIMARY KEY (group_id, type, value)
                );
                CREATE TABLE IF NOT EXISTS episode_map (
                    anidb_id TEXT NOT NULL, tvdb_id TEXT NOT NULL, anidb_season INTEGER NOT NULL,
                    tvdb_season INTEGER NOT NULL, start INTEGER NOT NULL, end INTEGER NOT NULL,
                    offset INTEGER NOT NULL DEFAULT 0, map TEXT NOT NULL DEFAULT ''
                );
                CREATE TABLE IF NOT EXISTS metadata_state (
                    source TEXT PRIMARY KEY, updated_at TEXT NOT NULL, row_count INTEGER NOT NULL
                );
            """)

    def connect(self) -> sqlite3.Connection:
        db = sqlite3.connect(self.path)
        db.row_factory = sqlite3.Row
        return db

    def search(self, title: str, year: int | None = None, *, series: bool = False,
               required_max_season: int = 0, limit: int = 10) -> list[Match]:
        """Match cleaned IMDb titles with V4 duplicate, year and US/original tie breaks."""
        key = clean_title(title)
        if not key:
            return []
        allowed = _SERIES if series else _MOVIES
        with self.connect() as db:
            rows = db.execute("SELECT * FROM titles WHERE clean_title=? AND year>0", (key,)).fetchall()
            if not rows and len(key) >= 12:
                rows = db.execute(
                    "SELECT * FROM titles WHERE length(clean_title)>length(?) "
                    "AND clean_title LIKE '%' || ?", (key, key),
                ).fetchall()
                if rows:
                    longest = max(len(row["clean_title"]) for row in rows)
                    rows = [row for row in rows if len(row["clean_title"]) == longest]
            grouped: dict[str, list[sqlite3.Row]] = {}
            for row in rows:
                if row["type"] in allowed:
                    grouped.setdefault(row["imdb_id"], []).append(row)
            duplicate = len({r["imdb_id"] for r in db.execute(
                "SELECT DISTINCT imdb_id FROM titles WHERE clean_title=? AND year>0", (key,)
            )}) > 1
            matches = []
            for imdb_id, aliases in grouped.items():
                max_season = db.execute("SELECT max_season FROM seasons WHERE imdb_id=?", (imdb_id,)).fetchone()
                best = min(aliases, key=lambda r: (
                    abs(r["year"] - year) if year else 0,
                    0 if not duplicate or r["original"] else 1,
                    r["year"],
                    0 if not duplicate or r["region"] == "US" else 1,
                    r["ordering"] or 0,
                ))
                if series and required_max_season > 0 and (
                    max_season is None or max_season[0] < max(1, required_max_season - 1)
                ):
                    continue
                types = {r["type"]: r["value"] for r in db.execute(
                    "SELECT type,value FROM anime_ids WHERE group_id=?", (imdb_id,)
                )}
                matches.append(Match(imdb_id, best["title"], best["year"], best["category"],
                                     best["title"], types))
            matches.sort(key=lambda m: (
                abs(m.year - year) if year else 0,
                min((0 if not duplicate or r["original"] else 1) for r in grouped[m.imdb_id]),
                m.year,
                min((0 if not duplicate or r["region"] == "US" else 1) for r in grouped[m.imdb_id]),
                -len(grouped[m.imdb_id]), m.imdb_id,
            ))
            return matches[:limit]

    def anime_lookup(self, source: str, value: str) -> dict[str, str] | None:
        with self.connect() as db:
            row = db.execute("SELECT group_id FROM anime_ids WHERE type=? AND value=? LIMIT 1", (source, value)).fetchone()
            if row is None:
                return None
            result = {r["type"]: r["value"] for r in db.execute(
                "SELECT type,value FROM anime_ids WHERE group_id=?", (row[0],))}
            result["imdb"] = row[0] if row[0].startswith("tt") else ""
            return result

    def map_anime_episode(self, anidb_id: str, season: int, episode: int) -> list[tuple[str, int, int]]:
        """Translate an AniDB season/episode into mapped TVDB season/episode coordinates."""
        with self.connect() as db:
            rows = db.execute("SELECT * FROM episode_map WHERE anidb_id=? AND anidb_season=? AND start<=? AND end>=?",
                              (anidb_id, season, episode, episode)).fetchall()
        return [(r["tvdb_id"], r["tvdb_season"], episode + r["offset"]) for r in rows]
