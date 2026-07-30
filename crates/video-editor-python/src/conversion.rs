use std::{collections::HashSet, path::PathBuf};

use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::{PyAny, PyFloat, PyInt, PyList, PyString, PyTuple},
};

pub(crate) fn path_from_python(value: &Bound<'_, PyAny>) -> PyResult<PathBuf> {
    let os = value.py().import("os")?;
    let path: String = os.call_method1("fspath", (value,))?.extract()?;
    Ok(PathBuf::from(path))
}

pub(crate) fn mapping_value(data: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    let mapping = data.py().import("collections.abc")?.getattr("Mapping")?;
    if !data.is_instance(&mapping)? {
        return Err(PyTypeError::new_err(
            "data must be a collections.abc.Mapping",
        ));
    }
    json_value(data, &mut HashSet::new())
}

fn json_value(
    value: &Bound<'_, PyAny>,
    ancestors: &mut HashSet<usize>,
) -> PyResult<serde_json::Value> {
    let py = value.py();
    if value.is_none() {
        return Ok(serde_json::Value::Null);
    }
    if let Ok(boolean) = value.extract::<bool>() {
        return Ok(serde_json::Value::Bool(boolean));
    }
    if let Ok(string) = value.cast::<PyString>() {
        return Ok(serde_json::Value::String(string.to_str()?.to_owned()));
    }
    if value.is_instance_of::<PyInt>() {
        if let Ok(integer) = value.extract::<i64>() {
            return Ok(serde_json::Value::Number(integer.into()));
        }
        if let Ok(integer) = value.extract::<u64>() {
            return Ok(serde_json::Value::Number(integer.into()));
        }
        return Err(PyValueError::new_err(
            "integer is outside the JSON 64-bit range",
        ));
    }
    if value.is_instance_of::<PyFloat>() {
        return serde_json::Number::from_f64(value.extract::<f64>()?)
            .map(serde_json::Value::Number)
            .ok_or_else(|| PyValueError::new_err("JSON numbers must be finite"));
    }
    let mapping = py.import("collections.abc")?.getattr("Mapping")?;
    if value.is_instance(&mapping)? {
        let identity = value.as_ptr() as usize;
        if !ancestors.insert(identity) {
            return Err(PyValueError::new_err(
                "recursive mappings are not supported",
            ));
        }
        let result = (|| {
            let mut object = serde_json::Map::new();
            for item in value.call_method0("items")?.try_iter()? {
                let item = item?;
                let pair: &Bound<'_, PyTuple> = item.cast()?;
                if pair.len() != 2 {
                    return Err(PyTypeError::new_err(
                        "mapping items must be key/value pairs",
                    ));
                }
                let key = pair
                    .get_item(0)?
                    .extract::<String>()
                    .map_err(|_| PyTypeError::new_err("JSON object keys must be strings"))?;
                object.insert(key, json_value(&pair.get_item(1)?, ancestors)?);
            }
            Ok(serde_json::Value::Object(object))
        })();
        ancestors.remove(&identity);
        return result;
    }
    if let Ok(sequence) = value.cast::<PyList>() {
        return json_array(sequence.iter(), value, ancestors);
    }
    if let Ok(sequence) = value.cast::<PyTuple>() {
        return json_array(sequence.iter(), value, ancestors);
    }
    Err(PyTypeError::new_err(format!(
        "unsupported JSON value of type {}",
        value.get_type().name()?
    )))
}

fn json_array<'py>(
    values: impl Iterator<Item = Bound<'py, PyAny>>,
    original: &Bound<'py, PyAny>,
    ancestors: &mut HashSet<usize>,
) -> PyResult<serde_json::Value> {
    let identity = original.as_ptr() as usize;
    if !ancestors.insert(identity) {
        return Err(PyValueError::new_err(
            "recursive sequences are not supported",
        ));
    }
    let result = values
        .map(|value| json_value(&value, ancestors))
        .collect::<PyResult<Vec<_>>>()
        .map(serde_json::Value::Array);
    ancestors.remove(&identity);
    result
}
