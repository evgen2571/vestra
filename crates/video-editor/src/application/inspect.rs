use std::path::Path;

use crate::{
    Category, Diagnostic,
    plan::{CompileOptions, compile},
    project::{LoadError, ValidatedProject},
};

use super::validate_project;

#[derive(Clone, Debug)]
pub struct Inspection {
    pub validated: ValidatedProject,
    pub output_path: std::path::PathBuf,
    pub width: u32,
    pub height: u32,
    pub preview: bool,
    pub image_count: usize,
    pub audio_count: usize,
    pub audio_end: Option<f64>,
}

pub fn inspect(path: &Path, preview: bool) -> Result<Inspection, LoadError> {
    let validated = validate_project(path)?;
    let plan = compile(&validated, CompileOptions { preview })
        .map_err(|diagnostic| LoadError::Diagnostics(vec![diagnostic]))?;
    let audio_end = validated
        .project
        .audio
        .as_ref()
        .filter(|track| validated.project.output.audio && !track.mute)
        .map(|track| {
            let source_duration = validated.audio_durations.get(&track.asset).ok_or_else(|| {
                LoadError::Diagnostics(vec![Diagnostic::error(
                    "MVP-INSPECT-AUDIO",
                    Category::Internal,
                    "validated audio duration is missing",
                    "/audio/asset",
                )])
            })?;
            Ok(track.timeline_start
                + (track.trim_end.unwrap_or(*source_duration) - track.trim_start))
        })
        .transpose()?;
    Ok(Inspection {
        output_path: plan.configured_output,
        width: plan.canvas.width,
        height: plan.canvas.height,
        preview,
        image_count: plan.images.len(),
        audio_count: validated
            .project
            .assets
            .iter()
            .filter(|asset| matches!(asset.kind, crate::project::AssetType::Audio))
            .count(),
        audio_end,
        validated,
    })
}
