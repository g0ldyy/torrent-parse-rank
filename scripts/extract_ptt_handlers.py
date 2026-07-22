#!/usr/bin/env python3
from __future__ import annotations

import argparse
import ast
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_HANDLERS_FILE = ROOT / ".upstream-tests-cache" / "PTT" / "PTT" / "handlers.py"
DEFAULT_OUT_FILE = ROOT / "crates" / "ptt-core" / "src" / "generated" / "handlers.json"


def flags_from_compile_call(call: ast.Call) -> int:
    # regex.IGNORECASE == 2
    flags = 0
    for arg in call.args[1:]:
        text = ast.unparse(arg)
        if "IGNORECASE" in text:
            flags |= 2
    for kw in call.keywords:
        if kw.arg == "flags" and "IGNORECASE" in ast.unparse(kw.value):
            flags |= 2
    return flags


def parse_options(node: ast.AST | None) -> dict[str, Any]:
    defaults = {
        "skipIfAlreadyFound": True,
        "skipFromTitle": False,
        "skipIfFirst": False,
        "remove": False,
    }
    if node is None:
        return defaults
    if not isinstance(node, ast.Dict):
        return defaults
    for k, v in zip(node.keys, node.values, strict=False):
        if not isinstance(k, ast.Constant) or not isinstance(k.value, str):
            continue
        key = k.value
        if isinstance(v, ast.Constant):
            defaults[key] = v.value
    return defaults


def transform_spec(node: ast.AST | None) -> str:
    if node is None:
        return "none"
    return ast.unparse(node)


def extract_handlers(handlers_file: Path) -> dict[str, list[dict[str, Any]]]:
    tree = ast.parse(handlers_file.read_text(encoding="utf-8"))
    handlers: list[dict[str, Any]] = []
    add_handler_calls = 0

    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        if not isinstance(node.func, ast.Attribute) or node.func.attr != "add_handler":
            continue
        add_handler_calls += 1

        args = node.args
        if not args:
            continue

        name_node = args[0]
        if not isinstance(name_node, ast.Constant) or not isinstance(name_node.value, str):
            continue
        hname = name_node.value

        handler_node = args[1] if len(args) >= 2 else None
        transformer_node = args[2] if len(args) >= 3 else None
        options_node = args[3] if len(args) >= 4 else None

        for kw in node.keywords:
            if kw.arg == "options":
                options_node = kw.value

        options = parse_options(options_node)

        entry: dict[str, Any] = {
            "name": hname,
            "options": options,
        }

        # Regex handler call pattern
        if (
            isinstance(handler_node, ast.Call)
            and isinstance(handler_node.func, ast.Attribute)
            and handler_node.func.attr == "compile"
        ):
            pattern_node = handler_node.args[0] if handler_node.args else None
            if not isinstance(pattern_node, ast.Constant) or not isinstance(
                pattern_node.value, str
            ):
                raise ValueError(
                    f"Handler {hname!r} uses a non-literal regex pattern; "
                    "the generator cannot preserve it safely."
                )
            entry["kind"] = "regex"
            entry["pattern"] = pattern_node.value
            entry["flags"] = flags_from_compile_call(handler_node)
            entry["transform"] = transform_spec(transformer_node)
        else:
            # Function handler
            entry["kind"] = "function"
            entry["function"] = ast.unparse(handler_node) if handler_node is not None else ""
            entry["transform"] = transform_spec(transformer_node)

        handlers.append(entry)

    if len(handlers) != add_handler_calls:
        raise ValueError(f"Extracted {len(handlers)} of {add_handler_calls} add_handler calls.")
    return {"handlers": handlers}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate Rust PTT handler data")
    parser.add_argument(
        "--source",
        type=Path,
        default=DEFAULT_HANDLERS_FILE,
        help="Path to upstream PTT/PTT/handlers.py",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUT_FILE,
        help="Path to generated handlers.json",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Fail when the existing output is not semantically current",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    if not args.source.is_file():
        raise FileNotFoundError(
            f"PTT handler source not found: {args.source}. "
            "Run scripts/fetch_upstream_tests.sh or pass --source."
        )

    payload = extract_handlers(args.source)
    if args.check:
        if not args.output.is_file():
            raise FileNotFoundError(f"Generated handler file not found: {args.output}")
        existing = json.loads(args.output.read_text(encoding="utf-8"))
        if existing != payload:
            raise SystemExit(
                f"{args.output} is stale for {args.source}; regenerate without --check."
            )
        print(f"verified {args.output} with {len(payload['handlers'])} handlers")
        return

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(payload, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    print(f"wrote {args.output} with {len(payload['handlers'])} handlers")


if __name__ == "__main__":
    main()
