use std::collections::{BTreeSet, HashSet};

use fancy_regex::RegexBuilder;
use ptt_core::parse_title;
use serde_json::{Map, Number, Value};
use strsim::normalized_levenshtein;
use thiserror::Error;
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

#[derive(Debug, Error)]
pub enum RtnError {
    #[error("{0}")]
    InvalidInput(String),
    #[error(transparent)]
    Ptt(#[from] ptt_core::ParseError),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Regex(#[from] fancy_regex::Error),
}

fn map_str<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    map.get(key).and_then(Value::as_str)
}

fn map_bool(map: &Map<String, Value>, key: &str) -> bool {
    map.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn map_array<'a>(map: &'a Map<String, Value>, key: &str) -> &'a [Value] {
    map.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn map_i64s(map: &Map<String, Value>, key: &str) -> Vec<i64> {
    map.get(key)
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default()
}

fn settings_value_array<'a>(settings: &'a Value, key: &str) -> &'a [Value] {
    settings
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn pattern_text(pattern: &Value) -> &str {
    match pattern {
        Value::String(s) => s.as_str(),
        Value::Object(obj) => obj
            .get("pattern")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        _ => "",
    }
}

const ANIME_LANGS: &[&str] = &["ja", "zh", "ko"];
const NON_ANIME_LANGS: &[&str] = &[
    "de", "es", "hi", "ta", "ru", "ua", "th", "it", "ar", "pt", "fr", "pa", "mr", "gu", "te", "kn",
    "ml", "vi", "id", "tr", "he", "fa", "el", "lt", "lv", "et", "pl", "cs", "sk", "hu", "ro", "bg",
    "sr", "hr", "sl", "nl", "da", "fi", "sv", "no", "ms",
];
const COMMON_LANGS: &[&str] = &[
    "de", "es", "hi", "ta", "ru", "ua", "th", "it", "zh", "ar", "fr",
];

const EXTRA_RULES: [(&str, &str, &str); 15] = [
    ("_3d", "extras", "three_d"),
    ("converted", "extras", "converted"),
    ("documentary", "extras", "documentary"),
    ("dubbed", "extras", "dubbed"),
    ("edition", "extras", "edition"),
    ("hardcoded", "extras", "hardcoded"),
    ("network", "extras", "network"),
    ("proper", "extras", "proper"),
    ("repack", "extras", "repack"),
    ("retail", "extras", "retail"),
    ("subbed", "extras", "subbed"),
    ("upscaled", "extras", "upscaled"),
    ("site", "extras", "site"),
    ("scene", "extras", "scene"),
    ("uncensored", "extras", "uncensored"),
];

const EXTRA_FETCH_ONLY_RULES: [(&str, &str, &str); 2] =
    [("size", "trash", "size"), ("bit_depth", "hdr", "10bit")];
const TRASH_QUALITIES: &[&str] = &["CAM", "PDTV", "R5", "SCR", "TeleCine", "TeleSync"];

fn ensure_non_empty_title(raw_title: &str) -> Result<(), RtnError> {
    if raw_title.is_empty() {
        return Err(RtnError::InvalidInput(
            "The input title must be a non-empty string.".to_string(),
        ));
    }
    Ok(())
}

fn value_is_active(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(v)) => *v,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(arr)) => !arr.is_empty(),
        Some(Value::Number(_)) | Some(Value::Object(_)) => true,
        Some(Value::Null) | None => false,
    }
}

fn quality_mapping(quality: &str) -> Option<(&'static str, &'static str)> {
    match quality {
        "WEB" => Some(("quality", "web")),
        "WEB-DL" => Some(("quality", "webdl")),
        "BluRay" => Some(("quality", "bluray")),
        "HDTV" => Some(("quality", "hdtv")),
        "VHS" => Some(("quality", "vhs")),
        "WEBMux" => Some(("quality", "webmux")),
        "BluRay REMUX" | "REMUX" => Some(("quality", "remux")),
        "WEBRip" => Some(("rips", "webrip")),
        "WEB-DLRip" => Some(("rips", "webdlrip")),
        "UHDRip" => Some(("rips", "uhdrip")),
        "HDRip" => Some(("rips", "hdrip")),
        "DVDRip" => Some(("rips", "dvdrip")),
        "BDRip" => Some(("rips", "bdrip")),
        "BRRip" => Some(("rips", "brrip")),
        "VHSRip" => Some(("rips", "vhsrip")),
        "PPVRip" => Some(("rips", "ppvrip")),
        "SATRip" => Some(("rips", "satrip")),
        "TeleCine" => Some(("trash", "telecine")),
        "TeleSync" => Some(("trash", "telesync")),
        "SCR" => Some(("trash", "screener")),
        "R5" => Some(("trash", "r5")),
        "CAM" => Some(("trash", "cam")),
        "PDTV" => Some(("trash", "pdtv")),
        _ => None,
    }
}

fn codec_key(codec: &str) -> Option<&'static str> {
    if codec.eq_ignore_ascii_case("avc") {
        Some("avc")
    } else if codec.eq_ignore_ascii_case("hevc") {
        Some("hevc")
    } else if codec.eq_ignore_ascii_case("xvid") {
        Some("xvid")
    } else if codec.eq_ignore_ascii_case("av1") {
        Some("av1")
    } else if codec.eq_ignore_ascii_case("mpeg") {
        Some("mpeg")
    } else {
        None
    }
}

fn hdr_key(hdr: &str) -> Option<&'static str> {
    match hdr {
        "DV" => Some("dolby_vision"),
        "HDR" => Some("hdr"),
        "HDR10+" => Some("hdr10plus"),
        "SDR" => Some("sdr"),
        _ => None,
    }
}

fn audio_mapping(audio: &str) -> Option<(&'static str, &'static str)> {
    match audio {
        "AAC" => Some(("audio", "aac")),
        "Atmos" => Some(("audio", "atmos")),
        "Dolby Digital" => Some(("audio", "dolby_digital")),
        "Dolby Digital Plus" => Some(("audio", "dolby_digital_plus")),
        "DTS Lossy" => Some(("audio", "dts_lossy")),
        "DTS Lossless" => Some(("audio", "dts_lossless")),
        "FLAC" => Some(("audio", "flac")),
        "MP3" => Some(("audio", "mp3")),
        "TrueHD" => Some(("audio", "truehd")),
        "HQ Clean Audio" => Some(("trash", "clean_audio")),
        _ => None,
    }
}

fn extend_lang_group(target: &mut HashSet<String>, group_name: &str, values: &[&str]) {
    if target.contains(group_name) {
        target.extend(values.iter().map(|v| (*v).to_string()));
    }
}

fn translate_char(ch: char) -> Option<&'static str> {
    match ch {
        'ā' | 'ă' | 'ą' | 'ǎ' | 'ǻ' => Some("a"),
        'ć' | 'č' | 'ç' | 'ĉ' | 'ċ' => Some("c"),
        'ď' | 'đ' => Some("d"),
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ę' | 'ě' | 'ə' => Some("e"),
        'ĝ' | 'ğ' | 'ġ' | 'ģ' | 'ǧ' => Some("g"),
        'ĥ' => Some("h"),
        'î' | 'ï' | 'ì' | 'í' | 'ī' | 'ĩ' | 'ĭ' | 'ı' | 'ǐ' => Some("i"),
        'ĵ' => Some("j"),
        'ķ' => Some("k"),
        'ĺ' | 'ļ' | 'ł' => Some("l"),
        'ń' | 'ň' | 'ñ' | 'ņ' | 'ǹ' | 'ŉ' => Some("n"),
        'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' | 'ǒ' | 'ǿ' => Some("o"),
        'œ' => Some("oe"),
        'ŕ' | 'ř' | 'ŗ' => Some("r"),
        'š' | 'ş' | 'ś' | 'ș' => Some("s"),
        'ß' => Some("ss"),
        'ť' | 'ţ' => Some("t"),
        'ū' | 'ŭ' | 'ũ' | 'û' | 'ü' | 'ù' | 'ú' | 'ų' | 'ű' | 'ǔ' | 'ǚ' | 'ǜ' => {
            Some("u")
        }
        'ŵ' => Some("w"),
        'ý' | 'ÿ' | 'ŷ' => Some("y"),
        'ž' | 'ż' | 'ź' => Some("z"),
        'æ' | 'ǽ' => Some("ae"),
        'ƒ' => Some("f"),
        '&' => Some("and"),
        '.' | '_' => Some(" "),
        '!' | '?' | ',' | ':' | ';' | '\'' => Some(""),
        _ => None,
    }
}

pub fn normalize_title(raw_title: &str, lower: bool) -> String {
    let base = if lower {
        raw_title.to_lowercase()
    } else {
        raw_title.to_string()
    };

    let mut translated = String::with_capacity(base.len());
    for ch in base.nfkd().filter(|c| !is_combining_mark(*c)) {
        if let Some(rep) = translate_char(ch) {
            translated.push_str(rep);
        } else {
            translated.push(ch);
        }
    }

    let mut cleaned = String::with_capacity(translated.len());
    for ch in translated.chars() {
        if ch.is_alphanumeric() || ch.is_whitespace() {
            cleaned.push(ch);
        }
    }

    cleaned.trim().to_string()
}

fn compile_pattern(pattern_value: &Value) -> Result<Option<fancy_regex::Regex>, RtnError> {
    match pattern_value {
        Value::String(pat) => {
            let re = RegexBuilder::new(pat).case_insensitive(true).build()?;
            Ok(Some(re))
        }
        Value::Object(obj) => {
            let pat = obj.get("pattern").and_then(Value::as_str).ok_or_else(|| {
                RtnError::InvalidInput("Pattern object must contain 'pattern'.".to_string())
            })?;
            let ignore_case = obj
                .get("ignore_case")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let re = RegexBuilder::new(pat)
                .case_insensitive(ignore_case)
                .build()?;
            Ok(Some(re))
        }
        Value::Null => Ok(None),
        _ => Err(RtnError::InvalidInput(
            "Pattern entries must be string/object/null.".to_string(),
        )),
    }
}

pub fn check_pattern(patterns: &[Value], raw_title: &str) -> Result<bool, RtnError> {
    for pattern in patterns {
        if let Some(re) = compile_pattern(pattern)?
            && re.is_match(raw_title)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn parse(raw_title: &str, translate_langs: bool) -> Result<Map<String, Value>, RtnError> {
    ensure_non_empty_title(raw_title)?;

    let mut data = parse_title(raw_title, translate_langs)?;

    let parsed_title = data
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    data.insert(
        "raw_title".to_string(),
        Value::String(raw_title.to_string()),
    );
    data.insert(
        "parsed_title".to_string(),
        Value::String(parsed_title.clone()),
    );
    data.insert(
        "normalized_title".to_string(),
        Value::String(normalize_title(&parsed_title, true)),
    );
    data.insert("_3d".to_string(), Value::Bool(map_bool(&data, "3d")));

    Ok(data)
}

pub fn extract_seasons(raw_title: &str) -> Result<Vec<i64>, RtnError> {
    ensure_non_empty_title(raw_title)?;
    let data = parse_title(raw_title, false)?;
    Ok(map_i64s(&data, "seasons"))
}

pub fn extract_episodes(raw_title: &str) -> Result<Vec<i64>, RtnError> {
    ensure_non_empty_title(raw_title)?;
    let data = parse_title(raw_title, false)?;
    Ok(map_i64s(&data, "episodes"))
}

pub fn episodes_from_season(raw_title: &str, season_num: i64) -> Result<Vec<i64>, RtnError> {
    if season_num <= 0 {
        return Err(RtnError::InvalidInput(
            "The season number must be a positive integer.".to_string(),
        ));
    }
    ensure_non_empty_title(raw_title)?;

    let data = parse_title(raw_title, false)?;
    let has_season = map_array(&data, "seasons")
        .iter()
        .filter_map(Value::as_i64)
        .any(|season| season == season_num);
    if !has_season {
        return Ok(Vec::new());
    }
    Ok(map_i64s(&data, "episodes"))
}

pub fn get_lev_ratio(
    correct_title: &str,
    parsed_title: &str,
    threshold: f64,
    aliases: &Map<String, Value>,
) -> Result<f64, RtnError> {
    if correct_title.is_empty() || parsed_title.is_empty() {
        return Err(RtnError::InvalidInput(
            "Both titles must be provided.".to_string(),
        ));
    }
    if !(0.0..=1.0).contains(&threshold) {
        return Err(RtnError::InvalidInput(
            "The threshold must be a number between 0 and 1.".to_string(),
        ));
    }

    let parsed_norm = normalize_title(parsed_title, true);
    let mut best = 0.0_f64;

    let correct_norm = normalize_title(correct_title, true);
    let mut update_best = |candidate: &str| {
        let score = normalized_levenshtein(candidate, &parsed_norm);
        if score >= threshold && score > best {
            best = score;
        }
    };

    update_best(&correct_norm);
    for alias_values in aliases.values().filter_map(Value::as_array) {
        for alias in alias_values.iter().filter_map(Value::as_str) {
            let alias_norm = normalize_title(alias, true);
            update_best(&alias_norm);
        }
    }

    Ok(best)
}

pub fn title_match(
    correct_title: &str,
    parsed_title: &str,
    threshold: f64,
    aliases: &Map<String, Value>,
) -> Result<bool, RtnError> {
    Ok(get_lev_ratio(correct_title, parsed_title, threshold, aliases)? >= threshold)
}

fn settings_languages(settings: &Value, key: &str) -> HashSet<String> {
    settings
        .get("languages")
        .and_then(Value::as_object)
        .and_then(|langs| langs.get(key))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn settings_option_bool(settings: &Value, key: &str, default: bool) -> bool {
    settings
        .get("options")
        .and_then(Value::as_object)
        .and_then(|opts| opts.get(key))
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

fn settings_resolution(settings: &Value, key: &str) -> Option<bool> {
    let field = if key.ends_with('p') {
        format!("r{key}")
    } else {
        key.to_string()
    };
    settings
        .get("resolutions")
        .and_then(Value::as_object)
        .and_then(|res| res.get(&field))
        .and_then(Value::as_bool)
}

fn custom_rank_bool(
    settings: &Value,
    category: &str,
    key: &str,
    field: &str,
    default: bool,
) -> bool {
    settings
        .get("custom_ranks")
        .and_then(Value::as_object)
        .and_then(|obj| obj.get(category))
        .and_then(Value::as_object)
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_object)
        .and_then(|obj| obj.get(field))
        .and_then(Value::as_bool)
        .unwrap_or(default)
}

fn custom_rank_i64(settings: &Value, category: &str, key: &str, field: &str, default: i64) -> i64 {
    settings
        .get("custom_ranks")
        .and_then(Value::as_object)
        .and_then(|obj| obj.get(category))
        .and_then(Value::as_object)
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_object)
        .and_then(|obj| obj.get(field))
        .and_then(Value::as_i64)
        .unwrap_or(default)
}

fn expand_lang_set(langs: &mut HashSet<String>) {
    extend_lang_group(langs, "anime", ANIME_LANGS);
    extend_lang_group(langs, "non_anime", NON_ANIME_LANGS);
    extend_lang_group(langs, "common", COMMON_LANGS);
    if langs.contains("all") {
        langs.extend(
            ANIME_LANGS
                .iter()
                .chain(NON_ANIME_LANGS.iter())
                .map(|v| (*v).to_string()),
        );
    }
}

pub fn populate_lang_sets(settings: &Value) -> (HashSet<String>, HashSet<String>, HashSet<String>) {
    let mut exclude = settings_languages(settings, "exclude");
    let mut required = settings_languages(settings, "required");
    let mut allowed = settings_languages(settings, "allowed");

    expand_lang_set(&mut exclude);
    expand_lang_set(&mut required);
    expand_lang_set(&mut allowed);

    (exclude, required, allowed)
}

pub fn trash_handler(
    data: &Map<String, Value>,
    settings: &Value,
    failed_keys: &mut BTreeSet<String>,
) -> bool {
    if settings_option_bool(settings, "remove_all_trash", true) {
        if let Some(quality) = map_str(data, "quality")
            && TRASH_QUALITIES.contains(&quality)
        {
            failed_keys.insert("trash_quality".to_string());
            return true;
        }
        if map_array(data, "audio")
            .iter()
            .filter_map(Value::as_str)
            .any(|audio| audio == "HQ Clean Audio")
        {
            failed_keys.insert("trash_audio".to_string());
            return true;
        }
        if map_bool(data, "trash") {
            failed_keys.insert("trash_flag".to_string());
            return true;
        }
    }
    false
}

pub fn adult_handler(
    data: &Map<String, Value>,
    settings: &Value,
    failed_keys: &mut BTreeSet<String>,
) -> bool {
    if map_bool(data, "adult") && settings_option_bool(settings, "remove_adult_content", true) {
        failed_keys.insert("trash_adult".to_string());
        return true;
    }
    false
}

pub fn check_required(data: &Map<String, Value>, settings: &Value) -> Result<bool, RtnError> {
    let raw_title = map_str(data, "raw_title").unwrap_or_default();
    let required = settings_value_array(settings, "require");
    if required.is_empty() {
        return Ok(false);
    }
    check_pattern(required, raw_title)
}

pub fn check_exclude(
    data: &Map<String, Value>,
    settings: &Value,
    failed_keys: &mut BTreeSet<String>,
) -> Result<bool, RtnError> {
    let raw_title = map_str(data, "raw_title").unwrap_or_default();
    for pattern in settings_value_array(settings, "exclude") {
        if let Some(re) = compile_pattern(pattern)?
            && re.is_match(raw_title)?
        {
            let pat = pattern_text(pattern);
            failed_keys.insert(format!("exclude_regex '{pat}'"));
            return Ok(true);
        }
    }

    Ok(false)
}

pub fn language_handler(
    data: &Map<String, Value>,
    settings: &Value,
    failed_keys: &mut BTreeSet<String>,
) -> bool {
    let (exclude, required, allowed) = populate_lang_sets(settings);
    let langs = map_array(data, "languages");

    if langs.is_empty() {
        if settings_option_bool(settings, "remove_unknown_languages", false) {
            failed_keys.insert("unknown_language".to_string());
            return true;
        }
        if !required.is_empty() {
            failed_keys.insert("missing_required_language".to_string());
            return true;
        }
        return false;
    }

    if !required.is_empty()
        && !langs
            .iter()
            .filter_map(Value::as_str)
            .any(|lang| required.contains(lang))
    {
        failed_keys.insert("missing_required_language".to_string());
        return true;
    }

    if langs
        .iter()
        .filter_map(Value::as_str)
        .any(|lang| lang == "en")
        && settings_option_bool(settings, "allow_english_in_languages", false)
    {
        return false;
    }

    if !allowed.is_empty()
        && langs
            .iter()
            .filter_map(Value::as_str)
            .any(|lang| allowed.contains(lang))
    {
        return false;
    }

    let mut excluded = false;
    for lang in langs.iter().filter_map(Value::as_str) {
        if exclude.contains(lang) {
            failed_keys.insert(format!("lang_{lang}"));
            excluded = true;
        }
    }
    excluded
}

pub fn fetch_resolution(
    data: &Map<String, Value>,
    settings: &Value,
    failed_keys: &mut BTreeSet<String>,
) -> bool {
    let resolution = map_str(data, "resolution")
        .unwrap_or_default()
        .to_lowercase();

    match settings_resolution(settings, &resolution) {
        Some(enabled) => {
            if !enabled {
                failed_keys.insert("resolution".to_string());
                return true;
            }
        }
        None => {
            if !settings_resolution(settings, "unknown").unwrap_or(true) {
                failed_keys.insert("resolution_unknown".to_string());
                return true;
            }
        }
    }
    false
}

pub fn fetch_quality(
    data: &Map<String, Value>,
    settings: &Value,
    failed_keys: &mut BTreeSet<String>,
) -> bool {
    let Some(quality) = map_str(data, "quality") else {
        return false;
    };

    if let Some((category, key)) = quality_mapping(quality)
        && !custom_rank_bool(settings, category, key, "fetch", true)
    {
        failed_keys.insert(format!("{category}_{key}"));
        return true;
    }

    false
}

pub fn fetch_codec(
    data: &Map<String, Value>,
    settings: &Value,
    failed_keys: &mut BTreeSet<String>,
) -> bool {
    let Some(codec) = map_str(data, "codec") else {
        return false;
    };
    let Some(key) = codec_key(codec) else {
        return false;
    };
    if !custom_rank_bool(settings, "quality", key, "fetch", true) {
        failed_keys.insert(format!("codec_{key}"));
        return true;
    }
    false
}

pub fn fetch_audio(
    data: &Map<String, Value>,
    settings: &Value,
    failed_keys: &mut BTreeSet<String>,
) -> bool {
    for audio in map_array(data, "audio").iter().filter_map(Value::as_str) {
        let Some((category, key)) = audio_mapping(audio) else {
            continue;
        };
        if !custom_rank_bool(settings, category, key, "fetch", true) {
            failed_keys.insert(format!("{category}_{key}"));
            return true;
        }
    }
    false
}

pub fn fetch_hdr(
    data: &Map<String, Value>,
    settings: &Value,
    failed_keys: &mut BTreeSet<String>,
) -> bool {
    for hdr in map_array(data, "hdr").iter().filter_map(Value::as_str) {
        if let Some(key) = hdr_key(hdr)
            && !custom_rank_bool(settings, "hdr", key, "fetch", true)
        {
            failed_keys.insert(format!("hdr_{key}"));
            return true;
        }
    }

    false
}

pub fn fetch_other(
    data: &Map<String, Value>,
    settings: &Value,
    failed_keys: &mut BTreeSet<String>,
) -> bool {
    for &(attr, category, key) in EXTRA_RULES.iter().chain(EXTRA_FETCH_ONLY_RULES.iter()) {
        if value_is_active(data.get(attr))
            && !custom_rank_bool(settings, category, key, "fetch", true)
        {
            failed_keys.insert(format!("{category}_{key}"));
            return true;
        }
    }

    false
}

fn run_fetch_pipeline(
    data: &Map<String, Value>,
    settings: &Value,
    failed_keys: &mut BTreeSet<String>,
    speed_mode: bool,
) -> Result<Option<bool>, RtnError> {
    if trash_handler(data, settings, failed_keys) && speed_mode {
        return Ok(Some(false));
    }
    if adult_handler(data, settings, failed_keys) && speed_mode {
        return Ok(Some(false));
    }
    if check_required(data, settings)? && speed_mode {
        return Ok(Some(true));
    }
    if check_exclude(data, settings, failed_keys)? && speed_mode {
        return Ok(Some(false));
    }
    if language_handler(data, settings, failed_keys) && speed_mode {
        return Ok(Some(false));
    }
    if fetch_resolution(data, settings, failed_keys) && speed_mode {
        return Ok(Some(false));
    }
    if fetch_quality(data, settings, failed_keys) && speed_mode {
        return Ok(Some(false));
    }
    if fetch_audio(data, settings, failed_keys) && speed_mode {
        return Ok(Some(false));
    }
    if fetch_hdr(data, settings, failed_keys) && speed_mode {
        return Ok(Some(false));
    }
    if fetch_codec(data, settings, failed_keys) && speed_mode {
        return Ok(Some(false));
    }
    if fetch_other(data, settings, failed_keys) && speed_mode {
        return Ok(Some(false));
    }
    Ok(None)
}

pub fn check_fetch(
    data: &Map<String, Value>,
    settings: &Value,
    speed_mode: bool,
) -> Result<(bool, Vec<String>), RtnError> {
    let mut failed_keys = BTreeSet::new();

    if let Some(fetchable) = run_fetch_pipeline(data, settings, &mut failed_keys, speed_mode)? {
        if fetchable {
            return Ok((true, Vec::new()));
        }
        return Ok((false, failed_keys.into_iter().collect()));
    }

    if failed_keys.is_empty() {
        Ok((true, Vec::new()))
    } else {
        Ok((false, failed_keys.into_iter().collect()))
    }
}

fn rank_model_value(rank_model: &Value, key: &str) -> i64 {
    rank_model
        .as_object()
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_i64)
        .unwrap_or(0)
}

fn rank_or_custom(
    rank_model: &Value,
    settings: &Value,
    category: &str,
    key: &str,
    fallback_field: &str,
) -> i64 {
    let use_custom = custom_rank_bool(settings, category, key, "use_custom_rank", false);
    if use_custom {
        custom_rank_i64(settings, category, key, "rank", 0)
    } else {
        rank_model_value(rank_model, fallback_field)
    }
}

pub fn calculate_preferred(data: &Map<String, Value>, settings: &Value) -> Result<i64, RtnError> {
    let patterns = settings_value_array(settings, "preferred");
    if patterns.is_empty() {
        return Ok(0);
    }
    let raw_title = map_str(data, "raw_title").unwrap_or_default();
    Ok(if check_pattern(patterns, raw_title)? {
        10_000
    } else {
        0
    })
}

pub fn calculate_preferred_langs(data: &Map<String, Value>, settings: &Value) -> i64 {
    let preferred = settings_languages(settings, "preferred");
    if preferred.is_empty() {
        return 0;
    }
    if map_array(data, "languages")
        .iter()
        .filter_map(Value::as_str)
        .any(|lang| preferred.contains(lang))
    {
        10_000
    } else {
        0
    }
}

pub fn calculate_quality_rank(
    data: &Map<String, Value>,
    settings: &Value,
    rank_model: &Value,
) -> i64 {
    let Some(quality) = map_str(data, "quality") else {
        return 0;
    };

    if let Some((category, key)) = quality_mapping(quality) {
        rank_or_custom(rank_model, settings, category, key, key)
    } else if quality == "TVRip" {
        rank_or_custom(rank_model, settings, "rips", "tvrip", "tvrip")
    } else {
        0
    }
}

pub fn calculate_codec_rank(
    data: &Map<String, Value>,
    settings: &Value,
    rank_model: &Value,
) -> i64 {
    let Some(codec) = map_str(data, "codec") else {
        return 0;
    };
    if let Some(key) = codec_key(codec) {
        rank_or_custom(rank_model, settings, "quality", key, key)
    } else {
        0
    }
}

pub fn calculate_hdr_rank(data: &Map<String, Value>, settings: &Value, rank_model: &Value) -> i64 {
    let mut total = 0;

    for hdr in map_array(data, "hdr").iter().filter_map(Value::as_str) {
        if let Some(key) = hdr_key(hdr) {
            total += rank_or_custom(rank_model, settings, "hdr", key, key);
        }
    }

    if data.get("bit_depth").and_then(Value::as_str).is_some() {
        total += rank_or_custom(rank_model, settings, "hdr", "10bit", "bit_10");
    }

    total
}

pub fn calculate_audio_rank(
    data: &Map<String, Value>,
    settings: &Value,
    rank_model: &Value,
) -> i64 {
    let mut total = 0;
    for audio in map_array(data, "audio").iter().filter_map(Value::as_str) {
        if let Some((category, key)) = audio_mapping(audio) {
            total += rank_or_custom(rank_model, settings, category, key, key);
        }
    }
    total
}

pub fn calculate_channels_rank(
    data: &Map<String, Value>,
    settings: &Value,
    rank_model: &Value,
) -> i64 {
    let mut total = 0;
    for channel in map_array(data, "channels").iter().filter_map(Value::as_str) {
        total += match channel {
            "5.1" | "7.1" => rank_or_custom(rank_model, settings, "audio", "surround", "surround"),
            "stereo" | "2.0" => rank_or_custom(rank_model, settings, "audio", "stereo", "stereo"),
            "mono" => rank_or_custom(rank_model, settings, "audio", "mono", "mono"),
            _ => 0,
        };
    }
    total
}

pub fn calculate_extra_ranks(
    data: &Map<String, Value>,
    settings: &Value,
    rank_model: &Value,
) -> i64 {
    let has_core = data.get("bit_depth").and_then(Value::as_str).is_some()
        || !map_array(data, "hdr").is_empty()
        || !map_array(data, "seasons").is_empty()
        || !map_array(data, "episodes").is_empty();
    if !has_core {
        return 0;
    }

    let mut total = 0;
    for (attr, category, key) in EXTRA_RULES {
        if value_is_active(data.get(attr)) {
            total += rank_or_custom(rank_model, settings, category, key, key);
        }
    }

    if data.get("size").and_then(Value::as_str).is_some() {
        total += rank_or_custom(rank_model, settings, "trash", "size", "size");
    }

    total
}

pub fn get_rank(
    data: &Map<String, Value>,
    settings: &Value,
    rank_model: &Value,
) -> Result<i64, RtnError> {
    if map_str(data, "raw_title").unwrap_or_default().is_empty() {
        return Err(RtnError::InvalidInput(
            "Parsed data cannot be empty.".to_string(),
        ));
    }

    let mut rank = 0;
    rank += calculate_quality_rank(data, settings, rank_model);
    rank += calculate_hdr_rank(data, settings, rank_model);
    rank += calculate_channels_rank(data, settings, rank_model);
    rank += calculate_audio_rank(data, settings, rank_model);
    rank += calculate_codec_rank(data, settings, rank_model);
    rank += calculate_extra_ranks(data, settings, rank_model);
    rank += calculate_preferred(data, settings)?;
    rank += calculate_preferred_langs(data, settings);
    Ok(rank)
}

pub fn parse_json_object(raw: &str, field_name: &str) -> Result<Map<String, Value>, RtnError> {
    let value = parse_json_value(raw, field_name)?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| RtnError::InvalidInput(format!("Expected JSON object for {field_name}.")))
}

pub fn parse_json_value(raw: &str, field_name: &str) -> Result<Value, RtnError> {
    let value: Value = serde_json::from_str(raw)?;
    if value.is_null() {
        return Err(RtnError::InvalidInput(format!(
            "Expected JSON value for {field_name}."
        )));
    }
    Ok(value)
}

pub fn vec_i64_to_value(items: Vec<i64>) -> Value {
    Value::Array(
        items
            .into_iter()
            .map(|v| Value::Number(Number::from(v)))
            .collect(),
    )
}
