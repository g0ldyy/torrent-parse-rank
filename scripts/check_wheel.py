#!/usr/bin/env python3
"""Validate that runtime wheels contain the current API but no Rust build sources."""

import argparse
from pathlib import Path
from zipfile import ZipFile

REQUIRED_PATHS = {
    "PTT/__init__.py",
    "RTN/__init__.py",
    "torrent_parse_rank_native/__init__.py",
}
FORBIDDEN_PATHS = {
    "torrent_parse_rank_native/.gitignore",
    "torrent_parse_rank_native/Cargo.toml",
    "torrent_parse_rank_native/src/lib.rs",
}


def validate_wheel(path: Path) -> None:
    if not path.is_file():
        raise ValueError(f"wheel does not exist: {path}")

    with ZipFile(path) as archive:
        names = set(archive.namelist())

    missing = sorted(REQUIRED_PATHS - names)
    forbidden = sorted(FORBIDDEN_PATHS & names)
    native_extensions = [
        name
        for name in names
        if name.startswith("torrent_parse_rank_native/_native.")
        and name.endswith((".so", ".pyd", ".dylib"))
    ]
    if missing or forbidden or len(native_extensions) != 1:
        raise ValueError(
            f"invalid wheel {path}: missing={missing}, forbidden={forbidden}, "
            f"native_extensions={native_extensions}"
        )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("wheels", nargs="+", type=Path)
    args = parser.parse_args()

    for wheel in args.wheels:
        validate_wheel(wheel)
        print(f"verified wheel contents: {wheel}")


if __name__ == "__main__":
    main()
