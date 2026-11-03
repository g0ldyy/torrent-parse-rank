from __future__ import annotations

import argparse
import json

from .api import match_title, serve
from .importers import import_anime_mappings, import_imdb, update_anime, update_imdb
from .index import MetadataIndex


def main() -> None:
    parser = argparse.ArgumentParser(prog="media-match")
    parser.add_argument("--db", default="metadata.sqlite3")
    commands = parser.add_subparsers(dest="command", required=True)
    imdb = commands.add_parser("import-imdb")
    imdb.add_argument("basics")
    imdb.add_argument("akas")
    imdb.add_argument("--episodes")
    commands.add_parser("update-imdb")
    commands.add_parser("update-anime")
    commands.add_parser("update-all")
    anime = commands.add_parser("import-anime")
    anime.add_argument("file")
    match = commands.add_parser("match")
    match.add_argument("title")
    match.add_argument("--year", type=int)
    match.add_argument("--series", action="store_true")
    server = commands.add_parser("serve")
    server.add_argument("--host", default="127.0.0.1")
    server.add_argument("--port", type=int, default=8765)
    args = parser.parse_args()
    index = MetadataIndex(args.db)
    if args.command == "import-imdb":
        count = import_imdb(index, args.basics, args.akas, args.episodes)
        print(f"Imported {count} IMDb title records")
    elif args.command == "update-imdb":
        print(f"Imported {update_imdb(index)} IMDb title records")
    elif args.command == "update-anime":
        print(f"Imported {update_anime(index)} anime ID records")
    elif args.command == "update-all":
        print(f"Imported {update_imdb(index)} IMDb title records")
        print(f"Imported {update_anime(index)} anime ID records")
    elif args.command == "import-anime":
        print(f"Imported {import_anime_mappings(index, args.file)} anime ID records")
    elif args.command == "match":
        print(json.dumps(match_title(index, args.title, args.year, series=args.series), ensure_ascii=False, indent=2))
    else:
        serve(index, args.host, args.port)


if __name__ == "__main__":
    main()
