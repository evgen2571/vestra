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
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
    types::{PyAny, PyList, PyModule, PyTuple, PyType},
};
use video_editor::{
    BackendPreference as NativeBackendPreference, Category as NativeCategory,
    Diagnostic as NativeDiagnostic, Editor as NativeEditor, EditorError,
    InspectionReport as NativeInspection, LoadError, PreflightOptions as NativePreflightOptions,
    PreflightReport as NativePreflight, Project as NativeProject, Severity as NativeSeverity,
    ValidationReport as NativeValidation,
};

mod conversion;
mod prepared;
mod render;

create_exception!(
    video_editor._native,
    VideoEditorError,
    pyo3::exceptions::PyException
);
create_exception!(video_editor._native, ProjectError, VideoEditorError);
create_exception!(video_editor._native, PreparationError, VideoEditorError);
create_exception!(video_editor._native, FrameRenderError, VideoEditorError);
create_exception!(video_editor._native, RenderError, VideoEditorError);
create_exception!(video_editor._native, CancelledError, RenderError);
create_exception!(
    video_editor._native,
    PreparedProjectBusyError,
    VideoEditorError
);

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

#[pyclass(
    name = "Project",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone)]
struct PyProject {
    inner: NativeProject,
}

#[pymethods]
impl PyProject {
    #[classmethod]
    fn load(_cls: &Bound<'_, PyType>, py: Python<'_>, path: &Bound<'_, PyAny>) -> PyResult<Self> {
        let path = conversion::path_from_python(path)?;
        match py.detach(|| NativeProject::load(path)) {
            Ok(inner) => Ok(Self { inner }),
            Err(error) => Err(project_error(py, error)?),
        }
    }

    #[classmethod]
    #[pyo3(signature = (text, *, base_directory = None))]
    fn from_json(
        _cls: &Bound<'_, PyType>,
        py: Python<'_>,
        text: String,
        base_directory: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let base_directory = base_directory
            .map(conversion::path_from_python)
            .transpose()?
            .unwrap_or_else(|| PathBuf::from("."));
        match py.detach(|| NativeProject::from_json(&text, base_directory)) {
            Ok(inner) => Ok(Self { inner }),
            Err(error) => Err(project_error(py, error)?),
        }
    }

    #[classmethod]
    #[pyo3(signature = (data, *, base_directory = None))]
    fn from_dict(
        _cls: &Bound<'_, PyType>,
        py: Python<'_>,
        data: &Bound<'_, PyAny>,
        base_directory: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let value = conversion::mapping_value(data)?;
        let base_directory = base_directory
            .map(conversion::path_from_python)
            .transpose()?
            .unwrap_or_else(|| PathBuf::from("."));
        match py.detach(|| NativeProject::from_value(value, base_directory)) {
            Ok(inner) => Ok(Self { inner }),
            Err(error) => Err(project_error(py, error)?),
        }
    }

    fn to_json(&self, py: Python<'_>) -> PyResult<String> {
        py.detach(|| self.inner.to_json())
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = py
            .detach(|| self.inner.to_value())
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let object = pythonize::pythonize(py, &value)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        Ok(object.unbind())
    }

    fn save(&self, py: Python<'_>, path: &Bound<'_, PyAny>) -> PyResult<()> {
        let path = conversion::path_from_python(path)?;
        match py.detach(|| self.inner.save(path)) {
            Ok(()) => Ok(()),
            Err(error) => Err(project_error(py, error)?),
        }
    }

    #[getter]
    fn base_directory(&self) -> PathBuf {
        self.inner.base_directory().to_path_buf()
    }
    #[getter]
    fn source_path(&self) -> Option<PathBuf> {
        self.inner.source_path().map(ToOwned::to_owned)
    }
    fn __repr__(&self) -> String {
        format!(
            "Project(source_path={:?}, base_directory={:?})",
            self.inner.source_path(),
            self.inner.base_directory()
        )
    }
}

#[pyclass(
    name = "Editor",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone)]
struct PyEditor {
    inner: NativeEditor,
}

#[pymethods]
impl PyEditor {
    #[new]
    fn new() -> Self {
        Self {
            inner: NativeEditor::new(),
        }
    }
    fn validate(&self, py: Python<'_>, project: &PyProject) -> PyValidationReport {
        PyValidationReport::from(py.detach(|| self.inner.validate(&project.inner)))
    }
    fn preflight(
        &self,
        py: Python<'_>,
        project: &PyProject,
        options: &PyPreflightOptions,
    ) -> PyPreflightReport {
        PyPreflightReport::from(
            py.detach(|| self.inner.preflight(&project.inner, options.inner.clone())),
        )
    }
    #[pyo3(signature = (project, *, preview = false))]
    #[expect(
        clippy::result_large_err,
        reason = "the Python bridge preserves native editor diagnostics"
    )]
    fn inspect(
        &self,
        py: Python<'_>,
        project: &PyProject,
        preview: bool,
    ) -> PyResult<PyInspectionReport> {
        match py.detach(|| self.inner.inspect(&project.inner, preview)) {
            Ok(report) => Ok(PyInspectionReport::from(report)),
            Err(error) => Err(editor_error(py, error)?),
        }
    }

    #[pyo3(signature = (project, options = None))]
    #[expect(
        clippy::result_large_err,
        reason = "the Python bridge preserves native preparation diagnostics"
    )]
    fn prepare(
        &self,
        py: Python<'_>,
        project: &PyProject,
        options: Option<&prepared::PyPrepareOptions>,
    ) -> PyResult<prepared::PyPreparedProject> {
        let options =
            options.map_or_else(video_editor::PrepareOptions::default, |value| value.inner);
        match py.detach(|| {
            if !prepared::wait_for_test_barrier() {
                return Err(None);
            }
            self.inner.prepare(&project.inner, options).map_err(Some)
        }) {
            Ok(value) => Ok(prepared::PyPreparedProject::new(value)),
            Err(Some(error)) => prepared::preparation_error(py, error),
            Err(None) => Err(PyRuntimeError::new_err(
                "prepared test synchronization failed",
            )),
        }
    }

    #[pyo3(signature = (project, request, *, progress = None, cancellation = None))]
    fn render(
        &self,
        py: Python<'_>,
        project: &PyProject,
        request: &render::PyRenderRequest,
        progress: Option<Py<PyAny>>,
        cancellation: Option<&render::PyCancellationToken>,
    ) -> PyResult<render::PyRenderResult> {
        render::render_one_shot(
            py,
            &self.inner,
            &project.inner,
            request,
            progress,
            cancellation,
        )
    }
}

#[pyclass(
    name = "PreflightOptions",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone)]
struct PyPreflightOptions {
    inner: NativePreflightOptions,
    kind: String,
    backend: Option<PyBackendPreference>,
    output: Option<PathBuf>,
    overwrite: bool,
}

#[pymethods]
impl PyPreflightOptions {
    #[classmethod]
    fn for_validation(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: NativePreflightOptions::for_validation(),
            kind: "validation".to_owned(),
            backend: None,
            output: None,
            overwrite: false,
        }
    }
    #[classmethod]
    fn for_inspection(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: NativePreflightOptions::for_inspection(),
            kind: "inspection".to_owned(),
            backend: None,
            output: None,
            overwrite: false,
        }
    }
    #[classmethod]
    fn for_preparation(_cls: &Bound<'_, PyType>, backend: &PyBackendPreference) -> Self {
        Self {
            inner: NativePreflightOptions::for_preparation(backend.native()),
            kind: "preparation".to_owned(),
            backend: Some(*backend),
            output: None,
            overwrite: false,
        }
    }
    #[classmethod]
    #[pyo3(signature = (backend, output, *, overwrite = false))]
    fn for_render(
        _cls: &Bound<'_, PyType>,
        backend: &PyBackendPreference,
        output: &Bound<'_, PyAny>,
        overwrite: bool,
    ) -> PyResult<Self> {
        let output = conversion::path_from_python(output)?;
        Ok(Self {
            inner: NativePreflightOptions::for_render(
                backend.native(),
                Some(output.clone()),
                overwrite,
            ),
            kind: "render".to_owned(),
            backend: Some(*backend),
            output: Some(output),
            overwrite,
        })
    }
    #[getter]
    fn kind(&self) -> String {
        self.kind.clone()
    }
    #[getter]
    fn backend(&self) -> Option<PyBackendPreference> {
        self.backend
    }
    #[getter]
    fn output(&self) -> Option<PathBuf> {
        self.output.clone()
    }
    #[getter]
    fn overwrite(&self) -> bool {
        self.overwrite
    }
}

#[pyclass(
    name = "BackendPreference",
    frozen,
    eq,
    hash,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub(crate) enum PyBackendPreference {
    #[pyo3(name = "AUTO")]
    Auto,
    #[pyo3(name = "CPU")]
    Cpu,
    #[pyo3(name = "WGPU")]
    Wgpu,
}
impl PyBackendPreference {
    const fn native(self) -> NativeBackendPreference {
        match self {
            Self::Auto => NativeBackendPreference::Auto,
            Self::Cpu => NativeBackendPreference::Cpu,
            Self::Wgpu => NativeBackendPreference::Wgpu,
        }
    }
    const fn as_value(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cpu => "cpu",
            Self::Wgpu => "wgpu",
        }
    }
}
#[pymethods]
impl PyBackendPreference {
    #[getter]
    fn value(&self) -> &'static str {
        (*self).as_value()
    }
    fn __str__(&self) -> &'static str {
        (*self).as_value()
    }
}

#[pyclass(
    name = "Category",
    frozen,
    eq,
    hash,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
enum PyCategory {
    #[pyo3(name = "USAGE")]
    Usage,
    #[pyo3(name = "PROJECT")]
    Project,
    #[pyo3(name = "SEMANTIC")]
    Semantic,
    #[pyo3(name = "ASSET")]
    Asset,
    #[pyo3(name = "MEDIA")]
    Media,
    #[pyo3(name = "BACKEND")]
    Backend,
    #[pyo3(name = "RENDER")]
    Render,
    #[pyo3(name = "OUTPUT")]
    Output,
    #[pyo3(name = "CANCELLATION")]
    Cancellation,
    #[pyo3(name = "INTERNAL")]
    Internal,
}
impl From<NativeCategory> for PyCategory {
    fn from(value: NativeCategory) -> Self {
        match value {
            NativeCategory::Usage => Self::Usage,
            NativeCategory::Project => Self::Project,
            NativeCategory::Semantic => Self::Semantic,
            NativeCategory::Asset => Self::Asset,
            NativeCategory::Media => Self::Media,
            NativeCategory::Backend => Self::Backend,
            NativeCategory::Render => Self::Render,
            NativeCategory::Output => Self::Output,
            NativeCategory::Cancellation => Self::Cancellation,
            NativeCategory::Internal => Self::Internal,
        }
    }
}
#[pymethods]
impl PyCategory {
    #[getter]
    fn value(&self) -> &'static str {
        match self {
            Self::Usage => "usage",
            Self::Project => "project",
            Self::Semantic => "semantic",
            Self::Asset => "asset",
            Self::Media => "media",
            Self::Backend => "backend",
            Self::Render => "render",
            Self::Output => "output",
            Self::Cancellation => "cancellation",
            Self::Internal => "internal",
        }
    }
    fn __str__(&self) -> &'static str {
        self.value()
    }
}

#[pyclass(
    name = "Severity",
    frozen,
    eq,
    hash,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
enum PySeverity {
    #[pyo3(name = "FATAL")]
    Fatal,
    #[pyo3(name = "WARNING")]
    Warning,
}
impl From<NativeSeverity> for PySeverity {
    fn from(value: NativeSeverity) -> Self {
        match value {
            NativeSeverity::Fatal => Self::Fatal,
            NativeSeverity::Warning => Self::Warning,
        }
    }
}
#[pymethods]
impl PySeverity {
    #[getter]
    fn value(&self) -> &'static str {
        match self {
            Self::Fatal => "fatal",
            Self::Warning => "warning",
        }
    }
    fn __str__(&self) -> &'static str {
        self.value()
    }
}

#[pyclass(
    name = "Diagnostic",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone)]
pub(crate) struct PyDiagnostic {
    #[pyo3(get)]
    code: String,
    #[pyo3(get)]
    category: PyCategory,
    #[pyo3(get)]
    severity: PySeverity,
    #[pyo3(get)]
    message: String,
    #[pyo3(get)]
    pointer: Option<String>,
    #[pyo3(get)]
    related_id: Option<String>,
    #[pyo3(get)]
    hint: Option<String>,
}
impl From<NativeDiagnostic> for PyDiagnostic {
    fn from(value: NativeDiagnostic) -> Self {
        Self {
            code: value.code,
            category: value.category.into(),
            severity: value.severity.into(),
            message: value.message,
            pointer: value.pointer,
            related_id: value.related_id,
            hint: value.hint,
        }
    }
}
#[pymethods]
impl PyDiagnostic {
    fn __str__(&self) -> String {
        format!("{}: {}", self.code, self.message)
    }
    fn __repr__(&self) -> String {
        format!(
            "Diagnostic(code={:?}, severity={})",
            self.code,
            self.severity.value()
        )
    }
}

fn diagnostics(values: &[NativeDiagnostic]) -> Vec<PyDiagnostic> {
    values.iter().cloned().map(PyDiagnostic::from).collect()
}

pub(crate) fn diagnostic_tuple(py: Python<'_>, values: &[PyDiagnostic]) -> PyResult<Py<PyAny>> {
    let values = values
        .iter()
        .cloned()
        .map(|value| Py::new(py, value))
        .collect::<PyResult<Vec<_>>>()?;
    Ok(PyTuple::new(py, values)?.into_any().unbind())
}

#[pyclass(
    name = "ValidationReport",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
struct PyValidationReport {
    #[pyo3(get)]
    is_valid: bool,
    diagnostics: Vec<PyDiagnostic>,
    errors: Vec<PyDiagnostic>,
    warnings: Vec<PyDiagnostic>,
}
#[pymethods]
impl PyValidationReport {
    #[getter]
    fn diagnostics(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        diagnostic_tuple(py, &self.diagnostics)
    }
    #[getter]
    fn errors(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        diagnostic_tuple(py, &self.errors)
    }
    #[getter]
    fn warnings(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        diagnostic_tuple(py, &self.warnings)
    }
}
impl From<NativeValidation> for PyValidationReport {
    fn from(value: NativeValidation) -> Self {
        Self {
            is_valid: value.is_valid(),
            diagnostics: diagnostics(value.diagnostics()),
            errors: value.errors().cloned().map(PyDiagnostic::from).collect(),
            warnings: value.warnings().cloned().map(PyDiagnostic::from).collect(),
        }
    }
}

#[pyclass(
    name = "PreflightReport",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
struct PyPreflightReport {
    #[pyo3(get)]
    is_ready: bool,
    #[pyo3(get)]
    is_valid: bool,
    diagnostics: Vec<PyDiagnostic>,
    errors: Vec<PyDiagnostic>,
    warnings: Vec<PyDiagnostic>,
}
#[pymethods]
impl PyPreflightReport {
    #[getter]
    fn diagnostics(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        diagnostic_tuple(py, &self.diagnostics)
    }
    #[getter]
    fn errors(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        diagnostic_tuple(py, &self.errors)
    }
    #[getter]
    fn warnings(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        diagnostic_tuple(py, &self.warnings)
    }
}
impl From<NativePreflight> for PyPreflightReport {
    fn from(value: NativePreflight) -> Self {
        Self {
            is_ready: value.is_ready(),
            is_valid: value.is_valid(),
            diagnostics: diagnostics(value.diagnostics()),
            errors: value.errors().cloned().map(PyDiagnostic::from).collect(),
            warnings: value.warnings().cloned().map(PyDiagnostic::from).collect(),
        }
    }
}

#[pyclass(
    name = "InspectOutput",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone)]
struct PyInspectOutput {
    #[pyo3(get)]
    path: PathBuf,
    #[pyo3(get)]
    width: u32,
    #[pyo3(get)]
    height: u32,
    #[pyo3(get)]
    frame_rate: String,
    #[pyo3(get)]
    duration_mode: String,
    #[pyo3(get)]
    duration: f64,
    #[pyo3(get)]
    total_frames: u64,
    #[pyo3(get)]
    preview: bool,
}
#[pyclass(
    name = "InspectAssets",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone)]
struct PyInspectAssets {
    #[pyo3(get)]
    images: usize,
    #[pyo3(get)]
    audio: usize,
}
#[pyclass(
    name = "InspectAudio",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone)]
struct PyInspectAudio {
    #[pyo3(get)]
    track_count: usize,
    #[pyo3(get)]
    clip_count: usize,
    #[pyo3(get)]
    end: f64,
    #[pyo3(get)]
    tracks: Vec<PyInspectAudioTrack>,
}
#[pyclass(
    name = "InspectAudioTrack",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone)]
struct PyInspectAudioTrack {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    mute: bool,
    #[pyo3(get)]
    gain: f64,
    #[pyo3(get)]
    clips: Vec<PyInspectAudioClip>,
}
#[pyclass(
    name = "InspectAudioClip",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone)]
struct PyInspectAudioClip {
    #[pyo3(get)]
    id: String,
    #[pyo3(get)]
    asset: String,
    #[pyo3(get)]
    start: f64,
    #[pyo3(get)]
    end: f64,
    #[pyo3(get)]
    trim_start: f64,
    #[pyo3(get)]
    trim_end: f64,
    #[pyo3(get)]
    mute: bool,
    #[pyo3(get)]
    gain: f64,
    #[pyo3(get)]
    gain_automation: Vec<PyInspectAudioGainKeyframe>,
    #[pyo3(get)]
    fade_in: f64,
    #[pyo3(get)]
    fade_out: f64,
    #[pyo3(get)]
    fade_in_curve: String,
    #[pyo3(get)]
    fade_out_curve: String,
}
#[pyclass(
    name = "InspectAudioGainKeyframe",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone)]
struct PyInspectAudioGainKeyframe {
    #[pyo3(get)]
    time: f64,
    #[pyo3(get)]
    gain: f64,
    #[pyo3(get)]
    interpolation: String,
}
#[pyclass(
    name = "InspectionReport",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
struct PyInspectionReport {
    #[pyo3(get)]
    project: PathBuf,
    #[pyo3(get)]
    name: Option<String>,
    #[pyo3(get)]
    output: PyInspectOutput,
    #[pyo3(get)]
    assets: PyInspectAssets,
    #[pyo3(get)]
    visual_clips: usize,
    #[pyo3(get)]
    flashes: usize,
    #[pyo3(get)]
    transitions: usize,
    #[pyo3(get)]
    audio: Option<PyInspectAudio>,
    warnings: Vec<PyDiagnostic>,
}
impl From<NativeInspection> for PyInspectionReport {
    fn from(value: NativeInspection) -> Self {
        Self {
            project: value.project,
            name: value.name,
            output: PyInspectOutput {
                path: value.output.path,
                width: value.output.width,
                height: value.output.height,
                frame_rate: value.output.frame_rate,
                duration_mode: value.output.duration_mode,
                duration: value.output.duration,
                total_frames: value.output.total_frames,
                preview: value.output.preview,
            },
            assets: PyInspectAssets {
                images: value.assets.images,
                audio: value.assets.audio,
            },
            visual_clips: value.visual_clips,
            flashes: value.flashes,
            transitions: value.transitions,
            audio: value.audio.map(|a| PyInspectAudio {
                track_count: a.track_count,
                clip_count: a.clip_count,
                end: a.end,
                tracks: a
                    .tracks
                    .into_iter()
                    .map(|track| PyInspectAudioTrack {
                        id: track.id,
                        mute: track.mute,
                        gain: track.gain,
                        clips: track
                            .clips
                            .into_iter()
                            .map(|clip| PyInspectAudioClip {
                                id: clip.id,
                                asset: clip.asset,
                                start: clip.start,
                                end: clip.end,
                                trim_start: clip.trim_start,
                                trim_end: clip.trim_end,
                                mute: clip.mute,
                                gain: clip.gain,
                                gain_automation: clip
                                    .gain_automation
                                    .into_iter()
                                    .map(|keyframe| PyInspectAudioGainKeyframe {
                                        time: keyframe.time,
                                        gain: keyframe.gain,
                                        interpolation: match keyframe.interpolation {
                                            video_editor::AudioGainInterpolation::Linear => {
                                                "linear".to_owned()
                                            }
                                            video_editor::AudioGainInterpolation::Hold => {
                                                "hold".to_owned()
                                            }
                                        },
                                    })
                                    .collect(),
                                fade_in: clip.fade_in,
                                fade_out: clip.fade_out,
                                fade_in_curve: match clip.fade_in_curve {
                                    video_editor::AudioFadeCurve::Linear => "linear".to_owned(),
                                    video_editor::AudioFadeCurve::EqualPower => {
                                        "equal_power".to_owned()
                                    }
                                },
                                fade_out_curve: match clip.fade_out_curve {
                                    video_editor::AudioFadeCurve::Linear => "linear".to_owned(),
                                    video_editor::AudioFadeCurve::EqualPower => {
                                        "equal_power".to_owned()
                                    }
                                },
                            })
                            .collect(),
                    })
                    .collect(),
            }),
            warnings: diagnostics(&value.warnings),
        }
    }
}
#[pymethods]
impl PyInspectionReport {
    #[getter]
    fn warnings(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        diagnostic_tuple(py, &self.warnings)
    }
}

#[pyfunction]
fn native_version() -> &'static str {
    NativeEditor::new().version().editor_version
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
    module.add_class::<PyInspectAudioTrack>()?;
    module.add_class::<PyInspectAudioClip>()?;
    module.add_class::<PyInspectAudioGainKeyframe>()?;
    module.add_class::<PyInspectionReport>()?;
    prepared::register(module)?;
    render::register(module)?;
    module.add_function(wrap_pyfunction!(native_version, module)?)?;
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
            ],
        )?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{PyBackendPreference, PyCategory, PySeverity};
    use video_editor::{BackendPreference, Category, Severity};

    #[test]
    fn enum_mappings_keep_the_sdk_string_contract() {
        assert_eq!(PyBackendPreference::Auto.native(), BackendPreference::Auto);
        assert_eq!(PyBackendPreference::Wgpu.as_value(), "wgpu");
        assert_eq!(
            PyCategory::from(Category::Cancellation).value(),
            "cancellation"
        );
        assert_eq!(PySeverity::from(Severity::Warning).value(), "warning");
    }
}
