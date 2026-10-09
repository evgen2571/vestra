//! Sparse palette-input lattice with alpha-aware encoded-byte filtering.

use image::{Rgba, RgbaImage};
use vestra_core::project::PaletteInputFilter;

pub(super) fn analyze(
    source: &RgbaImage,
    target: &mut RgbaImage,
    scale: u16,
    filter: PaletteInputFilter,
) {
    target.fill(0);
    let scale = u32::from(scale);
    for y in (0..source.height()).step_by(scale as usize) {
        for x in (0..source.width()).step_by(scale as usize) {
            let width = scale.min(source.width() - x);
            let height = scale.min(source.height() - y);
            let mut sum = [0_u32; 4];
            let mut count = 0;
            let mut add = |sx, sy| {
                let p = source.get_pixel(sx, sy);
                for c in 0..3 {
                    sum[c] += u32::from(p[c]) * u32::from(p[3]);
                }
                sum[3] += u32::from(p[3]);
                count += 1;
            };
            match filter {
                PaletteInputFilter::Nearest => add(x + width / 2, y + height / 2),
                PaletteInputFilter::Linear => {
                    for sy in [y + (height - 1) / 2, y + height / 2] {
                        for sx in [x + (width - 1) / 2, x + width / 2] {
                            add(sx, sy);
                        }
                    }
                }
                PaletteInputFilter::Area => {
                    for sy in y..y + height {
                        for sx in x..x + width {
                            add(sx, sy);
                        }
                    }
                }
            }
            let pixel = if sum[3] == 0 {
                Rgba([0; 4])
            } else {
                Rgba([
                    ((sum[0] + sum[3] / 2) / sum[3]) as u8,
                    ((sum[1] + sum[3] / 2) / sum[3]) as u8,
                    ((sum[2] + sum[3] / 2) / sum[3]) as u8,
                    ((sum[3] + count / 2) / count) as u8,
                ])
            };
            target.put_pixel(x, y, pixel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn center_and_area_filters_have_distinct_literal_results() {
        let source = RgbaImage::from_fn(4, 2, |x, y| {
            let value = [0, 40, 100, 200][x as usize] + 10 * y as u8;
            Rgba([value, value, value, 255])
        });
        for (filter, expected) in [
            (PaletteInputFilter::Nearest, 110),
            (PaletteInputFilter::Linear, 75),
            (PaletteInputFilter::Area, 90),
        ] {
            let mut output = RgbaImage::new(4, 2);
            analyze(&source, &mut output, 4, filter);
            assert_eq!(
                output.get_pixel(0, 0).0,
                [expected, expected, expected, 255]
            );
            assert!(
                output
                    .enumerate_pixels()
                    .filter(|(x, y, _)| *x != 0 || *y != 0)
                    .all(|(_, _, p)| p.0 == [0; 4])
            );
        }
    }

    #[test]
    fn filtered_colors_ignore_transparent_hidden_rgb() {
        let source = RgbaImage::from_fn(2, 2, |x, y| {
            if x == 0 && y == 0 {
                Rgba([20, 40, 60, 128])
            } else {
                Rgba([255, 0, 0, 0])
            }
        });
        for filter in [PaletteInputFilter::Linear, PaletteInputFilter::Area] {
            let mut output = RgbaImage::new(2, 2);
            analyze(&source, &mut output, 2, filter);
            assert_eq!(output.get_pixel(0, 0).0, [20, 40, 60, 32]);
        }
        let mut output = RgbaImage::new(2, 2);
        analyze(&source, &mut output, 2, PaletteInputFilter::Nearest);
        assert_eq!(output.get_pixel(0, 0).0, [0; 4]);
    }

    #[test]
    fn partial_cells_use_actual_extents_and_upward_half_ties() {
        let source = RgbaImage::from_fn(6, 3, |x, y| {
            let p = x as u8 + 10 * y as u8;
            Rgba([p, p, p, 255])
        });
        for filter in [
            PaletteInputFilter::Nearest,
            PaletteInputFilter::Linear,
            PaletteInputFilter::Area,
        ] {
            let mut output = RgbaImage::new(6, 3);
            analyze(&source, &mut output, 4, filter);
            assert_eq!(output.get_pixel(0, 0).0, [12, 12, 12, 255]);
            assert_eq!(output.get_pixel(4, 0).0, [15, 15, 15, 255]);
        }
    }

    #[test]
    fn maximum_cell_accumulation_is_bounded_and_preserves_white() {
        assert!(65536_u64 * 255 * 255 + 65536 * 255 / 2 < u64::from(u32::MAX));
        let source = RgbaImage::from_pixel(256, 256, Rgba([255; 4]));
        let mut output = RgbaImage::new(256, 256);
        analyze(&source, &mut output, 256, PaletteInputFilter::Area);
        assert_eq!(output.get_pixel(0, 0).0, [255; 4]);
    }
}
