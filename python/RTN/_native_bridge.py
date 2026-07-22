from typing import Any

import orjson
import regex

_EMPTY_OBJECT_JSON = "{}"


def _dumps(payload: Any) -> str:
    return orjson.dumps(payload).decode("utf-8")


def _serialize_pattern_item(item: Any) -> dict[str, Any] | None:
    if item is None:
        return None
    if isinstance(item, regex.Pattern):
        return {
            "pattern": item.pattern,
            "ignore_case": bool(item.flags & regex.IGNORECASE),
        }
    if isinstance(item, str):
        return {"pattern": item, "ignore_case": True}
    raise TypeError(f"Unsupported pattern item type: {type(item)}")


def pattern_list_key(items: list[Any]) -> tuple[tuple[str, bool] | None, ...]:
    key = []
    for item in items:
        serialized = _serialize_pattern_item(item)
        key.append(
            None if serialized is None else (serialized["pattern"], serialized["ignore_case"])
        )
    return tuple(key)


def pattern_key_to_json(items: tuple[tuple[str, bool] | None, ...]) -> str:
    return _dumps(
        [None if item is None else {"pattern": item[0], "ignore_case": item[1]} for item in items]
    )


def settings_to_json(settings: Any) -> str:
    return settings.model_dump_json(
        by_alias=True,
        context={"native_pattern_objects": True},
    )


def data_to_json(data: Any) -> str:
    return data.model_dump_json(by_alias=True)


def rank_model_to_json(rank_model: Any) -> str:
    return rank_model.model_dump_json(by_alias=True)


def data_settings_to_json(data: Any, settings: Any) -> tuple[str, str]:
    return data_to_json(data), settings_to_json(settings)


def data_settings_rank_to_json(data: Any, settings: Any, rank_model: Any) -> tuple[str, str, str]:
    data_json, settings_json = data_settings_to_json(data, settings)
    return data_json, settings_json, rank_model_to_json(rank_model)


def _validate_aliases(aliases: dict[str, list[str]] | None) -> None:
    if aliases is None:
        return
    if type(aliases) is not dict:
        raise TypeError("Aliases must be a dictionary or None.")
    if any(
        type(key) is not str
        or not key
        or type(values) is not list
        or any(type(value) is not str or not value for value in values)
        for key, values in aliases.items()
    ):
        raise TypeError("Aliases must map non-empty strings to lists of non-empty strings.")


def aliases_to_json(aliases: dict[str, list[str]] | None) -> str:
    _validate_aliases(aliases)
    if aliases is None:
        return _EMPTY_OBJECT_JSON
    return _dumps(aliases)
