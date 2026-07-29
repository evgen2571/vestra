//! Owned prepared-project and single-frame SDK types.

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use crate::{
    BackendFallback, CancellationToken, Diagnostic, EditorError, RenderResult,
    application::{self, ApplicationRenderError},
    render::{RenderBackendKind, RenderBackendPreference, RenderEvent},
};

/// Preparation configuration. The backend choice is fixed for the lifetime of
/// the returned [`PreparedProject`].
#[derive(Clone, Copy, Debug, Default)]
pub struct PrepareOptions {
    backend: RenderBackendPreference,
}

impl PrepareOptions {
    #[must_use]
    pub const fn new(backend: RenderBackendPreference) -> Self {
        Self { backend }
    }
    #[must_use]
    pub const fn backend(&self) -> RenderBackendPreference {
        self.backend
    }
    #[must_use]
    pub const fn with_backend(mut self, backend: RenderBackendPreference) -> Self {
        self.backend = backend;
        self
    }
}

/// Per-operation configuration for [`PreparedProject::render_video`].
#[derive(Clone, Debug)]
pub struct PreparedVideoRenderRequest {
    output: PathBuf,
    overwrite: bool,
}

impl PreparedVideoRenderRequest {
    #[must_use]
    pub fn new(output: impl Into<PathBuf>) -> Self {
        Self {
            output: output.into(),
            overwrite: false,
        }
    }
    #[must_use]
    pub const fn with_overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }
}

/// Pixel storage returned by CPU single-frame rendering.
///
/// Pixels are RGBA, eight bits per channel, row-major with the top row first,
/// tightly packed at `width * 4` bytes per row, and use unpremultiplied alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    Rgba8,
}

/// Backend retained by a prepared project.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendKind {
    Cpu,
    Wgpu,
}

impl From<RenderBackendKind> for BackendKind {
    fn from(value: RenderBackendKind) -> Self {
        match value {
            RenderBackendKind::Cpu => Self::Cpu,
            RenderBackendKind::Wgpu => Self::Wgpu,
        }
    }
}

/// An owned, CPU-accessible rendered frame.
#[derive(Clone, Debug)]
pub struct Frame {
    width: u32,
    height: u32,
    frame_number: u64,
    timestamp: Duration,
    pixels: Vec<u8>,
}

impl Frame {
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }
    #[must_use]
    pub const fn frame_number(&self) -> u64 {
        self.frame_number
    }
    #[must_use]
    pub const fn timestamp(&self) -> Duration {
        self.timestamp
    }
    #[must_use]
    pub const fn pixel_format(&self) -> PixelFormat {
        PixelFormat::Rgba8
    }
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.pixels
    }
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.pixels
    }
}

/// Stable facts captured during preparation, without renderer implementation
/// objects or mutable operation counters.
#[derive(Clone, Debug)]
pub struct PreparationReport {
    requested_backend: RenderBackendPreference,
    selected_backend: BackendKind,
    backend_fallback: Option<BackendFallback>,
    width: u32,
    height: u32,
    frame_rate: (u64, u64),
    duration: Duration,
    frame_count: u64,
    decoded_asset_count: usize,
    warnings: Vec<Diagnostic>,
    timings: PreparationTimings,
}

impl PreparationReport {
    #[must_use]
    pub const fn requested_backend(&self) -> RenderBackendPreference {
        self.requested_backend
    }
    #[must_use]
    pub const fn selected_backend(&self) -> BackendKind {
        self.selected_backend
    }
    #[must_use]
    pub fn backend_fallback(&self) -> Option<&BackendFallback> {
        self.backend_fallback.as_ref()
    }
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }
    #[must_use]
    pub const fn frame_rate(&self) -> (u64, u64) {
        self.frame_rate
    }
    #[must_use]
    pub const fn duration(&self) -> Duration {
        self.duration
    }
    #[must_use]
    pub const fn frame_count(&self) -> u64 {
        self.frame_count
    }
    #[must_use]
    pub const fn decoded_asset_count(&self) -> usize {
        self.decoded_asset_count
    }
    #[must_use]
    pub fn warnings(&self) -> &[Diagnostic] {
        &self.warnings
    }
    #[must_use]
    pub const fn timings(&self) -> &PreparationTimings {
        &self.timings
    }
    #[must_use]
    pub const fn supports_single_frame_rendering(&self) -> bool {
        matches!(self.selected_backend, BackendKind::Cpu)
    }
}

/// Timings for preparation only. Optional GPU stages are absent for CPU.
#[derive(Clone, Copy, Debug, Default)]
pub struct PreparationTimings {
    pub semantic_validation_ms: u128,
    pub preflight_ms: u128,
    pub plan_compile_ms: u128,
    pub asset_decode_ms: u128,
    pub backend_initialization_ms: Option<u128>,
    pub total_ms: u128,
}

/// An owned visual execution snapshot. It freezes the compiled plan, decoded
/// visual assets, resolved metadata, schedule, and selected backend. FFmpeg
/// reopens external media such as audio for each video operation, so those
/// source files must remain available and unchanged for repeatable output.
pub struct PreparedProject {
    prepared: application::PreparedRender,
    report: PreparationReport,
    project_path: PathBuf,
}

impl PreparedProject {
    pub(crate) fn new(
        prepared: application::PreparedRender,
        project_path: PathBuf,
        total_ms: u128,
    ) -> Self {
        let metadata = prepared.result_metadata();
        let timings = prepared.preparation_timings();
        let fallback = prepared.prepared_backend_fallback();
        let mut warnings = prepared.preparation_warnings().to_vec();
        if let Some(fallback) = &fallback {
            warnings.push(crate::render::backend_fallback_warning(fallback));
        }
        let warnings = crate::editor::Editor::operation_warnings(&warnings);
        Self {
            report: PreparationReport {
                requested_backend: prepared.requested_backend(),
                selected_backend: prepared.selected_backend().into(),
                backend_fallback: fallback,
                width: metadata.width,
                height: metadata.height,
                frame_rate: metadata.frame_rate_ratio,
                duration: metadata.duration,
                frame_count: metadata.frame_count,
                decoded_asset_count: metadata.decoded_asset_count,
                warnings,
                timings: PreparationTimings {
                    semantic_validation_ms: timings.validation_ms,
                    preflight_ms: timings.preflight_ms,
                    plan_compile_ms: timings.plan_compile_ms,
                    asset_decode_ms: timings.renderer.decode.as_millis(),
                    backend_initialization_ms: (timings.renderer.gpu_initialization
                        != Duration::ZERO)
                        .then_some(timings.renderer.gpu_initialization.as_millis()),
                    total_ms,
                },
            },
            prepared,
            project_path,
        }
    }
    #[must_use]
    pub const fn preparation_report(&self) -> &PreparationReport {
        &self.report
    }
    #[must_use]
    pub const fn supports_single_frame_rendering(&self) -> bool {
        self.report.supports_single_frame_rendering()
    }

    #[expect(clippy::result_large_err, reason = "frame failures retain diagnostics")]
    pub fn render_frame_number(&mut self, frame_number: u64) -> Result<Frame, EditorError> {
        let completion = self
            .prepared
            .render_frame(frame_number)
            .map_err(frame_error)?;
        let expected = usize::try_from(self.report.width)
            .ok()
            .and_then(|width| {
                usize::try_from(self.report.height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .and_then(|pixels| pixels.checked_mul(4));
        if expected != Some(completion.rgba.len()) {
            return Err(simple_error(
                "MVP-FRAME-BYTES",
                "backend returned a frame with an invalid byte length",
            ));
        }
        Ok(Frame {
            width: self.report.width,
            height: self.report.height,
            frame_number,
            timestamp: video_editor_core::timeline::frame_start_duration(
                frame_number,
                self.report.frame_rate.0,
                self.report.frame_rate.1,
            )
            .map_err(|_| {
                simple_error(
                    "MVP-TIMELINE-OVERFLOW",
                    "frame timestamp cannot be represented",
                )
            })?,
            pixels: completion.rgba,
        })
    }

    #[expect(clippy::result_large_err, reason = "frame failures retain diagnostics")]
    pub fn render_frame(&mut self, timestamp: Duration) -> Result<Frame, EditorError> {
        if timestamp >= self.report.duration {
            return Err(simple_error(
                "MVP-FRAME-RANGE",
                "timestamp is outside the prepared timeline",
            ));
        }
        let frame = video_editor_core::timeline::frame_at_duration(
            timestamp,
            self.report.frame_rate.0,
            self.report.frame_rate.1,
        )
        .map_err(|_| {
            simple_error(
                "MVP-TIMELINE-OVERFLOW",
                "timestamp cannot be mapped to a frame",
            )
        })?;
        if frame >= self.report.frame_count {
            return Err(simple_error(
                "MVP-FRAME-RANGE",
                "timestamp is outside the prepared timeline",
            ));
        }
        self.render_frame_number(frame)
    }

    #[expect(
        clippy::result_large_err,
        reason = "render failures retain diagnostics"
    )]
    pub fn render_video(
        &mut self,
        request: PreparedVideoRenderRequest,
        mut emit: impl FnMut(RenderEvent),
        cancellation: &CancellationToken,
    ) -> Result<RenderResult, EditorError> {
        let operation_started = Instant::now();
        let render_request = application::RenderRequest {
            output_override: Some(request.output),
            overwrite: request.overwrite,
            preview: false,
            cancelled: cancellation.flag(),
            backend_preference: self.report.requested_backend,
        };
        let summary =
            application::render_prepared_project(&mut self.prepared, render_request, &mut emit)
                .map_err(|error| prepared_operation_error(frame_error(error), operation_started))?;
        let mut result = application::render_result(
            &self.project_path,
            self.prepared.result_metadata(),
            summary,
            self.report.warnings.clone(),
        );
        result.timing_scope = crate::RenderTimingScope::PreparedOperation;
        result.timings.operation_total_ms = operation_started.elapsed().as_millis();
        result.timings.total_ms = result.timings.operation_total_ms;
        result.elapsed_ms = result.timings.operation_total_ms;
        Ok(result)
    }
}

fn simple_error(code: &str, message: &str) -> EditorError {
    EditorError::Project {
        errors: vec![Diagnostic::error(
            code,
            crate::Category::Render,
            message,
            "",
        )],
        warnings: Vec::new(),
        timings: crate::RenderTimings::default(),
    }
}
fn frame_error(error: ApplicationRenderError) -> EditorError {
    match error {
        ApplicationRenderError::Plan { diagnostic, .. } => {
            simple_error(&diagnostic.code, &diagnostic.message)
        }
        ApplicationRenderError::Render { error, .. } => EditorError::Render {
            diagnostic: Box::new(error.diagnostic),
            warnings: error.warnings,
            context: Box::new(error.context),
            temporary_removed: error.temporary_removed,
            timings: error.timings,
        },
    }
}

fn prepared_operation_error(mut error: EditorError, started: Instant) -> EditorError {
    let elapsed = started.elapsed().as_millis();
    match &mut error {
        EditorError::Project { timings, .. }
        | EditorError::Plan { timings, .. }
        | EditorError::Render { timings, .. } => {
            timings.operation_total_ms = elapsed;
            timings.total_ms = elapsed;
        }
    }
    error
}
