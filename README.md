<h1 align="center" id="title">torrent-parse-rank</h1>

<p align="center">
  A high-performance Rust implementation of The Best Damn Filename Parser You've Ever Used!
</p>

<p align="center">
  <img src="https://img.shields.io/badge/python-3.10%2B-3776AB?style=flat-square&logo=python&logoColor=white" />
  <img src="https://img.shields.io/badge/rust-core-000000?style=flat-square&logo=rust&logoColor=white" />
  <img src="https://img.shields.io/badge/license-GPLv3-blue?style=flat-square" />
</p>

## Credits

- Upstream PTT reference: https://github.com/dreulavelle/PTT
- Upstream RTN reference: https://github.com/dreulavelle/rank-torrent-name

## What You Get

- Fast `PTT` parsing API for raw torrent names.
- `RTN` ranking/filtering API with configurable quality rules.
- Python-first usage (`from PTT import parse_title`, `from RTN import RTN`).
- Rust performance without changing your Python integration style.

## Install In Your Python Project

### Option 1: Local path

```bash
uv add --editable /absolute/path/to/torrent-parse-rank
```

### Option 2: Git dependency

```bash
uv add "torrent-parse-rank@git+https://github.com/g0ldyy/torrent-parse-rank"
```

Notes:

- Installing from source requires a Rust toolchain (`cargo`) available in `PATH`.
- The import names are `PTT` and `RTN`.

## Quickstart

### 1) Parse a torrent title (PTT)

```python
from PTT import parse_title

data = parse_title("The.Simpsons.S01E01.1080p.BluRay.x265", False)
print(data["title"])      # The Simpsons
print(data["resolution"]) # 1080p
```

### 2) Parse + rank a candidate (RTN)

```python
from RTN import RTN
from RTN.models import DefaultRanking, SettingsModel

rtn = RTN(SettingsModel(), DefaultRanking())
item = rtn.rank(
    raw_title="The Walking Dead S05E03 720p x264-ASAP",
    infohash="c08a9ee8ce3a5c2c08865e2b05406273cabc97e7",
)

print(item.fetch)  # True/False after filters
print(item.rank)   # computed rank score
print(item.data.parsed_title)
```

### 3) Parse only (RTN)

```python
from RTN import parse

parsed = parse("Oppenheimer.2023.2160p.REMUX.DV.HDR10Plus.TrueHD.7.1.HEVC")
print(parsed.resolution)
print(parsed.codec)
```

## Development

```bash
cd torrent-parse-rank
uv sync --group dev
uv run maturin develop --release
```

## Quality / Tests

```bash
cd torrent-parse-rank
./scripts/quality_gate.sh
./scripts/quality_gate.sh --full
```

`--full` includes upstream parity tests.
Both variants rebuild the optimized native extension before running Python tests.

## Benchmarks

Python API speedup vs upstream (`PTT` + `RTN`): **~19.88x** geometric-mean throughput.

| Parser (N=30,000) | Upstream Python (items/s) | Rust port (items/s) | Speedup | Upstream p50 (ms) | Rust p50 (ms) |
|---|---:|---:|---:|---:|---:|
| `PTT.parse_title` | 1,815.6 | 40,355.6 | 22.23x | 0.556 | 0.024 |
| `RTN.parse` | 1,708.8 | 30,284.9 | 17.72x | 0.590 | 0.032 |

Full benchmark report: [`benchmarks/README.md`](benchmarks/README.md)

## IMDb and anime matching

`MediaMatch` adds an optional local SQLite metadata index. It uses RTN's existing
parser to normalize torrent titles; matching metadata does not alter RTN ranking.
IMDb matching supports IMDb's official `title.basics`, `title.akas`, and optional
`title.episode` TSV exports, with year, duplicate-title original/US preferences,
category filtering, and season-aware series matching. AniDB cross-ID data can be
loaded from the provided TSV schema or the manami-project anime-offline-database
JSON export; AniDB/TVDB episode crosswalk columns are also supported in TSV.

```python
from MediaMatch import MetadataIndex
from MediaMatch.api import match_title

index = MetadataIndex("metadata.sqlite3")
matches = match_title(index, "Dune 2021 2160p", year=2021)
```

```sh
media-match --db metadata.sqlite3 update-imdb
media-match --db metadata.sqlite3 update-anime
media-match --db metadata.sqlite3 match "Cowboy Bebop 1998"
media-match --db metadata.sqlite3 serve --host 127.0.0.1 --port 8765
```

The HTTP endpoint is `GET /match?title=...&year=...`. A scheduled GitHub workflow
refreshes both source datasets and publishes the generated SQLite index as a
30-day workflow artifact. For a persistent local service, schedule `update-all`
with your operating system's task scheduler.
