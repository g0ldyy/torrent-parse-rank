from typing import Any

import orjson
import regex

_PATTERN_FIELDS = ("require", "exclude", "preferred")


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


def _serialize_pattern_list(items: list[Any]) -> list[dict[str, Any] | None]:
    return [_serialize_pattern_item(item) for item in items]


def pattern_list_to_json(items: list[Any]) -> str:
    return _dumps(_serialize_pattern_list(items))


def settings_to_json(settings: Any) -> str:
    payload = settings.model_dump(mode="json", by_alias=True)
    for field in _PATTERN_FIELDS:
        payload[field] = _serialize_pattern_list(list(getattr(settings, field, ())))
    return _dumps(payload)


def data_to_json(data: Any) -> str:
    return _dumps(data.model_dump(mode="json", by_alias=True))


def rank_model_to_json(rank_model: Any) -> str:
    return _dumps(rank_model.model_dump(mode="json", by_alias=True))


def data_settings_to_json(data: Any, settings: Any) -> tuple[str, str]:
    return data_to_json(data), settings_to_json(settings)


def data_settings_rank_to_json(data: Any, settings: Any, rank_model: Any) -> tuple[str, str, str]:
    data_json, settings_json = data_settings_to_json(data, settings)
    return data_json, settings_json, rank_model_to_json(rank_model)


def aliases_to_json(aliases: dict | None) -> str:
    return _dumps(aliases or {})
