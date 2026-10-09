//! Canvas-anchored area analysis followed by antialiased glyph resolution.
use image::{Rgba, RgbaImage};
use vestra_core::{
    ascii::AsciiParameters,
    project::{AsciiColorMode, AsciiGlyphStyle, AsciiMode},
};

pub(super) fn analyze(source: &RgbaImage, target: &mut RgbaImage, p: AsciiParameters) {
    if source.width() < 2 || source.height() < 2 {
        return;
    }
    let columns = source.width().div_ceil(p.cell_width);
    for cy in 0..source.height().div_ceil(p.cell_height) {
        for cx in 0..columns {
            let (color, glyph) = cell(source, cx, cy, p);
            let index = 2 * (cy * columns + cx);
            target.put_pixel(index % source.width(), index / source.width(), Rgba(color));
            target.put_pixel(
                (index + 1) % source.width(),
                (index + 1) / source.width(),
                Rgba(glyph),
            );
        }
    }
}

fn cell(source: &RgbaImage, cx: u32, cy: u32, p: AsciiParameters) -> ([u8; 4], [u8; 4]) {
    let x0 = cx * p.cell_width;
    let y0 = cy * p.cell_height;
    let x1 = (x0 + p.cell_width).min(source.width());
    let y1 = (y0 + p.cell_height).min(source.height());
    let mut sums = [0u32; 4];
    let mut halves = [[0u32; 2]; 4];
    for y in y0..y1 {
        for x in x0..x1 {
            let rgba = source.get_pixel(x, y).0;
            let alpha = u32::from(rgba[3]);
            for c in 0..3 {
                sums[c] += u32::from(rgba[c]) * alpha;
            }
            sums[3] += alpha;
            let luma = (54 * u32::from(rgba[0])
                + 183 * u32::from(rgba[1])
                + 19 * u32::from(rgba[2])
                + 128)
                / 256;
            for side in [
                usize::from(2 * (x - x0) >= x1 - x0),
                2 + usize::from(2 * (y - y0) >= y1 - y0),
            ] {
                halves[side][0] += luma * alpha;
                halves[side][1] += alpha;
            }
        }
    }
    let denominator = sums[3].max(1);
    let color = [
        ((sums[0] + sums[3] / 2) / denominator) as u8,
        ((sums[1] + sums[3] / 2) / denominator) as u8,
        ((sums[2] + sums[3] / 2) / denominator) as u8,
        ((sums[3] + (x1 - x0) * (y1 - y0) / 2) / ((x1 - x0) * (y1 - y0))) as u8,
    ];
    let tone = (54 * u32::from(color[0]) + 183 * u32::from(color[1]) + 19 * u32::from(color[2]))
        as f32
        / 65280.0;
    let tone = if p.invert { 1.0 - tone } else { tone };
    let position = tone * (p.glyph_count - 1) as f32;
    // A narrow stateless crossfade at a density boundary avoids history/hysteresis.
    let index = position.floor().min((p.glyph_count - 1) as f32) as u32;
    let fraction = ((position - index as f32 - 0.4375) * 8.0).clamp(0.0, 1.0);
    let mean = |side: usize| {
        if halves[side][1] == 0 {
            0.0
        } else {
            halves[side][0] as f32 / halves[side][1] as f32 / 255.0
        }
    };
    let gx = mean(1) - mean(0);
    let gy = mean(3) - mean(2);
    let magnitude = (gx * gx + gy * gy).sqrt();
    let orientation = if gx.abs() > gy.abs() * 2.4142137 {
        1
    } else if gy.abs() > gx.abs() * 2.4142137 {
        0
    } else if gx * gy >= 0.0 {
        2
    } else {
        3
    };
    let edge = ((magnitude - p.edge_threshold as f32) * 8.0 + 0.5).clamp(0.0, 1.0)
        * p.edge_strength as f32;
    (
        color,
        [
            index as u8,
            (fraction * 255.0).round() as u8,
            orientation,
            (edge.clamp(0.0, 1.0) * 255.0).round() as u8,
        ],
    )
}

pub(super) fn resolve(
    source: &RgbaImage,
    analysis: &RgbaImage,
    target: &mut RgbaImage,
    atlas: &crate::ascii::PreparedGlyphAtlas,
    p: AsciiParameters,
) {
    let columns = source.width().div_ceil(p.cell_width);
    let level = crate::ascii::coverage_level(p.cell_width, p.cell_height);
    let atlas = &atlas.levels[level as usize];
    for (x, y, output) in target.enumerate_pixels_mut() {
        let input = source.get_pixel(x, y).0;
        if input[3] == 0 {
            *output = Rgba(input);
            continue;
        }
        let (color, info) = if source.width() < 2 || source.height() < 2 {
            cell(source, x / p.cell_width, y / p.cell_height, p)
        } else {
            let index = 2 * ((y / p.cell_height) * columns + x / p.cell_width);
            (
                analysis
                    .get_pixel(index % source.width(), index / source.width())
                    .0,
                analysis
                    .get_pixel((index + 1) % source.width(), (index + 1) / source.width())
                    .0,
            )
        };
        let uv = [
            (x % p.cell_width) as f32 + 0.5,
            (y % p.cell_height) as f32 + 0.5,
        ];
        let uv = [uv[0] / p.cell_width as f32, uv[1] / p.cell_height as f32];
        let first = coverage(atlas, level, u32::from(info[0]), uv, p, false);
        let second = coverage(
            atlas,
            level,
            (u32::from(info[0]) + 1).min(p.glyph_count - 1),
            uv,
            p,
            false,
        );
        let fill = first + (second - first) * (f32::from(info[1]) / 255.0);
        let edge = coverage(
            atlas,
            level,
            p.glyph_count + u32::from(info[2]),
            uv,
            p,
            true,
        );
        let edge_weight = f32::from(info[3]) / 255.0;
        let ink = match p.mode {
            AsciiMode::Fill => fill,
            AsciiMode::Edges => edge * edge_weight,
            AsciiMode::Hybrid => fill + (edge - fill) * edge_weight,
        };
        let foreground = match p.color_mode {
            AsciiColorMode::Monochrome => p.foreground,
            AsciiColorMode::Source => [color[0], color[1], color[2], 255],
            AsciiColorMode::Palette | AsciiColorMode::Rainbow => {
                let tone = (54 * u32::from(color[0])
                    + 183 * u32::from(color[1])
                    + 19 * u32::from(color[2])) as f32
                    / 65280.0;
                let pos = (if p.invert { 1.0 - tone } else { tone }) * (p.palette.len - 1) as f32;
                let lower = pos.floor() as usize;
                let upper = (lower + 1).min(p.palette.len as usize - 1);
                std::array::from_fn(|c| {
                    (f32::from(p.palette.colours[lower][c]) * (1.0 - pos.fract())
                        + f32::from(p.palette.colours[upper][c]) * pos.fract())
                    .round() as u8
                })
            }
        };
        let fa = f32::from(foreground[3]) / 255.0 * ink;
        let ba = f32::from(p.background[3]) / 255.0 * (1.0 - ink);
        let style_alpha = (fa + ba) * f32::from(input[3]) / 255.0;
        let strength = (p.amount * (1.0 - p.source_mix)) as f32;
        let original_alpha = f32::from(input[3]) / 255.0;
        let alpha = original_alpha * (1.0 - strength) + style_alpha * strength;
        let mut result = [0; 4];
        for c in 0..3 {
            let styled =
                (f32::from(foreground[c]) * fa + f32::from(p.background[c]) * ba) * original_alpha;
            let premul =
                f32::from(input[c]) * original_alpha * (1.0 - strength) + styled * strength;
            result[c] = if alpha > 0.0 {
                (premul / alpha).round().clamp(0.0, 255.0) as u8
            } else {
                0
            };
        }
        result[3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
        *output = Rgba(result);
    }
}

fn coverage(
    atlas: &RgbaImage,
    level: u32,
    index: u32,
    uv: [f32; 2],
    p: AsciiParameters,
    edge: bool,
) -> f32 {
    if p.glyph_style == AsciiGlyphStyle::Geometric {
        let x = (uv[0] - 0.5) * p.cell_width as f32;
        let y = (uv[1] - 0.5) * p.cell_height as f32;
        let aa = |distance: f32| (0.75 - distance).clamp(0.0, 1.0);
        if edge {
            return match index - p.glyph_count {
                0 => aa(y.abs()),
                1 => aa(x.abs()),
                2 => aa((x + y).abs() * 0.70710677),
                _ => aa((x - y).abs() * 0.70710677),
            };
        }
        let level = index as f32 / (p.glyph_count - 1).max(1) as f32;
        if level == 0.0 {
            return 0.0;
        }
        let dot =
            aa((x * x + y * y).sqrt() - level * p.cell_width.min(p.cell_height) as f32 * 0.45);
        let cross = aa(x.abs()).max(aa(y.abs()));
        return dot.max(cross * ((level - 0.5) * 2.0).clamp(0.0, 1.0));
    }
    let width = 32 >> level;
    let height = 48 >> level;
    let origin = [(index % 16 * width) as f32, (index / 16 * height) as f32];
    let pos = [uv[0] * width as f32 - 0.5, uv[1] * height as f32 - 0.5];
    let floor = [pos[0].floor(), pos[1].floor()];
    let fract = [pos[0] - floor[0], pos[1] - floor[1]];
    let sample = |dx: f32, dy: f32| {
        f32::from(
            atlas.get_pixel(
                (origin[0] + (floor[0] + dx).clamp(0.0, (width - 1) as f32)) as u32,
                (origin[1] + (floor[1] + dy).clamp(0.0, (height - 1) as f32)) as u32,
            )[0],
        ) / 255.0
    };
    let top = sample(0.0, 0.0) * (1.0 - fract[0]) + sample(1.0, 0.0) * fract[0];
    let bottom = sample(0.0, 1.0) * (1.0 - fract[0]) + sample(1.0, 1.0) * fract[0];
    top * (1.0 - fract[1]) + bottom * fract[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use vestra_core::{
        ascii::GlyphAtlasSpec, stylization::EvaluatedPalette, validation::ResourceLimits,
    };
    fn parameters() -> AsciiParameters {
        AsciiParameters {
            atlas: 0,
            glyph_count: 10,
            glyph_style: AsciiGlyphStyle::Characters,
            mode: AsciiMode::Fill,
            color_mode: AsciiColorMode::Source,
            foreground: [255; 4],
            background: [0, 0, 0, 255],
            palette: EvaluatedPalette {
                colours: [[0, 0, 0, 255]; 16],
                len: 2,
            },
            invert: false,
            cell_width: 8,
            cell_height: 12,
            edge_threshold: 0.15,
            edge_strength: 1.0,
            source_mix: 0.0,
            amount: 1.0,
        }
    }
    fn atlas(characters: &str) -> std::sync::Arc<crate::ascii::PreparedGlyphAtlas> {
        crate::ascii::prepare(
            &GlyphAtlasSpec {
                font: None,
                characters: characters.into(),
                edge_characters: "-|/\\".into(),
            },
            &[],
            0,
            &ResourceLimits::default(),
        )
        .unwrap()
    }
    #[test]
    fn cell_means_ignore_hidden_rgb_and_include_partial_borders() {
        let p = parameters();
        let mut a = RgbaImage::from_pixel(9, 13, Rgba([255, 0, 0, 0]));
        let mut b = RgbaImage::from_pixel(9, 13, Rgba([0, 255, 0, 0]));
        for image in [&mut a, &mut b] {
            image.put_pixel(0, 0, Rgba([20, 40, 60, 128]));
            image.put_pixel(8, 12, Rgba([40, 80, 120, 255]));
        }
        assert_eq!(cell(&a, 0, 0, p), cell(&b, 0, 0, p));
        assert_eq!(cell(&a, 0, 0, p).0[..3], [20, 40, 60]);
        assert_eq!(cell(&a, 1, 1, p).0, [40, 80, 120, 255]);
        assert_eq!(cell(&a, 1, 0, p).0, [0; 4]);
    }
    #[test]
    fn transparent_background_intersects_source_alpha_and_identity_preserves_bytes() {
        let mut p = parameters();
        p.background = [0; 4];
        let source = RgbaImage::from_pixel(9, 13, Rgba([220, 220, 220, 128]));
        let mut analysis = RgbaImage::new(9, 13);
        let mut result = analysis.clone();
        let atlas = atlas(" .:-=+*#%@");
        analyze(&source, &mut analysis, p);
        resolve(&source, &analysis, &mut result, &atlas, p);
        assert!(result.pixels().all(|pixel| pixel[3] <= 128));
        assert!(result.pixels().any(|pixel| pixel[3] > 0 && pixel[3] < 128));
        p.source_mix = 1.0;
        resolve(&source, &analysis, &mut result, &atlas, p);
        assert_eq!(result, source);
        let transparent = RgbaImage::from_pixel(9, 13, Rgba([100, 90, 80, 0]));
        analyze(&transparent, &mut analysis, p);
        resolve(&transparent, &analysis, &mut result, &atlas, p);
        assert_eq!(result, transparent);
    }
    #[test]
    fn tiny_cells_preserve_sparse_glyph_coverage_and_single_pixel_images() {
        let mut p = parameters();
        p.glyph_count = 1;
        p.cell_width = 2;
        p.cell_height = 2;
        p.color_mode = AsciiColorMode::Monochrome;
        let atlas = atlas(".");
        let source = RgbaImage::from_pixel(3, 3, Rgba([255; 4]));
        let mut analysis = RgbaImage::new(3, 3);
        let mut output = analysis.clone();
        analyze(&source, &mut analysis, p);
        resolve(&source, &analysis, &mut output, &atlas, p);
        assert!(
            output.pixels().any(|pixel| pixel[0] > 0),
            "area-filtered period must remain visible at cell 2×2"
        );
        for (width, height) in [(1, 1), (1, 7), (7, 1)] {
            let source = RgbaImage::from_pixel(width, height, Rgba([255; 4]));
            let mut analysis = RgbaImage::new(width, height);
            let mut output = analysis.clone();
            analyze(&source, &mut analysis, p);
            resolve(&source, &analysis, &mut output, &atlas, p);
            assert_eq!(output.dimensions(), (width, height));
        }
    }
    #[test]
    fn density_transition_is_stateless_and_continuous_near_midpoint() {
        let mut p = parameters();
        p.glyph_count = 2;
        let below = cell(
            &RgbaImage::from_pixel(8, 12, Rgba([127, 127, 127, 255])),
            0,
            0,
            p,
        )
        .1;
        let above = cell(
            &RgbaImage::from_pixel(8, 12, Rgba([128, 128, 128, 255])),
            0,
            0,
            p,
        )
        .1;
        assert_eq!(below[0], above[0]);
        assert!(above[1] > below[1]);
        assert!(above[1] - below[1] <= 9);
    }
}
