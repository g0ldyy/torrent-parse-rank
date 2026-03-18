use std::collections::{HashMap, HashSet};

use anyhow::Context;
use chrono::NaiveDate;
use fancy_regex::Regex;
use once_cell::sync::Lazy;
use pcre2::bytes::{Regex as PcreRegex, RegexBuilder as PcreRegexBuilder};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("regex error: {0}")]
    Regex(String),
    #[error("data error: {0}")]
    Data(String),
}

#[derive(Debug, Clone)]
struct MatchInfo {
    raw_match: String,
    match_index: usize,
}

#[derive(Debug, Clone)]
struct HandlerMatch {
    raw_match: String,
    match_index: usize,
    remove: bool,
    skip_from_title: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct HandlerTable {
    handlers: Vec<HandlerDefRaw>,
}

#[derive(Debug, Clone, Deserialize)]
struct HandlerDefRaw {
    name: String,
    kind: String,
    pattern: Option<String>,
    flags: Option<u32>,
    transform: String,
    #[serde(default)]
    function: Option<String>,
    options: HandlerOptionsRaw,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[allow(non_snake_case)]
struct HandlerOptionsRaw {
    skipIfAlreadyFound: Option<bool>,
    skipFromTitle: Option<bool>,
    skipIfFirst: Option<bool>,
    remove: Option<bool>,
}

#[derive(Debug, Clone, Default)]
struct HandlerOptions {
    skip_if_already_found: bool,
    skip_from_title: bool,
    skip_if_first: bool,
    remove: bool,
}

impl From<HandlerOptionsRaw> for HandlerOptions {
    fn from(v: HandlerOptionsRaw) -> Self {
        Self {
            skip_if_already_found: v.skipIfAlreadyFound.unwrap_or(true),
            skip_from_title: v.skipFromTitle.unwrap_or(false),
            skip_if_first: v.skipIfFirst.unwrap_or(false),
            remove: v.remove.unwrap_or(false),
        }
    }
}

#[derive(Debug, Clone)]
enum TransformSpec {
    None,
    Integer,
    FirstInteger,
    Boolean,
    Lowercase,
    Uppercase,
    Value(String),
    Date(Vec<String>),
    RangeFunc,
    RangeXOfYFunc,
    ArrayInteger,
    UniqConcatValue(String),
    TransformResolution,
}

#[derive(Debug, Clone)]
enum RuntimeHandlerKind {
    Regex(PcreRegex),
    Function(String),
}

#[derive(Debug, Clone)]
struct RuntimeHandler {
    name: String,
    kind: RuntimeHandlerKind,
    transform: TransformSpec,
    options: HandlerOptions,
}

#[derive(Debug, Clone)]
struct ParserEngine {
    handlers: Vec<RuntimeHandler>,
}

fn compile_regex(pattern: &str, ignore_case: bool) -> Result<PcreRegex, ParseError> {
    let normalized_pattern = normalize_pattern_for_pcre2(pattern);
    let mut builder = PcreRegexBuilder::new();
    builder.utf(true);
    builder.ucp(true);
    builder.caseless(ignore_case);
    match builder.build(&normalized_pattern) {
        Ok(re) => Ok(re),
        Err(first_err) => {
            if let Some(fallback) = simplify_pattern_for_pcre2(pattern) {
                let fallback = normalize_pattern_for_pcre2(&fallback);
                return builder.build(&fallback).map_err(|e| {
                    ParseError::Regex(format!(
                        "Error compiling regex: {e}; pattern={pattern}; fallback={fallback}"
                    ))
                });
            }
            Err(ParseError::Regex(format!(
                "Error compiling regex: {first_err}; pattern={pattern}"
            )))
        }
    }
}

fn normalize_pattern_for_pcre2(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek().copied() == Some('u') {
            chars.next(); // consume 'u'
            let mut hex = String::with_capacity(4);
            for _ in 0..4 {
                if let Some(h) = chars.peek().copied() {
                    if h.is_ascii_hexdigit() {
                        hex.push(h);
                        chars.next();
                    } else {
                        break;
                    }
                }
            }
            if hex.len() == 4 {
                out.push_str("\\x{");
                out.push_str(&hex);
                out.push('}');
                continue;
            }
            out.push('\\');
            out.push('u');
            out.push_str(&hex);
            continue;
        }
        out.push(c);
    }
    out
}

fn simplify_pattern_for_pcre2(pattern: &str) -> Option<String> {
    if pattern.contains("Featurettes?") {
        return Some(r"(?:\b(?:19\d{2}|20\d{2})\b.*\bFeaturettes?\b|\bFeaturettes?\b(?!.*\b(?:19\d{2}|20\d{2})\b))".to_owned());
    }
    if pattern.contains("(?:Sample)") {
        return Some(
            r"(?:\b(?:19\d{2}|20\d{2})\b.*\bSample\b|\bSample\b(?!.*\b(?:19\d{2}|20\d{2})\b))"
                .to_owned(),
        );
    }
    if pattern.contains("Trailers?") {
        return Some(r"(?:\b(?:19\d{2}|20\d{2})\b.*\bTrailers?\b|\bTrailers?\b(?!.*\b(?:19\d{2}|20\d{2}|.(Park|And))\b))".to_owned());
    }
    if pattern.contains("(?<!(?:seasons?|[Сс]езони?)\\W*)") {
        return Some(pattern.replace("(?<!(?:seasons?|[Сс]езони?)\\W*)", ""));
    }
    if pattern.contains("(?<=remux.*)") {
        return Some(r"\bBlu[ .-]*Ray\b(?=.*remux)".to_owned());
    }
    if pattern.contains("(?<!\\bEp?(?:isode)? ?\\d+\\b.*)") {
        return Some(pattern.replace("(?<!\\bEp?(?:isode)? ?\\d+\\b.*)", ""));
    }
    if pattern.contains("(?<=^\\[.+].+)") {
        return Some(pattern.replace("(?<=^\\[.+].+)", ""));
    }
    if pattern.contains("(?<=[ .,/-]+(?:[A-Z]{2}[ .,/-]+){2,})") {
        return Some(pattern.replace("(?<=[ .,/-]+(?:[A-Z]{2}[ .,/-]+){2,})", ""));
    }
    if pattern.contains("(?<=[ .,/-]+[A-Z]{2}[ .,/-]+)") {
        return Some(pattern.replace("(?<=[ .,/-]+[A-Z]{2}[ .,/-]+)", ""));
    }
    if pattern.contains("(?<!w{3}\\.\\w+\\.)") {
        return Some(pattern.replace("(?<!w{3}\\.\\w+\\.)", ""));
    }
    if pattern.contains("(?<!w{3}\\.\\w+\\.|Sci-)") {
        return Some(pattern.replace("(?<!w{3}\\.\\w+\\.|Sci-)", ""));
    }
    if pattern.contains("(?<=subs?\\([a-z,]+)") {
        return Some(pattern.replace("(?<=subs?\\([a-z,]+)", ""));
    }
    None
}

fn parse_quoted_arg(expr: &str, prefix: &str, suffix: &str) -> Option<String> {
    if !expr.starts_with(prefix) || !expr.ends_with(suffix) {
        return None;
    }
    let mut raw = expr[prefix.len()..expr.len() - suffix.len()]
        .trim()
        .to_owned();
    if raw.len() >= 2
        && ((raw.starts_with('\'') && raw.ends_with('\''))
            || (raw.starts_with('"') && raw.ends_with('"')))
    {
        raw = raw[1..raw.len() - 1].to_owned();
    }
    Some(raw)
}

fn parse_date_formats(expr: &str) -> Option<Vec<String>> {
    if !expr.starts_with("date(") || !expr.ends_with(')') {
        return None;
    }
    let inner = &expr[5..expr.len() - 1];
    let inner = inner.trim();
    if inner.starts_with('[') && inner.ends_with(']') {
        let inner = &inner[1..inner.len() - 1];
        let mut out = Vec::new();
        for part in inner.split(',') {
            let part = part.trim();
            let unquoted = if (part.starts_with('\'') && part.ends_with('\''))
                || (part.starts_with('"') && part.ends_with('"'))
            {
                part[1..part.len() - 1].to_owned()
            } else {
                part.to_owned()
            };
            if !unquoted.is_empty() {
                out.push(unquoted);
            }
        }
        return Some(out);
    }
    let fmt = parse_quoted_arg(expr, "date(", ")")?;
    Some(vec![fmt])
}

fn parse_transform(expr: &str) -> TransformSpec {
    let expr = expr.trim();
    match expr {
        "none" => TransformSpec::None,
        "integer" => TransformSpec::Integer,
        "first_integer" => TransformSpec::FirstInteger,
        "boolean" => TransformSpec::Boolean,
        "lowercase" => TransformSpec::Lowercase,
        "uppercase" => TransformSpec::Uppercase,
        "range_func" => TransformSpec::RangeFunc,
        "range_x_of_y_func" => TransformSpec::RangeXOfYFunc,
        "array(integer)" => TransformSpec::ArrayInteger,
        "transform_resolution" => TransformSpec::TransformResolution,
        _ => {
            if let Some(v) = parse_quoted_arg(expr, "value(", ")") {
                return TransformSpec::Value(v);
            }
            if let Some(v) = parse_quoted_arg(expr, "uniq_concat(value(", "))") {
                return TransformSpec::UniqConcatValue(v);
            }
            if let Some(v) = parse_date_formats(expr) {
                return TransformSpec::Date(v);
            }
            TransformSpec::None
        }
    }
}

impl ParserEngine {
    fn from_json(json_text: &str) -> Result<Self, ParseError> {
        let table: HandlerTable =
            serde_json::from_str(json_text).map_err(|e| ParseError::Data(e.to_string()))?;
        let mut handlers = Vec::with_capacity(table.handlers.len());
        for raw in table.handlers {
            let options: HandlerOptions = raw.options.into();
            let transform = parse_transform(&raw.transform);
            let kind = match raw.kind.as_str() {
                "regex" => {
                    let pat = raw
                        .pattern
                        .context("missing pattern")
                        .map_err(|e| ParseError::Data(e.to_string()))?;
                    let ignore_case = (raw.flags.unwrap_or(0) & 2) != 0;
                    RuntimeHandlerKind::Regex(compile_regex(&pat, ignore_case)?)
                }
                _ => RuntimeHandlerKind::Function(raw.function.unwrap_or_default()),
            };
            handlers.push(RuntimeHandler {
                name: raw.name,
                kind,
                transform,
                options,
            });
        }
        Ok(Self { handlers })
    }

    fn parse(
        &self,
        raw_title: &str,
        translate_languages: bool,
    ) -> Result<Map<String, Value>, ParseError> {
        let mut title = SUB_PATTERN.replace_all(raw_title, " ").to_string();
        let mut result = Map::new();
        let mut matched: HashMap<String, MatchInfo> = HashMap::new();
        let mut end_of_title = title.len();

        for handler in &self.handlers {
            let maybe_match = match &handler.kind {
                RuntimeHandlerKind::Regex(re) => {
                    self.apply_regex_handler(handler, re, &title, &mut result, &mut matched)?
                }
                RuntimeHandlerKind::Function(func_name) => self.apply_function_handler(
                    handler,
                    func_name,
                    &title,
                    &mut result,
                    &mut matched,
                )?,
            };

            let Some(match_result) = maybe_match else {
                continue;
            };

            if match_result.remove {
                let start = match_result.match_index.min(title.len());
                let end =
                    (match_result.match_index + match_result.raw_match.len()).min(title.len());
                if start <= end && title.is_char_boundary(start) && title.is_char_boundary(end) {
                    title.replace_range(start..end, "");
                }
            }
            if !match_result.skip_from_title
                && match_result.match_index > 1
                && match_result.match_index < end_of_title
            {
                end_of_title = match_result.match_index;
            }
            if match_result.remove
                && match_result.skip_from_title
                && match_result.match_index < end_of_title
            {
                end_of_title = end_of_title.saturating_sub(match_result.raw_match.len());
            }
        }

        result
            .entry("episodes".to_owned())
            .or_insert_with(|| json!([]));
        result
            .entry("seasons".to_owned())
            .or_insert_with(|| json!([]));
        result
            .entry("languages".to_owned())
            .or_insert_with(|| json!([]));

        post_process_result(raw_title, &mut result)?;

        if translate_languages && let Some(Value::Array(langs)) = result.get_mut("languages") {
            let mut translated = Vec::with_capacity(langs.len());
            for lang in langs.iter().filter_map(|v| v.as_str()) {
                if let Some(name) = LANGUAGES_TRANSLATION_TABLE.get(lang) {
                    translated.push(Value::String((*name).to_owned()));
                }
            }
            *langs = translated;
        }

        let end_of_title = end_of_title.min(title.len());
        let title_slice = if title.is_char_boundary(end_of_title) {
            &title[..end_of_title]
        } else {
            &title
        };
        result.insert("title".to_owned(), Value::String(clean_title(title_slice)));
        Ok(result)
    }

    fn apply_regex_handler(
        &self,
        handler: &RuntimeHandler,
        regex: &PcreRegex,
        title: &str,
        result: &mut Map<String, Value>,
        matched: &mut HashMap<String, MatchInfo>,
    ) -> Result<Option<HandlerMatch>, ParseError> {
        if result.contains_key(&handler.name) && handler.options.skip_if_already_found {
            return Ok(None);
        }

        let captures = regex
            .captures(title.as_bytes())
            .map_err(|e| ParseError::Regex(e.to_string()))?;
        let Some(captures) = captures else {
            return Ok(None);
        };
        let Some(m0) = captures.get(0) else {
            return Ok(None);
        };
        let raw_match = String::from_utf8_lossy(m0.as_bytes()).into_owned();
        let clean_match = captures
            .get(1)
            .map(|m| String::from_utf8_lossy(m.as_bytes()).into_owned())
            .unwrap_or_else(|| raw_match.clone());
        let existing = result.get(&handler.name);
        let transformed = transform_value(&handler.transform, &clean_match, existing)?;
        let Some(mut transformed) = transformed else {
            return Ok(None);
        };
        if let Value::String(s) = &mut transformed {
            *s = s.trim().to_owned();
        }

        let mut is_before_title = false;
        if let Some(caps) = BEFORE_TITLE_MATCH_REGEX
            .captures(title)
            .map_err(|e| ParseError::Regex(e.to_string()))?
            && let Some(g1) = caps.get(1)
        {
            is_before_title = g1.as_str().contains(&raw_match);
        }

        let mut has_other_match = false;
        let mut all_before_others = true;
        for (key, info) in matched.iter() {
            if key == &handler.name {
                continue;
            }
            has_other_match = true;
            if m0.start() >= info.match_index {
                all_before_others = false;
                break;
            }
        }
        let is_skip_if_first =
            handler.options.skip_if_first && has_other_match && all_before_others;

        if is_skip_if_first {
            return Ok(None);
        }

        matched
            .entry(handler.name.clone())
            .or_insert_with(|| MatchInfo {
                raw_match: raw_match.clone(),
                match_index: m0.start(),
            });
        result.insert(handler.name.clone(), transformed);
        Ok(Some(HandlerMatch {
            raw_match,
            match_index: m0.start(),
            remove: handler.options.remove,
            skip_from_title: is_before_title || handler.options.skip_from_title,
        }))
    }

    fn apply_function_handler(
        &self,
        _handler: &RuntimeHandler,
        function_name: &str,
        title: &str,
        result: &mut Map<String, Value>,
        matched: &mut HashMap<String, MatchInfo>,
    ) -> Result<Option<HandlerMatch>, ParseError> {
        match function_name {
            "is_adult_content" => {
                let already_adult = result
                    .get("adult")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if already_adult {
                    return Ok(None);
                }
                let lower = title.to_lowercase();
                if ADULT_KEYWORDS.iter().any(|kw| lower.contains(kw)) {
                    result.insert("adult".to_owned(), Value::Bool(true));
                }
                Ok(None)
            }
            "handle_bit_depth" => {
                if let Some(Value::String(s)) = result.get_mut("bit_depth") {
                    *s = s.replace([' ', '-'], "");
                }
                Ok(None)
            }
            "handle_space_in_codec" => {
                if let Some(Value::String(s)) = result.get_mut("codec") {
                    *s = s.replace([' ', '.', '-'], "");
                }
                Ok(None)
            }
            "handle_volumes" => {
                let start_index = matched.get("year").map(|m| m.match_index).unwrap_or(0);
                let slice = title.get(start_index..).unwrap_or(title);
                if let Some(caps) = HANDLE_VOLUMES_RE
                    .captures(slice)
                    .map_err(|e| ParseError::Regex(e.to_string()))?
                    && let Some(m0) = caps.get(0)
                {
                    let v = caps
                        .get(1)
                        .and_then(|m| m.as_str().parse::<i64>().ok())
                        .unwrap_or(0);
                    matched.insert(
                        "volumes".to_owned(),
                        MatchInfo {
                            raw_match: m0.as_str().to_owned(),
                            match_index: start_index + m0.start(),
                        },
                    );
                    result.insert(
                        "volumes".to_owned(),
                        Value::Array(vec![Value::Number(v.into())]),
                    );
                    return Ok(Some(HandlerMatch {
                        raw_match: m0.as_str().to_owned(),
                        match_index: start_index + m0.start(),
                        remove: true,
                        skip_from_title: false,
                    }));
                }
                Ok(None)
            }
            "handle_episodes" => {
                if result.contains_key("episodes") {
                    return Ok(None);
                }

                let mut start_indexes = Vec::new();
                if let Some(m) = matched.get("year")
                    && m.match_index != 0
                {
                    start_indexes.push(m.match_index);
                }
                if let Some(m) = matched.get("seasons")
                    && m.match_index != 0
                {
                    start_indexes.push(m.match_index);
                }
                let mut end_indexes = Vec::new();
                for key in ["resolution", "quality", "codec", "audio"] {
                    if let Some(m) = matched.get(key)
                        && m.match_index != 0
                    {
                        end_indexes.push(m.match_index);
                    }
                }
                let start_index = start_indexes.into_iter().min().unwrap_or(0);
                let end_index = end_indexes
                    .into_iter()
                    .chain(std::iter::once(title.len()))
                    .min()
                    .unwrap_or(title.len());
                let beginning_title = title.get(..end_index).unwrap_or(title);
                let middle_title = title.get(start_index..end_index).unwrap_or(beginning_title);

                let beginning_caps = HANDLE_EPISODES_BEGINNING_RE
                    .captures(beginning_title)
                    .map_err(|e| ParseError::Regex(e.to_string()))?;
                let (caps, from_beginning) = if let Some(c) = beginning_caps {
                    (c, true)
                } else if let Some(c) = HANDLE_EPISODES_MIDDLE_RE
                    .captures(middle_title)
                    .map_err(|e| ParseError::Regex(e.to_string()))?
                {
                    (c, false)
                } else {
                    return Ok(None);
                };

                if let Some(g1) = caps.get(1) {
                    let m0 = caps.get(0);
                    if from_beginning && let Some(m0) = m0 {
                        let match_start = m0.start();
                        let prefix = if match_start <= beginning_title.len() {
                            &beginning_title[..match_start]
                        } else {
                            beginning_title
                        };
                        let prefix_norm = prefix
                            .trim_end_matches(|c: char| !c.is_alphanumeric())
                            .to_lowercase();
                        if prefix_norm.ends_with("movie") || prefix_norm.ends_with("film") {
                            return Ok(None);
                        }
                    }

                    let episode_numbers: Vec<Value> = DIGITS_RE
                        .find_iter(g1.as_str())
                        .filter_map(Result::ok)
                        .filter_map(|m| m.as_str().parse::<i64>().ok())
                        .filter(|n| !matches!(n, 480 | 720 | 1080))
                        .map(|n| Value::Number(n.into()))
                        .collect();
                    if !episode_numbers.is_empty() {
                        result.insert("episodes".to_owned(), Value::Array(episode_numbers));
                        if let Some(m0) = m0 {
                            let idx = title.find(m0.as_str()).unwrap_or(0);
                            return Ok(Some(HandlerMatch {
                                raw_match: m0.as_str().to_owned(),
                                match_index: idx,
                                remove: false,
                                skip_from_title: false,
                            }));
                        }
                    }
                }
                Ok(None)
            }
            "handle_anime_eps" => {
                if !ANIME_SPECIAL_RE
                    .is_match(title)
                    .map_err(|e| ParseError::Regex(e.to_string()))?
                {
                    return Ok(None);
                }
                if result.contains_key("episodes") {
                    return Ok(None);
                }
                if let Some(m) = ANIME_EP_RE
                    .find(title)
                    .map_err(|e| ParseError::Regex(e.to_string()))?
                    && let Ok(n) = m.as_str().parse::<i64>()
                {
                    result.insert(
                        "episodes".to_owned(),
                        Value::Array(vec![Value::Number(n.into())]),
                    );
                    return Ok(Some(HandlerMatch {
                        raw_match: m.as_str().to_owned(),
                        match_index: m.start(),
                        remove: false,
                        skip_from_title: false,
                    }));
                }
                Ok(None)
            }
            "infer_language_based_on_naming" => {
                let has_pt_or_es = result
                    .get("languages")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(Value::as_str)
                            .any(|lang| lang == "pt" || lang == "es")
                    })
                    .unwrap_or(false);
                if !has_pt_or_es {
                    let mut should_add = false;
                    if let Some(ep_match) = matched.get("episodes") {
                        should_add = INFER_PT_EP_RE
                            .is_match(&ep_match.raw_match)
                            .map_err(|e| ParseError::Regex(e.to_string()))?;
                    }
                    if !should_add {
                        should_add = INFER_PT_TITLE_RE
                            .is_match(title)
                            .map_err(|e| ParseError::Regex(e.to_string()))?;
                    }
                    if should_add {
                        let mut langs: Vec<Value> = result
                            .get("languages")
                            .and_then(Value::as_array)
                            .cloned()
                            .unwrap_or_default();
                        langs.push(Value::String("pt".to_owned()));
                        result.insert("languages".to_owned(), Value::Array(langs));
                    }
                }
                Ok(None)
            }
            "handle_group" => {
                if let Some(group_match) = matched.get("group")
                    && group_match.raw_match.starts_with('[')
                    && group_match.raw_match.ends_with(']')
                {
                    let end_index = group_match.match_index + group_match.raw_match.len();
                    let overlap = matched
                        .iter()
                        .any(|(k, m)| k != "group" && m.match_index < end_index);
                    if overlap {
                        result.remove("group");
                    }
                }
                Ok(None)
            }
            "handle_group_exclusion" => {
                let remove = matches!(
                    result.get("group").and_then(Value::as_str),
                    Some("-") | Some("")
                );
                if remove {
                    result.remove("group");
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}

fn transform_value(
    spec: &TransformSpec,
    input: &str,
    existing: Option<&Value>,
) -> Result<Option<Value>, ParseError> {
    let v = match spec {
        TransformSpec::None => Some(Value::String(input.to_owned())),
        TransformSpec::Integer => integer_transform(input).map(|n| Value::Number(n.into())),
        TransformSpec::FirstInteger => {
            first_integer_transform(input).map(|n| Value::Number(n.into()))
        }
        TransformSpec::Boolean => Some(Value::Bool(true)),
        TransformSpec::Lowercase => Some(Value::String(input.to_lowercase())),
        TransformSpec::Uppercase => Some(Value::String(input.to_uppercase())),
        TransformSpec::Value(val) => {
            if val.contains("$1") {
                Some(Value::String(val.replace("$1", input)))
            } else {
                Some(Value::String(val.clone()))
            }
        }
        TransformSpec::RangeFunc => range_func(input).map(Value::Array),
        TransformSpec::RangeXOfYFunc => range_x_of_y_func(input).map(Value::Array),
        TransformSpec::ArrayInteger => {
            let item = integer_transform(input)
                .map(|n| Value::Number(n.into()))
                .unwrap_or(Value::Null);
            Some(Value::Array(vec![item]))
        }
        TransformSpec::UniqConcatValue(v) => {
            let mut existing_vec = existing
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let exists = existing_vec.iter().any(|e| e.as_str() == Some(v.as_str()));
            if !exists {
                existing_vec.push(Value::String(v.clone()));
            }
            Some(Value::Array(existing_vec))
        }
        TransformSpec::TransformResolution => Some(Value::String(transform_resolution(input))),
        TransformSpec::Date(formats) => parse_date_with_formats(input, formats).map(Value::String),
    };
    Ok(v)
}

fn integer_transform(input: &str) -> Option<i64> {
    let digits: String = input.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<i64>().ok()
}

fn first_integer_transform(input: &str) -> Option<i64> {
    DIGITS_RE
        .find(input)
        .ok()
        .flatten()
        .and_then(|m| m.as_str().parse::<i64>().ok())
}

fn range_func(input: &str) -> Option<Vec<Value>> {
    let numbers: Vec<i64> = DIGITS_RE
        .find_iter(input)
        .filter_map(Result::ok)
        .filter_map(|m| m.as_str().parse::<i64>().ok())
        .collect();

    if numbers.len() == 2 && numbers[0] < numbers[1] {
        return Some(
            (numbers[0]..=numbers[1])
                .map(|n| Value::Number(n.into()))
                .collect(),
        );
    }
    if numbers.len() > 2 && numbers.windows(2).all(|w| w[0] + 1 == w[1]) {
        return Some(
            numbers
                .into_iter()
                .map(|n| Value::Number(n.into()))
                .collect(),
        );
    }
    if numbers.len() == 1 {
        return Some(
            numbers
                .into_iter()
                .map(|n| Value::Number(n.into()))
                .collect(),
        );
    }
    None
}

fn site_tld_lang_code(tld: &str) -> Option<&'static str> {
    match tld {
        "nl" => Some("nl"),
        "fi" => Some("fi"),
        "se" => Some("sv"),
        "tel" => Some("te"),
        _ => None,
    }
}

fn post_process_result(raw_title: &str, result: &mut Map<String, Value>) -> Result<(), ParseError> {
    if let Some(Value::String(q)) = result.get("quality")
        && q == "REMUX"
        && BLURAY_HINT_RE
            .is_match(raw_title)
            .map_err(|e| ParseError::Regex(e.to_string()))?
    {
        result.insert(
            "quality".to_owned(),
            Value::String("BluRay REMUX".to_owned()),
        );
    }

    if MOVIE_EP_PREFIX_RE
        .is_match(raw_title)
        .map_err(|e| ParseError::Regex(e.to_string()))?
    {
        result.insert("episodes".to_owned(), Value::Array(vec![]));
    }

    let mut langs: Vec<String> = result
        .get("languages")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default();
    if langs.is_empty() {
        return Ok(());
    }

    if let Some(site) = result.get("site").and_then(Value::as_str) {
        let tld_raw = site
            .rsplit(['.', ' '])
            .next()
            .unwrap_or_default()
            .to_lowercase();
        let tld: String = tld_raw
            .chars()
            .filter(|c| c.is_ascii_alphabetic())
            .collect();
        if let Some(code) = site_tld_lang_code(&tld) {
            langs.retain(|l| l != code);
        }
    }

    // If `site` is not extracted, domain TLDs can still produce false language positives.
    for cap in RAW_SITE_TLD_RE
        .captures_iter(raw_title)
        .filter_map(Result::ok)
    {
        let Some(m) = cap.get(1) else { continue };
        let tld = m.as_str().to_lowercase();
        if let Some(code) = site_tld_lang_code(&tld) {
            langs.retain(|l| l != code);
        }
    }

    if langs.iter().any(|l| l == "de") {
        let has_de_hint = GERMAN_HINT_RE
            .is_match(raw_title)
            .map_err(|e| ParseError::Regex(e.to_string()))?;
        let has_de_code = DE_CODE_RE
            .is_match(raw_title)
            .map_err(|e| ParseError::Regex(e.to_string()))?;
        let code_count = UPPER_LANG_CODE_RE
            .find_iter(raw_title)
            .filter_map(Result::ok)
            .count();
        let keep_from_code_list = has_de_code && code_count >= 3;
        if !has_de_hint && !keep_from_code_list {
            langs.retain(|l| l != "de");
        }
    }

    if SCI_FI_RE
        .is_match(raw_title)
        .map_err(|e| ParseError::Regex(e.to_string()))?
    {
        langs.retain(|l| l != "fi");
    }

    // preserve insertion order while deduping
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for lang in langs {
        if seen.insert(lang.clone()) {
            deduped.push(Value::String(lang));
        }
    }
    result.insert("languages".to_owned(), Value::Array(deduped));
    Ok(())
}

fn range_x_of_y_func(input: &str) -> Option<Vec<Value>> {
    let numbers: Vec<i64> = DIGITS_RE
        .find_iter(input)
        .filter_map(Result::ok)
        .filter_map(|m| m.as_str().parse::<i64>().ok())
        .collect();
    if numbers.len() != 1 {
        return None;
    }
    Some((1..=numbers[0]).map(|n| Value::Number(n.into())).collect())
}

fn transform_resolution(input: &str) -> String {
    let lower = input.to_lowercase();
    if lower.contains("2160") || lower.contains("4k") {
        "2160p".to_owned()
    } else if lower.contains("1440") || lower.contains("2k") {
        "1440p".to_owned()
    } else if lower.contains("1080") {
        "1080p".to_owned()
    } else if lower.contains("720") {
        "720p".to_owned()
    } else if lower.contains("480") {
        "480p".to_owned()
    } else if lower.contains("360") {
        "360p".to_owned()
    } else {
        "240p".to_owned()
    }
}

fn parse_date_with_formats(input: &str, formats: &[String]) -> Option<String> {
    let sanitized = NON_WORD_RE.replace_all(input, " ").trim().to_owned();
    if sanitized.is_empty() {
        return None;
    }
    let normalized = convert_months(&sanitized);
    for fmt in formats {
        if let Some(date) = parse_date(&normalized, fmt) {
            return Some(date.format("%Y-%m-%d").to_string());
        }
    }
    None
}

fn parse_date(input: &str, fmt: &str) -> Option<NaiveDate> {
    let tokens: Vec<&str> = input.split_whitespace().collect();
    match fmt {
        "YYYYMMDD" => {
            let digits: String = input.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() != 8 {
                return None;
            }
            let y = digits[0..4].parse::<i32>().ok()?;
            let m = digits[4..6].parse::<u32>().ok()?;
            let d = digits[6..8].parse::<u32>().ok()?;
            NaiveDate::from_ymd_opt(y, m, d)
        }
        "YYYY MM DD" => {
            if tokens.len() < 3 {
                return None;
            }
            let y = tokens[0].parse::<i32>().ok()?;
            let m = tokens[1].parse::<u32>().ok()?;
            let d = tokens[2].parse::<u32>().ok()?;
            NaiveDate::from_ymd_opt(y, m, d)
        }
        "DD MM YYYY" => {
            if tokens.len() < 3 {
                return None;
            }
            let d = tokens[0].parse::<u32>().ok()?;
            let m = tokens[1].parse::<u32>().ok()?;
            let y = tokens[2].parse::<i32>().ok()?;
            NaiveDate::from_ymd_opt(y, m, d)
        }
        "MM DD YY" => {
            if tokens.len() < 3 {
                return None;
            }
            let m = tokens[0].parse::<u32>().ok()?;
            let d = tokens[1].parse::<u32>().ok()?;
            let y = parse_year_2(tokens[2])?;
            NaiveDate::from_ymd_opt(y, m, d)
        }
        "YY MM DD" => {
            if tokens.len() < 3 {
                return None;
            }
            let y = parse_year_2(tokens[0])?;
            let m = tokens[1].parse::<u32>().ok()?;
            let d = tokens[2].parse::<u32>().ok()?;
            NaiveDate::from_ymd_opt(y, m, d)
        }
        "DD MM YY" => {
            if tokens.len() < 3 {
                return None;
            }
            let d = tokens[0].parse::<u32>().ok()?;
            let m = tokens[1].parse::<u32>().ok()?;
            let y = parse_year_2(tokens[2])?;
            NaiveDate::from_ymd_opt(y, m, d)
        }
        "DD MMM YY" => {
            if tokens.len() < 3 {
                return None;
            }
            let d = parse_day(tokens[0])?;
            let m = month_to_num(tokens[1])?;
            let y = parse_year_2(tokens[2])?;
            NaiveDate::from_ymd_opt(y, m, d)
        }
        "DD MMM YYYY" | "Do MMM YYYY" | "Do MMMM YYYY" => {
            if tokens.len() < 3 {
                return None;
            }
            let d = parse_day(tokens[0])?;
            let m = month_to_num(tokens[1])?;
            let y = tokens[2].parse::<i32>().ok()?;
            NaiveDate::from_ymd_opt(y, m, d)
        }
        _ => None,
    }
}

fn parse_day(token: &str) -> Option<u32> {
    let cleaned = token.trim_end_matches(['s', 't', 'n', 'd', 'r', 'h']);
    cleaned.parse::<u32>().ok()
}

fn parse_year_2(token: &str) -> Option<i32> {
    let y = token.parse::<i32>().ok()?;
    if y < 100 {
        Some(if y <= 69 { 2000 + y } else { 1900 + y })
    } else {
        Some(y)
    }
}

fn month_to_num(token: &str) -> Option<u32> {
    let t = token.to_lowercase();
    if t.starts_with("jan") {
        Some(1)
    } else if t.starts_with("feb") {
        Some(2)
    } else if t.starts_with("mar") {
        Some(3)
    } else if t.starts_with("apr") {
        Some(4)
    } else if t.starts_with("may") {
        Some(5)
    } else if t.starts_with("jun") {
        Some(6)
    } else if t.starts_with("jul") {
        Some(7)
    } else if t.starts_with("aug") {
        Some(8)
    } else if t.starts_with("sep") {
        Some(9)
    } else if t.starts_with("oct") {
        Some(10)
    } else if t.starts_with("nov") {
        Some(11)
    } else if t.starts_with("dec") {
        Some(12)
    } else {
        None
    }
}

fn convert_months(input: &str) -> String {
    let mut out = input.to_owned();
    for (re, replacement) in MONTH_REGEXES.iter() {
        out = re.replace_all(&out, *replacement).to_string();
    }
    out
}

fn clean_title(raw_title: &str) -> String {
    let mut cleaned = raw_title.replace('_', " ");
    cleaned = MOVIE_REGEX.replace_all(&cleaned, "").to_string();
    cleaned = NOT_ALLOWED_SYMBOLS_AT_START_AND_END
        .replace_all(&cleaned, "")
        .to_string();
    cleaned = RUSSIAN_CAST_REGEX.replace_all(&cleaned, "").to_string();
    cleaned = STAR_REGEX_1.replace(&cleaned, "$1").to_string();
    cleaned = STAR_REGEX_2.replace(&cleaned, "$1").to_string();
    cleaned = ALT_TITLES_REGEX.replace_all(&cleaned, "").to_string();
    cleaned = NOT_ONLY_NON_ENGLISH_REGEX
        .replace_all(&cleaned, "")
        .to_string();
    cleaned = REMAINING_NOT_ALLOWED_SYMBOLS_AT_START_AND_END
        .replace_all(&cleaned, "")
        .to_string();
    cleaned = EMPTY_BRACKETS_REGEX.replace_all(&cleaned, "").to_string();
    cleaned = MP3_REGEX.replace_all(&cleaned, "").to_string();
    cleaned = PARANTHESES_WITHOUT_CONTENT
        .replace_all(&cleaned, "")
        .to_string();
    cleaned = SPECIAL_CHAR_SPACING.replace_all(&cleaned, "").to_string();

    for (open, close) in [("{", "}"), ("[", "]"), ("(", ")")] {
        if cleaned.matches(open).count() != cleaned.matches(close).count() {
            cleaned = cleaned.replace(open, "").replace(close, "");
        }
    }

    if !cleaned.contains(' ') && cleaned.contains('.') {
        cleaned = cleaned.replace('.', " ");
    }

    cleaned = normalize_mixed_script_title(cleaned);
    if !has_latin(&cleaned)
        && raw_title.contains('/')
        && has_latin(raw_title)
        && has_non_english(raw_title)
    {
        let recovered = recover_latin_from_slash(raw_title);
        if !recovered.is_empty() {
            cleaned = recovered;
        }
    }

    cleaned = REDUNDANT_SYMBOLS_AT_END
        .replace_all(&cleaned, "")
        .to_string();
    cleaned = SPACING_REGEX.replace_all(&cleaned, " ").to_string();
    cleaned.trim().to_owned()
}

fn normalize_mixed_script_title(mut title: String) -> String {
    if !has_latin(&title) || !has_non_english(&title) {
        return title;
    }

    // Prefer the Latin alternative when title is in the form "Non-Latin (Latin)".
    if let Some(caps) = LATIN_IN_PARENS_RE.captures(&title).ok().flatten()
        && let Some(inside) = caps.get(1).map(|m| m.as_str())
        && has_latin(inside)
        && !has_non_english(inside)
    {
        let before = title.split('(').next().unwrap_or_default();
        if has_non_english(before) && !has_latin(before) {
            title = inside.to_owned();
        }
    }

    // For mixed slash titles like "Русский / English", keep Latin segments.
    if title.contains('/') {
        let latin_parts: Vec<String> = title
            .split('/')
            .map(str::trim)
            .filter(|p| has_latin(p))
            .map(ToOwned::to_owned)
            .collect();
        if !latin_parts.is_empty() {
            title = latin_parts.join(" / ");
        } else if let Some(last) = title.split('/').map(str::trim).rfind(|p| !p.is_empty()) {
            title = last.to_owned();
        }
    }

    // Drop parenthetical metadata that still contains non-English text after selecting Latin title parts.
    title = strip_non_english_parens(&title);

    // Strip leading non-Latin prefix when title later contains Latin.
    if let Some((idx, _)) = title.char_indices().find(|(_, c)| c.is_ascii_alphabetic()) {
        let prefix = &title[..idx];
        if has_non_english(prefix) && !has_latin(prefix) {
            title = title[idx..]
                .trim_start_matches(|c: char| c.is_whitespace() || "-_[](){}./".contains(c))
                .to_owned();
        }
    }

    if !title.contains(' ') && title.contains('.') {
        title = title.replace('.', " ");
    }
    title
}

fn has_latin(s: &str) -> bool {
    let mut run = 0usize;
    for c in s.chars() {
        if c.is_ascii_alphabetic() {
            run += 1;
            if run >= 2 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

fn is_non_english_char(c: char) -> bool {
    matches!(c as u32, 0x3040..=0x30ff | 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff | 0xff66..=0xff9f | 0x0400..=0x04ff | 0x0600..=0x06ff | 0x0750..=0x077f | 0x0c80..=0x0cff | 0x0d00..=0x0d7f | 0x0e00..=0x0e7f)
}

fn has_non_english(s: &str) -> bool {
    s.chars().any(is_non_english_char)
}

fn recover_latin_from_slash(raw: &str) -> String {
    let after_first = raw
        .split_once('/')
        .map(|(_, rest)| rest.trim())
        .unwrap_or("");
    if after_first.is_empty() || !has_latin(after_first) {
        return String::new();
    }

    let mut candidate = strip_non_english_parens(after_first);
    candidate = PARENS_RE.replace_all(&candidate, "").to_string();
    let latin_parts: Vec<String> = candidate
        .split('/')
        .map(str::trim)
        .filter(|p| has_latin(p))
        .map(|p| {
            p.trim_matches(|c: char| "-_[](){}., ".contains(c))
                .to_owned()
        })
        .filter(|p| !p.is_empty())
        .collect();

    if latin_parts.is_empty() {
        return String::new();
    }

    let joined = latin_parts.join(" / ");
    let joined = if !joined.contains(' ') && joined.contains('.') {
        joined.replace('.', " ")
    } else {
        joined
    };
    SPACING_REGEX.replace_all(&joined, " ").trim().to_owned()
}

fn strip_non_english_parens(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let rest = &input[i..];
        if let Some(open_rel) = rest.find('(') {
            let open = i + open_rel;
            out.push_str(&input[i..open]);
            if let Some(close_rel) = input[open..].find(')') {
                let close = open + close_rel;
                let inner = &input[open + 1..close];
                if !(has_non_english(inner) && has_latin(input)) {
                    out.push_str(&input[open..=close]);
                }
                i = close + 1;
            } else {
                out.push_str(&input[open..]);
                break;
            }
        } else {
            out.push_str(rest);
            break;
        }
    }
    out
}

pub fn parse_title(
    raw_title: &str,
    translate_languages: bool,
) -> Result<Map<String, Value>, ParseError> {
    ENGINE.parse(raw_title, translate_languages)
}

pub fn parse_many<'a, I>(
    titles: I,
    translate_languages: bool,
) -> Result<Vec<Map<String, Value>>, ParseError>
where
    I: IntoIterator<Item = &'a str>,
{
    titles
        .into_iter()
        .map(|t| parse_title(t, translate_languages))
        .collect()
}

pub fn clean_title_native(raw_title: &str) -> String {
    clean_title(raw_title)
}

pub fn translate_langs_codes(langs: &[String]) -> Vec<String> {
    langs
        .iter()
        .filter_map(|lang| {
            LANGUAGES_TRANSLATION_TABLE
                .get(lang.as_str())
                .map(|value| (*value).to_owned())
        })
        .collect()
}

pub fn languages_translation_table() -> Vec<(String, String)> {
    LANGUAGES_TRANSLATION_TABLE
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

static HANDLERS_JSON: &str = include_str!("generated/handlers.json");
static ENGINE: Lazy<ParserEngine> =
    Lazy::new(|| ParserEngine::from_json(HANDLERS_JSON).expect("valid generated handler table"));

static SUB_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"_+").expect("valid regex"));
static BEFORE_TITLE_MATCH_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\[([^\[\]]+)\]").expect("valid regex"));
static DIGITS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\d+").expect("valid regex"));
static NON_WORD_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\W+").expect("valid regex"));
static LATIN_IN_PARENS_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\(([^()]*)\)").expect("valid regex"));
static PARENS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\([^()]*\)").expect("valid regex"));
static HANDLE_VOLUMES_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bvol(?:ume)?[. -]*(\d{1,2})").expect("valid regex"));
static HANDLE_EPISODES_BEGINNING_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(?:[ .]+-[ .]+|[\[(][ .]*)(\d{1,4})(?:a|b|v\d|\.\d)?(?:\W|$)(?!movie|film|\d+)",
    )
    .expect("valid regex")
});
static HANDLE_EPISODES_MIDDLE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)^(?:[\[(\-][ .]?)?(\d{1,4})(?:a|b|v\d)?(?:\W|$)(?!movie|film)(?!\[(480|720|1080)\])",
    )
    .expect("valid regex")
});
static ANIME_SPECIAL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"One.*?Piece|Bleach|Naruto").expect("valid regex"));
static ANIME_EP_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b\d{1,4}\b").expect("valid regex"));
static INFER_PT_EP_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)capitulo|ao").expect("valid regex"));
static INFER_PT_TITLE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)dublado").expect("valid regex"));

const NON_ENGLISH_CHARS: &str = "\u{3040}-\u{30ff}\u{3400}-\u{4dbf}\u{4e00}-\u{9fff}\u{f900}-\u{faff}\u{ff66}-\u{ff9f}\u{0400}-\u{04ff}\u{0600}-\u{06ff}\u{0750}-\u{077f}\u{0c80}-\u{0cff}\u{0d00}-\u{0d7f}\u{0e00}-\u{0e7f}";
static RUSSIAN_CAST_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\([^)]*[\u0400-\u04ff][^)]*\)$|/.*\(.*\)$").expect("valid regex"));
static ALT_TITLES_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(
        r"[^/|(]*[{NON_ENGLISH_CHARS}][^/|]*[/|]|[/|][^/|(]*[{NON_ENGLISH_CHARS}][^/|]*"
    ))
    .expect("valid regex")
});
static NOT_ONLY_NON_ENGLISH_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?!)").expect("valid regex"));
static NOT_ALLOWED_SYMBOLS_AT_START_AND_END: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(
        r"^[^\w{NON_ENGLISH_CHARS}#\[【★]+|[ \-:/\\\[\|{{(#$&^]+$"
    ))
    .expect("valid regex")
});
static REMAINING_NOT_ALLOWED_SYMBOLS_AT_START_AND_END: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!(r"^[^\w{NON_ENGLISH_CHARS}#]+|]$")).expect("valid regex"));
static REDUNDANT_SYMBOLS_AT_END: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[ \-:./\\]+$").expect("valid regex"));
static EMPTY_BRACKETS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\(\s*\)|\[\s*\]|\{\s*\}").expect("valid regex"));
static PARANTHESES_WITHOUT_CONTENT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\(\W*\)|\[\W*\]|\{\W*\}").expect("valid regex"));
static MOVIE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(?:\[|\()movie(?:\)|\])").expect("valid regex"));
static STAR_REGEX_1: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?:\[|【|★).*(?:\]|】|★)[ .]?(.+)").expect("valid regex"));
static STAR_REGEX_2: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(.+)[ .]?(?:\[|【|★).*(?:\]|】|★)$").expect("valid regex"));
static MP3_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bmp3$").expect("valid regex"));
static SPACING_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").expect("valid regex"));
static SPECIAL_CHAR_SPACING: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[\-\+\_\{\}\[\]]\W{2,}").expect("valid regex"));
static BLURAY_HINT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bblu[ .-]*ray\b").expect("valid regex"));
static MOVIE_EP_PREFIX_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(?:movie|film)\s*-\s*\d+\s*-").expect("valid regex"));
static SCI_FI_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bsci[ .-]?fi\b").expect("valid regex"));
static GERMAN_HINT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(?:GER|DEU|german|alem[aã]o|deutsch)\b|(?<=\.)de(?=\.)")
        .expect("valid regex")
});
static RAW_SITE_TLD_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\bwww\.[a-z0-9_-]+\.(nl|fi|se|tel)\b").expect("valid regex"));
static DE_CODE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\bDE\b").expect("valid regex"));
static UPPER_LANG_CODE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b[A-Z]{2,3}\b").expect("valid regex"));

const MONTH_MAPPING: &[(&str, &str)] = &[
    (r"\bJanu\b", "Jan"),
    (r"\bFebr\b", "Feb"),
    (r"\bMarc\b", "Mar"),
    (r"\bApri\b", "Apr"),
    (r"\bMay\b", "May"),
    (r"\bJune\b", "Jun"),
    (r"\bJuly\b", "Jul"),
    (r"\bAugu\b", "Aug"),
    (r"\bSept\b", "Sep"),
    (r"\bOcto\b", "Oct"),
    (r"\bNove\b", "Nov"),
    (r"\bDece\b", "Dec"),
];
static MONTH_REGEXES: Lazy<Vec<(Regex, &'static str)>> = Lazy::new(|| {
    MONTH_MAPPING
        .iter()
        .map(|(pat, replacement)| {
            (
                Regex::new(&format!("(?i){pat}")).expect("valid month regex"),
                *replacement,
            )
        })
        .collect()
});

static LANGUAGES_TRANSLATION_TABLE: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    HashMap::from([
        ("en", "English"),
        ("ja", "Japanese"),
        ("zh", "Chinese"),
        ("ru", "Russian"),
        ("ar", "Arabic"),
        ("pt", "Portuguese"),
        ("es", "Spanish"),
        ("fr", "French"),
        ("de", "German"),
        ("it", "Italian"),
        ("ko", "Korean"),
        ("hi", "Hindi"),
        ("bn", "Bengali"),
        ("pa", "Punjabi"),
        ("mr", "Marathi"),
        ("gu", "Gujarati"),
        ("ta", "Tamil"),
        ("te", "Telugu"),
        ("kn", "Kannada"),
        ("ml", "Malayalam"),
        ("th", "Thai"),
        ("vi", "Vietnamese"),
        ("id", "Indonesian"),
        ("tr", "Turkish"),
        ("he", "Hebrew"),
        ("fa", "Persian"),
        ("uk", "Ukrainian"),
        ("el", "Greek"),
        ("lt", "Lithuanian"),
        ("lv", "Latvian"),
        ("et", "Estonian"),
        ("pl", "Polish"),
        ("cs", "Czech"),
        ("sk", "Slovak"),
        ("hu", "Hungarian"),
        ("ro", "Romanian"),
        ("bg", "Bulgarian"),
        ("sr", "Serbian"),
        ("hr", "Croatian"),
        ("sl", "Slovenian"),
        ("nl", "Dutch"),
        ("da", "Danish"),
        ("fi", "Finnish"),
        ("sv", "Swedish"),
        ("no", "Norwegian"),
        ("ms", "Malay"),
        ("la", "Latino"),
    ])
});

static ADULT_KEYWORDS: Lazy<HashSet<String>> = Lazy::new(|| {
    include_str!("../data/combined-keywords.txt")
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
});
