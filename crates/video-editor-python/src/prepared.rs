use std::{
    sync::{Condvar, Mutex, OnceLock, TryLockError},
    time::Duration,
};

use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::{PyAny, PyBool, PyBytes, PyInt, PyModule},
};
use video_editor::{
    AdapterDeviceType as NativeAdapterDeviceType, AdapterInfo as NativeAdapterInfo,
    BackendFallback as NativeBackendFallback, BackendKind as NativeBackendKind, EditorError,
    Frame as NativeFrame, FrameRate as NativeFrameRate, GraphicsBackend as NativeGraphicsBackend,
    PixelFormat as NativePixelFormat, PreparationReport as NativePreparationReport,
    PreparationTimings as NativePreparationTimings, PrepareOptions as NativePrepareOptions,
    PreparedProject as NativePreparedProject,
};

use crate::{
    FrameRenderError, PreparationError, PreparedProjectBusyError, PyBackendPreference,
    PyDiagnostic, attach_error_context, diagnostic_tuple, diagnostics,
};

#[pyclass(
    name = "PrepareOptions",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
pub(crate) struct PyPrepareOptions {
    pub(crate) inner: NativePrepareOptions,
    backend: PyBackendPreference,
}

#[pymethods]
impl PyPrepareOptions {
    #[new]
    #[pyo3(signature = (*, backend = None))]
    fn new(backend: Option<&PyBackendPreference>) -> Self {
        let backend = backend.copied().unwrap_or(PyBackendPreference::Auto);
        Self {
            inner: NativePrepareOptions::new(backend.native()),
            backend,
        }
    }

    #[getter]
    fn backend(&self) -> PyBackendPreference {
        self.backend
    }
}

#[pyclass(
    name = "BackendKind",
    frozen,
    eq,
    hash,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PyBackendKind {
    #[pyo3(name = "CPU")]
    Cpu,
    #[pyo3(name = "WGPU")]
    Wgpu,
}
impl From<NativeBackendKind> for PyBackendKind {
    fn from(value: NativeBackendKind) -> Self {
        match value {
            NativeBackendKind::Cpu => Self::Cpu,
            NativeBackendKind::Wgpu => Self::Wgpu,
        }
    }
}
#[pymethods]
impl PyBackendKind {
    #[getter]
    fn value(&self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Wgpu => "wgpu",
        }
    }
    fn __str__(&self) -> &'static str {
        self.value()
    }
}

#[pyclass(
    name = "PixelFormat",
    frozen,
    eq,
    hash,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PyPixelFormat {
    #[pyo3(name = "RGBA8")]
    Rgba8,
}
impl From<NativePixelFormat> for PyPixelFormat {
    fn from(_: NativePixelFormat) -> Self {
        Self::Rgba8
    }
}
#[pymethods]
impl PyPixelFormat {
    #[getter]
    fn value(&self) -> &'static str {
        "rgba8"
    }
    fn __str__(&self) -> &'static str {
        self.value()
    }
}

#[pyclass(
    name = "AdapterDeviceType",
    frozen,
    eq,
    hash,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PyAdapterDeviceType {
    #[pyo3(name = "DISCRETE_GPU")]
    DiscreteGpu,
    #[pyo3(name = "INTEGRATED_GPU")]
    IntegratedGpu,
    #[pyo3(name = "VIRTUAL_GPU")]
    VirtualGpu,
    #[pyo3(name = "CPU")]
    Cpu,
    #[pyo3(name = "OTHER")]
    Other,
}
impl From<NativeAdapterDeviceType> for PyAdapterDeviceType {
    fn from(value: NativeAdapterDeviceType) -> Self {
        match value {
            NativeAdapterDeviceType::DiscreteGpu => Self::DiscreteGpu,
            NativeAdapterDeviceType::IntegratedGpu => Self::IntegratedGpu,
            NativeAdapterDeviceType::VirtualGpu => Self::VirtualGpu,
            NativeAdapterDeviceType::Cpu => Self::Cpu,
            NativeAdapterDeviceType::Other => Self::Other,
        }
    }
}
#[pymethods]
impl PyAdapterDeviceType {
    #[getter]
    fn value(&self) -> &'static str {
        match self {
            Self::DiscreteGpu => "discretegpu",
            Self::IntegratedGpu => "integratedgpu",
            Self::VirtualGpu => "virtualgpu",
            Self::Cpu => "cpu",
            Self::Other => "other",
        }
    }
    fn __str__(&self) -> &'static str {
        self.value()
    }
}

#[pyclass(
    name = "GraphicsBackend",
    frozen,
    eq,
    hash,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PyGraphicsBackend {
    #[pyo3(name = "VULKAN")]
    Vulkan,
    #[pyo3(name = "METAL")]
    Metal,
    #[pyo3(name = "DX12")]
    Dx12,
    #[pyo3(name = "GL")]
    Gl,
    #[pyo3(name = "BROWSER_WEBGPU")]
    BrowserWebGpu,
    #[pyo3(name = "OTHER")]
    Other,
}
impl From<NativeGraphicsBackend> for PyGraphicsBackend {
    fn from(value: NativeGraphicsBackend) -> Self {
        match value {
            NativeGraphicsBackend::Vulkan => Self::Vulkan,
            NativeGraphicsBackend::Metal => Self::Metal,
            NativeGraphicsBackend::Dx12 => Self::Dx12,
            NativeGraphicsBackend::Gl => Self::Gl,
            NativeGraphicsBackend::BrowserWebGpu => Self::BrowserWebGpu,
            NativeGraphicsBackend::Other => Self::Other,
        }
    }
}
#[pymethods]
impl PyGraphicsBackend {
    #[getter]
    fn value(&self) -> &'static str {
        match self {
            Self::Vulkan => "vulkan",
            Self::Metal => "metal",
            Self::Dx12 => "dx12",
            Self::Gl => "gl",
            Self::BrowserWebGpu => "browserwebgpu",
            Self::Other => "other",
        }
    }
    fn __str__(&self) -> &'static str {
        self.value()
    }
}

#[pyclass(
    name = "BackendFallback",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone)]
pub(crate) struct PyBackendFallback {
    #[pyo3(get)]
    code: String,
    #[pyo3(get)]
    stage: String,
    #[pyo3(get)]
    message: String,
}
impl From<&NativeBackendFallback> for PyBackendFallback {
    fn from(value: &NativeBackendFallback) -> Self {
        Self {
            code: value.code.clone(),
            stage: value.stage.clone(),
            message: value.message.clone(),
        }
    }
}

#[pyclass(
    name = "AdapterInfo",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone)]
pub(crate) struct PyAdapterInfo {
    #[pyo3(get)]
    adapter_name: String,
    #[pyo3(get)]
    device_type: PyAdapterDeviceType,
    #[pyo3(get)]
    graphics_backend: PyGraphicsBackend,
    #[pyo3(get)]
    driver_name: String,
    #[pyo3(get)]
    driver_info: String,
    #[pyo3(get)]
    vendor_id: u32,
    #[pyo3(get)]
    device_id: u32,
}
impl From<&NativeAdapterInfo> for PyAdapterInfo {
    fn from(value: &NativeAdapterInfo) -> Self {
        Self {
            adapter_name: value.adapter_name.clone(),
            device_type: value.device_type.into(),
            graphics_backend: value.graphics_backend.into(),
            driver_name: value.driver_name.clone(),
            driver_info: value.driver_info.clone(),
            vendor_id: value.vendor_id,
            device_id: value.device_id,
        }
    }
}

#[pyclass(
    name = "FrameRate",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone, Copy)]
pub(crate) struct PyFrameRate {
    inner: NativeFrameRate,
}
#[pymethods]
impl PyFrameRate {
    #[new]
    #[pyo3(signature = (numerator, denominator = None))]
    fn new(numerator: &Bound<'_, PyAny>, denominator: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let numerator = u64_from_python(numerator, "numerator")?;
        let denominator =
            denominator.map_or(Ok(1), |value| u64_from_python(value, "denominator"))?;
        NativeFrameRate::new(numerator, denominator)
            .map(|inner| Self { inner })
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }
    #[getter]
    fn numerator(&self) -> u64 {
        self.inner.numerator()
    }
    #[getter]
    fn denominator(&self) -> u64 {
        self.inner.denominator()
    }
    #[getter]
    fn value(&self) -> f64 {
        self.inner.numerator() as f64 / self.inner.denominator() as f64
    }
}
impl From<NativeFrameRate> for PyFrameRate {
    fn from(inner: NativeFrameRate) -> Self {
        Self { inner }
    }
}

#[pyclass(
    name = "PreparationTimings",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone, Copy)]
pub(crate) struct PyPreparationTimings {
    #[pyo3(get)]
    semantic_validation_ms: u128,
    #[pyo3(get)]
    preflight_ms: u128,
    #[pyo3(get)]
    plan_compile_ms: u128,
    #[pyo3(get)]
    asset_decode_ms: u128,
    #[pyo3(get)]
    audio_analysis_ms: u128,
    #[pyo3(get)]
    backend_initialization_ms: Option<u128>,
    #[pyo3(get)]
    total_ms: u128,
}
impl From<&NativePreparationTimings> for PyPreparationTimings {
    fn from(value: &NativePreparationTimings) -> Self {
        Self {
            semantic_validation_ms: value.semantic_validation_ms,
            preflight_ms: value.preflight_ms,
            plan_compile_ms: value.plan_compile_ms,
            asset_decode_ms: value.asset_decode_ms,
            audio_analysis_ms: value.audio_analysis_ms,
            backend_initialization_ms: value.backend_initialization_ms,
            total_ms: value.total_ms,
        }
    }
}

#[pyclass(
    name = "PreparationReport",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
#[derive(Clone)]
pub(crate) struct PyPreparationReport {
    #[pyo3(get)]
    requested_backend: PyBackendPreference,
    #[pyo3(get)]
    selected_backend: PyBackendKind,
    #[pyo3(get)]
    fallback: Option<PyBackendFallback>,
    #[pyo3(get)]
    adapter: Option<PyAdapterInfo>,
    #[pyo3(get)]
    width: u32,
    #[pyo3(get)]
    height: u32,
    #[pyo3(get)]
    frame_rate: PyFrameRate,
    duration_ns: u128,
    #[pyo3(get)]
    frame_count: u64,
    #[pyo3(get)]
    decoded_asset_count: usize,
    warnings: Vec<PyDiagnostic>,
    #[pyo3(get)]
    timings: PyPreparationTimings,
    #[pyo3(get)]
    supports_single_frame_rendering: bool,
}
impl From<&NativePreparationReport> for PyPreparationReport {
    fn from(value: &NativePreparationReport) -> Self {
        Self {
            requested_backend: match value.requested_backend() {
                video_editor::BackendPreference::Auto => PyBackendPreference::Auto,
                video_editor::BackendPreference::Cpu => PyBackendPreference::Cpu,
                video_editor::BackendPreference::Wgpu => PyBackendPreference::Wgpu,
            },
            selected_backend: value.selected_backend().into(),
            fallback: value.backend_fallback().map(Into::into),
            adapter: value.adapter().map(Into::into),
            width: value.width(),
            height: value.height(),
            frame_rate: value.frame_rate().into(),
            duration_ns: duration_to_ns(value.duration()),
            frame_count: value.frame_count(),
            decoded_asset_count: value.decoded_asset_count(),
            warnings: diagnostics(value.warnings()),
            timings: value.timings().into(),
            supports_single_frame_rendering: value.supports_single_frame_rendering(),
        }
    }
}
#[pymethods]
impl PyPreparationReport {
    #[getter]
    fn duration_ns(&self) -> u128 {
        self.duration_ns
    }
    #[getter]
    fn duration_seconds(&self) -> f64 {
        self.duration_ns as f64 / 1_000_000_000.0
    }
    #[getter]
    fn warnings(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        diagnostic_tuple(py, &self.warnings)
    }
}

#[pyclass(
    name = "Frame",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
pub(crate) struct PyFrame {
    inner: NativeFrame,
    timestamp_ns: u128,
}
#[pymethods]
impl PyFrame {
    #[getter]
    fn width(&self) -> u32 {
        self.inner.width()
    }
    #[getter]
    fn height(&self) -> u32 {
        self.inner.height()
    }
    #[getter]
    fn frame_number(&self) -> u64 {
        self.inner.frame_number()
    }
    #[getter]
    fn timestamp_ns(&self) -> u128 {
        self.timestamp_ns
    }
    #[getter]
    fn timestamp_seconds(&self) -> f64 {
        self.timestamp_ns as f64 / 1_000_000_000.0
    }
    #[getter]
    fn pixel_format(&self) -> PyPixelFormat {
        self.inner.pixel_format().into()
    }
    fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, self.inner.as_bytes())
    }
    fn __bytes__<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        self.to_bytes(py)
    }
}
impl From<NativeFrame> for PyFrame {
    fn from(inner: NativeFrame) -> Self {
        Self {
            timestamp_ns: duration_to_ns(inner.timestamp()),
            inner,
        }
    }
}

enum PreparedSlot {
    Ready(Box<NativePreparedProject>),
    Busy,
}
#[pyclass(
    name = "PreparedProject",
    frozen,
    skip_from_py_object,
    module = "video_editor._native"
)]
pub(crate) struct PyPreparedProject {
    state: Mutex<PreparedSlot>,
    report: PyPreparationReport,
}
impl PyPreparedProject {
    pub(crate) fn new(inner: NativePreparedProject) -> Self {
        let report = PyPreparationReport::from(inner.preparation_report());
        Self {
            state: Mutex::new(PreparedSlot::Ready(Box::new(inner))),
            report,
        }
    }
    fn take(&self) -> Result<NativePreparedProject, SlotError> {
        match self.state.try_lock() {
            Ok(mut slot) => match std::mem::replace(&mut *slot, PreparedSlot::Busy) {
                PreparedSlot::Ready(value) => Ok(*value),
                PreparedSlot::Busy => {
                    *slot = PreparedSlot::Busy;
                    Err(SlotError::Busy)
                }
            },
            Err(TryLockError::WouldBlock) => Err(SlotError::Busy),
            Err(TryLockError::Poisoned(_)) => Err(SlotError::Poisoned),
        }
    }
    fn restore(&self, value: NativePreparedProject) -> Result<(), SlotError> {
        match self.state.lock() {
            Ok(mut slot) => {
                *slot = PreparedSlot::Ready(Box::new(value));
                Ok(())
            }
            Err(_) => Err(SlotError::Poisoned),
        }
    }
    fn operation<T>(
        &self,
        py: Python<'_>,
        operation: impl FnOnce(&mut NativePreparedProject) -> Result<T, EditorError> + Send,
    ) -> Result<Result<T, EditorError>, SlotError>
    where
        T: Send,
    {
        let mut prepared = self.take()?;
        py.detach(|| {
            if let Err(error) = wait_for_prepared_test_barrier() {
                self.restore(prepared)?;
                return Err(error);
            }
            let result = operation(&mut prepared);
            self.restore(prepared)?;
            Ok(result)
        })
    }

    pub(crate) fn video_operation<T>(
        &self,
        py: Python<'_>,
        operation: impl FnOnce(&mut NativePreparedProject) -> Result<T, SlotError> + Send,
    ) -> Result<Result<T, EditorError>, SlotError>
    where
        T: Send,
    {
        let mut prepared = self.take()?;
        py.detach(|| {
            if let Err(error) = wait_for_prepared_test_barrier() {
                self.restore(prepared)?;
                return Err(error);
            }
            let value = operation(&mut prepared);
            self.restore(prepared)?;
            value.map(Ok)
        })
    }
}
#[derive(Clone, Copy)]
pub(crate) enum SlotError {
    Busy,
    Poisoned,
}
pub(crate) fn slot_error(py: Python<'_>, error: SlotError) -> PyResult<PyErr> {
    match error {
        SlotError::Busy => {
            let exception = PyErr::new::<PreparedProjectBusyError, _>("prepared project is busy");
            attach_error_context(py, &exception, "busy", &[], &[])?;
            Ok(exception)
        }
        SlotError::Poisoned => Ok(PyRuntimeError::new_err(
            "prepared project synchronization failed",
        )),
    }
}
pub(crate) fn preparation_error<T>(py: Python<'_>, error: EditorError) -> PyResult<T> {
    let error_diagnostics = diagnostics(error.diagnostics());
    let warnings = diagnostics(error.warnings());
    let exception = PyErr::new::<PreparationError, _>(error.to_string());
    attach_error_context(
        py,
        &exception,
        error.kind().as_str(),
        &error_diagnostics,
        &warnings,
    )?;
    Err(exception)
}
fn frame_error<T>(py: Python<'_>, error: EditorError) -> PyResult<T> {
    let error_diagnostics = diagnostics(error.diagnostics());
    let warnings = diagnostics(error.warnings());
    let exception = PyErr::new::<FrameRenderError, _>(error.to_string());
    attach_error_context(
        py,
        &exception,
        error.kind().as_str(),
        &error_diagnostics,
        &warnings,
    )?;
    Err(exception)
}

#[pymethods]
impl PyPreparedProject {
    #[getter]
    fn preparation_report(&self) -> PyPreparationReport {
        self.report.clone()
    }
    #[getter]
    fn supports_single_frame_rendering(&self) -> bool {
        self.report.supports_single_frame_rendering
    }
    #[expect(
        clippy::result_large_err,
        reason = "the Python bridge preserves native frame diagnostics"
    )]
    fn render_frame_number(
        &self,
        py: Python<'_>,
        frame_number: &Bound<'_, PyAny>,
    ) -> PyResult<PyFrame> {
        let number = u64_from_python(frame_number, "frame_number")?;
        match self.operation(py, move |prepared| prepared.render_frame_number(number)) {
            Ok(Ok(frame)) => Ok(frame.into()),
            Ok(Err(error)) => frame_error(py, error),
            Err(error) => Err(slot_error(py, error)?),
        }
    }
    #[expect(
        clippy::result_large_err,
        reason = "the Python bridge preserves native frame diagnostics"
    )]
    fn render_frame_ns(
        &self,
        py: Python<'_>,
        timestamp_ns: &Bound<'_, PyAny>,
    ) -> PyResult<PyFrame> {
        let timestamp = duration_from_ns(u128_from_python(timestamp_ns, "timestamp_ns")?)?;
        match self.operation(py, move |prepared| prepared.render_frame(timestamp)) {
            Ok(Ok(frame)) => Ok(frame.into()),
            Ok(Err(error)) => frame_error(py, error),
            Err(error) => Err(slot_error(py, error)?),
        }
    }
    #[expect(
        clippy::result_large_err,
        reason = "the Python bridge preserves native frame diagnostics"
    )]
    fn render_frame_seconds(
        &self,
        py: Python<'_>,
        seconds: &Bound<'_, PyAny>,
    ) -> PyResult<PyFrame> {
        let timestamp = duration_from_seconds(seconds)?;
        match self.operation(py, move |prepared| prepared.render_frame(timestamp)) {
            Ok(Ok(frame)) => Ok(frame.into()),
            Ok(Err(error)) => frame_error(py, error),
            Err(error) => Err(slot_error(py, error)?),
        }
    }
    #[pyo3(signature = (request, *, progress = None, cancellation = None))]
    fn render_video(
        &self,
        py: Python<'_>,
        request: &crate::render::PyPreparedVideoRenderRequest,
        progress: Option<Py<PyAny>>,
        cancellation: Option<&crate::render::PyCancellationToken>,
    ) -> PyResult<crate::render::PyRenderResult> {
        crate::render::render_prepared(py, self, request, progress, cancellation)
    }
}

fn u64_from_python(value: &Bound<'_, PyAny>, name: &str) -> PyResult<u64> {
    if value.is_instance_of::<PyBool>() {
        return Err(PyTypeError::new_err(format!(
            "{name} must be an integer, not bool"
        )));
    }
    if !value.is_instance_of::<PyInt>() {
        return Err(PyTypeError::new_err(format!("{name} must be an integer")));
    }
    value
        .extract()
        .map_err(|_| PyValueError::new_err(format!("{name} is outside the supported range")))
}
fn u128_from_python(value: &Bound<'_, PyAny>, name: &str) -> PyResult<u128> {
    if value.is_instance_of::<PyBool>() {
        return Err(PyTypeError::new_err(format!(
            "{name} must be an integer, not bool"
        )));
    }
    if !value.is_instance_of::<PyInt>() {
        return Err(PyTypeError::new_err(format!("{name} must be an integer")));
    }
    value.extract().map_err(|_| {
        PyValueError::new_err(format!(
            "{name} must be a non-negative integer within duration range"
        ))
    })
}
fn duration_to_ns(value: Duration) -> u128 {
    u128::from(value.as_secs()) * 1_000_000_000 + u128::from(value.subsec_nanos())
}
fn duration_from_ns(value: u128) -> PyResult<Duration> {
    let seconds = value / 1_000_000_000;
    let nanos = value % 1_000_000_000;
    let seconds = u64::try_from(seconds)
        .map_err(|_| PyValueError::new_err("timestamp_ns is outside the native duration range"))?;
    let nanos = u32::try_from(nanos)
        .map_err(|_| PyValueError::new_err("timestamp_ns is outside the native duration range"))?;
    Ok(Duration::new(seconds, nanos))
}
fn duration_from_seconds(value: &Bound<'_, PyAny>) -> PyResult<Duration> {
    if value.is_instance_of::<PyBool>() {
        return Err(PyTypeError::new_err(
            "seconds must be a finite float, not bool",
        ));
    }
    let seconds: f64 = value
        .extract()
        .map_err(|_| PyTypeError::new_err("seconds must be a finite float"))?;
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(PyValueError::new_err(
            "seconds must be finite and non-negative",
        ));
    }
    let nanos = seconds * 1_000_000_000.0;
    if nanos > u128::MAX as f64 {
        return Err(PyValueError::new_err(
            "seconds is outside the native duration range",
        ));
    }
    duration_from_ns(nanos.trunc() as u128)
}

#[derive(Default)]
struct PreparedTestBarrier {
    armed: bool,
    entered: bool,
    released: bool,
}
fn prepared_test_barrier() -> &'static (Mutex<PreparedTestBarrier>, Condvar) {
    static BARRIER: OnceLock<(Mutex<PreparedTestBarrier>, Condvar)> = OnceLock::new();
    BARRIER.get_or_init(|| (Mutex::new(PreparedTestBarrier::default()), Condvar::new()))
}
fn wait_for_prepared_test_barrier() -> Result<(), SlotError> {
    let (lock, wake) = prepared_test_barrier();
    let mut state = lock.lock().map_err(|_| SlotError::Poisoned)?;
    if !state.armed {
        return Ok(());
    }
    state.armed = false;
    state.entered = true;
    wake.notify_all();
    while !state.released {
        state = wake.wait(state).map_err(|_| SlotError::Poisoned)?;
    }
    state.entered = false;
    state.released = false;
    Ok(())
}

pub(crate) fn wait_for_test_barrier() -> bool {
    wait_for_prepared_test_barrier().is_ok()
}
#[pyfunction]
fn _test_arm_prepared_operation() -> PyResult<()> {
    let (lock, _) = prepared_test_barrier();
    let mut state = lock
        .lock()
        .map_err(|_| PyRuntimeError::new_err("prepared test synchronization was poisoned"))?;
    state.armed = true;
    state.entered = false;
    state.released = false;
    Ok(())
}
#[pyfunction]
fn _test_wait_until_prepared_operation_entered(py: Python<'_>) -> PyResult<()> {
    py.detach(|| {
        let (lock, wake) = prepared_test_barrier();
        let state = lock.lock().map_err(|_| SlotError::Poisoned)?;
        let _state = wake
            .wait_while(state, |state| !state.entered)
            .map_err(|_| SlotError::Poisoned)?;
        Ok::<(), SlotError>(())
    })
    .map_err(|_| PyRuntimeError::new_err("prepared test synchronization was poisoned"))
}
#[pyfunction]
fn _test_release_prepared_operation() -> PyResult<()> {
    let (lock, wake) = prepared_test_barrier();
    let mut state = lock
        .lock()
        .map_err(|_| PyRuntimeError::new_err("prepared test synchronization was poisoned"))?;
    state.released = true;
    wake.notify_all();
    Ok(())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyPrepareOptions>()?;
    module.add_class::<PyPreparedProject>()?;
    module.add_class::<PyPreparationReport>()?;
    module.add_class::<PyPreparationTimings>()?;
    module.add_class::<PyFrameRate>()?;
    module.add_class::<PyFrame>()?;
    module.add_class::<PyPixelFormat>()?;
    module.add_class::<PyBackendKind>()?;
    module.add_class::<PyBackendFallback>()?;
    module.add_class::<PyAdapterInfo>()?;
    module.add_class::<PyAdapterDeviceType>()?;
    module.add_class::<PyGraphicsBackend>()?;
    module.add_function(wrap_pyfunction!(_test_arm_prepared_operation, module)?)?;
    module.add_function(wrap_pyfunction!(
        _test_wait_until_prepared_operation_entered,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(_test_release_prepared_operation, module)?)?;
    Ok(())
}
