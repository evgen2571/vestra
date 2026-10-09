//! Backend-neutral palette evaluation in encoded RGB byte space.

use crate::project::{PaletteMode, parse_colour};

mod oklab {
    include!("oklab_constants.in");
}

/// Fixed-capacity evaluated palette, shared unchanged by CPU and WGPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvaluatedPalette {
    pub colours: [[u8; 4]; 16],
    pub len: u32,
}

pub(crate) fn compile_stops(stops: &[f64], count: usize) -> Option<[u16; 16]> {
    if stops.len() != count
        || !(2..=16).contains(&count)
        || stops.first() != Some(&0.0)
        || stops.last() != Some(&1.0)
    {
        return None;
    }
    let mut result = [0; 16];
    for (index, &stop) in stops.iter().enumerate() {
        if !stop.is_finite() || !(0.0..=1.0).contains(&stop) {
            return None;
        }
        result[index] = (stop * 65280.0).round() as u16;
        if index > 0 && result[index] <= result[index - 1] {
            return None;
        }
    }
    Some(result)
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

/// Encoded RGB, integer HSV with circular hue, or biased Q10 Oklab features.
#[must_use]
pub fn chromatic_features(pixel: [u8; 4], mode: PaletteMode) -> [u32; 3] {
    if mode == PaletteMode::NearestOklab {
        return oklab_features(pixel);
    }
    let [r, g, b] = [
        i32::from(pixel[0]),
        i32::from(pixel[1]),
        i32::from(pixel[2]),
    ];
    if mode != PaletteMode::NearestHue {
        return [r as u32, g as u32, b as u32];
    }
    let maximum = r.max(g).max(b);
    let minimum = r.min(g).min(b);
    let range = maximum - minimum;
    if range == 0 {
        return [0, 0, maximum as u32];
    }
    let hue = if maximum == r {
        (g - b) * 255 / range
    } else if maximum == g {
        510 + (b - r) * 255 / range
    } else {
        1020 + (r - g) * 255 / range
    };
    [
        hue.rem_euclid(1530) as u32,
        (range * 255 / maximum) as u32,
        maximum as u32,
    ]
}

fn oklab_features(pixel: [u8; 4]) -> [u32; 3] {
    let linear = [pixel[0], pixel[1], pixel[2]].map(|c| oklab::SRGB_LINEAR[c as usize]);
    let roots = oklab::RGB_TO_LMS.map(|row| {
        let sum: u32 = row.iter().zip(linear).map(|(a, b)| a * b).sum();
        cube_root_q10((sum + 16384) / 32768) as i32
    });
    let lab = oklab::LMS_TO_LAB.map(|row| {
        let sum: i32 = row.iter().zip(roots).map(|(a, b)| a * b).sum();
        sum.signum() * ((sum.abs() + 16384) / 32768)
    });
    [lab[0] as u32, (lab[1] + 512) as u32, (lab[2] + 512) as u32]
}

fn cube_root_q10(value: u32) -> u32 {
    let target = value * 16384;
    let (mut lower, mut upper) = (0, 1025);
    while lower + 1 < upper {
        let middle = (lower + upper) / 2;
        if middle * middle * middle <= target {
            lower = middle;
        } else {
            upper = middle;
        }
    }
    let halfway = lower * lower * lower + (12 * lower * lower + 6 * lower + 1) / 8;
    (lower + u32::from(target > halfway)).min(1024)
}

/// Integer squared distance; hue is circular and weighted by shared saturation.
#[must_use]
pub fn chromatic_distance(a: [u32; 3], b: [u32; 3], mode: PaletteMode) -> u32 {
    let mut delta = std::array::from_fn::<_, 3, _>(|i| a[i].abs_diff(b[i]));
    if mode == PaletteMode::NearestHue {
        delta[0] = delta[0].min(1530 - delta[0]) / 3 * a[1].min(b[1]) / 255;
    }
    delta.iter().map(|v| v * v).sum()
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
    fn nonuniform_stops_validate_quantized_spacing() {
        assert_eq!(
            compile_stops(&[0.0, 0.25, 1.0], 3),
            Some([0, 16320, 65280, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
        );
        for invalid in [
            &[0.0, 1.0][..],
            &[0.1, 0.5, 1.0],
            &[0.0, 0.5, 0.9],
            &[0.0, 0.0, 1.0],
            &[0.0, f64::NAN, 1.0],
            &[0.0, 0.000001, 1.0],
        ] {
            assert!(compile_stops(invalid, 3).is_none());
        }
    }

    #[test]
    fn oklab_integer_conversion_tracks_reference_and_preserves_neutrals() {
        fn reference(rgb: [u8; 3]) -> [f64; 3] {
            let linear = rgb.map(|c| {
                let v = f64::from(c) / 255.0;
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            });
            let matrices = [
                [0.4122214708, 0.5363325363, 0.0514459929],
                [0.2119034982, 0.6806995451, 0.1073969566],
                [0.0883024619, 0.2817188376, 0.6299787005],
            ];
            let roots = matrices.map(|row| {
                row.iter()
                    .zip(linear)
                    .map(|(a, b)| a * b)
                    .sum::<f64>()
                    .cbrt()
            });
            [
                [0.2104542553, 0.7936177850, -0.0040720468],
                [1.9779984951, -2.4285922050, 0.4505937099],
                [0.0259040371, 0.7827717662, -0.8086757660],
            ]
            .map(|row| row.iter().zip(roots).map(|(a, b)| a * b).sum())
        }
        let mut maximum = 0.0_f64;
        for range in [
            (0_u8..=255).step_by(17).collect::<Vec<_>>(),
            (0..=15).collect(),
        ] {
            for &r in &range {
                for &g in &range {
                    for &b in &range {
                        let feature = oklab_features([r, g, b, 255]);
                        assert!(feature[0] <= 1024 && feature[1] < 1024 && feature[2] < 1024);
                        let actual = [
                            f64::from(feature[0]) / 1024.0,
                            (f64::from(feature[1]) - 512.0) / 1024.0,
                            (f64::from(feature[2]) - 512.0) / 1024.0,
                        ];
                        for (a, b) in actual.into_iter().zip(reference([r, g, b])) {
                            maximum = maximum.max((a - b).abs());
                        }
                    }
                }
            }
        }
        eprintln!("OKLAB_Q10 maximum reference component error={maximum}");
        assert!(maximum < 0.004);
        assert_eq!(oklab_features([0, 0, 0, 255]), [0, 512, 512]);
        assert_eq!(oklab_features([255, 255, 255, 255]), [1024, 512, 512]);
        let mut previous = 0;
        for gray in 0..=255 {
            let [l, a, b] = oklab_features([gray, gray, gray, 255]);
            assert_eq!([a, b], [512, 512]);
            assert!(l >= previous);
            previous = l;
        }
        for value in 0..=65536 {
            let expected = ((f64::from(value) / 65536.0).cbrt() * 1024.0).round() as u32;
            assert_eq!(cube_root_q10(value), expected);
        }
    }

    #[test]
    #[ignore = "explicit exhaustive integer Oklab packing bounds over all RGB bytes"]
    fn oklab_all_rgb_bytes_fit_packed_features() {
        let mut minimum = [u32::MAX; 3];
        let mut maximum = [0; 3];
        for r in 0..=255 {
            for g in 0..=255 {
                for b in 0..=255 {
                    let feature = oklab_features([r, g, b, 255]);
                    assert!(feature[0] <= 1024 && feature[1] < 1024 && feature[2] < 1024);
                    for c in 0..3 {
                        minimum[c] = minimum[c].min(feature[c]);
                        maximum[c] = maximum[c].max(feature[c]);
                    }
                }
            }
        }
        eprintln!("OKLAB_Q10 exhaustive packed minimum={minimum:?} maximum={maximum:?}");
    }

    #[test]
    fn hue_distance_wraps_and_ignores_achromatic_hue() {
        let mode = PaletteMode::NearestHue;
        let a = chromatic_features([255, 0, 1, 255], mode);
        let b = chromatic_features([255, 1, 0, 255], mode);
        let green = chromatic_features([0, 255, 0, 255], mode);
        assert_eq!(a, [1529, 255, 255]);
        assert_eq!(b, [1, 255, 255]);
        assert!(chromatic_distance(a, b, mode) < chromatic_distance(a, green, mode));
        let gray = chromatic_features([127, 127, 127, 255], mode);
        assert_eq!(gray, [0, 0, 127]);
        assert_eq!(
            chromatic_distance(a, gray, mode),
            chromatic_distance(green, gray, mode)
        );
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
            stops: None,
            palette: palette(),
            mode: PaletteMode::Nearest,
            levels: 4,
            amount: scalar(1.0),
            phase: scalar(0.0),
            period: Some(2.0),
        };
        let evaluated =
            crate::plan::evaluate_effect(&effect, 500_000_000, 9_000_000_000, &context).unwrap();
        let EvaluatedEffect::PaletteMap {
            stops,
            palette,
            amount,
            mode,
            levels,
        } = evaluated
        else {
            panic!("palette mapping evaluated");
        };
        assert_eq!(&palette.colours[..2], &[[128, 128, 128, 255]; 2]);
        assert_eq!(amount, 1.0);
        assert!(stops.is_none());
        assert_eq!(mode, PaletteMode::Nearest);
        assert_eq!(levels, 4);
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
