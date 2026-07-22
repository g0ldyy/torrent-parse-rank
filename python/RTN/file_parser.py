import math
import subprocess
from fractions import Fraction
from pathlib import Path

import orjson
from pydantic import BaseModel, Field

FFPROBE_TIMEOUT_SECONDS = 30


class VideoTrack(BaseModel):
    """Model representing video track metadata"""

    codec: str = Field(default="", description="Codec of the video track")
    width: int = Field(default=0, description="Width of the video track")
    height: int = Field(default=0, description="Height of the video track")
    frame_rate: float = Field(default=0.0, description="Frame rate of the video track")


class AudioTrack(BaseModel):
    """Model representing audio track metadata"""

    codec: str = Field(default="", description="Codec of the audio track")
    channels: int = Field(default=0, description="Number of channels in the audio track")
    sample_rate: int = Field(default=0, description="Sample rate of the audio track")
    language: str = Field(default="", description="Language of the audio track")


class SubtitleTrack(BaseModel):
    """Model representing subtitle track metadata"""

    codec: str = Field(default="", description="Codec of the subtitle track")
    language: str = Field(default="", description="Language of the subtitle track")


class MediaMetadata(BaseModel):
    """Model representing complete media file metadata"""

    filename: str = Field(default="", description="Name of the media file")
    file_size: int = Field(default=0, description="Size of the media file in bytes")
    video: VideoTrack = Field(default_factory=VideoTrack, description="Video track metadata")
    duration: float = Field(default=0.0, description="Duration of the video in seconds")
    format: list[str] = Field(default_factory=list, description="Format of the video")
    bitrate: int = Field(default=0, description="Bitrate of the video in bits per second")
    audio: list[AudioTrack] = Field(default_factory=list, description="Audio tracks in the video")
    subtitles: list[SubtitleTrack] = Field(
        default_factory=list, description="Subtitles in the video"
    )

    @property
    def size_in_mb(self) -> float:
        """Return the file size in MB, rounded to 2 decimal places"""
        return round(self.file_size / (1024 * 1024), 2)

    @property
    def duration_in_mins(self) -> float:
        """Return the duration in minutes, rounded to 2 decimal places"""
        return round(self.duration / 60, 2)


def _stream_language(stream: dict) -> str:
    tags = stream.get("tags")
    if not isinstance(tags, dict):
        return ""
    language = tags.get("language")
    return language if isinstance(language, str) else ""


def _parse_frame_rate(frame_rate: object) -> float:
    if not isinstance(frame_rate, (str, int, float)):
        return 0.0
    try:
        frame_rate_text = str(frame_rate)
        if "/" in frame_rate_text:
            parsed = float(Fraction(frame_rate_text))
        else:
            parsed = float(frame_rate_text)
        return parsed if math.isfinite(parsed) else 0.0
    except (OverflowError, TypeError, ValueError, ZeroDivisionError):
        return 0.0


def _safe_int(value: object, default: int = 0) -> int:
    try:
        if value is None or isinstance(value, bool):
            return default
        return int(value)
    except (TypeError, ValueError, OverflowError):
        return default


def _safe_float(value: object, default: float = 0.0) -> float:
    try:
        if value is None or isinstance(value, bool):
            return default
        parsed = float(value)
        return parsed if math.isfinite(parsed) else default
    except (TypeError, ValueError, OverflowError):
        return default


def _stream_is_attached_picture(stream: dict) -> bool:
    disposition = stream.get("disposition")
    return isinstance(disposition, dict) and bool(disposition.get("attached_pic"))


def _video_priority(stream: dict) -> tuple[bool, int]:
    disposition = stream.get("disposition")
    is_default = isinstance(disposition, dict) and bool(disposition.get("default"))
    area = _safe_int(stream.get("width")) * _safe_int(stream.get("height"))
    return is_default, area


def parse_media_file(file_path: str | Path) -> MediaMetadata:
    """
    Parse a media file using ffprobe and return its metadata.

    Args:
        file_path: Path to the media file

    Returns:
        MediaMetadata object if successful

    Raises:
        FileNotFoundError: If the file doesn't exist
        RuntimeError: If ffprobe returns an error
        TimeoutError: If ffprobe does not finish within 30 seconds
    """
    path = Path(file_path)
    if not path.exists():
        raise FileNotFoundError(f"File {path} does not exist.")

    cmd = [
        "ffprobe",
        "-v",
        "error",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
        str(path),
    ]

    try:
        result = subprocess.run(
            cmd,
            check=True,
            capture_output=True,
            timeout=FFPROBE_TIMEOUT_SECONDS,
        )
    except FileNotFoundError as err:
        raise FileNotFoundError("Error: ffprobe not found. Ensure FFmpeg is installed.") from err
    except subprocess.TimeoutExpired as err:
        raise TimeoutError(
            f"ffprobe timed out after {FFPROBE_TIMEOUT_SECONDS} seconds for {path}"
        ) from err
    except subprocess.CalledProcessError as err:
        stderr = (err.stderr or b"").decode("utf-8", errors="replace").strip()
        raise RuntimeError(f"ffprobe error: {stderr or err}") from err

    probe_data = orjson.loads(result.stdout)
    if not isinstance(probe_data, dict):
        raise ValueError("ffprobe returned a non-object JSON document")
    format_info = probe_data.get("format", {})
    if not isinstance(format_info, dict):
        format_info = {}
    format_name = format_info.get("format_name")

    metadata_dict = {
        "filename": path.name,
        "file_size": _safe_int(format_info.get("size"), path.stat().st_size),
        "duration": round(_safe_float(format_info.get("duration")), 2),
        "format": format_name.split(",") if isinstance(format_name, str) else [],
        "bitrate": _safe_int(format_info.get("bit_rate")),
    }

    audio_tracks: list[AudioTrack] = []
    subtitle_tracks: list[SubtitleTrack] = []
    video_data: VideoTrack | None = None

    streams = probe_data.get("streams", [])
    if not isinstance(streams, list):
        streams = []
    selected_video_priority = (False, -1)
    for stream in streams:
        if not isinstance(stream, dict):
            continue
        codec_type = stream.get("codec_type")
        codec_name = stream.get("codec_name")
        codec = codec_name if isinstance(codec_name, str) else "unknown"

        if codec_type == "video" and not _stream_is_attached_picture(stream):
            priority = _video_priority(stream)
            if video_data is not None and priority <= selected_video_priority:
                continue
            fps = _parse_frame_rate(stream.get("r_frame_rate", "0/1"))
            video_data = VideoTrack(
                codec=codec,
                width=_safe_int(stream.get("width")),
                height=_safe_int(stream.get("height")),
                frame_rate=round(fps, 2),
            )
            selected_video_priority = priority
        elif codec_type == "audio":
            audio_tracks.append(
                AudioTrack(
                    codec=codec,
                    channels=_safe_int(stream.get("channels")),
                    sample_rate=_safe_int(stream.get("sample_rate")),
                    language=_stream_language(stream),
                )
            )
        elif codec_type == "subtitle":
            subtitle_tracks.append(SubtitleTrack(codec=codec, language=_stream_language(stream)))

    if video_data:
        metadata_dict["video"] = video_data
    if audio_tracks:
        metadata_dict["audio"] = audio_tracks
    if subtitle_tracks:
        metadata_dict["subtitles"] = subtitle_tracks

    return MediaMetadata(**metadata_dict)
