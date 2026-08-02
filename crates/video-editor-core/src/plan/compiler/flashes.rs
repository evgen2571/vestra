//! Compilation of declarative flash overlays into ordinary solid-color layers.

use crate::{
    Diagnostic,
    animation::{Interpolation, Keyframe, Track},
    plan::{CompiledLayer, CompiledVisualSource, DrawKey},
    project::{BlendMode, Flash, parse_colour},
};

use super::{
    clips::canvas_transform, first_frame_at_or_after, to_nanos, transitions::insert_keyframe,
};

pub(super) fn compile(
    flash: &Flash,
    rate: (u64, u64),
    frame_count: u64,
) -> Result<CompiledLayer, Diagnostic> {
    let start_nanos = to_nanos(flash.start, &flash.id)?;
    let duration_nanos = to_nanos(flash.duration, &flash.id)?;
    let colour = parse_colour(&flash.colour).ok_or_else(|| {
        Diagnostic::error(
            "MVP-PLAN-FLASH",
            crate::Category::Internal,
            "validated flash has invalid colour",
            "",
        )
    })?;
    let fade_in = to_nanos(flash.fade_in, &flash.id)?;
    let fade_out = to_nanos(flash.fade_out, &flash.id)?;
    let mut opacity = Track::new(if fade_in == 0 { flash.opacity } else { 0.0 });
    if fade_in > 0 {
        insert_keyframe(
            &mut opacity.keyframes,
            Keyframe {
                time: 0,
                value: 0.0,
                interpolation: Interpolation::Hold,
            },
        );
        insert_keyframe(
            &mut opacity.keyframes,
            Keyframe {
                time: fade_in,
                value: flash.opacity,
                interpolation: Interpolation::Linear,
            },
        );
    }
    if fade_out > 0 {
        let fade_out_start = duration_nanos.saturating_sub(fade_out);
        if fade_out_start != fade_in {
            insert_keyframe(
                &mut opacity.keyframes,
                Keyframe {
                    time: fade_out_start,
                    value: flash.opacity,
                    interpolation: Interpolation::Hold,
                },
            );
        }
        insert_keyframe(
            &mut opacity.keyframes,
            Keyframe {
                time: duration_nanos,
                value: 0.0,
                interpolation: Interpolation::Linear,
            },
        );
    }
    Ok(CompiledLayer {
        id: flash.id.clone(),
        start_nanos,
        duration_nanos,
        start_frame: first_frame_at_or_after(start_nanos, rate)?,
        end_frame: first_frame_at_or_after(start_nanos.saturating_add(duration_nanos), rate)?
            .min(frame_count),
        draw_key: DrawKey {
            layer: flash.layer,
            start_nanos,
            id: flash.id.clone(),
        },
        source: CompiledVisualSource::SolidColor { colour },
        transform: canvas_transform(),
        transform_contributions: Vec::new(),
        opacity,
        opacity_contributions: Vec::new(),
        effects: Vec::new(),
        blend_mode: BlendMode::Normal,
        content_dependency: crate::plan::TemporalDependency::Static,
    })
}
