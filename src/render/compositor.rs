use image::{Rgba, RgbaImage};

use crate::{
    plan::{RenderPlan, ScheduledItem},
    render::{animation, prepared::PreparedAssets},
};

pub fn compose(
    plan: &RenderPlan,
    assets: &mut PreparedAssets,
    active: &[ScheduledItem],
    time: u128,
) -> RgbaImage {
    let mut canvas = RgbaImage::from_pixel(
        plan.canvas.width,
        plan.canvas.height,
        Rgba(plan.canvas.background),
    );
    for item in active {
        match *item {
            ScheduledItem::Clip(index) => draw_clip(&mut canvas, plan, assets, index, time),
            ScheduledItem::Flash(index) => draw_flash(&mut canvas, &plan.flashes[index], time),
        }
    }
    canvas
}

fn draw_clip(
    canvas: &mut RgbaImage,
    plan: &RenderPlan,
    assets: &mut PreparedAssets,
    index: usize,
    time: u128,
) {
    let clip = &plan.clips[index];
    let time_seconds = time as f64 / 1_000_000_000.0;
    let relative = time_seconds - clip.start_nanos as f64 / 1_000_000_000.0;
    let (position, crop, opacity, scale) = animation::properties(
        &clip.animations,
        clip.position,
        clip.crop,
        clip.opacity,
        relative,
    );
    let opacity = opacity * animation::transition_opacity(&clip.transitions, time_seconds);
    if opacity <= 0.0 {
        return;
    }
    let bitmap = assets.bitmap_for(plan, index, crop, scale);
    let x = (position.x * f64::from(canvas.width()) - clip.anchor.x * f64::from(bitmap.width()))
        .round() as i64;
    let y = (position.y * f64::from(canvas.height()) - clip.anchor.y * f64::from(bitmap.height()))
        .round() as i64;
    blend_at(canvas, bitmap, x, y, opacity);
}

fn draw_flash(canvas: &mut RgbaImage, flash: &crate::plan::CompiledFlash, time: u128) {
    let time_seconds = time as f64 / 1_000_000_000.0;
    let start = flash.start_nanos as f64 / 1_000_000_000.0;
    let duration = flash.end_nanos.saturating_sub(flash.start_nanos) as f64 / 1_000_000_000.0;
    let relative = time_seconds - start;
    let fade_in = flash.fade_in_nanos as f64 / 1_000_000_000.0;
    let fade_out = flash.fade_out_nanos as f64 / 1_000_000_000.0;
    let attack = if fade_in > 0.0 && relative < fade_in {
        relative / fade_in
    } else {
        1.0
    };
    let release_start = duration - fade_out;
    let release = if fade_out > 0.0 && relative > release_start {
        (duration - relative) / fade_out
    } else {
        1.0
    };
    let opacity = flash.opacity * attack.min(release).clamp(0.0, 1.0);
    for destination in canvas.pixels_mut() {
        *destination = source_over(*destination, Rgba(flash.colour), opacity);
    }
}

fn blend_at(
    canvas: &mut RgbaImage,
    source: &RgbaImage,
    offset_x: i64,
    offset_y: i64,
    opacity: f64,
) {
    for (x, y, pixel) in source.enumerate_pixels() {
        let dx = offset_x + i64::from(x);
        let dy = offset_y + i64::from(y);
        if dx < 0 || dy < 0 || dx >= i64::from(canvas.width()) || dy >= i64::from(canvas.height()) {
            continue;
        }
        let destination = canvas.get_pixel_mut(dx as u32, dy as u32);
        *destination = source_over(*destination, *pixel, opacity);
    }
}

pub fn source_over(destination: Rgba<u8>, source: Rgba<u8>, opacity: f64) -> Rgba<u8> {
    let source_alpha = f64::from(source[3]) / 255.0 * opacity;
    let destination_alpha = f64::from(destination[3]) / 255.0;
    let alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    if alpha <= 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let mut result = [0; 4];
    for channel in 0..3 {
        let value = (f64::from(source[channel]) * source_alpha
            + f64::from(destination[channel]) * destination_alpha * (1.0 - source_alpha))
            / alpha;
        result[channel] = value.round().clamp(0.0, 255.0) as u8;
    }
    result[3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgba(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alpha_composition_is_known() {
        assert_eq!(
            source_over(Rgba([0, 0, 255, 255]), Rgba([255, 0, 0, 128]), 1.0),
            Rgba([128, 0, 127, 255])
        );
    }

    #[test]
    fn direct_flash_overlay_matches_source_over() {
        let mut canvas = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 255, 255]));
        let flash = crate::plan::CompiledFlash {
            start_nanos: 0,
            end_nanos: 2,
            start_frame: 0,
            end_frame: 1,
            draw_key: crate::plan::DrawKey {
                layer: 0,
                start_nanos: 0,
                id: "flash".to_owned(),
                kind: crate::plan::ItemKind::Flash,
            },
            colour: [255, 0, 0, 128],
            opacity: 1.0,
            fade_in_nanos: 0,
            fade_out_nanos: 0,
        };
        draw_flash(&mut canvas, &flash, 0);
        assert_eq!(*canvas.get_pixel(0, 0), Rgba([128, 0, 127, 255]));
    }
}
