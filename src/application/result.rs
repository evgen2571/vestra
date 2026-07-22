use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    Diagnostic,
    application::{Inspection, validate_project},
    project::LoadError,
    render::{RenderSummary, RenderTimings},
};

#[derive(Clone, Debug, Serialize)]
pub struct ValidateResult {
    pub project: PathBuf,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Serialize)]
pub struct VersionResult {
    pub editor_version: &'static str,
}

#[must_use]
pub const fn version_result() -> VersionResult {
    VersionResult {
        editor_version: env!("CARGO_PKG_VERSION"),
    }
}

pub fn validate_result(path: &Path) -> Result<ValidateResult, LoadError> {
    let validated = validate_project(path)?;
    Ok(ValidateResult {
        project: path.to_path_buf(),
        warnings: validated.warnings,
    })
}

#[derive(Clone, Debug, Serialize)]
pub struct InspectResult {
    pub project: PathBuf,
    pub name: Option<String>,
    pub output: InspectOutput,
    pub assets: InspectAssets,
    pub visual_clips: usize,
    pub flashes: usize,
    pub transitions: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<InspectAudio>,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Serialize)]
pub struct InspectOutput {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub frame_rate: String,
    pub duration_mode: crate::project::DurationMode,
    pub duration: f64,
    pub total_frames: u64,
    pub preview: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct InspectAssets {
    pub images: usize,
    pub audio: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct InspectAudio {
    pub asset: String,
    pub start: f64,
    pub end: f64,
}

pub fn inspect_result(path: &Path, inspection: Inspection) -> InspectResult {
    let audio = inspection
        .validated
        .project
        .audio
        .as_ref()
        .and_then(|track| {
            (inspection.validated.project.output.audio && !track.mute).then(|| InspectAudio {
                asset: track.asset.clone(),
                start: track.timeline_start,
                end: inspection.audio_end.unwrap_or(track.timeline_start),
            })
        });
    InspectResult {
        project: path.to_path_buf(),
        name: inspection.validated.project.name.clone(),
        output: InspectOutput {
            path: inspection.output_path,
            width: inspection.width,
            height: inspection.height,
            frame_rate: inspection.validated.project.output.frame_rate.display(),
            duration_mode: inspection.validated.project.output.duration_mode,
            duration: inspection.validated.duration,
            total_frames: inspection.validated.frame_count,
            preview: inspection.preview,
        },
        assets: InspectAssets {
            images: inspection.image_count,
            audio: inspection.audio_count,
        },
        visual_clips: inspection.validated.visual_counts().0,
        flashes: inspection.validated.visual_counts().1,
        transitions: inspection.validated.visual_counts().2,
        audio,
        warnings: inspection.validated.warnings,
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderResult {
    pub editor_version: &'static str,
    pub project: PathBuf,
    pub output: PathBuf,
    pub width: u32,
    pub height: u32,
    pub frame_rate: String,
    pub duration: f64,
    pub total_frames: u64,
    pub visual_clip_count: usize,
    pub audio_present: bool,
    pub preview: bool,
    pub elapsed_ms: u128,
    pub timings: RenderTimings,
    pub performance: crate::render::PreparationStats,
    pub render_backend: &'static str,
    pub encoder_backend: &'static str,
    pub warnings: Vec<Diagnostic>,
}

pub fn render_result(
    project: &Path,
    validated: crate::project::ValidatedProject,
    summary: RenderSummary,
) -> RenderResult {
    RenderResult {
        editor_version: env!("CARGO_PKG_VERSION"),
        project: project.to_path_buf(),
        output: summary.output_path,
        width: summary.width,
        height: summary.height,
        frame_rate: validated.project.output.frame_rate.display(),
        duration: summary.duration,
        total_frames: summary.frame_count,
        visual_clip_count: validated.visual_counts().0,
        audio_present: summary.audio_present,
        preview: summary.preview,
        elapsed_ms: summary.elapsed_ms,
        timings: summary.timings,
        performance: summary.performance,
        render_backend: "cpu",
        encoder_backend: "ffmpeg",
        warnings: validated.warnings,
    }
}
