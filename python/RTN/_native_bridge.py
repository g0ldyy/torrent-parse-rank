import json
from typing import Any

import regex


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
    return json.dumps(_serialize_pattern_list(items), ensure_ascii=False)


def settings_to_json(settings: Any) -> str:
    payload = settings.model_dump(mode="json", by_alias=True)
    payload["require"] = _serialize_pattern_list(list(getattr(settings, "require", [])))
    payload["exclude"] = _serialize_pattern_list(list(getattr(settings, "exclude", [])))
    payload["preferred"] = _serialize_pattern_list(list(getattr(settings, "preferred", [])))
    return json.dumps(payload, ensure_ascii=False)


def data_to_json(data: Any) -> str:
    return json.dumps(data.model_dump(mode="json", by_alias=True), ensure_ascii=False)


def rank_model_to_json(rank_model: Any) -> str:
    return json.dumps(rank_model.model_dump(mode="json", by_alias=True), ensure_ascii=False)


def aliases_to_json(aliases: dict | None) -> str:
    return json.dumps(aliases or {}, ensure_ascii=False)
