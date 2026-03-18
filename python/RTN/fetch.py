"""Functions to determine if a torrent should be fetched based on user settings."""

from collections.abc import Callable

from torrent_parse_rank_native._native import (
    rtn_adult_handler,
    rtn_check_exclude,
    rtn_check_fetch,
    rtn_check_required,
    rtn_fetch_audio,
    rtn_fetch_codec,
    rtn_fetch_hdr,
    rtn_fetch_other,
    rtn_fetch_quality,
    rtn_fetch_resolution,
    rtn_language_handler,
    rtn_populate_langs,
    rtn_trash_handler,
)

from ._native_bridge import data_settings_to_json, settings_to_json
from .models import ParsedData, SettingsModel

ANIME = {"ja", "zh", "ko"}
NON_ANIME = {
    "de",
    "es",
    "hi",
    "ta",
    "ru",
    "ua",
    "th",
    "it",
    "ar",
    "pt",
    "fr",
    "pa",
    "mr",
    "gu",
    "te",
    "kn",
    "ml",
    "vi",
    "id",
    "tr",
    "he",
    "fa",
    "el",
    "lt",
    "lv",
    "et",
    "pl",
    "cs",
    "sk",
    "hu",
    "ro",
    "bg",
    "sr",
    "hr",
    "sl",
    "nl",
    "da",
    "fi",
    "sv",
    "no",
    "ms",
}
COMMON = {"de", "es", "hi", "ta", "ru", "ua", "th", "it", "zh", "ar", "fr"}
ALL = ANIME | NON_ANIME


def _native_payload(data: ParsedData, settings: SettingsModel) -> tuple[str, str]:
    return data_settings_to_json(data, settings)


def _run_bool_with_failed_keys(
    native_fn: Callable[[str, str], tuple[bool, list[str]]],
    data: ParsedData,
    settings: SettingsModel,
    failed_keys: set[str],
) -> bool:
    data_json, settings_json = _native_payload(data, settings)
    res, keys = native_fn(data_json, settings_json)
    failed_keys.update(keys)
    return bool(res)


def _make_failed_key_handler(
    name: str, native_fn: Callable[[str, str], tuple[bool, list[str]]]
) -> Callable[[ParsedData, SettingsModel, set[str]], bool]:
    def handler(data: ParsedData, settings: SettingsModel, failed_keys: set[str]) -> bool:
        return _run_bool_with_failed_keys(native_fn, data, settings, failed_keys)

    handler.__name__ = name
    return handler


def check_fetch(
    data: ParsedData, settings: SettingsModel, speed_mode: bool = True
) -> tuple[bool, list[str]]:
    if not isinstance(data, ParsedData):
        raise TypeError("Parsed data must be an instance of ParsedData.")
    if not isinstance(settings, SettingsModel):
        raise TypeError("Settings must be an instance of SettingsModel.")

    data_json, settings_json = _native_payload(data, settings)
    return rtn_check_fetch(data_json, settings_json, speed_mode)


def populate_langs(settings: SettingsModel) -> None:
    exclude, required, allowed = rtn_populate_langs(settings_to_json(settings))
    settings.languages.exclude = list(exclude)
    settings.languages.required = list(required)
    settings.languages.allowed = list(allowed)


trash_handler = _make_failed_key_handler("trash_handler", rtn_trash_handler)
adult_handler = _make_failed_key_handler("adult_handler", rtn_adult_handler)
language_handler = _make_failed_key_handler("language_handler", rtn_language_handler)


def check_required(data: ParsedData, settings: SettingsModel) -> bool:
    data_json, settings_json = _native_payload(data, settings)
    return rtn_check_required(data_json, settings_json)


check_exclude = _make_failed_key_handler("check_exclude", rtn_check_exclude)
fetch_resolution = _make_failed_key_handler("fetch_resolution", rtn_fetch_resolution)
fetch_audio = _make_failed_key_handler("fetch_audio", rtn_fetch_audio)
fetch_quality = _make_failed_key_handler("fetch_quality", rtn_fetch_quality)
fetch_codec = _make_failed_key_handler("fetch_codec", rtn_fetch_codec)
fetch_hdr = _make_failed_key_handler("fetch_hdr", rtn_fetch_hdr)
fetch_other = _make_failed_key_handler("fetch_other", rtn_fetch_other)
