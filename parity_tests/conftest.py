import pytest

LEGACY_HDR10_CASES = {
    "Spider-Man - Complete Movie Collection (2002-2022) "
    "1080p.HEVC.HDR10.1920x800.x265. DTS-HD",
    "Агентство / The Agency / Сезон: 1 / Серии: 1-10 из 10 "
    "[2024 HEVC HDR10 Dolby Vision WEB-DL 2160p 4k] MVO (HDRezka Studio) "
    "+ DVO (Viruse Project) + Original + Sub (Eng)",
}


def pytest_collection_modifyitems(items):
    for item in items:
        if item.path.parent.name != "ptt" or not hasattr(item, "callspec"):
            continue
        params = item.callspec.params
        if params.get("release_name") not in LEGACY_HDR10_CASES:
            continue
        expected = params.get("expected_hdr", params.get("expected_output", {}).get("hdr", []))
        if "HDR" in expected:
            item.add_marker(
                pytest.mark.skip(
                    reason="Upstream expects generic HDR for HDR10; covered by local HDR format tests"
                )
            )
