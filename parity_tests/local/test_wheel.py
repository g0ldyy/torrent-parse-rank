import importlib.util
from pathlib import Path
from zipfile import ZipFile

import pytest

_CHECKER_PATH = Path(__file__).resolve().parents[2] / "scripts" / "check_wheel.py"
_CHECKER_SPEC = importlib.util.spec_from_file_location("check_wheel", _CHECKER_PATH)
assert _CHECKER_SPEC is not None and _CHECKER_SPEC.loader is not None
_CHECKER = importlib.util.module_from_spec(_CHECKER_SPEC)
_CHECKER_SPEC.loader.exec_module(_CHECKER)

FORBIDDEN_PATHS = _CHECKER.FORBIDDEN_PATHS
REQUIRED_PATHS = _CHECKER.REQUIRED_PATHS
validate_wheel = _CHECKER.validate_wheel


def _write_wheel(path: Path, names: set[str]) -> None:
    with ZipFile(path, "w") as archive:
        for name in names:
            archive.writestr(name, b"test")


def test_wheel_validator_accepts_one_current_native_package(tmp_path: Path):
    wheel = tmp_path / "current.whl"
    _write_wheel(
        wheel,
        REQUIRED_PATHS | {"torrent_parse_rank_native/_native.abi3.so"},
    )

    validate_wheel(wheel)


@pytest.mark.parametrize(
    "extra",
    sorted(FORBIDDEN_PATHS)
    + [
        "torrent_parse_rank_native/_native.second.so",
    ],
)
def test_wheel_validator_rejects_build_sources_and_duplicate_natives(tmp_path: Path, extra: str):
    wheel = tmp_path / "invalid.whl"
    _write_wheel(
        wheel,
        REQUIRED_PATHS | {"torrent_parse_rank_native/_native.abi3.so", extra},
    )

    with pytest.raises(ValueError):
        validate_wheel(wheel)
