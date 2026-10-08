//! Image-anchored tonal palette mapping and ordered dithering.

use image::RgbaImage;
use vestra_core::{project::DitherMatrix, stylization::EvaluatedPalette};

pub(super) fn palette_map(
    source: &RgbaImage,
    target: &mut RgbaImage,
    palette: &EvaluatedPalette,
    amount: f64,
    nearest: bool,
) {
    let last = palette.len - 1;
    let amount = (amount * 65535.0).round() as u32;
    for (input, output) in source.pixels().zip(target.pixels_mut()) {
        if input[3] == 0 {
            *output = *input;
            continue;
        }
        let position = luminance_key(input.0) * last;
        let colour = if nearest {
            palette.colours[((position + 32640) / 65280) as usize]
        } else {
            let lower = position / 65280;
            let upper = (lower + 1).min(last);
            let fraction = position % 65280;
            std::array::from_fn(|c| {
                let a = u32::from(palette.colours[lower as usize][c]);
                let b = u32::from(palette.colours[upper as usize][c]);
                ((a * (65280 - fraction) + b * fraction + 32640) / 65280) as u8
            })
        };
        output.0 = mix_rgb(input.0, colour, amount);
    }
}

pub(super) fn ordered_dither(
    source: &RgbaImage,
    target: &mut RgbaImage,
    palette: &EvaluatedPalette,
    amount: f64,
    strength: f64,
    matrix: DitherMatrix,
    scale: u8,
) {
    let bits = match matrix {
        DitherMatrix::Bayer2 => 1,
        DitherMatrix::Bayer4 => 2,
        DitherMatrix::Bayer8 => 3,
    };
    let amount = (amount * 65535.0).round() as u32;
    let scale = u32::from(scale);
    let last = palette.len - 1;
    let width = source.width();
    for (index, (input, output)) in source.pixels().zip(target.pixels_mut()).enumerate() {
        if input[3] == 0 {
            *output = *input;
            continue;
        }
        let x = index as u32 % width / scale;
        let y = index as u32 / width / scale;
        let threshold = (bayer_rank(x, y, bits) as f32 + 0.5) / (1_u32 << (2 * bits)) as f32;
        let position = tone(input.0) * last as f32 + 0.5 + strength as f32 * (threshold - 0.5);
        let selected = position.floor().clamp(0.0, last as f32) as usize;
        output.0 = mix_rgb(input.0, palette.colours[selected], amount);
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
