import inspect
from collections.abc import Callable
from typing import Any

import regex
from torrent_parse_rank_native import (
    ptt_clean_title,
    ptt_languages_translation_table,
    ptt_parse_title,
    ptt_parse_title_context,
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


RUSSIAN_CAST_REGEX = regex.compile(r"\([^)]*[\u0400-\u04ff][^)]*\)$|(?<=\/.*)\(.*\)$")
ALT_TITLES_REGEX = regex.compile(
    rf"[^/|(]*[{NON_ENGLISH_CHARS}][^/|]*[/|]|[/|][^/|(]*[{NON_ENGLISH_CHARS}][^/|]*"
)
NOT_ONLY_NON_ENGLISH_REGEX = regex.compile(
    rf"(?<=[a-zA-Z][^{NON_ENGLISH_CHARS}]+)[{NON_ENGLISH_CHARS}].*[{NON_ENGLISH_CHARS}]|[{NON_ENGLISH_CHARS}].*[{NON_ENGLISH_CHARS}](?=[^{NON_ENGLISH_CHARS}]+[a-zA-Z])"
)
NOT_ALLOWED_SYMBOLS_AT_START_AND_END = regex.compile(
    rf"^[^\w{NON_ENGLISH_CHARS}#[【★]+|[ \-:/\\[|{{(#$&^]+$"
)
REMAINING_NOT_ALLOWED_SYMBOLS_AT_START_AND_END = regex.compile(rf"^[^\w{NON_ENGLISH_CHARS}#]+|]$")
REDUNDANT_SYMBOLS_AT_END = regex.compile(r"[ \-:./\\]+$")
EMPTY_BRACKETS_REGEX = regex.compile(r"\(\s*\)|\[\s*\]|\{\s*\}")
PARANTHESES_WITHOUT_CONTENT = regex.compile(r"\(\W*\)|\[\W*\]|\{\W*\}")
MOVIE_REGEX = regex.compile(r"[[(]movie[)\]]", flags=regex.IGNORECASE)
STAR_REGEX_1 = regex.compile(r"^[[【★].*[\]】★][ .]?(.+)")
STAR_REGEX_2 = regex.compile(r"(.+)[ .]?[[【★].*[\]】★]$")
MP3_REGEX = regex.compile(r"\bmp3$")
SPACING_REGEX = regex.compile(r"\s+")
SPECIAL_CHAR_SPACING = regex.compile(r"[\-\+\_\{\}\[\]]\W{2,}")
SUB_PATTERN = regex.compile(r"_+")

BEFORE_TITLE_MATCH_REGEX = regex.compile(r"^\[([^[\]]+)]")

DEBUG_HANDLER = False

LANGUAGES_TRANSLATION_TABLE = ptt_languages_translation_table()


def extend_options(options: dict[str, Any] | None = None) -> dict[str, Any]:
    """
    Extend handler options with parser defaults.
    """
    if options is None:
        options = {}
    elif type(options) is not dict:
        raise TypeError("handler options must be a dictionary or None")
    else:
        options = dict(options)

    default_options = {
        "skipIfAlreadyFound": True,
        "skipFromTitle": False,
        "skipIfFirst": False,
        "remove": False,
    }
    unknown = set(options) - (set(default_options) | {"value"})
    if unknown:
        raise ValueError(f"unknown handler options: {sorted(unknown)}")
    if any(key in options and type(options[key]) is not bool for key in default_options):
        raise TypeError("handler control options must be booleans")
    for key, value in default_options.items():
        options.setdefault(key, value)
    return options


def create_handler_from_regexp(
    name: str,
    reg_exp: regex.Pattern,
    transformer: Callable,
    options: dict[str, Any] | None,
) -> Callable:
    """
    Build a current custom Python handler from a regex pattern.
    """
    if type(name) is not str or not name:
        raise TypeError("handler name must be a non-empty string")
    if not isinstance(reg_exp, regex.Pattern):
        raise TypeError("handler pattern must be a regex.Pattern")
    if not callable(transformer):
        raise TypeError("handler transformer must be callable")
    options = extend_options(options)
    param_count = len(inspect.signature(transformer).parameters)

    def handler(context: dict[str, Any]) -> dict[str, Any] | None:
        if type(context) is not dict or set(context) != {"title", "result", "matched"}:
            raise TypeError("handler context has an invalid schema")
        title = context["title"]
        result = context["result"]
        matched = context["matched"]
        if type(title) is not str or type(result) is not dict or type(matched) is not dict:
            raise TypeError("handler context has invalid field types")

        if name in result and options.get("skipIfAlreadyFound", False):
            return None

        if DEBUG_HANDLER is True or (isinstance(DEBUG_HANDLER, str) and DEBUG_HANDLER in name):
            print(
                name,
                "Try to match " + title,
                "To " + reg_exp.pattern,
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
        elif isinstance(handler_name, str) and isinstance(handler, regex.Pattern):
            transformer = transformer if callable(transformer) else (lambda x, *_: x)
            options = extend_options(options)
            handler = create_handler_from_regexp(handler_name, handler, transformer, options)
        elif isinstance(handler_name, str) and callable(handler):
            handler.handler_name = handler_name
        else:
            raise ValueError(
                f"Handler for {handler_name} should be either a regex pattern or a function. Got {type(handler)}"
            )

        self.handlers.append(handler)

    def parse(self, title: str, translate_languages: bool = False) -> dict[str, Any]:
        if type(title) is not str:
            raise TypeError("title must be a string")
        if not title:
            raise ValueError("title must be a non-empty string")
        if type(translate_languages) is not bool:
            raise TypeError("translate_languages must be a boolean")
        if not self.handlers:
            return ptt_parse_title(title, translate_languages)

        native_context = ptt_parse_title_context(title)
        if type(native_context) is not dict or set(native_context) != {
            "result",
            "working_title",
            "end_of_title",
            "matched",
        }:
            raise ValueError("native parser context has an invalid schema")
        result = native_context["result"]
        working_title = native_context["working_title"]
        end_of_title = native_context["end_of_title"]
        matched = native_context["matched"]
        if (
            type(result) is not dict
            or type(working_title) is not str
            or type(end_of_title) is not int
            or not 0 <= end_of_title <= len(working_title)
            or type(matched) is not dict
        ):
            raise ValueError("native parser context has invalid field types")

        for handler in self.handlers:
            match_result = handler({"title": working_title, "result": result, "matched": matched})
            if match_result is None:
                continue
            if type(match_result) is not dict or set(match_result) != {
                "raw_match",
                "match_index",
                "remove",
                "skip_from_title",
            }:
                raise TypeError("custom handler result has an invalid schema")

            raw_match = match_result["raw_match"]
            match_index = match_result["match_index"]
            remove = match_result["remove"]
            skip_from_title = match_result["skip_from_title"]
            if (
                type(raw_match) is not str
                or type(match_index) is not int
                or not 0 <= match_index <= len(working_title)
                or type(remove) is not bool
                or type(skip_from_title) is not bool
            ):
                raise TypeError("custom handler result has invalid field types")

            if remove:
                working_title = (
                    working_title[:match_index] + working_title[match_index + len(raw_match) :]
                )
            if not skip_from_title and 1 < match_index < end_of_title:
                end_of_title = match_index
            if remove and skip_from_title and match_index < end_of_title:
                end_of_title = max(0, end_of_title - len(raw_match))

        result.setdefault("episodes", [])
        result.setdefault("seasons", [])
        result.setdefault("languages", [])
        if translate_languages and result["languages"]:
            result["languages"] = ptt_translate_langs(result["languages"])
        result["title"] = ptt_clean_title(working_title[:end_of_title])
        return result
