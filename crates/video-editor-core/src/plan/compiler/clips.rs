//! Compilation of visible project clips into renderable layers.

use std::collections::BTreeMap;

use crate::{
    Category, Diagnostic,
    animation::Track,
    domain::{Crop, Point},
    plan::{
        CompilationStats, CompiledLayer, CompiledScalarProperty, CompiledSizing,
        CompiledTransformTracks, CompiledVisualSource, DrawKey, PlanCompileInput,
        ScalarPropertyConstraint, ScalarPropertyTarget, ScalarSignalInterner,
    },
    project::{Clip, VisualSource, parse_colour},
};

use super::{assets, effects, output, time, tracks};

pub(super) fn compile(
    clip: &Clip,
    validated: &PlanCompileInput<'_>,
    image_indices: &BTreeMap<String, usize>,
    compilation: &mut CompilationStats,
    scalar_signal_interner: &mut ScalarSignalInterner,
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
        VisualSource::Spectrum2D(spectrum) => {
            compilation.parsed_colour_count += 1;
            let colour = parse_colour(&spectrum.colour).ok_or_else(|| {
                Diagnostic::error(
                    "MVP-PLAN-SPECTRUM2D-COLOUR",
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
                                "MVP-PLAN-SPECTRUM2D-GRADIENT",
                                Category::Internal,
                                "validated Spectrum2D gradient start colour is invalid",
                                "",
                            )
                        })?,
                        parse_colour(&value.end_colour).ok_or_else(|| {
                            Diagnostic::error(
                                "MVP-PLAN-SPECTRUM2D-GRADIENT",
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
                                "MVP-PLAN-SPECTRUM2D-BAND",
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
                                            "MVP-PLAN-SPECTRUM2D-RESPONSE",
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
                                        "MVP-PLAN-SPECTRUM2D-RESPONSE",
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
    };
    let effects = clip
        .effects
        .iter()
        .map(|effect| {
            effects::compile_timed(effect, &clip.id, clip.duration, scalar_signal_interner)
        })
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
        blend_mode: clip.blend_mode,
        content_dependency: crate::plan::TemporalDependency::Static,
    })
}

fn compile_transform(
    clip: &Clip,
    scalar_signal_interner: &mut ScalarSignalInterner,
) -> Result<CompiledTransformTracks, Diagnostic> {
    match (&clip.source, &clip.transform) {
        (_, Some(transform)) => Ok(CompiledTransformTracks {
            position: tracks::compile(&transform.position, &clip.id)?,
            position_x_modifiers: super::signals::compile_modifiers(
                &transform.component_modifiers.position_x,
                scalar_signal_interner,
            )?,
            position_y_modifiers: super::signals::compile_modifiers(
                &transform.component_modifiers.position_y,
                scalar_signal_interner,
            )?,
            anchor: tracks::compile(&transform.anchor, &clip.id)?,
            scale: tracks::compile(&transform.scale, &clip.id)?,
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
                &clip.id,
                ScalarPropertyTarget::RotationDegrees.constraint(),
                scalar_signal_interner,
            )?,
        }),
        (VisualSource::SolidColor { .. } | VisualSource::Spectrum2D(_), None) => {
            Ok(canvas_transform())
        }
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
            ScalarPropertyTarget::RotationDegrees.constraint(),
        ),
    }
}
