use std::{fs, path::Path};

use serde_json::Value;

use crate::{Category, Diagnostic};

use super::{FORMAT_VERSION, LoadError, Project, ValidatedProject, ValidationOptions, validation};

pub fn load_and_validate(
    path: &Path,
    options: &ValidationOptions,
) -> Result<ValidatedProject, LoadError> {
    let data = fs::read(path).map_err(|error| {
        LoadError::Diagnostics(vec![Diagnostic::error(
            "MVP-PROJECT-READ",
            Category::Project,
            format!("cannot read project: {error}"),
            "",
        )])
    })?;
    let value: Value = serde_json::from_slice(&data).map_err(|error| {
        LoadError::Diagnostics(vec![Diagnostic::error(
            "MVP-PROJECT-JSON",
            Category::Project,
            format!("malformed JSON: {error}"),
            "",
        )])
    })?;
    let version = value
        .get("format_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            LoadError::Diagnostics(vec![Diagnostic::error(
                "MVP-VERSION-MISSING",
                Category::Version,
                "format_version is required and must be an integer",
                "/format_version",
            )])
        })?;
    match version {
        version if version == u64::from(FORMAT_VERSION) => {
            let project: Project = serde_json::from_value(value).map_err(|error| {
                LoadError::Diagnostics(vec![Diagnostic::error(
                    "MVP-PROJECT-SHAPE",
                    Category::Project,
                    format!("project does not match version 1: {error}"),
                    "",
                )])
            })?;
            validation::validate(project, path, options)
        }
        version if version == u64::from(super::v2::FORMAT_VERSION) => {
            let project: super::v2::Project = serde_json::from_value(value).map_err(|error| {
                LoadError::Diagnostics(vec![Diagnostic::error(
                    "MVP-PROJECT-SHAPE",
                    Category::Project,
                    format!("project does not match version 2: {error}"),
                    "",
                )])
            })?;
            validation::validate_v2(project, path, options)
        }
        _ => Err(LoadError::Diagnostics(vec![Diagnostic::error(
            "MVP-VERSION-UNSUPPORTED",
            Category::Version,
            format!("unsupported format_version {version}; supported versions: 1, 2"),
            "/format_version",
        )])),
    }
}
