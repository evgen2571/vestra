//! Compilation of visible project clips into renderable layers.

use std::{collections::BTreeMap, sync::Arc};

use crate::{
    Category, Diagnostic,
    animation::Track,
    domain::{Crop, Point},
    plan::{
        CompilationStats, CompiledComposition, CompiledLayer, CompiledScalarProperty,
        CompiledSizing, CompiledTransformTracks, CompiledVisualSource, DrawKey, PlanCompileInput,
        ScalarPropertyConstraint, ScalarPropertyTarget, ScalarSignalInterner,
    },
    project::{Clip, VisualSource, parse_colour},
};

use super::{assets, effects, output, time, tracks};

#[derive(Clone, Copy)]
struct GroupTiming {
    duration: f64,
    start_nanos: u128,
    parent_visible_window: (u128, u128),
}

#[expect(
    clippy::too_many_arguments,
    reason = "recursive compiler keeps validated inputs and shared source tables explicit"
)]
pub(super) fn compile(
    clip: &Clip,
    validated: &PlanCompileInput<'_>,
    image_indices: &BTreeMap<String, usize>,
    video_indices: &BTreeMap<String, usize>,
    font_indices: &BTreeMap<String, usize>,
    shapes: &mut Vec<crate::project::ShapeSource>,
    texts: &mut Vec<crate::project::TextSource>,
    compilation: &mut CompilationStats,
    scalar_signal_interner: &mut ScalarSignalInterner,
    next_compiled_identity: &mut usize,
    next_video_slot_index: &mut usize,
    effective_visible_window: (u128, u128),
) -> Result<CompiledLayer, Diagnostic> {
    let compiled_identity = *next_compiled_identity;
    *next_compiled_identity = (*next_compiled_identity).saturating_add(1);
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
        VisualSource::Video { asset } => CompiledVisualSource::Video {
            asset_index: assets::lookup_video(video_indices, asset, &clip.id)?,
            video_slot_index: {
                let slot = *next_video_slot_index;
                *next_video_slot_index =
                    (*next_video_slot_index).checked_add(1).ok_or_else(|| {
                        Diagnostic::error(
                            "VESTRA-PLAN-VIDEO-SLOTS",
                            Category::Internal,
                            "compiled Video slot count overflows usize",
                            "",
                        )
                    })?;
                slot
            },
            source_start: clip.source_start,
            playback_rate: clip.playback_rate,
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
                        "VESTRA-PLAN-COLOUR",
                        Category::Internal,
                        "validated solid color is invalid",
                        "",
                    )
                })?,
            }
        }
        VisualSource::Shape(shape) => {
            let shape_index = image_indices.len() + shapes.len();
            shapes.push(shape.clone());
            CompiledVisualSource::Shape { shape_index }
        }
        VisualSource::Text(text) => {
            let _ = assets::lookup_font(font_indices, &text.font, &clip.id)?;
            let text_index = image_indices.len() + shapes.len() + texts.len();
            texts.push(text.clone());
            CompiledVisualSource::Text { text_index }
        }
        VisualSource::Spectrum2D(spectrum) => {
            compilation.parsed_colour_count += 1;
            let colour = parse_colour(&spectrum.colour).ok_or_else(|| {
                Diagnostic::error(
                    "VESTRA-PLAN-SPECTRUM2D-COLOUR",
                    Category::Internal,
                    "validated Spectrum2D colour is invalid",
                    "",
                )
            })?;
            let gradient = spectrum
                .gradient
                .as_ref()
                .map(|value| {
                    Ok::<_, Diagnostic>((
                        value.direction,
                        parse_colour(&value.start_colour).ok_or_else(|| {
                            Diagnostic::error(
                                "VESTRA-PLAN-SPECTRUM2D-GRADIENT",
                                Category::Internal,
                                "validated Spectrum2D gradient start colour is invalid",
                                "",
                            )
                        })?,
                        parse_colour(&value.end_colour).ok_or_else(|| {
                            Diagnostic::error(
                                "VESTRA-PLAN-SPECTRUM2D-GRADIENT",
                                Category::Internal,
                                "validated Spectrum2D gradient end colour is invalid",
                                "",
                            )
                        })?,
                    ))
                })
                .transpose()?;
            let band_signals = spectrum
                .logarithmic_bands()
                .into_iter()
                .map(|(min_hz, max_hz)| {
                    let band =
                        crate::plan::AudioFrequencyBand::new(min_hz, max_hz).map_err(|error| {
                            Diagnostic::error(
                                "VESTRA-PLAN-SPECTRUM2D-BAND",
                                Category::Internal,
                                error.to_string(),
                                "",
                            )
                        })?;
                    let signal = crate::plan::CompiledScalarSignal::new(
                        crate::plan::RawScalarSignal::Audio(crate::plan::AudioScalarSignal {
                            tap: crate::plan::AudioAnalysisTap::Master,
                            feature: crate::plan::AudioScalarFeature::BandEnergy(band),
                        }),
                        vec![
                            crate::plan::CompiledSignalTransform::Gain(
                                crate::plan::GainTransform::new(spectrum.sensitivity).map_err(
                                    |error| {
                                        Diagnostic::error(
                                            "VESTRA-PLAN-SPECTRUM2D-RESPONSE",
                                            Category::Internal,
                                            error.to_string(),
                                            "",
                                        )
                                    },
                                )?,
                            ),
                            crate::plan::CompiledSignalTransform::Clamp(
                                crate::plan::ClampTransform::new(0.0, 1.0).map_err(|error| {
                                    Diagnostic::error(
                                        "VESTRA-PLAN-SPECTRUM2D-RESPONSE",
                                        Category::Internal,
                                        error.to_string(),
                                        "",
                                    )
                                })?,
                            ),
                            crate::plan::CompiledSignalTransform::Envelope(
                                crate::plan::EnvelopeTransform::new(
                                    time::to_nanos(spectrum.attack_seconds, "Spectrum2D attack")?,
                                    time::to_nanos(spectrum.release_seconds, "Spectrum2D release")?,
                                ),
                            ),
                        ],
                    );
                    Ok(scalar_signal_interner.intern(signal))
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            CompiledVisualSource::Spectrum2D {
                band_signals,
                x: spectrum.x,
                y: spectrum.y,
                width: spectrum.width,
                height: spectrum.height,
                bar_gap_ratio: spectrum.bar_gap_ratio,
                min_bar_height_ratio: spectrum.min_bar_height_ratio,
                layout: spectrum.layout.clone(),
                gradient,
                colour,
            }
        }
        VisualSource::ParticleSystem(system) => {
            compilation.parsed_colour_count += 1;
            CompiledVisualSource::ParticleSystem(Arc::new(super::particles::compile_with_signals(
                system,
                parse_colour,
                scalar_signal_interner,
            )?))
        }
        VisualSource::Group(group) => CompiledVisualSource::Group(Arc::new(compile_group(
            group,
            GroupTiming {
                duration: clip.duration,
                start_nanos,
                parent_visible_window: effective_visible_window,
            },
            validated,
            image_indices,
            video_indices,
            font_indices,
            shapes,
            texts,
            compilation,
            scalar_signal_interner,
            next_compiled_identity,
            next_video_slot_index,
        )?)),
    };
    let effects = clip
        .effects
        .iter()
        .map(|effect| {
            effects::compile_timed(effect, &clip.id, clip.duration, scalar_signal_interner)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let masks = clip
        .masks
        .iter()
        .map(|mask| {
            let input = match &mask.input {
                crate::project::MaskInput::Shape(shape) => {
                    let shape_index = image_indices.len() + shapes.len();
                    shapes.push(shape.clone());
                    crate::plan::CompiledMaskInput::Shape { shape_index }
                }
                crate::project::MaskInput::Image { asset, mode } => {
                    crate::plan::CompiledMaskInput::Image {
                        asset_index: assets::lookup(image_indices, asset, &clip.id)?,
                        mode: *mode,
                    }
                }
            };
            Ok(crate::plan::CompiledMask {
                input,
                operation: mask.operation,
                invert: mask.invert,
                strength: super::signals::compile_property(
                    &mask.strength,
                    &format!("{}/mask-strength", clip.id),
                    ScalarPropertyConstraint::ClosedRange { min: 0.0, max: 1.0 },
                    scalar_signal_interner,
                )?,
                feather: super::signals::compile_property(
                    &mask.feather,
                    &format!("{}/mask-feather", clip.id),
                    ScalarPropertyConstraint::ClosedRange {
                        min: 0.0,
                        max: f64::from(crate::project::MAX_MASK_FEATHER_PX),
                    },
                    scalar_signal_interner,
                )?,
                transform: compile_transform_tracks(
                    &mask.transform,
                    &clip.id,
                    scalar_signal_interner,
                )?,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok(CompiledLayer {
        compiled_identity,
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
        transform: compile_transform(clip, scalar_signal_interner)?,
        transform_contributions: Vec::new(),
        // Opacity is constrained only after generated transition/preset
        // contributions are applied during evaluation. Clamping inside the
        // scalar property would change the required ordering once modifiers
        // are present: authored -> modifiers -> generated contributions ->
        // final target constraint.
        opacity: super::signals::compile_property(
            &clip.opacity,
            &clip.id,
            ScalarPropertyConstraint::Finite,
            scalar_signal_interner,
        )?,
        opacity_contributions: Vec::new(),
        effects,
        masks,
        blend_mode: clip.blend_mode,
        content_dependency: crate::plan::TemporalDependency::Static,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "preset compilation forwards the recursive compiler context"
)]
pub(super) fn compile_with_preset(
    clip: &Clip,
    validated: &PlanCompileInput<'_>,
    image_indices: &BTreeMap<String, usize>,
    video_indices: &BTreeMap<String, usize>,
    font_indices: &BTreeMap<String, usize>,
    shapes: &mut Vec<crate::project::ShapeSource>,
    texts: &mut Vec<crate::project::TextSource>,
    compilation: &mut CompilationStats,
    scalar_signal_interner: &mut ScalarSignalInterner,
    next_compiled_identity: &mut usize,
    next_video_slot_index: &mut usize,
    effective_visible_window: (u128, u128),
) -> Result<CompiledLayer, Diagnostic> {
    let mut layer = compile(
        clip,
        validated,
        image_indices,
        video_indices,
        font_indices,
        shapes,
        texts,
        compilation,
        scalar_signal_interner,
        next_compiled_identity,
        next_video_slot_index,
        effective_visible_window,
    )?;
    if let Some(preset) = &clip.preset {
        super::presets::apply(&mut layer, preset, clip.duration, compilation)?;
    }
    Ok(layer)
}

#[expect(
    clippy::too_many_arguments,
    reason = "group compilation forwards the shared recursive compiler context"
)]
fn compile_group(
    group: &crate::project::Group,
    timing: GroupTiming,
    validated: &PlanCompileInput<'_>,
    image_indices: &BTreeMap<String, usize>,
    video_indices: &BTreeMap<String, usize>,
    font_indices: &BTreeMap<String, usize>,
    shapes: &mut Vec<crate::project::ShapeSource>,
    texts: &mut Vec<crate::project::TextSource>,
    compilation: &mut CompilationStats,
    scalar_signal_interner: &mut ScalarSignalInterner,
    next_compiled_identity: &mut usize,
    next_video_slot_index: &mut usize,
) -> Result<CompiledComposition, Diagnostic> {
    let duration_nanos = time::to_nanos(timing.duration, "Group")?;
    let visible_start = timing
        .parent_visible_window
        .0
        .saturating_sub(timing.start_nanos)
        .min(duration_nanos);
    let visible_end = timing
        .parent_visible_window
        .1
        .saturating_sub(timing.start_nanos)
        .min(duration_nanos);
    let effective_visible_window = (visible_start, visible_end.max(visible_start));
    let mut layers = Vec::new();
    for child in group.clips.iter().filter(|clip| clip.visible) {
        layers.push(compile_with_preset(
            child,
            validated,
            image_indices,
            video_indices,
            font_indices,
            shapes,
            texts,
            compilation,
            scalar_signal_interner,
            next_compiled_identity,
            next_video_slot_index,
            effective_visible_window,
        )?);
    }
    let indices = layers
        .iter()
        .enumerate()
        .map(|(index, layer)| (layer.id.clone(), index))
        .collect();
    super::transitions::compile_transition_placements_with_interner(
        &group.transitions,
        &indices,
        &mut layers,
        scalar_signal_interner,
    )?;
    compilation.compiled_transition_association_count = compilation
        .compiled_transition_association_count
        .saturating_add(group.transitions.len() as u64 * 2);
    let composition_end_frame =
        time::first_frame_at_or_after(duration_nanos, validated.frame_rate)?;
    let visible_start_frame =
        time::first_frame_at_or_after(effective_visible_window.0, validated.frame_rate)?;
    let visible_end_frame =
        time::first_frame_at_or_after(effective_visible_window.1, validated.frame_rate)?;
    let mut post_effects = Vec::new();
    super::finalize_composition_layers(
        &mut layers,
        &mut post_effects,
        duration_nanos,
        composition_end_frame,
        compilation,
        validated.limits.maximum_active_layers,
        super::ActiveLayerWindow {
            start_frame: visible_start_frame,
            end_frame: visible_end_frame.min(composition_end_frame),
        },
    )?;
    let dependency = layers.iter().fold(
        crate::plan::TemporalDependency::Static,
        |dependency, layer| {
            let child_start = layer.start_nanos;
            let child_end = child_start.saturating_add(layer.duration_nanos);
            let effective_start = child_start.max(effective_visible_window.0);
            let effective_end = child_end.min(effective_visible_window.1);
            let activity_changes = effective_start < effective_end
                && (effective_start > effective_visible_window.0
                    || effective_end < effective_visible_window.1);
            dependency
                .combine(if effective_start < effective_end {
                    layer.content_dependency
                } else {
                    crate::plan::TemporalDependency::Static
                })
                .combine(if activity_changes {
                    crate::plan::TemporalDependency::Dynamic
                } else {
                    crate::plan::TemporalDependency::Static
                })
        },
    );
    let schedule = crate::plan::ActiveSchedule::compile_layers(&layers);
    Ok(CompiledComposition {
        layers,
        schedule,
        dependency,
    })
}

fn compile_transform(
    clip: &Clip,
    scalar_signal_interner: &mut ScalarSignalInterner,
) -> Result<CompiledTransformTracks, Diagnostic> {
    match (&clip.source, &clip.transform) {
        (_, Some(transform)) => {
            compile_transform_tracks(transform, &clip.id, scalar_signal_interner)
        }
        (
            VisualSource::SolidColor { .. }
            | VisualSource::Shape(_)
            | VisualSource::Text(_)
            | VisualSource::Video { .. }
            | VisualSource::Spectrum2D(_)
            | VisualSource::ParticleSystem(_),
            None,
        ) => Ok(canvas_transform()),
        (VisualSource::Group(_), None) => Ok(canvas_transform()),
        (VisualSource::Image { .. }, None) => Err(Diagnostic::error(
            "VESTRA-PLAN-TRANSFORM",
            Category::Internal,
            format!(
                "validated image clip '{}' is missing its transform",
                clip.id
            ),
            "",
        )),
    }
}

fn compile_transform_tracks(
    transform: &crate::project::Transform,
    id: &str,
    scalar_signal_interner: &mut ScalarSignalInterner,
) -> Result<CompiledTransformTracks, Diagnostic> {
    Ok(CompiledTransformTracks {
        position: tracks::compile(&transform.position, id)?,
        position_x_modifiers: super::signals::compile_modifiers(
            &transform.component_modifiers.position_x,
            scalar_signal_interner,
        )?,
        position_y_modifiers: super::signals::compile_modifiers(
            &transform.component_modifiers.position_y,
            scalar_signal_interner,
        )?,
        anchor: tracks::compile(&transform.anchor, id)?,
        scale: tracks::compile(&transform.scale, id)?,
        scale_x_modifiers: super::signals::compile_modifiers(
            &transform.component_modifiers.scale_x,
            scalar_signal_interner,
        )?,
        scale_y_modifiers: super::signals::compile_modifiers(
            &transform.component_modifiers.scale_y,
            scalar_signal_interner,
        )?,
        rotation_degrees: super::signals::compile_property(
            &transform.rotation_degrees,
            id,
            ScalarPropertyTarget::RotationDegrees.constraint(),
            scalar_signal_interner,
        )?,
    })
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
            ScalarPropertyTarget::RotationDegrees.constraint(),
        ),
    }
}
