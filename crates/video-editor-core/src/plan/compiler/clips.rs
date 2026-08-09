//! Compilation of visible project clips into renderable layers.

use std::collections::BTreeMap;

use crate::{
    Category, Diagnostic,
    animation::Track,
    domain::{Crop, Point},
    plan::{
        CompilationStats, CompiledLayer, CompiledScalarProperty, CompiledSizing,
        CompiledTransformTracks, CompiledVisualSource, DrawKey, PlanCompileInput,
        ScalarPropertyConstraint,
    },
    project::{Clip, VisualSource, parse_colour},
};

use super::{assets, effects, output, time, tracks};

pub(super) fn compile(
    clip: &Clip,
    validated: &PlanCompileInput<'_>,
    image_indices: &BTreeMap<String, usize>,
    compilation: &mut CompilationStats,
) -> Result<CompiledLayer, Diagnostic> {
    let start_nanos = time::to_nanos(clip.start, &clip.id)?;
    let end_nanos = start_nanos.saturating_add(time::to_nanos(clip.duration, &clip.id)?);
    let source = match &clip.source {
        VisualSource::Image { asset } => CompiledVisualSource::Image {
            asset_index: assets::lookup(image_indices, asset, &clip.id)?,
            cacheable_crop: clip
                .crop
                .as_ref()
                .is_none_or(|track| track.keyframes.is_empty()),
            crop: match &clip.crop {
                Some(track) => tracks::compile(track, &clip.id)?,
                None => Track::new(Crop {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                }),
            },
            sizing: clip
                .sizing
                .as_ref()
                .map_or(CompiledSizing::Original, output::compile_sizing),
        },
        VisualSource::SolidColor { colour } => {
            compilation.parsed_colour_count += 1;
            CompiledVisualSource::SolidColor {
                colour: parse_colour(colour).ok_or_else(|| {
                    Diagnostic::error(
                        "MVP-PLAN-COLOUR",
                        Category::Internal,
                        "validated solid color is invalid",
                        "",
                    )
                })?,
            }
        }
    };
    let effects = clip
        .effects
        .iter()
        .map(|effect| effects::compile_timed(effect, &clip.id, clip.duration))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CompiledLayer {
        id: clip.id.clone(),
        start_nanos,
        duration_nanos: end_nanos - start_nanos,
        start_frame: time::first_frame_at_or_after(start_nanos, validated.frame_rate)?,
        end_frame: time::first_frame_at_or_after(end_nanos, validated.frame_rate)?
            .min(validated.frame_count),
        draw_key: DrawKey {
            layer: clip.layer,
            start_nanos,
            id: clip.id.clone(),
        },
        source,
        transform: compile_transform(clip)?,
        transform_contributions: Vec::new(),
        // Opacity is constrained only after generated transition/preset
        // contributions are applied during evaluation. Clamping inside the
        // scalar property would change the required ordering once modifiers
        // are present: authored -> modifiers -> generated contributions ->
        // final target constraint.
        opacity: crate::plan::CompiledScalarProperty::authored(tracks::compile(
            &clip.opacity,
            &clip.id,
        )?),
        opacity_contributions: Vec::new(),
        effects,
        blend_mode: clip.blend_mode,
        content_dependency: crate::plan::TemporalDependency::Static,
    })
}

fn compile_transform(clip: &Clip) -> Result<CompiledTransformTracks, Diagnostic> {
    match (&clip.source, &clip.transform) {
        (_, Some(transform)) => Ok(CompiledTransformTracks {
            position: tracks::compile(&transform.position, &clip.id)?,
            position_x_modifiers: Vec::new(),
            position_y_modifiers: Vec::new(),
            anchor: tracks::compile(&transform.anchor, &clip.id)?,
            scale: tracks::compile(&transform.scale, &clip.id)?,
            scale_x_modifiers: Vec::new(),
            scale_y_modifiers: Vec::new(),
            rotation_degrees: CompiledScalarProperty::constrained(
                tracks::compile(&transform.rotation_degrees, &clip.id)?,
                ScalarPropertyConstraint::Finite,
            ),
        }),
        (VisualSource::SolidColor { .. }, None) => Ok(canvas_transform()),
        (VisualSource::Image { .. }, None) => Err(Diagnostic::error(
            "MVP-PLAN-TRANSFORM",
            Category::Internal,
            format!(
                "validated image clip '{}' is missing its transform",
                clip.id
            ),
            "",
        )),
    }
}

pub(super) fn canvas_transform() -> CompiledTransformTracks {
    CompiledTransformTracks {
        position: Track::new(Point { x: 0.5, y: 0.5 }),
        position_x_modifiers: Vec::new(),
        position_y_modifiers: Vec::new(),
        anchor: Track::new(Point { x: 0.5, y: 0.5 }),
        scale: Track::new(Point { x: 1.0, y: 1.0 }),
        scale_x_modifiers: Vec::new(),
        scale_y_modifiers: Vec::new(),
        rotation_degrees: CompiledScalarProperty::constrained(
            Track::new(0.0),
            ScalarPropertyConstraint::Finite,
        ),
    }
}
