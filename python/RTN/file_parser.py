import subprocess
from fractions import Fraction
from pathlib import Path

import orjson
from pydantic import BaseModel, Field


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
    return stream.get("tags", {}).get("language") or ""


def _parse_frame_rate(frame_rate: str) -> float:
    try:
        if "/" in frame_rate:
            return float(Fraction(frame_rate))
        return float(frame_rate)
    except (ValueError, ZeroDivisionError):
        return 0.0


def parse_media_file(file_path: str | Path) -> MediaMetadata:
    """
    Parse a media file using ffprobe and return its metadata.

    Args:
        file_path: Path to the media file

    Returns:
        MediaMetadata object if successful

    Raises:
        FileNotFoundError: If the file doesn't exist
        subprocess.CalledProcessError: If ffprobe returns an error
    """
    path = Path(file_path)
    if not path.exists():
        raise FileNotFoundError(f"File {path} does not exist.")

    cmd = [
        "ffprobe",
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
        str(path),
    ]

    try:
        result = subprocess.check_output(cmd, text=True)
    except FileNotFoundError as err:
        raise FileNotFoundError("Error: ffprobe not found. Ensure FFmpeg is installed.") from err
    except subprocess.CalledProcessError as err:
        raise RuntimeError(f"ffprobe error: {err}") from err

    probe_data = orjson.loads(result)
    format_info = probe_data.get("format", {})

    metadata_dict = {
        "filename": path.name,
        "file_size": int(format_info.get("size", 0)),
        "duration": round(float(format_info.get("duration", 0)), 2),
        "format": format_info.get("format_name", "unknown").split(",")
        if format_info.get("format_name")
        else [],
        "bitrate": int(format_info.get("bit_rate", 0)),
    }

    audio_tracks: list[AudioTrack] = []
    subtitle_tracks: list[SubtitleTrack] = []
    video_data: VideoTrack | None = None

    for stream in probe_data.get("streams", []):
        codec_type = stream.get("codec_type")
        codec = stream.get("codec_name", "unknown")

        if codec_type == "video":
            fps = _parse_frame_rate(stream.get("r_frame_rate", "0/1"))
            video_data = VideoTrack(
                codec=codec,
                width=int(stream.get("width", 0)),
                height=int(stream.get("height", 0)),
                frame_rate=round(fps, 2),
            )
        elif codec_type == "audio":
            audio_tracks.append(
                AudioTrack(
                    codec=codec,
                    channels=int(stream.get("channels", 0)),
                    sample_rate=int(stream.get("sample_rate", 0)),
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
