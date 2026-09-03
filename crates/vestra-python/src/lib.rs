#![allow(
    clippy::missing_const_for_fn,
    reason = "PyO3 method signatures are clearer without const"
)]

use std::{
    path::PathBuf,
    sync::{Condvar, Mutex, OnceLock},
};

use pyo3::{
    create_exception,
    exceptions::{PyOSError, PyRuntimeError, PyValueError},
    prelude::*,
    types::{PyAny, PyList, PyModule},
};
use vestra::{Editor as NativeEditor, EditorError, LoadError};
use vestra_observability::{FileMode, LogFormat, LogOutput, ObservabilityConfig, Verbosity};

mod conversion;
mod diagnostics;
mod editor;
mod inspection;
mod prepared;
mod project;
mod render;

pub(crate) use diagnostics::{
    PyCategory, PyDiagnostic, PyPreflightReport, PySeverity, PyValidationReport, diagnostic_tuple,
    diagnostics,
};
pub(crate) use editor::{PyBackendPreference, PyEditor, PyPreflightOptions};
pub(crate) use inspection::{
    PyInspectAssets, PyInspectAudio, PyInspectAudioClip, PyInspectAudioEffect,
    PyInspectAudioGainKeyframe, PyInspectAudioTrack, PyInspectOutput, PyInspectionReport,
};
pub(crate) use project::PyProject;

create_exception!(
    vestra._native,
    VideoEditorError,
    pyo3::exceptions::PyException
);
create_exception!(vestra._native, ProjectError, VideoEditorError);
create_exception!(vestra._native, PreparationError, VideoEditorError);
create_exception!(vestra._native, FrameRenderError, VideoEditorError);
create_exception!(vestra._native, RenderError, VideoEditorError);
create_exception!(vestra._native, CancelledError, RenderError);
create_exception!(vestra._native, PreparedProjectBusyError, VideoEditorError);

fn attach_error_context(
    py: Python<'_>,
    error: &PyErr,
    kind: &str,
    diagnostics: &[PyDiagnostic],
    warnings: &[PyDiagnostic],
) -> PyResult<()> {
    let value = error.value(py);
    value.setattr("kind", kind)?;
    value.setattr("diagnostics", diagnostic_tuple(py, diagnostics)?)?;
    value.setattr("warnings", diagnostic_tuple(py, warnings)?)?;
    Ok(())
}

fn project_error(py: Python<'_>, error: LoadError) -> PyResult<PyErr> {
    let diagnostics = error
        .diagnostics()
        .iter()
        .cloned()
        .map(PyDiagnostic::from)
        .collect::<Vec<_>>();
    let message = diagnostics.first().map_or_else(
        || "project operation failed".to_owned(),
        |item| item.message.clone(),
    );
    let exception = PyErr::new::<ProjectError, _>(message);
    attach_error_context(py, &exception, "project", &diagnostics, &[])?;
    Ok(exception)
}

pub(crate) fn editor_error(py: Python<'_>, error: EditorError) -> PyResult<PyErr> {
    let error_diagnostics = diagnostics(error.diagnostics());
    let warnings = diagnostics(error.warnings());
    let message = error.to_string();
    let exception = PyErr::new::<VideoEditorError, _>(message);
    attach_error_context(
        py,
        &exception,
        error.kind().as_str(),
        &error_diagnostics,
        &warnings,
    )?;
    Ok(exception)
}

pub(crate) fn render_error(py: Python<'_>, error: EditorError) -> PyResult<PyErr> {
    let error_diagnostics = diagnostics(error.diagnostics());
    let warnings = diagnostics(error.warnings());
    let exception = if error.is_cancelled() {
        PyErr::new::<CancelledError, _>(error.to_string())
    } else {
        PyErr::new::<RenderError, _>(error.to_string())
    };
    attach_error_context(
        py,
        &exception,
        error.kind().as_str(),
        &error_diagnostics,
        &warnings,
    )?;
    let value = exception.value(py);
    value.setattr(
        "timings",
        Py::new(py, render::PyRenderTimings::from(error.timings()))?,
    )?;
    if let Some(context) = error.render_failure_context() {
        value.setattr(
            "failure_context",
            Py::new(py, render::PyRenderFailureContext::from(context))?,
        )?;
    } else {
        value.setattr("failure_context", py.None())?;
    }
    value.setattr("temporary_removed", error.temporary_output_removed())?;
    Ok(exception)
}

#[pyfunction]
fn native_version() -> &'static str {
    NativeEditor::new().version().editor_version
}

#[pyfunction]
fn video_duration(py: Python<'_>, path: &Bound<'_, PyAny>) -> PyResult<f64> {
    let path = conversion::path_from_python(path)?;
    py.detach(|| vestra::probe_video_duration(&path))
        .map_err(|error| PyValueError::new_err(error.to_string()))
}

#[pyfunction]
fn effect_definitions(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let value = serde_json::to_value(vestra::visual_effect_descriptors().collect::<Vec<_>>())
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    pythonize::pythonize(py, &value)
        .map(|value| value.unbind())
        .map_err(|error| PyValueError::new_err(error.to_string()))
}

#[pyfunction]
fn audio_effect_definitions(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let value = serde_json::to_value(vestra::audio_effect_descriptors().collect::<Vec<_>>())
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    pythonize::pythonize(py, &value)
        .map(|value| value.unbind())
        .map_err(|error| PyValueError::new_err(error.to_string()))
}

fn logging_level(value: &str) -> PyResult<Verbosity> {
    match value.to_ascii_lowercase().as_str() {
        "quiet" | "error" => Ok(Verbosity::Quiet),
        "normal" | "warn" | "warning" => Ok(Verbosity::Normal),
        "info" => Ok(Verbosity::Verbose),
        "debug" => Ok(Verbosity::Debug),
        "trace" => Ok(Verbosity::Trace),
        _ => Err(PyValueError::new_err(
            "level must be one of: error, warn, info, debug, trace",
        )),
    }
}

fn logging_format(value: &str) -> PyResult<LogFormat> {
    match value.to_ascii_lowercase().as_str() {
        "human" => Ok(LogFormat::Human),
        "json" => Ok(LogFormat::Json),
        _ => Err(PyValueError::new_err("format must be 'human' or 'json'")),
    }
}

fn logging_output(value: &str, file: Option<PathBuf>) -> PyResult<LogOutput> {
    match value.to_ascii_lowercase().as_str() {
        "stderr" if file.is_none() => Ok(LogOutput::Stderr),
        "stderr" if file.is_some() => Err(PyValueError::new_err(
            "output='stderr' does not accept a file path; use output='file' or 'stderr_and_file'",
        )),
        "file" => file.map_or_else(
            || {
                Err(PyValueError::new_err(
                    "a file path is required for the selected logging output",
                ))
            },
            |path| Ok(LogOutput::File(path)),
        ),
        "stderr_and_file" | "stderr+file" => file.map_or_else(
            || {
                Err(PyValueError::new_err(
                    "a file path is required for the selected logging output",
                ))
            },
            |path| Ok(LogOutput::StderrAndFile(path)),
        ),
        _ => Err(PyValueError::new_err(
            "output must be 'stderr', 'file', or 'stderr_and_file' (alias: 'stderr+file')",
        )),
    }
}

/// Installs the shared Rust tracing subscriber for an explicit Python process boundary.
#[pyfunction]
#[pyo3(signature = (level = "info", format = "human", output = "stderr", file = None, filter = None))]
fn configure_logging(
    level: &str,
    format: &str,
    output: &str,
    file: Option<&Bound<'_, PyAny>>,
    filter: Option<&str>,
) -> PyResult<()> {
    let verbosity = logging_level(level)?;
    let format = logging_format(format)?;
    let file = file.map(conversion::path_from_python).transpose()?;
    let output = logging_output(output, file)?;
    let mut config = ObservabilityConfig::with_verbosity(verbosity)
        .with_format(format)
        .with_output(output)
        .with_file_mode(FileMode::Append);
    if let Some(filter) = filter {
        config = config.with_filter(filter);
    }
    vestra_observability::try_init(config).map_err(|error| match error {
        vestra_observability::ObservabilityError::InvalidFilter { .. } => {
            PyValueError::new_err(error.to_string())
        }
        vestra_observability::ObservabilityError::OpenFile { .. } => {
            PyOSError::new_err(error.to_string())
        }
        vestra_observability::ObservabilityError::Install(_) => {
            PyRuntimeError::new_err(error.to_string())
        }
    })
}

#[derive(Default)]
struct DetachBarrier {
    entered: bool,
    released: bool,
}

fn detach_barrier() -> &'static (Mutex<DetachBarrier>, Condvar) {
    static BARRIER: OnceLock<(Mutex<DetachBarrier>, Condvar)> = OnceLock::new();
    BARRIER.get_or_init(|| (Mutex::new(DetachBarrier::default()), Condvar::new()))
}

/// Private debug-only test hook. It exercises the same `Python::detach` path
/// used by production operations without exposing a supported package API.
fn synchronization_error(operation: &str) -> PyErr {
    PyRuntimeError::new_err(format!("GIL test synchronization {operation} was poisoned"))
}

#[pyfunction]
fn _test_wait_while_detached(py: Python<'_>) -> PyResult<()> {
    py.detach(|| {
        let (lock, wake) = detach_barrier();
        let mut state = lock.lock().map_err(|_| synchronization_error("lock"))?;
        state.released = false;
        state.entered = true;
        wake.notify_all();
        while !state.released {
            state = wake
                .wait(state)
                .map_err(|_| synchronization_error("condition wait"))?;
        }
        state.entered = false;
        state.released = false;
        Ok(())
    })
}

#[pyfunction]
fn _test_wait_until_detached_entered(py: Python<'_>) -> PyResult<()> {
    py.detach(|| {
        let (lock, wake) = detach_barrier();
        let state = lock.lock().map_err(|_| synchronization_error("lock"))?;
        let _state = wake
            .wait_while(state, |state| !state.entered)
            .map_err(|_| synchronization_error("condition wait"))?;
        Ok(())
    })
}

#[pyfunction]
fn _test_release_detached_wait() -> PyResult<()> {
    let (lock, wake) = detach_barrier();
    let mut state = lock.lock().map_err(|_| synchronization_error("lock"))?;
    state.released = true;
    wake.notify_all();
    Ok(())
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add(
        "VideoEditorError",
        module.py().get_type::<VideoEditorError>(),
    )?;
    module.add("ProjectError", module.py().get_type::<ProjectError>())?;
    module.add(
        "PreparationError",
        module.py().get_type::<PreparationError>(),
    )?;
    module.add(
        "FrameRenderError",
        module.py().get_type::<FrameRenderError>(),
    )?;
    module.add("RenderError", module.py().get_type::<RenderError>())?;
    module.add("CancelledError", module.py().get_type::<CancelledError>())?;
    module.add(
        "PreparedProjectBusyError",
        module.py().get_type::<PreparedProjectBusyError>(),
    )?;
    module.add_class::<PyProject>()?;
    module.add_class::<PyEditor>()?;
    module.add_class::<PyPreflightOptions>()?;
    module.add_class::<PyBackendPreference>()?;
    module.add_class::<PyCategory>()?;
    module.add_class::<PySeverity>()?;
    module.add_class::<PyDiagnostic>()?;
    module.add_class::<PyValidationReport>()?;
    module.add_class::<PyPreflightReport>()?;
    module.add_class::<PyInspectOutput>()?;
    module.add_class::<PyInspectAssets>()?;
    module.add_class::<PyInspectAudio>()?;
    module.add_class::<PyInspectAudioEffect>()?;
    module.add_class::<PyInspectAudioTrack>()?;
    module.add_class::<PyInspectAudioClip>()?;
    module.add_class::<PyInspectAudioGainKeyframe>()?;
    module.add_class::<PyInspectionReport>()?;
    prepared::register(module)?;
    render::register(module)?;
    module.add_function(wrap_pyfunction!(native_version, module)?)?;
    module.add_function(wrap_pyfunction!(video_duration, module)?)?;
    module.add_function(wrap_pyfunction!(effect_definitions, module)?)?;
    module.add_function(wrap_pyfunction!(audio_effect_definitions, module)?)?;
    module.add_function(wrap_pyfunction!(configure_logging, module)?)?;
    module.add_function(wrap_pyfunction!(_test_wait_while_detached, module)?)?;
    module.add_function(wrap_pyfunction!(_test_wait_until_detached_entered, module)?)?;
    module.add_function(wrap_pyfunction!(_test_release_detached_wait, module)?)?;
    module.add(
        "__all__",
        PyList::new(
            module.py(),
            [
                "VideoEditorError",
                "ProjectError",
                "PreparationError",
                "FrameRenderError",
                "RenderError",
                "CancelledError",
                "PreparedProjectBusyError",
                "Project",
                "Editor",
                "PreflightOptions",
                "BackendPreference",
                "Category",
                "Severity",
                "Diagnostic",
                "ValidationReport",
                "PreflightReport",
                "InspectOutput",
                "InspectAssets",
                "InspectAudio",
                "InspectAudioTrack",
                "InspectAudioClip",
                "InspectAudioGainKeyframe",
                "InspectionReport",
                "PrepareOptions",
                "PreparedProject",
                "PreparationReport",
                "PreparationTimings",
                "FrameRate",
                "Frame",
                "PixelFormat",
                "BackendKind",
                "BackendFallback",
                "AdapterInfo",
                "AdapterDeviceType",
                "GraphicsBackend",
                "RenderRequest",
                "PreparedVideoRenderRequest",
                "CancellationToken",
                "RenderEvent",
                "RenderResult",
                "RenderTimingScope",
                "RenderTimings",
                "RenderPerformance",
                "RenderFailureContext",
                "RenderFailureStage",
                "native_version",
                "effect_definitions",
                "audio_effect_definitions",
                "configure_logging",
            ],
        )?,
    )?;
    Ok(())
}
