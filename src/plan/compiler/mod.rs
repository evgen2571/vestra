#![allow(
    clippy::result_large_err,
    reason = "plan compilation preserves machine-readable diagnostics"
)]

use std::collections::BTreeMap;

use crate::{
    Category, Diagnostic,
    media::EncoderSettings,
    plan::{Canvas, CompilationStats, RenderPlan},
    project::{ValidatedProject, parse_colour},
};

mod assets;
mod audio;
mod clips;
mod effects;
mod flashes;
mod limits;
mod metrics;
mod output;
mod presets;
mod time;
mod tracks;
mod transitions;

use time::{effective_dimensions, first_frame_at_or_after, to_nanos};
#[derive(Clone, Copy, Debug, Default)]
pub struct CompileOptions {
    pub preview: bool,
}

#[allow(
    clippy::result_large_err,
    reason = "compiler diagnostics are machine-readable"
)]
pub fn compile(
    validated: &ValidatedProject,
    options: CompileOptions,
) -> Result<RenderPlan, Diagnostic> {
    compile_canonical(validated, &validated.project, options)
}
fn compile_canonical(
    validated: &ValidatedProject,
    project: &crate::project::Project,
    options: CompileOptions,
) -> Result<RenderPlan, Diagnostic> {
    let (width, height) =
        effective_dimensions(project.output.width, project.output.height, options.preview);
    let background = parse_colour(&project.output.background).ok_or_else(|| {
        Diagnostic::error(
            "MVP-PLAN-BACKGROUND",
            Category::Internal,
            "validated background is invalid",
            "/output/background",
        )
    })?;
    let image_table = assets::build(validated, project);
    let mut layers = Vec::new();
    let mut compilation = CompilationStats {
        parsed_colour_count: 1,
        declared_clip_count: project.visual.clips.len(),
        hidden_clip_count: project
            .visual
            .clips
            .iter()
            .filter(|clip| !clip.visible)
            .count(),
        ..CompilationStats::default()
    };
    for clip in project.visual.clips.iter().filter(|clip| clip.visible) {
        layers.push(clips::compile(
            clip,
            validated,
            &image_table.indices,
            &mut compilation,
        )?);
        if let Some(preset) = &clip.preset {
            presets::apply(
                layers.last_mut().expect("layer was inserted"),
                preset,
                clip.duration,
                &mut compilation,
            )?;
        }
    }
    compilation.rendered_clip_count = layers
        .iter()
        .filter(|layer| layer.start_frame < layer.end_frame)
        .count();
    compilation.zero_frame_clip_count = layers.len() - compilation.rendered_clip_count;
    let indices: BTreeMap<String, usize> = layers
        .iter()
        .enumerate()
        .map(|(index, layer)| (layer.id.clone(), index))
        .collect();
    transitions::compile(
        &project.visual.transitions,
        &indices,
        &mut layers,
        &mut compilation,
    )?;
    for flash in &project.visual.flashes {
        layers.push(flashes::compile(
            flash,
            validated.frame_rate,
            validated.frame_count,
        )?);
    }
    compilation.parsed_colour_count += project.visual.flashes.len() as u64;
    let post_effects = project
        .visual
        .post_effects
        .iter()
        .map(|effect| effects::compile_timed(effect, "global post effect", validated.duration))
        .collect::<Result<Vec<_>, _>>()?;
    metrics::record(&mut compilation, &layers, &post_effects);
    limits::enforce_active_layer_limit(&layers, validated.limits.maximum_active_layers)?;
    Ok(RenderPlan {
        configured_output: output::resolve_path(validated),
        canvas: Canvas {
            width,
            height,
            background,
            preview: options.preview,
        },
        duration: validated.duration,
        frame_rate: validated.frame_rate,
        frame_count: validated.frame_count,
        encoder: EncoderSettings {
            width,
            height,
            frame_rate: validated.frame_rate,
            frame_count: validated.frame_count,
            duration: validated.duration,
            quality_crf: project.output.quality.crf(),
            audio: audio::compile(validated)?,
        },
        limits: validated.limits,
        images: image_table.images,
        layers,
        post_effects,
        compilation,
        warnings: validated.warnings.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::CompiledVisualSource;
    use crate::project::{ValidationOptions, load_and_validate};

    fn flash(fade_in: f64, fade_out: f64) -> crate::project::Flash {
        crate::project::Flash {
            id: "flash".to_owned(),
            start: 1.0,
            duration: 2.0,
            colour: "#ffffff".to_owned(),
            opacity: 0.7,
            fade_in,
            fade_out,
            layer: 1,
        }
    }

    #[test]
    fn compiles_transitions_and_flashes_to_normal_layers() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &ValidationOptions {
                check_backend: false,
                ..ValidationOptions::default()
            },
        )
        .expect("valid project");
        let plan = compile(&validated, CompileOptions::default()).expect("plan");
        assert_eq!(plan.frame_count, 144);
        assert_eq!(plan.images.len(), 2);
        assert!(
            plan.layers
                .iter()
                .any(|layer| matches!(layer.source, CompiledVisualSource::SolidColor { .. }))
        );
        assert!(
            plan.layers
                .iter()
                .any(|layer| !layer.opacity_contributions.is_empty())
        );
        // Includes authored tracks plus transition and flash tracks generated by
        // compilation, which are all evaluated while rendering this plan.
        assert_eq!(plan.compilation.keyframe_count, 12);
        assert_eq!(plan.compilation.compiled_transition_association_count, 2);
    }

    #[test]
    fn frame_intervals_are_half_open() {
        assert_eq!(first_frame_at_or_after(0, (24, 1)).expect("frame"), 0);
        assert_eq!(
            first_frame_at_or_after(1_000_000_000, (24, 1)).expect("frame"),
            24
        );
        assert_eq!(
            first_frame_at_or_after(1_000_000_001, (24, 1)).expect("frame"),
            25
        );
    }

    #[test]
    fn flash_without_fade_out_keeps_constant_opacity_until_its_end() {
        let layer = flashes::compile(&flash(0.0, 0.0), (24, 1), 100).expect("flash compiles");
        assert_eq!(layer.opacity.base_value, 0.7);
        assert!(layer.opacity.keyframes.is_empty());
    }

    #[test]
    fn flash_fade_out_holds_then_reaches_zero_at_end() {
        let layer = flashes::compile(&flash(0.0, 0.5), (24, 1), 100).expect("flash compiles");
        assert_eq!(layer.opacity.evaluate(1_500_000_000), 0.7);
        assert_eq!(layer.opacity.evaluate(2_000_000_000), 0.0);
    }

    #[test]
    fn flash_fade_in_only_reaches_and_holds_configured_opacity() {
        let layer = flashes::compile(&flash(0.5, 0.0), (24, 1), 100).expect("flash");
        assert_eq!(layer.opacity.evaluate(0), 0.0);
        assert_eq!(layer.opacity.evaluate(250_000_000), 0.35);
        assert_eq!(layer.opacity.evaluate(500_000_000), 0.7);
        assert_eq!(layer.opacity.evaluate(1_999_999_999), 0.7);
    }

    #[test]
    fn flash_with_both_fades_holds_between_their_boundaries() {
        let layer = flashes::compile(&flash(0.5, 0.5), (24, 1), 100).expect("flash");
        assert_eq!(layer.opacity.evaluate(250_000_000), 0.35);
        assert_eq!(layer.opacity.evaluate(500_000_000), 0.7);
        assert_eq!(layer.opacity.evaluate(1_500_000_000), 0.7);
        assert_eq!(layer.opacity.evaluate(1_750_000_000), 0.35);
        assert_eq!(layer.opacity.evaluate(2_000_000_000), 0.0);
    }

    #[test]
    fn flash_fades_can_fill_the_entire_interval() {
        let layer = flashes::compile(&flash(1.0, 1.0), (24, 1), 100).expect("flash");
        assert_eq!(layer.opacity.evaluate(1_000_000_000), 0.7);
        assert_eq!(layer.opacity.evaluate(1_500_000_000), 0.35);
        assert_eq!(layer.opacity.evaluate(2_000_000_000), 0.0);
        assert_eq!(layer.end_frame, 72);
    }

    #[test]
    fn zero_frame_layers_do_not_count_toward_active_layer_limit() {
        let mut layer = flashes::compile(&flash(0.0, 0.0), (24, 1), 100).expect("flash");
        layer.start_frame = 12;
        layer.end_frame = 12;
        limits::enforce_active_layer_limit(&[layer], 0).expect("zero-frame layer is never active");
    }

    #[test]
    fn directional_push_keeps_authored_tracks_and_has_correct_endpoints() {
        let mut outgoing = flashes::compile(&flash(0.0, 0.0), (24, 1), 100).expect("flash");
        outgoing.start_nanos = 0;
        transitions::push_layer(
            &mut outgoing,
            1_000_000_000,
            2_000_000_000,
            0.0,
            0.25,
            0.0,
            false,
        );
        let outgoing_contribution = &outgoing.transform_contributions[0];
        assert_eq!(
            outgoing_contribution
                .position_offset
                .evaluate(1_000_000_000)
                .x,
            0.0
        );
        assert_eq!(
            outgoing_contribution
                .position_offset
                .evaluate(2_000_000_000)
                .x,
            0.25
        );
        assert!(outgoing.transform.position.keyframes.is_empty());

        let mut incoming = flashes::compile(&flash(0.0, 0.0), (24, 1), 100).expect("flash");
        incoming.start_nanos = 0;
        transitions::push_layer(
            &mut incoming,
            1_000_000_000,
            2_000_000_000,
            0.0,
            0.25,
            0.0,
            true,
        );
        let incoming_contribution = &incoming.transform_contributions[0];
        assert_eq!(
            incoming_contribution
                .position_offset
                .evaluate(1_000_000_000)
                .x,
            -0.25
        );
        assert_eq!(
            incoming_contribution
                .position_offset
                .evaluate(2_000_000_000)
                .x,
            0.0
        );
        assert!(incoming.transform.position.keyframes.is_empty());
    }

    #[test]
    fn transient_preset_uses_its_own_interval_on_a_long_clip() {
        let mut layer = flashes::compile(&flash(0.0, 0.0), (30, 1), 300).expect("layer");
        layer.id = "clip".to_owned();
        let preset = crate::project::Preset::Impact {
            timing: crate::project::ActiveInterval {
                start: 1.25,
                duration: Some(0.28),
            },
            intensity: 1.0,
            seed: 7,
        };
        let mut compilation = CompilationStats::default();
        presets::apply(&mut layer, &preset, 4.0, &mut compilation).expect("preset compiles");
        assert_eq!(compilation.generated_local_effect_count, 3);
        assert_eq!(layer.transform_contributions.len(), 1);
        assert_eq!(layer.effects.len(), 3);
        assert!(matches!(
            layer.effects[0].effect,
            crate::plan::CompiledEffect::CameraShake { .. }
        ));
        assert!(matches!(
            layer.effects[1].effect,
            crate::plan::CompiledEffect::ChromaticAberration { .. }
        ));
        assert!(matches!(
            layer.effects[2].effect,
            crate::plan::CompiledEffect::Tint { .. }
        ));
        assert!(
            layer
                .transform_contributions
                .iter()
                .all(|contribution| contribution.start >= 1_250_000_000)
        );
        assert!(
            layer
                .effects
                .iter()
                .all(|effect| { effect.start == 1_250_000_000 && effect.end == 1_530_000_000 })
        );
        assert!(
            layer
                .effects
                .iter()
                .all(|effect| !effect.active_at(1_000_000_000))
        );
        assert!(
            layer
                .effects
                .iter()
                .all(|effect| !effect.active_at(1_530_000_000))
        );
    }

    #[test]
    fn flash_cut_holds_visibility_until_its_peak_then_switches() {
        let mut outgoing = flashes::compile(&flash(0.0, 0.0), (30, 1), 300).expect("out");
        outgoing.id = "out".to_owned();
        outgoing.start_nanos = 0;
        let mut incoming = flashes::compile(&flash(0.0, 0.0), (30, 1), 300).expect("in");
        incoming.id = "in".to_owned();
        incoming.start_nanos = 0;
        let mut layers = vec![outgoing, incoming];
        let indices = BTreeMap::from([("out".to_owned(), 0), ("in".to_owned(), 1)]);
        transitions::compile(
            &[crate::project::Transition::FlashCut {
                id: "cut".to_owned(),
                outgoing: "out".to_owned(),
                incoming: "in".to_owned(),
                start: 1.0,
                duration: 0.2,
                interpolation: crate::project::Interpolation::Named(
                    crate::project::InterpolationName::Linear,
                ),
                colour: "#ffffff".to_owned(),
                intensity: 1.0,
            }],
            &indices,
            &mut layers,
            &mut CompilationStats::default(),
        )
        .expect("flash cut compiles");
        let outgoing = &layers[0].opacity_contributions[0];
        let incoming = &layers[1].opacity_contributions[0];
        assert_eq!(outgoing.evaluate(1_099_999_999), 1.0);
        assert_eq!(incoming.evaluate(1_099_999_999), 0.0);
        assert_eq!(outgoing.evaluate(1_100_000_000), 0.0);
        assert_eq!(incoming.evaluate(1_100_000_000), 1.0);
    }

    #[test]
    fn zoom_blur_transition_compiles_to_a_radial_blur_effect() {
        let mut layer = flashes::compile(&flash(0.0, 0.0), (30, 1), 300).expect("layer");
        layer.start_nanos = 0;
        transitions::zoom_layer(
            &mut layer,
            1_000_000_000,
            1_200_000_000,
            1.0,
            1.1,
            Some(4.0),
        );
        assert!(matches!(
            layer.effects[0].effect,
            crate::plan::CompiledEffect::ZoomBlur { .. }
        ));
        assert!(!layer.effects[0].active_at(1_200_000_000));
    }
}
