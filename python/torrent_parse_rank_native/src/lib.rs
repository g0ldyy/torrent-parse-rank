use std::collections::BTreeSet;

use ptt_core::{
    clean_title_native, languages_translation_table, parse_many, parse_title, parse_title_context,
    translate_langs_codes,
};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyList};
use rtn_core::{
    CompiledPatterns, RtnError, adult_handler, calculate_audio_rank, calculate_channels_rank,
    calculate_codec_rank, calculate_extra_ranks, calculate_hdr_rank, calculate_preferred,
    calculate_preferred_langs, calculate_quality_rank, check_exclude, check_fetch,
    check_fetch_and_rank_many, check_required, episodes_from_season, extract_episodes,
    extract_seasons, fetch_audio, fetch_codec, fetch_hdr, fetch_other, fetch_quality,
    fetch_resolution, get_lev_ratio, get_rank, language_handler, normalize_title, parse,
    parse_json_object, parse_json_value, populate_lang_sets, title_match, trash_handler,
};
use serde_json::{Map, Value};

fn value_to_py(py: Python<'_>, value: &Value) -> PyResult<Py<PyAny>> {
    match value {
        Value::Null => Ok(py.None()),
        Value::Bool(b) => Ok((*b).into_pyobject(py)?.to_owned().into_any().unbind()),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.into_pyobject(py)?.into_any().unbind())
            } else if let Some(u) = n.as_u64() {
                Ok(u.into_pyobject(py)?.into_any().unbind())
            } else if let Some(f) = n.as_f64() {
                Ok(f.into_pyobject(py)?.into_any().unbind())
            } else {
                Ok(py.None())
            }
        }
        Value::String(s) => Ok(s.into_pyobject(py)?.into_any().unbind()),
        Value::Array(items) => {
            let list = PyList::empty(py);
            for item in items {
                list.append(value_to_py(py, item)?)?;
            }
            Ok(list.into_any().unbind())
        }
        Value::Object(map) => {
            let dict = PyDict::new(py);
            for (k, v) in map {
                dict.set_item(k, value_to_py(py, v)?)?;
            }
            Ok(dict.into_any().unbind())
        }
    }
}

fn map_to_py(py: Python<'_>, map: &Map<String, Value>) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    for (k, v) in map {
        dict.set_item(k, value_to_py(py, v)?)?;
    }
    Ok(dict.into_any().unbind())
}

fn parse_data_and_settings(
    data_json: &str,
    settings_json: &str,
) -> Result<(Map<String, Value>, Value), RtnError> {
    let data = parse_json_object(data_json, "data_json")?;
    validate_data_root(&data)?;
    let settings = Value::Object(parse_json_object(settings_json, "settings_json")?);
    Ok((data, settings))
}

fn validate_data_root(data: &Map<String, Value>) -> Result<(), RtnError> {
    if data
        .get("raw_title")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        return Err(RtnError::InvalidInput(
            "data_json.raw_title must be a non-empty string.".to_string(),
        ));
    }
    Ok(())
}

fn to_py_value_error<E: std::fmt::Display>(err: E) -> PyErr {
    PyValueError::new_err(err.to_string())
}

struct NonBooleanFloat(f64);

impl FromPyObject<'_, '_> for NonBooleanFloat {
    type Error = PyErr;

    fn extract(value: Borrowed<'_, '_, PyAny>) -> PyResult<Self> {
        if value.is_instance_of::<PyBool>() {
            return Err(PyTypeError::new_err("threshold must be a number, not bool"));
        }
        Ok(Self(value.extract()?))
    }
}

struct NonBooleanInteger(i64);

impl FromPyObject<'_, '_> for NonBooleanInteger {
    type Error = PyErr;

    fn extract(value: Borrowed<'_, '_, PyAny>) -> PyResult<Self> {
        if value.is_instance_of::<PyBool>() {
            return Err(PyTypeError::new_err(
                "season_num must be an integer, not bool",
            ));
        }
        Ok(Self(value.extract()?))
    }
}

fn parse_data_and_settings_py(
    data_json: &str,
    settings_json: &str,
) -> PyResult<(Map<String, Value>, Value)> {
    parse_data_and_settings(data_json, settings_json).map_err(to_py_value_error)
}

fn parse_data_settings_rank_py(
    data_json: &str,
    settings_json: &str,
    rank_model_json: &str,
) -> PyResult<(Map<String, Value>, Value, Value)> {
    let (data, settings) = parse_data_and_settings_py(data_json, settings_json)?;
    let rank_model = Value::Object(
        parse_json_object(rank_model_json, "rank_model_json").map_err(to_py_value_error)?,
    );
    Ok((data, settings, rank_model))
}

macro_rules! wrap_failed_bool_fn {
    ($name:ident, $core_fn:path) => {
        #[pyfunction]
        fn $name(data_json: &str, settings_json: &str) -> PyResult<(bool, Vec<String>)> {
            let (data, settings) = parse_data_and_settings_py(data_json, settings_json)?;
            let mut failed = BTreeSet::new();
            let res = $core_fn(&data, &settings, &mut failed);
            Ok((res, failed.into_iter().collect()))
        }
    };
}

macro_rules! wrap_failed_result_fn {
    ($name:ident, $core_fn:path) => {
        #[pyfunction]
        fn $name(data_json: &str, settings_json: &str) -> PyResult<(bool, Vec<String>)> {
            let (data, settings) = parse_data_and_settings_py(data_json, settings_json)?;
            let mut failed = BTreeSet::new();
            let res = $core_fn(&data, &settings, &mut failed).map_err(to_py_value_error)?;
            Ok((res, failed.into_iter().collect()))
        }
    };
}

macro_rules! wrap_rank_component_fn {
    ($name:ident, $core_fn:path) => {
        #[pyfunction]
        fn $name(data_json: &str, settings_json: &str, rank_model_json: &str) -> PyResult<i64> {
            let (data, settings, rank_model) =
                parse_data_settings_rank_py(data_json, settings_json, rank_model_json)?;
            Ok($core_fn(&data, &settings, &rank_model))
        }
    };
}

#[pyfunction]
#[pyo3(signature = (raw_title, translate_languages=false))]
fn ptt_parse_title(
    py: Python<'_>,
    raw_title: &str,
    translate_languages: bool,
) -> PyResult<Py<PyAny>> {
    if raw_title.is_empty() {
        return Err(PyValueError::new_err(
            "raw_title must be a non-empty string.",
        ));
    }
    let parsed = parse_title(raw_title, translate_languages).map_err(to_py_value_error)?;
    map_to_py(py, &parsed)
}

fn byte_to_char_index(text: &str, byte_index: usize) -> usize {
    let mut boundary = byte_index.min(text.len());
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    text[..boundary].chars().count()
}

#[pyfunction]
fn ptt_parse_title_context(py: Python<'_>, raw_title: &str) -> PyResult<Py<PyAny>> {
    if raw_title.is_empty() {
        return Err(PyValueError::new_err(
            "raw_title must be a non-empty string.",
        ));
    }
    let context = parse_title_context(raw_title, false).map_err(to_py_value_error)?;
    let output = PyDict::new(py);
    output.set_item("result", map_to_py(py, &context.result)?)?;
    output.set_item("working_title", &context.working_title)?;
    output.set_item(
        "end_of_title",
        byte_to_char_index(&context.working_title, context.end_of_title),
    )?;

    let matched = PyDict::new(py);
    for (name, info) in context.matched {
        let match_info = PyDict::new(py);
        match_info.set_item("raw_match", info.raw_match)?;
        match_info.set_item(
            "match_index",
            byte_to_char_index(&context.working_title, info.match_index),
        )?;
        matched.set_item(name, match_info)?;
    }
    output.set_item("matched", matched)?;
    Ok(output.into_any().unbind())
}

#[pyfunction]
#[pyo3(signature = (titles, translate_languages=false))]
fn ptt_parse_many(
    py: Python<'_>,
    titles: Vec<String>,
    translate_languages: bool,
) -> PyResult<Py<PyAny>> {
    if let Some(index) = titles.iter().position(String::is_empty) {
        return Err(PyValueError::new_err(format!(
            "titles[{index}] must be a non-empty string."
        )));
    }
    let refs: Vec<&str> = titles.iter().map(String::as_str).collect();
    let parsed = parse_many(refs, translate_languages).map_err(to_py_value_error)?;
    let list = PyList::empty(py);
    for item in parsed {
        list.append(map_to_py(py, &item)?)?;
    }
    Ok(list.into_any().unbind())
}

#[pyfunction]
fn ptt_clean_title(raw_title: &str) -> String {
    clean_title_native(raw_title)
}

#[pyfunction]
fn ptt_translate_langs(langs: Vec<String>) -> Vec<String> {
    translate_langs_codes(&langs)
}

#[pyfunction]
fn ptt_languages_translation_table(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    for (key, value) in languages_translation_table() {
        dict.set_item(key, value)?;
    }
    Ok(dict.into_any().unbind())
}

#[pyfunction]
#[pyo3(signature = (raw_title, translate_langs=false))]
fn rtn_parse(py: Python<'_>, raw_title: &str, translate_langs: bool) -> PyResult<Py<PyAny>> {
    let parsed = parse(raw_title, translate_langs).map_err(to_py_value_error)?;
    map_to_py(py, &parsed)
}

#[pyfunction]
#[pyo3(signature = (raw_title, lower=true))]
fn rtn_normalize_title(raw_title: &str, lower: bool) -> String {
    normalize_title(raw_title, lower)
}

#[pyclass(frozen, name = "_RtnPatternSet")]
struct RtnPatternSet {
    patterns: CompiledPatterns,
}

#[pymethods]
impl RtnPatternSet {
    #[new]
    fn new(patterns_json: &str) -> PyResult<Self> {
        let patterns =
            match parse_json_value(patterns_json, "patterns_json").map_err(to_py_value_error)? {
                Value::Array(patterns) => patterns,
                _ => return Err(PyValueError::new_err("patterns_json must be a JSON array.")),
            };
        Ok(Self {
            patterns: CompiledPatterns::new(&patterns),
        })
    }

    fn is_match(&self, raw_title: &str) -> PyResult<bool> {
        self.patterns.is_match(raw_title).map_err(to_py_value_error)
    }
}

#[pyfunction]
#[pyo3(
    signature = (correct_title, parsed_title, threshold=NonBooleanFloat(0.85), aliases_json="{}"),
    text_signature = "(correct_title, parsed_title, threshold=0.85, aliases_json='{}')"
)]
fn rtn_get_lev_ratio(
    correct_title: &str,
    parsed_title: &str,
    threshold: NonBooleanFloat,
    aliases_json: &str,
) -> PyResult<f64> {
    let aliases = parse_json_object(aliases_json, "aliases_json").map_err(to_py_value_error)?;
    get_lev_ratio(correct_title, parsed_title, threshold.0, &aliases).map_err(to_py_value_error)
}

#[pyfunction]
#[pyo3(
    signature = (correct_title, parsed_title, threshold=NonBooleanFloat(0.85), aliases_json="{}"),
    text_signature = "(correct_title, parsed_title, threshold=0.85, aliases_json='{}')"
)]
fn rtn_title_match(
    correct_title: &str,
    parsed_title: &str,
    threshold: NonBooleanFloat,
    aliases_json: &str,
) -> PyResult<bool> {
    let aliases = parse_json_object(aliases_json, "aliases_json").map_err(to_py_value_error)?;
    title_match(correct_title, parsed_title, threshold.0, &aliases).map_err(to_py_value_error)
}

#[pyfunction]
fn rtn_extract_seasons(raw_title: &str) -> PyResult<Vec<i64>> {
    extract_seasons(raw_title).map_err(to_py_value_error)
}

#[pyfunction]
fn rtn_extract_episodes(raw_title: &str) -> PyResult<Vec<i64>> {
    extract_episodes(raw_title).map_err(to_py_value_error)
}

#[pyfunction]
fn rtn_episodes_from_season(raw_title: &str, season_num: NonBooleanInteger) -> PyResult<Vec<i64>> {
    episodes_from_season(raw_title, season_num.0).map_err(to_py_value_error)
}

#[pyfunction]
#[pyo3(signature = (data_json, settings_json, speed_mode=true))]
fn rtn_check_fetch(
    data_json: &str,
    settings_json: &str,
    speed_mode: bool,
) -> PyResult<(bool, Vec<String>)> {
    let (data, settings) = parse_data_and_settings_py(data_json, settings_json)?;
    check_fetch(&data, &settings, speed_mode).map_err(to_py_value_error)
}

#[pyfunction]
#[pyo3(signature = (data_json, settings_json, rank_model_json, speed_mode=true))]
fn rtn_check_fetch_and_rank(
    data_json: &str,
    settings_json: &str,
    rank_model_json: &str,
    speed_mode: bool,
) -> PyResult<(bool, Vec<String>, i64)> {
    let (data, settings, rank_model) =
        parse_data_settings_rank_py(data_json, settings_json, rank_model_json)?;
    let (fetch, failed_keys) =
        check_fetch(&data, &settings, speed_mode).map_err(to_py_value_error)?;
    let rank = get_rank(&data, &settings, &rank_model).map_err(to_py_value_error)?;
    Ok((fetch, failed_keys, rank))
}

#[pyfunction]
#[pyo3(signature = (data_jsons, settings_json, rank_model_json, speed_mode=true))]
fn rtn_check_fetch_and_rank_many(
    data_jsons: Vec<String>,
    settings_json: &str,
    rank_model_json: &str,
    speed_mode: bool,
) -> PyResult<Vec<(bool, Vec<String>, i64)>> {
    let settings = Value::Object(
        parse_json_object(settings_json, "settings_json").map_err(to_py_value_error)?,
    );
    let rank_model = Value::Object(
        parse_json_object(rank_model_json, "rank_model_json").map_err(to_py_value_error)?,
    );

    let data_items = data_jsons
        .iter()
        .map(|data_json| parse_json_object(data_json, "data_json"))
        .collect::<Result<Vec<_>, _>>()
        .map_err(to_py_value_error)?;
    for data in &data_items {
        validate_data_root(data).map_err(to_py_value_error)?;
    }

    check_fetch_and_rank_many(&data_items, &settings, &rank_model, speed_mode)
        .map_err(to_py_value_error)
}

wrap_failed_bool_fn!(rtn_trash_handler, trash_handler);
wrap_failed_bool_fn!(rtn_adult_handler, adult_handler);
wrap_failed_bool_fn!(rtn_language_handler, language_handler);

#[pyfunction]
fn rtn_check_required(data_json: &str, settings_json: &str) -> PyResult<bool> {
    let (data, settings) = parse_data_and_settings_py(data_json, settings_json)?;
    check_required(&data, &settings).map_err(to_py_value_error)
}

wrap_failed_result_fn!(rtn_check_exclude, check_exclude);
wrap_failed_bool_fn!(rtn_fetch_resolution, fetch_resolution);
wrap_failed_bool_fn!(rtn_fetch_audio, fetch_audio);
wrap_failed_bool_fn!(rtn_fetch_quality, fetch_quality);
wrap_failed_bool_fn!(rtn_fetch_codec, fetch_codec);
wrap_failed_bool_fn!(rtn_fetch_hdr, fetch_hdr);
wrap_failed_bool_fn!(rtn_fetch_other, fetch_other);

#[pyfunction]
fn rtn_populate_langs(settings_json: &str) -> PyResult<(Vec<String>, Vec<String>, Vec<String>)> {
    let settings = Value::Object(
        parse_json_object(settings_json, "settings_json").map_err(to_py_value_error)?,
    );
    let (exclude, required, allowed) = populate_lang_sets(&settings);
    let mut exclude: Vec<_> = exclude.into_iter().collect();
    let mut required: Vec<_> = required.into_iter().collect();
    let mut allowed: Vec<_> = allowed.into_iter().collect();
    exclude.sort_unstable();
    required.sort_unstable();
    allowed.sort_unstable();
    Ok((exclude, required, allowed))
}

#[pyfunction]
fn rtn_get_rank(data_json: &str, settings_json: &str, rank_model_json: &str) -> PyResult<i64> {
    let (data, settings, rank_model) =
        parse_data_settings_rank_py(data_json, settings_json, rank_model_json)?;
    get_rank(&data, &settings, &rank_model).map_err(to_py_value_error)
}

#[pyfunction]
fn rtn_calculate_preferred(data_json: &str, settings_json: &str) -> PyResult<i64> {
    let (data, settings) = parse_data_and_settings_py(data_json, settings_json)?;
    calculate_preferred(&data, &settings).map_err(to_py_value_error)
}

wrap_rank_component_fn!(rtn_calculate_audio_rank, calculate_audio_rank);
wrap_rank_component_fn!(rtn_calculate_quality_rank, calculate_quality_rank);
wrap_rank_component_fn!(rtn_calculate_codec_rank, calculate_codec_rank);
wrap_rank_component_fn!(rtn_calculate_hdr_rank, calculate_hdr_rank);
wrap_rank_component_fn!(rtn_calculate_channels_rank, calculate_channels_rank);
wrap_rank_component_fn!(rtn_calculate_extra_ranks, calculate_extra_ranks);

#[pyfunction]
fn rtn_calculate_preferred_langs(data_json: &str, settings_json: &str) -> PyResult<i64> {
    let (data, settings) = parse_data_and_settings_py(data_json, settings_json)?;
    Ok(calculate_preferred_langs(&data, &settings))
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ptt_parse_title, m)?)?;
    m.add_function(wrap_pyfunction!(ptt_parse_title_context, m)?)?;
    m.add_function(wrap_pyfunction!(ptt_parse_many, m)?)?;
    m.add_function(wrap_pyfunction!(ptt_clean_title, m)?)?;
    m.add_function(wrap_pyfunction!(ptt_translate_langs, m)?)?;
    m.add_function(wrap_pyfunction!(ptt_languages_translation_table, m)?)?;

    m.add_function(wrap_pyfunction!(rtn_parse, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_normalize_title, m)?)?;
    m.add_class::<RtnPatternSet>()?;
    m.add_function(wrap_pyfunction!(rtn_get_lev_ratio, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_title_match, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_extract_seasons, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_extract_episodes, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_episodes_from_season, m)?)?;

    m.add_function(wrap_pyfunction!(rtn_check_fetch, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_check_fetch_and_rank, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_check_fetch_and_rank_many, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_trash_handler, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_adult_handler, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_language_handler, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_check_required, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_check_exclude, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_fetch_resolution, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_fetch_audio, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_fetch_quality, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_fetch_codec, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_fetch_hdr, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_fetch_other, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_populate_langs, m)?)?;

    m.add_function(wrap_pyfunction!(rtn_get_rank, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_calculate_preferred, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_calculate_audio_rank, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_calculate_quality_rank, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_calculate_codec_rank, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_calculate_hdr_rank, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_calculate_channels_rank, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_calculate_extra_ranks, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_calculate_preferred_langs, m)?)?;

    Ok(())
}
