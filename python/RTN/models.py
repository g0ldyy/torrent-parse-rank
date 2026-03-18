"""
This module contains models used in the RTN package for parsing torrent titles, ranking media quality, and defining user settings.

Models:
- `ParsedData`: Model for storing parsed information from a torrent title.
- `BaseRankingModel`: Base class for ranking models used in the context of media quality and attributes.
- `Torrent`: Model for representing a torrent with metadata parsed from its title and additional computed properties.
- `DefaultRanking`: Ranking model preset that prioritizes the highest quality and most desirable attributes.
- `CustomRank`: Model used in the `SettingsModel` for defining custom ranks for specific attributes.
- `SettingsModel`: User-defined settings model for ranking torrents, including preferences for filtering torrents based on regex patterns and customizing ranks for specific torrent attributes.

For more information on each model, refer to the respective docstrings.

Note:
- The `ParsedData` model contains attributes for storing parsed information from a torrent title.
- The `BaseRankingModel` model is a base class for ranking models used in the context of media quality and attributes.
- The `CustomRank` model is used in the `SettingsModel` for defining custom ranks for specific attributes.
- The `SettingsModel` model allows users to define custom settings for ranking torrents based on quality attributes and regex patterns.
"""

import json
from pathlib import Path
from typing import Any, TypeAlias

import regex
from pydantic import (
    BaseModel,
    ConfigDict,
    Field,
    field_serializer,
    field_validator,
    model_validator,
)
from regex import Pattern

from RTN.exceptions import GarbageTorrent

INFOHASH_PATTERN: Pattern = regex.compile(r"^[a-fA-F0-9]{32}$|^[a-fA-F0-9]{40}$")


class ParsedData(BaseModel):
    """Parsed data model for a torrent title."""

    raw_title: str
    parsed_title: str = ""
    normalized_title: str = ""
    trash: bool = False
    adult: bool = False
    year: int | None = None
    resolution: str = "unknown"
    seasons: list[int] = Field(default_factory=list)
    episodes: list[int] = Field(default_factory=list)
    complete: bool = False
    volumes: list[int] = Field(default_factory=list)
    languages: list[str] = Field(default_factory=list)
    quality: str | None = None
    hdr: list[str] = Field(default_factory=list)
    codec: str | None = None
    audio: list[str] = Field(default_factory=list)
    channels: list[str] = Field(default_factory=list)
    dubbed: bool = False
    subbed: bool = False
    date: str | None = None
    group: str | None = None
    edition: str | None = None
    bit_depth: str | None = None
    bitrate: str | None = None
    network: str | None = None
    extended: bool = False
    converted: bool = False
    hardcoded: bool = False
    region: str | None = None
    ppv: bool = False
    _3d: bool = False
    site: str | None = None
    size: str | None = None
    proper: bool = False
    repack: bool = False
    retail: bool = False
    upscaled: bool = False
    remastered: bool = False
    unrated: bool = False
    uncensored: bool = False
    documentary: bool = False
    commentary: bool = False
    episode_code: str | None = None
    country: str | None = None
    container: str | None = None
    extension: str | None = None
    extras: list[str] = Field(default_factory=list)
    torrent: bool = False
    scene: bool = False

    model_config = ConfigDict(from_attributes=True)

    @property
    def type(self) -> str:
        """Returns the type of the torrent based on its attributes."""
        if not self.seasons and not self.episodes:
            return "movie"
        return "show"

    def to_dict(self):
        return self.model_dump_json()


class Torrent(BaseModel):
    """
    Represents a torrent with metadata parsed from its title and additional computed properties.

    Attributes:
        `raw_title` (str): The original title of the torrent.
        `infohash` (str): The SHA-1 hash identifier of the torrent.
        `data` (ParsedData): Metadata extracted from the torrent title.
        `fetch` (bool): Indicates whether the torrent meets the criteria for fetching based on user settings.
        `rank` (int): The computed ranking score of the torrent based on user-defined preferences.
        `lev_ratio` (float): The Levenshtein ratio comparing the parsed title and the raw title for similarity.

    Methods:
        __eq__: Determines equality based on the infohash of the torrent, allowing for easy comparison.
        __hash__: Generates a hash based on the infohash of the torrent for set operations.

    Raises:
        `GarbageTorrent`: If the title is identified as trash and should be ignored by the scraper.

    Example:
        >>> torrent = Torrent(
        ...     raw_title="The Walking Dead S05E03 720p HDTV x264-ASAP[ettv]",
        ...     infohash="c08a9ee8ce3a5c2c08865e2b05406273cabc97e7",
        ...     data=ParsedData(...),
        ...     fetch=True,
        ...     rank=500,
        ...     lev_ratio=0.95,
        ... )
        >>> isinstance(torrent, Torrent)
        True
        >>> torrent.raw_title
        'The Walking Dead S05E03 720p HDTV x264-ASAP[ettv]'
        >>> torrent.infohash
        'c08a9ee8ce3a5c2c08865e2b05406273cabc97e7'
        >>> torrent.data.parsed_title
        'The Walking Dead'
        >>> torrent.fetch
        True
        >>> torrent.rank
        500
        >>> torrent.lev_ratio
        0.95
    """

    infohash: str
    raw_title: str
    torrent: str | None = None
    seeders: int | None = 0
    leechers: int | None = 0
    trackers: list[str] | None = Field(default_factory=list)
    data: ParsedData
    fetch: bool = False
    rank: int = 0
    lev_ratio: float = 0.0

    model_config = ConfigDict(from_attributes=True, frozen=True)

    @field_validator("infohash")
    def validate_infohash(cls, v):
        """Validates infohash length and format (MD5 or SHA-1)."""
        if len(v) not in (32, 40) or not INFOHASH_PATTERN.match(v):
            raise GarbageTorrent(
                "Infohash must be a 32-character MD5 hash or a 40-character SHA-1 hash."
            )
        return v

    def __eq__(self, other: object) -> bool:
        """Compares Torrent objects based on their infohash."""
        return isinstance(other, Torrent) and self.infohash == other.infohash

    def __hash__(self) -> int:
        return hash(self.infohash)

    def to_dict(self):
        return self.model_dump_json()


class BaseRankingModel(BaseModel):
    """
    A base class for ranking models used in the context of media quality and attributes.
    The ranking values are used to determine the quality of a media item based on its attributes.

    Note:
        - The higher the ranking value, the better the quality of the media item.
        - The default ranking values are set to 0, which means that the attribute does not affect the overall rank.
        - Users can customize the ranking values based on their preferences and requirements by using inheritance.
    """

    # quality
    av1: int = 0
    avc: int = 0
    bluray: int = 0
    dvd: int = 0
    hdtv: int = 0
    hevc: int = 0
    mpeg: int = 0
    remux: int = 0
    vhs: int = 0
    web: int = 0
    webdl: int = 0
    webmux: int = 0
    xvid: int = 0

    # rips
    bdrip: int = 0
    brrip: int = 0
    dvdrip: int = 0
    hdrip: int = 0
    ppvrip: int = 0
    tvrip: int = 0
    uhdrip: int = 0
    vhsrip: int = 0
    webdlrip: int = 0
    webrip: int = 0

    # hdr
    bit_10: int = 0
    dolby_vision: int = 0
    hdr: int = 0
    hdr10plus: int = 0
    sdr: int = 0

    # audio
    aac: int = 0
    atmos: int = 0
    dolby_digital: int = 0
    dolby_digital_plus: int = 0
    dts_lossy: int = 0
    dts_lossless: int = 0
    # opus: int = 0
    # pcm: int = 0
    flac: int = 0
    mono: int = 0
    mp3: int = 0
    stereo: int = 0
    surround: int = 0
    truehd: int = 0

    # extras
    three_d: int = 0
    converted: int = 0
    documentary: int = 0
    commentary: int = 0
    uncensored: int = 0
    dubbed: int = 0
    edition: int = 0
    hardcoded: int = 0
    network: int = 0
    proper: int = 0
    repack: int = 0
    retail: int = 0
    subbed: int = 0
    upscaled: int = 0
    scene: int = 0

    # trash
    cam: int = 0
    clean_audio: int = 0
    r5: int = 0
    pdtv: int = 0
    satrip: int = 0
    screener: int = 0
    site: int = 0
    size: int = 0
    telecine: int = 0
    telesync: int = 0


class DefaultRanking(BaseRankingModel):
    """Ranking model preset that covers the highest qualities like 4K HDR."""

    # quality
    av1: int = 500
    avc: int = 500
    bluray: int = 100
    dvd: int = -5000
    hdtv: int = -5000
    hevc: int = 500
    mpeg: int = -1000
    remux: int = 10000
    vhs: int = -10000
    web: int = 100
    webdl: int = 200
    webmux: int = -10000
    xvid: int = -10000
    pdtv: int = -10000

    # rips
    bdrip: int = -5000
    brrip: int = -10000
    dvdrip: int = -5000
    hdrip: int = -10000
    ppvrip: int = -10000
    tvrip: int = -10000
    uhdrip: int = -5000
    vhsrip: int = -10000
    webdlrip: int = -10000
    webrip: int = -1000

    # hdr
    bit_10: int = 100
    dolby_vision: int = 3000
    hdr: int = 2000
    hdr10plus: int = 2100

    # audio
    aac: int = 100
    atmos: int = 1000
    dolby_digital: int = 50
    dolby_digital_plus: int = 150
    dts_lossy: int = 100
    dts_lossless: int = 2000
    mp3: int = -1000
    truehd: int = 2000

    # extras
    three_d: int = -10000
    converted: int = -1000
    documentary: int = -250
    dubbed: int = -1000
    edition: int = 100
    proper: int = 20
    repack: int = 20
    site: int = -10000
    upscaled: int = -10000

    # trash
    cam: int = -10000
    clean_audio: int = -10000
    r5: int = -10000
    satrip: int = -10000
    screener: int = -10000
    size: int = -10000
    telecine: int = -10000
    telesync: int = -10000


class ConfigModelBase(BaseModel):
    """Base class for config models that need dict-like behavior"""

    def __getitem__(self, key: str) -> Any:
        return getattr(self, key)

    def get(self, key: str, default: Any = None) -> Any:
        try:
            return self[key]
        except (KeyError, AttributeError):
            return default


class ResolutionConfig(ConfigModelBase):
    """Configuration for which resolutions are enabled."""

    def __getitem__(self, key: str) -> Any:
        # Special handling for resolution fields - add 'r' prefix
        field_name = f"r{key}" if key.endswith("p") else key
        return getattr(self, field_name)

    r2160p: bool = Field(default=False)
    r1440p: bool = Field(default=True)
    r1080p: bool = Field(default=True)
    r720p: bool = Field(default=True)
    r576p: bool = Field(default=False)
    r480p: bool = Field(default=False)
    r360p: bool = Field(default=False)
    r240p: bool = Field(default=False)
    unknown: bool = Field(default=True)

    model_config = ConfigDict(populate_by_name=True)

    def json(self, **kwargs) -> str:
        """Ensure alias serialization for JSON output"""
        return super().model_dump_json(by_alias=True, **kwargs)


class OptionsConfig(ConfigModelBase):
    """Configuration for various options."""

    title_similarity: float = Field(default=0.85)
    remove_all_trash: bool = Field(default=True)
    remove_ranks_under: int = Field(default=-10000)
    remove_unknown_languages: bool = Field(default=False)
    allow_english_in_languages: bool = Field(default=True)
    enable_fetch_speed_mode: bool = Field(default=True)
    remove_adult_content: bool = Field(default=True)


class LanguagesConfig(ConfigModelBase):
    """Configuration for which languages are enabled.

    Attributes:
        required: Languages that MUST be present in the torrent. If set, torrents without
                  at least one of these languages will be excluded.
        allowed: Languages that bypass the exclusion logic. If a torrent contains any of
                 these languages, it won't be excluded even if it also contains excluded languages.
        exclude: Languages that should be excluded from results.
        preferred: Languages that are preferred (used for ranking).
    """

    required: list[str] = Field(default_factory=list)
    allowed: list[str] = Field(default_factory=list)
    exclude: list[str] = Field(default_factory=list)
    preferred: list[str] = Field(default_factory=list)


class CustomRank(BaseModel):
    """Custom Ranks used in SettingsModel."""

    fetch: bool = Field(default=True)
    use_custom_rank: bool = Field(default=False)
    rank: int = Field(default=0)


def _rank_true() -> CustomRank:
    return CustomRank(fetch=True)


def _rank_false() -> CustomRank:
    return CustomRank(fetch=False)


def _rank_field(fetch: bool = True, *, alias: str | None = None) -> Any:
    factory = _rank_true if fetch else _rank_false
    return Field(default_factory=factory, alias=alias)


def _compile_pattern(pattern: "PatternType") -> Pattern:
    if isinstance(pattern, str):
        if pattern.startswith("/") and pattern.endswith("/"):
            return regex.compile(pattern[1:-1])
        return regex.compile(pattern, regex.IGNORECASE)
    if isinstance(pattern, Pattern):
        return pattern
    raise ValueError(f"Invalid pattern type: {type(pattern)}")


def _compile_pattern_list(values: Any) -> list[Pattern]:
    if values is None:
        return []
    if not isinstance(values, (list, tuple)):
        return []
    return [_compile_pattern(value) for value in values]


class QualityRankModel(ConfigModelBase):
    """Ranking configuration for quality attributes."""

    av1: CustomRank = _rank_field(fetch=False)
    avc: CustomRank = _rank_field()
    bluray: CustomRank = _rank_field()
    dvd: CustomRank = _rank_field(fetch=False)
    hdtv: CustomRank = _rank_field()
    hevc: CustomRank = _rank_field()
    mpeg: CustomRank = _rank_field(fetch=False)
    remux: CustomRank = _rank_field(fetch=False)
    vhs: CustomRank = _rank_field(fetch=False)
    web: CustomRank = _rank_field()
    webdl: CustomRank = _rank_field()
    webmux: CustomRank = _rank_field(fetch=False)
    xvid: CustomRank = _rank_field(fetch=False)


class RipsRankModel(ConfigModelBase):
    """Ranking configuration for rips attributes."""

    bdrip: CustomRank = _rank_field(fetch=False)
    brrip: CustomRank = _rank_field()
    dvdrip: CustomRank = _rank_field(fetch=False)
    hdrip: CustomRank = _rank_field()
    ppvrip: CustomRank = _rank_field(fetch=False)
    satrip: CustomRank = _rank_field(fetch=False)
    tvrip: CustomRank = _rank_field(fetch=False)
    uhdrip: CustomRank = _rank_field(fetch=False)
    vhsrip: CustomRank = _rank_field(fetch=False)
    webdlrip: CustomRank = _rank_field(fetch=False)
    webrip: CustomRank = _rank_field()


class HdrRankModel(ConfigModelBase):
    """Ranking configuration for HDR attributes."""

    def __getitem__(self, key: str) -> Any:
        # Special handling for '10bit' key
        if key == "10bit":
            return self.bit10
        return super().__getitem__(key)

    bit10: CustomRank = _rank_field()
    dolby_vision: CustomRank = _rank_field(fetch=False)
    hdr: CustomRank = _rank_field()
    hdr10plus: CustomRank = _rank_field()
    sdr: CustomRank = _rank_field()


class AudioRankModel(ConfigModelBase):
    """Ranking configuration for audio attributes."""

    aac: CustomRank = _rank_field()
    atmos: CustomRank = _rank_field()
    dolby_digital: CustomRank = _rank_field()
    dolby_digital_plus: CustomRank = _rank_field()
    dts_lossy: CustomRank = _rank_field()
    dts_lossless: CustomRank = _rank_field()
    # opus: CustomRank = _rank_field()
    # pcm: CustomRank = _rank_field()
    flac: CustomRank = _rank_field()
    mono: CustomRank = _rank_field(fetch=False)
    mp3: CustomRank = _rank_field(fetch=False)
    stereo: CustomRank = _rank_field()
    surround: CustomRank = _rank_field()
    truehd: CustomRank = _rank_field()


class ExtrasRankModel(ConfigModelBase):
    """Ranking configuration for extras attributes."""

    three_d: CustomRank = _rank_field(fetch=False, alias="3d")
    converted: CustomRank = _rank_field(fetch=False)
    documentary: CustomRank = _rank_field(fetch=False)
    dubbed: CustomRank = _rank_field()
    edition: CustomRank = _rank_field()
    hardcoded: CustomRank = _rank_field()
    network: CustomRank = _rank_field()
    proper: CustomRank = _rank_field()
    repack: CustomRank = _rank_field()
    retail: CustomRank = _rank_field()
    site: CustomRank = _rank_field(fetch=False)
    subbed: CustomRank = _rank_field()
    upscaled: CustomRank = _rank_field(fetch=False)
    scene: CustomRank = _rank_field()
    uncensored: CustomRank = _rank_field()

    model_config = ConfigDict(
        populate_by_name=True,
        alias_generator=lambda field_name: "3d" if field_name == "three_d" else field_name,
    )


class TrashRankModel(ConfigModelBase):
    """Ranking configuration for trash attributes."""

    cam: CustomRank = _rank_field(fetch=False)
    clean_audio: CustomRank = _rank_field(fetch=False)
    pdtv: CustomRank = _rank_field(fetch=False)
    r5: CustomRank = _rank_field(fetch=False)
    screener: CustomRank = _rank_field(fetch=False)
    size: CustomRank = _rank_field(fetch=False)
    telecine: CustomRank = _rank_field(fetch=False)
    telesync: CustomRank = _rank_field(fetch=False)


class CustomRanksConfig(ConfigModelBase):
    """Configuration for custom ranks."""

    quality: QualityRankModel = Field(default_factory=QualityRankModel)
    rips: RipsRankModel = Field(default_factory=RipsRankModel)
    hdr: HdrRankModel = Field(default_factory=HdrRankModel)
    audio: AudioRankModel = Field(default_factory=AudioRankModel)
    extras: ExtrasRankModel = Field(default_factory=ExtrasRankModel)
    trash: TrashRankModel = Field(default_factory=TrashRankModel)


PatternType: TypeAlias = regex.Pattern[str] | str

CustomRankDict: TypeAlias = dict[str, CustomRank]


class SettingsModel(BaseModel):
    """
    Represents user-defined settings for ranking torrents, including preferences for filtering torrents
    based on regex patterns and customizing ranks for specific torrent attributes.

    Attributes:
        require (List[str | Pattern]): Patterns torrents must match to be considered.
        exclude (List[str | Pattern]): Patterns that, if matched, result in torrent exclusion.
        preferred (List[str | Pattern]): Patterns indicating preferred attributes in torrents. Given +10000 points by default.
        resolutions (ResolutionConfig): Configuration for which resolutions are enabled.
        options (OptionsConfig): Configuration for various options like title similarity and trash removal.
        languages (LanguagesConfig): Configuration for which languages are enabled, excluded, and preferred.
        custom_ranks (CustomRanksConfig): Custom ranking configurations for specific attributes.

    Methods:
        compile_and_validate_patterns: Compiles string patterns to regex.Pattern objects, handling case sensitivity.

    Note:
        - Patterns enclosed in '/' are compiled as case-sensitive.
        - Patterns not enclosed are compiled as case-insensitive by default.
        - The model supports advanced regex features for precise filtering and ranking.

    Example:
        >>> settings = SettingsModel(
        ...     require=["\\b4K|1080p\\b", "720p"],
        ...     exclude=["CAM", "TS"],
        ...     preferred=["BluRay", r"/\\bS\\d+/", "/HDR|HDR10/"],
        ...     resolutions=ResolutionConfig(r1080p=True, r720p=True),
        ...     options=OptionsConfig(remove_all_trash=True),
        ...     languages=LanguagesConfig(required=["en"]),
        ...     custom_ranks=CustomRanksConfig()
        ... )
        >>> print([p.pattern for p in settings.require])
        ['\\b4K|1080p\\b', '720p']
        >>> print(settings.resolutions.r1080p)
        True
        >>> print(settings.options.remove_all_trash)
        True
    """

    name: str = Field(default="example", description="Name of the settings")
    enabled: bool = Field(default=True, description="Whether these settings will be used or not")
    require: list[PatternType] = Field(
        default_factory=list,
        description="Patterns torrents must match to be considered",
    )
    exclude: list[PatternType] = Field(
        default_factory=list,
        description="Patterns that, if matched, result in torrent exclusion",
    )
    preferred: list[PatternType] = Field(
        default_factory=list,
        description="Patterns indicating preferred attributes in torrents",
    )
    resolutions: ResolutionConfig = Field(
        default_factory=ResolutionConfig,
        description="Configuration for enabled resolutions",
    )
    options: OptionsConfig = Field(
        default_factory=OptionsConfig,
        description="General options for torrent filtering and ranking",
    )
    languages: LanguagesConfig = Field(
        default_factory=LanguagesConfig,
        description="Language preferences and restrictions",
    )
    custom_ranks: CustomRanksConfig = Field(
        default_factory=CustomRanksConfig,
        description="Custom ranking configurations for specific attributes",
    )

    @model_validator(mode="before")
    def compile_and_validate_patterns(cls, values: dict[str, Any]) -> dict[str, Any]:
        """Compile string patterns to regex.Pattern, keeping compiled patterns unchanged."""
        for field in ("require", "exclude", "preferred"):
            values[field] = _compile_pattern_list(values.get(field))

        return values

    @field_serializer("require", "exclude", "preferred", when_used="always")
    def serialize_patterns(self, values: list[PatternType]) -> list[str]:
        """Convert regex patterns to strings for JSON serialization."""
        return [v.pattern if isinstance(v, regex.Pattern) else v for v in values]

    def __getitem__(self, item: str) -> CustomRankDict:
        """Access custom rank settings via attribute keys."""
        return self.custom_ranks[item]

    model_config = ConfigDict(
        arbitrary_types_allowed=True,
        from_attributes=True,
    )

    def save(self, path: str | Path) -> None:
        """
        Save settings to a JSON file.

        Args:
            path: Path where the settings file should be saved.
                 Can be either a string or Path object.

        Example:
            >>> settings = SettingsModel()
            >>> settings.save("my_settings.json")
            >>> settings.save(Path("configs/my_settings.json"))
        """
        path = Path(path)
        path.parent.mkdir(parents=True, exist_ok=True)

        with path.open("w", encoding="utf-8") as f:
            json.dump(self.model_dump(mode="json"), f, indent=4)

    @classmethod
    def load(cls, path: str | Path) -> "SettingsModel":
        """
        Load settings from a JSON file.

        Args:
            path: Path to the settings file.

        Returns:
            SettingsModel: A new settings instance with the loaded configuration.

        Raises:
            FileNotFoundError: If the settings file doesn't exist.
            JSONDecodeError: If the settings file is corrupted or contains invalid JSON.
            ValidationError: If the settings file contains invalid configuration.
        """
        path = Path(path)

        if not path.exists():
            raise FileNotFoundError(f"Settings file not found: {path}")

        with path.open("r", encoding="utf-8") as f:
            data = json.load(f)

        return cls.model_validate(data)

    @classmethod
    def load_or_default(cls, path: str | Path | None = None) -> "SettingsModel":
        """
        Load settings from a file if it exists, otherwise create default settings and save them.

        Args:
            path: Optional path to the settings file.
                If None, returns default settings without saving.

        Returns:
            SettingsModel: Either the loaded settings or default settings.

        Raises:
            JSONDecodeError: If the settings file is corrupted or contains invalid JSON.
        """
        if path is None:
            return cls()

        path = Path(path)
        try:
            return cls.load(path)
        except FileNotFoundError:
            settings = cls()
            settings.save(path)
            return settings

    def changed_only(self) -> dict[str, Any]:
        """
        Compare the provided settings with the default settings and return only the changed fields.

        Args:
            settings: The settings to compare against the default.

        Returns:
            Dict[str, Any]: A dictionary containing only the changed fields and their values.
        """
        return self.model_dump(mode="json", exclude_unset=True, exclude_defaults=True)
