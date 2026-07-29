use std::{
    fmt,
    path::{Path, PathBuf},
    time::Instant,
};

use crate::{
    CancellationToken, Diagnostic, InspectionReport, PreflightReport, Project, RenderEvent,
    RenderFailureContext, RenderResult, ValidationReport,
    application::{self, ApplicationRenderError, PreparationTimings, RenderRequest},
    project::{LoadError, ValidatedProject, ValidationOptions},
    render::RenderBackendPreference,
};

/// Reusable entry point for the supported SDK workflows.
#[derive(Clone, Debug, Default)]
pub struct Editor;

/// Builder for persistent SDK configuration. Phase 5 has no persistent
/// runtime options yet, but the builder provides the stable construction path.
#[derive(Clone, Debug, Default)]
pub struct EditorBuilder;

/// The operation whose environment requirements are being checked.
#[derive(Clone, Debug)]
pub enum PreflightTarget {
    /// Resolve only files and media required to inspect a project.
    Inspect,
    /// Check complete default render readiness for the CLI validation contract.
    Validate,
    /// Check an actual render destination and backend.
    Render {
        backend: RenderBackendPreference,
        output: Option<PathBuf>,
        overwrite: bool,
    },
}

/// Explicit, operation-aware environment checks. No accepted option is ignored.
#[derive(Clone, Debug)]
pub struct PreflightOptions {
    target: PreflightTarget,
}

impl PreflightOptions {
    #[must_use]
    pub fn for_inspection() -> Self {
        Self {
            target: PreflightTarget::Inspect,
        }
    }
    #[must_use]
    pub fn for_validation() -> Self {
        Self {
            target: PreflightTarget::Validate,
        }
    }
    #[must_use]
    pub fn for_render(
        backend: RenderBackendPreference,
        output: Option<PathBuf>,
        overwrite: bool,
    ) -> Self {
        Self {
            target: PreflightTarget::Render {
                backend,
                output,
                overwrite,
            },
        }
    }
}
impl Default for PreflightOptions {
    fn default() -> Self {
        Self::for_inspection()
    }
}

struct InternalPreflightOutcome {
    report: PreflightReport,
    resolved: Option<ValidatedProject>,
}

/// An SDK operation failed after parsing or during rendering.
#[derive(Debug)]
pub enum EditorError {
    Project {
        errors: Vec<Diagnostic>,
        warnings: Vec<Diagnostic>,
        timings: crate::RenderTimings,
    },
    Plan {
        diagnostic: Box<Diagnostic>,
        warnings: Vec<Diagnostic>,
        timings: crate::RenderTimings,
    },
    Render {
        diagnostic: Box<Diagnostic>,
        warnings: Vec<Diagnostic>,
        context: Box<RenderFailureContext>,
        temporary_removed: bool,
        timings: crate::RenderTimings,
    },
}

impl EditorError {
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            Self::Project { errors, .. } => errors,
            Self::Plan { diagnostic, .. } | Self::Render { diagnostic, .. } => {
                std::slice::from_ref(diagnostic)
            }
        }
    }
    #[must_use]
    pub fn warnings(&self) -> &[Diagnostic] {
        match self {
            Self::Project { warnings, .. } => warnings,
            Self::Plan { warnings, .. } | Self::Render { warnings, .. } => warnings,
        }
    }
}

impl fmt::Display for EditorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(diagnostic) = self.diagnostics().first() {
            formatter.write_str(&diagnostic.message)
        } else {
            formatter.write_str("editor operation failed")
        }
    }
}
impl std::error::Error for EditorError {}

impl Editor {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
    #[must_use]
    pub fn builder() -> EditorBuilder {
        EditorBuilder
    }
    #[expect(
        clippy::result_large_err,
        reason = "load diagnostics retain operation timings for CLI report output"
    )]
    pub fn load_project(&self, path: impl AsRef<Path>) -> Result<Project, EditorError> {
        Project::load(path).map_err(Self::project_error)
    }

    #[expect(
        clippy::result_large_err,
        reason = "validation diagnostics retain operation timings for CLI report output"
    )]
    pub fn validate_path(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<crate::ValidateResult, EditorError> {
        let project = self.load_project(path.as_ref())?;
        let report = self.validate(&project);
        if report.is_valid() {
            Ok(crate::ValidateResult {
                project: path.as_ref().to_path_buf(),
                warnings: report.warnings().cloned().collect(),
            })
        } else {
            Err(Self::diagnostic_error(
                report.diagnostics().to_vec(),
                crate::RenderTimings::default(),
            ))
        }
    }

    /// Validates only canonical project data. It does not read assets, run subprocesses, or
    /// initialize a rendering backend.
    #[must_use]
    pub fn validate(&self, project: &Project) -> ValidationReport {
        let diagnostics = video_editor_core::validation::validate(
            project.canonical(),
            video_editor_core::validation::ResourceLimits::default(),
        )
        .into_diagnostics();
        ValidationReport { diagnostics }
    }

    /// Resolves environment-dependent project requirements without terminal output.
    #[must_use]
    pub fn preflight(&self, project: &Project, options: PreflightOptions) -> PreflightReport {
        let validation = self.validate(project);
        self.run_preflight(project, &validation, &options).report
    }

    fn run_preflight(
        &self,
        project: &Project,
        validation: &ValidationReport,
        options: &PreflightOptions,
    ) -> InternalPreflightOutcome {
        let outcome = crate::project::validation::preflight(
            project,
            validation,
            &ValidationOptions::default(),
        );
        let mut diagnostics = outcome.diagnostics;
        let target = &options.target;
        let render_target = match target {
            PreflightTarget::Render {
                backend,
                output,
                overwrite,
            } => Some((*backend, output.clone(), *overwrite)),
            PreflightTarget::Validate => Some((RenderBackendPreference::Auto, None, false)),
            PreflightTarget::Inspect => None,
        };
        if let Some((backend, override_output, overwrite)) = render_target {
            let output = override_output.unwrap_or_else(|| {
                let configured = PathBuf::from(&project.canonical().output.path);
                if configured.is_absolute() {
                    configured
                } else {
                    project.base_directory().join(configured)
                }
            });
            if let Err(error) = video_editor_media::check_output(&output, overwrite) {
                diagnostics.push(Diagnostic::error(
                    "MVP-OUTPUT-PATH",
                    crate::Category::Output,
                    error.to_string(),
                    "/output/path",
                ));
            }
            if let Err(error) = video_editor_media::check_ffmpeg_available(None) {
                diagnostics.push(Diagnostic::error(
                    "MVP-ENCODER-UNAVAILABLE",
                    crate::Category::Backend,
                    format!("FFmpeg is unavailable: {error}"),
                    "",
                ));
            }
            let probe = video_editor_render::probe_backend(match backend {
                RenderBackendPreference::Auto => video_editor_render::RenderBackendPreference::Auto,
                RenderBackendPreference::Cpu => video_editor_render::RenderBackendPreference::Cpu,
                RenderBackendPreference::Wgpu => video_editor_render::RenderBackendPreference::Wgpu,
            });
            diagnostics.extend(probe.diagnostics);
        }
        let has_errors = diagnostics
            .iter()
            .any(|item| item.severity == crate::Severity::Fatal);
        InternalPreflightOutcome {
            report: PreflightReport { diagnostics },
            resolved: if has_errors { None } else { outcome.resolved },
        }
    }

    #[expect(
        clippy::result_large_err,
        reason = "inspection diagnostics retain operation timings for CLI report output"
    )]
    pub fn inspect(
        &self,
        project: &Project,
        preview: bool,
    ) -> Result<InspectionReport, EditorError> {
        let validation = self.validate(project);
        let outcome = self.run_preflight(project, &validation, &PreflightOptions::for_inspection());
        let validated = outcome.resolved.ok_or_else(|| {
            Self::diagnostic_error(outcome.report.diagnostics, crate::RenderTimings::default())
        })?;
        application::inspect(project, validated, preview)
            .map(|inspection| {
                application::inspect_result(Self::project_display_path(project), inspection)
            })
            .map_err(Self::project_error)
    }

    #[expect(
        clippy::result_large_err,
        reason = "render diagnostics retain operation timings for CLI report output"
    )]
    pub fn render(
        &self,
        project: &Project,
        request: SdkRenderRequest,
        emit: &mut dyn FnMut(RenderEvent),
        cancellation: &CancellationToken,
    ) -> Result<RenderResult, EditorError> {
        let operation_started = Instant::now();
        let validation_started = Instant::now();
        let validation = self.validate(project);
        let validation_elapsed = validation_started.elapsed();
        // Keep the one-shot SDK's established operation preflight contract.
        // The private renderer preparation that follows remains independent of
        // FFmpeg and output paths, so a future prepared API can call it alone.
        let options = PreflightOptions::for_render(
            request.backend,
            request.output.clone(),
            request.overwrite,
        );
        let preflight_started = Instant::now();
        let preflight = self.run_preflight(project, &validation, &options);
        let preflight_elapsed = preflight_started.elapsed();
        let mut warnings = Self::operation_warnings(&preflight.report.diagnostics);
        let validated = preflight.resolved.ok_or_else(|| {
            Self::diagnostic_error(
                preflight.report.diagnostics,
                Self::operation_timings(
                    project,
                    operation_started,
                    validation_elapsed,
                    preflight_elapsed,
                ),
            )
        })?;
        let render_request = RenderRequest {
            output_override: request.output,
            overwrite: request.overwrite,
            preview: request.preview,
            cancelled: cancellation.flag(),
            backend_preference: request.backend,
        };
        let mut prepared = application::prepare_project(
            validated,
            &render_request,
            warnings.clone(),
            PreparationTimings {
                validation_ms: validation_elapsed.as_millis(),
                preflight_ms: preflight_elapsed.as_millis(),
                ..PreparationTimings::default()
            },
        )
        .map_err(|error| {
            Self::render_error(
                error,
                project,
                operation_started,
                validation_elapsed,
                preflight_elapsed,
                warnings.clone(),
            )
        })?;
        let operation_preparation = prepared.preparation_timings();
        let mut summary =
            match application::render_prepared_project(&mut prepared, render_request, emit) {
                Ok(summary) => summary,
                Err(error) => {
                    let mut editor_error = Self::render_error(
                        error,
                        project,
                        operation_started,
                        validation_elapsed,
                        preflight_elapsed,
                        warnings.clone(),
                    );
                    Self::apply_error_preparation_timings(
                        &mut editor_error,
                        operation_preparation.renderer,
                    );
                    return Err(editor_error);
                }
            };
        let preparation_timings = prepared.preparation_timings();
        summary.timings.plan_compile_ms = preparation_timings.plan_compile_ms;
        summary.timings.project_parse_ms = project.parse_elapsed().as_millis();
        summary.timings.semantic_validation_ms = preparation_timings.validation_ms;
        summary.timings.preflight_ms = preparation_timings.preflight_ms;
        Self::apply_renderer_preparation_timings(
            &mut summary.timings,
            preparation_timings.renderer,
        );
        summary.timings.operation_total_ms = operation_started.elapsed().as_millis();
        summary.timings.total_ms = summary.timings.operation_total_ms;
        summary.elapsed_ms = summary.timings.operation_total_ms;
        warnings = prepared.preparation_warnings().to_vec();
        if let Some(fallback) = summary.backend_fallback.as_ref() {
            warnings.push(crate::render::backend_fallback_warning(fallback));
            warnings = Self::operation_warnings(&warnings);
        }
        Ok(application::render_result(
            Self::project_display_path(project),
            prepared.result_metadata(),
            summary,
            warnings,
        ))
    }

    #[must_use]
    pub const fn version(&self) -> crate::VersionResult {
        application::version_result()
    }

    fn project_display_path(project: &Project) -> &Path {
        project.source_path().unwrap_or(project.base_directory())
    }
    fn project_error(error: LoadError) -> EditorError {
        match error {
            LoadError::Diagnostics(diagnostics) => {
                Self::diagnostic_error(diagnostics, crate::RenderTimings::default())
            }
        }
    }
    fn render_error(
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
                timings.total_ms = timings.operation_total_ms;
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
    fn diagnostic_error(
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
    fn operation_timings(
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
            total_ms: operation_total_ms,
            ..crate::RenderTimings::default()
        }
    }
    fn apply_renderer_preparation_timings(
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
    fn apply_error_preparation_timings(
        error: &mut EditorError,
        preparation: crate::render::PreparationTimings,
    ) {
        if let EditorError::Render { timings, .. } = error {
            Self::apply_renderer_preparation_timings(timings, preparation);
        }
    }
    fn operation_warnings(diagnostics: &[Diagnostic]) -> Vec<Diagnostic> {
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

impl EditorBuilder {
    #[expect(
        clippy::result_large_err,
        reason = "builder keeps the SDK error type consistent with other entry points"
    )]
    pub fn build(self) -> Result<Editor, EditorError> {
        Ok(Editor::new())
    }
}

/// SDK-owned rendering inputs. It intentionally contains no terminal options.
#[derive(Clone, Debug, Default)]
pub struct SdkRenderRequest {
    pub output: Option<PathBuf>,
    pub overwrite: bool,
    pub preview: bool,
    pub backend: RenderBackendPreference,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_failure_keeps_compilation_timing_and_deduplicates_fallback_warning() {
        let project = Project::from_json(
            r##"{"schema_version":1,"output":{"path":"out.mp4","width":2,"height":2,"frame_rate":1,"background":"#000000","quality":"preview","audio":false,"duration_mode":"explicit","duration":1},"assets":[],"visual":{"clips":[]}}"##,
            ".",
        )
        .expect("project");
        let fallback = crate::render::backend_fallback_warning(&crate::BackendFallback {
            code: "WGPU-PIPELINE-CREATION".to_owned(),
            stage: "wgpu_preparation".to_owned(),
            message: "injected pipeline preparation failure".to_owned(),
        });
        let error = crate::render::RenderError {
            diagnostic: Diagnostic::error("MVP-RENDER", crate::Category::Render, "failed", ""),
            warnings: vec![fallback.clone()],
            temporary_removed: true,
            context: crate::RenderFailureContext {
                stage: crate::RenderFailureStage::FrameComposition,
                last_completed_frame_index: None,
                completed_frames: 0,
                attempted_frame: Some(0),
                total_frames: 1,
                timeline_position: Some(0.0),
                progress: Some(0.0),
                output_path: None,
                temporary_output_path: None,
            },
            timings: crate::RenderTimings {
                asset_decode_ms: 17,
                ..crate::RenderTimings::default()
            },
        };
        let result = Editor::render_error(
            ApplicationRenderError::Render {
                error: Box::new(error),
                plan_compile_elapsed_ms: 23,
            },
            &project,
            Instant::now(),
            std::time::Duration::from_millis(5),
            std::time::Duration::from_millis(7),
            vec![fallback],
        );

        let EditorError::Render {
            warnings, timings, ..
        } = result
        else {
            panic!("expected render error");
        };
        assert_eq!(warnings.len(), 1);
        assert_eq!(timings.asset_decode_ms, 17);
        assert_eq!(timings.plan_compile_ms, 23);
        assert_eq!(timings.semantic_validation_ms, 5);
        assert_eq!(timings.preflight_ms, 7);
    }

    #[test]
    fn renderer_preparation_timing_snapshot_is_merged_without_operation_timing() {
        let mut timings = crate::RenderTimings {
            frame_render_ms: 31,
            ..crate::RenderTimings::default()
        };
        Editor::apply_renderer_preparation_timings(
            &mut timings,
            crate::render::PreparationTimings {
                decode: std::time::Duration::from_millis(11),
                gpu_initialization: std::time::Duration::from_millis(13),
                gpu_adapter_request: std::time::Duration::from_millis(17),
                gpu_device_request: std::time::Duration::from_millis(19),
                gpu_pipeline_creation: std::time::Duration::from_millis(23),
                texture_upload: std::time::Duration::from_millis(29),
                ..crate::render::PreparationTimings::default()
            },
        );
        assert_eq!(timings.asset_decode_ms, 11);
        assert_eq!(timings.gpu_initialization_ms, Some(13));
        assert_eq!(timings.gpu_adapter_request_ms, Some(17));
        assert_eq!(timings.gpu_device_request_ms, Some(19));
        assert_eq!(timings.gpu_pipeline_creation_ms, Some(23));
        assert_eq!(timings.texture_upload_ms, Some(29));
        assert_eq!(timings.frame_render_ms, 31);
    }

    #[test]
    fn prepared_render_failure_retains_renderer_preparation_timing_snapshot() {
        let mut error = EditorError::Render {
            diagnostic: Box::new(Diagnostic::error(
                "MVP-RENDER",
                crate::Category::Render,
                "failed",
                "",
            )),
            warnings: Vec::new(),
            context: Box::new(crate::RenderFailureContext {
                stage: crate::RenderFailureStage::FrameComposition,
                last_completed_frame_index: None,
                completed_frames: 0,
                attempted_frame: None,
                total_frames: 1,
                timeline_position: None,
                progress: Some(0.0),
                output_path: None,
                temporary_output_path: None,
            }),
            temporary_removed: true,
            timings: crate::RenderTimings::default(),
        };
        Editor::apply_error_preparation_timings(
            &mut error,
            crate::render::PreparationTimings {
                decode: std::time::Duration::from_millis(37),
                gpu_initialization: std::time::Duration::from_millis(41),
                ..crate::render::PreparationTimings::default()
            },
        );
        let EditorError::Render { timings, .. } = error else {
            panic!("expected render error");
        };
        assert_eq!(timings.asset_decode_ms, 37);
        assert_eq!(timings.gpu_initialization_ms, Some(41));
    }
}
