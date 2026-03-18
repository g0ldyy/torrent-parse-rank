use std::collections::{BTreeSet, HashMap, HashSet};

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

fn map_strings(map: &Map<String, Value>, key: &str) -> Vec<String> {
    map.get(key)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn map_i64s(map: &Map<String, Value>, key: &str) -> Vec<i64> {
    map.get(key)
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default()
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
    if raw_title.is_empty() {
        return Err(RtnError::InvalidInput(
            "The input title must be a non-empty string.".to_string(),
        ));
    }

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
    if raw_title.is_empty() {
        return Err(RtnError::InvalidInput(
            "The input title must be a non-empty string.".to_string(),
        ));
    }
    let data = parse_title(raw_title, false)?;
    Ok(map_i64s(&data, "seasons"))
}

pub fn extract_episodes(raw_title: &str) -> Result<Vec<i64>, RtnError> {
    if raw_title.is_empty() {
        return Err(RtnError::InvalidInput(
            "The input title must be a non-empty string.".to_string(),
        ));
    }
    let data = parse_title(raw_title, false)?;
    Ok(map_i64s(&data, "episodes"))
}

pub fn episodes_from_season(raw_title: &str, season_num: i64) -> Result<Vec<i64>, RtnError> {
    if season_num <= 0 {
        return Err(RtnError::InvalidInput(
            "The season number must be a positive integer.".to_string(),
        ));
    }
    if raw_title.is_empty() {
        return Err(RtnError::InvalidInput(
            "The input title must be a non-empty string.".to_string(),
        ));
    }

    let data = parse_title(raw_title, false)?;
    let seasons = map_i64s(&data, "seasons");
    if !seasons.contains(&season_num) {
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

    let mut titles = vec![normalize_title(correct_title, true)];
    for alias_values in aliases.values() {
        if let Some(alias_list) = alias_values.as_array() {
            for alias in alias_list.iter().filter_map(Value::as_str) {
                titles.push(normalize_title(alias, true));
            }
        }
    }

    for title in titles {
        let score = normalized_levenshtein(&title, &parsed_norm);
        if score >= threshold && score > best {
            best = score;
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

pub fn populate_lang_sets(settings: &Value) -> (HashSet<String>, HashSet<String>, HashSet<String>) {
    let anime: HashSet<String> = ["ja", "zh", "ko"]
        .iter()
        .map(|v| (*v).to_string())
        .collect();
    let non_anime: HashSet<String> = [
        "de", "es", "hi", "ta", "ru", "ua", "th", "it", "ar", "pt", "fr", "pa", "mr", "gu", "te",
        "kn", "ml", "vi", "id", "tr", "he", "fa", "el", "lt", "lv", "et", "pl", "cs", "sk", "hu",
        "ro", "bg", "sr", "hr", "sl", "nl", "da", "fi", "sv", "no", "ms",
    ]
    .iter()
    .map(|v| (*v).to_string())
    .collect();
    let common: HashSet<String> = [
        "de", "es", "hi", "ta", "ru", "ua", "th", "it", "zh", "ar", "fr",
    ]
    .iter()
    .map(|v| (*v).to_string())
    .collect();
    let all: HashSet<String> = anime.union(&non_anime).cloned().collect();

    let mut exclude = settings_languages(settings, "exclude");
    let mut required = settings_languages(settings, "required");
    let mut allowed = settings_languages(settings, "allowed");

    let groups: HashMap<&str, &HashSet<String>> = [
        ("anime", &anime),
        ("non_anime", &non_anime),
        ("common", &common),
        ("all", &all),
    ]
    .into_iter()
    .collect();

    for (name, set) in groups {
        if exclude.contains(name) {
            exclude.extend(set.iter().cloned());
        }
        if required.contains(name) {
            required.extend(set.iter().cloned());
        }
        if allowed.contains(name) {
            allowed.extend(set.iter().cloned());
        }
    }

    (exclude, required, allowed)
}

pub fn trash_handler(
    data: &Map<String, Value>,
    settings: &Value,
    failed_keys: &mut BTreeSet<String>,
) -> bool {
    if settings_option_bool(settings, "remove_all_trash", true) {
        if let Some(quality) = map_str(data, "quality")
            && ["CAM", "PDTV", "R5", "SCR", "TeleCine", "TeleSync"].contains(&quality)
        {
            failed_keys.insert("trash_quality".to_string());
            return true;
        }
        if map_strings(data, "audio")
            .iter()
            .any(|a| a == "HQ Clean Audio")
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
    let required = settings
        .get("require")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if required.is_empty() {
        return Ok(false);
    }
    check_pattern(&required, raw_title)
}

pub fn check_exclude(
    data: &Map<String, Value>,
    settings: &Value,
    failed_keys: &mut BTreeSet<String>,
) -> Result<bool, RtnError> {
    let raw_title = map_str(data, "raw_title").unwrap_or_default();
    let exclude = settings
        .get("exclude")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for pattern in exclude {
        if let Some(re) = compile_pattern(&pattern)?
            && re.is_match(raw_title)?
        {
            let pat = match pattern {
                Value::String(s) => s,
                Value::Object(obj) => obj
                    .get("pattern")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                _ => String::new(),
            };
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
    let langs: Vec<String> = map_strings(data, "languages");

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

    if !required.is_empty() && !langs.iter().any(|lang| required.contains(lang)) {
        failed_keys.insert("missing_required_language".to_string());
        return true;
    }

    if langs.iter().any(|lang| lang == "en")
        && settings_option_bool(settings, "allow_english_in_languages", false)
    {
        return false;
    }

    if !allowed.is_empty() && langs.iter().any(|lang| allowed.contains(lang)) {
        return false;
    }

    let mut excluded = false;
    for lang in langs {
        if exclude.contains(&lang) {
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

    let mapped = match quality {
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
    };

    if let Some((category, key)) = mapped
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
    let key = codec.to_lowercase();
    if ["avc", "hevc", "av1", "xvid", "mpeg"].contains(&key.as_str())
        && !custom_rank_bool(settings, "quality", &key, "fetch", true)
    {
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
    let map: HashMap<&str, &str> = [
        ("AAC", "aac"),
        ("Atmos", "atmos"),
        ("Dolby Digital", "dolby_digital"),
        ("Dolby Digital Plus", "dolby_digital_plus"),
        ("DTS Lossy", "dts_lossy"),
        ("DTS Lossless", "dts_lossless"),
        ("FLAC", "flac"),
        ("MP3", "mp3"),
        ("TrueHD", "truehd"),
        ("HQ Clean Audio", "clean_audio"),
    ]
    .into_iter()
    .collect();

    for audio in map_strings(data, "audio") {
        let Some(key) = map.get(audio.as_str()) else {
            continue;
        };
        let (category, effective) = if audio == "HQ Clean Audio" {
            ("trash", *key)
        } else {
            ("audio", *key)
        };
        if !custom_rank_bool(settings, category, effective, "fetch", true) {
            failed_keys.insert(format!("{category}_{effective}"));
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
    let map: HashMap<&str, &str> = [
        ("DV", "dolby_vision"),
        ("HDR", "hdr"),
        ("HDR10+", "hdr10plus"),
        ("SDR", "sdr"),
    ]
    .into_iter()
    .collect();

    for hdr in map_strings(data, "hdr") {
        if let Some(key) = map.get(hdr.as_str())
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
    let map: [(&str, &str, &str); 17] = [
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
        ("size", "trash", "size"),
        ("bit_depth", "hdr", "10bit"),
        ("scene", "extras", "scene"),
        ("uncensored", "extras", "uncensored"),
    ];

    for (attr, category, key) in map {
        let active = match data.get(attr) {
            Some(Value::Bool(v)) => *v,
            Some(Value::String(s)) => !s.is_empty(),
            Some(Value::Array(arr)) => !arr.is_empty(),
            Some(Value::Number(_)) => true,
            Some(Value::Null) | None => false,
            Some(Value::Object(_)) => true,
        };
        if active && !custom_rank_bool(settings, category, key, "fetch", true) {
            failed_keys.insert(format!("{category}_{key}"));
            return true;
        }
    }

    false
}

pub fn check_fetch(
    data: &Map<String, Value>,
    settings: &Value,
    speed_mode: bool,
) -> Result<(bool, Vec<String>), RtnError> {
    let mut failed_keys = BTreeSet::new();

    if speed_mode {
        if trash_handler(data, settings, &mut failed_keys) {
            return Ok((false, failed_keys.into_iter().collect()));
        }
        if adult_handler(data, settings, &mut failed_keys) {
            return Ok((false, failed_keys.into_iter().collect()));
        }
        if check_required(data, settings)? {
            return Ok((true, Vec::new()));
        }
        if check_exclude(data, settings, &mut failed_keys)? {
            return Ok((false, failed_keys.into_iter().collect()));
        }
        if language_handler(data, settings, &mut failed_keys) {
            return Ok((false, failed_keys.into_iter().collect()));
        }
        if fetch_resolution(data, settings, &mut failed_keys) {
            return Ok((false, failed_keys.into_iter().collect()));
        }
        if fetch_quality(data, settings, &mut failed_keys) {
            return Ok((false, failed_keys.into_iter().collect()));
        }
        if fetch_audio(data, settings, &mut failed_keys) {
            return Ok((false, failed_keys.into_iter().collect()));
        }
        if fetch_hdr(data, settings, &mut failed_keys) {
            return Ok((false, failed_keys.into_iter().collect()));
        }
        if fetch_codec(data, settings, &mut failed_keys) {
            return Ok((false, failed_keys.into_iter().collect()));
        }
        if fetch_other(data, settings, &mut failed_keys) {
            return Ok((false, failed_keys.into_iter().collect()));
        }
    } else {
        trash_handler(data, settings, &mut failed_keys);
        adult_handler(data, settings, &mut failed_keys);
        let _ = check_required(data, settings)?;
        let _ = check_exclude(data, settings, &mut failed_keys)?;
        language_handler(data, settings, &mut failed_keys);
        fetch_resolution(data, settings, &mut failed_keys);
        fetch_quality(data, settings, &mut failed_keys);
        fetch_audio(data, settings, &mut failed_keys);
        fetch_hdr(data, settings, &mut failed_keys);
        fetch_codec(data, settings, &mut failed_keys);
        fetch_other(data, settings, &mut failed_keys);
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
    let patterns = settings
        .get("preferred")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if patterns.is_empty() {
        return Ok(0);
    }
    let raw_title = map_str(data, "raw_title").unwrap_or_default();
    Ok(if check_pattern(&patterns, raw_title)? {
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
    let langs = map_strings(data, "languages");
    if langs.iter().any(|lang| preferred.contains(lang)) {
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

    match quality {
        "WEB" => rank_or_custom(rank_model, settings, "quality", "web", "web"),
        "WEB-DL" => rank_or_custom(rank_model, settings, "quality", "webdl", "webdl"),
        "BluRay" => rank_or_custom(rank_model, settings, "quality", "bluray", "bluray"),
        "HDTV" => rank_or_custom(rank_model, settings, "quality", "hdtv", "hdtv"),
        "VHS" => rank_or_custom(rank_model, settings, "quality", "vhs", "vhs"),
        "WEBMux" => rank_or_custom(rank_model, settings, "quality", "webmux", "webmux"),
        "BluRay REMUX" | "REMUX" => {
            rank_or_custom(rank_model, settings, "quality", "remux", "remux")
        }
        "WEBRip" => rank_or_custom(rank_model, settings, "rips", "webrip", "webrip"),
        "WEB-DLRip" => rank_or_custom(rank_model, settings, "rips", "webdlrip", "webdlrip"),
        "UHDRip" => rank_or_custom(rank_model, settings, "rips", "uhdrip", "uhdrip"),
        "HDRip" => rank_or_custom(rank_model, settings, "rips", "hdrip", "hdrip"),
        "DVDRip" => rank_or_custom(rank_model, settings, "rips", "dvdrip", "dvdrip"),
        "BDRip" => rank_or_custom(rank_model, settings, "rips", "bdrip", "bdrip"),
        "BRRip" => rank_or_custom(rank_model, settings, "rips", "brrip", "brrip"),
        "VHSRip" => rank_or_custom(rank_model, settings, "rips", "vhsrip", "vhsrip"),
        "PPVRip" => rank_or_custom(rank_model, settings, "rips", "ppvrip", "ppvrip"),
        "SATRip" => rank_or_custom(rank_model, settings, "rips", "satrip", "satrip"),
        "TVRip" => rank_or_custom(rank_model, settings, "rips", "tvrip", "tvrip"),
        "TeleCine" => rank_or_custom(rank_model, settings, "trash", "telecine", "telecine"),
        "TeleSync" => rank_or_custom(rank_model, settings, "trash", "telesync", "telesync"),
        "SCR" => rank_or_custom(rank_model, settings, "trash", "screener", "screener"),
        "R5" => rank_or_custom(rank_model, settings, "trash", "r5", "r5"),
        "CAM" => rank_or_custom(rank_model, settings, "trash", "cam", "cam"),
        "PDTV" => rank_or_custom(rank_model, settings, "trash", "pdtv", "pdtv"),
        _ => 0,
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
    match codec.to_lowercase().as_str() {
        "avc" => rank_or_custom(rank_model, settings, "quality", "avc", "avc"),
        "hevc" => rank_or_custom(rank_model, settings, "quality", "hevc", "hevc"),
        "xvid" => rank_or_custom(rank_model, settings, "quality", "xvid", "xvid"),
        "av1" => rank_or_custom(rank_model, settings, "quality", "av1", "av1"),
        "mpeg" => rank_or_custom(rank_model, settings, "quality", "mpeg", "mpeg"),
        _ => 0,
    }
}

pub fn calculate_hdr_rank(data: &Map<String, Value>, settings: &Value, rank_model: &Value) -> i64 {
    let mut total = 0;

    for hdr in map_strings(data, "hdr") {
        total += match hdr.as_str() {
            "DV" => rank_or_custom(rank_model, settings, "hdr", "dolby_vision", "dolby_vision"),
            "HDR" => rank_or_custom(rank_model, settings, "hdr", "hdr", "hdr"),
            "HDR10+" => rank_or_custom(rank_model, settings, "hdr", "hdr10plus", "hdr10plus"),
            "SDR" => rank_or_custom(rank_model, settings, "hdr", "sdr", "sdr"),
            _ => 0,
        };
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
    for audio in map_strings(data, "audio") {
        total += match audio.as_str() {
            "AAC" => rank_or_custom(rank_model, settings, "audio", "aac", "aac"),
            "Atmos" => rank_or_custom(rank_model, settings, "audio", "atmos", "atmos"),
            "Dolby Digital" => rank_or_custom(
                rank_model,
                settings,
                "audio",
                "dolby_digital",
                "dolby_digital",
            ),
            "Dolby Digital Plus" => rank_or_custom(
                rank_model,
                settings,
                "audio",
                "dolby_digital_plus",
                "dolby_digital_plus",
            ),
            "DTS Lossy" => rank_or_custom(rank_model, settings, "audio", "dts_lossy", "dts_lossy"),
            "DTS Lossless" => rank_or_custom(
                rank_model,
                settings,
                "audio",
                "dts_lossless",
                "dts_lossless",
            ),
            "FLAC" => rank_or_custom(rank_model, settings, "audio", "flac", "flac"),
            "MP3" => rank_or_custom(rank_model, settings, "audio", "mp3", "mp3"),
            "TrueHD" => rank_or_custom(rank_model, settings, "audio", "truehd", "truehd"),
            "HQ Clean Audio" => {
                rank_or_custom(rank_model, settings, "trash", "clean_audio", "clean_audio")
            }
            _ => 0,
        };
    }
    total
}

pub fn calculate_channels_rank(
    data: &Map<String, Value>,
    settings: &Value,
    rank_model: &Value,
) -> i64 {
    let mut total = 0;
    for channel in map_strings(data, "channels") {
        total += match channel.as_str() {
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
        || !map_strings(data, "hdr").is_empty()
        || !map_i64s(data, "seasons").is_empty()
        || !map_i64s(data, "episodes").is_empty();
    if !has_core {
        return 0;
    }

    let mut total = 0;
    let checks: [(&str, &str, &str, &str); 15] = [
        ("_3d", "extras", "three_d", "three_d"),
        ("converted", "extras", "converted", "converted"),
        ("documentary", "extras", "documentary", "documentary"),
        ("dubbed", "extras", "dubbed", "dubbed"),
        ("edition", "extras", "edition", "edition"),
        ("hardcoded", "extras", "hardcoded", "hardcoded"),
        ("network", "extras", "network", "network"),
        ("proper", "extras", "proper", "proper"),
        ("repack", "extras", "repack", "repack"),
        ("retail", "extras", "retail", "retail"),
        ("subbed", "extras", "subbed", "subbed"),
        ("upscaled", "extras", "upscaled", "upscaled"),
        ("site", "extras", "site", "site"),
        ("scene", "extras", "scene", "scene"),
        ("uncensored", "extras", "uncensored", "uncensored"),
    ];

    for (attr, category, key, field) in checks {
        let active = match data.get(attr) {
            Some(Value::Bool(v)) => *v,
            Some(Value::String(s)) => !s.is_empty(),
            Some(Value::Array(arr)) => !arr.is_empty(),
            Some(Value::Null) | None => false,
            Some(_) => true,
        };
        if active {
            total += rank_or_custom(rank_model, settings, category, key, field);
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
    let value: Value = serde_json::from_str(raw)?;
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
