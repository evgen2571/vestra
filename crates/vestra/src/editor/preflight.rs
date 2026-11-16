//! Environment-dependent preflight checks.
use super::{Editor, PreflightOptions, PreflightTarget};
use crate::{
    Diagnostic, PreflightReport, Project, ValidationReport,
    project::{ValidatedProject, ValidationOptions},
    render::RenderBackendPreference,
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) struct InternalPreflightOutcome {
    pub(super) report: PreflightReport,
    pub(super) resolved: Option<ValidatedProject>,
}

impl Editor {
    pub(super) fn run_preflight(
        &self,
        project: &Project,
        validation: &ValidationReport,
        options: &PreflightOptions,
        cancelled: &AtomicBool,
    ) -> InternalPreflightOutcome {
        let outcome = crate::project::validation::preflight(
            project,
            validation,
            &ValidationOptions {
                limits: self.limits,
            },
            cancelled,
        );
        if cancelled.load(Ordering::Relaxed) {
            return InternalPreflightOutcome {
                report: PreflightReport {
                    diagnostics: outcome.diagnostics,
                },
                resolved: None,
            };
        }
        let mut diagnostics = outcome.diagnostics;
        let target = &options.target;
        let render_target = match target {
            PreflightTarget::Render {
                backend,
                output,
                overwrite,
            } => Some((*backend, output.clone(), *overwrite)),
            PreflightTarget::Validate => Some((RenderBackendPreference::Auto, None, false)),
            PreflightTarget::Preparation { backend } => {
                let probe = vestra_render::probe_backend(match backend {
                    RenderBackendPreference::Auto => vestra_render::RenderBackendPreference::Auto,
                    RenderBackendPreference::Cpu => vestra_render::RenderBackendPreference::Cpu,
                    RenderBackendPreference::Wgpu => vestra_render::RenderBackendPreference::Wgpu,
                });
                diagnostics.extend(probe.diagnostics);
                None
            }
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
            if let Err(error) = vestra_media::check_output(&output, overwrite) {
                diagnostics.push(Diagnostic::error(
                    "VESTRA-OUTPUT-PATH",
                    crate::Category::Output,
                    error.to_string(),
                    "/output/path",
                ));
            }
            if let Err(error) = vestra_media::check_output_encoder_available(
                &output,
                self.ffmpeg_executable.as_deref(),
            ) {
                let message = self.ffmpeg_executable.as_ref().map_or_else(
                    || format!("FFmpeg is unavailable: {error}"),
                    |path| format!("FFmpeg is unavailable ({}): {error}", path.display()),
                );
                diagnostics.push(Diagnostic::error(
                    "VESTRA-ENCODER-UNAVAILABLE",
                    crate::Category::Backend,
                    message,
                    "",
                ));
            }
            let probe = vestra_render::probe_backend(match backend {
                RenderBackendPreference::Auto => vestra_render::RenderBackendPreference::Auto,
                RenderBackendPreference::Cpu => vestra_render::RenderBackendPreference::Cpu,
                RenderBackendPreference::Wgpu => vestra_render::RenderBackendPreference::Wgpu,
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
}
