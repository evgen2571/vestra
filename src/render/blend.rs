use image::{Rgba, RgbaImage};

/// Composites straight-alpha source pixels over a straight-alpha destination.
pub(crate) fn source_over(destination: Rgba<u8>, source: Rgba<u8>, opacity: f64) -> Rgba<u8> {
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

pub(crate) fn blend_surface(
    canvas: &mut RgbaImage,
    source: &RgbaImage,
    mode: crate::project::BlendMode,
    opacity: f64,
) {
    for (destination, source) in canvas.pixels_mut().zip(source.pixels()) {
        *destination = blend_pixel(*destination, *source, mode, opacity);
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

    #[test]
    fn normal_blend_preserves_straight_alpha_composition() {
        assert_eq!(
            source_over(Rgba([0, 0, 255, 255]), Rgba([255, 0, 0, 128]), 1.0),
            Rgba([128, 0, 127, 255])
        );
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
}
