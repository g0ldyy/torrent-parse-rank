import argparse
import json
import os
import stat
import tempfile
from collections.abc import Iterable
from pathlib import Path


def _regular_file(filename: str) -> Path:
    path = Path(filename)
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"not a regular file: {path}")
    return path


def _directory(directory: str) -> Path:
    path = Path(directory)
    if path.is_symlink() or not path.is_dir():
        raise ValueError(f"not a directory: {path}")
    return path


def _atomic_write_lines(path: Path, lines: Iterable[str]) -> None:
    mode = stat.S_IMODE(path.stat().st_mode) if path.exists() else None
    descriptor, temporary_name = tempfile.mkstemp(
        dir=path.parent, prefix=f".{path.name}.", suffix=".tmp"
    )
    temporary_path = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as output:
            output.writelines(lines)
            output.flush()
            os.fsync(output.fileno())
        if mode is not None:
            temporary_path.chmod(mode)
        os.replace(temporary_path, path)
    except BaseException:
        temporary_path.unlink(missing_ok=True)
        raise


def main() -> None:
    parser = argparse.ArgumentParser(description="Parse a filename or torrent name")
    subparsers = parser.add_subparsers(dest="command", required=True, help="Commands")

    parse_parser = subparsers.add_parser("parse", help="Parse a filename or torrent name")
    parse_parser.add_argument(
        "filename", type=str, help="The name of the file or torrent to be parsed"
    )
    parse_parser.add_argument(
        "-tl",
        "--translate-languages",
        action="store_true",
        help="Translate language codes (e.g., 'en', 'jp') to full language names (e.g., 'English', 'Japanese')",
    )

    sort_parser = subparsers.add_parser(
        "sort",
        help="Sort a file by count. Requires `keyword,count` format on every line.",
    )
    sort_parser.add_argument("filename", type=str, help="File to sort")

    combine_parser = subparsers.add_parser(
        "combine", help="Combine and sort keywords from txt files"
    )
    combine_parser.add_argument("directory", type=str, help="Directory containing txt files")

    dedupe_parser = subparsers.add_parser(
        "dedupe",
        help="Deduplicate and sort a file. Requires `keyword` format on every line.",
    )
    dedupe_parser.add_argument("filename", type=str, help="File to deduplicate and sort")

    args = parser.parse_args()

    if args.command == "parse":
        from PTT import parse_title

        result = parse_title(args.filename, translate_languages=args.translate_languages)
        print(json.dumps(result, indent=4))
        return

    try:
        if args.command == "sort":
            sort_by_count(args.filename)
        elif args.command == "combine":
            combine_keywords(args.directory)
        else:
            dedupe_and_sort(args.filename)
    except (OSError, ValueError) as error:
        parser.error(str(error))


def combine_keywords(directory: str) -> None:
    """Combine keywords from all txt files in a directory into a sorted unique list."""
    source_directory = _directory(directory)
    output_file = source_directory / "combined-keywords.txt"
    source_files = sorted(
        path
        for path in source_directory.iterdir()
        if path.suffix == ".txt"
        and path.name != output_file.name
        and not path.is_symlink()
        and path.is_file()
    )
    keywords: set[str] = set()
    for source_file in source_files:
        with source_file.open(encoding="utf-8") as source:
            keywords.update(line.strip() for line in source if line.strip())

    _atomic_write_lines(output_file, (f"{keyword}\n" for keyword in sorted(keywords)))
    print(f"Combined and sorted into {output_file.name} using {len(source_files)} files")


def sort_by_count(filename: str) -> None:
    """Sort lines in `name,count` format by descending count."""
    path = _regular_file(filename)
    entries: list[tuple[str, int]] = []
    with path.open(encoding="utf-8") as source:
        for line_number, raw_line in enumerate(source, start=1):
            line = raw_line.strip()
            if not line:
                continue
            fields = line.split(",")
            if len(fields) != 2 or not fields[0].strip():
                raise ValueError(f"invalid keyword,count entry on line {line_number}")
            try:
                count = int(fields[1])
            except ValueError as error:
                raise ValueError(
                    f"invalid integer count on line {line_number}: {fields[1]!r}"
                ) from error
            entries.append((fields[0].strip(), count))

    entries.sort(key=lambda entry: (-entry[1], entry[0]))
    _atomic_write_lines(path, (f"{name},{count}\n" for name, count in entries))
    print(f"Sorted by count {path.name}")


def dedupe_and_sort(filename: str) -> None:
    """Deduplicate lines and write them back sorted."""
    path = _regular_file(filename)
    with path.open(encoding="utf-8") as source:
        unique_keywords = {line.strip() for line in source if line.strip()}

    _atomic_write_lines(path, (f"{keyword}\n" for keyword in sorted(unique_keywords)))
    print(f"Deduplicated and sorted {path.name}")


if __name__ == "__main__":
    main()
