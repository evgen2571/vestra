//! SDK failure conversion, timing snapshots and lifecycle reporting.
use super::{Editor, EditorError};
use crate::{
    Diagnostic, Project, RenderFailureContext, application::ApplicationRenderError,
    project::LoadError, render::LifecycleEmitter,
};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};
use vestra_core::OperationId;

pub(crate) fn log_render_failure(
    operation_id: OperationId,
    output_path: &Path,
    operation_started: Instant,
    error: &EditorError,
) {
    let diagnostic = error.diagnostics().first();
    tracing::error!(
        target: "vestra.render",
        operation_id = %operation_id,
        output = %output_path.display(),
        stage = error
            .render_failure_context()
            .map_or(error.kind().as_str(), |context| context.stage.as_str()),
        error_code = diagnostic.map_or("unknown", |item| item.code.as_str()),
        error = diagnostic.map_or("render operation failed", |item| item.message.as_str()),
        elapsed_ms = crate::render::trace_milliseconds(operation_started.elapsed()),
        "render failed"
    );
}

pub(crate) fn log_render_timing_summary(
    operation_id: OperationId,
    actual_backend: &str,
    audio_analysis_ms: u128,
    timings: &crate::RenderTimings,
) {
    let common = (
        crate::render::trace_millisecond_value(timings.project_parse_ms),
        crate::render::trace_millisecond_value(timings.semantic_validation_ms),
        crate::render::trace_millisecond_value(timings.preflight_ms),
        crate::render::trace_millisecond_value(timings.plan_compile_ms),
        crate::render::trace_millisecond_value(timings.asset_decode_ms),
        crate::render::trace_millisecond_value(timings.track_evaluation_ms),
        crate::render::trace_millisecond_value(timings.frame_render_ms),
        crate::render::trace_millisecond_value(timings.encoder_write_ms),
        crate::render::trace_millisecond_value(timings.encoder_finalize_ms),
        crate::render::trace_millisecond_value(timings.output_publish_ms),
        crate::render::trace_millisecond_value(timings.operation_total_ms),
    );
    if actual_backend == "wgpu" {
        tracing::debug!(
            target: "vestra.performance",
            operation_id = %operation_id,
            actual_backend,
            project_parse_ms = common.0,
            semantic_validation_ms = common.1,
            preflight_ms = common.2,
            plan_compile_ms = common.3,
            asset_decode_ms = common.4,
            audio_analysis_ms = crate::render::trace_millisecond_value(audio_analysis_ms),
            gpu_initialization_ms = crate::render::trace_millisecond_value(timings.gpu_initialization_ms.unwrap_or_default()),
            gpu_adapter_request_ms = crate::render::trace_millisecond_value(timings.gpu_adapter_request_ms.unwrap_or_default()),
            gpu_device_request_ms = crate::render::trace_millisecond_value(timings.gpu_device_request_ms.unwrap_or_default()),
            gpu_pipeline_creation_ms = crate::render::trace_millisecond_value(timings.gpu_pipeline_creation_ms.unwrap_or_default()),
            texture_upload_ms = crate::render::trace_millisecond_value(timings.texture_upload_ms.unwrap_or_default()),
            gpu_frame_command_encode_ms = crate::render::trace_millisecond_value(timings.gpu_frame_command_encode_ms.unwrap_or_default()),
            gpu_submission_ms = crate::render::trace_millisecond_value(timings.gpu_submission_ms.unwrap_or_default()),
            gpu_readback_wait_ms = crate::render::trace_millisecond_value(timings.gpu_readback_wait_ms.unwrap_or_default()),
            row_repack_ms = crate::render::trace_millisecond_value(timings.row_repack_ms.unwrap_or_default()),
            track_evaluation_ms = common.5,
            frame_render_ms = common.6,
            encoder_write_ms = common.7,
            encoder_finalize_ms = common.8,
            output_publish_ms = common.9,
            operation_total_ms = common.10,
            elapsed_ms = common.10,
            "render timing summary"
        );
    } else {
        tracing::debug!(
            target: "vestra.performance",
            operation_id = %operation_id,
            actual_backend,
            project_parse_ms = common.0,
            semantic_validation_ms = common.1,
            preflight_ms = common.2,
            plan_compile_ms = common.3,
            asset_decode_ms = common.4,
            audio_analysis_ms = crate::render::trace_millisecond_value(audio_analysis_ms),
            track_evaluation_ms = common.5,
            frame_render_ms = common.6,
            encoder_write_ms = common.7,
            encoder_finalize_ms = common.8,
            output_publish_ms = common.9,
            operation_total_ms = common.10,
            elapsed_ms = common.10,
            "render timing summary"
        );
    }
}

impl Editor {
    pub(crate) fn cancelled_editor_error(
        operation_started: Instant,
        total_frames: u64,
        output_path: PathBuf,
    ) -> EditorError {
        let operation_total_ms = operation_started.elapsed().as_millis();
        EditorError::Render {
            diagnostic: Box::new(Diagnostic::error(
                "VESTRA-CANCELLED",
                crate::Category::Cancellation,
                "render cancelled",
                "",
            )),
            warnings: Vec::new(),
            context: Box::new(RenderFailureContext {
                stage: crate::RenderFailureStage::Cancellation,
                last_completed_frame_index: None,
                completed_frames: 0,
                attempted_frame: None,
                total_frames,
                timeline_position: None,
                progress: (total_frames > 0).then_some(0.0),
                output_path: Some(output_path),
                temporary_output_path: None,
            }),
            temporary_removed: false,
            timings: crate::RenderTimings {
                operation_total_ms,
                ..crate::RenderTimings::default()
            },
        }
    }
    pub(super) fn project_error(error: LoadError) -> EditorError {
        match error {
            LoadError::Diagnostics(diagnostics) => {
                Self::diagnostic_error(diagnostics, crate::RenderTimings::default())
            }
        }
    }
    pub(super) fn render_error(
        error: ApplicationRenderError,
        project: &Project,
        operation_started: Instant,
        validation_elapsed: std::time::Duration,
        preflight_elapsed: std::time::Duration,
        warnings: Vec<Diagnostic>,
    ) -> EditorError {
        match error {
            ApplicationRenderError::Plan {
                diagnostic,
                plan_compile_elapsed_ms,
                ..
            } => EditorError::Plan {
                diagnostic: Box::new(diagnostic),
                warnings,
                timings: crate::RenderTimings {
                    plan_compile_ms: plan_compile_elapsed_ms,
                    ..Self::operation_timings(
                        project,
                        operation_started,
                        validation_elapsed,
                        preflight_elapsed,
                    )
                },
            },
            ApplicationRenderError::Render {
                error,
                plan_compile_elapsed_ms,
            } => {
                let error = *error;
                let mut all_warnings = warnings;
                all_warnings.extend(error.warnings);
                let mut timings = error.timings;
                timings.project_parse_ms = project.parse_elapsed().as_millis();
                timings.semantic_validation_ms = validation_elapsed.as_millis();
                timings.preflight_ms = preflight_elapsed.as_millis();
                timings.plan_compile_ms = plan_compile_elapsed_ms;
                timings.operation_total_ms = operation_started.elapsed().as_millis();
                EditorError::Render {
                    diagnostic: Box::new(error.diagnostic),
                    warnings: Self::operation_warnings(&all_warnings),
                    context: Box::new(error.context),
                    temporary_removed: error.temporary_removed,
                    timings,
                }
            }
        }
    }
    pub(super) fn diagnostic_error(
        diagnostics: Vec<Diagnostic>,
        timings: crate::RenderTimings,
    ) -> EditorError {
        let errors = diagnostics
            .iter()
            .filter(|item| item.severity == crate::Severity::Fatal)
            .cloned()
            .collect();
        let warnings = diagnostics
            .into_iter()
            .filter(|item| item.severity == crate::Severity::Warning)
            .collect();
        EditorError::Project {
            errors,
            warnings,
            timings,
        }
    }
    pub(super) fn operation_timings(
        project: &Project,
        operation_started: Instant,
        validation_elapsed: std::time::Duration,
        preflight_elapsed: std::time::Duration,
    ) -> crate::RenderTimings {
        let operation_total_ms = operation_started.elapsed().as_millis();
        crate::RenderTimings {
            project_parse_ms: project.parse_elapsed().as_millis(),
            operation_total_ms,
            semantic_validation_ms: validation_elapsed.as_millis(),
            preflight_ms: preflight_elapsed.as_millis(),
            ..crate::RenderTimings::default()
        }
    }
    pub(crate) fn apply_renderer_preparation_timings(
        timings: &mut crate::RenderTimings,
        preparation: crate::render::PreparationTimings,
    ) {
        timings.asset_decode_ms = preparation.decode.as_millis();
        if preparation.gpu_initialization != std::time::Duration::ZERO {
            timings.gpu_initialization_ms = Some(preparation.gpu_initialization.as_millis());
            timings.gpu_adapter_request_ms = Some(preparation.gpu_adapter_request.as_millis());
            timings.gpu_device_request_ms = Some(preparation.gpu_device_request.as_millis());
            timings.gpu_pipeline_creation_ms = Some(preparation.gpu_pipeline_creation.as_millis());
            timings.texture_upload_ms = Some(preparation.texture_upload.as_millis());
        }
    }
    pub(super) fn apply_error_preparation_timings(
        error: &mut EditorError,
        preparation: crate::render::PreparationTimings,
    ) {
        if let EditorError::Render { timings, .. } = error {
            Self::apply_renderer_preparation_timings(timings, preparation);
        }
    }
    pub(crate) fn operation_warnings(diagnostics: &[Diagnostic]) -> Vec<Diagnostic> {
        let mut warnings = Vec::new();
        for diagnostic in diagnostics
            .iter()
            .filter(|item| item.severity == crate::Severity::Warning)
        {
            let duplicate = warnings.iter().any(|existing: &Diagnostic| {
                existing.code == diagnostic.code
                    && existing.category == diagnostic.category
                    && existing.severity == diagnostic.severity
                    && existing.message == diagnostic.message
                    && existing.pointer == diagnostic.pointer
                    && existing.related_id == diagnostic.related_id
                    && existing.hint == diagnostic.hint
            });
            if !duplicate {
                warnings.push(diagnostic.clone());
            }
        }
        warnings
    }
}

pub(crate) fn finish_render_error(
    lifecycle: &mut LifecycleEmitter<'_>,
    operation_id: OperationId,
    output_path: &Path,
    operation_started: Instant,
    error: &EditorError,
) {
    if error.is_cancelled() {
        lifecycle.cancelled();
        tracing::info!(
            target: "vestra.render",
            operation_id = %operation_id,
            stage = %error.render_failure_context().map_or("render", |context| context.stage.as_str()),
            elapsed_ms = crate::render::trace_milliseconds(operation_started.elapsed()),
            "render cancelled"
        );
    } else {
        lifecycle.failed();
        log_render_failure(operation_id, output_path, operation_started, error);
    }
}

pub(crate) fn cancel_preparation(
    lifecycle: &mut LifecycleEmitter<'_>,
    operation_id: OperationId,
    operation_started: Instant,
    total_frames: u64,
    output_path: PathBuf,
) -> EditorError {
    lifecycle.cancelled();
    tracing::info!(
        target: "vestra.render",
        operation_id = %operation_id,
        stage = "preparing",
        elapsed_ms = crate::render::trace_milliseconds(operation_started.elapsed()),
        "render cancelled"
    );
    Editor::cancelled_editor_error(operation_started, total_frames, output_path)
}
