import re
from collections.abc import Callable
from datetime import datetime

MAX_EXPANDED_RANGE_ITEMS = 10_000


def _require_string(value: str) -> str:
    if type(value) is not str:
        raise TypeError("transformer input must be a string")
    return value


def _require_scalar(value: str | int, field_name: str) -> str | int:
    if type(value) not in (str, int):
        raise TypeError(f"{field_name} must be a string or integer")
    return value


def none(input_value: str) -> str:
    return _require_string(input_value)


def value(
    val: str | int | Callable[[str], str | int],
) -> Callable[[str], str | int]:
    if type(val) not in (str, int) and not callable(val):
        raise TypeError("transformer value must be a string, integer, or callable")

    def inner(input_value: str, existing_value: str | int | None = None) -> str | int:
        input_value = _require_string(input_value)
        if existing_value is not None:
            _require_scalar(existing_value, "existing value")
        if isinstance(val, str):
            return val.replace("$1", input_value)
        if callable(val):
            return _require_scalar(val(input_value), "callable result")
        return _require_scalar(val, "transformer value")

    return inner


def integer(input_value: str) -> int | None:
    input_value = _require_string(input_value)
    digits = re.sub(r"\D", "", input_value)
    return int(digits) if digits else None


def first_integer(input_value: str) -> int | None:
    input_value = _require_string(input_value)
    found = re.findall(r"\d+", input_value)
    return int(found[0]) if found else None


def boolean(input_value: str, existing_value=None) -> bool:
    _require_string(input_value)
    return True


def lowercase(input_value: str) -> str:
    return _require_string(input_value).lower()


def uppercase(input_value: str) -> str:
    return _require_string(input_value).upper()


month_mapping = {
    r"\bJanu\b": "Jan",
    r"\bFebr\b": "Feb",
    r"\bMarc\b": "Mar",
    r"\bApri\b": "Apr",
    r"\bMay\b": "May",
    r"\bJune\b": "Jun",
    r"\bJuly\b": "Jul",
    r"\bAugu\b": "Aug",
    r"\bSept\b": "Sep",
    r"\bOcto\b": "Oct",
    r"\bNove\b": "Nov",
    r"\bDece\b": "Dec",
}
MONTH_PATTERNS = [
    (re.compile(pattern, flags=re.IGNORECASE), replacement)
    for pattern, replacement in month_mapping.items()
]


def convert_months(date_str: str) -> str:
    date_str = _require_string(date_str)
    for month_re, shortened in MONTH_PATTERNS:
        date_str = month_re.sub(shortened, date_str)
    return date_str


def _normalize_custom_date_format(fmt: str) -> str:
    converted = fmt
    converted = converted.replace("Do", "%d")
    converted = converted.replace("YYYY", "%Y")
    converted = converted.replace("YY", "%y")
    converted = converted.replace("MMMM", "%B")
    converted = converted.replace("MMM", "%b")
    converted = converted.replace("MM", "%m")
    converted = converted.replace("DD", "%d")
    return converted


def _normalize_day_ordinals(text: str) -> str:
    return re.sub(r"\b(\d{1,2})(st|nd|rd|th)\b", r"\1", text, flags=re.IGNORECASE)


def date(date_format: str | list[str]) -> Callable[[str], str | None]:
    if type(date_format) is str:
        if not date_format:
            raise ValueError("date format must be non-empty")
        formats = (date_format,)
    elif type(date_format) is list:
        if not date_format or any(type(fmt) is not str or not fmt for fmt in date_format):
            raise ValueError("date formats must be a non-empty list of non-empty strings")
        formats = tuple(date_format)
    else:
        raise TypeError("date format must be a string or list of strings")

    def inner(input_value: str) -> str | None:
        input_value = _require_string(input_value)
        sanitized = re.sub(r"\W+", " ", input_value).strip()
        sanitized = _normalize_day_ordinals(convert_months(sanitized))
        for fmt in formats:
            py_fmt = _normalize_custom_date_format(fmt)
            try:
                return datetime.strptime(sanitized, py_fmt).strftime("%Y-%m-%d")
            except ValueError:
                continue
        return None

    return inner


def range_func(input_str: str) -> list[int] | None:
    input_str = _require_string(input_str)
    numbers = [int(x) for x in re.findall(r"\d+", input_str)]

    if (
        len(numbers) == 2
        and numbers[0] < numbers[1]
        and numbers[1] - numbers[0] + 1 <= MAX_EXPANDED_RANGE_ITEMS
    ):
        return list(range(numbers[0], numbers[1] + 1))
    if len(numbers) > 2 and all(numbers[i] + 1 == numbers[i + 1] for i in range(len(numbers) - 1)):
        return numbers
    if len(numbers) == 1:
        return numbers

    return None


def range_x_of_y_func(input_str: str) -> list[int] | None:
    input_str = _require_string(input_str)
    numbers = [int(x) for x in re.findall(r"\d+", input_str)]
    if len(numbers) != 1 or not 1 <= numbers[0] <= MAX_EXPANDED_RANGE_ITEMS:
        return None
    return list(range(1, numbers[0] + 1))


def year_range(input_value: str) -> str | None:
    input_value = _require_string(input_value)
    parts = re.findall(r"\d+", input_value)
    if not parts:
        return None

    try:
        start = int(parts[0])
        end = int(parts[1]) if len(parts) > 1 else None
    except ValueError:
        return None

    if not end:
        return str(start)

    if end < 100:
        end += start - start % 100

    if end <= start:
        return None

    return f"{start}-{end}"


def array(
    chain: Callable[[str], str | int | None] | None = None,
) -> Callable[[str], list[str | int | None]]:
    if chain is not None and not callable(chain):
        raise TypeError("array chain must be callable or None")

    def inner(input_value: str) -> list[str | int | None]:
        input_value = _require_string(input_value)
        output_value = chain(input_value) if chain else input_value
        if output_value is not None:
            _require_scalar(output_value, "array chain result")
        return [output_value]

    return inner


def uniq_concat(
    chain: Callable[[str], str | int],
) -> Callable[[str, list[str | int] | None], list[str | int]]:
    if not callable(chain):
        raise TypeError("unique-concat chain must be callable")

    def inner(input_value: str, result: list[str | int] | None = None) -> list[str | int]:
        input_value = _require_string(input_value)
        if result is None:
            result = []
        elif type(result) is not list or any(type(item) not in (str, int) for item in result):
            raise TypeError("unique-concat result must be a list of strings or integers")
        output_value = _require_scalar(chain(input_value), "unique-concat chain result")
        if output_value not in result:
            result.append(output_value)
        return result

    return inner


def transform_resolution(input_value: str) -> str:
    input_value = lowercase(_require_string(input_value))

    if "2160" in input_value or "4k" in input_value:
        return "2160p"
    if "1440" in input_value or "2k" in input_value:
        return "1440p"
    if "1080" in input_value:
        return "1080p"
    if "720" in input_value:
        return "720p"
    if "480" in input_value:
        return "480p"
    if "360" in input_value:
        return "360p"
    if "240" in input_value:
        return "240p"
    return input_value
