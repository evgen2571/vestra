//! Shared fixture loading for renderer unit tests.

use std::collections::BTreeMap;

use vestra_core::{plan::PlanCompileInput, project::Project};

use crate::{Category, Diagnostic};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ValidationOptions {
    pub(crate) check_backend: bool,
}

pub(crate) fn load_and_validate(
    path: &std::path::Path,
    options: &ValidationOptions,
) -> Result<PlanCompileInput<'static>, Diagnostic> {
    let _ = options.check_backend;
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
    };
    let bytes = std::fs::read(&path).map_err(|error| {
        Diagnostic::error(
            "VESTRA-PROJECT-READ",
            Category::Project,
            format!("cannot read project: {error}"),
            "",
        )
    })?;
    let project = Box::leak(Box::new(
        serde_json::from_slice::<Project>(&bytes).map_err(|error| {
            Diagnostic::error(
                "VESTRA-PROJECT-SHAPE",
                Category::Project,
                format!("project does not match the canonical format: {error}"),
                "",
            )
        })?,
    ));
    let root = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let asset_paths = Box::leak(Box::new(
        project
            .assets
            .iter()
            .map(|asset| (asset.id.clone(), root.join(&asset.source)))
            .collect::<BTreeMap<_, _>>(),
    ));
    let audio_durations = Box::leak(Box::new(BTreeMap::new()));
    let duration = project
        .visual
        .clips
        .iter()
        .map(|clip| clip.start + clip.duration)
        .fold(0.0_f64, f64::max);
    let frame_rate = project.output.frame_rate.rational().map_err(|message| {
        Diagnostic::error("VESTRA-OUTPUT-FPS", Category::Semantic, message, "")
    })?;
    let frame_count = vestra_core::timeline::frame_count(
        vestra_core::timeline::seconds_to_nanos(duration).unwrap_or(0),
        frame_rate.0,
        frame_rate.1,
    )
    .map_err(|_| {
        Diagnostic::error(
            "VESTRA-TIMELINE-OVERFLOW",
            Category::Semantic,
            "project duration or frame rate cannot be represented safely",
            "",
        )
    })?;
    Ok(PlanCompileInput::new(
        project,
        vestra_core::validation::ResourceLimits::default(),
        Box::leak(path.into_boxed_path()),
        asset_paths,
        audio_durations,
        duration,
        frame_rate,
        frame_count,
        &[],
    ))
}
