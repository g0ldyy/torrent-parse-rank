"""Functions to determine if a torrent should be fetched based on user settings."""

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

from ._native_bridge import data_to_json, settings_to_json
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


def _merge_failed_keys(failed_keys: set, keys: list[str]) -> None:
    for key in keys:
        failed_keys.add(key)


def check_fetch(
    data: ParsedData, settings: SettingsModel, speed_mode: bool = True
) -> tuple[bool, list]:
    if not isinstance(data, ParsedData):
        raise TypeError("Parsed data must be an instance of ParsedData.")
    if not isinstance(settings, SettingsModel):
        raise TypeError("Settings must be an instance of SettingsModel.")

    return rtn_check_fetch(data_to_json(data), settings_to_json(settings), speed_mode)


def populate_langs(settings: SettingsModel) -> None:
    exclude, required, allowed = rtn_populate_langs(settings_to_json(settings))
    settings.languages.exclude = list(exclude)
    settings.languages.required = list(required)
    settings.languages.allowed = list(allowed)


def trash_handler(data: ParsedData, settings: SettingsModel, failed_keys: set) -> bool:
    res, keys = rtn_trash_handler(data_to_json(data), settings_to_json(settings))
    _merge_failed_keys(failed_keys, keys)
    return res


def adult_handler(data: ParsedData, settings: SettingsModel, failed_keys: set) -> bool:
    res, keys = rtn_adult_handler(data_to_json(data), settings_to_json(settings))
    _merge_failed_keys(failed_keys, keys)
    return res


def language_handler(data: ParsedData, settings: SettingsModel, failed_keys: set) -> bool:
    res, keys = rtn_language_handler(data_to_json(data), settings_to_json(settings))
    _merge_failed_keys(failed_keys, keys)
    return res


def check_required(data: ParsedData, settings: SettingsModel) -> bool:
    return rtn_check_required(data_to_json(data), settings_to_json(settings))


def check_exclude(data: ParsedData, settings: SettingsModel, failed_keys: set) -> bool:
    res, keys = rtn_check_exclude(data_to_json(data), settings_to_json(settings))
    _merge_failed_keys(failed_keys, keys)
    return res


def fetch_resolution(data: ParsedData, settings: SettingsModel, failed_keys: set) -> bool:
    res, keys = rtn_fetch_resolution(data_to_json(data), settings_to_json(settings))
    _merge_failed_keys(failed_keys, keys)
    return res


def fetch_audio(data: ParsedData, settings: SettingsModel, failed_keys: set) -> bool:
    res, keys = rtn_fetch_audio(data_to_json(data), settings_to_json(settings))
    _merge_failed_keys(failed_keys, keys)
    return res


def fetch_quality(data: ParsedData, settings: SettingsModel, failed_keys: set) -> bool:
    res, keys = rtn_fetch_quality(data_to_json(data), settings_to_json(settings))
    _merge_failed_keys(failed_keys, keys)
    return res


def fetch_codec(data: ParsedData, settings: SettingsModel, failed_keys: set) -> bool:
    res, keys = rtn_fetch_codec(data_to_json(data), settings_to_json(settings))
    _merge_failed_keys(failed_keys, keys)
    return res


def fetch_hdr(data: ParsedData, settings: SettingsModel, failed_keys: set) -> bool:
    res, keys = rtn_fetch_hdr(data_to_json(data), settings_to_json(settings))
    _merge_failed_keys(failed_keys, keys)
    return res


def fetch_other(data: ParsedData, settings: SettingsModel, failed_keys: set) -> bool:
    res, keys = rtn_fetch_other(data_to_json(data), settings_to_json(settings))
    _merge_failed_keys(failed_keys, keys)
    return res
