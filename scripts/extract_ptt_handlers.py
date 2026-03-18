#!/usr/bin/env python3
from __future__ import annotations

import ast
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
HANDLERS_FILE = ROOT / "PTT" / "PTT" / "handlers.py"
OUT_FILE = ROOT / "rust-port" / "crates" / "ptt-core" / "src" / "generated" / "handlers.json"


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


def main() -> None:
    tree = ast.parse(HANDLERS_FILE.read_text(encoding="utf-8"))
    handlers: list[dict[str, Any]] = []

    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        if not isinstance(node.func, ast.Attribute) or node.func.attr != "add_handler":
            continue

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
                # Skip non-literal regex patterns for now
                continue
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

    OUT_FILE.parent.mkdir(parents=True, exist_ok=True)
    OUT_FILE.write_text(
        json.dumps({"handlers": handlers}, indent=2, ensure_ascii=False),
        encoding="utf-8",
    )
    print(f"wrote {OUT_FILE} with {len(handlers)} handlers")


if __name__ == "__main__":
    main()
