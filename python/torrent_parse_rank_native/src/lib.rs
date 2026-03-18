use std::collections::BTreeSet;

use ptt_core::{
    clean_title_native, languages_translation_table, parse_many, parse_title, translate_langs_codes,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use rtn_core::{
    RtnError, adult_handler, calculate_audio_rank, calculate_channels_rank, calculate_codec_rank,
    calculate_extra_ranks, calculate_hdr_rank, calculate_preferred, calculate_preferred_langs,
    calculate_quality_rank, check_exclude, check_fetch, check_required, episodes_from_season,
    extract_episodes, extract_seasons, fetch_audio, fetch_codec, fetch_hdr, fetch_other,
    fetch_quality, fetch_resolution, get_lev_ratio, get_rank, language_handler, normalize_title,
    parse, parse_json_object, parse_json_value, populate_lang_sets, title_match, trash_handler,
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
    value_to_py(py, &Value::Object(map.clone()))
}

fn parse_data_and_settings(
    data_json: &str,
    settings_json: &str,
) -> Result<(Map<String, Value>, Value), RtnError> {
    Ok((
        parse_json_object(data_json, "data_json")?,
        parse_json_value(settings_json, "settings_json")?,
    ))
}

fn to_py_value_error<E: std::fmt::Display>(err: E) -> PyErr {
    PyValueError::new_err(err.to_string())
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
    let rank_model =
        parse_json_value(rank_model_json, "rank_model_json").map_err(to_py_value_error)?;
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
    let parsed = parse_title(raw_title, translate_languages).map_err(to_py_value_error)?;
    value_to_py(py, &Value::Object(parsed))
}

#[pyfunction]
#[pyo3(signature = (titles, translate_languages=false))]
fn ptt_parse_many(
    py: Python<'_>,
    titles: Vec<String>,
    translate_languages: bool,
) -> PyResult<Py<PyAny>> {
    let refs: Vec<&str> = titles.iter().map(String::as_str).collect();
    let parsed = parse_many(refs, translate_languages).map_err(to_py_value_error)?;
    let list = PyList::empty(py);
    for item in parsed {
        list.append(value_to_py(py, &Value::Object(item))?)?;
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

#[pyfunction]
fn rtn_check_pattern(patterns_json: &str, raw_title: &str) -> PyResult<bool> {
    let patterns = parse_json_value(patterns_json, "patterns_json").map_err(to_py_value_error)?;
    let arr = patterns
        .as_array()
        .cloned()
        .ok_or_else(|| PyValueError::new_err("patterns_json must be a JSON array."))?;
    rtn_core::check_pattern(&arr, raw_title).map_err(to_py_value_error)
}

#[pyfunction]
#[pyo3(signature = (correct_title, parsed_title, threshold=0.85, aliases_json="{}"))]
fn rtn_get_lev_ratio(
    correct_title: &str,
    parsed_title: &str,
    threshold: f64,
    aliases_json: &str,
) -> PyResult<f64> {
    let aliases = parse_json_object(aliases_json, "aliases_json").map_err(to_py_value_error)?;
    get_lev_ratio(correct_title, parsed_title, threshold, &aliases).map_err(to_py_value_error)
}

#[pyfunction]
#[pyo3(signature = (correct_title, parsed_title, threshold=0.85, aliases_json="{}"))]
fn rtn_title_match(
    correct_title: &str,
    parsed_title: &str,
    threshold: f64,
    aliases_json: &str,
) -> PyResult<bool> {
    let aliases = parse_json_object(aliases_json, "aliases_json").map_err(to_py_value_error)?;
    title_match(correct_title, parsed_title, threshold, &aliases).map_err(to_py_value_error)
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
fn rtn_episodes_from_season(raw_title: &str, season_num: i64) -> PyResult<Vec<i64>> {
    episodes_from_season(raw_title, season_num).map_err(to_py_value_error)
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
    let settings = parse_json_value(settings_json, "settings_json").map_err(to_py_value_error)?;
    let (exclude, required, allowed) = populate_lang_sets(&settings);
    Ok((
        exclude.into_iter().collect(),
        required.into_iter().collect(),
        allowed.into_iter().collect(),
    ))
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
    m.add_function(wrap_pyfunction!(ptt_parse_many, m)?)?;
    m.add_function(wrap_pyfunction!(ptt_clean_title, m)?)?;
    m.add_function(wrap_pyfunction!(ptt_translate_langs, m)?)?;
    m.add_function(wrap_pyfunction!(ptt_languages_translation_table, m)?)?;

    m.add_function(wrap_pyfunction!(rtn_parse, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_normalize_title, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_check_pattern, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_get_lev_ratio, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_title_match, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_extract_seasons, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_extract_episodes, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_episodes_from_season, m)?)?;

    m.add_function(wrap_pyfunction!(rtn_check_fetch, m)?)?;
    m.add_function(wrap_pyfunction!(rtn_check_fetch_and_rank, m)?)?;
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
