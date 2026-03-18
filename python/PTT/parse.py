import inspect
import re
import warnings
from collections.abc import Callable
from typing import Any

from torrent_parse_rank_native import (
    ptt_clean_title,
    ptt_languages_translation_table,
    ptt_parse_title,
    ptt_translate_langs,
)

# Non-English characters range
NON_ENGLISH_CHARS = (
    "\u3040-\u30ff"  # Japanese characters
    "\u3400-\u4dbf"  # Chinese characters
    "\u4e00-\u9fff"  # Chinese characters
    "\uf900-\ufaff"  # CJK Compatibility Ideographs
    "\uff66-\uff9f"  # Halfwidth Katakana Japanese characters
    "\u0400-\u04ff"  # Cyrillic characters (Russian)
    "\u0600-\u06ff"  # Arabic characters
    "\u0750-\u077f"  # Arabic characters
    "\u0c80-\u0cff"  # Kannada characters
    "\u0d00-\u0d7f"  # Malayalam characters
    "\u0e00-\u0e7f"  # Thai characters
)


CURLY_BRACKETS = ["{", "}"]
SQUARE_BRACKETS = ["[", "]"]
PARENTHESES = ["(", ")"]
BRACKETS = [CURLY_BRACKETS, SQUARE_BRACKETS, PARENTHESES]


def _safe_compile(pattern: str, flags: int = 0) -> re.Pattern[str]:
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("error", FutureWarning)
            return re.compile(pattern, flags)
    except (re.error, FutureWarning):
        # Compatibility constant fallback for patterns only supported by the `regex` package.
        return re.compile(r"(?:)")


RUSSIAN_CAST_REGEX = _safe_compile(r"\([^)]*[\u0400-\u04ff][^)]*\)$|(?<=\/.*)\(.*\)$")
ALT_TITLES_REGEX = _safe_compile(
    rf"[^/|(]*[{NON_ENGLISH_CHARS}][^/|]*[/|]|[/|][^/|(]*[{NON_ENGLISH_CHARS}][^/|]*"
)
NOT_ONLY_NON_ENGLISH_REGEX = _safe_compile(
    rf"(?<=[a-zA-Z][^{NON_ENGLISH_CHARS}]+)[{NON_ENGLISH_CHARS}].*[{NON_ENGLISH_CHARS}]|[{NON_ENGLISH_CHARS}].*[{NON_ENGLISH_CHARS}](?=[^{NON_ENGLISH_CHARS}]+[a-zA-Z])"
)
NOT_ALLOWED_SYMBOLS_AT_START_AND_END = _safe_compile(
    rf"^[^\w{NON_ENGLISH_CHARS}#[【★]+|[ \-:/\\[|{{(#$&^]+$"
)
REMAINING_NOT_ALLOWED_SYMBOLS_AT_START_AND_END = _safe_compile(rf"^[^\w{NON_ENGLISH_CHARS}#]+|]$")
REDUNDANT_SYMBOLS_AT_END = _safe_compile(r"[ \-:./\\]+$")
EMPTY_BRACKETS_REGEX = _safe_compile(r"\(\s*\)|\[\s*\]|\{\s*\}")
PARANTHESES_WITHOUT_CONTENT = _safe_compile(r"\(\W*\)|\[\W*\]|\{\W*\}")
MOVIE_REGEX = _safe_compile(r"[[(]movie[)\]]", flags=re.IGNORECASE)
STAR_REGEX_1 = _safe_compile(r"^[[【★].*[\]】★][ .]?(.+)")
STAR_REGEX_2 = _safe_compile(r"(.+)[ .]?[[【★].*[\]】★]$")
MP3_REGEX = _safe_compile(r"\bmp3$")
SPACING_REGEX = _safe_compile(r"\s+")
SPECIAL_CHAR_SPACING = _safe_compile(r"[\-\+\_\{\}\[\]]\W{2,}")
SUB_PATTERN = _safe_compile(r"_+")

BEFORE_TITLE_MATCH_REGEX = _safe_compile(r"^\[([^[\]]+)]")

DEBUG_HANDLER = False

LANGUAGES_TRANSLATION_TABLE = ptt_languages_translation_table()


def extend_options(options: dict[str, Any] | None = None) -> dict[str, Any]:
    """
    Extend handler options with parser defaults.
    """
    options = options or {}
    default_options = {
        "skipIfAlreadyFound": True,
        "skipFromTitle": False,
        "skipIfFirst": False,
        "remove": False,
    }
    for key, value in default_options.items():
        options.setdefault(key, value)
    return options


def create_handler_from_regexp(
    name: str, reg_exp: Any, transformer: Callable, options: dict[str, Any]
) -> Callable:
    """
    Compatibility helper to build a Python handler from a regex-like object.
    """
    param_count = len(inspect.signature(transformer).parameters)

    def handler(context: dict[str, Any]) -> dict[str, Any] | None:
        title = context["title"]
        result = context["result"]
        matched = context["matched"]

        if name in result and options.get("skipIfAlreadyFound", False):
            return None

        if DEBUG_HANDLER is True or (isinstance(DEBUG_HANDLER, str) and DEBUG_HANDLER in name):
            print(
                name,
                "Try to match " + title,
                "To " + getattr(reg_exp, "pattern", str(reg_exp)),
            )

        match = reg_exp.search(title)

        if DEBUG_HANDLER is True or (isinstance(DEBUG_HANDLER, str) and DEBUG_HANDLER in name):
            print("Matched " + str(match))

        if not match:
            return None

        raw_match = match.group(0)
        clean_match = match.group(1) if len(match.groups()) >= 1 else raw_match
        transformed = transformer(
            clean_match or raw_match, *([result.get(name)] if param_count > 1 else [])
        )
        if isinstance(transformed, str):
            transformed = transformed.strip()

        before_title_match = BEFORE_TITLE_MATCH_REGEX.match(title)
        is_before_title = before_title_match is not None and raw_match in before_title_match.group(
            1
        )

        other_matches = {k: v for k, v in matched.items() if k != name}
        is_skip_if_first = (
            options.get("skipIfFirst", False)
            and other_matches
            and all(match.start() < other_matches[k]["match_index"] for k in other_matches)
        )

        if transformed is None or is_skip_if_first:
            return None

        matched[name] = matched.get(name, {"raw_match": raw_match, "match_index": match.start()})
        result[name] = options.get("value", transformed)
        return {
            "raw_match": raw_match,
            "match_index": match.start(),
            "remove": options.get("remove", False),
            "skip_from_title": is_before_title or options.get("skipFromTitle", False),
        }

    handler.__name__ = name
    handler.handler_name = name
    return handler


def clean_title(raw_title: str) -> str:
    """
    Native title cleanup helper (Rust implementation).
    """
    return ptt_clean_title(raw_title)


def translate_langs(langs: list[str]) -> list[str]:
    """
    Translate language codes to display names.
    """
    return ptt_translate_langs(langs)


class Parser:
    """
    API-compatible parser wrapper.

    Core parsing runs in Rust through `ptt_parse_title`.
    """

    def __init__(self):
        self.handlers: list[Callable] = []

    def add_handler(
        self,
        handler_name: str,
        handler: Callable | Any | None = None,
        transformer: Callable | None = None,
        options: dict[str, Any] | None = None,
    ):
        if handler is None and callable(handler_name):
            handler = handler_name
            handler.handler_name = getattr(handler_name, "__name__", "unknown")
        elif isinstance(handler_name, str) and hasattr(handler, "search"):
            transformer = transformer if callable(transformer) else (lambda x, *_: x)
            options = extend_options(options if isinstance(options, dict) else {})
            handler = create_handler_from_regexp(handler_name, handler, transformer, options)
        elif isinstance(handler_name, str) and callable(handler):
            handler.handler_name = handler_name
        else:
            raise ValueError(
                f"Handler for {handler_name} should be either a regex pattern or a function. Got {type(handler)}"
            )

        self.handlers.append(handler)

    def parse(self, title: str, translate_languages: bool = False) -> dict[str, Any]:
        return ptt_parse_title(title, translate_languages)
