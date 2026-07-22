import subprocess
from unittest.mock import patch

import orjson
import pytest
from RTN.file_parser import FFPROBE_TIMEOUT_SECONDS, parse_media_file


def _completed_probe(payload: object) -> subprocess.CompletedProcess:
    return subprocess.CompletedProcess(
        args=["ffprobe"],
        returncode=0,
        stdout=orjson.dumps(payload),
        stderr=b"",
    )


def test_media_parser_ignores_cover_art_and_tolerates_null_numbers(tmp_path):
    media_file = tmp_path / "video.mkv"
    media_file.write_bytes(b"media")
    payload = {
        "format": {
            "size": None,
            "duration": None,
            "bit_rate": "invalid",
            "format_name": "matroska,webm",
        },
        "streams": [
            {
                "codec_type": "video",
                "codec_name": "mjpeg",
                "width": 4000,
                "height": 4000,
                "disposition": {"attached_pic": 1},
            },
            {
                "codec_type": "video",
                "codec_name": "h264",
                "width": "1920",
                "height": "1080",
                "r_frame_rate": "24000/1001",
                "disposition": {"default": 1},
            },
            {
                "codec_type": "audio",
                "codec_name": "aac",
                "channels": None,
                "sample_rate": "invalid",
                "tags": None,
            },
        ],
    }

    with patch("RTN.file_parser.subprocess.run", return_value=_completed_probe(payload)):
        metadata = parse_media_file(media_file)

    assert metadata.file_size == len(b"media")
    assert metadata.duration == 0
    assert metadata.bitrate == 0
    assert metadata.format == ["matroska", "webm"]
    assert metadata.video.codec == "h264"
    assert metadata.video.width == 1920
    assert metadata.video.frame_rate == 23.98
    assert metadata.audio[0].channels == 0
    assert metadata.audio[0].sample_rate == 0
    assert metadata.audio[0].language == ""


def test_media_parser_bounds_ffprobe_runtime(tmp_path):
    media_file = tmp_path / "video.mkv"
    media_file.touch()
    timeout = subprocess.TimeoutExpired(["ffprobe"], FFPROBE_TIMEOUT_SECONDS)

    with patch("RTN.file_parser.subprocess.run", side_effect=timeout):
        with pytest.raises(TimeoutError, match="timed out after 30 seconds"):
            parse_media_file(media_file)


@pytest.mark.parametrize("value", ["NaN", "Infinity", "-Infinity"])
def test_media_parser_rejects_non_finite_numbers(tmp_path, value):
    media_file = tmp_path / "video.mkv"
    media_file.write_bytes(b"media")
    payload = {
        "format": {"duration": value},
        "streams": [
            {
                "codec_type": "video",
                "codec_name": "h264",
                "r_frame_rate": value,
            }
        ],
    }

    with patch("RTN.file_parser.subprocess.run", return_value=_completed_probe(payload)):
        metadata = parse_media_file(media_file)

    assert metadata.duration == 0
    assert metadata.video.frame_rate == 0
