"""Additional parsing patterns and utilities for RTN."""

from typing import Any

from torrent_parse_rank_native._native import rtn_check_pattern, rtn_normalize_title

from ._native_bridge import pattern_list_to_json

translationTable: dict[str, Any] = {}


def normalize_title(raw_title: str, lower: bool = True) -> str:
    return rtn_normalize_title(raw_title, lower)


def check_pattern(patterns: list, raw_title: str) -> bool:
    return rtn_check_pattern(pattern_list_to_json(patterns), raw_title)
