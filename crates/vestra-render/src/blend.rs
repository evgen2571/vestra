use image::{Rgba, RgbaImage};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CompositionCaseCounts {
    pub(crate) source_alpha_zero: u64,
    pub(crate) source_alpha_opaque: u64,
    pub(crate) destination_alpha_zero: u64,
    pub(crate) destination_alpha_opaque: u64,
    pub(crate) general_partial_alpha: u64,
}

impl CompositionCaseCounts {
    pub(crate) fn record(&mut self, destination: Rgba<u8>, source: Rgba<u8>, opacity: f64) {
        let source_alpha = f64::from(source[3]) / 255.0 * opacity;
        if source_alpha == 0.0 {
            self.source_alpha_zero += 1;
        } else if source_alpha == 1.0 {
            self.source_alpha_opaque += 1;
        } else if destination[3] == 0 {
            self.destination_alpha_zero += 1;
        } else if destination[3] == u8::MAX {
            self.destination_alpha_opaque += 1;
        } else {
            self.general_partial_alpha += 1;
        }
    }
}

/// Composites straight-alpha source pixels over a straight-alpha destination.
pub(crate) fn source_over(destination: Rgba<u8>, source: Rgba<u8>, opacity: f64) -> Rgba<u8> {
    let source_alpha = f64::from(source[3]) / 255.0 * opacity;
    let destination_alpha = f64::from(destination[3]) / 255.0;
    if source_alpha == 0.0 {
        return if destination_alpha > 0.0 {
            destination
        } else {
            Rgba([0, 0, 0, 0])
        };
    }
    let alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    if alpha <= 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let mut result = [0; 4];
    for channel in 0..3 {
        result[channel] = ((f64::from(source[channel]) * source_alpha
            + f64::from(destination[channel]) * destination_alpha * (1.0 - source_alpha))
            / alpha)
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    result[3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgba(result)
}

pub(crate) fn blend_surface(
    canvas: &mut RgbaImage,
    source: &RgbaImage,
    mode: crate::project::BlendMode,
    opacity: f64,
    cases: Option<&mut CompositionCaseCounts>,
) {
    if matches!(mode, crate::project::BlendMode::Normal) {
        match cases {
            Some(cases) => {
                for (destination, source) in canvas.pixels_mut().zip(source.pixels()) {
                    cases.record(*destination, *source, opacity);
                    *destination = source_over(*destination, *source, opacity);
                }
            }
            None => {
                for (destination, source) in canvas.pixels_mut().zip(source.pixels()) {
                    *destination = source_over(*destination, *source, opacity);
                }
            }
        }
    } else {
        for (destination, source) in canvas.pixels_mut().zip(source.pixels()) {
            *destination = blend_pixel(*destination, *source, mode, opacity);
        }
    }
}

pub(crate) fn blend_pixel(
    destination: Rgba<u8>,
    source: Rgba<u8>,
    mode: crate::project::BlendMode,
    opacity: f64,
) -> Rgba<u8> {
    if matches!(mode, crate::project::BlendMode::Normal) {
        return source_over(destination, source, opacity);
    }
    let source_alpha = f64::from(source[3]) / 255.0 * opacity;
    let destination_alpha = f64::from(destination[3]) / 255.0;
    let alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    if alpha <= 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let mut result = [0; 4];
    for channel in 0..3 {
        let source_channel = f64::from(source[channel]) / 255.0;
        let destination_channel = f64::from(destination[channel]) / 255.0;
        let blended = match mode {
            crate::project::BlendMode::Normal => source_channel,
            crate::project::BlendMode::Add => (source_channel + destination_channel).min(1.0),
            crate::project::BlendMode::Screen => {
                1.0 - (1.0 - source_channel) * (1.0 - destination_channel)
            }
            crate::project::BlendMode::Multiply => source_channel * destination_channel,
            crate::project::BlendMode::Overlay => {
                if destination_channel <= 0.5 {
                    2.0 * source_channel * destination_channel
                } else {
                    1.0 - 2.0 * (1.0 - source_channel) * (1.0 - destination_channel)
                }
            }
        };
        let premultiplied = blended * source_alpha * destination_alpha
            + source_channel * source_alpha * (1.0 - destination_alpha)
            + destination_channel * destination_alpha * (1.0 - source_alpha);
        result[channel] = (premultiplied / alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    result[3] = (alpha * 255.0).round() as u8;
    Rgba(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_over_reference(destination: Rgba<u8>, source: Rgba<u8>, opacity: f64) -> Rgba<u8> {
        let source_alpha = f64::from(source[3]) / 255.0 * opacity;
        let destination_alpha = f64::from(destination[3]) / 255.0;
        let alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
        if alpha <= 0.0 {
            return Rgba([0, 0, 0, 0]);
        }
        let mut result = [0; 4];
        for channel in 0..3 {
            result[channel] = ((f64::from(source[channel]) * source_alpha
                + f64::from(destination[channel]) * destination_alpha * (1.0 - source_alpha))
                / alpha)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
        result[3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
        Rgba(result)
    }

    #[test]
    fn normal_blend_preserves_straight_alpha_composition() {
        assert_eq!(
            source_over(Rgba([0, 0, 255, 255]), Rgba([255, 0, 0, 128]), 1.0),
            Rgba([128, 0, 127, 255])
        );
    }

    #[test]
    fn source_over_fast_path_is_byte_identical_to_reference() {
        let colours = [
            [0, 0, 0],
            [255, 255, 255],
            [255, 0, 0],
            [0, 255, 0],
            [0, 0, 255],
            [17, 83, 211],
            [241, 129, 7],
        ];
        let opacities = [0.0, 1.0 / 255.0, 0.125, 0.5, 0.996_093_75, 1.0];
        for source_alpha in 0..=u8::MAX {
            for destination_alpha in 0..=u8::MAX {
                for opacity in opacities {
                    for source_rgb in colours {
                        for destination_rgb in colours {
                            let source =
                                Rgba([source_rgb[0], source_rgb[1], source_rgb[2], source_alpha]);
                            let destination = Rgba([
                                destination_rgb[0],
                                destination_rgb[1],
                                destination_rgb[2],
                                destination_alpha,
                            ]);
                            assert_eq!(
                                source_over(destination, source, opacity),
                                source_over_reference(destination, source, opacity),
                                "source alpha {source_alpha}, destination alpha {destination_alpha}, opacity {opacity}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn normal_surface_is_byte_identical_to_reference_surface() {
        let width = 37;
        let height = 29;
        let mut optimized = RgbaImage::new(width, height);
        let mut reference = RgbaImage::new(width, height);
        let source = RgbaImage::from_fn(width, height, |x, y| {
            Rgba([
                ((x * 17 + y * 3) % 256) as u8,
                ((x * 5 + y * 23) % 256) as u8,
                ((x * 31 + y * 7) % 256) as u8,
                ((x * 11 + y * 19) % 256) as u8,
            ])
        });
        for (index, pixel) in optimized.pixels_mut().enumerate() {
            *pixel = Rgba([
                ((index * 13) % 256) as u8,
                ((index * 29 + 7) % 256) as u8,
                ((index * 47 + 3) % 256) as u8,
                ((index * 61) % 256) as u8,
            ]);
        }
        reference.clone_from(&optimized);
        for opacity in [0.0, 0.125, 0.5, 0.996_093_75, 1.0] {
            let mut expected = reference.clone();
            for (destination, source) in expected.pixels_mut().zip(source.pixels()) {
                *destination = source_over_reference(*destination, *source, opacity);
            }
            let mut actual = reference.clone();
            blend_surface(
                &mut actual,
                &source,
                crate::project::BlendMode::Normal,
                opacity,
                None,
            );
            assert_eq!(actual, expected, "opacity {opacity}");
        }
    }

    #[test]
    fn blend_modes_are_distinct_with_transparent_pixels() {
        let destination = Rgba([100, 120, 140, 128]);
        let source = Rgba([180, 80, 40, 160]);
        let add = blend_pixel(destination, source, crate::project::BlendMode::Add, 1.0);
        let multiply = blend_pixel(
            destination,
            source,
            crate::project::BlendMode::Multiply,
            1.0,
        );
        let screen = blend_pixel(destination, source, crate::project::BlendMode::Screen, 1.0);
        let overlay = blend_pixel(destination, source, crate::project::BlendMode::Overlay, 1.0);
        assert_ne!(add, multiply);
        assert_ne!(screen, overlay);
        assert_eq!(
            blend_pixel(
                destination,
                Rgba([1, 2, 3, 0]),
                crate::project::BlendMode::Add,
                1.0
            ),
            destination
        );
    }

    #[test]
    fn every_declared_blend_mode_handles_partial_alpha() {
        let destination = Rgba([48, 160, 220, 96]);
        let source = Rgba([210, 80, 30, 144]);
        for mode in [
            crate::project::BlendMode::Normal,
            crate::project::BlendMode::Add,
            crate::project::BlendMode::Screen,
            crate::project::BlendMode::Multiply,
            crate::project::BlendMode::Overlay,
        ] {
            let pixel = blend_pixel(destination, source, mode, 0.6);
            assert!(pixel[3] >= destination[3], "{mode:?}");
            assert!(pixel[3] >= source[3] / 2, "{mode:?}");
        }
    }

    fn premultiplied_source_over(destination: [f64; 4], source: [f64; 4]) -> [f64; 4] {
        let alpha = source[3] + destination[3] * (1.0 - source[3]);
        if alpha <= 0.0 {
            return [0.0; 4];
        }
        [
            source[0] * source[3] + destination[0] * (1.0 - source[3]),
            source[1] * source[3] + destination[1] * (1.0 - source[3]),
            source[2] * source[3] + destination[2] * (1.0 - source[3]),
            alpha,
        ]
    }

    fn resolve_straight(premultiplied: [f64; 4]) -> [f64; 4] {
        if premultiplied[3] <= f64::EPSILON {
            return [0.0; 4];
        }
        [
            premultiplied[0] / premultiplied[3],
            premultiplied[1] / premultiplied[3],
            premultiplied[2] / premultiplied[3],
            premultiplied[3],
        ]
    }

    #[test]
    fn normal_reference_matches_premultiplied_accumulation_and_resolve() {
        let source = [100.0 / 255.0, 0.0, 0.0, 128.0 / 255.0];
        let first = premultiplied_source_over([0.0; 4], source);
        let resolved = resolve_straight(premultiplied_source_over(first, source));
        assert_eq!((resolved[0] * 255.0).round() as u8, 100);
        assert_eq!((resolved[3] * 255.0).round() as u8, 192);
        assert_eq!(
            source_over(Rgba([100, 0, 0, 128]), Rgba([100, 0, 0, 128]), 1.0),
            Rgba([100, 0, 0, 192])
        );
    }

    #[test]
    fn normal_resolve_zero_alpha_discards_rgb() {
        assert_eq!(resolve_straight([0.75, 0.25, 1.0, 0.0]), [0.0; 4]);
    }

    #[test]
    fn additive_reference_and_saturation_are_cpu_authoritative() {
        let unsaturated = Rgba([100, 0, 0, 128]);
        assert_eq!(
            blend_pixel(
                unsaturated,
                unsaturated,
                crate::project::BlendMode::Add,
                1.0
            ),
            Rgba([134, 0, 0, 192])
        );

        let destination = Rgba([220, 10, 180, 180]);
        let source = Rgba([200, 240, 160, 200]);
        let expected = blend_pixel(destination, source, crate::project::BlendMode::Add, 1.0);
        assert_eq!(expected, Rgba([236, 209, 219, 239]));
        assert!(f64::from(source[0]) / 255.0 + f64::from(destination[0]) / 255.0 > 1.0);
    }

    #[test]
    fn additive_covers_alpha_combinations_without_changing_cpu_semantics() {
        for (destination, source) in [
            (Rgba([0, 20, 30, 0]), Rgba([40, 50, 60, 128])),
            (Rgba([40, 50, 60, 255]), Rgba([100, 110, 120, 128])),
            (Rgba([240, 10, 10, 128]), Rgba([240, 20, 20, 64])),
        ] {
            let result = blend_pixel(destination, source, crate::project::BlendMode::Add, 1.0);
            let expected_alpha = (f64::from(source[3]) / 255.0
                + f64::from(destination[3]) / 255.0 * (1.0 - f64::from(source[3]) / 255.0))
                * 255.0;
            assert_eq!(result[3], expected_alpha.round() as u8);
        }
    }
}
