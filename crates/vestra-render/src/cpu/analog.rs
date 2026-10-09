//! Stateless screen printing, bounded sorting and analog display simulation.
use image::{Rgba, RgbaImage};
use std::collections::HashMap;
use vestra_core::{
    plan::EffectOperation,
    project::{HalftoneMode, PixelSortDirection, PixelSortOrder},
};

fn bytes(p: [f32; 4]) -> Rgba<u8> {
    Rgba(p.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8))
}
fn rgba(p: Rgba<u8>) -> [f32; 4] {
    p.0.map(|v| f32::from(v) / 255.0)
}
fn key(p: Rgba<u8>) -> u32 {
    54 * u32::from(p[0]) + 183 * u32::from(p[1]) + 19 * u32::from(p[2])
}
fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let alpha = a[3] * (1.0 - t) + b[3] * t;
    if alpha <= 1e-7 {
        return [0.0; 4];
    }
    [
        (a[0] * a[3] * (1.0 - t) + b[0] * b[3] * t) / alpha,
        (a[1] * a[3] * (1.0 - t) + b[1] * b[3] * t) / alpha,
        (a[2] * a[3] * (1.0 - t) + b[2] * b[3] * t) / alpha,
        alpha,
    ]
}
fn sample(source: &RgbaImage, x: f32, y: f32) -> [f32; 4] {
    let x = (x - 0.5).clamp(0.0, source.width() as f32 - 1.0);
    let y = (y - 0.5).clamp(0.0, source.height() as f32 - 1.0);
    let ix = x.floor() as u32;
    let iy = y.floor() as u32;
    let a = rgba(*source.get_pixel(ix, iy));
    let b = rgba(*source.get_pixel((ix + 1).min(source.width() - 1), iy));
    let c = rgba(*source.get_pixel(ix, (iy + 1).min(source.height() - 1)));
    let d = rgba(*source.get_pixel(
        (ix + 1).min(source.width() - 1),
        (iy + 1).min(source.height() - 1),
    ));
    rgba(bytes(mix(
        mix(a, b, x - ix as f32),
        mix(c, d, x - ix as f32),
        y - iy as f32,
    )))
}

pub(super) fn pixel_sort(source: &RgbaImage, target: &mut RgbaImage, operation: EffectOperation) {
    let EffectOperation::PixelSort {
        direction,
        order,
        lower_threshold: lower,
        upper_threshold: upper,
        segment_length: length,
        amount,
    } = operation
    else {
        unreachable!()
    };
    let vertical = direction == PixelSortDirection::Vertical;
    let (lines, extent) = if vertical {
        (source.width(), source.height())
    } else {
        (source.height(), source.width())
    };
    let coord = |line, pos| if vertical { (line, pos) } else { (pos, line) };
    let lo = (lower * 65280.0).ceil() as u32;
    let hi = (upper * 65280.0).floor() as u32;
    let eligible = |p: Rgba<u8>| p[3] != 0 && key(p) >= lo && key(p) <= hi && lower <= upper;
    let mut run = Vec::with_capacity(usize::from(length));
    for line in 0..lines {
        for block in (0..extent).step_by(usize::from(length)) {
            let end = (block + u32::from(length)).min(extent);
            let mut pos = block;
            while pos < end {
                let (x, y) = coord(line, pos);
                let p = *source.get_pixel(x, y);
                if !eligible(p) {
                    *target.get_pixel_mut(x, y) = p;
                    pos += 1;
                    continue;
                }
                let start = pos;
                run.clear();
                while pos < end {
                    let (x, y) = coord(line, pos);
                    let p = *source.get_pixel(x, y);
                    if !eligible(p) {
                        break;
                    }
                    run.push(p);
                    pos += 1;
                }
                run.sort_by_key(|p| {
                    if order == PixelSortOrder::Ascending {
                        key(*p)
                    } else {
                        65280 - key(*p)
                    }
                });
                for (offset, p) in run.iter().enumerate() {
                    let (x, y) = coord(line, start + offset as u32);
                    *target.get_pixel_mut(x, y) =
                        bytes(mix(rgba(*source.get_pixel(x, y)), rgba(*p), amount as f32));
                }
            }
        }
    }
}

fn cell(x: f32, y: f32, size: f32, s: f32, c: f32) -> [i32; 2] {
    // Integer pixel centers keep GPU fused arithmetic from changing cell membership.
    let x = (x * 2.0) as i32;
    let y = (y * 2.0) as i32;
    let s = (s * crate::halftone::GRID_SCALE) as i32;
    let c = (c * crate::halftone::GRID_SCALE) as i32;
    let denominator = (size * crate::halftone::GRID_SCALE) as i32 * 2;
    [
        (x * c - y * s).div_euclid(denominator),
        (x * s + y * c).div_euclid(denominator),
    ]
}

fn bounds(id: [i32; 2], size: f32, s: f32, c: f32, width: u32, height: u32) -> [i32; 4] {
    let mut lo = [f32::INFINITY; 2];
    let mut hi = [f32::NEG_INFINITY; 2];
    for dx in 0..=1 {
        for dy in 0..=1 {
            let u = (id[0] + dx) as f32 * size;
            let v = (id[1] + dy) as f32 * size;
            let norm = s * s + c * c;
            let p = [(u * c + v * s) / norm, (-u * s + v * c) / norm];
            for k in 0..2 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
    }
    [
        (lo[0] - 0.5).floor().max(0.0) as i32,
        (lo[1] - 0.5).floor().max(0.0) as i32,
        (hi[0] - 0.5).ceil().min(width as f32 - 1.0) as i32,
        (hi[1] - 0.5).ceil().min(height as f32 - 1.0) as i32,
    ]
}
fn interval(lo: f32, hi: f32, a: f32, b: f32) -> [f32; 2] {
    if a.abs() < 1e-6 {
        if b >= lo && b < hi {
            [-1e20, 1e20]
        } else {
            [1e20, -1e20]
        }
    } else {
        let x = (lo - b) / a;
        let y = (hi - b) / a;
        [x.min(y), x.max(y)]
    }
}
fn row_span(id: [i32; 2], size: f32, s: f32, c: f32, y: i32, width: u32) -> [i32; 2] {
    let yf = y as f32 + 0.5;
    let u = interval(id[0] as f32 * size, (id[0] + 1) as f32 * size, c, -yf * s);
    let v = interval(id[1] as f32 * size, (id[1] + 1) as f32 * size, s, yf * c);
    [
        (u[0].max(v[0]) - 0.5).floor().clamp(0.0, width as f32) as i32,
        (u[1].min(v[1]) - 0.5)
            .ceil()
            .clamp(-1.0, width as f32 - 1.0) as i32,
    ]
}
fn representative(id: [i32; 2], size: f32, s: f32, c: f32, width: u32, height: u32) -> [u32; 2] {
    let b = bounds(id, size, s, c, width, height);
    for y in b[1]..=b[3] {
        let span = row_span(id, size, s, c, y, width);
        for x in span[0]..=span[1].min(span[0] + 3) {
            if cell(x as f32 + 0.5, y as f32 + 0.5, size, s, c) == id {
                return [x as u32, y as u32];
            }
        }
    }
    unreachable!("a source pixel establishes that this cell is nonempty")
}
pub(super) fn analyze(
    source: &RgbaImage,
    target: &mut RgbaImage,
    size: f64,
    angle: f64,
    mode: HalftoneMode,
) {
    let (size, orientations) = crate::halftone::grid_parameters(size, angle);
    target.fill(0);
    let count = if mode == HalftoneMode::Rgb { 3 } else { 1 };
    for channel in 0..count {
        let [s, c] = orientations[channel];
        let mut cells = HashMap::<[i32; 2], ([u32; 2], [u64; 4])>::new();
        for (x, y, p) in source.enumerate_pixels() {
            let id = cell(x as f32 + 0.5, y as f32 + 0.5, size, s, c);
            let entry = cells.entry(id).or_insert(([x, y], [0; 4]));
            for k in 0..3 {
                entry.1[k] += u64::from(p[k]) * u64::from(p[3]);
            }
            entry.1[3] += u64::from(p[3]);
        }
        for (_, (rep, sum)) in cells {
            let out = target.get_pixel_mut(rep[0], rep[1]);
            if mode == HalftoneMode::Rgb {
                out[channel] = (sum[channel] + sum[3] / 2).checked_div(sum[3]).unwrap_or(0) as u8;
            } else {
                for k in 0..3 {
                    out[k] = (sum[k] + sum[3] / 2).checked_div(sum[3]).unwrap_or(0) as u8;
                }
            }
            out[3] = 255;
        }
    }
}

fn smooth(lo: f32, hi: f32, v: f32) -> f32 {
    let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn dot(
    analysis: &RgbaImage,
    position: [f32; 2],
    geometry: [f32; 2],
    orientation: [f32; 2],
    channel: Option<usize>,
    invert: bool,
    reps: &mut HashMap<(i32, i32, usize), [u32; 2]>,
) -> (f32, [f32; 4]) {
    let [x, y] = position;
    let [size, softness] = geometry;
    let [s, c] = orientation;
    let u = (x * c - y * s) / size;
    let v = (x * s + y * c) / size;
    let id = cell(x, y, size, s, c);
    let cx = (id[0] as f32 + 0.5) * size;
    let cy = (id[1] as f32 + 0.5) * size;
    let rep = *reps
        .entry((id[0], id[1], channel.map_or(0, |ch| ch + 1)))
        .or_insert_with(|| representative(id, size, s, c, analysis.width(), analysis.height()));
    let colour = rgba(*analysis.get_pixel(rep[0], rep[1]));
    let mut tone = if let Some(channel) = channel {
        colour[channel]
    } else {
        (54.0 * colour[0] + 183.0 * colour[1] + 19.0 * colour[2]) / 256.0
    };
    if invert {
        tone = 1.0 - tone;
    }
    let radius = tone.sqrt() * size / std::f32::consts::SQRT_2;
    let distance = ((u * size - cx).powi(2) + (v * size - cy).powi(2)).sqrt();
    let coverage = if tone <= 0.0 {
        0.0
    } else if tone >= 1.0 {
        1.0
    } else if softness <= 0.0 {
        f32::from(distance <= radius)
    } else {
        smooth(distance - softness / 2.0, distance + softness / 2.0, radius)
    };
    (coverage, colour)
}
pub(super) fn halftone(
    source: &RgbaImage,
    analysis: &RgbaImage,
    target: &mut RgbaImage,
    operation: EffectOperation,
) {
    let EffectOperation::Halftone {
        cell_size,
        angle_degrees,
        softness,
        amount,
        mode,
        foreground,
        background,
        invert,
    } = operation
    else {
        unreachable!()
    };
    let mut reps = HashMap::new();
    let foreground = rgba(Rgba(foreground));
    let background = rgba(Rgba(background));
    let (cell_size, orientations) = crate::halftone::grid_parameters(cell_size, angle_degrees);
    for (x, y, out) in target.enumerate_pixels_mut() {
        let original = rgba(*source.get_pixel(x, y));
        if original[3] == 0.0 {
            *out = *source.get_pixel(x, y);
            continue;
        }
        let (mut screen, col) = dot(
            analysis,
            [x as f32 + 0.5, y as f32 + 0.5],
            [cell_size, softness as f32],
            orientations[0],
            None,
            invert,
            &mut reps,
        );
        let mut rgb = [0.0; 3];
        for ch in 0..3 {
            if mode == HalftoneMode::Rgb {
                screen = dot(
                    analysis,
                    [x as f32 + 0.5, y as f32 + 0.5],
                    [cell_size, softness as f32],
                    orientations[ch],
                    Some(ch),
                    invert,
                    &mut reps,
                )
                .0;
            }
            let fg = if mode == HalftoneMode::Source {
                col[ch]
            } else {
                foreground[ch]
            };
            rgb[ch] = background[ch] * (1.0 - screen) + fg * screen;
        }
        *out = bytes(std::array::from_fn(|ch| {
            if ch == 3 {
                original[3]
            } else {
                original[ch] + (rgb[ch] - original[ch]) * amount as f32
            }
        }));
    }
}
fn hash(mut v: u32) -> u32 {
    v ^= v >> 16;
    v = v.wrapping_mul(0x7feb352d);
    v ^= v >> 15;
    v = v.wrapping_mul(0x846ca68b);
    v ^ (v >> 16)
}
fn noise(x: u32, y: u32, seed: u32, phase: f32) -> f32 {
    let h = hash(x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263) ^ seed);
    let a = (h & 65535) as f32 / 32767.5 - 1.0;
    let b = (h >> 16) as f32 / 32767.5 - 1.0;
    a.mul_add(phase.cos(), b * phase.sin())
}
pub(super) fn crt(source: &RgbaImage, target: &mut RgbaImage, operation: EffectOperation) {
    let EffectOperation::Crt {
        amount,
        curvature,
        scanline_strength,
        scanline_spacing,
        mask_strength,
        grain,
        jitter,
        flicker,
        rolling_strength,
        rolling_width,
        phase,
        mask_spacing,
        seed,
    } = operation
    else {
        unreachable!()
    };
    let phase = phase as f32;
    let seed = (seed as u32) ^ ((seed >> 32) as u32);
    let sampling = crate::crt::Sampling::new(curvature, jitter, seed, phase);
    let width = source.width() as f32;
    let height = source.height() as f32;
    for (x, y, out) in target.enumerate_pixels_mut() {
        let original = rgba(*source.get_pixel(x, y));
        let [sx, sy] = sampling.position(x, y, source.width(), source.height());
        let edge = (sx.min(width - sx).min(sy.min(height - sy)) + 0.5).clamp(0.0, 1.0);
        let mut screen = sample(source, sx, sy);
        screen[3] *= edge;
        // Pixel-width integration prevents scanline aliasing as authored spacing changes.
        let freq = std::f32::consts::TAU / scanline_spacing as f32;
        let aa = (freq / 2.0).sin() / (freq / 2.0);
        let scan = 1.0 - scanline_strength as f32 * (0.5 - 0.5 * (y as f32 * freq).cos() * aa);
        let rolling_phase = (y as f32 + 0.5) / height * std::f32::consts::TAU - phase;
        let roll = (-2.0 * rolling_phase.sin().powi(2)
            / (rolling_width as f32 * std::f32::consts::TAU).powi(2))
        .exp();
        let light =
            scan * (1.0 + flicker as f32 * phase.sin()) * (1.0 - rolling_strength as f32 * roll);
        let n = grain as f32 * noise(x, y, seed, phase * 3.0);
        for (ch, v) in screen.iter_mut().enumerate().take(3) {
            let mask = if (x / u32::from(mask_spacing)) % 3 == ch as u32 {
                1.0
            } else {
                1.0 - mask_strength as f32
            };
            *v = (*v * light * mask + n).clamp(0.0, 1.0);
        }
        *out = bytes(mix(original, screen, amount as f32));
        // Curvature can leave sub-byte coverage; match transparent compositing
        // after alpha quantization instead of retaining invisible straight RGB.
        if out[3] == 0 {
            *out = Rgba([0; 4]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sorting_is_stable_and_bounded_in_both_directions() {
        for direction in [PixelSortDirection::Horizontal, PixelSortDirection::Vertical] {
            let vertical = direction == PixelSortDirection::Vertical;
            let mut src =
                RgbaImage::new(if vertical { 1 } else { 7 }, if vertical { 7 } else { 1 });
            for (i, p) in src.pixels_mut().enumerate() {
                *p = Rgba([((6 - i) * 30) as u8; 4]);
                p[3] = 255;
            }
            let mut out = src.clone();
            pixel_sort(
                &src,
                &mut out,
                EffectOperation::PixelSort {
                    direction,
                    order: PixelSortOrder::Ascending,
                    lower_threshold: 0.0,
                    upper_threshold: 1.0,
                    segment_length: 4,
                    amount: 1.0,
                },
            );
            let values = out.pixels().map(|p| p[0]).collect::<Vec<_>>();
            assert_eq!(values, [90, 120, 150, 180, 0, 30, 60]);
        }
    }
    #[test]
    fn diagonal_grid_membership_is_exact_at_output_limits() {
        let (size, [[s, c], ..]) = crate::halftone::grid_parameters(2.0, 45.0);
        for pixel in [0, 1, 100, 4095, 8191] {
            let center = pixel as f32 + 0.5;
            assert_eq!(cell(center, center, size, s, c)[0], 0);
            assert!(cell(center + 1.0, center, size, s, c)[0] >= 0);
            assert!(cell(center, center + 1.0, size, s, c)[0] < 0);
        }
    }
    #[test]
    fn analysis_ignores_hidden_colour_and_clips_borders() {
        let mut src = RgbaImage::new(2, 1);
        src.put_pixel(0, 0, Rgba([255, 0, 0, 0]));
        src.put_pixel(1, 0, Rgba([0, 100, 200, 255]));
        let mut out = src.clone();
        analyze(&src, &mut out, 4.0, 0.0, HalftoneMode::Luminance);
        assert_eq!(out.get_pixel(0, 0).0, [0, 100, 200, 255]);
    }
    #[test]
    fn rotated_representatives_are_injective_and_valid() {
        for (w, h) in [(1, 19), (19, 1), (19, 17)] {
            for angle in [
                0.0_f32,
                0.2617994,
                std::f32::consts::FRAC_PI_4,
                std::f32::consts::FRAC_PI_2,
                2.3561945,
            ] {
                let [s, c] = crate::halftone::grid_parameters(
                    6.0,
                    f64::from(angle) * 180.0 / f64::from(std::f32::consts::PI),
                )
                .1[0];
                for y in 0..h {
                    for x in 0..w {
                        let id = cell(x as f32 + 0.5, y as f32 + 0.5, 6.0, s, c);
                        let r = representative(id, 6.0, s, c, w, h);
                        assert_eq!(cell(r[0] as f32 + 0.5, r[1] as f32 + 0.5, 6.0, s, c), id);
                    }
                }
            }
        }
    }
    #[test]
    fn crt_clears_colour_when_curved_edge_alpha_rounds_to_zero() {
        let source = RgbaImage::from_pixel(6, 6, Rgba([60, 157, 79, 1]));
        let mut target = source.clone();
        crt(
            &source,
            &mut target,
            EffectOperation::Crt {
                amount: 1.0,
                curvature: 0.4,
                scanline_strength: 0.0,
                scanline_spacing: 2.0,
                mask_strength: 0.0,
                mask_spacing: 1,
                grain: 0.0,
                jitter: 0.0,
                flicker: 0.0,
                rolling_strength: 0.0,
                rolling_width: 0.12,
                phase: 0.0,
                seed: 0,
            },
        );
        assert_eq!(*target.get_pixel(0, 0), Rgba([0; 4]));
        assert_eq!(*target.get_pixel(1, 1), *source.get_pixel(1, 1));
    }
    #[test]
    fn default_scanline_spacing_alternates_and_one_pixel_spacing_is_filtered() {
        let source = RgbaImage::from_pixel(2, 4, Rgba([128, 128, 128, 255]));
        let mut target = source.clone();
        let mut operation = EffectOperation::Crt {
            amount: 1.0,
            curvature: 0.0,
            scanline_strength: 0.2,
            scanline_spacing: 2.0,
            mask_strength: 0.0,
            mask_spacing: 1,
            grain: 0.0,
            jitter: 0.0,
            flicker: 0.0,
            rolling_strength: 0.0,
            rolling_width: 0.12,
            phase: 0.0,
            seed: 0,
        };
        crt(&source, &mut target, operation);
        assert!(target.get_pixel(0, 0)[0] > target.get_pixel(0, 1)[0]);
        assert_eq!(target.get_pixel(0, 0), target.get_pixel(0, 2));
        if let EffectOperation::Crt {
            scanline_spacing, ..
        } = &mut operation
        {
            *scanline_spacing = 1.0;
        }
        crt(&source, &mut target, operation);
        assert!(target.pixels().all(|p| p == target.get_pixel(0, 0)));
    }
    #[test]
    fn halftone_preserves_black_and_white_with_soft_edges() {
        for tone in [0, 255] {
            let source = RgbaImage::from_pixel(8, 8, Rgba([tone, tone, tone, 255]));
            let mut analysis = source.clone();
            let mut target = source.clone();
            analyze(&source, &mut analysis, 6.0, 15.0, HalftoneMode::Luminance);
            halftone(
                &source,
                &analysis,
                &mut target,
                EffectOperation::Halftone {
                    cell_size: 6.0,
                    angle_degrees: 15.0,
                    softness: 2.0,
                    amount: 1.0,
                    mode: HalftoneMode::Luminance,
                    foreground: [255; 4],
                    background: [0, 0, 0, 255],
                    invert: false,
                },
            );
            assert_eq!(target, source);
        }
    }
    #[test]
    fn noise_is_continuous_at_period_boundary() {
        for x in 0..20 {
            let a = noise(x, 7, 123, 0.0);
            let b = noise(x, 7, 123, std::f32::consts::TAU);
            assert!((a - b).abs() < 1e-6);
            let c = noise(x, 7, 123, 0.001);
            assert!((a - c).abs() < 0.003);
        }
    }
}
