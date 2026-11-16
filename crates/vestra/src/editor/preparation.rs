//! Shared validation, preflight and backend preparation.
#[cfg(test)]
use super::test_preflight_warning;
use super::{Editor, EditorError, PreflightOptions, SdkRenderRequest};
use crate::{
    Diagnostic, PrepareOptions, Project,
    application::{self, PreparationTimings, RenderRequest},
    project::ValidatedProject,
    render::RenderBackendPreference,
};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

struct PreparationContext {
    warnings: Vec<Diagnostic>,
    timings: PreparationTimings,
    operation_started: Instant,
    validation_elapsed: std::time::Duration,
    preflight_elapsed: std::time::Duration,
}

/// The one internal preparation coordinator serves both public reusable
/// preparation and the one-shot render path. Only preflight
/// target selection differs; validation through backend construction is shared.
#[derive(Clone, Copy)]
pub(super) enum InternalPreparationTarget<'a> {
    Public(PrepareOptions),
    OneShot(&'a SdkRenderRequest),
}

pub(super) struct CoordinatedPreparation {
    pub(super) prepared: application::PreparedRender,
    pub(super) started: Instant,
}

impl Editor {
    #[expect(
        clippy::result_large_err,
        reason = "preparation failures retain diagnostics and timing context"
    )]
    pub(super) fn prepare_internal(
        &self,
        project: &Project,
        target: InternalPreparationTarget<'_>,
        cancelled: &AtomicBool,
    ) -> Result<CoordinatedPreparation, EditorError> {
        let _prepare_span = tracing::debug_span!(
            target: "vestra.render",
            "prepare",
            stage = "prepare"
        )
        .entered();
        let started = Instant::now();
        let (frame_rate, quality, preview, resolution, silent) = match target {
            InternalPreparationTarget::Public(options) => (
                options.frame_rate(),
                options.quality(),
                false,
                options.resolution(),
                false,
            ),
            InternalPreparationTarget::OneShot(request) => (
                request.frame_rate,
                request.quality,
                request.preview,
                request.resolution,
                vestra_media::is_animated_output(&Self::render_output_path(project, request)),
            ),
        };
        let effective_project = project.with_render_overrides(frame_rate, quality, preview, silent);
        let project = effective_project.as_ref();
        let validation_started = Instant::now();
        let validation = self.validate(project);
        let validation_elapsed = validation_started.elapsed();
        tracing::debug!(
            target: "vestra.project",
            stage = "validate",
            elapsed_ms = crate::render::trace_milliseconds(validation_elapsed),
            "project validation completed"
        );
        let preflight_started = Instant::now();
        let (options, backend, preview) = match target {
            InternalPreparationTarget::Public(options) => (
                PreflightOptions::for_preparation(options.backend()),
                options.backend(),
                false,
            ),
            InternalPreparationTarget::OneShot(request) => (
                PreflightOptions::for_render(
                    request.backend,
                    request.output.clone(),
                    request.overwrite,
                ),
                request.backend,
                request.preview,
            ),
        };
        let outcome = self.run_preflight(project, &validation, &options, cancelled);
        let preflight_elapsed = preflight_started.elapsed();
        tracing::debug!(
            target: "vestra.project",
            stage = "preflight",
            requested_backend = backend.as_str(),
            elapsed_ms = crate::render::trace_milliseconds(preflight_elapsed),
            diagnostics = outcome.report.diagnostics.len(),
            "render preflight completed"
        );
        let warnings = Self::operation_warnings(&outcome.report.diagnostics);
        #[cfg(test)]
        let warnings = {
            let mut warnings = warnings;
            if let Some(warning) = test_preflight_warning() {
                warnings.push(warning);
            }
            Self::operation_warnings(&warnings)
        };
        let validated = outcome
            .resolved
            .filter(|_| !cancelled.load(Ordering::Relaxed))
            .ok_or_else(|| {
                Self::diagnostic_error(
                    outcome.report.diagnostics,
                    Self::operation_timings(
                        project,
                        started,
                        validation_elapsed,
                        preflight_elapsed,
                    ),
                )
            })?;
        let prepared = self.prepare_validated(
            project,
            validated,
            backend,
            preview,
            resolution,
            PreparationContext {
                warnings: warnings.clone(),
                timings: PreparationTimings {
                    validation_ms: validation_elapsed.as_millis(),
                    preflight_ms: preflight_elapsed.as_millis(),
                    ..PreparationTimings::default()
                },
                operation_started: started,
                validation_elapsed,
                preflight_elapsed,
            },
        )?;
        Ok(CoordinatedPreparation { prepared, started })
    }

    #[expect(
        clippy::result_large_err,
        reason = "the shared coordinator preserves preparation diagnostics"
    )]
    fn prepare_validated(
        &self,
        project: &Project,
        validated: ValidatedProject,
        backend: RenderBackendPreference,
        preview: bool,
        resolution: Option<(u32, u32)>,
        context: PreparationContext,
    ) -> Result<application::PreparedRender, EditorError> {
        let request = RenderRequest {
            frame_range: None,
            resolution,
            output_override: None,
            overwrite: false,
            preview,
            cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            backend_preference: backend,
        };
        application::prepare_project(
            validated,
            &request,
            context.warnings.clone(),
            context.timings,
            self.ffmpeg_executable.clone(),
        )
        .map_err(|error| {
            Self::render_error(
                error,
                project,
                context.operation_started,
                context.validation_elapsed,
                context.preflight_elapsed,
                context.warnings,
            )
        })
    }
}
