//! Backend-neutral palette evaluation in encoded RGB byte space.

use crate::project::{PaletteMode, parse_colour};

/// Fixed-capacity evaluated palette, shared unchanged by CPU and WGPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvaluatedPalette {
    pub colours: [[u8; 4]; 16],
    pub len: u32,
}

pub(crate) fn compile_palette(colours: &[String]) -> Option<EvaluatedPalette> {
    if !(2..=16).contains(&colours.len()) {
        return None;
    }
    let mut result = EvaluatedPalette {
        colours: [[0; 4]; 16],
        len: colours.len() as u32,
    };
    for (target, authored) in result.colours.iter_mut().zip(colours) {
        let parsed = parse_colour(authored)?;
        if parsed[3] != 255 {
            return None;
        }
        *target = parsed;
    }
    Some(result)
}

pub(crate) fn evaluate_palette(
    palette: &EvaluatedPalette,
    mode: PaletteMode,
    phase: f64,
    period: Option<f64>,
    local_time: u128,
) -> EvaluatedPalette {
    // Reduce time before adding phase so large authored phases cannot lose the
    // procedural component. This is a pure local-timeline function, no history.
    let cycle = period.map_or(0.0, |seconds| {
        let nanos = seconds * 1_000_000_000.0;
        if nanos >= 1.0 && nanos < u128::MAX as f64 && nanos.fract() == 0.0 {
            (local_time % nanos as u128) as f64 / nanos
        } else if nanos.is_finite() {
            (local_time as f64).rem_euclid(nanos) / nanos
        } else {
            (local_time as f64 / 1_000_000_000.0) / seconds
        }
    });
    let phase = (phase.rem_euclid(1.0) + cycle).rem_euclid(1.0);
    if mode == PaletteMode::Rainbow {
        let mut result = EvaluatedPalette {
            colours: [[0; 4]; 16],
            len: 16,
        };
        for (index, stop) in result.colours.iter_mut().enumerate() {
            let tone = index as f64 / 15.0;
            *stop = hsv_to_rgb((phase + tone).rem_euclid(1.0), tone);
        }
        return result;
    }
    let offset = phase * f64::from(palette.len);
    let whole = offset.floor() as usize;
    let fraction = offset.fract();
    let mut result = *palette;
    let count = palette.len as usize;
    for (index, stop) in result.colours[..count].iter_mut().enumerate() {
        let a = palette.colours[(index + whole) % count];
        let b = palette.colours[(index + whole + 1) % count];
        for channel in 0..3 {
            stop[channel] = (f64::from(a[channel]) * (1.0 - fraction)
                + f64::from(b[channel]) * fraction)
                .round() as u8;
        }
        stop[3] = 255;
    }
    result
}

fn hsv_to_rgb(hue: f64, value: f64) -> [u8; 4] {
    let h = hue * 6.0;
    let x = value * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
    let channels = match h.floor() as u8 {
        0 => [value, x, 0.0],
        1 => [x, value, 0.0],
        2 => [0.0, value, x],
        3 => [0.0, x, value],
        4 => [x, 0.0, value],
        _ => [value, 0.0, x],
    };
    [
        (channels[0] * 255.0).round() as u8,
        (channels[1] * 255.0).round() as u8,
        (channels[2] * 255.0).round() as u8,
        255,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        animation::Track,
        plan::{
            CompiledEffect, CompiledScalarProperty, EvaluatedEffect, EvaluationContext,
            PreparedScalarSignals, compiled_effect_pass_plan, effect_pass_plan,
        },
    };

    fn palette() -> EvaluatedPalette {
        compile_palette(&["#000000".to_owned(), "#ffffff".to_owned()]).unwrap()
    }

    #[test]
    fn palette_rotation_interpolates_authored_order_without_sorting() {
        let authored = compile_palette(&["#ff0000".to_owned(), "#0000ff".to_owned()]).unwrap();
        let at_quarter = evaluate_palette(&authored, PaletteMode::Gradient, 0.25, None, 0);
        assert_eq!(
            &at_quarter.colours[..2],
            &[[128, 0, 128, 255], [128, 0, 128, 255]]
        );
        let at_half = evaluate_palette(&authored, PaletteMode::Gradient, 0.5, None, 0);
        assert_eq!(&at_half.colours[..2], &[[0, 0, 255, 255], [255, 0, 0, 255]]);
    }

    #[test]
    fn palette_animation_is_periodic_under_nonsequential_evaluation() {
        let authored = palette();
        for mode in [
            PaletteMode::Gradient,
            PaletteMode::Nearest,
            PaletteMode::Rainbow,
        ] {
            for time in [7_450_000_000, 0, 500_000_000, 1_250_000_000] {
                assert_eq!(
                    evaluate_palette(&authored, mode, -0.25, Some(2.0), time),
                    evaluate_palette(&authored, mode, -0.25, Some(2.0), time + 2_000_000_000)
                );
            }
        }
    }

    #[test]
    fn palette_animation_is_continuous_across_loop_seam() {
        let authored = palette();
        for mode in [PaletteMode::Gradient, PaletteMode::Rainbow] {
            let before = evaluate_palette(&authored, mode, 0.0, Some(2.0), 1_999_999_000);
            let after = evaluate_palette(&authored, mode, 0.0, Some(2.0), 2_000_001_000);
            for (a, b) in before.colours.iter().zip(after.colours) {
                assert!(a.iter().zip(b).all(|(&a, b)| a.abs_diff(b) <= 1));
            }
        }
    }

    #[test]
    fn rainbow_retains_dark_and_bright_tonal_endpoints() {
        let evaluated = evaluate_palette(&palette(), PaletteMode::Rainbow, 0.0, None, 0);
        assert_eq!(evaluated.len, 16);
        assert_eq!(evaluated.colours[0], [0, 0, 0, 255]);
        assert_eq!(evaluated.colours[15], [255, 0, 0, 255]);
        assert_eq!(evaluated.colours[5], [0, 85, 0, 255]);
    }

    #[test]
    fn core_evaluation_and_passes_share_exact_palette_and_identity() {
        let scalar = |value| CompiledScalarProperty::authored(Track::new(value));
        let signals = PreparedScalarSignals::empty();
        let context = EvaluationContext::new(&signals);
        let mut effect = CompiledEffect::PaletteMap {
            palette: palette(),
            mode: PaletteMode::Nearest,
            amount: scalar(1.0),
            phase: scalar(0.0),
            period: Some(2.0),
        };
        let evaluated =
            crate::plan::evaluate_effect(&effect, 500_000_000, 9_000_000_000, &context).unwrap();
        let EvaluatedEffect::PaletteMap {
            palette,
            amount,
            nearest,
        } = evaluated
        else {
            panic!("palette mapping evaluated");
        };
        assert_eq!(&palette.colours[..2], &[[128, 128, 128, 255]; 2]);
        assert_eq!(amount, 1.0);
        assert!(nearest);
        assert_eq!(effect_pass_plan(&evaluated).len(), 1);
        assert!(
            !effect_pass_plan(&evaluated)
                .requirements()
                .retains_original()
        );
        assert_eq!(compiled_effect_pass_plan(&effect).len(), 1);
        if let CompiledEffect::PaletteMap { amount, .. } = &mut effect {
            *amount = scalar(0.0);
        }
        let identity =
            crate::plan::evaluate_effect(&effect, 500_000_000, 9_000_000_000, &context).unwrap();
        assert!(effect_pass_plan(&identity).is_empty());
    }
}
