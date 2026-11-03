"""Streaming importers for IMDb's public TSV exports and anime mapping files."""

from __future__ import annotations

import csv
import gzip
import io
import json
import urllib.request
from datetime import UTC, datetime
from pathlib import Path

from .index import MetadataIndex, clean_title

IMDB_BASE = "https://datasets.imdbws.com/"
_TYPES = {"movie", "tvMovie", "short", "tvShort", "tvSpecial", "tvSeries", "tvMiniSeries"}


def _open_tsv(source: str | Path):
    if str(source).startswith(("https://", "http://")):
        response = urllib.request.urlopen(str(source), timeout=120)
        return io.TextIOWrapper(gzip.GzipFile(fileobj=response), encoding="utf-8", newline="")
    path = Path(source)
    binary = gzip.open(path, "rb") if path.suffix == ".gz" else path.open("rb")
    return io.TextIOWrapper(binary, encoding="utf-8", newline="")


def import_imdb(index: MetadataIndex, basics: str | Path, akas: str | Path,
                episodes: str | Path | None = None) -> int:
    """Import title.basics, title.akas and optional title.episode in one transaction."""
    db = index.connect()
    count = 0
    try:
        db.execute("BEGIN IMMEDIATE")
        db.execute("DELETE FROM titles")
        db.execute("DELETE FROM seasons")
        db.execute("CREATE TEMP TABLE imdb_akas (tid TEXT, ordering INTEGER, title TEXT, clean_title TEXT, region TEXT, original INTEGER)")
        for row in csv.DictReader(_open_tsv(basics), delimiter="\t"):
            if row["titleType"] not in _TYPES or row["startYear"] == "\\N":
                continue
            imdb_id, title_type, title, year = row["tconst"], row["titleType"], row["primaryTitle"], int(row["startYear"])
            category = "series" if title_type in {"tvSeries", "tvMiniSeries"} else "movie"
            db.execute("INSERT OR IGNORE INTO titles VALUES(?,?,?,?,?,?,?,?,?)",
                       (imdb_id, title, clean_title(title), year, category, None, 0, 0, title_type))
            original_title = row["originalTitle"]
            if original_title != "\\N" and clean_title(original_title) != clean_title(title):
                db.execute("INSERT OR IGNORE INTO titles VALUES(?,?,?,?,?,?,?,?,?)",
                           (imdb_id, original_title, clean_title(original_title), year,
                            category, None, 1, 0, title_type))
            count += 1
        db.executemany("INSERT INTO imdb_akas VALUES(?,?,?,?,?,?)", (
            (row["titleId"], int(row["ordering"]), row["title"], clean_title(row["title"]),
             None if row["region"] == "\\N" else row["region"],
             int(row["isOriginalTitle"] == "1"))
            for row in csv.DictReader(_open_tsv(akas), delimiter="\t")
        ))
        alias_rows = db.execute("""INSERT OR IGNORE INTO titles
            SELECT b.imdb_id,a.title,a.clean_title,b.year,b.category,a.region,a.original,a.ordering,b.type
            FROM imdb_akas a JOIN (SELECT imdb_id,year,category,type FROM titles GROUP BY imdb_id) b
            ON b.imdb_id=a.tid""").rowcount
        count += max(alias_rows, 0)
        if episodes:
            seasons: dict[str, int] = {}
            for row in csv.DictReader(_open_tsv(episodes), delimiter="\t"):
                if row["seasonNumber"] != "\\N":
                    seasons[row["parentTconst"]] = max(seasons.get(row["parentTconst"], 0), int(row["seasonNumber"]))
            db.executemany("INSERT OR REPLACE INTO seasons VALUES(?,?)", seasons.items())
        db.execute("INSERT OR REPLACE INTO metadata_state VALUES(?,?,?)",
                   ("imdb", datetime.now(UTC).isoformat(), count))
        db.commit()
    except BaseException:
        db.rollback()
        raise
    finally:
        db.close()
    return count


def update_imdb(index: MetadataIndex) -> int:
    return import_imdb(index, *(IMDB_BASE + f"{name}.tsv.gz" for name in
                                ("title.basics", "title.akas", "title.episode")))


def import_anime_mappings(index: MetadataIndex, path: str | Path) -> int:
    """Import schema-shaped TSV or manami-project anime-offline-database JSON."""
    path = Path(path)
    if path.suffix.lower() == ".json":
        payload = json.loads(path.read_text(encoding="utf-8"))
        rows = []
        source_names = {"anidb": "ANIDB", "anilist": "ANILIST", "animeplanet": "ANIME-PLANET",
                        "anisearch": "ANISEARCH", "imdb": "IMDB", "kitsu": "KITSU",
                        "mal": "MAL", "livechart": "LIVECHART", "tmdb": "TMDB",
                        "tvdb": "TVDB", "trakt": "TRAKT"}
        for entry in payload.get("data", []):
            row = {k: "" for k in source_names}
            for item in entry.get("sources", []):
                for key, source in source_names.items():
                    if item.get("source", "").upper() == source:
                        row[key] = str(item.get("id", ""))
            rows.append(row)
    else:
        rows = csv.DictReader(path.open(encoding="utf-8-sig", newline=""), delimiter="\t")
    mappings = []
    episode_maps = []
    known = {"imdb", "anidb", "anilist", "animeplanet", "anisearch", "kitsu", "mal",
             "livechart", "notifymoe", "tmdb", "tvdb", "lboxd", "trakt"}
    db = index.connect()
    try:
        db.execute("BEGIN IMMEDIATE")
        db.execute("DELETE FROM anime_ids")
        db.execute("DELETE FROM episode_map")
        for row in rows:
            imdb_id = row.get("imdb", "")
            key = imdb_id or row.get("anidb", "")
            if not key:
                continue
            for source in known:
                value = (row.get(source) or "").strip()
                if value and value != "\\N":
                    mappings.append((key, source, value))
            if row.get("anidb_season") and row.get("tvdb_season"):
                episode_maps.append((row["anidb"], row["tvdb"], int(row["anidb_season"]),
                    int(row["tvdb_season"]), int(row.get("start", 1)), int(row.get("end", 9999)),
                    int(row.get("offset", 0)), row.get("map", "")))
        db.executemany("INSERT OR IGNORE INTO anime_ids VALUES(?,?,?)", mappings)
        db.executemany("INSERT INTO episode_map VALUES(?,?,?,?,?,?,?,?)", episode_maps)
        db.execute("INSERT OR REPLACE INTO metadata_state VALUES(?,?,?)",
                   ("anime", datetime.now(UTC).isoformat(), len(mappings)))
        db.commit()
    except BaseException:
        db.rollback()
        raise
    finally:
        db.close()
    return len(mappings)


def update_anime(index: MetadataIndex) -> int:
    """Refresh ID crosswalks from the maintained anime-offline-database export."""
    url = "https://raw.githubusercontent.com/manami-project/anime-offline-database/master/anime-offline-database.json"
    with urllib.request.urlopen(url, timeout=120) as response:
        payload = response.read()
    import tempfile
    with tempfile.NamedTemporaryFile(suffix=".json") as temporary:
        temporary.write(payload)
        temporary.flush()
        return import_anime_mappings(index, temporary.name)
