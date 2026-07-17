use std::{fs, path::Path};

use serde::Serialize;

use crate::{Diagnostic, render::RenderFailureContext};

pub fn write_report<T: Serialize>(path: &Path, report: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(report)
        .map_err(|error| format!("cannot serialize report: {error}"))?;
    fs::write(path, bytes).map_err(|error| format!("cannot write report: {error}"))
}

#[derive(Serialize)]
struct SuccessReport<'a, T: Serialize> {
    report_schema_version: u8,
    status: &'static str,
    command: &'static str,
    result: &'a T,
}

pub fn write_success_report<T: Serialize>(
    path: &Path,
    command: &'static str,
    result: &T,
) -> Result<(), String> {
    write_report(
        path,
        &SuccessReport {
            report_schema_version: 1,
            status: "success",
            command,
            result,
        },
    )
}

#[derive(Serialize)]
struct CommandFailureReport<'a> {
    report_schema_version: u8,
    status: &'static str,
    command: &'a str,
    failure_category: &'a str,
    failure_stage: &'a str,
    diagnostics: &'a [Diagnostic],
    #[serde(skip_serializing_if = "Option::is_none")]
    project_path: Option<&'a Path>,
    elapsed_ms: u128,
}

pub fn write_command_failure_report(
    path: &Path,
    command: &str,
    category: &str,
    stage: &str,
    diagnostics: &[Diagnostic],
    project_path: Option<&Path>,
    elapsed_ms: u128,
) -> Result<(), String> {
    write_report(
        path,
        &CommandFailureReport {
            report_schema_version: 1,
            status: "failure",
            command,
            failure_category: category,
            failure_stage: stage,
            diagnostics,
            project_path,
            elapsed_ms,
        },
    )
}

#[derive(Serialize)]
struct PlanFailureReport<'a> {
    report_schema_version: u8,
    status: &'static str,
    command: &'static str,
    failure_category: &'static str,
    failure_stage: &'static str,
    message: &'a str,
    diagnostics: [&'a Diagnostic; 1],
    project_path: &'a Path,
    warnings: &'a [Diagnostic],
    timings: PlanFailureTimings,
}

#[derive(Serialize)]
struct PlanFailureTimings {
    project_load_and_validation_ms: u128,
    plan_compile_ms: u128,
}

pub fn write_plan_failure_report(
    path: &Path,
    project_path: &Path,
    diagnostic: &Diagnostic,
    warnings: &[Diagnostic],
    validation_elapsed_ms: u128,
    plan_compile_elapsed_ms: u128,
) -> Result<(), String> {
    write_report(
        path,
        &PlanFailureReport {
            report_schema_version: 1,
            status: "failure",
            command: "render",
            failure_category: "plan",
            failure_stage: "plan_compilation",
            message: &diagnostic.message,
            diagnostics: [diagnostic],
            project_path,
            warnings,
            timings: PlanFailureTimings {
                project_load_and_validation_ms: validation_elapsed_ms,
                plan_compile_ms: plan_compile_elapsed_ms,
            },
        },
    )
}

#[derive(Serialize)]
struct RenderFailureReport<'a> {
    report_schema_version: u8,
    status: &'static str,
    command: &'static str,
    failure_category: String,
    failure_stage: &'a crate::render::RenderFailureStage,
    message: &'a str,
    diagnostics: [&'a Diagnostic; 1],
    project_path: &'a Path,
    #[serde(skip_serializing_if = "Option::is_none")]
    requested_output_path: Option<&'a Path>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temporary_output_path: Option<&'a Path>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_completed_frame_index: Option<u64>,
    completed_frames: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_frames: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    progress: Option<f64>,
    warnings: &'a [Diagnostic],
    failure_context: &'a RenderFailureContext,
    temporary_removed: bool,
    elapsed_ms: u128,
}

pub fn write_render_failure_report(
    path: &Path,
    project_path: &Path,
    diagnostic: &Diagnostic,
    context: &RenderFailureContext,
    warnings: &[Diagnostic],
    temporary_removed: bool,
    elapsed_ms: u128,
) -> Result<(), String> {
    write_report(
        path,
        &RenderFailureReport {
            report_schema_version: 1,
            status: "failure",
            command: "render",
            failure_category: format!("{:?}", diagnostic.category).to_ascii_lowercase(),
            failure_stage: &context.stage,
            message: &diagnostic.message,
            diagnostics: [diagnostic],
            project_path,
            requested_output_path: context.output_path.as_deref(),
            temporary_output_path: context.temporary_output_path.as_deref(),
            last_completed_frame_index: context.last_completed_frame_index,
            completed_frames: context.completed_frames,
            total_frames: Some(context.total_frames),
            progress: context.progress,
            warnings,
            failure_context: context,
            temporary_removed,
            elapsed_ms,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Category;

    #[test]
    fn plan_failure_report_preserves_diagnostics_warnings_and_timings() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let report_path = directory.path().join("report.json");
        let project_path = directory.path().join("project.json");
        let diagnostic = Diagnostic::error(
            "MVP-PLAN-ASSET",
            Category::Internal,
            "validated clip has no image asset",
            "/visual/clips/0",
        );
        let warnings = vec![Diagnostic::warning(
            "MVP-CLIP-HIDDEN",
            "clip is invisible",
            "/visual/clips/1",
        )];
        write_plan_failure_report(&report_path, &project_path, &diagnostic, &warnings, 11, 7)
            .expect("write report");
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(report_path).expect("read report"))
                .expect("report JSON");
        assert_eq!(report["failure_category"], "plan");
        assert_eq!(report["failure_stage"], "plan_compilation");
        assert_eq!(report["diagnostics"][0]["code"], "MVP-PLAN-ASSET");
        assert_eq!(report["warnings"][0]["code"], "MVP-CLIP-HIDDEN");
        assert_eq!(report["timings"]["project_load_and_validation_ms"], 11);
        assert_eq!(report["timings"]["plan_compile_ms"], 7);
    }
}
