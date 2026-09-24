use std::{
    path::PathBuf,
    sync::{
        Condvar, Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

static NATIVE_RENDER_INVOCATIONS: AtomicUsize = AtomicUsize::new(0);
static CALLBACK_PYTHON_ATTACHMENTS: AtomicUsize = AtomicUsize::new(0);

use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyModule};
use vestra::{
    CancellationToken as NativeCancellationToken, Editor as NativeEditor, EditorError,
    PreparedVideoRenderRequest as NativePreparedRequest, Project as NativeProject,
    RenderEvent as NativeRenderEvent, RenderFailureContext as NativeRenderFailureContext,
    RenderFailureStage as NativeRenderFailureStage, RenderObserverControl,
    RenderPerformance as NativeRenderPerformance, RenderRequest as NativeRenderRequest,
    RenderResult as NativeRenderResult, RenderTimingScope as NativeRenderTimingScope,
    RenderTimings as NativeRenderTimings,
};

use crate::{
    PyBackendPreference, PyDiagnostic, conversion, diagnostic_tuple, diagnostics, render_error,
};

/// Validates callback input while Python is still attached, before a prepared
/// slot is acquired or a one-shot operation starts preparing native state.
pub(crate) fn validate_progress(
    py: Python<'_>,
    on_progress: Option<Py<PyAny>>,
) -> PyResult<Option<Py<PyAny>>> {
    if let Some(callback) = on_progress.as_ref()
        && !callback.bind(py).is_callable()
    {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "on_progress must be callable or None",
        ));
    }
    Ok(on_progress)
}

#[pyclass(
    name = "CancellationToken",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone)]
pub(crate) struct PyCancellationToken {
    pub(crate) inner: NativeCancellationToken,
}

#[pymethods]
impl PyCancellationToken {
    #[new]
    fn new() -> Self {
        Self {
            inner: NativeCancellationToken::new(),
        }
    }

    fn cancel(&self, py: Python<'_>) {
        py.detach(|| self.inner.cancel());
    }

    #[getter]
    fn is_cancelled(&self, py: Python<'_>) -> bool {
        py.detach(|| self.inner.is_cancelled())
    }
}

#[pyclass(
    name = "PreparedVideoRenderRequest",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
pub(crate) struct PyPreparedVideoRenderRequest {
    inner: NativePreparedRequest,
    output: PathBuf,
    overwrite: bool,
}

#[pymethods]
impl PyPreparedVideoRenderRequest {
    #[new]
    #[pyo3(signature = (output, *, overwrite = false))]
    fn new(output: &Bound<'_, PyAny>, overwrite: bool) -> PyResult<Self> {
        let output = conversion::path_from_python(output)?;
        Ok(Self {
            inner: NativePreparedRequest::new(output.clone()).with_overwrite(overwrite),
            output,
            overwrite,
        })
    }

    #[getter]
    fn output(&self) -> PathBuf {
        self.output.clone()
    }
    #[getter]
    fn overwrite(&self) -> bool {
        self.overwrite
    }
}

#[pyclass(
    name = "RenderRequest",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
pub(crate) struct PyRenderRequest {
    pub(crate) inner: NativeRenderRequest,
    output: PathBuf,
    backend: PyBackendPreference,
    overwrite: bool,
    preview: bool,
}

#[pymethods]
impl PyRenderRequest {
    #[new]
    #[pyo3(signature = (output, *, backend = None, overwrite = false, preview = false))]
    fn new(
        output: &Bound<'_, PyAny>,
        backend: Option<&PyBackendPreference>,
        overwrite: bool,
        preview: bool,
    ) -> PyResult<Self> {
        let output = conversion::path_from_python(output)?;
        let backend = backend.copied().unwrap_or(PyBackendPreference::Auto);
        Ok(Self {
            inner: NativeRenderRequest {
                output: Some(output.clone()),
                overwrite,
                preview,
                backend: backend.native(),
                progress_mode: vestra::ProgressMode::Auto,
            },
            output,
            backend,
            overwrite,
            preview,
        })
    }

    #[getter]
    fn output(&self) -> PathBuf {
        self.output.clone()
    }
    #[getter]
    fn backend(&self) -> PyBackendPreference {
        self.backend
    }
    #[getter]
    fn overwrite(&self) -> bool {
        self.overwrite
    }
    #[getter]
    fn preview(&self) -> bool {
        self.preview
    }
}

#[pyclass(
    name = "RenderTimingScope",
    frozen,
    eq,
    hash,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PyRenderTimingScope {
    #[pyo3(name = "ONE_SHOT")]
    OneShot,
    #[pyo3(name = "PREPARED_OPERATION")]
    PreparedOperation,
}
impl From<NativeRenderTimingScope> for PyRenderTimingScope {
    fn from(value: NativeRenderTimingScope) -> Self {
        match value {
            NativeRenderTimingScope::OneShot => Self::OneShot,
            NativeRenderTimingScope::PreparedOperation => Self::PreparedOperation,
        }
    }
}
#[pymethods]
impl PyRenderTimingScope {
    #[getter]
    fn value(&self) -> &'static str {
        match self {
            Self::OneShot => "one_shot",
            Self::PreparedOperation => "prepared_operation",
        }
    }
    fn __str__(&self) -> &'static str {
        self.value()
    }
}

#[pyclass(
    name = "RenderFailureStage",
    frozen,
    eq,
    hash,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PyRenderFailureStage {
    #[pyo3(name = "OUTPUT_PREPARATION")]
    OutputPreparation,
    #[pyo3(name = "ASSET_PREPARATION")]
    AssetPreparation,
    #[pyo3(name = "ENCODER_STARTUP")]
    EncoderStartup,
    #[pyo3(name = "FRAME_COMPOSITION")]
    FrameComposition,
    #[pyo3(name = "FRAME_WRITE")]
    FrameWrite,
    #[pyo3(name = "ENCODER_FINALIZATION")]
    EncoderFinalization,
    #[pyo3(name = "OUTPUT_PUBLICATION")]
    OutputPublication,
    #[pyo3(name = "CANCELLATION")]
    Cancellation,
}
impl From<NativeRenderFailureStage> for PyRenderFailureStage {
    fn from(value: NativeRenderFailureStage) -> Self {
        match value {
            NativeRenderFailureStage::OutputPreparation => Self::OutputPreparation,
            NativeRenderFailureStage::AssetPreparation => Self::AssetPreparation,
            NativeRenderFailureStage::EncoderStartup => Self::EncoderStartup,
            NativeRenderFailureStage::FrameComposition => Self::FrameComposition,
            NativeRenderFailureStage::FrameWrite => Self::FrameWrite,
            NativeRenderFailureStage::EncoderFinalization => Self::EncoderFinalization,
            NativeRenderFailureStage::OutputPublication => Self::OutputPublication,
            NativeRenderFailureStage::Cancellation => Self::Cancellation,
        }
    }
}
#[pymethods]
impl PyRenderFailureStage {
    #[getter]
    fn value(&self) -> &'static str {
        match self {
            Self::OutputPreparation => "output_preparation",
            Self::AssetPreparation => "asset_preparation",
            Self::EncoderStartup => "encoder_startup",
            Self::FrameComposition => "frame_composition",
            Self::FrameWrite => "frame_write",
            Self::EncoderFinalization => "encoder_finalization",
            Self::OutputPublication => "output_publication",
            Self::Cancellation => "cancellation",
        }
    }
    fn __str__(&self) -> &'static str {
        self.value()
    }
}

#[pyclass(
    name = "RenderEvent",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
pub(crate) struct PyRenderEvent {
    #[pyo3(get)]
    schema_version: u8,
    #[pyo3(get)]
    kind: String,
    #[pyo3(get)]
    operation_id: u64,
    #[pyo3(get)]
    stage: Option<String>,
    #[pyo3(get)]
    frame: Option<u64>,
    #[pyo3(get)]
    total_frames: Option<u64>,
    #[pyo3(get)]
    fraction: Option<f64>,
    #[pyo3(get)]
    output_path: Option<PathBuf>,
}
impl From<NativeRenderEvent> for PyRenderEvent {
    fn from(value: NativeRenderEvent) -> Self {
        let schema_version = value.schema_version();
        let operation_id = value.operation_id().value();
        match value {
            NativeRenderEvent::Started {
                total_frames,
                output_path,
                ..
            } => Self {
                schema_version,
                kind: "started".to_owned(),
                operation_id,
                stage: None,
                frame: None,
                total_frames,
                fraction: None,
                output_path: Some(output_path),
            },
            NativeRenderEvent::StageChanged { stage, .. } => Self {
                schema_version,
                kind: "stage_changed".to_owned(),
                operation_id,
                stage: Some(stage.as_str().to_owned()),
                frame: None,
                total_frames: None,
                fraction: None,
                output_path: None,
            },
            NativeRenderEvent::Progress {
                frame,
                total_frames,
                fraction,
                ..
            } => Self {
                schema_version,
                kind: "progress".to_owned(),
                operation_id,
                stage: Some("rendering".to_owned()),
                frame: Some(frame),
                total_frames: Some(total_frames),
                fraction: Some(fraction),
                output_path: None,
            },
            NativeRenderEvent::Completed { output_path, .. } => Self {
                schema_version,
                kind: "completed".to_owned(),
                operation_id,
                stage: None,
                frame: None,
                total_frames: None,
                fraction: None,
                output_path: Some(output_path),
            },
            NativeRenderEvent::Cancelled { .. } => Self {
                schema_version,
                kind: "cancelled".to_owned(),
                operation_id,
                stage: None,
                frame: None,
                total_frames: None,
                fraction: None,
                output_path: None,
            },
            NativeRenderEvent::Failed { .. } => Self {
                schema_version,
                kind: "failed".to_owned(),
                operation_id,
                stage: None,
                frame: None,
                total_frames: None,
                fraction: None,
                output_path: None,
            },
        }
    }
}
#[pymethods]
impl PyRenderEvent {}

#[pyclass(
    name = "RenderTimings",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone)]
pub(crate) struct PyRenderTimings {
    #[pyo3(get)]
    project_parse_ms: u128,
    #[pyo3(get)]
    operation_total_ms: u128,
    #[pyo3(get)]
    semantic_validation_ms: u128,
    #[pyo3(get)]
    preflight_ms: u128,
    #[pyo3(get)]
    plan_compile_ms: u128,
    #[pyo3(get)]
    asset_decode_ms: u128,
    #[pyo3(get)]
    gpu_initialization_ms: Option<u128>,
    #[pyo3(get)]
    gpu_adapter_request_ms: Option<u128>,
    #[pyo3(get)]
    gpu_device_request_ms: Option<u128>,
    #[pyo3(get)]
    gpu_pipeline_creation_ms: Option<u128>,
    #[pyo3(get)]
    texture_upload_ms: Option<u128>,
    #[pyo3(get)]
    gpu_frame_command_encode_ms: Option<u128>,
    #[pyo3(get)]
    gpu_submission_ms: Option<u128>,
    #[pyo3(get)]
    gpu_readback_wait_ms: Option<u128>,
    #[pyo3(get)]
    row_repack_ms: Option<u128>,
    #[pyo3(get)]
    track_evaluation_ms: u128,
    #[pyo3(get)]
    frame_render_ms: u128,
    #[pyo3(get)]
    encoder_write_ms: u128,
    #[pyo3(get)]
    encoder_finalize_ms: u128,
    #[pyo3(get)]
    output_publish_ms: u128,
}
impl From<&NativeRenderTimings> for PyRenderTimings {
    fn from(v: &NativeRenderTimings) -> Self {
        Self {
            project_parse_ms: v.project_parse_ms,
            operation_total_ms: v.operation_total_ms,
            semantic_validation_ms: v.semantic_validation_ms,
            preflight_ms: v.preflight_ms,
            plan_compile_ms: v.plan_compile_ms,
            asset_decode_ms: v.asset_decode_ms,
            gpu_initialization_ms: v.gpu_initialization_ms,
            gpu_adapter_request_ms: v.gpu_adapter_request_ms,
            gpu_device_request_ms: v.gpu_device_request_ms,
            gpu_pipeline_creation_ms: v.gpu_pipeline_creation_ms,
            texture_upload_ms: v.texture_upload_ms,
            gpu_frame_command_encode_ms: v.gpu_frame_command_encode_ms,
            gpu_submission_ms: v.gpu_submission_ms,
            gpu_readback_wait_ms: v.gpu_readback_wait_ms,
            row_repack_ms: v.row_repack_ms,
            track_evaluation_ms: v.track_evaluation_ms,
            frame_render_ms: v.frame_render_ms,
            encoder_write_ms: v.encoder_write_ms,
            encoder_finalize_ms: v.encoder_finalize_ms,
            output_publish_ms: v.output_publish_ms,
        }
    }
}

#[pyclass(
    name = "RenderFailureContext",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone)]
pub(crate) struct PyRenderFailureContext {
    #[pyo3(get)]
    stage: PyRenderFailureStage,
    #[pyo3(get)]
    last_completed_frame_index: Option<u64>,
    #[pyo3(get)]
    completed_frames: u64,
    #[pyo3(get)]
    attempted_frame: Option<u64>,
    #[pyo3(get)]
    total_frames: u64,
    #[pyo3(get)]
    timeline_position: Option<f64>,
    #[pyo3(get)]
    progress: Option<f64>,
    #[pyo3(get)]
    output_path: Option<PathBuf>,
    #[pyo3(get)]
    temporary_output_path: Option<PathBuf>,
}
impl From<&NativeRenderFailureContext> for PyRenderFailureContext {
    fn from(v: &NativeRenderFailureContext) -> Self {
        Self {
            stage: v.stage.into(),
            last_completed_frame_index: v.last_completed_frame_index,
            completed_frames: v.completed_frames,
            attempted_frame: v.attempted_frame,
            total_frames: v.total_frames,
            timeline_position: v.timeline_position,
            progress: v.progress,
            output_path: v.output_path.clone(),
            temporary_output_path: v.temporary_output_path.clone(),
        }
    }
}

#[pyclass(
    name = "RenderPerformance",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone)]
pub(crate) struct PyRenderPerformance {
    #[pyo3(get)]
    rendered_frame_count: u64,
    #[pyo3(get)]
    command_submission_count: u64,
    #[pyo3(get)]
    decoded_image_count: usize,
    #[pyo3(get)]
    bitmap_cache_hits: u64,
    #[pyo3(get)]
    bitmap_cache_misses: u64,
    #[pyo3(get)]
    uploaded_texture_count: usize,
}
impl From<&NativeRenderPerformance> for PyRenderPerformance {
    fn from(v: &NativeRenderPerformance) -> Self {
        Self {
            rendered_frame_count: v.rendered_frame_count,
            command_submission_count: v.command_submission_count,
            decoded_image_count: v.decoded_image_count,
            bitmap_cache_hits: v.bitmap_cache_hits,
            bitmap_cache_misses: v.bitmap_cache_misses,
            uploaded_texture_count: v.uploaded_texture_count,
        }
    }
}

#[pyclass(
    name = "RenderResult",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
pub(crate) struct PyRenderResult {
    #[pyo3(get)]
    editor_version: String,
    #[pyo3(get)]
    project_path: PathBuf,
    #[pyo3(get)]
    output_path: PathBuf,
    #[pyo3(get)]
    width: u32,
    #[pyo3(get)]
    height: u32,
    #[pyo3(get)]
    frame_rate: String,
    #[pyo3(get)]
    duration_seconds: f64,
    #[pyo3(get)]
    total_frames: u64,
    #[pyo3(get)]
    visual_clip_count: usize,
    #[pyo3(get)]
    audio_present: bool,
    #[pyo3(get)]
    preview: bool,
    #[pyo3(get)]
    elapsed_ms: u128,
    #[pyo3(get)]
    timing_scope: PyRenderTimingScope,
    #[pyo3(get)]
    timings: PyRenderTimings,
    #[pyo3(get)]
    performance: PyRenderPerformance,
    #[pyo3(get)]
    requested_backend: PyBackendPreference,
    #[pyo3(get)]
    selected_backend: String,
    #[pyo3(get)]
    encoder_backend: String,
    #[pyo3(get)]
    fallback: Option<crate::prepared::PyBackendFallback>,
    #[pyo3(get)]
    adapter: Option<crate::prepared::PyAdapterInfo>,
    warnings: Vec<PyDiagnostic>,
}
impl From<NativeRenderResult> for PyRenderResult {
    fn from(v: NativeRenderResult) -> Self {
        let requested_backend = match v.requested_render_backend {
            vestra::BackendPreference::Auto => PyBackendPreference::Auto,
            vestra::BackendPreference::Cpu => PyBackendPreference::Cpu,
            vestra::BackendPreference::Wgpu => PyBackendPreference::Wgpu,
        };
        Self {
            editor_version: v.editor_version.to_owned(),
            project_path: v.project,
            output_path: v.output,
            width: v.width,
            height: v.height,
            frame_rate: v.frame_rate,
            duration_seconds: v.duration,
            total_frames: v.total_frames,
            visual_clip_count: v.visual_clip_count,
            audio_present: v.audio_present,
            preview: v.preview,
            elapsed_ms: v.elapsed_ms,
            timing_scope: v.timing_scope.into(),
            timings: (&v.timings).into(),
            performance: (&v.performance).into(),
            requested_backend,
            selected_backend: v.render_backend.to_owned(),
            encoder_backend: v.encoder_backend.to_owned(),
            fallback: v.backend_fallback.as_ref().map(Into::into),
            adapter: v.adapter.as_ref().map(Into::into),
            warnings: diagnostics(&v.warnings),
        }
    }
}
#[pymethods]
impl PyRenderResult {
    #[getter]
    fn elapsed_seconds(&self) -> f64 {
        self.elapsed_ms as f64 / 1_000.0
    }
    #[getter]
    fn warnings(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        diagnostic_tuple(py, &self.warnings)
    }
}

struct CallbackState {
    callback: Option<Py<PyAny>>,
    first_error: Option<PyErr>,
    disabled: bool,
}
impl CallbackState {
    fn new(callback: Option<Py<PyAny>>) -> Self {
        Self {
            callback,
            first_error: None,
            disabled: false,
        }
    }
    fn observe(&mut self, event: NativeRenderEvent) -> RenderObserverControl {
        if self.disabled {
            return RenderObserverControl::Cancel;
        }
        let Some(callback) = self.callback.as_ref() else {
            return RenderObserverControl::Continue;
        };
        let terminal = event.is_terminal();
        CALLBACK_PYTHON_ATTACHMENTS.fetch_add(1, Ordering::Relaxed);
        // Rendering runs detached from Python so decoding, composition, and
        // encoding do not monopolize the interpreter. Reattach only for the
        // callback, where Python owns the callable and any exception it raises.
        let result = Python::attach(|py| {
            let snapshot = Py::new(py, PyRenderEvent::from(event))?;
            callback.bind(py).call1((snapshot,)).map(|_| ())
        });
        if let Err(error) = result {
            // The native observer turns callback failure into cancellation at
            // the next engine boundary. Keep the original Python exception for
            // the caller; native cleanup is attached as secondary context.
            if terminal {
                // Publication already succeeded for `Completed`; callback
                // failure is observer-side and cannot rewrite that result.
                return RenderObserverControl::Continue;
            }
            self.first_error = Some(error);
            self.disabled = true;
            RenderObserverControl::Cancel
        } else {
            RenderObserverControl::Continue
        }
    }
}

struct Invocation {
    result: Result<NativeRenderResult, EditorError>,
    callback_error: Option<PyErr>,
}

pub(crate) fn render_prepared(
    py: Python<'_>,
    prepared: &crate::prepared::PyPreparedProject,
    request: &PyPreparedVideoRenderRequest,
    show_progress: bool,
    on_progress: Option<Py<PyAny>>,
    cancellation: Option<&PyCancellationToken>,
) -> PyResult<PyRenderResult> {
    let progress = validate_progress(py, on_progress)?;
    let request = request.inner.clone();
    let cancellation =
        cancellation.map_or_else(NativeCancellationToken::new, |token| token.inner.clone());
    match prepared.video_operation(py, move |native| {
        wait_for_video_render_test_barrier()?;
        NATIVE_RENDER_INVOCATIONS.fetch_add(1, Ordering::Relaxed);
        let (result, callback_error) = if let Some(progress) = progress {
            let mut state = CallbackState::new(Some(progress));
            let result = native.render_video_with_observer(
                request,
                |event| state.observe(event),
                &cancellation,
            );
            (result, state.first_error)
        } else {
            let mode = if show_progress {
                vestra::ProgressMode::Auto
            } else {
                vestra::ProgressMode::Disabled
            };
            let result = native.render_video_with_progress(
                request.with_progress_mode(mode),
                None,
                &cancellation,
            );
            (result, None)
        };
        Ok(Invocation {
            result,
            callback_error,
        })
    }) {
        Ok(Ok(invocation)) => finish_invocation(py, invocation),
        Ok(Err(error)) => Err(render_error(py, error)?),
        Err(error) => Err(crate::prepared::slot_error(py, error)?),
    }
}

pub(crate) fn render_one_shot(
    py: Python<'_>,
    editor: &NativeEditor,
    project: &NativeProject,
    request: &PyRenderRequest,
    show_progress: bool,
    on_progress: Option<Py<PyAny>>,
    cancellation: Option<&PyCancellationToken>,
) -> PyResult<PyRenderResult> {
    let progress = validate_progress(py, on_progress)?;
    let request = request.inner.clone();
    let cancellation =
        cancellation.map_or_else(NativeCancellationToken::new, |token| token.inner.clone());
    // The one-shot path follows the same rule as prepared operations: native
    // rendering is detached, while callbacks briefly reattach the interpreter
    // in CallbackState::observe.
    let invocation = py.detach(|| {
        NATIVE_RENDER_INVOCATIONS.fetch_add(1, Ordering::Relaxed);
        if let Some(progress) = progress {
            let mut state = CallbackState::new(Some(progress));
            let result = editor.render_with_observer(
                project,
                request,
                |event| state.observe(event),
                &cancellation,
            );
            Invocation {
                result,
                callback_error: state.first_error,
            }
        } else {
            let mode = if show_progress {
                vestra::ProgressMode::Auto
            } else {
                vestra::ProgressMode::Disabled
            };
            Invocation {
                result: editor.render_with_progress(
                    project,
                    request.with_progress_mode(mode),
                    None,
                    &cancellation,
                ),
                callback_error: None,
            }
        }
    });
    finish_invocation(py, invocation)
}

#[derive(Default)]
struct VideoRenderTestBarrier {
    armed: bool,
    entered: bool,
    released: bool,
}

fn video_render_test_barrier() -> &'static (Mutex<VideoRenderTestBarrier>, Condvar) {
    static BARRIER: OnceLock<(Mutex<VideoRenderTestBarrier>, Condvar)> = OnceLock::new();
    BARRIER.get_or_init(|| {
        (
            Mutex::new(VideoRenderTestBarrier::default()),
            Condvar::new(),
        )
    })
}

fn wait_for_video_render_test_barrier() -> Result<(), crate::prepared::SlotError> {
    let (lock, wake) = video_render_test_barrier();
    let mut state = lock
        .lock()
        .map_err(|_| crate::prepared::SlotError::Poisoned)?;
    if !state.armed {
        return Ok(());
    }
    state.armed = false;
    state.entered = true;
    wake.notify_all();
    while !state.released {
        state = wake
            .wait(state)
            .map_err(|_| crate::prepared::SlotError::Poisoned)?;
    }
    state.entered = false;
    state.released = false;
    Ok(())
}

fn video_render_synchronization_error() -> PyErr {
    PyRuntimeError::new_err("video render test synchronization was poisoned")
}

#[pyfunction]
fn _test_arm_video_render() -> PyResult<()> {
    let (lock, _) = video_render_test_barrier();
    let mut state = lock
        .lock()
        .map_err(|_| video_render_synchronization_error())?;
    state.armed = true;
    state.entered = false;
    state.released = false;
    Ok(())
}

#[pyfunction]
fn _test_wait_until_video_render_entered(py: Python<'_>) -> PyResult<()> {
    py.detach(|| {
        let (lock, wake) = video_render_test_barrier();
        let state = lock
            .lock()
            .map_err(|_| video_render_synchronization_error())?;
        let _state = wake
            .wait_while(state, |state| !state.entered)
            .map_err(|_| video_render_synchronization_error())?;
        Ok(())
    })
}

#[pyfunction]
fn _test_release_video_render() -> PyResult<()> {
    let (lock, wake) = video_render_test_barrier();
    let mut state = lock
        .lock()
        .map_err(|_| video_render_synchronization_error())?;
    state.released = true;
    wake.notify_all();
    Ok(())
}

#[pyfunction]
fn _test_reset_native_render_invocation_count() -> PyResult<()> {
    NATIVE_RENDER_INVOCATIONS.store(0, Ordering::Relaxed);
    Ok(())
}

#[pyfunction]
fn _test_native_render_invocation_count() -> PyResult<usize> {
    Ok(NATIVE_RENDER_INVOCATIONS.load(Ordering::Relaxed))
}

#[pyfunction]
fn _test_reset_callback_attach_count() -> PyResult<()> {
    CALLBACK_PYTHON_ATTACHMENTS.store(0, Ordering::Relaxed);
    Ok(())
}

#[pyfunction]
fn _test_callback_attach_count() -> PyResult<usize> {
    Ok(CALLBACK_PYTHON_ATTACHMENTS.load(Ordering::Relaxed))
}

fn finish_invocation(py: Python<'_>, invocation: Invocation) -> PyResult<PyRenderResult> {
    if let Some(error) = invocation.callback_error {
        if let Err(native_error) = invocation.result {
            // Preserve the callback exception as the primary exception. The
            // native cancellation/error is still useful to callers inspecting
            // cleanup, but must never replace the original Python failure.
            if let Ok(context) = render_error(py, native_error) {
                let _ = error
                    .value(py)
                    .setattr("render_cleanup_error", context.value(py));
            }
        }
        return Err(error);
    }
    match invocation.result {
        Ok(result) => Ok(result.into()),
        Err(error) => Err(render_error(py, error)?),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCancellationToken>()?;
    module.add_class::<PyPreparedVideoRenderRequest>()?;
    module.add_class::<PyRenderRequest>()?;
    module.add_class::<PyRenderEvent>()?;
    module.add_class::<PyRenderResult>()?;
    module.add_class::<PyRenderTimingScope>()?;
    module.add_class::<PyRenderTimings>()?;
    module.add_class::<PyRenderPerformance>()?;
    module.add_class::<PyRenderFailureContext>()?;
    module.add_class::<PyRenderFailureStage>()?;
    module.add_function(wrap_pyfunction!(_test_arm_video_render, module)?)?;
    module.add_function(wrap_pyfunction!(
        _test_wait_until_video_render_entered,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(_test_release_video_render, module)?)?;
    module.add_function(wrap_pyfunction!(
        _test_reset_native_render_invocation_count,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        _test_native_render_invocation_count,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(_test_reset_callback_attach_count, module)?)?;
    module.add_function(wrap_pyfunction!(_test_callback_attach_count, module)?)?;
    Ok(())
}
