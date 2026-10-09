//! Image-anchored palette/channel quantization and ordered dithering.

use image::RgbaImage;
use vestra_core::{
    project::{DitherMatrix, PaletteMode},
    stylization::EvaluatedPalette,
};

#[expect(
    clippy::too_many_arguments,
    reason = "consumes canonical palette controls"
)]
pub(super) fn palette_map(
    source: &RgbaImage,
    analysis: Option<&RgbaImage>,
    input_scale: u16,
    target: &mut RgbaImage,
    palette: &EvaluatedPalette,
    stops: Option<&[u16; 16]>,
    amount: f64,
    mode: PaletteMode,
    levels: u16,
    interpolation: vestra_core::project::PaletteInterpolation,
) {
    let features = palette.colours.map(|p| {
        vestra_core::stylization::chromatic_features(
            p,
            if interpolation == vestra_core::project::PaletteInterpolation::Oklab
                && matches!(mode, PaletteMode::Gradient | PaletteMode::Rainbow)
            {
                PaletteMode::NearestOklab
            } else {
                mode
            },
        )
    });
    let last = palette.len - 1;
    let amount = (amount * 65535.0).round() as u32;
    let input_scale = u32::from(input_scale);
    let analysis = analysis.unwrap_or(source);
    for (index, ((base, input), output)) in source
        .pixels()
        .zip(analysis.pixels())
        .zip(target.pixels_mut())
        .enumerate()
    {
        if base[3] == 0 {
            *output = *base;
            continue;
        }
        let input = if input_scale > 1 {
            let x = index as u32 % source.width() / input_scale * input_scale;
            let y = index as u32 / source.width() / input_scale * input_scale;
            analysis.get_pixel(x, y)
        } else {
            input
        };
        let position = luminance_key(input.0) * last;
        let colour = if mode == PaletteMode::RgbChannels {
            channel_quantize(input.0, levels, 0, 0)
        } else if matches!(
            mode,
            PaletteMode::NearestRgb | PaletteMode::NearestHue | PaletteMode::NearestOklab
        ) {
            palette.colours[chromatic_index(input.0, &features, palette, mode, 0, 0)]
        } else if let Some(stops) = stops {
            let (lower, fraction, span) =
                tonal_interval(luminance_key(input.0), stops, palette.len);
            if mode == PaletteMode::Nearest {
                palette.colours[lower + usize::from(fraction * 2 >= span)]
            } else {
                gradient_colour(
                    palette,
                    &features,
                    lower,
                    lower + 1,
                    fraction,
                    span,
                    interpolation,
                )
            }
        } else if mode == PaletteMode::Nearest {
            palette.colours[((position + 32640) / 65280) as usize]
        } else {
            let lower = position / 65280;
            let upper = (lower + 1).min(last);
            let fraction = position % 65280;
            gradient_colour(
                palette,
                &features,
                lower as usize,
                upper as usize,
                fraction,
                65280,
                interpolation,
            )
        };
        output.0 = mix_rgb(base.0, colour, amount);
    }
}

fn gradient_colour(
    palette: &EvaluatedPalette,
    features: &[[u32; 3]; 16],
    lower: usize,
    upper: usize,
    fraction: u32,
    span: u32,
    interpolation: vestra_core::project::PaletteInterpolation,
) -> [u8; 4] {
    if fraction == 0 || palette.colours[lower] == palette.colours[upper] {
        return palette.colours[lower];
    }
    if fraction == span {
        return palette.colours[upper];
    }
    if interpolation == vestra_core::project::PaletteInterpolation::Oklab {
        return vestra_core::stylization::interpolate_oklab_features(
            features[lower],
            features[upper],
            fraction,
            span,
        );
    }
    std::array::from_fn(|c| {
        let a = u32::from(palette.colours[lower][c]);
        let b = u32::from(palette.colours[upper][c]);
        ((a * (span - fraction) + b * fraction + span / 2) / span) as u8
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "consumes the canonical dither operation controls directly"
)]
pub(super) fn ordered_dither(
    source: &RgbaImage,
    analysis: Option<&RgbaImage>,
    input_scale: u16,
    target: &mut RgbaImage,
    palette: &EvaluatedPalette,
    stops: Option<&[u16; 16]>,
    amount: f64,
    strength: f64,
    matrix: DitherMatrix,
    scale: u8,
    seed: u32,
    mode: PaletteMode,
    levels: u16,
) {
    let bits = match matrix {
        DitherMatrix::Bayer2 => 1,
        DitherMatrix::Bayer4 => 2,
        DitherMatrix::Bayer8 => 3,
        DitherMatrix::BlueNoise => 5,
    };
    let amount = (amount * 65535.0).round() as u32;
    let scale = u32::from(scale);
    let features = palette
        .colours
        .map(|p| vestra_core::stylization::chromatic_features(p, mode));
    let last = palette.len - 1;
    let input_scale = u32::from(input_scale);
    let analysis = analysis.unwrap_or(source);
    let width = source.width();
    for (index, ((base, input), output)) in source
        .pixels()
        .zip(analysis.pixels())
        .zip(target.pixels_mut())
        .enumerate()
    {
        if base[3] == 0 {
            *output = *base;
            continue;
        }
        let input = if input_scale > 1 {
            let x = index as u32 % width / input_scale * input_scale;
            let y = index as u32 / width / input_scale * input_scale;
            analysis.get_pixel(x, y)
        } else {
            input
        };
        let x = index as u32 % width / scale;
        let y = index as u32 / width / scale;
        let rank = if matrix == DitherMatrix::BlueNoise {
            let seed = seed ^ (seed >> 13) ^ (seed >> 26);
            let mut x = (x + (seed & 31)) & 31;
            let mut y = (y + ((seed >> 5) & 31)) & 31;
            if seed & 1024 != 0 {
                std::mem::swap(&mut x, &mut y);
            }
            if seed & 2048 != 0 {
                x = 31 - x;
            }
            if seed & 4096 != 0 {
                y = 31 - y;
            }
            const RANKS: [u16; 1024] = include!("../blue_noise_ranks.in");
            u32::from(RANKS[(y * 32 + x) as usize])
        } else {
            bayer_rank(x, y, bits)
        };
        let threshold = (rank as f32 + 0.5) / (1_u32 << (2 * bits)) as f32;
        let position = tone(input.0) * last as f32 + 0.5 + strength as f32 * (threshold - 0.5);
        let selected = if matches!(
            mode,
            PaletteMode::NearestRgb | PaletteMode::NearestHue | PaletteMode::NearestOklab
        ) {
            chromatic_index(
                input.0,
                &features,
                palette,
                mode,
                (threshold * 4096.0) as u32,
                (strength as f32 * 4096.0).round() as u32,
            )
        } else if let Some(stops) = stops {
            let (lower, fraction, span) =
                tonal_interval(luminance_key(input.0), stops, palette.len);
            let strength = (strength as f32 * 4096.0).round() as u32;
            let nearest = if fraction * 2 >= span { 4096 } else { 0 };
            let probability = ((fraction * 4096 + span / 2) / span * strength
                + nearest * (4096 - strength)
                + 2048)
                / 4096;
            lower + usize::from((threshold * 4096.0) as u32 >= 4096 - probability)
        } else {
            position.floor().clamp(0.0, last as f32) as usize
        };
        let colour = if mode == PaletteMode::RgbChannels {
            channel_quantize(
                input.0,
                levels,
                (threshold * 4096.0) as u32,
                (strength as f32 * 4096.0).round() as u32,
            )
        } else {
            palette.colours[selected]
        };
        output.0 = mix_rgb(base.0, colour, amount);
    }
}

fn tonal_interval(key: u32, stops: &[u16; 16], count: u32) -> (usize, u32, u32) {
    let mut lower = 0;
    while lower + 2 < count as usize && key >= u32::from(stops[lower + 1]) {
        lower += 1;
    }
    (
        lower,
        key - u32::from(stops[lower]),
        u32::from(stops[lower + 1] - stops[lower]),
    )
}

fn channel_quantize(pixel: [u8; 4], levels: u16, threshold: u32, strength: u32) -> [u8; 4] {
    let last = u32::from(levels - 1);
    let mut output = pixel;
    for channel in &mut output[..3] {
        let position = u32::from(*channel) * last;
        let lower = position / 255;
        let fraction = position % 255;
        let nearest = if fraction >= 128 { 4096 } else { 0 };
        let probability =
            ((fraction * 4096 + 127) / 255 * strength + nearest * (4096 - strength) + 2048) / 4096;
        let index = (lower + u32::from(threshold < probability)).min(last);
        *channel = ((index * 255 + last / 2) / last) as u8;
    }
    output
}

fn chromatic_index(
    pixel: [u8; 4],
    features: &[[u32; 3]; 16],
    palette: &EvaluatedPalette,
    mode: PaletteMode,
    threshold: u32,
    strength: u32,
) -> usize {
    let input = vestra_core::stylization::chromatic_features(pixel, mode);
    let mut indices = [0, 0];
    let mut distances = [u32::MAX, u32::MAX];
    for (i, feature) in features[..palette.len as usize].iter().enumerate() {
        if mode == PaletteMode::NearestOklab && pixel[..3] == palette.colours[i][..3] {
            return i;
        }
        let distance = vestra_core::stylization::chromatic_distance(input, *feature, mode);
        if distance < distances[0] {
            distances[1] = distances[0];
            indices[1] = indices[0];
            distances[0] = distance;
            indices[0] = i;
        } else if distance < distances[1] {
            distances[1] = distance;
            indices[1] = i;
        }
    }
    let sum = distances[0] + distances[1];
    let probability = if mode == PaletteMode::NearestOklab {
        ((u64::from(distances[0]) * 4096 + u64::from(sum) / 2)
            .checked_div(u64::from(sum))
            .unwrap_or(0)) as u32
    } else {
        (distances[0] * 4096 + sum / 2)
            .checked_div(sum)
            .unwrap_or(0)
    };
    if threshold < (probability * strength + 2048) / 4096 {
        indices[1]
    } else {
        indices[0]
    }
}

fn bayer_rank(x: u32, y: u32, bits: u32) -> u32 {
    let mut rank = 0;
    for bit in 0..bits {
        let xb = (x >> bit) & 1;
        let yb = (y >> bit) & 1;
        rank = 4 * rank + ((xb ^ yb) << 1) + yb;
    }
    rank
}

fn luminance_key(pixel: [u8; 4]) -> u32 {
    54 * u32::from(pixel[0]) + 183 * u32::from(pixel[1]) + 19 * u32::from(pixel[2])
}

fn tone(pixel: [u8; 4]) -> f32 {
    luminance_key(pixel) as f32 / 65280.0
}

fn mix_rgb(input: [u8; 4], colour: [u8; 4], amount: u32) -> [u8; 4] {
    let mut output = input;
    for c in 0..3 {
        output[c] =
            ((u32::from(input[c]) * (65535 - amount) + u32::from(colour[c]) * amount + 32767)
                / 65535) as u8;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn prepared_input_keeps_original_rgb_and_hidden_alpha_for_blending() {
        let mut palette = EvaluatedPalette {
            colours: [[0, 0, 0, 255]; 16],
            len: 2,
        };
        palette.colours[1] = [255; 4];
        let source = RgbaImage::from_fn(2, 1, |x, _| {
            if x == 0 {
                Rgba([64, 64, 64, 128])
            } else {
                Rgba([27, 39, 51, 0])
            }
        });
        let mut analysis = source.clone();
        super::super::colour_adjust::apply(&source, &mut analysis, 1.0, 1.0, 0.0, 1.0);
        for dither in [false, true] {
            let mut output = source.clone();
            if dither {
                ordered_dither(
                    &source,
                    Some(&analysis),
                    1,
                    &mut output,
                    &palette,
                    None,
                    0.5,
                    0.0,
                    DitherMatrix::BlueNoise,
                    1,
                    0,
                    PaletteMode::Nearest,
                    4,
                );
            } else {
                palette_map(
                    &source,
                    Some(&analysis),
                    1,
                    &mut output,
                    &palette,
                    None,
                    0.5,
                    PaletteMode::Nearest,
                    4,
                    vestra_core::project::PaletteInterpolation::Rgb,
                );
            }
            assert_eq!(output.as_raw(), &[160, 160, 160, 128, 27, 39, 51, 0]);
        }
    }

    #[test]
    fn oklab_gradient_preserves_partial_and_hidden_alpha() {
        let mut palette = EvaluatedPalette {
            colours: [[0, 0, 0, 255]; 16],
            len: 3,
        };
        palette.colours[1] = [255; 4];
        palette.colours[2] = [255, 0, 0, 255];
        let stops = [0, 32768, 65280, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let source = RgbaImage::from_fn(5, 1, |x, _| {
            if x == 4 {
                return Rgba([27, 39, 51, 0]);
            }
            let c = [0, 64, 128, 255][x as usize];
            Rgba([c, c, c, 128])
        });
        let mut output = source.clone();
        palette_map(
            &source,
            None,
            1,
            &mut output,
            &palette,
            Some(&stops),
            1.0,
            PaletteMode::Gradient,
            4,
            vestra_core::project::PaletteInterpolation::Oklab,
        );
        assert_eq!(
            output.as_raw(),
            &[
                0, 0, 0, 128, 99, 99, 99, 128, 255, 255, 255, 128, 255, 0, 0, 128, 27, 39, 51, 0
            ]
        );
        palette_map(
            &source,
            None,
            1,
            &mut output,
            &palette,
            Some(&stops),
            0.0,
            PaletteMode::Gradient,
            4,
            vestra_core::project::PaletteInterpolation::Oklab,
        );
        assert_eq!(source, output);
    }

    #[test]
    fn nonuniform_stops_have_literal_tones_and_dither_coverage() {
        let mut palette = EvaluatedPalette {
            colours: [[0, 0, 0, 255]; 16],
            len: 3,
        };
        let stops = [0, 16384, 65280, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        palette.colours[1] = [255, 0, 0, 255];
        palette.colours[2] = [255; 4];
        // Gray 32 is exactly halfway between the first two authored positions.
        let source = RgbaImage::from_pixel(32, 32, Rgba([32, 32, 32, 128]));
        let mut output = source.clone();
        palette_map(
            &source,
            None,
            1,
            &mut output,
            &palette,
            Some(&stops),
            1.0,
            PaletteMode::Gradient,
            4,
            vestra_core::project::PaletteInterpolation::Rgb,
        );
        assert!(output.pixels().all(|p| p.0 == [128, 0, 0, 128]));
        palette_map(
            &source,
            None,
            1,
            &mut output,
            &palette,
            Some(&stops),
            1.0,
            PaletteMode::Nearest,
            4,
            vestra_core::project::PaletteInterpolation::Rgb,
        );
        assert!(output.pixels().all(|p| p.0 == [255, 0, 0, 128]));
        for matrix in [
            DitherMatrix::Bayer2,
            DitherMatrix::Bayer4,
            DitherMatrix::Bayer8,
            DitherMatrix::BlueNoise,
        ] {
            ordered_dither(
                &source,
                None,
                1,
                &mut output,
                &palette,
                Some(&stops),
                1.0,
                1.0,
                matrix,
                1,
                37,
                PaletteMode::Nearest,
                4,
            );
            assert_eq!(output.pixels().filter(|p| p[0] == 255).count(), 512);
            assert!(
                output
                    .pixels()
                    .all(|p| p[1] == 0 && p[2] == 0 && p[3] == 128)
            );
            ordered_dither(
                &source,
                None,
                1,
                &mut output,
                &palette,
                Some(&stops),
                1.0,
                0.0,
                matrix,
                1,
                37,
                PaletteMode::Nearest,
                4,
            );
            assert!(output.pixels().all(|p| p.0 == [255, 0, 0, 128]));
        }
        let source = RgbaImage::from_fn(3, 1, |x, _| {
            let c = [0, 64, 255][x as usize];
            Rgba([c, c, c, 128])
        });
        let mut output = source.clone();
        palette_map(
            &source,
            None,
            1,
            &mut output,
            &palette,
            Some(&stops),
            1.0,
            PaletteMode::Gradient,
            4,
            vestra_core::project::PaletteInterpolation::Rgb,
        );
        assert_eq!(
            output.as_raw(),
            &[0, 0, 0, 128, 255, 0, 0, 128, 255, 255, 255, 128]
        );
        let source = RgbaImage::from_pixel(4, 1, Rgba([27, 39, 51, 0]));
        let mut output = source.clone();
        palette_map(
            &source,
            None,
            1,
            &mut output,
            &palette,
            Some(&stops),
            1.0,
            PaletteMode::Gradient,
            4,
            vestra_core::project::PaletteInterpolation::Rgb,
        );
        assert_eq!(source, output);
    }

    #[test]
    fn oklab_exact_palette_match_wins_rounded_feature_tie() {
        let mode = PaletteMode::NearestOklab;
        let a = [0, 255, 255, 255];
        let b = [1, 255, 255, 255];
        assert_eq!(
            vestra_core::stylization::chromatic_features(a, mode),
            vestra_core::stylization::chromatic_features(b, mode)
        );
        let mut palette = EvaluatedPalette {
            colours: [a; 16],
            len: 2,
        };
        palette.colours[1] = b;
        let source = RgbaImage::from_pixel(32, 32, Rgba([1, 255, 255, 128]));
        let mut output = source.clone();
        palette_map(
            &source,
            None,
            1,
            &mut output,
            &palette,
            None,
            1.0,
            mode,
            4,
            vestra_core::project::PaletteInterpolation::Rgb,
        );
        assert_eq!(source, output);
        ordered_dither(
            &source,
            None,
            1,
            &mut output,
            &palette,
            None,
            1.0,
            1.0,
            DitherMatrix::BlueNoise,
            1,
            37,
            mode,
            4,
        );
        assert_eq!(source, output);
    }

    #[test]
    fn channel_dither_retains_average_tone_alpha_and_exact_byte_levels() {
        let palette = EvaluatedPalette {
            colours: [[255; 4]; 16],
            len: 2,
        };
        let source = RgbaImage::from_pixel(32, 32, Rgba([64, 128, 192, 128]));
        let mut output = source.clone();
        ordered_dither(
            &source,
            None,
            1,
            &mut output,
            &palette,
            None,
            1.0,
            1.0,
            DitherMatrix::BlueNoise,
            1,
            37,
            PaletteMode::RgbChannels,
            2,
        );
        for (channel, count) in [(0, 257), (1, 514), (2, 771)] {
            assert_eq!(output.pixels().filter(|p| p[channel] == 255).count(), count);
            assert!(output.pixels().all(|p| [0, 255].contains(&p[channel])));
        }
        assert!(output.pixels().all(|p| p[3] == 128));
        ordered_dither(
            &source,
            None,
            1,
            &mut output,
            &palette,
            None,
            1.0,
            0.0,
            DitherMatrix::BlueNoise,
            1,
            37,
            PaletteMode::RgbChannels,
            2,
        );
        assert!(output.pixels().all(|p| p.0 == [0, 255, 255, 128]));
        for strength in [0.0, 0.5, 1.0] {
            ordered_dither(
                &source,
                None,
                1,
                &mut output,
                &palette,
                None,
                1.0,
                strength,
                DitherMatrix::BlueNoise,
                1,
                37,
                PaletteMode::RgbChannels,
                256,
            );
            assert_eq!(source, output);
        }
        let hidden = RgbaImage::from_pixel(32, 32, Rgba([37, 92, 154, 0]));
        ordered_dither(
            &hidden,
            None,
            1,
            &mut output,
            &palette,
            None,
            1.0,
            1.0,
            DitherMatrix::BlueNoise,
            1,
            37,
            PaletteMode::RgbChannels,
            2,
        );
        assert_eq!(hidden, output);
    }

    #[test]
    fn chromatic_ties_duplicates_and_transparency_are_stable() {
        let mut palette = EvaluatedPalette {
            colours: [[255, 0, 0, 255]; 16],
            len: 2,
        };
        palette.colours[1] = [0, 0, 255, 255];
        for mode in [PaletteMode::NearestRgb, PaletteMode::NearestHue] {
            let source = RgbaImage::from_pixel(32, 32, Rgba([127, 0, 127, 128]));
            let mut output = source.clone();
            palette_map(
                &source,
                None,
                1,
                &mut output,
                &palette,
                None,
                1.0,
                mode,
                4,
                vestra_core::project::PaletteInterpolation::Rgb,
            );
            assert!(output.pixels().all(|p| p.0 == [255, 0, 0, 128]));
            for strength in [0.0, 1.0] {
                ordered_dither(
                    &source,
                    None,
                    1,
                    &mut output,
                    &palette,
                    None,
                    1.0,
                    strength,
                    DitherMatrix::BlueNoise,
                    1,
                    37,
                    mode,
                    4,
                );
                assert_eq!(
                    output.pixels().filter(|p| p[2] == 255).count(),
                    if strength == 0.0 { 0 } else { 512 },
                );
                assert!(output.pixels().all(|p| p[3] == 128));
            }
            let hidden = RgbaImage::from_pixel(32, 32, Rgba([37, 92, 154, 0]));
            palette_map(
                &hidden,
                None,
                1,
                &mut output,
                &palette,
                None,
                1.0,
                mode,
                4,
                vestra_core::project::PaletteInterpolation::Rgb,
            );
            assert_eq!(hidden, output);
            ordered_dither(
                &hidden,
                None,
                1,
                &mut output,
                &palette,
                None,
                1.0,
                1.0,
                DitherMatrix::BlueNoise,
                1,
                0,
                mode,
                4,
            );
            assert_eq!(hidden, output);
            let features = [[255, 0, 0]; 16];
            assert_eq!(
                chromatic_index(
                    [255, 0, 0, 255],
                    &features,
                    &palette,
                    PaletteMode::NearestRgb,
                    0,
                    4096
                ),
                0
            );
        }
    }

    #[test]
    fn blue_noise_tile_has_uniform_tone_coverage_and_preserves_alpha() {
        let palette = EvaluatedPalette {
            colours: [[255; 4]; 16],
            len: 2,
        };
        let mut palette = palette;
        palette.colours[0] = [0, 0, 0, 255];
        for (tone, expected_white) in [(0, 0), (64, 257), (128, 514), (192, 771), (255, 1024)] {
            let source = RgbaImage::from_pixel(32, 32, Rgba([tone, tone, tone, 128]));
            let mut output = source.clone();
            ordered_dither(
                &source,
                None,
                1,
                &mut output,
                &palette,
                None,
                1.0,
                1.0,
                DitherMatrix::BlueNoise,
                1,
                37,
                PaletteMode::Nearest,
                4,
            );
            assert_eq!(
                output.pixels().filter(|p| p[0] == 255).count(),
                expected_white
            );
            assert!(output.pixels().all(|p| p[3] == 128));
        }
        let source = RgbaImage::from_pixel(32, 32, Rgba([37, 92, 154, 0]));
        let mut output = source.clone();
        ordered_dither(
            &source,
            None,
            1,
            &mut output,
            &palette,
            None,
            1.0,
            1.0,
            DitherMatrix::BlueNoise,
            1,
            0,
            PaletteMode::Nearest,
            4,
        );
        assert_eq!(source, output);
    }

    #[test]
    fn bayer_output_is_independent_of_blue_noise_seed() {
        let palette = EvaluatedPalette {
            colours: [[255; 4]; 16],
            len: 2,
        };
        let mut palette = palette;
        palette.colours[0] = [0, 0, 0, 255];
        let source = RgbaImage::from_pixel(32, 32, Rgba([128, 128, 128, 255]));
        for matrix in [
            DitherMatrix::Bayer2,
            DitherMatrix::Bayer4,
            DitherMatrix::Bayer8,
        ] {
            let mut a = source.clone();
            let mut b = source.clone();
            ordered_dither(
                &source,
                None,
                1,
                &mut a,
                &palette,
                None,
                1.0,
                1.0,
                matrix,
                1,
                0,
                PaletteMode::Nearest,
                4,
            );
            ordered_dither(
                &source,
                None,
                1,
                &mut b,
                &palette,
                None,
                1.0,
                1.0,
                matrix,
                1,
                u32::MAX,
                PaletteMode::Nearest,
                4,
            );
            assert_eq!(a, b);
            assert_eq!(a.pixels().filter(|p| p[0] == 255).count(), 512);
        }
    }
}
