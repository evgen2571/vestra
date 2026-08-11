use std::collections::BTreeMap;

use crate::{
    plan::{CompileOptions, compile},
    project::{LoadError, Project, ValidatedProject},
};

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
    pub processed_audio_durations: BTreeMap<String, f64>,
}

pub fn inspect(
    _project: &Project,
    validated: ValidatedProject,
    preview: bool,
) -> Result<Inspection, LoadError> {
    let plan = compile(&validated, CompileOptions { preview })
        .map_err(|diagnostic| LoadError::Diagnostics(vec![diagnostic]))?;
    let processed_audio_durations = plan
        .audio_mix
        .tracks
        .iter()
        .flat_map(|track| track.clips.iter())
        .map(|clip| (clip.id.clone(), clip.processed_duration))
        .collect::<BTreeMap<_, _>>();
    let audio_end = validated.project.audio.as_ref().map(|_| {
        plan.audio_mix
            .tracks
            .iter()
            .flat_map(|track| track.clips.iter())
            .map(|clip| clip.start + clip.processed_duration)
            .fold(0.0, f64::max)
    });
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
        processed_audio_durations,
        validated,
    })
}
